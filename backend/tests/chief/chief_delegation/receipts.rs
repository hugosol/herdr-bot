use super::*;
use super::reports::{content, delegated, notification_behavior, prompts};

// Contract's immutable identity oracle is the original literal body. Resend
// accepts only the saved ID; forged/revised content cannot overwrite it. A
// notification failure is not receipt, while reliable receipt suppresses all
// later (including overlapping) sends, even when the sender lost its response.
#[tokio::test]
async fn manual_resend_keeps_original_report_and_stops_notifying_after_receipt() {
    let f = Fixture::new().await;
    delegated(&f).await;
    notification_behavior(&f, json!({"fail_prompt":true}));
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("blocked")})).await;
    assert_eq!(saved["notification"]["status"], "failed");
    let id = saved["report"]["id"].as_str().unwrap();
    let path = format!("/api/chief/reports/{id}/resend");
    let before = prompts(&f);
    let viewed = post(&f.server, &format!("/api/chief/reports/{id}/view"), json!({"sessionId":"chief-a"})).await;
    assert_eq!(viewed["report"], saved["report"]);
    assert_eq!(viewed["process"], false);
    assert_eq!(viewed["received"], false);
    assert_eq!(viewed["receipt"], Value::Null);
    let mut altered = content("success");
    altered["summary"] = json!("Revised outcome must never replace the original");
    let rejected = post_json_over_tcp(f.server.addr(), &path, &json!({"sessionId":"worker-a","body":altered}).to_string()).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert_eq!(prompts(&f), before);
    assert_eq!(body(get_over_tcp(f.server.addr(), &format!("/api/chief/reports/{id}")).await, 200), saved["report"]);
    let denied = post_json_over_tcp(f.server.addr(), &path, &json!({"sessionId":"chief-a"}).to_string()).await;
    assert_eq!(denied.status, 403, "{}", denied.body);
    post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"unregister"})).await;
    let absent = post(&f.server, &path, json!({"sessionId":"worker-a"})).await;
    assert_eq!(absent["report"], saved["report"]);
    assert_eq!(absent["received"], false);
    assert_eq!(absent["notification"]["status"], "not-attempted");
    assert_eq!(prompts(&f), before);
    replacement_chief(&f, "chief-b").await;
    // Both original and current chief still have external processes. Manual
    // resend must choose the authoritative replacement, not the delegator.
    let starts = common::read_json_lines(&f.herdr.path().join("started.jsonl"));
    let mut original = starts[0].clone();
    let mut current = starts.last().unwrap().clone();
    for (agent, identity) in [(&mut original, "chief-a"), (&mut current, "chief-b")] {
        agent["agent"] = json!("pi");
        agent["agent_session"] = json!({"source":"herdr:pi","agent":"pi","kind":"path","value":f.root.path().join(format!("{identity}.jsonl"))});
    }
    let target = current["pane_id"].clone();
    assert_ne!(target, original["pane_id"]);
    f.herdr.set_behavior(json!({"agents":[original,current]}));
    let before_resend = prompts(&f);
    let resent = post(&f.server, &path, json!({"sessionId":"worker-a"})).await;
    assert_eq!(resent["report"], saved["report"]);
    assert_eq!(resent["notification"]["status"], "submitted");
    assert_eq!(resent["received"], false);
    assert_eq!(prompts(&f), before_resend + 1);
    let calls = f.herdr.calls();
    let sent = calls.iter().filter(|call| call["kind"] == "agent_prompt").last().unwrap();
    assert_eq!(sent["target"], target);
    let accepted = post(&f.server, &format!("/api/chief/reports/{id}/receive"), json!({"sessionId":"chief-b"})).await;
    assert_eq!(accepted["process"], true);
    let input = json!({"sessionId":"worker-a"}).to_string();
    let (a,b) = tokio::join!(post_json_over_tcp(f.server.addr(), &path, &input),post_json_over_tcp(f.server.addr(), &path, &input));
    for response in [a,b] {
        let result = body(response, 200);
        assert_eq!(result["received"], true);
        assert_eq!(result["receipt"], accepted["receipt"]);
        assert_eq!(result["notification"]["status"], "not-attempted");
        assert_eq!(result["report"], saved["report"]);
    }
    assert_eq!(prompts(&f), before_resend + 1);
    assert_eq!(body(get_over_tcp(f.server.addr(), "/api/chief/reports").await, 200)["reports"], json!([saved["report"]]));
}

// Resume uses actual history/native identity and the production Herdr adapter.
// Original report and delegation IDs are independent literal saved facts. A
// resume that replays startup, invents a new delegation/Run, or loses report
// provenance fails the public counts/readback or external prompt observation.
#[tokio::test]
async fn restored_independent_sender_and_direct_manager_resend_original_without_redelegation() {
    for direct_manager in [false, true] {
        let (mut f, sender) = if direct_manager {
            let (f, _, request) = map::map_fixture().await;
            let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
            body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 201);
            (f, "flow-manager")
        } else {
            let f = Fixture::new().await;
            delegated(&f).await;
            // This is an actual distinct native Chat from the same Task, not
            // a fabricated session string claiming the sender's Task name.
            f.identity("same-task-other-chat");
            let preview = post(&f.server, "/api/chats/preview", json!({"zone":"work-zone","taskId":f.worker["id"]})).await;
            let started = body(post_json_over_tcp(f.server.addr(), "/api/chats", &json!({"zone":"work-zone","taskId":f.worker["id"],"chatId":preview["chatId"],"input":"Unrelated independent work"}).to_string()).await, 202);
            assert_eq!(terminal(&f.server, started["id"].as_str().unwrap()).await["stage"], "succeeded");
            (f, "worker-a")
        };
        let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":sender,"body":content("blocked")})).await;
        let path = format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap());
        let delegations = body(get_over_tcp(f.server.addr(), &format!("/api/chief/delegations?sessionId={sender}")).await, 200);
        let runs_before = body(get_over_tcp(f.server.addr(), "/api/workflow/runs").await, 200);
        let run_ids_before: Vec<_> = runs_before["runs"].as_array().unwrap().iter().map(|run| run["id"].clone()).collect();
        assert_eq!(run_ids_before.len(), usize::from(direct_manager));
        let before_resume = prompts(&f);
        f.identity(sender);
        drop(f.server);
        f.server = TestServer::start(&f.config).await;
        let resumed = post(&f.server, "/api/chats/resume", json!({"sessionId":sender})).await;
        assert!(["restored", "located"].contains(&resumed["outcome"].as_str().unwrap()), "{resumed}");
        if !direct_manager {
            f.identity("same-task-other-chat");
            post(&f.server, "/api/chats/resume", json!({"sessionId":"same-task-other-chat"})).await;
            let denied = post_json_over_tcp(f.server.addr(), &format!("{path}/resend"), &json!({"sessionId":"same-task-other-chat"}).to_string()).await;
            assert_eq!(denied.status, 403, "{}", denied.body);
        }
        assert_eq!(prompts(&f), before_resume, "neither resumed Chat replays its original input");
        assert_eq!(body(get_over_tcp(f.server.addr(), &format!("/api/chief/delegations?sessionId={sender}")).await, 200), delegations);
        let runs_after = body(get_over_tcp(f.server.addr(), "/api/workflow/runs").await, 200);
        let run_ids_after: Vec<_> = runs_after["runs"].as_array().unwrap().iter().map(|run| run["id"].clone()).collect();
        assert_eq!(run_ids_after, run_ids_before, "Resume cannot create a new Run");
        assert_eq!(body(get_over_tcp(f.server.addr(), &path).await, 200), saved["report"]);
        let no_chief = post(&f.server, &format!("{path}/resend"), json!({"sessionId":sender})).await;
        assert_eq!(no_chief["notification"]["status"], "not-attempted");
        assert_eq!(no_chief["report"], saved["report"]);
        post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
        notification_behavior(&f, json!({}));
        let resent = post(&f.server, &format!("{path}/resend"), json!({"sessionId":sender})).await;
        assert_eq!(resent["notification"]["status"], "submitted");
        assert_eq!(resent["report"], saved["report"]);
        assert_eq!(prompts(&f), before_resume + 1);
        assert_eq!(body(get_over_tcp(f.server.addr(), "/api/chief/reports").await, 200)["reports"], json!([saved["report"]]));
    }
}

// Real read-only receipt journal leaves the immutable report retrievable but
// cannot grant process=true. Existing FileStore sync/write failures must be
// propagated; after repairing this test-owned fault an explicit call may claim.
#[tokio::test]
async fn receipt_storage_failure_never_grants_processing_or_loses_original_report() {
    let f = Fixture::new().await;
    delegated(&f).await;
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("blocked")})).await;
    let path = format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap());
    let file = f.config.sessions.history_file.with_extension("chief-reports.receipts.jsonl");
    fs::write(&file, "").unwrap();
    struct Restore(std::path::PathBuf, fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Err(error) = fs::set_permissions(&self.0, self.1.clone()) {
                eprintln!("owned receipt permission cleanup failed: {error}");
                if !std::thread::panicking() { panic!("receipt permission cleanup: {error}"); }
            }
        }
    }
    let original = fs::metadata(&file).unwrap().permissions();
    let restore = Restore(file.clone(), original.clone());
    let mut readonly = original;
    readonly.set_readonly(true);
    fs::set_permissions(&file, readonly).unwrap();
    let failed = body(post_json_over_tcp(f.server.addr(), &format!("{path}/receive"), &json!({"sessionId":"chief-a"}).to_string()).await, 500);
    assert_ne!(failed["process"], true);
    assert!(!failed["message"].as_str().unwrap().is_empty());
    assert_eq!(body(get_over_tcp(f.server.addr(), &format!("{path}/receipt")).await, 200), json!({"received":false,"receipt":null}));
    assert_eq!(body(get_over_tcp(f.server.addr(), &path).await, 200), saved["report"]);
    drop(restore);
    let accepted = post(&f.server, &format!("{path}/receive"), json!({"sessionId":"chief-a"})).await;
    assert_eq!(accepted["process"], true);
    assert_eq!(accepted["report"], saved["report"]);
}

// Once notification enters the actual external prompt command its authorized
// effect cannot be retracted by replacing chief. The final result must preserve
// that submitted fact, while the old chief cannot acquire a new receipt grant.
#[tokio::test]
async fn handoff_during_authorized_external_input_preserves_submission_without_stale_receipt_grant() {
    let f = Fixture::new().await;
    delegated(&f).await;
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("success")})).await;
    let path = format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap());
    let release = f.root.path().join("release-authorized-notification");
    let guard = Release(release.clone());
    notification_behavior(&f, json!({"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let before = prompts(&f);
    let addr = f.server.addr();
    let resend_path = format!("{path}/resend");
    let sending = tokio::spawn(async move { post_json_over_tcp(addr, &resend_path, &json!({"sessionId":"worker-a"}).to_string()).await });
    let entered = common::wait_until(Duration::from_secs(5), || prompts(&f) == before + 1).await;
    if entered {
        post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"unregister"})).await;
        replacement_chief(&f, "chief-b").await;
    }
    drop(guard);
    let result = body(tokio::time::timeout(Duration::from_secs(15), sending).await.unwrap().unwrap(), 200);
    assert!(entered, "resend must reach the authorized external prompt");
    assert_eq!(result["notification"]["status"], "submitted", "external effect is not rolled back by chief replacement");
    assert_eq!(result["received"], false, "Herdr submission does not manufacture receipt");
    let denied = post_json_over_tcp(f.server.addr(), &format!("{path}/receive"), &json!({"sessionId":"chief-a"}).to_string()).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    let accepted = post(&f.server, &format!("{path}/receive"), json!({"sessionId":"chief-b"})).await;
    assert_eq!(accepted["process"], true);
    assert_eq!(accepted["report"], saved["report"]);
}

struct Release(std::path::PathBuf);
impl Drop for Release {
    fn drop(&mut self) {
        if let Err(error) = fs::write(&self.0, "release owned CLI gate") {
            eprintln!("owned inventory gate cleanup failed: {error}");
            if !std::thread::panicking() { panic!("inventory gate cleanup: {error}"); }
        }
    }
}

// A controlled external inventory await exposes the actual authorization race.
// Receipt or revocation while it is blocked must invalidate a pending resend;
// caching the pre-query receipt/chief state would submit an extra external input.
#[tokio::test]
async fn pending_resend_rechecks_receipt_and_chief_after_external_inventory() {
    for revoke in [false, true] {
        let f = Fixture::new().await;
        delegated(&f).await;
        let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("success")})).await;
        let path = format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap());
        let release = f.root.path().join("release-resend-inventory");
        let guard = Release(release.clone());
        notification_behavior(&f, json!({"agent_list_gate":{"wait_for_file":release,"timeout_ms":10000}}));
        let calls = f.herdr.calls().len();
        let prompts_before = prompts(&f);
        let addr = f.server.addr();
        let resend_path = format!("{path}/resend");
        let sending = tokio::spawn(async move { post_json_over_tcp(addr, &resend_path, &json!({"sessionId":"worker-a"}).to_string()).await });
        let entered = common::wait_until(Duration::from_secs(5), || f.herdr.calls()[calls..].iter().any(|call| call["kind"] == "agent_list")).await;
        if entered {
            if revoke {
                post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"unregister"})).await;
                replacement_chief(&f, "chief-b").await;
            } else {
                let received = post(&f.server, &format!("{path}/receive"), json!({"sessionId":"chief-a"})).await;
                assert_eq!(received["process"], true);
            }
        }
        drop(guard);
        let result = body(tokio::time::timeout(Duration::from_secs(15), sending).await.unwrap().unwrap(), 200);
        assert!(entered, "resend must reach external inventory gate");
        assert_eq!(result["report"], saved["report"]);
        assert_eq!(result["notification"]["status"], "not-attempted");
        // A replacement chief has one legitimate startup input of its own.
        assert_eq!(prompts(&f), prompts_before + usize::from(revoke));
        assert_eq!(result["received"], !revoke);
    }
}

async fn replacement_chief(f: &Fixture, id: &str) {
    let tasks = body(get_over_tcp(f.server.addr(), "/api/tasks").await, 200);
    let task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "entry").unwrap();
    f.identity(id);
    let preview = post(&f.server, "/api/chats/preview", json!({"zone":"work-zone","taskId":task["id"]})).await;
    let started = body(post_json_over_tcp(f.server.addr(), "/api/chats", &json!({"zone":"work-zone","taskId":task["id"],"chatId":preview["chatId"]}).to_string()).await, 202);
    assert_eq!(terminal(&f.server, started["id"].as_str().unwrap()).await["stage"], "succeeded");
    post(&f.server, "/api/sessions/events", json!({"sessionId":id,"kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
}

// Reliable receipt is independent of the receiver's response/history. Drop the
// actual TCP caller before reading its response, then terminate the backend.
// An in-memory receipt, restart replay, chief-local dedup or stale authorization
// would fail public reads, the bounded prompt observation or the new-chief read.
#[tokio::test]
async fn lost_receipt_response_and_pre_presentation_crash_allow_only_explicit_current_chief_view() {
    use std::io::Write;
    let mut f = Fixture::new().await;
    delegated(&f).await;
    notification_behavior(&f, json!({}));
    let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("success")})).await;
    let id = saved["report"]["id"].as_str().unwrap();
    let path = format!("/api/chief/reports/{id}");
    let input = json!({"sessionId":"chief-a"}).to_string();
    let mut caller = std::net::TcpStream::connect(f.server.addr()).unwrap();
    write!(caller, "POST {path}/receive HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{input}", f.server.addr(), input.len()).unwrap();
    caller.flush().unwrap();
    // Keep transport alive until the public receipt confirms acceptance. An
    // immediate close can cancel an unaccepted HTTP body, which is not loss of
    // a committed receipt response. The caller never reads any response bytes.
    let receipt = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let result = body(get_over_tcp(f.server.addr(), &format!("{path}/receipt")).await, 200);
            if result["received"] == true { break result["receipt"].clone(); }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.expect("receipt must commit before losing the response");
    drop(caller); // accepted response is lost; no model presentation
    let before_restart = prompts(&f);
    drop(f.server);
    f.server = TestServer::start(&f.config).await;
    post(&f.server, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
    // Actual policy: online 10s + retention 30s + maintenance scan 5s. This
    // 46s window crosses restart/registration and the complete maintenance
    // upper bound; it is not a claim of exactly-once visible model output.
    tokio::time::sleep(Duration::from_secs(46)).await;
    assert_eq!(prompts(&f), before_restart, "no replay after receipt-before-presentation crash");
    assert_eq!(body(get_over_tcp(f.server.addr(), &format!("{path}/receipt")).await, 200), json!({"received":true,"receipt":receipt}));
    replacement_chief(&f, "chief-b").await;
    let before_recovery = prompts(&f);
    let old = post_json_over_tcp(f.server.addr(), &format!("{path}/receive"), &json!({"sessionId":"chief-a"}).to_string()).await;
    assert_eq!(old.status, 409, "{}", old.body);
    let viewed = post(&f.server, &format!("{path}/view"), json!({"sessionId":"chief-b"})).await;
    assert_eq!(viewed["report"], saved["report"]);
    assert_eq!(viewed["receipt"], receipt);
    assert_eq!(viewed["received"], true);
    assert_eq!(viewed["process"], false, "explicit view is not another receive grant");
    let old_view = post_json_over_tcp(f.server.addr(), &format!("{path}/view"), &json!({"sessionId":"chief-a"}).to_string()).await;
    assert_eq!(old_view.status, 409, "{}", old_view.body);
    let duplicate = post(&f.server, &format!("{path}/receive"), json!({"sessionId":"chief-b"})).await;
    assert_eq!(duplicate["process"], false);
    assert_eq!(duplicate["receipt"], receipt);
    let resend = post(&f.server, &format!("{path}/resend"), json!({"sessionId":"worker-a"})).await;
    assert_eq!(resend["received"], true);
    assert_eq!(resend["notification"]["status"], "not-attempted");
    assert_eq!(prompts(&f), before_recovery);
    assert_eq!(body(get_over_tcp(f.server.addr(), "/api/chief/reports").await, 200)["reports"], json!([saved["report"]]));
    // New work cannot be received by a revoked chief, even with a live process.
    let fresh = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("blocked")})).await;
    let fresh_path = format!("/api/chief/reports/{}/receive", fresh["report"]["id"].as_str().unwrap());
    let stale = post_json_over_tcp(f.server.addr(), &fresh_path, &json!({"sessionId":"chief-a"}).to_string()).await;
    assert_eq!(stale.status, 409, "{}", stale.body);
    let granted = post(&f.server, &fresh_path, json!({"sessionId":"chief-b"})).await;
    assert_eq!(granted["process"], true);
    assert_eq!(granted["receipt"]["sessionId"], "chief-b");
    println!("receipt crash recovery observationMs=46000 receipt={receipt} view={viewed}");
}

// Approved seam S2: actual HTTP, owned storage and real backend processes.
// Independent contract: only one concurrent receiver gets processing authority;
// notification submission itself must NOT be treated as reliable receipt.
// A non-persistent or read-then-write receipt admits two processing winners.
#[tokio::test]
async fn concurrent_receivers_grant_processing_once_and_preserve_receipt_after_restart() {
    let f = Fixture::new().await;
    delegated(&f).await;
    notification_behavior(&f, json!({}));
    let sent = post(&f.server, "/api/chief/reports", json!({"sessionId":"worker-a","body":content("success")})).await;
    assert_eq!(sent["notification"]["status"], "submitted");
    let id = sent["report"]["id"].as_str().unwrap();
    let receipt_path = format!("/api/chief/reports/{id}/receipt");
    let before = body(get_over_tcp(f.server.addr(), &receipt_path).await, 200);
    assert_eq!(before, json!({"received":false,"receipt":null}));
    let receive_path = format!("/api/chief/reports/{id}/receive");
    let request = json!({"sessionId":"chief-a"}).to_string();
    let (first, second) = tokio::join!(
        post_json_over_tcp(f.server.addr(), &receive_path, &request),
        post_json_over_tcp(f.server.addr(), &receive_path, &request)
    );
    let results = [body(first, 200), body(second, 200)];
    assert_eq!(results.iter().filter(|result| result["process"] == true).count(), 1);
    for result in &results {
        assert_eq!(result["received"], true);
        assert_eq!(result["report"], sent["report"]);
        assert_eq!(result["receipt"]["reportId"], id);
        assert_eq!(result["receipt"]["sessionId"], "chief-a");
        assert!(!result["receipt"]["claimId"].as_str().unwrap().is_empty());
    }
    assert_eq!(results[0]["receipt"], results[1]["receipt"]);
    let receipt = results[0]["receipt"].clone();
    drop(f.server);
    let recovered = TestServer::start(&f.config).await;
    post(&recovered, "/api/sessions/events", json!({"sessionId":"chief-a","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
    assert_eq!(body(get_over_tcp(recovered.addr(), &receipt_path).await, 200), json!({"received":true,"receipt":receipt}));
    let duplicate = post(&recovered, &receive_path, json!({"sessionId":"chief-a"})).await;
    assert_eq!(duplicate["received"], true);
    assert_eq!(duplicate["process"], false);
    assert_eq!(duplicate["receipt"], receipt);
    assert_eq!(duplicate["report"], sent["report"]);
}
