//! Approved S2/S3/S4 seams: real HTTP, external controlled host and disk records.
//! P8/P10/P13/P14/P15/P16: a prompt-first launch or a copied manager input
//! would violate independent literal expectations at the host boundary.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, time::Duration};

struct Scenario {
    data: TempDir,
    herdr: HerdrFixture,
    config: herdr_bot::Config,
    server: TestServer,
    zone: String,
    manager_task: String,
}
impl Scenario {
    async fn new() -> Self {
        let data = TempDir::new("dispatch records");
        let herdr = HerdrFixture::new("dispatch external");
        let fixtures = common::repo_fixtures_dir();
        let mut config = common::config(&fixtures.join("operations/success.bat"), &fixtures.join("operations"));
        config.tasks.storage_file = data.path().join("tasks.json");
        let mut catalog: toml::Value = toml::from_str(&fs::read_to_string(&config.tasks.metadata_file).unwrap()).unwrap();
        for entry in catalog["extensions"].as_array_mut().unwrap() {
            let path = data.path().join(format!("{}.ts", entry["id"].as_str().unwrap()));
            fs::write(&path, "export default function () {}\n").unwrap();
            entry["entrypoint"] = toml::Value::String(path.to_string_lossy().into_owned());
        }
        config.tasks.metadata_file = data.path().join("options.toml");
        fs::write(&config.tasks.metadata_file, toml::to_string(&catalog).unwrap()).unwrap();
        config.zones.registry_file = data.path().join("zones.json");
        config.scheduler.map_directory = data.path().join("maps");
        config.scheduler.bot_directory = data.path().join("bots");
        config.scheduler.run_directory = data.path().join("runs");
        config.scheduler.run_instructions = std::path::absolute(fixtures.parent().unwrap().join("docs/workflow-runtime.md")).unwrap();
        config.herdr.executable = herdr.executable().to_owned();
        herdr.set_workspaces(json!([{"workspace_id":"w-manager","label":data.path().file_name().unwrap().to_str().unwrap()}]));
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-05"},"native_cwd":data.path()}));
        fs::create_dir_all(&config.scheduler.map_directory).unwrap();
        fs::create_dir_all(&config.scheduler.bot_directory).unwrap();
        let server = TestServer::start(&config).await;
        let node_zone = data.path().join("node-zone");
        fs::create_dir(&node_zone).unwrap();
        for path in [data.path(), node_zone.as_path()] {
            assert_eq!(post_json_over_tcp(server.addr(), "/api/zones", &json!({"path":path}).to_string()).await.status, 200);
        }
        let zone = data.path().to_string_lossy().into_owned();
        let mut ids = Vec::new();
        for role in ["manager", "node"] {
            let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":role,"agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":if role == "node" {"NODE-FIXED"} else {"MANAGER-FIXED"},"extensions":[]}).to_string()).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let body: Value = serde_json::from_str(&response.body).unwrap();
            ids.push(body["tasks"].as_array().unwrap().iter().find(|t| t["name"] == role).unwrap()["id"].as_str().unwrap().to_owned());
        }
        fs::write(config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"BOT-WHEN-SECRET","plan_guide":"BOT-PLAN-SECRET","node_guide":"BOT-NODE-SECRET","task_id":ids[1]}).to_string()).unwrap();
        fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"MAP-SECRET","nodes":{
            "A":{"title":"Worker A","bot_id":"worker","input":"NODE-INPUT\n第二行","zone":node_zone,"blocked_by":[]},
            "B":{"title":"Worker B","bot_id":"worker","input":"B-INPUT","zone":node_zone,"blocked_by":[]},
            "C":{"title":"Worker C","bot_id":"worker","input":"C-INPUT","zone":node_zone,"blocked_by":["A","B"]}
        }}).to_string()).unwrap();
        Self { data, herdr, config, server, zone, manager_task:ids.remove(0) }
    }
    async fn start(&self, identity: &str) -> Value {
        self.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":self.data.path()}));
        let response = post_json_over_tcp(self.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":self.manager_task,"zone":self.zone,"userInput":"MANAGER-USER-SECRET"}).to_string()).await;
        assert_eq!(response.status, 201, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    async fn dispatch(&self, identity: &str, nodes: &[&str]) -> Value {
        let response = post_json_over_tcp(self.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":identity,"nodes":nodes}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
}

// P24.01/P27.01/P28.01: independent literals require two append-only reports
// from the real current node identity, never a state transition or read receipt.
// A report attributed from caller-supplied metadata or cleared by reads violates
// this scenario at the HTTP, persisted-message and external-input seams.
#[tokio::test]
async fn node_reports_are_system_attributed_append_only_and_do_not_complete_work() {
    let s = Scenario::new().await;
    let started = s.start("manager-report").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-report"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-report", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let mut reports = Vec::new();
    for body in ["Result: C:\\results\\one.txt; draft only", "Result: C:\\results\\two.txt; reviewed"] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-report","body":body}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], "submitted");
        assert_eq!(value["report"]["body"], body);
        reports.push(value["report"].clone());
    }
    assert_ne!(reports[0]["id"], reports[1]["id"]);
    for report in &reports {
        assert!(!report["id"].as_str().unwrap().is_empty());
        assert!(report["at"].as_u64().unwrap() >= started["run"]["createdAt"].as_u64().unwrap());
        assert_eq!(report["type"], "report");
        assert_eq!(report["runId"], id);
        assert_eq!(report["nodeId"], "A");
        let run: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(report["executionId"], run["nodes"]["A"]["execution"]["id"]);
        assert_eq!(report["source"]["sessionId"], "node-report");
        assert_eq!(report["target"]["sessionId"], "manager-report");
    }
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    for _ in 0..2 {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["reports"], json!(reports));
        assert_eq!(value["pendingReports"], json!({"A":reports[1]["id"]}));
    }
    let calls = s.herdr.calls();
    let inputs: Vec<_> = calls.iter().filter(|c| c["kind"] == "agent_prompt").collect();
    assert_eq!(inputs.len(), 4);
    for (input, report) in inputs[2..].iter().zip(&reports) {
        assert_eq!(input["target"], started["run"]["manager"]["agentName"]);
        assert!(input["text"].as_str().unwrap().contains(report["body"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains(report["id"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains("node: A"));
    }
}

// P25.01: a real Windows read-only messages file must reject persistence.
// The external adapter remains real so any accidental prompt is observable.
#[tokio::test]
async fn report_save_failure_has_no_message_pending_item_or_host_delivery() {
    let s = Scenario::new().await;
    let started = s.start("manager-save-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-save-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-save-error", &["A"]).await;
    let path = s.config.scheduler.run_directory.join(id).join("messages.jsonl");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-save-error","body":"must not persist or notify"}).to_string()).await;
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["code"], "report-save-failed");
    assert_eq!(value["saved"], false);
    assert_eq!(value["notification"]["status"], "not-attempted");
    assert!(fs::read(&path).unwrap().is_empty());
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reports"], json!([]));
    assert_eq!(value["pendingReports"], json!({}));
    assert_eq!(s.herdr.calls(), calls);
}

// P26.01/P28.01/P68.02: the external prompt gate holds delivery after
// persistence. Saved reports must already be visible to the manager; a failed
// single attempt must not pause or resend during the 300 ms post-result window.
#[tokio::test]
async fn delayed_failed_notification_preserves_saved_report_and_pending_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-notify-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-notify-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-notify-error", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let release = s.data.path().join("notify-release");
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/node/report", &json!({"sessionId":"node-notify-error","body":"SENSITIVE-RESULT: saved before notification"}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 3).await;
    assert!(!request.is_finished());
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(reports.len(), 1);
    let report_id = reports[0]["id"].clone();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-notify-error"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let status: Value = serde_json::from_str(&response.body).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(status["pendingReports"], json!({"A":report_id}));
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["saved"], true);
    assert_eq!(value["notification"]["status"], "failed");
    assert!(!value["notification"]["reason"].as_str().unwrap().is_empty());
    assert_eq!(value["report"]["id"], report_id);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    let calls = s.herdr.calls();
    assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
    println!("P26.01/P68.02 external calls after 300ms: {}", json!(calls));
}

// P24.01: neither a manager/unknown session nor forged ownership fields may
// produce a report or manager input. The model-facing body is the only business
// argument; sessionId belongs to the trusted public Session transport adapter.
#[tokio::test]
async fn report_rejects_non_node_identity_and_caller_supplied_ownership() {
    let s = Scenario::new().await;
    let started = s.start("manager-reject").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-reject"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-reject", &["A"]).await;
    let calls = s.herdr.calls();
    for (body, expected) in [
        (json!({"sessionId":"manager-reject","body":"manager cannot report"}), 403),
        (json!({"sessionId":"unassociated","body":"no binding"}), 403),
        (json!({"sessionId":"node-reject","body":"forged","runId":"another-run"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","nodeId":"B"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","target":"another-chat"}), 400),
        (json!({"sessionId":"node-reject"}), 400),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
        assert_eq!(response.status, expected, "{}", response.body);
    }
    assert!(fs::read(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("messages.jsonl")).unwrap().is_empty());
    assert_eq!(s.herdr.calls(), calls);
}

// P61.01/P68.02: isolate the production diagnostic sink in a child process.
// Closing its stderr pipe is a real OS write failure, not a mocked logger.
// Independent literals prohibit report text in diagnostics; 300 ms of actual
// external calls after the result distinguishes one attempt from retry loops.
#[tokio::test]
async fn report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries() {
    const MODE: &str = "HERDR_REPORT_DIAGNOSTIC_TEST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        let s = Scenario::new().await;
        let started = s.start("manager-diagnostic").await;
        let id = started["id"].as_str().unwrap();
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-diagnostic"},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-diagnostic", &["A"]).await;
        let before = fs::read(s.config.scheduler.run_directory.join(id).join("run.json")).unwrap();
        let failed = mode != "success";
        s.herdr.set_behavior(json!({"fail_prompt":failed}));
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-diagnostic","body":"SENSITIVE-DIAGNOSTIC-BODY"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], if failed {"failed"} else {"submitted"});
        assert_eq!(value["diagnostic"]["status"], if mode == "broken" {"failed"} else {"written"});
        tokio::time::sleep(Duration::from_millis(300)).await;
        let root = s.config.scheduler.run_directory.join(id);
        assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), vec![value["report"].clone()]);
        assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
        println!("REPORT-EVIDENCE {}", json!({"runId":id,"reportId":value["report"]["id"],"targetChat":started["run"]["manager"],"result":value["notification"]["status"],"externalCalls":calls,"observationMs":300}));
        return;
    }
    for mode in ["success", "failure", "broken"] {
        let cwd = TempDir::new("report diagnostic child cwd");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries", "--nocapture"])
            .env(MODE, mode).current_dir(cwd.path())
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
            .spawn().unwrap();
        if mode == "broken" { drop(child.stderr.take()); }
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{mode}\n{stdout}\n{stderr}");
        let evidence: Value = serde_json::from_str(stdout.lines().find_map(|line| line.strip_prefix("REPORT-EVIDENCE ")).unwrap()).unwrap();
        if mode != "broken" {
            assert!(!stderr.contains("SENSITIVE-DIAGNOSTIC-BODY"));
            let diagnostic: Value = serde_json::from_str(stderr.lines().find(|line| line.contains("workflow-report-delivery")).unwrap()).unwrap();
            assert_eq!(diagnostic["runId"], evidence["runId"]);
            assert_eq!(diagnostic["reportId"], evidence["reportId"]);
            assert_eq!(diagnostic["targetChat"], evidence["targetChat"]);
            assert_eq!(diagnostic["result"], evidence["result"]);
            assert!(!diagnostic["reason"].as_str().unwrap().is_empty());
        }
        println!("P61.01/P68.02 mode={mode} stderr={stderr} evidence={evidence}");
    }
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
    assert_eq!(prompt["text"], format!("/skill:teach NODE-FIXED\n\nNode ID: A\nNode: Worker A\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\nNODE-INPUT\n第二行", s.config.scheduler.run_instructions.display(),root.join("run.json").display(),root.join("history.jsonl").display(),root.join("messages.jsonl").display()));
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
    assert_eq!(history.len(),2);
    assert_eq!(history[0]["nodeId"],"A");
    assert_eq!(history[0]["status"],"failed");
    assert_eq!(history[1]["nodeId"],"B");
    assert_eq!(history[1]["status"],"running");
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
    assert_eq!(fs::read_to_string(root.join("history.jsonl")).unwrap().lines().count(),2);
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
