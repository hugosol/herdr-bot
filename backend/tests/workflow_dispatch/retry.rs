//! Approved S2/S3/S4: HTTP, real storage, production Herdr adapter with owned host.
use super::*;

async fn action(s: &Scenario, name: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn failed_execution(s: &Scenario) -> (Value, Value) {
    let started = s.start("retry-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-old"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("retry-manager", &["A"]).await;
    let old = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    action(s, "fail", json!({"sessionId":"retry-manager","nodeId":"A","reason":"Original execution left partial output"}), 200).await;
    (started, old)
}
async fn report(s: &Scenario, identity: &str, body: &str) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":identity,"body":body}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str::<Value>(&response.body).unwrap()["report"].clone()
}

// P27.02/P41.01/P68.07: old/current/old/current reports retain all literal
// bodies, but only the current execution's latest ID is a handoff pending item.
// Plausible violation: last report wins regardless of execution, or old Chat
// loses report access. Use the real report/collect/complete HTTP interfaces.
#[tokio::test]
async fn late_old_reports_remain_history_without_controlling_new_execution() {
    let s = Scenario::new().await;
    let (started, old) = failed_execution(&s).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = report(&s, "retry-old", "OLD-1: partial output only").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retry = action(&s, "retry", retry_body(), 200).await;
    let new = retry["snapshot"]["nodes"]["A"]["execution"].clone();
    assert!(retry["pendingReports"].get("A").is_none());
    let late = report(&s, "retry-old", "OLD-2: late old completion claim").await;
    let query = action(&s, "status", json!({"sessionId":"retry-manager"}), 200).await;
    assert!(query["pendingReports"].get("A").is_none());
    assert_eq!(query["snapshot"]["nodes"]["A"]["status"], "running");
    let current = report(&s, "retry-new", "NEW-1: current result").await;
    let later = report(&s, "retry-old", "OLD-3: newer timestamp is not applicability").await;
    let collect = action(&s, "collect", json!({"sessionId":"retry-manager"}), 200).await;
    assert_eq!(collect["pendingReports"]["A"], current["id"]);
    let latest = report(&s, "retry-new", "NEW-2: current final result").await;
    let query = action(&s, "status", json!({"sessionId":"retry-manager"}), 200).await;
    assert_eq!(query["pendingReports"]["A"], latest["id"]);
    assert_eq!(query["reports"], json!([first, late, current, later, latest]));
    for index in [0, 1, 3] {
        assert_eq!(query["reports"][index]["executionId"], old["id"]);
        assert_eq!(query["reports"][index]["source"], old["chat"]);
    }
    for index in [2, 4] { assert_eq!(query["reports"][index]["executionId"], new["id"]); }
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), query["reports"].as_array().unwrap().to_owned());
    let calls = s.herdr.calls();
    let denied = action(&s, "complete", json!({"sessionId":"retry-old","nodeId":"A","reason":"old cannot complete new"}), 403).await;
    assert!(denied["code"].is_string());
    for name in ["fail", "retry"] {
        let mut body = retry_body(); body["sessionId"] = json!("retry-old");
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
        assert!(response.status == 400 || response.status == 403);
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(action(&s, "status", json!({"sessionId":"retry-manager"}), 200).await["snapshot"], retry["snapshot"]);
    let completed = action(&s, "complete", json!({"sessionId":"retry-manager","nodeId":"A","reason":"Accept only NEW-2"}), 200).await;
    assert!(completed["pendingReports"].get("A").is_none());
    report(&s, "retry-old", "OLD-4: after handoff").await;
    assert!(action(&s, "status", json!({"sessionId":"retry-manager"}), 200).await["pendingReports"].get("A").is_none());
}

// P14.02/P41.01: a third Chat cannot reuse the first execution's native
// identity just because the second execution is now current. The host remains
// real; only its external identity response is controlled.
#[tokio::test]
async fn retry_rejects_reuse_of_any_retained_native_identity() {
    let s = Scenario::new().await;
    let (_, old) = failed_execution(&s).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-second"},"native_cwd":s.data.path().join("node-zone")}));
    let second = action(&s, "retry", retry_body(), 200).await;
    action(&s, "fail", json!({"sessionId":"retry-manager","nodeId":"A","reason":"Second attempt also failed"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-old"},"native_cwd":s.data.path().join("node-zone")}));
    let result = action(&s, "retry", retry_body(), 200).await;
    assert_eq!(result["result"]["status"], "failed");
    assert_eq!(result["result"]["failure"]["stage"], "identity");
    assert_eq!(result["result"]["effects"]["sessionId"], "retry-old");
    assert_eq!(result["result"]["effects"]["inputAttempted"], false);
    assert!(result["snapshot"]["nodes"]["A"]["execution"]["chat"].is_null());
    assert_eq!(result["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], old);
    assert_eq!(result["snapshot"]["nodes"]["A"]["previousExecutions"][1]["execution"], second["snapshot"]["nodes"]["A"]["execution"]);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
    assert_eq!(report(&s, "retry-old", "Original remains unambiguous").await["executionId"], old["id"]);
}

// P13.03/P45.03/P41.01: controlled identity/submit failure must retain a
// distinct attempt and old failure; errors cannot be attributed to the old Chat.
// Existing shared launch behavior is verified directly, without fabricated red.
#[tokio::test]
async fn retry_startup_failures_keep_both_attempts_and_do_not_rollback() {
    for stage in ["identity", "submit"] {
        let s = Scenario::new().await;
        let (started, old) = failed_execution(&s).await;
        let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
        let artifact = s.data.path().join("node-zone/old-artifact");
        fs::write(&artifact, "keep old side effects").unwrap();
        let before_calls = s.herdr.calls().len();
        s.herdr.set_behavior(if stage == "identity" { json!({"fail_agent_get":true}) } else { json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-submit-failure"},"native_cwd":s.data.path().join("node-zone"),"fail_prompt":true}) });
        let retried = action(&s, "retry", retry_body(), 200).await;
        let node = &retried["snapshot"]["nodes"]["A"];
        assert_eq!(retried["result"]["status"], "failed");
        assert_eq!(node["failure"]["stage"], stage);
        assert_ne!(node["execution"]["id"], old["id"]);
        assert_eq!(node["previousExecutions"][0]["execution"], old);
        assert_eq!(node["previousExecutions"][0]["failure"]["message"], "Original execution left partial output");
        let effects = &retried["result"]["effects"];
        assert_eq!(effects["agentStarted"], true);
        assert_eq!(effects["associationSaved"], stage == "submit");
        assert_eq!(effects["inputAttempted"], stage == "submit");
        assert_eq!(effects["inputSubmitted"], false);
        assert_ne!(effects["agentName"], old["chat"]["agentName"]);
        assert_ne!(effects["tabId"], old["chat"]["tabId"]);
        if stage == "submit" { assert_eq!(node["execution"]["chat"]["sessionId"], "retry-submit-failure"); }
        else { assert!(node["execution"]["chat"].is_null()); }
        assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(), 3);
        let events = common::read_json_lines(&root.join("history.jsonl"));
        assert_eq!(events[2]["action"], "retry");
        assert_eq!(events[2]["failure"], node["failure"]);
        assert_eq!(events[2]["previousExecution"], old);
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), retried["snapshot"]);
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), if stage == "identity" { 2 } else { 3 });
        let expected = if stage == "identity" { vec!["workspace_list", "tab_create", "agent_start", "agent_get"] } else { vec!["workspace_list", "tab_create", "agent_start", "agent_get", "agent_prompt"] };
        assert_eq!(calls[before_calls..].iter().map(|call| call["kind"].as_str().unwrap()).collect::<Vec<_>>(), expected);
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(s.herdr.calls(), calls);
        assert_eq!(fs::read_to_string(artifact).unwrap(), "keep old side effects");
    }
}

// P13.03/P45.03: disk becomes unavailable only after new Chat started.
// Result must identify the uncommitted new attempt, snapshot stays old, no input.
#[tokio::test]
async fn retry_association_failure_exposes_new_attempt_without_overwriting_old_snapshot() {
    let s = Scenario::new().await;
    let (started, old) = failed_execution(&s).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("run.json");
    let original = fs::read(&path).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let release = s.data.path().join("retry-identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-uncommitted"},"native_cwd":s.data.path().join("node-zone"),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/retry", &retry_body().to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count() == 3).await;
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count(), 3);
    fs::remove_file(&path).unwrap(); fs::create_dir(&path).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    // Restore the owned fault before assertions, preserving original facts.
    fs::remove_dir(&path).unwrap(); fs::write(&path, &original).unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let retry: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(retry["result"]["failure"]["stage"], "association");
    assert_eq!(retry["result"]["effects"]["sessionId"], "retry-uncommitted");
    assert_eq!(retry["result"]["effects"]["agentStarted"], true);
    assert_eq!(retry["result"]["effects"]["associationSaved"], false);
    assert_eq!(retry["result"]["effects"]["inputAttempted"], false);
    assert!(retry["result"]["persistenceError"].is_string());
    assert!(retry["result"]["execution"]["id"].is_string());
    assert_ne!(retry["result"]["execution"]["id"], old["id"]);
    assert_eq!(retry["result"]["execution"]["chat"]["sessionId"], "retry-uncommitted");
    assert_eq!(retry["snapshot"], serde_json::from_slice::<Value>(&original).unwrap());
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(), 2);
    assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(), 3);
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
}

// P36.03/P41.01: pause rejects before any external call; resume alone does
// nothing. Explicit confirmed retry afterward is the only new launch.
#[tokio::test]
async fn paused_retry_requires_explicit_resume_and_human_confirmation() {
    let s = Scenario::new().await;
    let (started, _) = failed_execution(&s).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    action(&s, "pause", json!({"sessionId":"retry-manager"}), 200).await;
    let before = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    let paused = action(&s, "retry", retry_body(), 409).await;
    assert_eq!(paused["code"], "run-paused");
    assert_eq!(paused["changed"], false);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    action(&s, "resume", json!({"sessionId":"retry-manager"}), 200).await;
    for body in [json!({"sessionId":"retry-manager","nodeId":"A","confirmed":false,"reason":"Not checked"}), json!({"sessionId":"retry-manager","nodeId":"A","confirmed":true,"reason":"  "})] {
        assert_eq!(action(&s, "retry", body, 409).await["code"], "retry-confirmation-required");
    }
    for identity in ["retry-old", "stranger"] {
        let mut body = retry_body(); body["sessionId"] = json!(identity);
        action(&s, "retry", body, 403).await;
    }
    for field in ["runId", "executionId", "role"] {
        let mut body = retry_body(); body[field] = json!("forged");
        action(&s, "retry", body, 400).await;
    }
    assert_eq!(action(&s, "retry", json!({"sessionId":"retry-manager","nodeId":"B","confirmed":true,"reason":"pending is not failed"}), 409).await["code"], "node-not-retryable");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"resumed-retry"},"native_cwd":s.data.path().join("node-zone")}));
    assert_eq!(action(&s, "retry", retry_body(), 200).await["result"]["status"], "running");
    let calls = s.herdr.calls();
    assert_eq!(action(&s, "retry", retry_body(), 409).await["code"], "node-not-retryable");
    assert_eq!(s.herdr.calls(), calls);
}

// P35.04: reach terminal through actual handoffs, not a fabricated snapshot.
// Retry cannot create another Chat or alter the first finish/old failure facts.
#[tokio::test]
async fn terminal_run_rejects_retry_without_new_chat_or_record_changes() {
    let s = Scenario::new().await;
    let (started, _) = failed_execution(&s).await;
    action(&s, "complete", json!({"sessionId":"retry-manager","nodeId":"A","reason":"User supplied final result"}), 200).await;
    for (node, native) in [("B", "terminal-B"), ("C", "terminal-C")] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":native},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("retry-manager", &[node]).await;
        action(&s, "complete", json!({"sessionId":"retry-manager","nodeId":node,"reason":"Verified final output"}), 200).await;
    }
    let finished = action(&s, "finish", json!({"sessionId":"retry-manager"}), 200).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let bytes = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    let denied = action(&s, "retry", retry_body(), 409).await;
    assert_eq!(denied["code"], "run-terminal");
    assert_eq!(denied["snapshot"], finished["snapshot"]);
    assert_eq!(action(&s, "status", json!({"sessionId":"retry-manager"}), 200).await["snapshot"], finished["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), bytes);
}

// P41.01: a dispatch whose native identity could not be observed must first
// be reconciled. Human confirmation on retry alone cannot erase uncertainty.
#[tokio::test]
async fn uncertain_dispatch_cannot_retry_until_original_chat_is_reconciled() {
    let s = Scenario::new().await;
    let started = s.start("retry-manager").await;
    s.herdr.set_behavior(json!({"fail_agent_get":true}));
    let failed = s.dispatch("retry-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let original = failed["snapshot"]["nodes"]["A"]["execution"].clone();
    let calls = s.herdr.calls();
    assert_eq!(action(&s, "retry", retry_body(), 409).await["code"], "dispatch-reconciliation-required");
    let uncertain = action(&s, "reconcile_dispatch", json!({"sessionId":"retry-manager","nodeId":"A","finding":"uncertain","reason":"User cannot yet find original native Chat"}), 200).await;
    assert_eq!(uncertain["snapshot"], failed["snapshot"]);
    let bytes = fs::read(root.join("history.jsonl")).unwrap();
    assert_eq!(action(&s, "retry", retry_body(), 409).await["code"], "dispatch-reconciliation-required");
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), bytes);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"reconciled-original"},"native_cwd":s.data.path().join("node-zone")}));
    let reconciled = action(&s, "reconcile_dispatch", json!({"sessionId":"retry-manager","nodeId":"A","finding":"created","reason":"User located original host agent"}), 200).await;
    assert_eq!(reconciled["snapshot"]["nodes"]["A"]["execution"]["id"], original["id"]);
    action(&s, "fail", json!({"sessionId":"retry-manager","nodeId":"A","reason":"Original Chat confirmed unusable"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"after-reconciliation-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retry = action(&s, "retry", retry_body(), 200).await;
    assert_eq!(retry["result"]["status"], "running");
    assert_ne!(retry["snapshot"]["nodes"]["A"]["execution"]["id"], original["id"]);
    assert_eq!(retry["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], reconciled["snapshot"]["nodes"]["A"]["execution"]);
    assert_eq!(common::read_json_lines(&root.join("history.jsonl"))[0]["failure"]["stage"], "identity");
}

// P14.02/P41.01: unavailable actual Workspace or definition zone cannot
// fall back. Even resolution failure has its own attempt identity and cause.
#[tokio::test]
async fn unavailable_retry_targets_fail_without_fallback_or_reusing_old_execution() {
    for target in ["workspace", "zone"] {
        let s = Scenario::new().await;
        let (started, old) = failed_execution(&s).await;
        let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
        let before_tabs = s.herdr.tabs();
        if target == "workspace" {
            s.herdr.set_workspaces(json!([{"workspace_id":"w-default","label":"bot-playground","focused":true}]));
        } else { fs::remove_dir(s.data.path().join("node-zone")).unwrap(); }
        let retry = action(&s, "retry", retry_body(), 200).await;
        assert_eq!(retry["result"]["status"], "failed");
        assert_eq!(retry["result"]["failure"]["stage"], if target == "workspace" { "workspace" } else { "resolve" });
        let node = &retry["snapshot"]["nodes"]["A"];
        assert!(node["execution"]["id"].is_string());
        assert_ne!(node["execution"]["id"], old["id"]);
        assert!(node["execution"]["chat"].is_null());
        assert_eq!(node["previousExecutions"][0]["execution"], old);
        assert_eq!(node["previousExecutions"][0]["failure"]["stage"], "execution");
        assert_eq!(common::read_json_lines(&root.join("history.jsonl"))[2]["execution"], node["execution"]);
        assert_eq!(s.herdr.tabs(), before_tabs);
        assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 2);
        let calls = s.herdr.calls();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(s.herdr.calls(), calls);
    }
}

fn retry_body() -> Value {
    json!({"sessionId":"retry-manager","nodeId":"A","confirmed":true,"reason":"User checked old Chat and partial output; start independent work"})
}

// P13.03/P14.02/P15.03/P41.01/P68.07: literal trigger and retained artifact
// distinguish a fresh complete launch from input-only replay or old-Chat reuse.
// At the external prompt gate, the new association must already be durable.
#[tokio::test]
async fn retry_saves_new_identity_before_full_trigger_and_retains_original_facts() {
    let s = Scenario::new().await;
    let (started, old) = failed_execution(&s).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let artifact = s.data.path().join("node-zone/partial-result.txt");
    fs::write(&artifact, "OLD SIDE EFFECT: retain exactly\n").unwrap();
    let before_calls = s.herdr.calls().len();
    let release = s.data.path().join("retry-prompt-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retry-new"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/retry", &retry_body().to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 3).await;
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    let new = &saved["nodes"]["A"]["execution"];
    assert_eq!(new["chat"]["sessionId"], "retry-new");
    assert_ne!(new["id"], old["id"]);
    assert_eq!(saved["nodes"]["A"]["previousExecutions"][0]["execution"], old);
    assert_eq!(saved["nodes"]["A"]["previousExecutions"][0]["failure"]["message"], "Original execution left partial output");
    let calls = s.herdr.calls();
    assert_eq!(calls[before_calls..].iter().map(|call| call["kind"].as_str().unwrap()).collect::<Vec<_>>(), ["workspace_list", "tab_create", "agent_start", "agent_get", "agent_prompt"]);
    let prompt = calls.iter().filter(|c| c["kind"] == "agent_prompt").nth(2).unwrap();
    assert_eq!(prompt["text"], format!("/skill:teach NODE-FIXED\n\nNode ID: A\nNode: Worker A\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\nNODE-INPUT\n第二行", s.config.scheduler.run_instructions.display(), root.join("run.json").display(), root.join("history.jsonl").display(), root.join("messages.jsonl").display()));
    let launch = calls.iter().filter(|c| c["kind"] == "agent_start").nth(2).unwrap();
    assert_eq!(&launch["args"].as_array().unwrap()[..4], &[json!("--model"), json!("openai-codex/gpt-6.1-sol"), json!("--thinking"), json!("high")]);
    let native = common::read_json_lines(&s.herdr.path().join("started.jsonl")).pop().unwrap();
    assert_eq!(native["actual_cwd"], s.data.path().join("node-zone").to_string_lossy().as_ref());
    assert_eq!(native["workspace_id"], "w-manager");
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let retried: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(retried["result"]["status"], "running");
    assert_eq!(retried["snapshot"]["nodes"]["A"]["status"], "running");
    assert!(retried["snapshot"]["nodes"]["A"]["failure"].is_null());
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["action"], "retry");
    assert_eq!(events[2]["reason"], "User checked old Chat and partial output; start independent work");
    assert_eq!(events[2]["previousExecution"], old);
    assert_eq!(events[2]["previousFailure"]["message"], "Original execution left partial output");
    assert_eq!(s.herdr.tabs().len(), 3);
    assert_eq!(fs::read_to_string(artifact).unwrap(), "OLD SIDE EFFECT: retain exactly\n");
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
}
