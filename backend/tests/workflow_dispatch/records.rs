//! Ticket 14 approved seams: S2 real TCP HTTP and S4 public run records.
//!
//! P43 already met its criterion before changes; its first run was green.
//! P44 below requires explicit partial-effect feedback, not an unexplained OS
//! error or rollback claim. The independent expected outcome is the contract's
//! committed completion and absent completion history after append fails.
use super::*;

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
        "A":{"status":"failed","execution":{"id":"execution-current","taskId":"fixture-node-task","zone":"C:/fixture/node","chat":chat},"failure":{"stage":"execution","message":"CURRENT dependency unavailable"},"completion":null,"previousExecutions":[{"execution":{"id":"execution-old","taskId":"fixture-node-task","zone":"C:/fixture/node","chat":old_chat},"failure":{"stage":"execution","message":"OLD artifact invalid"}}]},
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
    assert_eq!(detail["run"], current);
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
