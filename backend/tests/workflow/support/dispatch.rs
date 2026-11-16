use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, time::Duration};

// Callers state their scenario's declaration explicitly; dispatch and complete
// never manufacture a report on behalf of a node.
async fn submit_report(s: &Scenario, identity: &str, body: &str, can_complete: bool) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":identity,"body":body,"canComplete":can_complete}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str::<Value>(&response.body).unwrap()["report"].clone()
}

struct Scenario {
    data: TempDir,
    herdr: HerdrFixture,
    config: herdr_bot::Config,
    server: TestServer,
    app: axum::Router,
    zone: String,
    manager_task: String,
    prepared_fixtures: std::sync::Mutex<std::collections::BTreeSet<String>>,
}
impl Scenario {
    async fn new() -> Self {
        Self::with_collect_wait(5400).await
    }
    async fn with_collect_wait(seconds: u64) -> Self {
        let data = TempDir::new("dispatch records");
        let herdr = HerdrFixture::new("dispatch external");
        let fixtures = common::repo_fixtures_dir();
        let mut config = common::config(&fixtures.join("operations/success.bat"), &fixtures.join("operations"));
        config.tasks.storage_file = data.path().join("tasks.json");
        config.prompts_file = data.path().join("prompts.local.toml");
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
        config.scheduler.collect_wait_seconds = herdr_bot::config::CollectWaitSeconds::new(seconds).unwrap();
        config.scheduler.run_instructions = std::path::absolute(fixtures.parent().unwrap().join("docs/workflow-runtime.md")).unwrap();
        config.herdr.executable = herdr.executable().to_owned();
        herdr.set_workspaces(json!([{"workspace_id":"w-manager","label":data.path().file_name().unwrap().to_str().unwrap()}]));
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-05"},"native_cwd":data.path()}));
        fs::create_dir_all(&config.scheduler.map_directory).unwrap();
        fs::create_dir_all(&config.scheduler.bot_directory).unwrap();
        let app = herdr_bot::app(&config);
        let server = TestServer::start_app(app.clone()).await;
        let node_zone = data.path().join("node-zone");
        fs::create_dir(&node_zone).unwrap();
        for (id, path) in [("manager", data.path()), ("node", node_zone.as_path())] {
            assert_eq!(post_json_over_tcp(server.addr(), "/api/zones", &json!({"id":id,"path":path}).to_string()).await.status, 200);
        }
        let zone = "manager".to_owned();
        let mut ids = Vec::new();
        for role in ["manager", "node"] {
            let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":if role == "node" {"normal"} else {"manager"},"agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":if role == "node" {"NODE-FIXED"} else {"MANAGER-FIXED"}}).to_string()).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let body: Value = serde_json::from_str(&response.body).unwrap();
            ids.push(body["tasks"].as_array().unwrap().iter().find(|t| t["name"] == role).unwrap()["id"].as_str().unwrap().to_owned());
        }
        fs::write(config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"BOT-WHEN-SECRET","plan_guide":"BOT-PLAN-SECRET","node_guide":"BOT-NODE-SECRET","task_id":ids[1]}).to_string()).unwrap();
        fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"MAP-SECRET","nodes":{
            "A":{"title":"Worker A","bot_id":"worker","input":"NODE-INPUT\n第二行","zone":"node","blocked_by":[],"output_rule":[{"id":"A"},{"id":"B"},{"id":"otherVolume"},{"id":"result"},{"id":"brief"},{"id":"shared"}]},
            "B":{"title":"Worker B","bot_id":"worker","input":"B-INPUT","zone":"node","blocked_by":[],"output_rule":[{"id":"A"},{"id":"B"},{"id":"review"},{"id":"summary"}]},
            "C":{"title":"Worker C","bot_id":"worker","input":"C-INPUT","zone":"node","blocked_by":["A","B"],"output_rule":[{"id":"summary"}]}
        }}).to_string()).unwrap();
        Self { data, herdr, config, server, app, zone, manager_task:ids.remove(0), prepared_fixtures: std::sync::Mutex::new(std::collections::BTreeSet::new()) }
    }
    async fn start(&self, identity: &str) -> Value {
        self.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":self.data.path()}));
        let response = post_json_over_tcp(self.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":self.manager_task,"zone":self.zone,"userInput":"MANAGER-USER-SECRET"}).to_string()).await;
        assert_eq!(response.status, 201, "{}", response.body);
        let mut started: Value = serde_json::from_str(&response.body).unwrap();
        started["run"] = self.prepare_fixture(identity).await.expect("new fixture run is not prepared yet");
        started
    }
    // Unrelated dispatch scenarios attest the fixture's actual bindings through
    // the same public API as managers, including independently prepared children.
    async fn prepare_fixture(&self, identity: &str) -> Option<Value> {
        if self.prepared_fixtures.lock().unwrap().contains(identity) { return None; }
        let response = post_json_over_tcp(self.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":identity}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let status: Value = serde_json::from_str(&response.body).unwrap();
        if status["snapshot"]["inputsReady"] == true {
            self.prepared_fixtures.lock().unwrap().insert(identity.to_owned());
            return Some(status["snapshot"].clone());
        }
        let response = post_json_over_tcp(self.server.addr(), "/api/workflow/manager/prepare_inputs", &json!({"sessionId":identity,"fileInputs":status["snapshot"]["fileInputs"],"confirmed":true,"reason":"Fixture bindings reviewed against this scenario's requirements"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let snapshot = serde_json::from_str::<Value>(&response.body).unwrap()["snapshot"].clone();
        self.prepared_fixtures.lock().unwrap().insert(identity.to_owned());
        Some(snapshot)
    }
    async fn dispatch(&self, identity: &str, nodes: &[&str]) -> Value {
        let _ = self.prepare_fixture(identity).await;
        let response = post_json_over_tcp(self.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":identity,"nodes":nodes}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
}
