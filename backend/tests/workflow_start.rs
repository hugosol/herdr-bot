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
    fixture_for_agent("omp").await
}

async fn fixture_for_agent(agent: &str) -> (TempDir, HerdrFixture, herdr_bot::Config, TestServer, String, Vec<String>) {
    let data = TempDir::new("workflow start records");
    let herdr = HerdrFixture::new("workflow manager external");
    let fixtures = common::repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations/success.bat"),
        &fixtures.join("operations"),
    );
    config.tasks.storage_file = data.path().join("tasks.json");
    config.prompts_file = data.path().join("prompts.local.toml");
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
        &json!({"id":"manager","path":data.path()}).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "{}", response.body);
    let zone = serde_json::from_str::<Value>(&response.body).unwrap()["zones"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut ids = Vec::new();
    for role in ["manager", "node"] {
        let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":if role == "node" {"normal"} else {"manager"},"agent":agent,"chatName":"{chat.id}","model":"default","thinking":"default","skill":null,"prompt":"FIXED-MANAGER-ROLE"}).to_string()).await;
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

// S2, nesting P08: descriptions roundtrip literally and empty descriptions stay
// omitted. Invalid descriptions and duplicate IDs reject before publication;
// a second Map distinguishes local uniqueness from global uniqueness.
#[tokio::test]
async fn named_input_declarations_are_local_and_duplicate_save_preserves_map() {
    let (_data, herdr, config, server, _zone, _ids) = fixture().await;
    let route = "/api/scheduler/maps/work.json/inputs";
    let draft = get_over_tcp(server.addr(), route).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let original: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body).unwrap();
    let declarations = json!([{"id":"brief","required":true,"description":"Invoice **source** <plain text>\nKeep the original date."},{"id":"appendix","required":false}]);
    let mut submitted = declarations.clone();
    submitted[1]["description"] = json!("");
    let saved = post_json_over_tcp(server.addr(), route, &json!({"input_files":submitted,"input_requirements":"Review the invoice date and obtain explicit scope approval.","revision":draft["revision"]}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    let map: Value = serde_json::from_str(&read.body).unwrap();
    let mut expected = original;
    expected["input_files"] = declarations.clone();
    expected["input_requirements"] = json!("Review the invoice date and obtain explicit scope approval.");
    assert_eq!(map, expected);
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), expected);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(config.scheduler.map_directory.join("work.json")).unwrap()).unwrap(), expected);
    let edited = get_over_tcp(server.addr(), route).await;
    assert_eq!(edited.status, 200, "{}", edited.body);
    let draft: Value = serde_json::from_str(&edited.body).unwrap();
    assert_eq!(draft["input_files"], declarations);
    assert_eq!(draft["input_requirements"], expected["input_requirements"]);
    let persisted = fs::read(config.scheduler.map_directory.join("work.json")).unwrap();
    for description in [Value::Null, json!(42), json!(true), json!([]), json!({})] {
        let mut invalid = declarations.clone();
        invalid[0]["description"] = description.clone();
        let rejected = post_json_over_tcp(server.addr(), route, &json!({"input_files":invalid,"input_requirements":"Must not replace general requirements.","revision":draft["revision"]}).to_string()).await;
        assert_eq!(rejected.status, 422, "description {description}: {}", rejected.body);
        assert_eq!(fs::read(config.scheduler.map_directory.join("work.json")).unwrap(), persisted);
        assert_eq!(get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body, read.body);
    }
    let rejected = post_json_over_tcp(server.addr(), route, &json!({"input_files":[{"id":"brief","required":true},{"id":"brief","required":false}],"input_requirements":"Review the invoice date and obtain explicit scope approval.","revision":draft["revision"]}).to_string()).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(rejected.body.contains("brief"));
    assert_eq!(get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body, read.body);
    fs::write(config.scheduler.map_directory.join("other.json"), serde_json::to_vec(&map).unwrap()).unwrap();
    let other = get_over_tcp(server.addr(), "/api/scheduler/maps/other.json").await;
    assert_eq!(other.status, 200, "{}", other.body);
    assert_eq!(serde_json::from_str::<Value>(&other.body).unwrap()["input_files"], declarations);
    assert!(herdr.calls().is_empty());
}

// S2, nesting P08/P09/P10: independent business markers are deliberately not
// file discovery hints. Dropping bindings or sharing a same-ID binding across
// Runs disagrees with the public, restarted readback of two real input files.
#[tokio::test]
async fn root_runs_preserve_named_bindings_separately_from_business_text() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let map_path = config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["input_files"] = json!([{"id":"brief","required":true},{"id":"appendix","required":false}]);
    fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
    fs::write(config.scheduler.map_directory.join("other.json"), serde_json::to_vec(&map).unwrap()).unwrap();
    let mut expected_runs = Vec::new();
    for (index, name) in ["work.json", "work.json", "other.json"].iter().enumerate() {
        let file = data.path().join(format!("actual brief {index}.txt"));
        fs::write(&file, format!("independent material {index}")).unwrap();
        let bindings = json!([{"id":"brief","path":file}]);
        let business = format!("BUSINESS-{index}: brief is a word, not a file protocol");
        let input = json!({"managerTaskId":ids[0],"zone":zone,"userInput":business,"fileInputs":bindings});
        let preflight = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/{name}/preflight"), &input.to_string()).await;
        assert_eq!(preflight.status, 200, "{}", preflight.body);
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("named-manager-{index}")},"native_cwd":data.path()}));
        let started = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/{name}/runs"), &input.to_string()).await;
        assert_eq!(started.status, 201, "{}", started.body);
        let id = serde_json::from_str::<Value>(&started.body).unwrap()["id"].as_str().unwrap().to_owned();
        expected_runs.push((id, bindings, business));
    }
    drop(server);
    let server = TestServer::start(&config).await;
    for (id, bindings, business) in expected_runs {
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(detail.status, 200, "{}", detail.body);
        let detail: Value = serde_json::from_str(&detail.body).unwrap();
        assert_eq!(detail["run"]["fileInputs"], bindings);
        assert_eq!(detail["run"]["inputsReady"], false, "readable startup bindings are not semantic confirmation, including after reload");
        assert_eq!(detail["run"]["userInput"], business);
    }
}

// S2, nesting P11: duplicate and undeclared IDs must be rejected by
// both public entries before any external effect or Run allocation. Business
// text containing an ID/path must not fill a missing binding.
#[tokio::test]
async fn invalid_root_input_ids_reject_preflight_and_start_without_host_effects() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let map_path = config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["input_files"] = json!([{"id":"brief","required":true}]);
    fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
    let file = data.path().join("brief.txt");
    fs::write(&file, "material").unwrap();
    for (bindings, code, input_id) in [
        (json!([{"id":"brief","path":file},{"id":"alien","path":file}]), "run-input-undeclared", "alien"),
        (json!([{"id":"brief","path":file},{"id":"brief","path":file}]), "run-input-duplicate", "brief"),
    ] {
        for action in ["preflight", "runs"] {
            let rejected = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/work.json/{action}"), &json!({"managerTaskId":ids[0],"zone":zone,"userInput":format!("brief {}",file.display()),"fileInputs":bindings}).to_string()).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            let body: Value = serde_json::from_str(&rejected.body).unwrap();
            assert!(body["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == code && issue["inputId"] == input_id && issue["target"] == "run-input"), "{body}");
            assert!(herdr.calls().is_empty());
            assert!(!config.scheduler.run_directory.exists());
        }
    }
}

// S2, nesting P10: omission and explicit empty bindings both mean absence,
// including undeclared interfaces. A manufactured default address fails this
// public persisted readback even when starting the manager succeeds.
#[tokio::test]
async fn roots_without_files_and_omitted_optional_inputs_start_with_empty_bindings() {
    for (declarations, bindings) in [(json!(null), None), (json!([]), Some(json!([]))), (json!([{"id":"optional","required":false}]), None)] {
        let (_data, _herdr, config, server, zone, ids) = fixture().await;
        let map_path = config.scheduler.map_directory.join("work.json");
        let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
        if !declarations.is_null() { map["input_files"] = declarations; }
        fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
        let mut input = json!({"managerTaskId":ids[0],"zone":zone,"userInput":"optional: no file was supplied"});
        if let Some(bindings) = bindings { input["fileInputs"] = bindings; }
        let started = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs", &input.to_string()).await;
        assert_eq!(started.status, 201, "{}", started.body);
        let body: Value = serde_json::from_str(&started.body).unwrap();
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{}",body["id"].as_str().unwrap())).await;
        assert_eq!(serde_json::from_str::<Value>(&detail.body).unwrap()["run"]["fileInputs"], json!([]));
    }
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
    let task = json!({"name":"manager","taskType":"manager","agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":"FIXED-ROLE-MARKER"});
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
        "/skill:teach FIXED-ROLE-MARKER\n\nRun ID: {}\nMap: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n中间资料位置: {}\n\nLAST-USER-MARKER",
        original["id"].as_str().unwrap(),
        config.scheduler.map_directory.join("work.json").display(),
        config.scheduler.run_instructions.display(),
        root.join("run.json").display(),
        root.join("history.jsonl").display(),
        root.join("messages.jsonl").display(),
        data.path().join(".manager-zone").join(".workflow").join(original["id"].as_str().unwrap()).display()
    );
    assert_eq!(common::prompt_body(&prompt), expected);
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

// The literal identities are independent expected values from canonical OMP
// filenames. Keep the native files absent: creating empty files would bypass
// the startup timing condition and exercise a malformed-header failure instead.
#[tokio::test]
async fn missing_canonical_omp_files_commit_manager_and_node_identity_before_instructions() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let manager_file = data.path().join(
        "2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl",
    );
    let node_file = data.path().join(
        "2026-10-29T02-02-17-097Z_01a18564-c889-700c-92c3-5aa0460154fe.jsonl",
    );
    let release = data.path().join("missing-native-manager-prompt-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":manager_file},
        "native_cwd":data.path(),
        "prompt_gate":{"wait_for_file":release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let manager_task = ids[0].clone();
    let manager_zone = zone.clone();
    let request = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/scheduler/maps/work.json/runs", &json!({
            "managerTaskId":manager_task,"zone":manager_zone,"userInput":"missing native startup"
        }).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|call| call["kind"] == "agent_prompt")
    }).await, "manager must reach the external first-instruction gate without a native file");
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap().next().unwrap().unwrap().path();
    let committed: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(committed["manager"]["sessionId"], "01a18564-c4a1-700b-81b2-4ffb35f043ed");
    assert_eq!(committed["managerTaskId"], ids[0]);
    let history: Value =
        serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let manager = history.as_array().unwrap().iter()
        .find(|row| row["sessionId"] == "01a18564-c4a1-700b-81b2-4ffb35f043ed").unwrap();
    assert_eq!(manager["sourceTaskId"], ids[0]);
    assert_eq!(manager["agentType"], "omp");
    assert_eq!(manager["cwd"], json!(data.path()));
    assert_eq!(manager["sessionFile"], json!(manager_file));
    assert_eq!(manager["startupIdentityPath"], json!(manager_file));
    assert!(!manager_file.exists(), "startup must not manufacture the native file");
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 201, "{}", response.body);
    let started: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(started["run"]["manager"]["sessionId"], "01a18564-c4a1-700b-81b2-4ffb35f043ed");
    let status = post_json_over_tcp(server.addr(), "/api/workflow/manager/status",
        &json!({"sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed"}).to_string()).await;
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(serde_json::from_str::<Value>(&status.body).unwrap()["snapshot"]["id"], committed["id"]);

    let prepared = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs", &json!({"sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed","fileInputs":[],"confirmed":true,"reason":"Reviewed fixture inputs before native identity checks"}).to_string()).await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let node_release = data.path().join("missing-native-node-prompt-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":node_file},
        "native_cwd":data.path(),
        "prompt_gate":{"wait_for_file":node_release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let dispatch = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({
            "sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed","nodes":["A"]
        }).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2
    }).await, "node must reach its first-instruction gate without a native file");
    let saved: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "01a18564-c889-700c-92c3-5aa0460154fe");
    assert_eq!(saved["nodes"]["A"]["execution"]["zone"], json!(data.path()));
    let history: Value =
        serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let node = history.as_array().unwrap().iter()
        .find(|row| row["sessionId"] == "01a18564-c889-700c-92c3-5aa0460154fe").unwrap();
    assert_eq!(node["sourceTaskId"], ids[1]);
    assert_eq!(node["agentType"], "omp");
    assert_eq!(node["cwd"], json!(data.path()));
    assert_eq!(node["sessionFile"], json!(node_file));
    assert_eq!(node["startupIdentityPath"], json!(node_file));
    let native = common::read_json_lines(&herdr.path().join("started.jsonl")).pop().unwrap();
    assert_eq!(native["actual_cwd"], json!(data.path()));
    assert_eq!(herdr.tabs().last().unwrap()["cwd"], json!(data.path()));
    assert!(!manager_file.exists());
    assert!(!node_file.exists());
    fs::write(node_release, b"release").unwrap();
    let dispatched = dispatch.await.unwrap();
    assert_eq!(dispatched.status, 200, "{}", dispatched.body);
    let result: Value = serde_json::from_str(&dispatched.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
    assert!(!manager_file.exists(), "manager file must remain owned by OMP");
    assert!(!node_file.exists(), "node file must remain owned by OMP");
}

#[tokio::test]
async fn missing_pi_node_file_still_submits_initial_instructions_after_association() {
    let (data, herdr, config, server, zone, ids) = fixture_for_agent("pi").await;
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-manager"},"native_cwd":data.path()}));
    let started = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    let run: Value = serde_json::from_str(&started.body).unwrap();
    let prepared = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs", &json!({"sessionId":"pi-manager","fileInputs":[],"confirmed":true,"reason":"Reviewed fixture inputs before native identity checks"}).to_string()).await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let file = data.path().join("2026-10-29T06-31-42-214Z_01a1865b-7185-74a2-af0e-9d7eb972c0ee.jsonl");
    let release = data.path().join("pi-node-input-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":file},
        "native_cwd":data.path(), "prompt_gate":{"wait_for_file":release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let request = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/manager/dispatch",
            &json!({"sessionId":"pi-manager","nodes":["A"]}).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2
    }).await, "pi node never received initial instructions while its native file was absent");
    let root = config.scheduler.run_directory.join(run["id"].as_str().unwrap());
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "01a1865b-7185-74a2-af0e-9d7eb972c0ee");
    let history: Value = serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let node = history.as_array().unwrap().iter().find(|entry| entry["sessionId"] == "01a1865b-7185-74a2-af0e-9d7eb972c0ee").unwrap();
    assert_eq!(node["startupIdentityPath"], json!(file));
    assert_eq!(node["agentType"], "pi");
    assert!(!file.exists(), "herdr-bot must not manufacture native files");
    fs::write(release, "release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let result: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
}

#[tokio::test]
async fn existing_omp_file_header_takes_precedence_over_canonical_filename() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let file = data.path().join("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl");
    let contents = format!("{}\n", json!({"type":"session","id":"actual-header-identity","cwd":data.path()}));
    fs::write(&file, &contents).unwrap();
    herdr.set_behavior(json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":file}}));
    let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs", &json!({
        "managerTaskId":ids[0],"zone":zone,"userInput":""
    }).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    let started: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(started["run"]["manager"]["sessionId"], "actual-header-identity");
    let history: Value = serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    assert_eq!(history[0]["sessionId"], "actual-header-identity");
    assert_eq!(fs::read_to_string(&file).unwrap(), contents);
}

// A plausible but unsafe implementation extracts any filename suffix, or
// falls back after every read/header error. Only official missing canonical
// paths with a reported working directory qualify for the startup exception.
#[tokio::test]
async fn invalid_native_paths_and_existing_header_errors_never_submit_instructions() {
    for case in [
        "uuid-only", "invalid-timestamp", "invalid-uuid", "wrong-extension",
        "relative-path", "empty-file", "invalid-json", "wrong-header-type",
        "missing-header-id", "unreadable-directory", "wrong-source", "wrong-agent", "missing-cwd",
    ] {
        let (data, herdr, config, server, zone, ids) = fixture().await;
        let canonical = data.path().join(
            "2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl",
        );
        let file = match case {
            "uuid-only" => data.path().join("01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            "invalid-timestamp" => data.path().join("2026-99-99T99-99-99-999Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            "invalid-uuid" => data.path().join("2026-10-29T02-02-16-097Z_not-a-uuid.jsonl"),
            "wrong-extension" => data.path().join("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.txt"),
            "relative-path" => std::path::PathBuf::from("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            _ => canonical,
        };
        let bytes = match case {
            "empty-file" => Some(""),
            "invalid-json" => Some("not json\n"),
            "wrong-header-type" => Some("{\"type\":\"message\",\"id\":\"not-a-session\"}\n"),
            "missing-header-id" => Some("{\"type\":\"session\"}\n"),
            _ => None,
        };
        if let Some(bytes) = bytes {
            fs::write(&file, bytes).unwrap();
        } else if case == "unreadable-directory" {
            fs::create_dir(&file).unwrap();
        }
        herdr.set_behavior(json!({
            "native_session":{
                "source":if case == "wrong-source" { "custom:omp" } else { "herdr:omp" },
                "agent":if case == "wrong-agent" { "pi" } else { "omp" },
                "kind":"path","value":file
            },
            "native_cwd":if case == "missing-cwd" { Value::Null } else { json!(data.path()) }
        }));
        let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string()).await;
        assert_eq!(response.status, 500, "{case}: {}", response.body);
        let failure: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(failure["startupError"]["stage"], "identity", "{case}: {}", response.body);
        assert_eq!(failure["startupError"]["effects"]["inputAttempted"], false, "{case}");
        let calls = herdr.calls();
        assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_get").count(), 1, "{case}: invalid identity is not a pending identity");
        assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"), "{case}");
        let root = config.scheduler.run_directory.join(failure["id"].as_str().unwrap());
        let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
        assert!(saved["manager"].is_null(), "{case}");
        if let Some(bytes) = bytes {
            assert_eq!(fs::read(&file).unwrap(), bytes.as_bytes(), "{case}");
        } else if case == "unreadable-directory" {
            assert!(file.is_dir());
        } else if case != "relative-path" {
            assert!(!file.exists(), "{case}: rejection must not manufacture a native file");
        }
    }
}

// Herdr can acknowledge agent start before publishing agent_session. Waiting
// for that read-only fact must not duplicate the agent or submit input early.
#[tokio::test]
async fn manager_waits_for_late_native_identity_without_restarting_or_replaying_input() {
    let (data, herdr, _config, server, zone, ids) = fixture().await;
    herdr.set_behavior(json!({
        "native_session_pending_reads":2,
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"late-manager-identity"},
        "native_cwd":data.path()
    }));
    let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Initialize once after identity"}).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["run"]["manager"]["sessionId"], "late-manager-identity");
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    let prompt = calls.iter().position(|call| call["kind"] == "agent_prompt").unwrap();
    assert!(calls[..prompt].iter().filter(|call| call["kind"] == "agent_get").count() >= 3);
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_prompt").count(), 1);
}

#[tokio::test]
async fn manager_identity_wait_expires_without_restarting_or_submitting_input() {
    let (_data, herdr, _config, server, zone, ids) = fixture().await;
    herdr.set_behavior(json!({"native_session":null}));
    let started = std::time::Instant::now();
    let response = tokio::time::timeout(std::time::Duration::from_secs(40),
        post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Do not send without identity"}).to_string())
    ).await.expect("identity acquisition must terminate within its bounded startup budget");
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(started.elapsed() >= std::time::Duration::from_secs(30), "missing identity must get the full acquisition window");
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["startupError"]["stage"], "identity");
    assert_eq!(body["startupError"]["effects"]["agentStarted"], true);
    assert_eq!(body["startupError"]["effects"]["inputAttempted"], false);
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"));
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

// A failed Chat-history write follows a committed Run association, but must
// still stop before initial input. Only the startup diagnostic is process-local.
#[tokio::test]
async fn manager_history_failure_retains_saved_identity_without_initial_prompt() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let release = data.path().join("history-identity-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-history-failed"},"native_cwd":data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let request = common::OpenJsonRequest::submit(
        server.addr(),
        "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
    ).await;
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|call| call["kind"] == "agent_get")
    }).await);
    let history = &config.sessions.history_file;
    let original_history = fs::read(history).ok();
    if history.exists() { fs::remove_file(history).unwrap(); }
    fs::create_dir_all(history).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.response().await;
    assert_eq!(response.status, 500, "{}", response.body);
    let failure: Value = serde_json::from_str(&response.body).unwrap();
    let error = &failure["startupError"];
    assert_eq!(error["stage"], "history");
    assert_eq!(error["effects"]["sessionId"], "manager-history-failed");
    assert_eq!(error["effects"]["agentStarted"], true);
    assert_eq!(error["effects"]["associationSaved"], true);
    assert_eq!(error["effects"]["inputAttempted"], false);
    assert_eq!(error["effects"]["inputSubmitted"], false);
    let id = failure["id"].as_str().unwrap();
    let root = config.scheduler.run_directory.join(id);
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["manager"]["sessionId"], "manager-history-failed");
    assert!(saved.get("startupError").is_none());
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
    assert!(history.is_dir(), "failed history publication must not replace the fault");
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"));
    let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    assert_eq!(serde_json::from_str::<Value>(&detail.body).unwrap()["startupError"], *error);
    let restarted = TestServer::start(&config).await;
    let detail = get_over_tcp(restarted.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert!(detail.get("startupError").is_none() || detail["startupError"].is_null());
    assert_eq!(detail["run"]["manager"], saved["manager"]);
    assert_eq!(herdr.calls(), calls);
    fs::remove_dir(history).unwrap();
    if let Some(bytes) = original_history { fs::write(history, bytes).unwrap(); }
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
        let prompt = common::prompt_body(calls
            .iter()
            .rev()
            .find(|c| c["kind"] == "agent_prompt")
            .unwrap());
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
