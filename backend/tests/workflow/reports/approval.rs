//! S1: public HTTP, real owned Run storage and recording external Herdr.
use super::*;

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
