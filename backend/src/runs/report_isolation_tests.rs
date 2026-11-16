//! P19. Approved seam: manager HTTP / real Map and files. The controlled
//! script report-submission boundary supplies ordering, not storage, output
//! resolution or completion decisions. No public execution-selection API.
use super::*;
#[path = "../../tests/common/mod.rs"]
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, sync::Arc};

struct Fixture {
    data: TempDir,
    herdr: HerdrFixture,
    config: crate::Config,
    server: TestServer,
    state: AppState,
    manager: Value,
    targets: Vec<Value>,
}
impl Fixture {
    async fn new() -> Self {
        let data = TempDir::new("script report isolation");
        // The same target's fixture is built by the backend test entry point;
        // focused --lib feedback first builds --bin herdr-fixture explicitly.
        let binary = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().join("herdr-fixture.exe");
        let herdr = HerdrFixture::from_executable("script report isolation host", &binary);
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
        config.prompts_file = data.path().join("prompts.toml");
        config.zones.registry_file = data.path().join("zones.json");
        config.scheduler.map_directory = data.path().join("maps");
        config.scheduler.bot_directory = data.path().join("bots");
        config.scheduler.run_directory = data.path().join("runs");
        config.scheduler.run_instructions = std::path::absolute(fixtures.parent().unwrap().join("docs/workflow-runtime.md")).unwrap();
        config.herdr.executable = herdr.executable().to_owned();
        for dir in [&config.scheduler.map_directory, &config.scheduler.bot_directory] { fs::create_dir_all(dir).unwrap(); }
        herdr.set_workspaces(json!([{"workspace_id":"isolation-workspace","label":data.path().file_name().unwrap().to_str().unwrap()}]));
        let (app, state) = crate::app_with_state(&config, Arc::new(crate::SystemPowerOff), Arc::new(crate::sessions::SessionsState::with_policy(config.sessions.policy())));
        let server = TestServer::start_app(app).await;
        let mut fixture = Self { data, herdr, config, server, state, manager: Value::Null, targets: Vec::new() };
        fixture.post("/api/zones", json!({"id":"isolation","path":fixture.data.path()}), 200).await;
        let mut tasks = Vec::new();
        for name in ["manager", "worker", "first-target", "second-target"] {
            let response = fixture.post("/api/tasks", json!({"name":name,"taskType":if name == "manager" {"manager"} else {"normal"},"agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"Only the explicit assigned work"}), 200).await;
            tasks.push(response["tasks"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap().clone());
        }
        fixture.manager = tasks[0]["id"].clone();
        fixture.targets = tasks[2..].to_vec();
        fs::write(fixture.config.scheduler.bot_directory.join("creator.json"), json!({"id":"creator","name":"creator","when":"","plan_guide":"","node_guide":"","rust_script":"create_blank_bot"}).to_string()).unwrap();
        fs::write(fixture.config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"","plan_guide":"","node_guide":"","task_id":tasks[1]["id"]}).to_string()).unwrap();
        fs::write(fixture.config.scheduler.map_directory.join("isolation.json"), json!({"title":"Report isolation","description":"","nodes":{
            "create":{"title":"Create","bot_id":"creator","input":"No natural language arguments","input_rule":[{"id":"request","type":"file","zone_id":"isolation","path":"request.json"}],"output_rule":[{"id":"baseline"},{"id":"handoff"},{"id":"legacy"}],"zone":"isolation","blocked_by":[]},
            "decoy":{"title":"Other node","bot_id":"worker","input":"Other scope","output_rule":[{"id":"legacy"}],"zone":"isolation","blocked_by":[]},
            "missing":{"title":"No inherited material","bot_id":"worker","input":"Read legacy","input_rule":[{"id":"legacy","type":"node","node_id":"create","output_id":"legacy"}],"zone":"isolation","blocked_by":["create"]},
            "consumer":{"title":"Actual current JSON","bot_id":"worker","input":"Read the named JSON files","input_rule":[{"id":"baseline","type":"node","node_id":"create","output_id":"baseline"},{"id":"handoff","type":"node","node_id":"create","output_id":"handoff"}],"zone":"isolation","blocked_by":["create"]}
        }}).to_string()).unwrap();
        let read = get_over_tcp(fixture.server.addr(), "/api/scheduler/maps/isolation.json").await;
        assert_eq!(read.status, 200, "{}", read.body);
        fixture
    }
    async fn post(&self, path: &str, body: Value, status: u16) -> Value {
        let response = post_json_over_tcp(self.server.addr(), path, &body.to_string()).await;
        assert_eq!(response.status, status, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    fn native(&self, id: &str) { self.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":id},"native_cwd":self.data.path(),"consume_named_json":true})); }
    fn input(&self, index: usize) {
        fs::write(self.data.path().join("request.json"), json!({"schema_version":1,"kind":"request","target":{"type":"task","task_id":self.targets[index]["id"]},"bot":{"id":format!("created-{index}"),"name":format!("Created {index}")},"requirements":"Read only","constraints":[],"non_goals":[],"resource_limits":[],"authorization":{"create_blank":true,"write_materials":true,"save_guides":false},"confirmation":{"scope":"create_blank","evidence":"Explicit isolated creation authorization"}}).to_string()).unwrap();
    }
    async fn start(&self, manager: &str) -> Value {
        self.native(manager);
        let run = self.post("/api/scheduler/maps/isolation.json/runs", json!({"managerTaskId":self.manager,"zone":"isolation","userInput":"Isolated report ordering"}), 201).await;
        self.action(manager, "prepare_inputs", json!({"fileInputs":[],"confirmed":true,"reason":"Actual named request inspected"}), 200).await;
        run
    }
    async fn action(&self, manager: &str, action: &str, extra: Value, status: u16) -> Value {
        let mut body = extra;
        body["sessionId"] = json!(manager);
        self.post(&format!("/api/workflow/manager/{action}"), body, status).await
    }
    async fn detail(&self, run: &Value) -> Value {
        let response = get_over_tcp(self.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    async fn count(&self, run: &Value, count: usize) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let read = self.detail(run).await;
            if read["reports"].as_array().unwrap().len() == count { return read; }
            assert!(tokio::time::Instant::now() < deadline, "{read}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    async fn submit(&self, run: &Value, execution: &Value, can_complete: bool, outputs: Value) -> Value {
        let start = execution["lifecycle"]["startedAt"].as_u64().unwrap();
        let lifecycle = if can_complete { Lifecycle::Succeeded { started_at:start, ended_at:start+1 } } else { Lifecycle::Failed { started_at:start, ended_at:start+1, failure:json!({"stage":"read_back","code":"read-back-failed","message":"Controlled script submission outcome","effects":{"write":"succeeded","readBack":"failed","materials":[]}}) } };
        let submitted = reports::submit(self.state.clone(), run["id"].as_str().unwrap().into(), "create".into(), execution["id"].as_str().unwrap().into(), reports::ReportSubmission { origin:reports::ReportOrigin::Script(lifecycle), body:format!("Controlled result {can_complete}"), can_complete, outputs:serde_json::from_value(outputs).unwrap() }).await;
        match submitted { Ok(result) => serde_json::to_value(result.report).unwrap(), Err(_) => panic!("controlled report must save and notify") }
    }
    async fn host_exited(&self) {
        assert!(common::wait_until(Duration::from_secs(5), || {
            use std::os::windows::fs::OpenOptionsExt;
            fs::OpenOptions::new().read(true).write(true).share_mode(0).open(self.herdr.executable()).is_ok()
        }).await, "owned notification CLI must exit before cleanup");
    }
}

// Independent P19 oracle: only current execution's last committed report may
// qualify first complete. Global-last/first-true/old-output fallback violate
// literal HTTP rejections, pending IDs and the actual consumer file contents.
#[tokio::test]
async fn script_reports_isolate_late_old_true_latest_false_and_material_scope() {
    let f = Fixture::new().await;
    f.input(0);
    let run = f.start("isolation-manager").await;
    f.action("isolation-manager", "dispatch", json!({"nodes":["create"]}), 200).await;
    let first = f.count(&run, 1).await;
    let old = first["run"]["nodes"]["create"]["execution"].clone();
    assert_eq!(first["reports"][0]["canComplete"], true);
    f.host_exited().await;
    let legacy = f.data.path().join("old-only.json");
    fs::write(&legacy, b"{\"scope\":\"OLD-413\"}").unwrap();
    let old_outputs = json!([{"id":"legacy","path":legacy}]);
    f.action("isolation-manager", "fail", json!({"nodeId":"create","reason":"Old success rejected after effect inspection"}), 200).await;
    f.input(1);
    let gate = common::file_gate::Gate::new(&f.config.tasks.storage_file);
    let retried = f.action("isolation-manager", "retry", json!({"nodeId":"create","confirmed":true,"reason":"New isolated target authorized"}), 200).await;
    assert!(common::wait_until(Duration::from_secs(5), || gate.reached()).await);
    let current = retried["snapshot"]["nodes"]["create"]["execution"].clone();
    let late = f.submit(&run, &old, true, old_outputs.clone()).await;
    let status = f.action("isolation-manager", "status", json!({}), 200).await;
    assert!(status["pendingReports"].get("create").is_none());
    assert_eq!(f.action("isolation-manager", "complete", json!({"nodeId":"create","reason":"Old true is not current"}), 409).await["code"], "report-not-completable");
    let progress = f.submit(&run, &current, false, json!([])).await;
    let later = f.submit(&run, &old, true, old_outputs.clone()).await;
    assert_eq!(f.action("isolation-manager", "collect", json!({}), 200).await["pendingReports"]["create"], progress["id"]);
    assert_eq!(f.action("isolation-manager", "complete", json!({"nodeId":"create","reason":"Old true cannot override new false"}), 409).await["code"], "report-not-completable");
    let earlier_true = f.submit(&run, &current, true, json!([])).await;
    let latest_false = f.submit(&run, &current, false, json!([])).await;
    assert_eq!(f.action("isolation-manager", "collect", json!({}), 200).await["pendingReports"]["create"], latest_false["id"]);
    let before = f.detail(&run).await;
    assert_eq!(f.action("isolation-manager", "complete", json!({"nodeId":"create","reason":"Earlier current true was revoked"}), 409).await["code"], "report-not-completable");
    assert_eq!(f.detail(&run).await, before);
    drop(gate);
    let done = f.count(&run, 7).await;
    let ready = &done["reports"][6];
    assert_eq!(ready["canComplete"], true);
    assert_eq!(ready["executionId"], current["id"]);
    let last = f.submit(&run, &old, true, old_outputs).await;
    assert_eq!(f.action("isolation-manager", "collect", json!({}), 200).await["pendingReports"]["create"], ready["id"]);
    f.action("isolation-manager", "complete", json!({"nodeId":"create","reason":"Accept real current business result"}), 200).await;
    // Same ID from another node and another actual Run cannot fill legacy.
    for (manager, session) in [("isolation-manager", "decoy-same-run"), ("other-manager", "decoy-other-run")] {
        if manager == "other-manager" { f.start(manager).await; }
        f.native(session);
        f.action(manager, "dispatch", json!({"nodes":["decoy"]}), 200).await;
        f.post("/api/workflow/node/report", json!({"sessionId":session,"body":"Decoy scope only","canComplete":true,"outputs":[{"id":"legacy","path":legacy}]}), 200).await;
    }
    let calls = f.herdr.calls();
    let rejected = f.action("isolation-manager", "dispatch", json!({"nodes":["missing"]}), 200).await;
    assert_eq!(rejected["results"][0]["status"], "failed");
    assert_eq!(rejected["results"][0]["failure"]["stage"], "resolve");
    assert_eq!(f.herdr.calls(), calls);
    f.native("current-consumer");
    let consumed = f.action("isolation-manager", "dispatch", json!({"nodes":["consumer"]}), 200).await;
    assert_eq!(consumed["results"][0]["status"], "running");
    assert_eq!(consumed["snapshot"]["nodes"]["consumer"]["execution"]["inputs"], ready["outputs"]);
    let actual = common::read_json_lines(&f.herdr.path().join("consumed.jsonl"));
    assert_eq!(actual.last().unwrap()["files"]["baseline"]["bot"]["id"], "created-1");
    assert_eq!(actual.last().unwrap()["files"]["handoff"]["bot_id"], "created-1");
    let all = f.detail(&run).await;
    assert_eq!(&all["reports"].as_array().unwrap()[..8], &[first["reports"][0].clone(), late, progress, later, earlier_true, latest_false, ready.clone(), last]);
    assert_eq!(fs::read(legacy).unwrap(), b"{\"scope\":\"OLD-413\"}");
    f.host_exited().await;
    println!("P19 actual Run={} old={} current={} orderedReports={} consumer={}", run["id"], old["id"], current["id"], all["reports"], json!(actual));
}
