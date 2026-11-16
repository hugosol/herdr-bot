//! P13/P14/P19: approved S1 manager HTTP, real Map/files and controlled host.
use super::*;

async fn action(s: &Scenario, name: &str, extra: Value, status: u16) -> Value {
    let mut body = json!({"sessionId":"failure-manager"});
    if name != "collect" { body["nodeId"] = json!("create"); }
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    request(s, &format!("/api/workflow/manager/{name}"), body, status).await
}
async fn reports(s: &Scenario, run: &Value, count: usize) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let read = detail(s, run).await;
        if read["reports"].as_array().unwrap().len() == count {
            let ids = read["reports"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                ids.iter().all(|id| s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.contains(id))))
                    && fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok()
            }).await, "owned notification processes must exit");
            return read;
        }
        assert!(tokio::time::Instant::now() < deadline, "missing reports: {read}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// Expected: partial material failure reports false but leaves node running;
// only explicit fail + confirmed retry creates fresh work. A Chat-only fail
// guard, automatic failure/retry, lost old baseline, or output inheritance is
// distinguished by public states, held real reads and two actual consumers.
#[tokio::test]
async fn rust_retry_requires_explicit_failure_and_preserves_partial_effects_and_fresh_materials() {
    let s = Scenario::new().await;
    creator(&s).await;
    let target = target_task(&s).await;
    let mut input = business_request(json!({"type":"task","task_id":target["id"]}));
    fs::write(s.data.path().join("request.json"), input.to_string()).unwrap();
    map_for_request(&s, "Business comes only from the named request").await;
    let mut map: Value = serde_json::from_slice(&fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap()).unwrap();
    map["nodes"]["old_consumer"] = json!({"title":"Read partial baseline independently","bot_id":"worker","input":"Read the actual named baseline","input_rule":[{"id":"baseline","type":"node","node_id":"create","output_id":"baseline"}],"zone":"node","blocked_by":[]});
    fs::write(s.config.scheduler.map_directory.join("work.json"), map.to_string()).unwrap();
    let run = s.start("failure-manager").await;
    let gate = super::super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file);
    let launched = s.dispatch("failure-manager", &["create"]).await;
    let old_id = launched["snapshot"]["nodes"]["create"]["execution"]["id"].as_str().unwrap();
    assert!(common::wait_until(Duration::from_secs(5), || gate.reached()).await);
    let collision = s.data.path().join(".manager-zone/.workflow").join(run["id"].as_str().unwrap()).join(old_id).join("handoff.json");
    fs::create_dir_all(collision.parent().unwrap()).unwrap();
    fs::write(&collision, b"UNOWNED HANDOFF").unwrap();
    let before = detail(&s, &run).await;
    assert_eq!(before["run"]["nodes"]["create"]["status"], "running");
    drop(gate);
    let failed = reports(&s, &run, 1).await;
    let old = failed["run"]["nodes"]["create"]["execution"].clone();
    let report = &failed["reports"][0];
    assert_eq!(failed["run"]["nodes"]["create"]["status"], "running");
    assert!(failed["run"]["nodes"]["create"]["failure"].is_null());
    assert_eq!(old["lifecycle"]["status"], "failed");
    assert_eq!(old["lifecycle"]["failure"]["code"], "material-write-failed");
    assert_eq!(old["lifecycle"]["failure"]["effects"]["write"], "succeeded");
    assert_eq!(old["lifecycle"]["failure"]["effects"]["readBack"], "succeeded");
    assert!(!old["lifecycle"]["failure"]["message"].as_str().unwrap().is_empty());
    assert_eq!(report["canComplete"], false);
    assert!(report["body"].as_str().unwrap().contains("material-write-failed"));
    assert_eq!(report["executionId"], old["id"]);
    assert_eq!(report["outputs"].as_array().unwrap().len(), 1);
    let old_path = report["outputs"][0]["path"].as_str().unwrap();
    let old_bytes = fs::read(old_path).unwrap();
    let old_baseline: Value = serde_json::from_slice(&old_bytes).unwrap();
    assert_eq!(old_baseline["bot"]["id"], "invoice-review");
    let calls = s.herdr.calls();
    action(&s, "retry", json!({"confirmed":true,"reason":"Cannot bypass explicit fail"}), 409).await;
    action(&s, "complete", json!({"reason":"False report cannot hand off"}), 409).await;
    action(&s, "fail", json!({"reason":"  "}), 409).await;
    assert_eq!(detail(&s, &run).await, failed);
    assert_eq!(s.herdr.calls(), calls);
    let marked = action(&s, "fail", json!({"reason":"Inspected partial baseline and saved Bot; new work authorized separately"}), 200).await;
    assert_eq!(marked["snapshot"]["nodes"]["create"]["status"], "failed");
    assert_eq!(marked["snapshot"]["nodes"]["create"]["execution"], old);
    let fixed = detail(&s, &run).await;
    let calls = s.herdr.calls();
    for extra in [json!({"confirmed":false,"reason":"Not confirmed"}), json!({"confirmed":true,"reason":" "})] {
        assert_eq!(action(&s, "retry", extra, 409).await["code"], "retry-confirmation-required");
    }
    for extra in [json!({"reason":"Missing confirmed"}), json!({"confirmed":true})] { action(&s, "retry", extra, 400).await; }
    assert_eq!(action(&s, "recover", json!({"confirmed":true,"reason":"Ended script cannot resume a Chat"}), 409).await["code"], "node-not-recoverable");
    assert_eq!(detail(&s, &run).await, fixed);
    assert_eq!(s.herdr.calls(), calls);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"partial-consumer"},"native_cwd":s.data.path().join("node-zone"),"consume_named_json":true}));
    let consumer = s.dispatch("failure-manager", &["old_consumer"]).await;
    assert_eq!(consumer["results"][0]["status"], "running");
    let consumed = common::read_json_lines(&s.herdr.path().join("consumed.jsonl"));
    assert_eq!(consumed.last().unwrap()["files"]["baseline"], old_baseline);
    // A different authorized Bot identity is explicit input, never rollback or
    // deletion of the already-saved Bot to make retry look idempotent.
    let tasks = request(&s, "/api/tasks", json!({"name":"Replacement review target","taskType":"normal","agent":"pi","chatName":"Replacement {chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"只读替代目标，不修改。"}), 200).await;
    let replacement = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "Replacement review target").unwrap();
    input["target"]["task_id"] = replacement["id"].clone();
    input["bot"]["id"] = json!("invoice-review-replacement");
    input["bot"]["name"] = json!("Replacement reviewer");
    fs::write(s.data.path().join("request.json"), input.to_string()).unwrap();
    let gate = super::super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file);
    let retry = action(&s, "retry", json!({"confirmed":true,"reason":"Partial effects inspected; authorize independent replacement Bot"}), 200).await;
    assert!(common::wait_until(Duration::from_secs(5), || gate.reached()).await);
    let current = &retry["snapshot"]["nodes"]["create"]["execution"];
    assert_ne!(current["id"], old["id"]);
    for key in ["chat", "taskId", "agent", "childRunId"] { assert!(current.get(key).is_none()); }
    assert_eq!(retry["snapshot"]["nodes"]["create"]["previousExecutions"][0]["execution"], old);
    assert_eq!(retry["snapshot"]["nodes"]["create"]["previousExecutions"][0]["failure"], marked["snapshot"]["nodes"]["create"]["failure"]);
    assert!(retry["pendingReports"].get("create").is_none());
    assert_eq!(action(&s, "complete", json!({"reason":"No current report"}), 409).await["code"], "report-not-completable");
    assert_eq!(detail(&s, &run).await["reports"], failed["reports"]);
    drop(gate);
    let done = reports(&s, &run, 2).await;
    let new_report = &done["reports"][1];
    assert_eq!(new_report["executionId"], current["id"]);
    assert_eq!(new_report["canComplete"], true, "{done}");
    assert_eq!(done["reports"][0], *report);
    assert_eq!(done["run"]["nodes"]["create"]["status"], "running");
    assert_ne!(new_report["outputs"][0]["path"], report["outputs"][0]["path"]);
    assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    assert_eq!(fs::read(&collision).unwrap(), b"UNOWNED HANDOFF");
    assert_eq!(action(&s, "collect", json!({}), 200).await["pendingReports"]["create"], new_report["id"]);
    action(&s, "complete", json!({"reason":"Accept only the replacement execution"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"replacement-consumer"},"native_cwd":s.data.path().join("node-zone"),"consume_named_json":true}));
    let consumer = s.dispatch("failure-manager", &["consume"]).await;
    assert_eq!(consumer["results"][0]["status"], "running");
    assert_eq!(consumer["snapshot"]["nodes"]["consume"]["execution"]["inputs"], new_report["outputs"]);
    let consumed = common::read_json_lines(&s.herdr.path().join("consumed.jsonl"));
    assert_eq!(consumed.last().unwrap()["files"]["baseline"]["bot"]["id"], "invoice-review-replacement");
    assert_eq!(consumed[0]["files"]["baseline"]["bot"]["id"], "invoice-review");
    assert_eq!(consumed.last().unwrap()["files"]["handoff"]["bot_id"], "invoice-review-replacement");
    assert_eq!(consumed.last().unwrap()["files"]["baseline"]["request"], input);
    println!("P13/P14/P19 run={} old={} current={} reports={} consumers={}", run["id"], old["id"], current["id"], done["reports"], json!(consumed));
}
