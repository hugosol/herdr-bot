
// Approved S2/S3/S4 seams. P17.03: A is still pending while its real
// initial-input call is gated. A queued dispatch must recheck the later running
// fact, not authorize from that pending snapshot. Literal rejection + no second
// host call distinguish stale authorization; both action snapshots must agree.
#[tokio::test]
async fn serialized_dispatch_rechecks_pending_snapshot_after_in_flight_launch() {
    let s = Scenario::new().await;
    s.start("manager-recheck").await;
    let release = s.data.path().join("recheck-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-recheck-A"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let addr = s.server.addr();
    let first = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-recheck","nodes":["A"]}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 2).await, "initial input must reach the external gate");
    let query = post_json_over_tcp(addr, "/api/workflow/manager/status", &json!({"sessionId":"manager-recheck"}).to_string()).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let before: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(before["snapshot"]["nodes"]["A"]["status"], "pending");
    assert_eq!(before["ready"], json!(["A", "B"]));
    let queued = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-recheck","nodes":["A"]}).to_string()).await;
    let queued = queued.response();
    tokio::pin!(queued);
    // Bounded negative HTTP observation, not a sleep used to order operations.
    assert!(tokio::time::timeout(Duration::from_millis(200), &mut queued).await.is_err(), "same-Run action must wait for the gated launch");
    let calls = s.herdr.calls();
    fs::write(&release, b"release").unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), first.response()).await.unwrap();
    assert_eq!(first.status, 200, "{}", first.body);
    let first: Value = serde_json::from_str(&first.body).unwrap();
    assert_eq!(first["results"][0]["status"], "running");
    let queued = tokio::time::timeout(Duration::from_secs(5), &mut queued).await.unwrap();
    assert_eq!(queued.status, 200, "{}", queued.body);
    let queued: Value = serde_json::from_str(&queued.body).unwrap();
    assert_eq!(queued["results"], json!([{"nodeId":"A","status":"rejected","reasons":["node 状态为 running，仅 pending 可派发"]}]));
    assert_eq!(queued["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(queued["ready"], json!(["B"]));
    for field in ["snapshot", "ready", "blocked", "canFinish"] {
        assert_eq!(queued[field], first[field]);
    }
    assert_eq!(s.herdr.calls(), calls, "queued dispatch must not create another Chat");
    println!("P17.03 observationWindowMs=200 before={before} first={first} queued={queued} calls={calls:?}");
}

// Approved S2/S3/S4 seams. P21.01/P36.02 (also P17.03): contract requires
// pause AFTER both A and B, not between targets. Two external input gates expose
// both intervals. Same-Run reads must pass (file coordination is not held), an
// independent Run must actually launch, and an existing Chat must still publish
// activity. Pending HTTP observations are bounded to 200ms per gate; gates, not
// elapsed time, establish order. History and literal action snapshots distinguish
// a per-target lock, a global lock, or a pause based on the old pending snapshot.
#[tokio::test]
async fn serialized_batch_finishes_before_pause_without_blocking_other_run_or_chat() {
    let s = Scenario::new().await;
    let first = s.start("manager-batch").await;
    let other = s.start("manager-independent").await;
    let root = s.config.scheduler.run_directory.join(first["id"].as_str().unwrap());
    let release_a = s.data.path().join("batch-A-release");
    let release_b = s.data.path().join("batch-B-release");
    let addr = s.server.addr();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"batch-node-A"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"match":"Node ID: A","wait_for_file":release_a,"timeout_ms":30000}}));
    let batch = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-batch","nodes":["A","B"]}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|c| c["kind"] == "agent_prompt" && c["text"].as_str().is_some_and(|t| t.contains("Node ID: A")))).await, "A must reach its external input gate");
    let pause = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/pause", &json!({"sessionId":"manager-batch"}).to_string()).await;
    let pause = pause.response();
    tokio::pin!(pause);
    assert!(tokio::time::timeout(Duration::from_millis(200), &mut pause).await.is_err(), "pause must wait while A input is in flight");
    let query = tokio::time::timeout(Duration::from_secs(5), post_json_over_tcp(addr, "/api/workflow/manager/status", &json!({"sessionId":"manager-batch"}).to_string())).await.expect("business wait must not hold Run file coordination");
    assert_eq!(query.status, 200, "{}", query.body);
    let during_a: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(during_a["snapshot"]["status"], "running");
    assert_eq!(during_a["snapshot"]["nodes"]["A"]["status"], "pending");
    assert_eq!(during_a["snapshot"]["nodes"]["B"]["status"], "pending");
    assert_eq!(during_a["snapshot"]["nodes"]["A"]["execution"]["chat"]["sessionId"], "batch-node-A");

    // Exercise an actual independent launch, not just a status read that never
    // needs the business boundary. B does not match the held A input gate.
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"independent-node-B"},"native_cwd":s.data.path().join("node-zone")}));
    let independent = tokio::time::timeout(Duration::from_secs(5), s.dispatch("manager-independent", &["B"])).await.expect("independent Run must advance before A gate is released");
    assert_eq!(independent["snapshot"]["id"], other["id"]);
    assert_eq!(independent["results"][0]["status"], "running");
    assert_eq!(independent["snapshot"]["nodes"]["B"]["execution"]["chat"]["sessionId"], "independent-node-B");
    for event in [json!({"sessionId":"batch-node-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":"batch-node-A","kind":"state","data":{"value":"working"}})] {
        let response = tokio::time::timeout(Duration::from_secs(5), post_json_over_tcp(addr, "/api/sessions/events", &event.to_string())).await.expect("existing Chat activity must not wait for batch/pause");
        assert_eq!(response.status, 200, "{}", response.body);
    }
    let mut feed = common::sse::SseConnection::open(addr, "/api/sessions/events", &[]).await;
    assert_eq!(feed.status, 200);
    let sessions = feed.next_event("snapshot").await.json();
    assert!(sessions["sessions"].as_array().unwrap().iter().any(|c| c["sessionId"] == "batch-node-A" && c["state"] == "working"));
    assert!(!release_a.exists());
    assert!(!release_b.exists());

    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"batch-node-B"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"match":"Node ID: B","wait_for_file":release_b,"timeout_ms":30000}}));
    fs::write(&release_a, b"release").unwrap();
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt" && c["text"].as_str().is_some_and(|t| t.contains("Node ID: B"))).count() == 2).await, "same batch B must still launch after A, before pause");
    assert!(tokio::time::timeout(Duration::from_millis(200), &mut pause).await.is_err(), "pause must wait for the FULL batch, not just A");
    let query = tokio::time::timeout(Duration::from_secs(5), post_json_over_tcp(addr, "/api/workflow/manager/status", &json!({"sessionId":"manager-batch"}).to_string())).await.expect("second host gate must also leave Run files readable");
    assert_eq!(query.status, 200, "{}", query.body);
    let during_b: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(during_b["snapshot"]["status"], "running");
    assert_eq!(during_b["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(during_b["snapshot"]["nodes"]["B"]["status"], "pending");
    assert_eq!(during_b["snapshot"]["nodes"]["B"]["execution"]["chat"]["sessionId"], "batch-node-B");
    let before_b = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(before_b.len(), 2);
    assert_eq!(before_b[0]["action"], "prepare_inputs");
    assert_eq!(before_b[1]["action"], "dispatch");
    assert_eq!(before_b[1]["nodeId"], "A");
    fs::write(&release_b, b"release").unwrap();
    let batch = tokio::time::timeout(Duration::from_secs(5), batch.response()).await.unwrap();
    assert_eq!(batch.status, 200, "{}", batch.body);
    let batch: Value = serde_json::from_str(&batch.body).unwrap();
    assert_eq!(batch["results"].as_array().unwrap().iter().map(|r| (r["nodeId"].as_str().unwrap(), r["status"].as_str().unwrap())).collect::<Vec<_>>(), vec![("A", "running"), ("B", "running")]);
    assert_eq!(batch["snapshot"]["status"], "running", "batch result snapshot precedes queued pause");
    assert_eq!(batch["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(batch["snapshot"]["nodes"]["B"]["status"], "running");
    let paused = tokio::time::timeout(Duration::from_secs(5), &mut pause).await.unwrap();
    assert_eq!(paused.status, 200, "{}", paused.body);
    let paused: Value = serde_json::from_str(&paused.body).unwrap();
    assert_eq!(paused["changed"], true);
    assert_eq!(paused["snapshot"]["status"], "paused");
    assert_eq!(paused["snapshot"]["nodes"], batch["snapshot"]["nodes"], "pause must recheck and preserve both newly committed executions");
    assert_eq!(paused["ready"], json!([]));
    let history = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(history.iter().map(|e| (e["action"].as_str().unwrap(), e["nodeId"].as_str())).collect::<Vec<_>>(), vec![("prepare_inputs", None), ("dispatch", Some("A")), ("dispatch", Some("B")), ("pause", None)]);
    let disk: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(disk, paused["snapshot"]);
    let calls = s.herdr.calls();
    assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_start").count(), 5, "two managers + independent B + full A/B batch, with no stop/relaunch");
    assert!(calls.iter().all(|c| matches!(c["kind"].as_str(), Some("workspace_list" | "tab_create" | "agent_start" | "agent_get" | "agent_prompt"))));
    println!("P17.03/P21.01/P36.02 observationWindowMsPerGate=200 duringA={during_a} independent={independent} chat={sessions} duringB={during_b} batch={batch} pause={paused} history={history:?} calls={calls:?}");
}

#[tokio::test]
async fn node_dispatch_commits_identity_before_literal_trigger_and_uses_manager_workspace() {
    let s = Scenario::new().await;
    let started = s.start("manager-05").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let release = s.data.path().join("prompt-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-native-05"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-05","nodes":["A"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 2).await;
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "node-native-05");
    let calls = s.herdr.calls();
    let prompt = calls.iter().filter(|c| c["kind"] == "agent_prompt").nth(1).unwrap();
    let text = common::prompt_body(prompt);
    assert!(text.starts_with(&format!("/skill:teach NODE-FIXED\n\nNode ID: A\nNode: Worker A\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\n", s.config.scheduler.run_instructions.display(),root.join("run.json").display(),root.join("history.jsonl").display(),root.join("messages.jsonl").display())));
    assert!(text.ends_with("\nNODE-INPUT\n第二行"));
    assert_eq!(saved["nodes"]["A"]["execution"]["reportPolicy"], "auto");
    let start = calls.iter().filter(|c| c["kind"] == "agent_start").nth(1).unwrap();
    assert_eq!(&start["args"].as_array().unwrap()[..4], &[json!("--model"),json!("openai-codex/gpt-6.1-sol"),json!("--thinking"),json!("high")]);
    let tab = s.herdr.tabs().pop().unwrap();
    assert_eq!(tab["workspace"], "w-manager");
    let native = common::read_json_lines(&s.herdr.path().join("started.jsonl")).pop().unwrap();
    assert_eq!(native["actual_cwd"], s.data.path().join("node-zone").to_string_lossy().as_ref());
    assert_eq!(native["workspace_id"], "w-manager");
    assert_eq!(tab["cwd"], s.data.path().join("node-zone").to_string_lossy().as_ref());
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let result: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
    let definition: Value = serde_json::from_slice(&fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap()).unwrap();
    assert_eq!(definition["nodes"]["A"]["input"], "NODE-INPUT\n第二行");
    assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await.status, 200);
}

// P11/P18/P19/P20/P38: one rejected target, one external start failure,
// one success; repeated dispatch must not create another Chat. The gate makes
// the failure deterministic without replacing the production host adapter.
#[tokio::test]
async fn mixed_batch_preserves_failures_success_and_other_run_without_automatic_actions() {
    let s = Scenario::new().await;
    let first = s.start("manager-first").await;
    let second = s.start("manager-second").await;
    let other_path = s.config.scheduler.run_directory.join(second["id"].as_str().unwrap()).join("run.json");
    let other_bytes = fs::read(&other_path).unwrap();
    let release = s.data.path().join("start-release");
    s.herdr.set_behavior(json!({"fail_agent_start":true,"agent_start_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-first","nodes":["C","A","B"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count() == 3).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-B-native"}}));
    fs::write(release,b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status,200,"{}",response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(batch["results"][0]["status"],"rejected");
    assert_eq!(batch["results"][1]["status"],"failed");
    assert_eq!(batch["results"][1]["failure"]["stage"],"start");
    assert_eq!(batch["results"][2]["status"],"running");
    assert_eq!(batch["snapshot"]["status"],"running");
    let root = s.config.scheduler.run_directory.join(first["id"].as_str().unwrap());
    let disk: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(disk["nodes"]["A"]["failure"],batch["results"][1]["failure"]);
    assert!(disk["nodes"]["A"]["failure"]["message"].as_str().unwrap().contains("受控夹具拒绝启动"));
    let history = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(history.len(),3);
    assert_eq!(history[0]["action"],"prepare_inputs");
    assert_eq!(history[1]["nodeId"],"A");
    assert_eq!(history[1]["status"],"failed");
    assert_eq!(history[2]["nodeId"],"B");
    assert_eq!(history[2]["status"],"running");
    let calls = s.herdr.calls();
    let repeated = s.dispatch("manager-first", &["A","B"]).await;
    assert_eq!(repeated["results"][0]["status"],"rejected");
    assert_eq!(repeated["results"][1]["status"],"rejected");
    assert_eq!(repeated["snapshot"],disk);
    let queried = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-first"}).to_string()).await;
    assert_eq!(serde_json::from_str::<Value>(&queried.body).unwrap()["snapshot"],disk);
    let other = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-second"}).to_string()).await;
    assert_eq!(serde_json::from_str::<Value>(&other.body).unwrap()["snapshot"],second["run"]);
    assert_eq!(fs::read(other_path).unwrap(),other_bytes);
    // Bounded negative observation: no retry, terminate, pause or hidden launch
    // during requests plus the following 100ms; not an infinite-time claim.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(s.herdr.calls(),calls);
    assert_eq!(fs::read_to_string(root.join("history.jsonl")).unwrap().lines().count(),3);
}

// P13.02/P45.02: identity and submit failures leave the external Chat intact;
// identity failure must never submit. These criteria already pass the launch
// implementation, so verify them without manufacturing an unrelated red.
#[tokio::test]
async fn identity_and_submit_failures_keep_external_effects_in_query_and_disk() {
    for stage in ["identity", "submit"] {
        let s = Scenario::new().await;
        let started = s.start("manager-errors").await;
        s.herdr.set_behavior(if stage == "identity" { json!({"fail_agent_get":true}) } else { json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-submit-failed"},"fail_prompt":true}) });
        let batch = s.dispatch("manager-errors", &["A"]).await;
        let failure = &batch["results"][0]["failure"];
        assert_eq!(failure["stage"],stage);
        assert_eq!(failure["effects"]["agentStarted"],true);
        assert_eq!(failure["effects"]["inputSubmitted"],false);
        assert_eq!(failure["effects"]["associationSaved"],stage == "submit");
        assert_eq!(failure["effects"]["inputAttempted"],stage == "submit");
        assert!(failure["effects"]["agentName"].is_string());
        let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
        let disk: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(disk["nodes"]["A"]["failure"],*failure);
        if stage == "submit" {
            assert_eq!(disk["nodes"]["A"]["execution"]["chat"]["sessionId"],"node-submit-failed");
        }
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(),if stage == "identity" {1} else {2});
        assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(),2);
        let queried = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-errors"}).to_string()).await;
        assert_eq!(serde_json::from_str::<Value>(&queried.body).unwrap()["snapshot"],disk);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(s.herdr.calls(),calls);
    }
}

#[tokio::test]
async fn association_write_failure_exposes_native_chat_without_submitting_or_claiming_rollback() {
    let s = Scenario::new().await;
    let started = s.start("manager-association").await;
    let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
    let original = fs::read(&path).unwrap();
    let release = s.data.path().join("identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-association-failed"},"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr,"/api/workflow/manager/dispatch",&json!({"sessionId":"manager-association","nodes":["A"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5),|| s.herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count() == 2).await;
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(release,b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status,200,"{}",response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(batch["results"][0]["failure"]["stage"],"association");
    assert_eq!(batch["results"][0]["effects"]["sessionId"],"node-association-failed");
    assert_eq!(batch["results"][0]["effects"]["agentStarted"],true);
    assert_eq!(batch["results"][0]["effects"]["associationSaved"],false);
    assert!(batch["results"][0]["persistenceError"].is_string());
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
    assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(),2);
    // Storage remains unavailable: response/diagnostic carry effects, and the
    // snapshot must not adopt the uncommitted candidate. No fake disk success.
    assert_eq!(batch["snapshot"],started["run"]);
    fs::remove_dir(&path).unwrap();
    fs::write(path,original).unwrap();
}

#[tokio::test]
async fn node_history_failure_retains_saved_identity_without_initial_prompt() {
    let s = Scenario::new().await;
    let started = s.start("manager-history").await;
    let release = s.data.path().join("history-identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-history-failed"},"native_cwd":s.data.path().join("node-zone"),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let request = common::OpenJsonRequest::submit(
        s.server.addr(), "/api/workflow/manager/dispatch",
        &json!({"sessionId":"manager-history","nodes":["A"]}).to_string(),
    ).await;
    assert!(common::wait_until(Duration::from_secs(5), || {
        s.herdr.calls().iter().filter(|call| call["kind"] == "agent_get").count() == 2
    }).await);
    let original_history = fs::read(&s.config.sessions.history_file).unwrap();
    fs::remove_file(&s.config.sessions.history_file).unwrap();
    fs::create_dir(&s.config.sessions.history_file).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.response().await;
    assert_eq!(response.status, 200, "{}", response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    let result = &batch["results"][0];
    assert_eq!(result["status"], "failed");
    assert_eq!(result["failure"]["stage"], "history");
    assert_eq!(result["effects"]["sessionId"], "node-history-failed");
    assert_eq!(result["effects"]["agentStarted"], true);
    assert_eq!(result["effects"]["associationSaved"], true);
    assert_eq!(result["effects"]["inputAttempted"], false);
    assert_eq!(result["effects"]["inputSubmitted"], false);
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved, batch["snapshot"]);
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "node-history-failed");
    assert_eq!(saved["nodes"]["A"]["failure"], result["failure"]);
    assert_eq!(common::read_json_lines(&root.join("history.jsonl"))[1]["failure"], result["failure"]);
    assert!(s.config.sessions.history_file.is_dir());
    let calls = s.herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 2);
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_prompt").count(), 1);
    let queried = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-history"}).to_string()).await;
    assert_eq!(queried.status, 200, "{}", queried.body);
    assert_eq!(serde_json::from_str::<Value>(&queried.body).unwrap()["snapshot"], saved);
    assert_eq!(s.herdr.calls(), calls);
    fs::remove_dir(&s.config.sessions.history_file).unwrap();
    fs::write(&s.config.sessions.history_file, original_history).unwrap();
}

#[tokio::test]
async fn missing_manager_workspace_never_falls_back_or_creates_a_tab() {
    let s = Scenario::new().await;
    s.start("manager-workspace").await;
    s.herdr.set_workspaces(json!([{"workspace_id":"w-default","label":"bot-playground","focused":true}]));
    let before = s.herdr.tabs();
    let batch = s.dispatch("manager-workspace", &["A"]).await;
    assert_eq!(batch["results"][0]["failure"]["stage"],"workspace");
    assert_eq!(batch["results"][0]["effects"]["agentStarted"],false);
    assert_eq!(s.herdr.tabs(),before);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(),1);
}

// A new Tab is insufficient: replaying the manager native identity must fail
// without sending node instructions, even though the host created another pane.
#[tokio::test]
async fn new_tab_with_reused_native_identity_is_not_accepted_as_independent_chat() {
    let s = Scenario::new().await;
    s.start("same-native").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"same-native"}}));
    let batch = s.dispatch("same-native", &["A"]).await;
    assert_eq!(batch["results"][0]["status"],"failed");
    assert_eq!(batch["results"][0]["failure"]["stage"],"identity");
    assert_eq!(batch["results"][0]["effects"]["sessionId"],"same-native");
    assert_eq!(s.herdr.tabs().len(),2);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
}

#[tokio::test]
async fn manager_query_derives_ready_targets_and_rejects_unassociated_dispatch() {
    let s = Scenario::new().await;
    s.start("manager-ready").await;
    let status = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-ready"}).to_string()).await;
    assert_eq!(status.status,200,"{}",status.body);
    let query: Value = serde_json::from_str(&status.body).unwrap();
    assert_eq!(query["ready"],json!(["A","B"]));
    assert_eq!(query["blocked"]["C"],json!(["前置 A 尚未 completed","前置 B 尚未 completed"]));
    assert_eq!(query["canFinish"],false);
    let calls = s.herdr.calls();
    for body in [json!({"sessionId":"stranger","nodes":["A"]}),json!({"sessionId":"manager-ready","nodes":["A"],"runId":"invented"})] {
        let response = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/dispatch",&body.to_string()).await;
        assert!(response.status == 403 || response.status == 400,"{}",response.body);
    }
    assert_eq!(s.herdr.calls(),calls);
}
