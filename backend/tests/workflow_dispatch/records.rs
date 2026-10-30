//! Ticket 14 approved seams: S2 real TCP HTTP and S4 public run records.
//!
//! P43 already met its criterion before changes; its first run was green.
//! P44 below requires explicit partial-effect feedback, not an unexplained OS
//! error or rollback claim. The independent expected outcome is the contract's
//! committed completion and absent completion history after append fails.
use super::*;

async fn rename(s: &Scenario, id: &str, body: Value) -> common::HttpResponse {
    common::request_json_over_tcp(s.server.addr(), "PATCH", &format!("/api/workflow/runs/{id}"), Some(&body.to_string())).await
}

#[tokio::test]
async fn historical_run_rename_trims_preserves_records_and_survives_restart() {
    let s = Scenario::new().await;
    let original = json!({"id":"rename-history","map":"removed-map.json","createdAt":100,"managerTaskId":"retired-task","zone":"C:/historical","workspaceName":"old-workspace","userInput":"original input","manager":null,"status":"completed","finished":null,"nodes":{}});
    for id in ["rename-history", "rename-duplicate"] {
        let root = s.config.scheduler.run_directory.join(id);
        fs::create_dir_all(&root).unwrap();
        let mut run = original.clone();
        run["id"] = json!(id);
        fs::write(root.join("run.json"), run.to_string()).unwrap();
        fs::write(root.join("history.jsonl"), b"").unwrap();
        fs::write(root.join("messages.jsonl"), b"").unwrap();
    }
    let root = s.config.scheduler.run_directory.join("rename-history");
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    for body in [json!({"name":" \n\t "}), json!({"name":null}), json!({"name":3}), json!({}), json!({"name":"valid","status":"running"})] {
        let response = rename(&s, "rename-history", body).await;
        assert_eq!(response.status, 400, "{}", response.body);
        assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    }
    let calls = s.herdr.calls();
    let response = rename(&s, "rename-history", json!({"name":"  历史 Run  \n"})).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let renamed: Value = serde_json::from_str(&response.body).unwrap();
    let mut expected = original;
    expected["name"] = json!("历史 Run");
    assert_eq!(renamed["run"], expected);
    assert_eq!(renamed["managed"], false);
    assert_eq!(renamed["reports"], json!([]));
    assert_eq!(renamed["pendingReports"], json!({}));
    let get = get_over_tcp(s.server.addr(), "/api/workflow/runs/rename-history").await;
    assert_eq!(serde_json::from_str::<Value>(&get.body).unwrap(), renamed);
    assert_eq!(rename(&s, "rename-duplicate", json!({"name":"历史 Run"})).await.status, 200);
    assert_eq!(s.herdr.calls(), calls, "human names never operate Chats");
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), before[1]);
    assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), before[2]);
    drop(s.server);
    let restarted = TestServer::start_app(herdr_bot::app(&s.config)).await;
    let get = get_over_tcp(restarted.addr(), "/api/workflow/runs/rename-history").await;
    assert_eq!(get.status, 200, "{}", get.body);
    assert_eq!(serde_json::from_str::<Value>(&get.body).unwrap(), renamed);
    let list = get_over_tcp(restarted.addr(), "/api/workflow/runs").await;
    let list: Value = serde_json::from_str(&list.body).unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 2);
    assert!(list["runs"].as_array().unwrap().iter().all(|run| run["run"]["name"] == "历史 Run"));
    drop(restarted);
}

#[tokio::test]
async fn rename_during_startup_survives_late_association_and_workflow_writes() {
    let s = Scenario::new().await;
    let release = s.data.path().join("rename-start-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"rename-manager"},"native_cwd":s.data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let pending = common::OpenJsonRequest::submit(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"original input"}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|call| call["kind"] == "agent_get")).await);
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    let list: Value = serde_json::from_str(&list.body).unwrap();
    let id = list["runs"][0]["id"].as_str().unwrap();
    let before = list["runs"][0]["run"].clone();
    let renamed = tokio::time::timeout(Duration::from_secs(2), rename(&s, id, json!({"name":"Startup name"}))).await.expect("rename must not wait for external identity");
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let renamed: Value = serde_json::from_str(&renamed.body).unwrap();
    let mut expected = before;
    expected["name"] = json!("Startup name");
    assert_eq!(renamed["run"], expected);
    assert_eq!(renamed["managed"], true);
    fs::write(&release, b"release").unwrap();
    let started = tokio::time::timeout(Duration::from_secs(5), pending.response()).await.unwrap();
    assert_eq!(started.status, 201, "{}", started.body);
    let started: Value = serde_json::from_str(&started.body).unwrap();
    assert_eq!(started["run"]["name"], "Startup name");
    assert_eq!(started["run"]["manager"]["sessionId"], "rename-manager");
    for action in ["pause", "resume"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), r#"{"sessionId":"rename-manager"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["snapshot"]["name"], "Startup name");
    }
    let renamed = rename(&s, id, json!({"name":"Final name"})).await;
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let renamed: Value = serde_json::from_str(&renamed.body).unwrap();
    let mut expected = started["run"].clone();
    expected["name"] = json!("Final name");
    assert_eq!(renamed["run"], expected, "rename preserves workflow state and all identities");
    drop(s.server);
    let restarted = TestServer::start_app(herdr_bot::app(&s.config)).await;
    let get = get_over_tcp(restarted.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(get.status, 200, "{}", get.body);
    let get: Value = serde_json::from_str(&get.body).unwrap();
    assert_eq!(get["run"], expected);
    assert_eq!(get["managed"], false);
    drop(restarted);
}

#[tokio::test]
async fn rename_waits_for_in_flight_dispatch_and_retains_its_committed_facts() {
    let s = Scenario::new().await;
    let started = s.start("rename-dispatch-manager").await;
    let id = started["id"].as_str().unwrap();
    assert_eq!(rename(&s, id, json!({"name":"Before dispatch"})).await.status, 200);
    let release = s.data.path().join("rename-dispatch-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"rename-node-A"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let dispatch = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", r#"{"sessionId":"rename-dispatch-manager","nodes":["A"]}"#).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2).await);
    let addr = s.server.addr();
    let path = format!("/api/workflow/runs/{id}");
    let queued = tokio::spawn(async move {
        common::request_json_over_tcp(addr, "PATCH", &path, Some(r#"{"name":"After dispatch"}"#)).await
    });
    tokio::pin!(queued);
    assert!(tokio::time::timeout(Duration::from_millis(200), &mut queued).await.is_err(), "rename must serialize with an accepted workflow mutation");
    fs::write(&release, b"release").unwrap();
    let dispatched = tokio::time::timeout(Duration::from_secs(5), dispatch.response()).await.unwrap();
    assert_eq!(dispatched.status, 200, "{}", dispatched.body);
    let dispatched: Value = serde_json::from_str(&dispatched.body).unwrap();
    assert_eq!(dispatched["snapshot"]["name"], "Before dispatch");
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running");
    let renamed = tokio::time::timeout(Duration::from_secs(5), &mut queued).await.unwrap().unwrap();
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let renamed: Value = serde_json::from_str(&renamed.body).unwrap();
    let mut expected = dispatched["snapshot"].clone();
    expected["name"] = json!("After dispatch");
    assert_eq!(renamed["run"], expected);
    let paused = post_json_over_tcp(addr, "/api/workflow/manager/pause", r#"{"sessionId":"rename-dispatch-manager"}"#).await;
    assert_eq!(paused.status, 200, "{}", paused.body);
    let paused: Value = serde_json::from_str(&paused.body).unwrap();
    assert_eq!(paused["snapshot"]["name"], "After dispatch");
    assert_eq!(paused["snapshot"]["nodes"], expected["nodes"]);
}

// P46.01: independent public-format literals deliberately disagree: history
// says running, current facts say paused. Replaying history or deriving current
// execution from the latest (old-execution) report would give a wrong result.
// Read each disk record plus the existing HTTP detail; no private state seam.
#[tokio::test]
async fn independent_records_read_current_facts_without_replaying_history() {
    let s = Scenario::new().await;
    let root = s.config.scheduler.run_directory.join("independent-records");
    fs::create_dir_all(&root).unwrap();
    let manager = json!({"sessionId":"fixture-manager","workspaceId":"w-fixture","tabId":"tm","paneId":"pm","agentName":"am"});
    let old_chat = json!({"sessionId":"fixture-old","workspaceId":"w-fixture","tabId":"to","paneId":"po","agentName":"ao"});
    let chat = json!({"sessionId":"fixture-current","workspaceId":"w-fixture","tabId":"tc","paneId":"pc","agentName":"ac"});
    let current = json!({"id":"independent-records","map":"work.json","createdAt":100,"managerTaskId":"fixture-manager-task","zone":"C:/fixture/manager","workspaceName":"fixture-workspace","userInput":"INDEPENDENT-INPUT","manager":manager,"status":"paused","finished":null,"nodes":{
        "A":{"status":"failed","execution":{"id":"execution-current","taskId":"fixture-node-task","agent":"omp","zone":"C:/fixture/node","chat":chat},"failure":{"stage":"execution","message":"CURRENT dependency unavailable"},"completion":null,"previousExecutions":[{"execution":{"id":"execution-old","taskId":"fixture-node-task","agent":"omp","zone":"C:/fixture/node","chat":old_chat},"failure":{"stage":"execution","message":"OLD artifact invalid"}}]},
        "B":{"status":"completed","execution":null,"failure":null,"completion":{"id":"completion-fixture","at":120,"actor":manager,"reason":"User supplied verified results/B.md; manual limitation retained"},"previousExecutions":[]}
    }});
    let history = json!({"id":"history-independent","at":110,"actor":manager,"action":"resume","before":"paused","after":"running"});
    let report = json!({"id":"report-independent","type":"report","at":130,"runId":"independent-records","nodeId":"A","executionId":"execution-old","source":old_chat,"target":manager,"body":"OLD-REPORT: partial results/old.md"});
    fs::write(root.join("run.json"), current.to_string()).unwrap();
    fs::write(root.join("history.jsonl"), format!("{history}\n")).unwrap();
    fs::write(root.join("messages.jsonl"), format!("{report}\n")).unwrap();
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let response = get_over_tcp(s.server.addr(), "/api/workflow/runs/independent-records").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let detail: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(detail["managed"], false);
    assert_eq!(detail["run"]["status"], "paused");
    assert_eq!(detail["run"]["nodes"]["A"]["execution"]["id"], "execution-current");
    assert_eq!(detail["run"]["nodes"]["A"]["previousExecutions"][0]["failure"]["message"], "OLD artifact invalid");
    assert_eq!(detail["reports"], json!([report]));
    assert_eq!(detail["pendingReports"], json!({}));
    assert_eq!(common::read_json_lines(&root.join("history.jsonl")), vec![history]);
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), vec![report]);
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    let mut names = fs::read_dir(&root).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["history.jsonl", "messages.jsonl", "run.json"]);
    println!("P46.01 independent detail={detail} files={names:?}");
}

// P46.01 real operations retain failure/recovery/completion explanations and
// old executions/reports in their separate records. Queries and collect must
// not append a fourth business record or consume report history.
#[tokio::test]
async fn operated_records_preserve_old_execution_recovery_completion_and_reports() {
    let s = Scenario::new().await;
    let started = s.start("records-manager").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-old-A"},"native_cwd":s.data.path().join("node-zone")}));
    let launched = s.dispatch("records-manager", &["A"]).await;
    let old = launched["snapshot"]["nodes"]["A"]["execution"].clone();
    action(&s, "fail", json!({"nodeId":"A","reason":"First failure: missing dependency"}), 200).await;
    action(&s, "recover", json!({"nodeId":"A","confirmed":true,"reason":"User repaired dependency; continue original Chat"}), 200).await;
    action(&s, "fail", json!({"nodeId":"A","reason":"Second failure: original output invalid"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-new-A"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = action(&s, "retry", json!({"nodeId":"A","confirmed":true,"reason":"User checked old effects; start independent execution"}), 200).await;
    assert_ne!(retried["snapshot"]["nodes"]["A"]["execution"]["id"], old["id"]);
    let mut reports = Vec::new();
    for (session, text) in [("records-new-A", "NEW results/A.md; verified"), ("records-old-A", "OLD late cleanup; not new handoff")] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":session,"body":text}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["saved"], true);
        reports.push(body["report"].clone());
    }
    let pending = action(&s, "status", json!({}), 200).await;
    assert_eq!(pending["pendingReports"], json!({"A":reports[0]["id"]}));
    let completed = action(&s, "complete", json!({"nodeId":"A","reason":"User verified NEW results/A.md; old cleanup is not handed off"}), 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], old);
    assert_eq!(completed["snapshot"]["nodes"]["A"]["previousExecutions"][0]["failure"]["message"], "Second failure: original output invalid");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["completion"]["reason"], "User verified NEW results/A.md; old cleanup is not handed off");
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let detail: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(detail["run"], completed["snapshot"]);
    assert_eq!(detail["reports"], json!(reports));
    assert_eq!(detail["pendingReports"], json!({}));
    assert_eq!(action(&s, "collect", json!({}), 200).await["reason"], "notification");
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.iter().map(|event| event["action"].as_str().unwrap()).collect::<Vec<_>>(), vec!["dispatch", "fail", "recover", "fail", "retry", "complete"]);
    assert_eq!(events[1]["reason"], "First failure: missing dependency");
    assert_eq!(events[2]["reason"], "User repaired dependency; continue original Chat");
    assert_eq!(events[4]["previousExecution"], old);
    assert_eq!(events[5]["reason"], "User verified NEW results/A.md; old cleanup is not handed off");
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    let mut names = fs::read_dir(&root).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["history.jsonl", "messages.jsonl", "run.json"]);
    println!("P46.01 operated detail={detail} history={events:?} files={names:?}");
}

async fn action(s: &Scenario, name: &str, extra: Value, status: u16) -> Value {
    let mut body = extra;
    body["sessionId"] = json!("records-manager");
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// P43.01 (contract literal): a failed current-state commit cannot become a
// current fact or success history. Plausible violation: publishing the mutated
// candidate before replace succeeds. Real Windows readonly files leave replace
// and HTTP handling exercised; query while readonly, disk bytes, history, and
// the next explicit legal call distinguish the violation. No storage mock.
#[tokio::test]
async fn current_write_failure_preserves_queries_disk_and_success_history() {
    let s = Scenario::new().await;
    let started = s.start("records-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-node-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("records-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("run.json");
    let calls = s.herdr.calls();
    for (name, extra, code) in [
        ("pause", json!({}), "pause-failed"),
        ("fail", json!({"nodeId":"A","reason":"Dependency unavailable"}), "fail-failed"),
        ("complete", json!({"nodeId":"A","reason":"User checked results/A.md; manual handoff"}), "complete-failed"),
    ] {
        let before = fs::read(&path).unwrap();
        let history = fs::read(root.join("history.jsonl")).unwrap();
        let messages = fs::read(root.join("messages.jsonl")).unwrap();
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let failed = action(&s, name, extra.clone(), 500).await;
        let queried = action(&s, "status", json!({}), 200).await;
        fs::set_permissions(&path, original).unwrap();
        assert_eq!(failed["code"], code);
        assert_eq!(queried["snapshot"], serde_json::from_slice::<Value>(&before).unwrap());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
        assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), messages);
        let accepted = action(&s, name, extra, 200).await;
        assert_eq!(accepted["changed"], true);
        println!("P43.01 action={name} failed={failed} query={queried} accepted={accepted}");
    }
    assert_eq!(s.herdr.calls(), calls);
}

// P44.01: make only the real history file readonly, so current-state replace
// succeeds and append fails. A rollback or persistent poison flag is detected
// by status, disk, absent new history, repeated completion and a legal pause.
#[tokio::test]
async fn history_failure_reports_committed_completion_and_allows_next_action() {
    let s = Scenario::new().await;
    let started = s.start("records-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-node-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("records-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("history.jsonl");
    let before = fs::read(&path).unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let completed = action(&s, "complete", json!({"nodeId":"A","reason":"User verified results/A.md; handoff accepted"}), 200).await;
    let queried = action(&s, "status", json!({}), 200).await;
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["completion"]["reason"], "User verified results/A.md; handoff accepted");
    assert_eq!(queried["snapshot"], completed["snapshot"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), completed["snapshot"]);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(completed["historyError"].is_string());
    assert_eq!(completed["code"], "history-append-failed");
    assert_eq!(completed["stateCommitted"], true);
    assert_eq!(completed["message"], "当前状态已提交，但执行历史追加失败；状态未回滚。请先查询现场，不要盲目重试。");
    let repeated = action(&s, "complete", json!({"nodeId":"A","reason":"Must not replace first completion"}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], completed["snapshot"]);
    assert_eq!(fs::read(&path).unwrap(), before, "duplicate completion cannot invent missing history");
    let paused = action(&s, "pause", json!({}), 200).await;
    assert_eq!(paused["changed"], true);
    assert_eq!(paused["snapshot"]["status"], "paused");
    let events = common::read_json_lines(&path);
    assert_eq!(events.iter().map(|event| event["action"].as_str().unwrap()).collect::<Vec<_>>(), vec!["dispatch", "pause"]);
    println!("P44.01 completion={completed} query={queried} repeated={repeated} next={paused} history={events:?}");
}

// Same P44 boundary for a real dispatch batch: history failure of A must not
// cancel legal B, and an independently failed launch must explain its committed
// failed state too. The fixture only supplies external launch outcomes.
#[tokio::test]
async fn batch_history_failure_explains_each_committed_target_without_stopping_batch() {
    let s = Scenario::new().await;
    let started = s.start("records-manager").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let path = root.join("history.jsonl");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let release = s.data.path().join("records-A-prompt-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-batch-A"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let request = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"records-manager","nodes":["A","B"]}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2).await);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"records-batch-B"},"native_cwd":s.data.path().join("node-zone")}));
    fs::write(release, b"release").unwrap();
    let response = request.response().await;
    assert_eq!(response.status, 200, "{}", response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    s.herdr.set_behavior(json!({"fail_agent_start":true}));
    let retried = action(&s, "fail", json!({"nodeId":"A","reason":"User requests independent retry after checking original"}), 200).await;
    let failed = action(&s, "retry", json!({"nodeId":"A","confirmed":true,"reason":"User checked old effects; independent attempt"}), 200).await;
    fs::set_permissions(&path, original).unwrap();
    for result in batch["results"].as_array().unwrap().iter().chain(std::iter::once(&failed["result"])) {
        assert_eq!(result["code"], "history-append-failed");
        assert_eq!(result["stateCommitted"], true);
        assert!(result["historyError"].is_string());
        assert!(result["message"].as_str().unwrap().contains("状态未回滚"));
    }
    assert_eq!(batch["results"][0]["status"], "running");
    assert_eq!(batch["results"][1]["status"], "running");
    assert_eq!(failed["result"]["status"], "failed");
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["stage"], "start");
    assert_eq!(fs::read(&path).unwrap(), b"");
    let queried = action(&s, "status", json!({}), 200).await;
    assert_eq!(queried["snapshot"], failed["snapshot"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), failed["snapshot"]);
    assert_eq!(action(&s, "pause", json!({}), 200).await["changed"], true);
    println!("P44.01 batch={batch} fail={retried} retry={failed} query={queried} hostCalls={:?}", s.herdr.calls());
}
