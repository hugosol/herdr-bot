use super::*;

fn content(result: &str) -> Value {
    json!({"background":"User requested a private brief summary; publishing was forbidden.","execution":"Brief analyst, supplied brief 731, work-zone","result":result,"summary":"The brief was analyzed; publication remains blocked pending user decision.","deliverables":["C:/isolated-delivery/summary.txt"],"pendingDecisions":["Does the summary meet the request?"]})
}
async fn delegated(f: &Fixture) -> Value {
    let preview = f.preview().await;
    let accepted = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 202);
    assert_eq!(terminal(&f.server, accepted["id"].as_str().unwrap()).await["stage"], "succeeded");
    accepted
}
fn prompts(f: &Fixture) -> usize {
    f.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count()
}
fn notification_behavior(f: &Fixture, mut behavior: Value) {
    let mut chief = common::read_json_lines(&f.herdr.path().join("started.jsonl"))[0].clone();
    chief["agent"] = json!("pi");
    chief["agent_session"] = json!({"source":"herdr:pi","agent":"pi","kind":"path","value":f.root.path().join("chief-a.jsonl")});
    behavior["agents"] = json!([chief]);
    f.herdr.set_behavior(behavior);
}

// Approved S2 seam: HTTP submit/list/get, real isolated storage and actual
// backend restart. P21 independently requires saving even without a chief.
// A notification prerequisite or in-memory-only save would fail this readback.
#[tokio::test]
async fn p21_no_chief_report_is_saved_and_survives_actual_backend_restart() {
    let f = Fixture::new().await;
    let delegation = delegated(&f).await;
    post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"unregister"})).await;
    let before = prompts(&f);
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("blocked")})).await;
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["notification"]["status"], "not-attempted");
    let report = saved["report"].clone();
    assert_eq!(report["delegationId"], delegation["id"]);
    assert_eq!(report["from"], "worker-a");
    assert_eq!(report["body"], content("blocked"));
    assert!(!report["id"].as_str().unwrap().is_empty());
    assert_eq!(prompts(&f), before);
    let list = body(get_over_tcp(f.server.addr(), "/api/chief/reports?sessionId=worker-a").await, 200);
    assert_eq!(list["reports"], json!([report]));
    drop(f.server);
    let recovered = TestServer::start(&f.config).await;
    let read = body(get_over_tcp(recovered.addr(), &format!("/api/chief/reports/{}", report["id"].as_str().unwrap())).await, 200);
    assert_eq!(read, report);
    println!("P21 absent-chief durable report={read}");
}

// P21: the external prompt is held AFTER the real adapter enters it. At that
// point HTTP reads must already expose the immutable fact. A notify-before-save
// implementation, rollback on failure or restart replay fails this scenario.
#[tokio::test]
async fn p21_report_is_public_before_notification_and_failure_survives_restart_without_replay() {
    let f = Fixture::new().await;
    delegated(&f).await;
    let before = prompts(&f);
    let release = f.root.path().join("release-chief-notification");
    struct Release(std::path::PathBuf);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Err(error) = fs::write(&self.0, "release owned notification") {
                eprintln!("owned notification gate cleanup failed: {error}");
                if !std::thread::panicking() { panic!("notification gate cleanup: {error}"); }
            }
        }
    }
    let gate = Release(release.clone());
    notification_behavior(&f, json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = f.server.addr();
    let sending = tokio::spawn(async move { post_json_over_tcp(addr, "/api/chief/reports", &json!({"sessionId":"worker-a","body":content("failure")}).to_string()).await });
    let observed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if prompts(&f) == before + 1 { break; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        body(get_over_tcp(addr, "/api/chief/reports?sessionId=worker-a").await, 200)
    }).await;
    drop(gate);
    let result = body(sending.await.unwrap(), 200);
    let report = result["report"].clone();
    assert_eq!(observed.expect("notification must reach external gate")["reports"], json!([report]));
    assert_eq!(result["saved"], true);
    assert_eq!(result["notification"]["status"], "failed");
    assert_eq!(report["body"], content("failure"));
    let call = f.herdr.calls().into_iter().filter(|call| call["kind"] == "agent_prompt").last().unwrap();
    let chief_pane = f.herdr.tabs()[0]["pane_id"].clone();
    assert_eq!(call["target"], chief_pane, "route by current chief native identity, never sender or display name");
    assert!(call["text"].as_str().unwrap().contains(report["id"].as_str().unwrap()));
    drop(f.server);
    let recovered = TestServer::start(&f.config).await;
    assert_eq!(body(get_over_tcp(recovered.addr(), &format!("/api/chief/reports/{}", report["id"].as_str().unwrap())).await, 200), report);
    // Existing production policy: online 10s + retention 30s, scan every 5s.
    // Observe 46s after actual restart/re-registration: no automatic replay,
    // even across startup and an entire maintenance/expiry budget.
    post(&recovered, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
    tokio::time::sleep(Duration::from_secs(46)).await;
    assert_eq!(f.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count(), before + 1);
    assert_eq!(body(get_over_tcp(recovered.addr(), "/api/chief/reports").await, 200)["reports"], json!([report]));
    println!("P21 failed-notification restart observationMs=46000 report={report}");
}

// P17 independent oracle: actual delegation identity, not shared Task, name,
// tool availability or claimed ownership, grants chief-report eligibility.
// Real root manager / node / same-Task Chat are created through public APIs.
#[tokio::test]
async fn p17_only_actual_independent_chat_or_direct_manager_can_submit_chief_reports() {
    let (f, _, request) = map::map_fixture().await;
    let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
    let started = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 201);
    post(&f.server, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"flow-manager","fileInputs":started["run"]["fileInputs"],"confirmed":true,"reason":"Independent supplied brief approval"})).await;
    f.identity("internal-reviewer");
    fs::write(f.root.path().join("internal-reviewer.jsonl"), json!({"type":"session","version":3,"id":"internal-reviewer","timestamp":"2026-11-12T00:00:00.000Z","cwd":f.root.path().join("materials")}).to_string()+"\n").unwrap();
    let mut behavior: Value = serde_json::from_slice(&fs::read(f.herdr.path().join("behavior.json")).unwrap()).unwrap();
    behavior["native_cwd"] = json!(f.root.path().join("materials"));
    f.herdr.set_behavior(behavior);
    let dispatched = post(&f.server, "/api/workflow/manager/dispatch", json!({"sessionId":"flow-manager","nodes":["review"]})).await;
    assert_eq!(dispatched["snapshot"]["nodes"]["review"]["execution"]["chat"]["sessionId"], "internal-reviewer");
    f.identity("same-task-other-chat");
    f.herdr.set_workspaces(json!([{"workspace_id":"ordinary-workspace","label":f.config.herdr.workspace_name}]));
    let ordinary = post(&f.server, "/api/chats/preview", json!({"zone":"work-zone","taskId":f.worker["id"]})).await;
    let ordinary = body(post_json_over_tcp(f.server.addr(), "/api/chats", &json!({"zone":"work-zone","taskId":f.worker["id"],"chatId":ordinary["chatId"],"input":"Unrelated ordinary work"}).to_string()).await, 202);
    let ordinary = terminal(&f.server, ordinary["id"].as_str().unwrap()).await;
    assert_eq!(ordinary["stage"], "succeeded", "{ordinary}");
    let before = prompts(&f);
    for sender in ["unknown-chat", "same-task-other-chat", "internal-reviewer", "chief-a"] {
        let denied = body(post_json_over_tcp(f.server.addr(), "/api/chief/reports", &json!({"sessionId":sender,"body":content("success")}).to_string()).await, 403);
        assert_eq!(denied["code"], "chief-report-unassociated", "{sender}: {denied}");
    }
    for key in ["delegationId", "runId", "botId", "taskId", "from", "to", "recipient", "id"] {
        let mut forged = json!({"sessionId":"flow-manager","body":content("success")});
        forged[key] = preview["previewId"].clone();
        body(post_json_over_tcp(f.server.addr(), "/api/chief/reports", &forged.to_string()).await, 400);
    }
    assert_eq!(body(get_over_tcp(f.server.addr(), "/api/chief/reports").await, 200)["reports"], json!([]));
    assert_eq!(prompts(&f), before, "unauthorized identities must neither save nor notify");
    notification_behavior(&f, json!({}));
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"flow-manager","body":content("success")})).await;
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["notification"]["status"], "submitted");
    assert_eq!(saved["report"]["delegationId"], preview["previewId"]);
    assert_eq!(saved["report"]["from"], "flow-manager");
    assert_eq!(saved["report"]["body"], content("success"));
    assert_eq!(prompts(&f), before + 1);
    let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
    let text = submitted.last().unwrap()["text"].as_str().unwrap();
    let delivered: Value = serde_json::from_str(text.split_once("\n\n").unwrap().1).unwrap();
    assert_eq!(delivered, saved["report"], "current chief receives complete original content and stable identity");
    assert_eq!(body(get_over_tcp(f.server.addr(), &format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap())).await, 200), saved["report"]);
    println!("P17 real direct-manager success, unrelated/same-Task/node/forged identities denied; report={}", saved["report"]);
}

// P21: real Windows filesystem permissions, not a mocked store. A path that
// notifies first, ignores save failure or claims success must fail this test.
#[tokio::test]
async fn p21_real_save_failure_neither_notifies_nor_claims_success() {
    let f = Fixture::new().await;
    delegated(&f).await;
    notification_behavior(&f, json!({}));
    let path = f.config.sessions.history_file.with_extension("chief-reports.jsonl");
    fs::write(&path, "").unwrap();
    struct Restore(std::path::PathBuf, fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Err(error) = fs::set_permissions(&self.0, self.1.clone()) {
                eprintln!("owned journal permission cleanup failed: {error}");
                if !std::thread::panicking() { panic!("journal permission cleanup: {error}"); }
            }
        }
    }
    let original = fs::metadata(&path).unwrap().permissions();
    let restore = Restore(path.clone(), original.clone());
    let mut readonly = original;
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let before = f.herdr.calls();
    let failed = body(post_json_over_tcp(f.server.addr(), "/api/chief/reports", &json!({"sessionId":"worker-a","body":content("failure")}).to_string()).await, 500);
    drop(restore);
    assert_eq!(failed["code"], "report-save-failed");
    assert_eq!(failed["saved"], false);
    assert_eq!(failed["notification"]["status"], "not-attempted");
    assert!(!failed["message"].as_str().unwrap().is_empty());
    assert_eq!(f.herdr.calls(), before, "no inventory query or notification may precede saving");
    assert_eq!(body(get_over_tcp(f.server.addr(), "/api/chief/reports").await, 200)["reports"], json!([]));
    println!("P21 real read-only journal error={failed}");
}

// Development regression for this ticket's current-recipient policy and shared
// result categories, not ticket 16's S3/P22 response-loss acceptance. The actual
// CLI records accepted input then returns an unusable acknowledgement. Routing
// to the original delegator or interpreting that as a failed save must fail.
#[tokio::test]
async fn report_targets_current_chief_and_preserves_saved_fact_when_acknowledgement_is_uncertain() {
    let f = Fixture::new().await;
    delegated(&f).await;
    post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"unregister"})).await;
    let tasks = body(get_over_tcp(f.server.addr(), "/api/tasks").await, 200);
    let task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "entry").unwrap();
    f.identity("chief-b");
    let preview = post(&f.server, "/api/chats/preview", json!({"zone":"work-zone","taskId":task["id"]})).await;
    let started = body(post_json_over_tcp(f.server.addr(), "/api/chats", &json!({"zone":"work-zone","taskId":task["id"],"chatId":preview["chatId"]}).to_string()).await, 202);
    assert_eq!(terminal(&f.server, started["id"].as_str().unwrap()).await["stage"], "succeeded");
    post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-b","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
    let starts = common::read_json_lines(&f.herdr.path().join("started.jsonl"));
    let mut original = starts[0].clone();
    let mut current = starts.last().unwrap().clone();
    for (agent, id) in [(&mut original, "chief-a"), (&mut current, "chief-b")] {
        agent["agent"] = json!("pi");
        agent["agent_session"] = json!({"source":"herdr:pi","agent":"pi","kind":"path","value":f.root.path().join(format!("{id}.jsonl"))});
    }
    let target = current["pane_id"].clone();
    assert_ne!(target, original["pane_id"]);
    f.herdr.set_behavior(json!({"agents":[original,current],"invalid_prompt_response":true}));
    let before = prompts(&f);
    let result = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("success")})).await;
    assert_eq!(result["saved"], true);
    assert_eq!(result["notification"]["status"], "uncertain");
    let accepted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
    assert_eq!(accepted.last().unwrap()["target"], target);
    assert_eq!(prompts(&f), before + 1);
    assert_eq!(body(get_over_tcp(f.server.addr(), &format!("/api/chief/reports/{}", result["report"]["id"].as_str().unwrap())).await, 200), result["report"]);
    println!("current-chief uncertain input outcome={result}");
}
