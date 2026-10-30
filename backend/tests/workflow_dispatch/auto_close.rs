//! Approved S2/S3: real workflow HTTP and production CLI transport to an owned host.
#[path = "completion_commit.rs"]
mod completion_commit;
use super::*;

async fn set_auto_close(s: &Scenario, node: &str, enabled: bool) {
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":node}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let draft: Value = serde_json::from_str(&response.body).unwrap();
    let mut content = draft["content"].clone();
    content["auto_close"] = json!(enabled);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":node,"content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
}

fn node_host(s: &Scenario, identity: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
}

async fn manager_action(s: &Scenario, name: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn closes(s: &Scenario) -> Vec<Value> {
    s.herdr.calls().into_iter().filter(|call| call["kind"] == "tab_close").collect()
}

async fn wait_closed(s: &Scenario, count: usize) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if common::read_json_lines(&s.herdr.path().join("closed.jsonl")).len() >= count { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("the eligible close command must reach the external host");
}

// P9: an old execution has no close choice even when its Map now enables it.
// Defaulting from the live Map or opportunistically rewriting on GET would
// violate the independent no-command / byte-preservation expectations.
#[tokio::test]
async fn auto_close_old_execution_defaults_false_without_read_compensation_or_history_rewrite() {
    let s = Scenario::new().await;
    let started = s.start("close-legacy-manager").await;
    node_host(&s, "close-legacy-A");
    s.dispatch("close-legacy-manager", &["A"]).await;
    set_auto_close(&s, "A", true).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let mut run: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    run["nodes"]["A"]["execution"].as_object_mut().unwrap().remove("autoClose");
    fs::write(root.join("run.json"), run.to_string()).unwrap();
    let mut history = common::read_json_lines(&root.join("history.jsonl"));
    history[0]["execution"].as_object_mut().unwrap().remove("autoClose");
    fs::write(root.join("history.jsonl"), format!("{}\n", history[0])).unwrap();
    let old_run = fs::read(root.join("run.json")).unwrap();
    let old_history = fs::read(root.join("history.jsonl")).unwrap();
    let before_calls = s.herdr.calls();
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["run"]["nodes"]["A"]["execution"]["autoClose"], false);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), old_run);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), old_history);
    assert_eq!(s.herdr.calls(), before_calls);
    let completed = manager_action(&s, "complete", json!({"sessionId":"close-legacy-manager","nodeId":"A","reason":"Legacy handoff accepted"}), 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert!(fs::read(root.join("history.jsonl")).unwrap().starts_with(&old_history));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), before_calls, "neither loading nor completing legacy data may close a Tab");
}

// P13/P10: retry creates an independent Tab; manager, another running node and
// the previous execution remain present. Scanning by name or closing the old
// execution would disagree with the single exact argv observed after complete.
#[tokio::test]
async fn auto_close_targets_only_current_execution_once_among_other_tabs() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    set_auto_close(&s, "B", true).await;
    let started = s.start("close-target-manager").await;
    node_host(&s, "close-target-old");
    let old = s.dispatch("close-target-manager", &["A"]).await;
    manager_action(&s, "fail", json!({"sessionId":"close-target-manager","nodeId":"A","reason":"Keep old Tab for investigation"}), 200).await;
    assert!(closes(&s).is_empty());
    node_host(&s, "close-target-current");
    let retried = manager_action(&s, "retry", json!({"sessionId":"close-target-manager","nodeId":"A","confirmed":true,"reason":"Independent replacement approved"}), 200).await;
    assert_eq!(retried["result"]["status"], "running");
    node_host(&s, "close-target-other");
    let other = s.dispatch("close-target-manager", &["B"]).await;
    let current = &retried["snapshot"]["nodes"]["A"]["execution"];
    let tab = current["chat"]["tabId"].as_str().unwrap();
    for decoy in [&started["run"]["manager"]["tabId"], &old["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"], &other["snapshot"]["nodes"]["B"]["execution"]["chat"]["tabId"]] {
        assert_ne!(decoy, &json!(tab));
    }
    assert_eq!(s.herdr.tabs().len(), 4, "all four distinct Tabs were really created");
    let before = s.herdr.calls().len();
    let completed = manager_action(&s, "complete", json!({"sessionId":"close-target-manager","nodeId":"A","reason":"Current execution delivered"}), 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], old["snapshot"]["nodes"]["A"]["execution"]);
    wait_closed(&s, 1).await;
    let repeated = manager_action(&s, "complete", json!({"sessionId":"close-target-manager","nodeId":"A","reason":"Duplicate must not close again"}), 200).await;
    assert_eq!(repeated["changed"], false);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let calls = s.herdr.calls();
    assert_eq!(calls[before..].iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab","close",tab])], "no target discovery, manager/other/old close or repeated close");
}

// P15/P16: the external close is held behind a release file. An awaited close
// (or an operation/file guard moved into its task) would prevent the response or
// dependent C dispatch until release. A working session must not need stop/idle.
#[tokio::test]
async fn auto_close_while_working_does_not_block_complete_or_same_run_downstream_dispatch() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    s.start("close-held-manager").await;
    node_host(&s, "close-held-B");
    s.dispatch("close-held-manager", &["B"]).await;
    manager_action(&s, "complete", json!({"sessionId":"close-held-manager","nodeId":"B","reason":"B ready"}), 200).await;
    node_host(&s, "close-held-A");
    let dispatched = s.dispatch("close-held-manager", &["A"]).await;
    let target = dispatched["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"].as_str().unwrap();
    for event in [json!({"sessionId":"close-held-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":"close-held-A","kind":"state","data":{"value":"working"}})] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    let release = s.data.path().join("release-close");
    // Always unblock the owned child, even if an assertion fails.
    struct Release(std::path::PathBuf);
    impl Drop for Release { fn drop(&mut self) { let _ = fs::write(&self.0, "release"); } }
    let _release_on_drop = Release(release.clone());
    s.herdr.set_behavior(json!({"tab_close_gate":{"wait_for_file":release,"timeout_ms":30000},"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"close-held-C"},"native_cwd":s.data.path().join("node-zone")}));
    let before = s.herdr.calls().len();
    let addr = s.server.addr();
    let complete = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/manager/complete", &json!({"sessionId":"close-held-manager","nodeId":"A","reason":"Ready while agent still working"}).to_string()).await
    });
    assert!(common::wait_until(Duration::from_secs(10), || !closes(&s).is_empty()).await, "close must be issued without a stop/idle event");
    let mut events = vec!["close-entered"];
    assert!(!release.exists());
    assert!(common::read_json_lines(&s.herdr.path().join("closed.jsonl")).is_empty());
    let response = tokio::time::timeout(Duration::from_secs(10), complete).await.expect("complete must return before close release").unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let completed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(completed["ready"], json!(["C"]));
    events.push("complete-returned");
    assert_eq!(s.herdr.calls()[before..].iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab","close",target])], "completion itself must not stop or probe the working agent");
    let next = tokio::time::timeout(Duration::from_secs(10), s.dispatch("close-held-manager", &["C"])).await.expect("same Run must progress while close is held");
    assert_eq!(next["results"][0]["status"], "running");
    events.push("downstream-dispatched");
    assert!(!release.exists());
    assert!(common::read_json_lines(&s.herdr.path().join("closed.jsonl")).is_empty(), "the external close has not returned during either business operation");
    let calls = s.herdr.calls();
    assert_eq!(calls[before]["argv"], json!(["tab","close",target]));
    assert!(calls[before..].iter().all(|call| ["tab_close","workspace_list","tab_create","agent_start","agent_get","agent_prompt"].contains(&call["kind"].as_str().unwrap())), "no stop or idle probing: {:?}", &calls[before..]);
    fs::write(&release, "release").unwrap();
    events.push("close-released");
    wait_closed(&s, 1).await;
    events.push("close-returned");
    assert_eq!(events, ["close-entered","complete-returned","downstream-dispatched","close-released","close-returned"]);
    assert_eq!(closes(&s).len(), 1);
    println!("P15/P16 event order: {events:?}");
}

// P10: only manager completion is the trigger. Reports, stopped/waiting Chat
// observations, pause/resume and execution failure cannot close independently.
// Existing rules explicitly allow manual handoff from execution-stage failed,
// even while paused, without report approval or an idle/exit precondition.
#[tokio::test]
async fn auto_close_preserves_manager_eligibility_and_ignores_non_completion_entries() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    s.start("close-eligibility-manager").await;
    let pending = manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":"A","reason":"Not dispatched"}), 409).await;
    assert_eq!(pending["code"], "node-not-completable");
    node_host(&s, "close-eligibility-A");
    s.dispatch("close-eligibility-manager", &["A"]).await;
    manager_action(&s, "complete", json!({"sessionId":"close-eligibility-A","nodeId":"A","reason":"Node cannot accept own handoff"}), 403).await;
    let blank = manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":"A","reason":"  "}), 409).await;
    assert_eq!(blank["code"], "completion-reason-required");
    let unknown = manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":"unknown","reason":"No target"}), 409).await;
    assert_eq!(unknown["code"], "node-unknown");
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"close-eligibility-A","body":"A report is not manager acceptance"}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    for event in [
        json!({"sessionId":"close-eligibility-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}),
        json!({"sessionId":"close-eligibility-A","kind":"state","data":{"value":"working"}}),
        json!({"sessionId":"close-eligibility-A","kind":"state","data":{"value":"idle"}}),
        json!({"sessionId":"close-eligibility-A","kind":"state","data":{"value":"awaiting-input"}}),
        json!({"sessionId":"close-eligibility-A","kind":"unregister"}),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    manager_action(&s, "pause", json!({"sessionId":"close-eligibility-manager"}), 200).await;
    manager_action(&s, "resume", json!({"sessionId":"close-eligibility-manager"}), 200).await;
    let failed = manager_action(&s, "fail", json!({"sessionId":"close-eligibility-manager","nodeId":"A","reason":"Manual repair required"}), 200).await;
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["stage"], "execution");
    manager_action(&s, "pause", json!({"sessionId":"close-eligibility-manager"}), 200).await;
    manager_action(&s, "finish", json!({"sessionId":"close-eligibility-manager"}), 409).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(closes(&s).is_empty(), "none of the rejected or non-completion entries qualifies");
    let completed = manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":"A","reason":"User repaired outputs; manager accepts handoff"}), 200).await;
    assert_eq!(completed["snapshot"]["status"], "paused");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    wait_closed(&s, 1).await;
    manager_action(&s, "resume", json!({"sessionId":"close-eligibility-manager"}), 200).await;
    for node in ["B", "C"] {
        node_host(&s, &format!("close-eligibility-{node}"));
        s.dispatch("close-eligibility-manager", &[node]).await;
        manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":node,"reason":"False setting handoff"}), 200).await;
    }
    let finish = manager_action(&s, "finish", json!({"sessionId":"close-eligibility-manager"}), 200).await;
    assert_eq!(finish["snapshot"]["status"], "completed");
    manager_action(&s, "complete", json!({"sessionId":"close-eligibility-manager","nodeId":"A","reason":"Terminal rejection"}), 409).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(closes(&s).len(), 1, "false executions and whole-Run finish do not trigger close");
}

// P10: even an enabled execution with a saved Tab is not completable when
// initial submission failed. Auto-close must not weaken this existing rule.
#[tokio::test]
async fn auto_close_does_not_close_dispatch_failure_or_its_rejected_complete() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    s.start("close-dispatch-failure-manager").await;
    s.herdr.set_behavior(json!({"fail_prompt":true,"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"close-dispatch-failed"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("close-dispatch-failure-manager", &["A"]).await;
    assert_eq!(dispatched["results"][0]["status"], "failed");
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["failure"]["stage"], "submit");
    assert!(dispatched["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"].is_string());
    let before = s.herdr.calls();
    let rejected = manager_action(&s, "complete", json!({"sessionId":"close-dispatch-failure-manager","nodeId":"A","reason":"Cannot skip dispatch failure"}), 409).await;
    assert_eq!(rejected["code"], "node-not-completable");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(closes(&s).is_empty());
    assert_eq!(s.herdr.calls(), before);
}

// P7/P10: the ticket fixes the choice at dispatch, not at completion or Run
// creation. Reading the current Map at completion would close A and leave B
// open. Both directions observe actual command targets, not only stored flags.
#[tokio::test]
async fn auto_close_uses_dispatch_choice_after_map_changes_in_both_directions() {
    let s = Scenario::new().await;
    s.start("close-map-manager").await;
    set_auto_close(&s, "B", true).await;
    node_host(&s, "close-map-A");
    let a = s.dispatch("close-map-manager", &["A"]).await;
    assert_eq!(a["results"][0]["status"], "running");
    set_auto_close(&s, "A", true).await;
    manager_action(&s, "complete", json!({"sessionId":"close-map-manager","nodeId":"A","reason":"A delivered"}), 200).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(closes(&s).is_empty(), "a false execution must not use the newly true Map");
    node_host(&s, "close-map-B");
    let b = s.dispatch("close-map-manager", &["B"]).await;
    assert_eq!(b["results"][0]["status"], "running");
    let target = b["snapshot"]["nodes"]["B"]["execution"]["chat"]["tabId"].as_str().unwrap();
    set_auto_close(&s, "B", false).await;
    let completed = manager_action(&s, "complete", json!({"sessionId":"close-map-manager","nodeId":"B","reason":"B delivered"}), 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["B"]["status"], "completed");
    wait_closed(&s, 1).await;
    assert_eq!(closes(&s).iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab","close",target])]);
    assert_eq!(a["snapshot"]["nodes"]["A"]["execution"]["autoClose"], false);
    assert_eq!(b["snapshot"]["nodes"]["B"]["execution"]["autoClose"], true);
}
