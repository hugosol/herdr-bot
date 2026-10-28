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
