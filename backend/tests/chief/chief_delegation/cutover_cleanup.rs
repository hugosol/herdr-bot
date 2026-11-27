use super::*;
use sha2::{Digest, Sha256};

// P57 (S1/S4): real chief and Map delegation before cutover; preserved Chats
// can still be recovered, but neither Chat history nor an archived Run can
// stand in for the old delegation or grant live management after cleanup.
#[tokio::test]
async fn cutover_rebuilds_current_chief_without_old_report_or_run_authority() {
    let (f, _, request) = map::map_fixture().await;
    let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
    let commit = json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string();
    let started = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &commit).await, 201);
    let run_id = started["id"].as_str().unwrap();
    let outstanding = f.preview().await;
    let unconfirmed = json!({"sessionId":"chief-a","previewId":outstanding["previewId"]}).to_string();
    let manage = json!({"sessionId":"flow-manager"});
    let status = post(&f.server, "/api/workflow/manager/status", manage.clone()).await;
    assert_eq!(status["snapshot"]["id"], run_id);
    let artifact = f.root.path().join("work/retained-output.txt");
    fs::write(&artifact, b"Existing work preserved during cutover\r\n").unwrap();
    let mut content = reports::content("blocked");
    content["deliverables"] = json!([artifact]);
    let submission = json!({"sessionId":"flow-manager","body":content}).to_string();
    let saved = body(post_json_over_tcp(f.server.addr(), "/api/chief/reports", &submission).await, 200);
    assert_eq!(saved["saved"], true);
    let report_id = saved["report"]["id"].as_str().unwrap();
    let receive = format!("/api/chief/reports/{report_id}/receive");
    assert_eq!(post(&f.server, &receive, json!({"sessionId":"chief-a"})).await["process"], true);
    let history = body(get_over_tcp(f.server.addr(), "/api/sessions/history").await, 200);
    let root = f.root.path();
    let chat_files = ["history.json", "chief-a.jsonl", "flow-manager.jsonl", "materials/brief.txt", "zones.json", "work/retained-output.txt"];
    let bytes = chat_files.map(|name| fs::read(root.join(name)).unwrap());
    let online_run = f.config.scheduler.run_directory.join(run_id);
    let records = ["run.json", "map.json", "history.jsonl", "messages.jsonl"];
    let run_bytes = records.map(|name| fs::read(online_run.join(name)).unwrap());
    let fresh_request = f.request();
    drop(f.server);

    let archive = root.join("offline-runs");
    fs::create_dir(&archive).unwrap();
    fs::rename(&online_run, archive.join(run_id)).unwrap();
    for extension in ["chief.json", "delegations.json", "chief-reports.jsonl", "chief-reports.receipts.jsonl"] {
        fs::remove_file(f.config.sessions.history_file.with_extension(extension)).unwrap();
    }
    let server = TestServer::start(&f.config).await;
    let addr = server.addr();
    let chief_session = json!({"sessionId":"chief-a"});
    assert!(body(get_over_tcp(addr, "/api/chief").await, 200)["current"].is_null());
    let not_current = body(post_json_over_tcp(addr, "/api/chief/qualify", &chief_session.to_string()).await, 409);
    assert_eq!(not_current["code"], "chief-not-current");
    let prompt_count = f.herdr.calls().into_iter().filter(|call| call["kind"] == "agent_prompt").count();
    f.herdr.set_workspaces(json!([{"workspace_id":"cutover-resume","label":f.config.herdr.workspace_name}]));
    for session in ["chief-a", "flow-manager"] {
        f.herdr.set_behavior(json!({"agents":[], "native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":root.join(format!("{session}.jsonl"))}}));
        let resumed = post(&server, "/api/chats/resume", json!({"sessionId":session})).await;
        assert_eq!(resumed["outcome"], "restored", "{resumed}");
        if session == "chief-a" {
            let pending = body(get_over_tcp(addr, "/api/chief").await, 200);
            assert_eq!(pending["current"]["phase"], "awaiting-registration");
        }
        post(&server, "/api/sessions/events", json!({"sessionId":session,"kind":"register","data":{"cwd":root.join("work"),"agentType":"pi"}})).await;
    }
    let qualified = post(&server, "/api/chief/qualify", chief_session.clone()).await;
    assert_eq!(qualified["current"]["sessionId"], "chief-a");
    assert_eq!(qualified["current"]["phase"], "registered");
    let before_rejections = f.herdr.calls();
    let old_preview = body(post_json_over_tcp(addr, "/api/chief/delegations", &unconfirmed).await, 409);
    assert_eq!(old_preview["code"], "delegation-preview-missing");
    assert_eq!(body(get_over_tcp(addr, "/api/chief/delegations?sessionId=flow-manager").await, 200)["delegations"], json!([]));
    let denied = body(post_json_over_tcp(addr, "/api/chief/reports", &submission).await, 403);
    assert_eq!(denied["code"], "chief-report-unassociated");
    for path in ["view", "receive", "resend"] {
        let action = if path == "resend" { &manage } else { &chief_session };
        let missing = body(post_json_over_tcp(addr, &format!("/api/chief/reports/{report_id}/{path}"), &action.to_string()).await, 404);
        assert_eq!(missing["code"], "chief-report-not-found");
    }
    let manager_denied = body(post_json_over_tcp(addr, "/api/workflow/manager/status", &manage.to_string()).await, 403);
    assert_eq!(manager_denied["code"], "manager-unassociated");
    let finish_denied = body(post_json_over_tcp(addr, "/api/workflow/manager/finish", &manage.to_string()).await, 403);
    assert_eq!(finish_denied["code"], "manager-unassociated");
    assert_eq!(body(get_over_tcp(addr, "/api/workflow/runs").await, 200)["runs"], json!([]));
    assert_eq!(body(get_over_tcp(addr, "/api/chief/reports").await, 200)["reports"], json!([]));
    assert_eq!(f.herdr.calls(), before_rejections);
    assert_eq!(before_rejections.iter().filter(|call| call["kind"] == "agent_prompt").count(), prompt_count, "restore and registration must not replay business or reports");
    let available = post(&server, "/api/chief/delegations/preview", fresh_request).await;
    assert_eq!(available["facts"]["request"]["sessionId"], "chief-a");
    assert_ne!(available["previewId"], outstanding["previewId"]);
    assert_eq!(f.herdr.calls(), before_rejections);
    let current_history = body(get_over_tcp(addr, "/api/sessions/history").await, 200);
    for id in ["chief-a", "flow-manager"] {
        assert!(history["sessions"].as_array().unwrap().iter().any(|row| row["sessionId"] == id));
        assert!(current_history["sessions"].as_array().unwrap().iter().any(|row| row["sessionId"] == id));
    }
    for (name, expected) in chat_files.into_iter().zip(bytes) {
        assert_eq!(fs::read(root.join(name)).unwrap(), expected, "retained {name}");
    }
    for (name, expected) in records.into_iter().zip(run_bytes) {
        assert_eq!(fs::read(archive.join(run_id).join(name)).unwrap(), expected, "archived {name}");
    }
    println!("P57 restored and registered chief-a; old delegation, report {report_id}, and Run {run_id} rejected; actual native Chat and material bytes retained");
}

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
