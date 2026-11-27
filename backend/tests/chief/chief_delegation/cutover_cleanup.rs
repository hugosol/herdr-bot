use super::*;
use sha2::{Digest, Sha256};

// P56, S1/S4. Before deleting anything, produce all four chief records through
// the public operations. Deleting the history's containing directory or sweeping
// history.* would destroy a negative control even if the new reports list is empty.
#[tokio::test]
async fn cutover_removes_only_chief_records_and_preserves_chat_zone_and_artifact_bytes() {
    let f = Fixture::new().await;
    reports::delegated(&f).await;
    reports::notification_behavior(&f, json!({}));
    let artifact = f.root.path().join("work/business.bin");
    let artifact_bytes = b"\x00business result\r\n\xff";
    fs::write(&artifact, artifact_bytes).unwrap();
    let mut report = reports::content("success");
    report["deliverables"] = json!([artifact]);
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":report})).await;
    assert_eq!(saved["saved"], true);
    let id = saved["report"]["id"].as_str().unwrap();
    let receipt = post(&f.server, &format!("/api/chief/reports/{id}/receive"), json!({"sessionId":"chief-a"})).await;
    assert_eq!(receipt["process"], true);
    assert_eq!(receipt["receipt"]["sessionId"], "chief-a");
    let history = body(get_over_tcp(f.server.addr(), "/api/sessions/history").await, 200);
    let zones = body(get_over_tcp(f.server.addr(), "/api/zones").await, 200);
    for session in ["chief-a", "worker-a"] {
        assert!(history["sessions"].as_array().unwrap().iter().any(|row| row["sessionId"] == session));
    }
    let unrelated = f.root.path().join("history.other.json");
    fs::write(&unrelated, b"{\"unrelated\":true}\r\n").unwrap();
    drop(f.server); // the owned real backend has exited before disk operations
    let root = f.root.path();
    let retain = ["history.json", "chief-a.jsonl", "worker-a.jsonl", "zones.json", "work/business.bin", "history.other.json"];
    let remove = ["history.chief.json", "history.delegations.json", "history.chief-reports.jsonl", "history.chief-reports.receipts.jsonl"];
    let before: Vec<_> = retain.iter().chain(&remove).map(|name| (*name, fs::read(root.join(name)).unwrap())).collect();
    assert_eq!(fs::read(&artifact).unwrap(), artifact_bytes);
    for (name, bytes) in &before {
        assert!(!bytes.is_empty(), "missing pre-cutover fact: {name}");
        println!("P56 before {name}: {} bytes, sha256={:x}", bytes.len(), Sha256::digest(bytes));
    }
    // Rehearse the explicit-path filesystem operations, not a new service API.
    fs::remove_file(root.join("history.chief.json")).unwrap();
    fs::remove_file(root.join("history.delegations.json")).unwrap();
    fs::remove_file(root.join("history.chief-reports.jsonl")).unwrap();
    fs::remove_file(root.join("history.chief-reports.receipts.jsonl")).unwrap();
    let recovered = TestServer::start(&f.config).await;
    assert_eq!(body(get_over_tcp(recovered.addr(), "/api/chief/reports").await, 200)["reports"], json!([]));
    assert_eq!(body(get_over_tcp(recovered.addr(), "/api/sessions/history").await, 200), history);
    assert_eq!(body(get_over_tcp(recovered.addr(), "/api/zones").await, 200), zones);
    for (name, bytes) in before {
        if retain.contains(&name) {
            let current = fs::read(root.join(name)).unwrap();
            assert_eq!(current, bytes, "retained {name}");
            println!("P56 retained {name}: sha256={:x}", Sha256::digest(&current));
        } else {
            assert_eq!(fs::metadata(root.join(name)).unwrap_err().kind(), std::io::ErrorKind::NotFound, "removed {name}");
            println!("P56 removed {name}");
        }
    }
}
