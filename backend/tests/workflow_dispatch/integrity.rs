//! P47/P48 approved seams: real HTTP, public records, controlled external host.
//!
//! The gate below introduces corruption after admission, before the append.
//! P47.01 requires the log stay unchanged even when a Chat already started;
//! successful current-state commit still has to be reported as partial effect.
#[tokio::test]
async fn integrity_dispatch_never_appends_to_damaged_history_after_external_wait() {
    for broken in ["{\"action\":\"unfinished\"}", "{bad-history}\n"] {
        let s = Scenario::new().await;
        let started = s.start("integrity-manager").await;
        let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
        let release = s.data.path().join("integrity-release");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"integrity-node"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
        let dispatch = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", r#"{"sessionId":"integrity-manager","nodes":["A"]}"#).await;
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2).await);
        fs::write(root.join("history.jsonl"), broken).unwrap();
        fs::write(&release, b"release").unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), dispatch.response()).await.unwrap();
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), broken.as_bytes(), "do not concatenate onto damaged log");
        assert_eq!(value["results"][0]["stateCommitted"], true);
        assert_eq!(value["results"][0]["code"], "history-append-failed");
        assert!(value["results"][0]["historyError"].as_str().unwrap().contains("history.jsonl"));
        assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "running");
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), value["snapshot"]);
        println!("P47.01 damagedHistory={broken:?} partial={value} calls={:?}", s.herdr.calls());
    }
}
use super::*;

// P48.01: a real external start is gated, not a mocked file operation. The
// same collection stays readable and a disjoint Run commits pause before the
// gate releases. A global/file lock held across host startup would time out.
#[tokio::test]
async fn integrity_external_start_keeps_same_run_readable_and_disjoint_run_writable() {
    let s = Scenario::new().await;
    let first = s.start("integrity-manager").await;
    let other = s.start("integrity-other-manager").await;
    let release = s.data.path().join("external-start-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"integrity-node"},"native_cwd":s.data.path().join("node-zone"),"agent_start_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let dispatch = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", r#"{"sessionId":"integrity-manager","nodes":["A"]}"#).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_start").count() == 3).await);
    let pending = dispatch.response();
    tokio::pin!(pending);
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut pending).await.is_err());
    let query = tokio::time::timeout(Duration::from_secs(2), get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", first["id"].as_str().unwrap()))).await.expect("start wait cannot own file access");
    assert_eq!(query.status, 200, "{}", query.body);
    assert_eq!(serde_json::from_str::<Value>(&query.body).unwrap()["run"]["nodes"]["A"]["status"], "pending");
    let paused = tokio::time::timeout(Duration::from_secs(2), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"integrity-other-manager"}"#)).await.expect("disjoint Run must commit during host wait");
    assert_eq!(paused.status, 200, "{}", paused.body);
    let paused: Value = serde_json::from_str(&paused.body).unwrap();
    assert_eq!(paused["snapshot"]["id"], other["id"]);
    assert_eq!(paused["snapshot"]["status"], "paused");
    fs::write(&release, b"release").unwrap();
    let launched = tokio::time::timeout(Duration::from_secs(5), pending).await.unwrap();
    assert_eq!(launched.status, 200, "{}", launched.body);
    assert_eq!(serde_json::from_str::<Value>(&launched.body).unwrap()["results"][0]["status"], "running");
    println!("P48.01 externalStart observationWindowMs=100 during={} disjoint={} launched={} calls={:?}", query.body, paused, launched.body, s.herdr.calls());
}

// P48.01: deny DELETE sharing on the actual Windows target, but allow reads
// and writes. A delete-first or truncate-in-place writer would lose/change the
// committed bytes; a same-directory replacement fails and preserves them.
// Releasing the handle must allow the next explicit replacement to succeed.
#[cfg(windows)]
#[tokio::test]
async fn integrity_windows_replace_failure_keeps_formal_record_and_next_replace_succeeds() {
    use std::{io::Read, os::windows::fs::OpenOptionsExt};
    let s = Scenario::new().await;
    let started = s.start("integrity-manager").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("run.json");
    let original = fs::read(&path).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    // FILE_SHARE_READ | FILE_SHARE_WRITE, deliberately not FILE_SHARE_DELETE.
    let mut held = fs::OpenOptions::new().read(true).share_mode(3).open(&path).unwrap();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"integrity-manager"}"#).await;
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(path.is_file(), "formal file must never be removed on failure");
    assert_eq!(fs::read(&path).unwrap(), original);
    let mut held_bytes = Vec::new();
    held.read_to_end(&mut held_bytes).unwrap();
    assert_eq!(held_bytes, original, "do not truncate/overwrite the old file in place");
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let query = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(query.status, 200, "{}", query.body);
    assert_eq!(serde_json::from_str::<Value>(&query.body).unwrap()["run"]["status"], "running");
    drop(held);
    let next = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"integrity-manager"}"#).await;
    assert_eq!(next.status, 200, "{}", next.body);
    let next: Value = serde_json::from_str(&next.body).unwrap();
    assert_eq!(next["snapshot"]["status"], "paused");
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(), next["snapshot"]);
    let mut files = fs::read_dir(&root).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, vec!["history.jsonl", "messages.jsonl", "run.json"]);
    println!("P48.01 Windows deny-delete failure={} originalBytes={} next={} files={files:?}", response.body, original.len(), next);
}

// P48.01: real readers overlap repeated replacements. A delete-first writer,
// partial JSON publication, or uncoordinated append would produce HTTP failures,
// invalid JSON, or history other than these forty explicit transitions.
#[tokio::test]
async fn integrity_concurrent_reads_observe_complete_records_during_replacements() {
    let s = Scenario::new().await;
    let started = s.start("integrity-manager").await;
    let addr = s.server.addr();
    let id = started["id"].as_str().unwrap().to_owned();
    let reader_id = id.clone();
    let readers = tokio::spawn(async move {
        for _ in 0..80 {
            let response = get_over_tcp(addr, &format!("/api/workflow/runs/{reader_id}")).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let detail: Value = serde_json::from_str(&response.body).unwrap();
            assert!(matches!(detail["run"]["status"].as_str(), Some("running" | "paused")));
            assert_eq!(detail["reports"], json!([]));
        }
    });
    for _ in 0..20 {
        for name in ["pause", "resume"] {
            let response = post_json_over_tcp(addr, &format!("/api/workflow/manager/{name}"), r#"{"sessionId":"integrity-manager"}"#).await;
            assert_eq!(response.status, 200, "{}", response.body);
            assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["changed"], true);
        }
    }
    tokio::time::timeout(Duration::from_secs(10), readers).await.unwrap().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let history = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(history.len(), 40);
    assert!(history.chunks_exact(2).all(|pair| pair[0]["action"] == "pause" && pair[1]["action"] == "resume"));
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap()["status"], "running");
    println!("P48.01 concurrentReads=80 replacements=40 finalStatus=running history={history:?}");
}

// P47.01: validation stays ahead of actual message/history appends. The public
// result must explain failure without rewriting bad bytes or notifying a host.
#[tokio::test]
async fn integrity_message_and_transition_appends_preserve_damaged_logs() {
    let s = Scenario::new().await;
    let started = s.start("integrity-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"integrity-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("integrity-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let calls = s.herdr.calls();
    let current = fs::read(root.join("run.json")).unwrap();
    fs::remove_file(root.join("messages.jsonl")).unwrap();
    let missing = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"integrity-node","body":"must not be recreated"}"#).await;
    assert_eq!(missing.status, 500, "{}", missing.body);
    assert!(missing.body.contains("messages.jsonl"), "{}", missing.body);
    assert!(!root.join("messages.jsonl").exists());
    for broken in ["{\"body\":\"unfinished\"}", "{bad-report}\n", "{}\n"] {
        fs::write(root.join("messages.jsonl"), broken).unwrap();
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"integrity-node","body":"must not be appended"}"#).await;
        assert_eq!(response.status, 500, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], false);
        assert_eq!(value["notification"]["status"], "not-attempted");
        assert!(value["message"].as_str().unwrap().contains("messages.jsonl"));
        assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), broken.as_bytes());
        assert_eq!(fs::read(root.join("run.json")).unwrap(), current);
        println!("P47.01 brokenMessages={broken:?} response={value}");
    }
    fs::write(root.join("messages.jsonl"), b"").unwrap();
    let broken = b"{\"action\":\"unfinished\"}";
    fs::write(root.join("history.jsonl"), broken).unwrap();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"integrity-manager"}"#).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["stateCommitted"], true);
    assert_eq!(value["snapshot"]["status"], "paused");
    assert!(value["historyError"].as_str().unwrap().contains("history.jsonl"));
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), broken);
    assert_eq!(s.herdr.calls(), calls);
    println!("P47.01 brokenTransition={value} externalCalls={calls:?}");
}

// P47.01: missing required files stay missing; malformed current/report records
// stay byte-identical. Path-bearing HTTP errors, not empty successful snapshots,
// are the independent contract outcome. Filesystem is real, no storage double.
#[tokio::test]
async fn integrity_missing_and_corrupt_required_records_are_never_recreated() {
    let s = Scenario::new().await;
    let started = s.start("integrity-manager").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let names = ["run.json", "history.jsonl", "messages.jsonl"];
    let original = names.map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    for (index, name) in names.iter().enumerate() {
        fs::remove_file(root.join(name)).unwrap();
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 500, "{name}: {}", response.body);
        assert!(response.body.contains(name), "identify missing file: {}", response.body);
        assert!(!root.join(name).exists());
        let status = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", r#"{"sessionId":"integrity-manager"}"#).await;
        assert_eq!(status.status, 500, "missing record is not missing identity: {}", status.body);
        assert!(status.body.contains(name), "{}", status.body);
        for (other, bytes) in names.iter().zip(&original).filter(|(other, _)| *other != name) {
            assert_eq!(fs::read(root.join(other)).unwrap(), *bytes);
        }
        fs::write(root.join(name), &original[index]).unwrap();
        println!("P47.01 missing={name} response={}", response.body);
    }
    for (name, broken) in [("run.json", "{bad-current"), ("run.json", "{}"), ("messages.jsonl", "{bad-report}\n"), ("messages.jsonl", "{}\n"), ("messages.jsonl", "{\"body\":\"unfinished\"}")] {
        let index = names.iter().position(|item| *item == name).unwrap();
        fs::write(root.join(name), broken).unwrap();
        let before = names.map(|name| fs::read(root.join(name)).unwrap());
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 500, "{name}: {}", response.body);
        assert!(response.body.contains(name), "identify corruption: {}", response.body);
        assert_eq!(names.map(|name| fs::read(root.join(name)).unwrap()), before);
        fs::write(root.join(name), &original[index]).unwrap();
        println!("P47.01 corrupt={name} response={}", response.body);
    }
    assert_eq!(s.herdr.calls(), calls);
}

// P47.01: history corruption must be reported, not silently ignored by detail.
// Independent literal malformed records and unterminated JSON distinguish a
// reader that only checks file existence. Every observed byte must survive.
#[tokio::test]
async fn integrity_corrupt_history_is_rejected_without_changing_any_record() {
    let s = Scenario::new().await;
    let started = s.start("integrity-manager").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    for broken in [b"{not-json}\n".as_slice(), b"{\"action\":\"pause\"}".as_slice(), b"null\n".as_slice()] {
        fs::write(root.join("history.jsonl"), broken).unwrap();
        let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 500, "corrupt history must not look healthy: {}", response.body);
        assert!(response.body.contains("history.jsonl"), "identify the damaged record: {}", response.body);
        assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
        println!("P47.01 corruptHistory={broken:?} response={}", response.body);
    }
}
