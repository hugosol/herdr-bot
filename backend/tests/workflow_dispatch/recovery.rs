//! Approved seams S2/S3/S4: real HTTP, owned host adapter and persisted records.

// P40.01/P68.06: confirmed repair resumes original tracking even while paused.
// A relaunch or erasure of failure history would violate the contract. Verify
// same literal native identity, intact fail event and no host calls for 150ms.
#[tokio::test]
async fn confirmed_recover_restores_original_execution_while_paused_without_relaunch() {
    let s = Scenario::new().await;
    let started = s.start("manager-recover").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"recover-original"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("manager-recover", &["A"]).await;
    let original = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    action(&s, "fail", json!({"sessionId":"manager-recover","nodeId":"A","reason":"Dependency unavailable"}), 200).await;
    action(&s, "pause", json!({"sessionId":"manager-recover"}), 200).await;
    let before = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    let rejected = action(&s, "recover", json!({"sessionId":"manager-recover","nodeId":"A","confirmed":false,"reason":"Not yet checked"}), 409).await;
    assert_eq!(rejected["code"], "recovery-confirmation-required");
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let recovered = action(&s, "recover", json!({"sessionId":"manager-recover","nodeId":"A","confirmed":true,"reason":"User restored dependency; original Chat can continue"}), 200).await;
    assert_eq!(recovered["changed"], true);
    assert_eq!(recovered["snapshot"]["status"], "paused");
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["execution"], original);
    assert!(recovered["snapshot"]["nodes"]["A"]["failure"].is_null());
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 4);
    assert_eq!(events[1]["action"], "fail");
    assert_eq!(events[1]["reason"], "Dependency unavailable");
    assert_eq!(events[3]["action"], "recover");
    assert_eq!(events[3]["failure"]["message"], "Dependency unavailable");
    assert_eq!(events[3]["execution"], original);
    assert_eq!(events[3]["reason"], "User restored dependency; original Chat can continue");
    let query = action(&s, "status", json!({"sessionId":"manager-recover"}), 200).await;
    assert_eq!(query["snapshot"], recovered["snapshot"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), recovered["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
}

use super::*;

// P39.01/P68.06: an uncertain dispatch remains failed with its first cause;
// neither uncertainty nor confirmation of no Chat may secretly retry. A real
// controlled launch failure supplies the condition, not a hand-built Run.
#[tokio::test]
async fn dispatch_reconciliation_keeps_uncertain_failed_then_confirmed_absence_pending() {
    let s = Scenario::new().await;
    let started = s.start("manager-reconcile-absent").await;
    s.herdr.set_behavior(json!({"fail_agent_start":true}));
    let dispatched = s.dispatch("manager-reconcile-absent", &["A"]).await;
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["failure"]["stage"], "start");
    let failure = dispatched["snapshot"]["nodes"]["A"]["failure"].clone();
    let execution = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let calls = s.herdr.calls();
    let uncertain = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-absent","nodeId":"A","finding":"uncertain","reason":"User cannot confirm a Chat exists"}), 200).await;
    assert_eq!(uncertain["changed"], false);
    assert_eq!(uncertain["snapshot"], dispatched["snapshot"]);
    assert_eq!(uncertain["ready"], json!(["B"]));
    let complete = action(&s, "complete", json!({"sessionId":"manager-reconcile-absent","nodeId":"A","reason":"Cannot bypass dispatch"}), 409).await;
    assert_eq!(complete["code"], "node-not-completable");
    let recover = action(&s, "recover", json!({"sessionId":"manager-reconcile-absent","nodeId":"A","confirmed":true,"reason":"Cannot bypass reconciliation"}), 409).await;
    assert_eq!(recover["code"], "node-not-recoverable");
    let absent = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-absent","nodeId":"A","finding":"not_created","reason":"User inspected Herdr: startup was rejected, no native Chat"}), 200).await;
    assert_eq!(absent["changed"], true);
    assert_eq!(absent["snapshot"]["nodes"]["A"]["status"], "pending");
    assert!(absent["snapshot"]["nodes"]["A"]["failure"].is_null());
    assert!(absent["snapshot"]["nodes"]["A"]["execution"].is_null());
    assert_eq!(absent["ready"], json!(["A", "B"]));
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["failure"], failure);
    assert_eq!(events[1]["finding"], "uncertain");
    assert_eq!(events[1]["after"], "failed");
    assert_eq!(events[2]["finding"], "not_created");
    assert_eq!(events[2]["execution"], execution);
    assert_eq!(events[2]["failure"], failure);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), absent["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    assert!(fs::read(root.join("messages.jsonl")).unwrap().is_empty());
}


// P39.01/P68.06: confirmed creation must bind the actual original host Chat,
// not a caller-supplied identity or a new launch. Identity-read failure leaves
// a real attempted agent; repair only its host observation, then reconcile.
#[tokio::test]
async fn created_dispatch_reconciliation_observes_original_host_identity_without_start_or_input() {
    let s = Scenario::new().await;
    let started = s.start("manager-reconcile-created").await;
    s.herdr.set_behavior(json!({"fail_agent_get":true}));
    let dispatched = s.dispatch("manager-reconcile-created", &["A"]).await;
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["failure"]["stage"], "identity");
    let original = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    assert!(original["chat"].is_null());
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let calls = s.herdr.calls();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"reconciled-native-A"},"native_cwd":s.data.path().join("node-zone")}));
    let reconciled = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-created","nodeId":"A","finding":"created","reason":"User verified original Chat is present and usable"}), 200).await;
    assert_eq!(reconciled["changed"], true);
    assert_eq!(reconciled["snapshot"]["nodes"]["A"]["status"], "running");
    assert!(reconciled["snapshot"]["nodes"]["A"]["failure"].is_null());
    let execution = &reconciled["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(execution["id"], original["id"]);
    assert_eq!(execution["taskId"], original["taskId"]);
    assert_eq!(execution["zone"], original["zone"]);
    assert_eq!(execution["chat"]["sessionId"], "reconciled-native-A");
    assert_eq!(execution["chat"]["workspaceId"], "w-manager");
    for (key, effect) in [("agentName","agentName"),("tabId","tabId"),("paneId","paneId")] {
        assert_eq!(execution["chat"][key], dispatched["snapshot"]["nodes"]["A"]["failure"]["effects"][effect]);
    }
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["failure"]["stage"], "identity");
    assert_eq!(events[1]["finding"], "created");
    assert_eq!(events[1]["execution"], original);
    assert_eq!(events[1]["currentExecution"], *execution);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), reconciled["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let after = s.herdr.calls();
    assert_eq!(&after[..calls.len()], &calls);
    assert_eq!(after.len(), calls.len() + 1);
    assert_eq!(after.last().unwrap()["kind"], "agent_get");
    assert_eq!(after.last().unwrap()["target"], execution["chat"]["agentName"]);
    assert!(fs::read(root.join("messages.jsonl")).unwrap().is_empty());
    // Reconciliation did not submit input. An execution fail must not invent
    // startup effects simply because tracking is now running.
    let failed = action(&s, "fail", json!({"sessionId":"manager-reconcile-created","nodeId":"A","reason":"User found the existing execution unusable"}), 200).await;
    assert!(failed["snapshot"]["nodes"]["A"]["failure"].get("effects").is_none());
    assert_eq!(failed["snapshot"]["nodes"]["A"]["execution"], *execution);
    assert_eq!(s.herdr.calls(), after);
}

// P39.01/P42.01/P68.06: execution failure cannot be rewritten through dispatch
// reconciliation. Explicit manual handoff alone releases dependencies; no fake
// report or downstream material delivery may appear. Existing Chat can report
// a result address afterwards. Contract literals below distinguish those acts.
#[tokio::test]
async fn manual_failed_execution_handoff_unlocks_dependency_without_fabricating_or_forwarding_reports() {
    let s = Scenario::new().await;
    let started = s.start("manager-manual").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    for (node, identity) in [("A", "manual-A"), ("B", "manual-B")] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-manual", &[node]).await;
    }
    action(&s, "complete", json!({"sessionId":"manager-manual","nodeId":"B","reason":"B artifact ready"}), 200).await;
    let failed = action(&s, "fail", json!({"sessionId":"manager-manual","nodeId":"A","reason":"A execution interrupted"}), 200).await;
    assert_eq!(failed["ready"], json!([]));
    let before = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for finding in ["created", "not_created", "uncertain"] {
        let rejected = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-manual","nodeId":"A","finding":finding,"reason":"Must not rewrite execution failure"}), 409).await;
        assert_eq!(rejected["code"], "node-not-dispatch-failed");
    }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let complete = action(&s, "complete", json!({"sessionId":"manager-manual","nodeId":"A","reason":"User supplied repaired artifact at results/A.md; limitation: manual review only"}), 200).await;
    assert_eq!(complete["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(complete["snapshot"]["nodes"]["A"]["execution"], failed["snapshot"]["nodes"]["A"]["execution"]);
    assert_eq!(complete["snapshot"]["nodes"]["A"]["completion"]["reason"], "User supplied repaired artifact at results/A.md; limitation: manual review only");
    assert_eq!(complete["ready"], json!(["C"]));
    assert_eq!(complete["snapshot"]["nodes"]["C"]["status"], "pending");
    assert_eq!(complete["reports"], json!([]));
    assert_eq!(complete["pendingReports"], json!({}));
    assert!(fs::read(root.join("messages.jsonl")).unwrap().is_empty());
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 5);
    assert_eq!(events[3]["action"], "fail");
    assert_eq!(events[4]["action"], "complete");
    assert_eq!(events[4]["before"], "failed");
    assert_eq!(events[4]["after"], "completed");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"manual-A","body":"Result: results/A.md. Limitation: manual review only."}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let messages = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["body"], "Result: results/A.md. Limitation: manual review only.");
    assert_eq!(messages[0]["source"]["sessionId"], "manual-A");
    assert_eq!(messages[0]["target"]["sessionId"], "manager-manual");
    let after = s.herdr.calls();
    assert_eq!(after.len(), calls.len() + 1);
    assert_eq!(after.last().unwrap()["kind"], "agent_prompt");
    assert_eq!(after.last().unwrap()["target"], started["run"]["manager"]["agentName"]);
    let status = action(&s, "status", json!({"sessionId":"manager-manual"}), 200).await;
    assert_eq!(status["snapshot"], complete["snapshot"]);
    assert_eq!(status["pendingReports"], json!({}));
}

// P35.03: reach terminal status through real dispatch/complete/finish, then
// reject every ticket-owned mutation before host observation or state writes.
#[tokio::test]
async fn terminal_run_rejects_failure_reconciliation_recovery_and_manual_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-terminal-repair").await;
    for node in ["A", "B", "C"] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("terminal-{node}")},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-terminal-repair", &[node]).await;
        action(&s, "complete", json!({"sessionId":"manager-terminal-repair","nodeId":node,"reason":"Verified handoff"}), 200).await;
    }
    let finished = action(&s, "finish", json!({"sessionId":"manager-terminal-repair"}), 200).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for (name, extra) in [("fail", json!({"reason":"overwrite"})), ("reconcile_dispatch", json!({"finding":"created","reason":"overwrite"})), ("recover", json!({"confirmed":true,"reason":"overwrite"})), ("complete", json!({"reason":"overwrite"}))] {
        let mut body = extra;
        body["sessionId"] = json!("manager-terminal-repair");
        body["nodeId"] = json!("A");
        let rejected = action(&s, name, body, 409).await;
        assert_eq!(rejected["code"], "run-terminal");
        assert_eq!(rejected["changed"], false);
        assert_eq!(rejected["snapshot"], finished["snapshot"]);
    }
    let query = action(&s, "status", json!({"sessionId":"manager-terminal-repair"}), 200).await;
    assert_eq!(query["snapshot"], finished["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
}

// P39.01: confirmations do not authorize invented/reassigned Chat identity.
// A wrong cwd, manager identity, unavailable host and denial of a known Chat
// must retain the exact persisted failure and history; only agent_get is allowed.
#[tokio::test]
async fn reconciliation_rejects_unverified_or_conflicting_chat_facts_without_mutation() {
    let s = Scenario::new().await;
    let started = s.start("manager-reconcile-guard").await;
    s.herdr.set_behavior(json!({"fail_agent_get":true}));
    s.dispatch("manager-reconcile-guard", &["A"]).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"known-B"},"native_cwd":s.data.path().join("node-zone"),"fail_prompt":true}));
    s.dispatch("manager-reconcile-guard", &["B"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for (behavior, code) in [
        (json!({"fail_agent_get":true}), "dispatch-chat-unverified"),
        (json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"other-cwd"},"native_cwd":s.data.path()}), "dispatch-chat-mismatch"),
        (json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-reconcile-guard"},"native_cwd":s.data.path().join("node-zone")}), "dispatch-chat-mismatch"),
        (json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"known-B"},"native_cwd":s.data.path().join("node-zone")}), "dispatch-chat-mismatch"),
    ] {
        s.herdr.set_behavior(behavior);
        let rejected = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-guard","nodeId":"A","finding":"created","reason":"Inspect the original Chat"}), 409).await;
        assert_eq!(rejected["code"], code);
    }
    let rejected = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-guard","nodeId":"B","finding":"not_created","reason":"Must not erase known Chat"}), 409).await;
    assert_eq!(rejected["code"], "dispatch-chat-already-known");
    // An already-associated submit failure can be reconciled only to that
    // exact session, not another native identity now returned by the host.
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"replacement-B"},"native_cwd":s.data.path().join("node-zone")}));
    let rejected = action(&s, "reconcile_dispatch", json!({"sessionId":"manager-reconcile-guard","nodeId":"B","finding":"created","reason":"Must retain original session"}), 409).await;
    assert_eq!(rejected["code"], "dispatch-chat-mismatch");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let after = s.herdr.calls();
    assert_eq!(&after[..calls.len()], &calls);
    assert_eq!(after.len(), calls.len() + 5);
    assert!(after[calls.len()..].iter().all(|call| call["kind"] == "agent_get"));
}

// S2 authorization and typed actions: no arbitrary-state setter or caller
// ownership fields. Invalid requests must not touch host, state or history.
#[tokio::test]
async fn failure_actions_reject_wrong_role_unknown_fields_and_invalid_transitions() {
    let s = Scenario::new().await;
    let started = s.start("manager-repair-admission").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-repair-admission"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-repair-admission", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for (name, extra, code) in [
        ("fail", json!({"reason":"Not started"}), "node-not-failable"),
        ("recover", json!({"confirmed":true,"reason":"Not failed"}), "node-not-recoverable"),
        ("reconcile_dispatch", json!({"finding":"uncertain","reason":"Not failed"}), "node-not-dispatch-failed"),
    ] {
        let mut body = extra;
        body["nodeId"] = json!("B");
        for session in ["node-repair-admission", "unassociated"] {
            body["sessionId"] = json!(session);
            let rejected = action(&s, name, body.clone(), 403).await;
            assert_eq!(rejected["code"], "manager-unassociated");
        }
        body["sessionId"] = json!("manager-repair-admission");
        assert_eq!(action(&s, name, body.clone(), 409).await["code"], code);
        body["runId"] = started["id"].clone();
        assert_eq!(action(&s, name, body, 400).await["code"], "invalid-request");
    }
    assert_eq!(action(&s, "fail", json!({"sessionId":"manager-repair-admission","nodeId":"A","reason":"  "}), 409).await["code"], "failure-reason-required");
    assert_eq!(action(&s, "reconcile_dispatch", json!({"sessionId":"manager-repair-admission","nodeId":"A","finding":"running","reason":"Arbitrary status forbidden"}), 400).await["code"], "invalid-request");
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls(), calls);
}

// S2/S4 error feedback: a real read-only current-facts file rejects commit.
// Every ticket action must return an error, not publish an uncommitted state or
// append success history; restoring writability permits the explicit action.
#[tokio::test]
async fn failure_actions_do_not_publish_uncommitted_state_or_success_history() {
    let s = Scenario::new().await;
    let started = s.start("manager-repair-write").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"write-original-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-repair-write", &["A"]).await;
    s.herdr.set_behavior(json!({"fail_agent_start":true}));
    s.dispatch("manager-repair-write", &["B"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("run.json");
    let calls = s.herdr.calls();
    for (name, node, extra, code) in [
        ("fail", "A", json!({"reason":"Execution failed"}), "fail-failed"),
        ("recover", "A", json!({"confirmed":true,"reason":"User repaired original execution"}), "recover-failed"),
        ("reconcile_dispatch", "B", json!({"finding":"not_created","reason":"User confirmed no Chat was created"}), "reconcile-dispatch-failed"),
    ] {
        let mut body = extra;
        body["sessionId"] = json!("manager-repair-write");
        body["nodeId"] = json!(node);
        let first = fs::read(&path).unwrap();
        let history = fs::read(root.join("history.jsonl")).unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
        fs::set_permissions(&path, original).unwrap();
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], code);
        assert_eq!(fs::read(&path).unwrap(), first);
        assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
        let query = action(&s, "status", json!({"sessionId":"manager-repair-write"}), 200).await;
        assert_eq!(query["snapshot"], serde_json::from_slice::<Value>(&first).unwrap());
        assert_eq!(action(&s, name, body, 200).await["changed"], true);
    }
    assert_eq!(s.herdr.calls(), calls);
}

async fn action(s: &Scenario, name: &str, body: Value, expected_status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, expected_status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// P20.04/P38.02/P68.06. Contract source: execution failure retains the original
// Chat and first reason. Violation: repeated fail overwrites the reason or
// terminates/retries. Observe HTTP + literal reason on disk + all host calls
// over a bounded 150ms window; the real launch adapter stays in the path.
#[tokio::test]
async fn execution_fail_preserves_original_chat_first_reason_and_history_without_host_actions() {
    let s = Scenario::new().await;
    let started = s.start("manager-failure").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"original-A"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("manager-failure", &["A"]).await;
    let original = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let calls = s.herdr.calls();
    let failed = action(&s, "fail", json!({"sessionId":"manager-failure","nodeId":"A","reason":"Build rejected: missing source artifact"}), 200).await;
    assert_eq!(failed["changed"], true);
    assert_eq!(failed["snapshot"]["status"], "running");
    assert_eq!(failed["snapshot"]["nodes"]["A"]["status"], "failed");
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["stage"], "execution");
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["message"], "Build rejected: missing source artifact");
    assert_eq!(failed["snapshot"]["nodes"]["A"]["execution"], original);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["action"], "fail");
    assert_eq!(events[1]["reason"], "Build rejected: missing source artifact");
    assert_eq!(events[1]["execution"], original);
    assert_eq!(events[1]["actor"], started["run"]["manager"]);
    let repeated = action(&s, "fail", json!({"sessionId":"manager-failure","nodeId":"A","reason":"MUST NOT OVERWRITE"}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], failed["snapshot"]);
    let query = action(&s, "status", json!({"sessionId":"manager-failure"}), 200).await;
    assert_eq!(query["snapshot"], serde_json::from_slice::<Value>(&first).unwrap());
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert!(fs::read(root.join("messages.jsonl")).unwrap().is_empty());
}
