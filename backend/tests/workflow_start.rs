//! Approved seams S2 HTTP, S3 controlled Herdr, S4 actual records.
//! Expected sequence: all three records exist before external start, native
//! identity is persisted before prompt. A prompt-first implementation violates
//! P13 even if its final response happens to contain a session ID.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

async fn fixture() -> (
    TempDir,
    HerdrFixture,
    herdr_bot::Config,
    TestServer,
    String,
    Vec<String>,
) {
    let data = TempDir::new("workflow start records");
    let herdr = HerdrFixture::new("workflow manager external");
    let fixtures = common::repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations/success.bat"),
        &fixtures.join("operations"),
    );
    config.tasks.storage_file = data.path().join("tasks.json");
    let mut catalog: toml::Value =
        toml::from_str(&fs::read_to_string(&config.tasks.metadata_file).unwrap()).unwrap();
    for entry in catalog["extensions"].as_array_mut().unwrap() {
        let path = data
            .path()
            .join(format!("{}.ts", entry["id"].as_str().unwrap()));
        fs::write(&path, "export default function () {}\n").unwrap();
        entry["entrypoint"] = toml::Value::String(path.to_string_lossy().into_owned());
    }
    config.tasks.metadata_file = data.path().join("options.toml");
    fs::write(
        &config.tasks.metadata_file,
        toml::to_string(&catalog).unwrap(),
    )
    .unwrap();
    config.zones.registry_file = data.path().join("zones.json");
    config.scheduler.map_directory = data.path().join("maps");
    config.scheduler.bot_directory = data.path().join("bots");
    config.scheduler.run_directory = data.path().join("nondefault-runs");
    config.scheduler.run_instructions = data.path().join("runtime-instructions.md");
    fs::copy(
        fixtures.parent().unwrap().join("docs/workflow-runtime.md"),
        &config.scheduler.run_instructions,
    )
    .unwrap();
    config.herdr.executable = herdr.executable().to_path_buf();
    herdr.set_workspaces(json!([{"workspace_id":"w-manager", "label":data.path().file_name().unwrap().to_str().unwrap()}]));
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp", "agent":"omp", "kind":"id", "value":"manager-native-04"}, "native_cwd":data.path()}));
    fs::create_dir_all(&config.scheduler.map_directory).unwrap();
    fs::create_dir_all(&config.scheduler.bot_directory).unwrap();
    let server = TestServer::start(&config).await;
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({"path":data.path()}).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "{}", response.body);
    let zone = serde_json::from_str::<Value>(&response.body).unwrap()["zones"][0]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut ids = Vec::new();
    for role in ["manager", "node"] {
        let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":role,"agent":"omp","chatName":"{chat.id}","model":"default","thinking":"default","skill":null,"prompt":"FIXED-MANAGER-ROLE","extensions":[]}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        ids.push(
            body["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == role)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    fs::write(config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"","plan_guide":"","node_guide":"","task_id":ids[1]}).to_string()).unwrap();
    fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"MAP-BODY-NOT-INJECTED","nodes":{"A":{"title":"A","bot_id":"worker","input":"NODE-INPUT-NOT-INJECTED","zone":zone,"blocked_by":[]}}}).to_string()).unwrap();
    (data, herdr, config, server, zone, ids)
}

// P13/P15/P17: external start/prompt gates expose the actual temporal facts.
// Independent trigger literal comes from the catalog's declared mapping, not
// agent_launch_args. Status is called over HTTP with native transport identity.
#[tokio::test]
async fn manager_trigger_and_status_follow_committed_identity() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    // P16 uses distinct nonempty document markers, unlike P2's empty documents.
    fs::write(config.scheduler.bot_directory.join("worker.json"), json!({
        "id":"worker", "name":"worker", "when":"BOT-WHEN-NOT-INJECTED",
        "plan_guide":"BOT-PLAN-NOT-INJECTED", "node_guide":"BOT-NODE-NOT-INJECTED", "task_id":ids[1]
    }).to_string()).unwrap();
    let task = json!({"name":"manager","taskType":"manager","agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":"FIXED-ROLE-MARKER","extensions":[]});
    let update = common::request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/tasks/{}", ids[0]),
        Some(&task.to_string()),
    )
    .await;
    assert_eq!(update.status, 200, "{}", update.body);
    let start_release = data.path().join("start-release");
    let prompt_release = data.path().join("prompt-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"trigger-native-04"},"native_cwd":data.path(),"agent_start_gate":{"wait_for_file":start_release,"timeout_ms":10000},"prompt_gate":{"wait_for_file":prompt_release,"timeout_ms":10000}}));
    let addr = server.addr();
    let manager = ids[0].clone();
    let request = tokio::spawn(async move {
        post_json_over_tcp(
            addr,
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":manager,"zone":zone,"userInput":"LAST-USER-MARKER"})
                .to_string(),
        )
        .await
    });
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_start")
    })
    .await;
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert!(original["manager"].is_null());
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
    assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
    let start = herdr
        .calls()
        .into_iter()
        .find(|c| c["kind"] == "agent_start")
        .unwrap();
    assert_eq!(
        &start["args"].as_array().unwrap()[..4],
        &[
            json!("--model"),
            json!("openai-codex/gpt-6.1-sol"),
            json!("--thinking"),
            json!("high")
        ]
    );
    fs::write(start_release, b"release").unwrap();
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_prompt")
    })
    .await;
    let committed: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(committed["manager"]["sessionId"], "trigger-native-04");
    let prompt = herdr
        .calls()
        .into_iter()
        .find(|c| c["kind"] == "agent_prompt")
        .unwrap();
    let expected = format!(
        "/skill:teach FIXED-ROLE-MARKER\n\nRun ID: {}\nMap: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\nLAST-USER-MARKER",
        original["id"].as_str().unwrap(),
        config.scheduler.map_directory.join("work.json").display(),
        config.scheduler.run_instructions.display(),
        root.join("run.json").display(),
        root.join("history.jsonl").display(),
        root.join("messages.jsonl").display()
    );
    assert_eq!(prompt["text"], expected);
    let status = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"trigger-native-04"}).to_string(),
    )
    .await;
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(
        serde_json::from_str::<Value>(&status.body).unwrap()["snapshot"],
        committed
    );
    let denied = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"unassociated"}).to_string(),
    )
    .await;
    assert_eq!(denied.status, 403);
    let invented = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"trigger-native-04","runId":original["id"]}).to_string(),
    )
    .await;
    assert_eq!(invented.status, 400);
    fs::write(prompt_release, b"release").unwrap();
    assert_eq!(request.await.unwrap().status, 201);
    let saved = common::get_over_tcp(server.addr(), &format!("/api/tasks/{}", ids[0])).await;
    assert_eq!(
        serde_json::from_str::<Value>(&saved.body).unwrap()["task"]["prompt"],
        "FIXED-ROLE-MARKER"
    );
}

// P13: an initialization error must precede every external call; an identity
// error can leave a real Chat, but cannot submit an instruction. Neither error
// is recorded as a persisted business failure or retried autonomously.
#[tokio::test]
async fn initialization_and_identity_failures_stop_before_initial_instructions() {
    for stage in ["initialize", "identity", "submit"] {
        let (data, herdr, config, server, zone, ids) = fixture().await;
        if stage == "initialize" {
            fs::write(&config.scheduler.run_directory, b"not a directory").unwrap();
        }
        if stage == "identity" {
            herdr.set_behavior(json!({"fail_agent_get":true}));
        }
        if stage == "submit" {
            herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"submit-failure-chat"},"native_cwd":data.path(),"fail_prompt":true}));
        }
        let response = post_json_over_tcp(
            server.addr(),
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
        )
        .await;
        assert_eq!(response.status, 500, "{}", response.body);
        let detail: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(detail["startupError"]["stage"], stage);
        assert_eq!(detail["startupError"]["effects"]["inputSubmitted"], false);
        assert_eq!(detail["startupError"]["effects"]["inputAttempted"], stage == "submit");
        let calls = herdr.calls();
        if stage == "initialize" {
            assert!(calls.is_empty());
        } else {
            assert_eq!(
                calls.iter().filter(|c| c["kind"] == "agent_start").count(),
                1
            );
            assert_eq!(
                calls.iter().filter(|c| c["kind"] == "agent_prompt").count(),
                usize::from(stage == "submit")
            );
            let root = config
                .scheduler
                .run_directory
                .join(detail["id"].as_str().unwrap());
            assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
            assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
        }
        let refreshed = get_over_tcp(
            server.addr(),
            &format!("/api/workflow/runs/{}", detail["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(refreshed.status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&refreshed.body).unwrap()["startupError"],
            detail["startupError"]
        );
        assert_eq!(herdr.calls(), calls);
    }
}

// P13/P45: the external identity gate distinguishes persisted linkage from
// a plausible prompt-first bug; association failure must preserve external refs.
#[tokio::test]
async fn association_failure_never_submits_and_exposes_real_started_effects() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let release = data.path().join("identity-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"association-failure-chat"},"native_cwd":data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = server.addr();
    let request = tokio::spawn(async move {
        post_json_over_tcp(
            addr,
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
        )
        .await
    });
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_get")
    })
    .await;
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(root.join("history.jsonl").is_file());
    assert!(root.join("messages.jsonl").is_file());
    let original = fs::read(root.join("run.json")).unwrap();
    let id = serde_json::from_slice::<Value>(&original).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(herdr.calls().iter().all(|c| c["kind"] != "agent_prompt"));
    // The S4 file boundary deterministically rejects whole-file replacement.
    fs::remove_file(root.join("run.json")).unwrap();
    fs::create_dir(root.join("run.json")).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 500);
    let failure: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(failure["id"], id, "{}", response.body);
    assert_eq!(failure["startupError"]["stage"], "association");
    assert_eq!(
        failure["startupError"]["effects"]["sessionId"],
        "association-failure-chat"
    );
    assert_eq!(failure["startupError"]["effects"]["agentStarted"], true);
    assert_eq!(failure["startupError"]["effects"]["inputSubmitted"], false);
    assert!(herdr.calls().iter().all(|c| c["kind"] != "agent_prompt"));
    let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(
        serde_json::from_str::<Value>(&detail.body).unwrap()["startupError"],
        failure["startupError"]
    );
    fs::remove_dir(root.join("run.json")).unwrap();
    fs::write(root.join("run.json"), original).unwrap();
}

#[tokio::test]
async fn valid_empty_bot_documents_start_manager_and_preserve_literal_requirements() {
    let (_data, herdr, config, server, zone, ids) = fixture().await;
    for requirements in ["", "# 本次独立标记\n\n- 保留  空格\n末尾\n"] {
        let response = post_json_over_tcp(
            server.addr(),
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":requirements}).to_string(),
        )
        .await;
        assert_eq!(response.status, 201, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        let id = body["run"]["id"].as_str().unwrap();
        assert_eq!(body["run"]["userInput"], requirements);
        assert_eq!(body["run"]["manager"]["sessionId"], "manager-native-04");
        assert_eq!(body["run"]["manager"]["workspaceId"], "w-manager");
        assert_eq!(body["run"]["managerTaskId"], ids[0]);
        let root = config.scheduler.run_directory.join(id);
        let persisted: Value =
            serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
        assert_eq!(persisted["userInput"], requirements);
        assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
        assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
        assert_eq!(persisted["nodes"]["A"]["status"], "pending");
        assert!(persisted["nodes"]["A"].get("ready").is_none());
        let calls = herdr.calls();
        let prompt = calls
            .iter()
            .rev()
            .find(|c| c["kind"] == "agent_prompt")
            .unwrap()["text"]
            .as_str()
            .unwrap();
        assert!(prompt.starts_with("FIXED-MANAGER-ROLE\n\nRun ID: "));
        assert!(prompt.contains("work.json"));
        assert!(!prompt.contains("MAP-BODY-NOT-INJECTED"));
        assert!(!prompt.contains("NODE-INPUT-NOT-INJECTED"));
        if requirements.is_empty() {
            assert!(!prompt.ends_with("\n\n"));
        } else {
            assert!(prompt.ends_with("\n\n# 本次独立标记\n\n- 保留  空格\n末尾\n"));
        }
        let count = calls.iter().filter(|c| c["kind"] == "agent_start").count();
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(detail.status, 200, "{}", detail.body);
        assert_eq!(
            serde_json::from_str::<Value>(&detail.body).unwrap()["run"]["userInput"],
            requirements
        );
        assert_eq!(
            herdr
                .calls()
                .iter()
                .filter(|c| c["kind"] == "agent_start")
                .count(),
            count
        );
    }
}
