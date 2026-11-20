//! S1: public HTTP, real owned Run storage and recording external Herdr.

// P12: deny Windows replacement on the owned Run, not a simulated response.
// A memory-only approval or swallowed save error would permit complete below.
#[tokio::test]
async fn approval_save_failure_preserves_original_error_and_cannot_authorize_complete() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = dispatched(true, json!([])).await;
    submit_report(&s, "approval-node", "Ready but not yet approved", true).await;
    let before = status(&s).await;
    let context = command(&s, "/api/workflow/node/context", json!({"sessionId":"approval-node"}), 200).await;
    let path = s.config.scheduler.run_directory.join(before["snapshot"]["id"].as_str().unwrap()).join("run.json");
    // Permit reads/writes, but deny FILE_SHARE_DELETE so atomic publication fails.
    let held = fs::OpenOptions::new().read(true).share_mode(3).open(&path).unwrap();
    let calls = s.herdr.calls();
    let failed = command(&s, "/api/workflow/node/approve", json!({"sessionId":"approval-node","executionId":context["executionId"],"source":"auto"}), 500).await;
    assert_eq!(failed["code"], "approval-save-failed");
    // Windows replacement reports ERROR_ACCESS_DENIED for this deny-delete handle.
    assert!(failed["message"].as_str().unwrap().contains("os error 5"), "retain underlying replacement error: {failed}");
    assert_ne!(failed["approved"], true);
    assert_eq!(status(&s).await["snapshot"], before["snapshot"]);
    assert_eq!(command(&s, "/api/workflow/node/context", json!({"sessionId":"approval-node"}), 200).await["approved"], false);
    let denied = command(&s, "/api/workflow/manager/complete", json!({"sessionId":"approval-manager","nodeId":"A","reason":"Failed approval is not authority"}), 409).await;
    assert_eq!(denied["code"], "node-not-approved");
    assert_eq!(denied["reports"], before["reports"]);
    assert_eq!(s.herdr.calls(), calls);
    drop(held);
    println!("P12 failed={failed} denied={denied} calls={calls:?}");
}
use super::*;

// P13: start simultaneous real HTTP requests; exactly one may introduce the
// fact. Duplicate business effects would alter the full Run or host transcript.
#[tokio::test]
async fn repeated_and_concurrent_approval_has_one_fact_and_no_business_effects() {
    let s = dispatched(true, json!([])).await;
    let before = status(&s).await;
    let context = command(&s, "/api/workflow/node/context", json!({"sessionId":"approval-node"}), 200).await;
    let body = json!({"sessionId":"approval-node","executionId":context["executionId"],"source":"auto"});
    let calls = s.herdr.calls();
    let (first, second) = tokio::join!(command(&s, "/api/workflow/node/approve", body.clone(), 200), command(&s, "/api/workflow/node/approve", body.clone(), 200));
    assert_ne!(first["changed"], second["changed"]);
    for response in [&first, &second] {
        assert_eq!(response["approved"], true);
        assert_eq!(response["executionId"], context["executionId"]);
        assert_eq!(response["approvalSource"], "auto");
    }
    let repeated = command(&s, "/api/workflow/node/approve", body, 200).await;
    assert_eq!(repeated["changed"], false);
    let after = status(&s).await;
    let mut expected = before["snapshot"].clone();
    expected["nodes"]["A"]["execution"]["approved"] = json!(true);
    expected["nodes"]["A"]["execution"]["approvalSource"] = json!("auto");
    assert_eq!(after["snapshot"], expected);
    assert_eq!(after["reports"], before["reports"]);
    assert_eq!(after["pendingReports"], before["pendingReports"]);
    assert_eq!(s.herdr.calls(), calls);
    println!("P13 first={first} second={second} repeated={repeated} after={after} calls={calls:?}");
}

// P16: approval is not a digest signature. Mutate actual bytes and append a
// latest non-ready report; approval stays true but completion must still reject.
#[tokio::test]
async fn material_changes_and_later_reports_keep_approval_without_bypassing_latest_report() {
    let s = dispatched(true, json!([{"id":"delivery"}])).await;
    let material = s.data.path().join("node-zone/delivery.txt");
    fs::write(&material, "Original accepted work\n").unwrap();
    let report = json!({"sessionId":"approval-node","body":"Original ready work","canComplete":true,"outputs":[{"id":"delivery","path":material}]});
    command(&s, "/api/workflow/node/report", report, 200).await;
    approve(&s, "approval-node", "auto").await;
    let approved = status(&s).await;
    fs::write(&material, "Revised work; known remaining issue\n").unwrap();
    command(&s, "/api/workflow/node/report", json!({"sessionId":"approval-node","body":"Revised material is not ready yet","canComplete":false,"outputs":[{"id":"delivery","path":material}]}), 200).await;
    let calls = s.herdr.calls();
    let revised = status(&s).await;
    assert_eq!(revised["snapshot"]["nodes"]["A"]["execution"], approved["snapshot"]["nodes"]["A"]["execution"]);
    assert_eq!(revised["reports"].as_array().unwrap().len(), 2);
    let body = json!({"sessionId":"approval-manager","nodeId":"A","reason":"Evaluate current material"});
    let denied = command(&s, "/api/workflow/manager/complete", body.clone(), 409).await;
    assert_eq!(denied["code"], "report-not-completable");
    assert_eq!(denied["snapshot"], revised["snapshot"]);
    assert_eq!(fs::read_to_string(&material).unwrap(), "Revised work; known remaining issue\n");
    assert_eq!(s.herdr.calls(), calls);
    submit_report(&s, "approval-node", "Revised work now ready, limitations documented", true).await;
    let completed = command(&s, "/api/workflow/manager/complete", body, 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["execution"]["approved"], true);
    assert_eq!(completed["snapshot"]["nodes"]["C"]["status"], "pending");
    assert_eq!(fs::read_to_string(material).unwrap(), "Revised work; known remaining issue\n");
    println!("P16 revised={revised} denied={denied} completed={completed}");
}

// P10: use actual ordinary node, independent same-Task Chat, root managers,
// another node/Run and a legally retired execution. A caller-selected ID must
// never redirect approval to another execution.
#[tokio::test]
async fn approval_only_changes_the_callers_current_execution() {
    let s = dispatched(true, json!([])).await;
    let original = status(&s).await["snapshot"]["nodes"]["A"]["execution"].clone();
    let task = original["taskId"].as_str().unwrap();
    s.herdr.set_workspaces(json!([
        {"workspace_id":"w-manager","label":s.data.path().file_name().unwrap().to_str().unwrap()},
        {"workspace_id":"w-independent","label":s.config.herdr.workspace_name}
    ]));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"independent-chat"},"native_cwd":s.data.path().join("node-zone")}));
    let preview = command(&s, "/api/chats/preview", json!({"zone":"node","taskId":task}), 200).await;
    let created = command(&s, "/api/chats", json!({"zone":"node","taskId":task,"chatId":preview["chatId"],"expectedName":preview["name"],"input":"Independent business, not workflow"}), 202).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let response = get_over_tcp(s.server.addr(), &format!("/api/chats/{}", created["id"].as_str().unwrap())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let view: Value = serde_json::from_str(&response.body).unwrap();
        if view["stage"] == "succeeded" { break; }
        assert_ne!(view["stage"], "failed", "{view}");
        assert!(tokio::time::Instant::now() < deadline, "independent Chat did not start: {view}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"other-node"},"native_cwd":s.data.path().join("node-zone")}));
    let other = s.dispatch("approval-manager", &["B"]).await["snapshot"]["nodes"]["B"]["execution"].clone();
    s.start("other-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"other-run-node"},"native_cwd":s.data.path().join("node-zone")}));
    let other_run = s.dispatch("other-manager", &["A"]).await;
    command(&s, "/api/workflow/manager/fail", json!({"sessionId":"approval-manager","nodeId":"A","reason":"Independent retry required"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"current-node"},"native_cwd":s.data.path().join("node-zone")}));
    let retry = command(&s, "/api/workflow/manager/retry", json!({"sessionId":"approval-manager","nodeId":"A","confirmed":true,"reason":"Original effects checked"}), 200).await;
    let current = &retry["snapshot"]["nodes"]["A"]["execution"];
    let calls = s.herdr.calls();
    for session in ["independent-chat", "approval-manager", "other-manager", "unknown", ""] {
        command(&s, "/api/workflow/node/context", json!({"sessionId":session}), 403).await;
        command(&s, "/api/workflow/node/approve", json!({"sessionId":session,"executionId":current["id"],"source":"auto"}), 403).await;
    }
    // Keep the old mixed policy scenario's still-valid strict request checks.
    for input in [json!({}), json!({"sessionId":null}), json!({"sessionId":"current-node","runId":retry["snapshot"]["id"]}), json!(["current-node"])] {
        command(&s, "/api/workflow/node/context", input, 400).await;
    }
    for (route, body) in [("/api/workflow/node/context?sessionId=current-node", r#"{"sessionId":"current-node"}"#), ("/api/workflow/node/context", "{")] {
        let refused = post_json_over_tcp(s.server.addr(), route, body).await;
        assert_eq!(refused.status, 400, "{}", refused.body);
    }
    for (session, target) in [("other-node", current["id"].clone()), ("other-run-node", current["id"].clone()), ("current-node", other["id"].clone()), ("current-node", other_run["snapshot"]["nodes"]["A"]["execution"]["id"].clone()), ("approval-node", current["id"].clone())] {
        let denied = command(&s, "/api/workflow/node/approve", json!({"sessionId":session,"executionId":target,"source":"auto"}), 400).await;
        assert_eq!(denied["code"], "approval-execution-mismatch");
    }
    let stale = command(&s, "/api/workflow/node/approve", json!({"sessionId":"approval-node","executionId":original["id"],"source":"auto"}), 400).await;
    assert_eq!(stale["code"], "approval-execution-stale");
    let wrong_source = command(&s, "/api/workflow/node/approve", json!({"sessionId":"current-node","executionId":current["id"],"source":"manual"}), 400).await;
    assert_eq!(wrong_source["code"], "approval-source-mismatch");
    assert_eq!(status(&s).await["snapshot"], retry["snapshot"]);
    assert_eq!(command(&s, "/api/workflow/manager/status", json!({"sessionId":"other-manager"}), 200).await["snapshot"], other_run["snapshot"]);
    approve(&s, "other-node", "auto").await;
    let after_other = status(&s).await;
    assert_eq!(after_other["snapshot"]["nodes"]["B"]["execution"]["approved"], true);
    assert_eq!(after_other["snapshot"]["nodes"]["A"]["execution"]["approved"], false);
    approve(&s, "current-node", "auto").await;
    assert_eq!(status(&s).await["snapshot"]["nodes"]["A"]["execution"]["approved"], true);
    assert_eq!(command(&s, "/api/workflow/manager/status", json!({"sessionId":"other-manager"}), 200).await["snapshot"], other_run["snapshot"]);
    assert_eq!(s.herdr.calls(), calls);
    println!("P10 old={original} current={current} other={other} stale={stale} wrongSource={wrong_source} calls={calls:?}");
}

// P11/P24: S1 transport gate, NOT a native confirmation. Hyper acknowledges
// Expect:100-continue, then the last request byte stays owned by this caller.
// Query/report/independent branch and legal retry must finish before release.
#[tokio::test]
async fn inflight_old_approval_does_not_block_run_or_approve_retry() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let s = dispatched(false, json!([])).await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["B"]["output_rule"] = json!([]);
    fs::write(path, map.to_string()).unwrap();
    let context = command(&s, "/api/workflow/node/context", json!({"sessionId":"approval-node"}), 200).await;
    let body = json!({"sessionId":"approval-node","executionId":context["executionId"],"source":"manual"}).to_string();
    let mut stream = tokio::net::TcpStream::connect(s.server.addr()).await.unwrap();
    let clock = std::time::Instant::now();
    let mut times = Vec::new();
    stream.write_all(format!("POST /api/workflow/node/approve HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n", s.server.addr(), body.len()).as_bytes()).await.unwrap();
    let mut acknowledged = [0u8; 25];
    tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut acknowledged)).await.unwrap().unwrap();
    assert_eq!(&acknowledged, b"HTTP/1.1 100 Continue\r\n\r\n");
    stream.write_all(&body.as_bytes()[..body.len()-1]).await.unwrap();
    times.push(("approval accepted; body gated", clock.elapsed()));
    tokio::time::timeout(Duration::from_secs(2), status(&s)).await.expect("query blocked by approval");
    times.push(("query returned", clock.elapsed()));
    tokio::time::timeout(Duration::from_secs(2), submit_report(&s, "approval-node", "Progress while confirmation is outstanding", false)).await.expect("report blocked by approval");
    times.push(("report returned", clock.elapsed()));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"independent-branch"},"native_cwd":s.data.path().join("node-zone")}));
    tokio::time::timeout(Duration::from_secs(2), s.dispatch("approval-manager", &["B"])).await.expect("independent branch dispatch blocked by approval");
    submit_report(&s, "independent-branch", "Independent work ready", true).await;
    approve(&s, "independent-branch", "auto").await;
    let branch = tokio::time::timeout(Duration::from_secs(2), command(&s, "/api/workflow/manager/complete", json!({"sessionId":"approval-manager","nodeId":"B","reason":"Accept independent work while A waits"}), 200)).await.expect("independent completion blocked by approval");
    assert_eq!(branch["snapshot"]["nodes"]["B"]["status"], "completed");
    assert_eq!(branch["snapshot"]["nodes"]["A"]["execution"]["approved"], false);
    times.push(("independent branch completed", clock.elapsed()));
    command(&s, "/api/workflow/manager/fail", json!({"sessionId":"approval-manager","nodeId":"A","reason":"Old work requires independent retry"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-after-inflight"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = command(&s, "/api/workflow/manager/retry", json!({"sessionId":"approval-manager","nodeId":"A","confirmed":true,"reason":"Old effects inspected"}), 200).await;
    assert_ne!(retried["snapshot"]["nodes"]["A"]["execution"]["id"], context["executionId"]);
    times.push(("retry returned", clock.elapsed()));
    let mut byte = [0u8; 1];
    assert_eq!(stream.try_read(&mut byte).unwrap_err().kind(), std::io::ErrorKind::WouldBlock, "old approval must still be in flight");
    let calls = s.herdr.calls();
    stream.write_all(&body.as_bytes()[body.len()-1..]).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response)).await.unwrap().unwrap();
    let response = String::from_utf8(response).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
    assert!(response.contains("approval-execution-stale"), "{response}");
    times.push(("old approval rejected after release", clock.elapsed()));
    let after = status(&s).await;
    assert_eq!(after["snapshot"], retried["snapshot"]);
    assert_eq!(after["reports"], retried["reports"]);
    assert_eq!(after["snapshot"]["nodes"]["A"]["execution"]["approved"], false);
    assert_eq!(s.herdr.calls(), calls);
    println!("P11/P24 monotonic={times:?} oldResponse={response} after={after} calls={calls:?}");
}

async fn command(s: &Scenario, route: &str, body: Value, expected: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), route, &body.to_string()).await;
    assert_eq!(response.status, expected, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
fn configure(s: &Scenario, auto: bool, outputs: Value) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["A"]["auto_approved"] = json!(auto);
    map["nodes"]["A"]["output_rule"] = outputs;
    fs::write(path, map.to_string()).unwrap();
}
async fn approve(s: &Scenario, session: &str, source: &str) -> Value {
    let context = command(s, "/api/workflow/node/context", json!({"sessionId":session}), 200).await;
    command(s, "/api/workflow/node/approve", json!({"sessionId":session,"executionId":context["executionId"],"source":source}), 200).await
}
async fn status(s: &Scenario) -> Value {
    command(s, "/api/workflow/manager/status", json!({"sessionId":"approval-manager"}), 200).await
}
async fn dispatched(auto: bool, outputs: Value) -> Scenario {
    let s = Scenario::new().await;
    configure(&s, auto, outputs);
    s.start("approval-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"approval-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("approval-manager", &["A"]).await;
    s
}

// P21/P18/P19: a ready report cannot substitute for approval; approval alone
// cannot advance the graph. Observe the public snapshot and real host calls.
#[tokio::test]
async fn ready_report_requires_approval_and_then_explicit_manager_completion() {
    let s = dispatched(true, json!([])).await;
    let calls = s.herdr.calls();
    submit_report(&s, "approval-node", "Ready for handoff", true).await;
    let reported = status(&s).await;
    assert_eq!(reported["snapshot"]["nodes"]["A"]["execution"]["approved"], false);
    assert_eq!(reported["reports"].as_array().unwrap().len(), 1);
    let report_calls = s.herdr.calls();
    assert_eq!(report_calls[calls.len()..].iter().filter(|call| call["kind"] == "agent_prompt").count(), 1);
    let body = json!({"sessionId":"approval-manager","nodeId":"A","reason":"Accepted literal handoff"});
    let denied = command(&s, "/api/workflow/manager/complete", body.clone(), 409).await;
    assert_eq!(denied["code"], "node-not-approved");
    assert_eq!(denied["snapshot"], reported["snapshot"]);
    assert_eq!(s.herdr.calls(), report_calls);
    approve(&s, "approval-node", "auto").await;
    let approved = status(&s).await;
    assert_eq!(approved["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(approved["snapshot"]["nodes"]["A"]["completion"], Value::Null);
    assert_eq!(approved["snapshot"]["nodes"]["C"]["status"], "pending");
    assert_eq!(approved["snapshot"]["status"], "running");
    assert_eq!(approved["reports"], reported["reports"]);
    assert_eq!(s.herdr.calls(), report_calls);
    let completed = command(&s, "/api/workflow/manager/complete", body, 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert!(completed["snapshot"]["nodes"]["A"]["completion"]["id"].is_string());
    assert_eq!(completed["snapshot"]["nodes"]["C"]["status"], "pending");
    assert_eq!(s.herdr.calls(), report_calls);
    println!("P18/P19/P21 denied={denied} approved={approved} completed={completed} calls={report_calls:?}");
}

// P22: approval is necessary, not sufficient. A broken gate might use an
// earlier true or ignore the declared delivery. Keep report append real and
// send true then false after approval, before finally providing the output.
#[tokio::test]
async fn approved_execution_still_requires_latest_ready_report_and_declared_deliveries() {
    let s = dispatched(true, json!([{"id":"delivery"}])).await;
    approve(&s, "approval-node", "auto").await;
    let body = json!({"sessionId":"approval-manager","nodeId":"A","reason":"Accept only current handoff"});
    for (ready, code) in [(None, "report-not-completable"), (Some(false), "report-not-completable"), (Some(true), "output-missing"), (Some(false), "report-not-completable")] {
        if let Some(ready) = ready { submit_report(&s, "approval-node", "Current handoff state", ready).await; }
        let before = status(&s).await;
        let calls = s.herdr.calls();
        let denied = command(&s, "/api/workflow/manager/complete", body.clone(), 409).await;
        assert_eq!(denied["code"], code);
        assert_eq!(denied["snapshot"], before["snapshot"]);
        assert_eq!(denied["reports"], before["reports"]);
        assert_eq!(denied["snapshot"]["nodes"]["A"]["execution"]["approved"], true);
        assert_eq!(s.herdr.calls(), calls);
        println!("P22 ready={ready:?} denied={denied}");
    }
    let delivery = s.data.path().join("actual-delivery.txt");
    fs::write(&delivery, "Independent completed work").unwrap();
    command(&s, "/api/workflow/node/report", json!({"sessionId":"approval-node","body":"Delivery registered","canComplete":true,"outputs":[{"id":"delivery","path":delivery}]}), 200).await;
    let calls = s.herdr.calls();
    let completed = command(&s, "/api/workflow/manager/complete", body, 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(fs::read_to_string(delivery).unwrap(), "Independent completed work");
    assert_eq!(s.herdr.calls(), calls);
}

// P17/P18/P19: reports remain freely sendable in both modes, before and after
// approval. A report-driven approval or notification on approve would change
// these independently observed facts and the external prompt count.
#[tokio::test]
async fn reports_before_and_after_approval_save_and_notify_without_changing_approval() {
    for (auto, source) in [(true, "auto"), (false, "manual")] {
        let s = dispatched(auto, json!([])).await;
        let calls = s.herdr.calls();
        for (index, (body, ready)) in [("Progress", false), ("Problem", false), ("Failure", false), ("Result", true)].into_iter().enumerate() {
            if index == 2 { approve(&s, "approval-node", source).await; }
            let reported = command(&s, "/api/workflow/node/report", json!({"sessionId":"approval-node","body":body,"canComplete":ready}), 200).await;
            assert_eq!(reported["saved"], true);
            assert_eq!(reported["notification"]["status"], "submitted");
            assert_eq!(reported["report"]["body"], body);
            let current = status(&s).await;
            assert_eq!(current["snapshot"]["nodes"]["A"]["execution"]["approved"], index >= 2);
            assert_eq!(current["snapshot"]["nodes"]["A"]["status"], "running");
            assert_eq!(current["reports"].as_array().unwrap().len(), index + 1);
        }
        let after = s.herdr.calls();
        assert_eq!(after[calls.len()..].iter().filter(|call| call["kind"] == "agent_prompt").count(), 4);
        assert!(!after[calls.len()..].iter().any(|call| matches!(call["kind"].as_str(), Some("agent_start" | "tab_close"))));
        println!("P17/P18/P19 auto={auto} status={} calls={after:?}", status(&s).await);
    }
}

// P2/P9/P18/P19/P23: mode is not a fact. A plausible error pre-approves
// auto nodes or routes approval through report/dispatch. No report, output,
// artifact or description is supplied; compare the actual external calls.
#[tokio::test]
async fn approval_without_report_or_artifact_only_changes_the_approval_fact() {
    for (auto, source) in [(true, "auto"), (false, "manual")] {
        let s = dispatched(auto, json!([])).await;
        let before = status(&s).await;
        let execution = &before["snapshot"]["nodes"]["A"]["execution"];
        assert_eq!(execution["autoApproved"], auto);
        assert_eq!(execution["approved"], false);
        assert!(execution["approvalSource"].is_null());
        assert_eq!(before["reports"], json!([]));
        let calls = s.herdr.calls();
        let approved = approve(&s, "approval-node", source).await;
        assert_eq!(approved["approved"], true);
        assert_eq!(approved["approvalSource"], source);
        let after = status(&s).await;
        let mut expected = before["snapshot"].clone();
        expected["nodes"]["A"]["execution"]["approved"] = json!(true);
        expected["nodes"]["A"]["execution"]["approvalSource"] = json!(source);
        assert_eq!(after["snapshot"], expected);
        assert_eq!(after["reports"], json!([]));
        assert_eq!(s.herdr.calls(), calls, "approval must not notify, launch, close or retry");
        println!("P2/P9/P18/P19/P23 source={source} before={before} after={after} calls={calls:?}");
    }
}
