//! S3 offline deployment seam; all inputs and mutable records are test-owned.
//! P39/P40/P43: approved stable IDs, not mutable names, determine assignments.
//! A plausible violation is a name-based migration or rebuilding the Task list.
mod common;
use common::TempDir;
use serde_json::{Value, json};
use std::{fs, process::Command};

struct OwnedMigration(std::process::Child);
impl Drop for OwnedMigration {
    fn drop(&mut self) {
        let cleanup = (|| -> std::io::Result<()> {
            if self.0.try_wait()?.is_none() {
                let terminated = self.0.kill();
                let exited = self.0.wait();
                exited?;
                terminated?;
            }
            Ok(())
        })();
        if let Err(error) = cleanup {
            eprintln!("Owned migration cleanup failed separately: {error}");
            if !std::thread::panicking() { panic!("owned migration process did not cleanly exit: {error}"); }
        }
    }
}

const IDS: [&str; 22] = [
    "tpl-18e0c8126d1ad150-0", "tpl-18e0ceac0a89f9bc-0", "tpl-18e0e21b49001550-0",
    "tpl-18e104e3c5eaca34-0", "tpl-18e11ab9f1807a04-0", "task-18e25fe9c214b9d0-0",
    "task-18e2c2827c484128-0", "task-18e315be1cf0ef10-0", "task-18e318b13b71a8bc-0",
    "task-18e319d73777fc6c-1", "task-18e31d30fe69a51c-2", "task-18e31d34797f1464-3",
    "task-18e31d347ccbeb4c-4", "task-18e33b438fa74338-0", "task-18e33fdc55177d1c-1",
    "task-18e3404478db1ef8-2", "task-18e340545eb481bc-3", "task-18e340f5287462ec-4",
    "task-18e3415003d8cd00-5", "task-18e3454a33cecffc-6", "task-18e34ad4c8802738-7",
    "task-18e44b3c39532a38-0",
];
fn release() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../migrations/initial-suite-v1") }
fn run(root: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_migrate-initial-suites"))
        .arg(root.path().join("tasks.json")).arg(root.path().join("suites.json"))
        .arg(root.path().join("mcp.json")).arg(root.path().join("state"))
        .arg(release()).output().unwrap()
}
fn read(root: &TempDir, name: &str) -> Value { serde_json::from_slice(&fs::read(root.path().join(name)).unwrap()).unwrap() }
fn fixture() -> TempDir {
    let root = TempDir::new("initial suite migration");
    // Independent approved baseline: no fixture expectation is computed from
    // the mapping that the migration itself consumes.
    let tasks: Vec<_> = IDS.iter().enumerate().map(|(i, id)| json!({
        "id":id,"name":format!("Renamed {i}"),"chatName":"literal {chat.id}","taskType":if i == 6 {"manager"} else {"normal"},
        "agent":if [1,3,11,14].contains(&i) {"pi"} else {"omp"},"model":"edited-model","thinking":"high","skill":"teach",
        "prompt":"new user edit must survive\nline two","extensions":if i == 1 {json!(["pi-web-access"])} else if i == 2 {json!(["omp-codebase-tools","omp-connect-bot","omp-oh-my-wayfinder"])} else if [0,4,5,7,8,9,10,12,20].contains(&i) {json!(["omp-codebase-tools"])} else {json!([])}
    })).collect();
    fs::write(root.path().join("tasks.json"), json!({"tasks":tasks}).to_string()).unwrap();
    root
}
#[test]
fn approved_ids_migrate_without_rebuilding_tasks_or_changing_references() {
    let root = fixture();
    let before = read(&root, "tasks.json");
    // Independently owned Bot, Chat history and Run/node facts use their real
    // storage locations; no migration output supplies these expectations.
    let references = [
        ("meta-data/bots/worker.json",json!({"id":"worker","name":"Worker","when":"","plan_guide":"","node_guide":"","task_id":IDS[0]})),
        (".data/session-history.json",json!([{"sessionId":"native-91","agentType":"omp","sourceTaskId":IDS[2],"cwd":"C:/owned-zone","firstRecordedAt":123}])),
        (".data/workflow-runs/run-41/run.json",json!({"id":"run-41","manager":{"task_id":IDS[6],"session_id":"native-manager"},"nodes":{"node-7":{"task_id":IDS[4],"session_id":"native-82","execution_id":"execution-9"}}})),
    ];
    for (path,value) in &references {
        fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
        fs::write(root.path().join(path),value.to_string()).unwrap();
    }
    let result = run(&root);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let after = read(&root, "tasks.json");
    for (i, id) in IDS.iter().enumerate() {
        let task = &after["tasks"][i];
        assert_eq!(task["id"], *id);
        let expected = if [1,3,6,18].contains(&i) { Value::Null } else if i == 2 { json!("code-implementation") } else { json!("codebase-exploration") };
        assert_eq!(task["suite"], expected, "{id}");
        assert!(task.get("extensions").is_none());
        let mut preserved = task.clone(); preserved.as_object_mut().unwrap().remove("suite");
        let mut original = before["tasks"][i].clone(); original.as_object_mut().unwrap().remove("extensions");
        assert_eq!(preserved, original, "identity and each trigger field must survive");
    }
    for (path,value) in references {
        assert_eq!(fs::read_to_string(root.path().join(&path)).unwrap(),value.to_string(),"{path}");
    }
    assert_eq!(read(&root,"meta-data/bots/worker.json")["task_id"],IDS[0]);
    assert_eq!(read(&root,".data/session-history.json")[0]["sourceTaskId"],IDS[2]);
    assert_eq!(read(&root,".data/session-history.json")[0]["sessionId"],"native-91");
    let run_record = read(&root,".data/workflow-runs/run-41/run.json");
    assert_eq!(run_record["id"],"run-41");
    assert_eq!(run_record["manager"]["task_id"],IDS[6]);
    assert_eq!(run_record["nodes"]["node-7"]["task_id"],IDS[4]);
    assert_eq!(run_record["nodes"]["node-7"]["session_id"],"native-82");
    assert_eq!(run_record["nodes"]["node-7"]["execution_id"],"execution-9");
    assert_eq!(read(&root,"suites.json")["suites"].as_array().unwrap().len(),2);
    let bytes = fs::read(root.path().join("tasks.json")).unwrap();
    assert!(run(&root).status.success());
    assert_eq!(fs::read(root.path().join("tasks.json")).unwrap(),bytes);
    assert!(root.path().join("state/complete.json").is_file());
    let mut newer = read(&root,"tasks.json");
    newer["tasks"][0]["suite"] = json!("a-new-user-suite");
    newer["tasks"][0]["prompt"] = json!("post-cutover edit");
    let edited = newer.to_string();
    fs::write(root.path().join("tasks.json"),&edited).unwrap();
    assert!(run(&root).status.success());
    assert_eq!(fs::read_to_string(root.path().join("tasks.json")).unwrap(),edited);
}

// P39/P43: the release is independently compared with the approved 18/11
// declaration, not whatever skills happen to be found in source directories.
#[tokio::test]
async fn released_suites_expose_only_the_approved_resources() {
    let root = fixture();
    assert!(run(&root).status.success());
    let mut config = common::config(std::path::Path::new("unused.bat"),root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    let server = common::TestServer::start(&config).await;
    let response = common::get_over_tcp(server.addr(),"/api/tasks").await;
    assert_eq!(response.status,200,"{}",response.body);
    let result: Value = serde_json::from_str(&response.body).unwrap();
    let suites = result["options"]["suites"].as_array().unwrap();
    assert_eq!(suites.len(),2);
    for (id, expected_names, expected_agents) in [
        ("codebase-exploration",vec!["grill-with-docs","teach","code-review","wayfinder","writing-for-agents","improve-codebase-architecture","expert-translate","grilling","domain-modeling","codebase-design","research","prototype","lighthouse","backtracer","setup-matt-pocock-skills","to-contract","to-spec","to-tickets"],json!(["omp","pi"])),
        ("code-implementation",vec!["tdd","to-tickets","retro","resolving-merge-conflicts","writing-for-agents","codebase-design","code-review","setup-matt-pocock-skills","to-contract","to-spec","domain-modeling"],json!(["omp"])),
    ] {
        let suite = suites.iter().find(|suite| suite["id"] == id).unwrap();
        let names: Vec<_> = suite["skills"].as_array().unwrap().iter().map(|path| {
            let path = path.as_str().unwrap();
            if path == "${roots.expert_translate}" { "expert-translate" } else { path.rsplit('/').next().unwrap() }
        }).collect();
        assert_eq!(names,expected_names);
        assert_eq!(suite["agents"],expected_agents);
        assert_eq!(suite["mcp"],json!(["codebase-memory-mcp"]));
        assert_eq!(suite["prompts"],json!([]));
        let languages: Vec<_> = suite["lsp"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(languages,vec!["jdtls","pyright","rust-analyzer","typescript-language-server"]);
        assert_eq!(suite["lsp"]["pyright"]["command"],"pyright-langserver");
        let mut extensions = json!({"omp":["${roots.toolkit}/entrypoints/omp-auto/codebase-tools.ts","${roots.toolkit}/entrypoints/omp-auto/connect-bot.ts"]});
        if id == "codebase-exploration" {
            extensions["pi"] = json!(["${roots.toolkit}/entrypoints/pi-auto/codebase-tools.ts","${roots.toolkit}/entrypoints/pi-auto/connect-bot.ts"]);
            assert_eq!(suite["pi"],json!({"codemode":"on"}));
        } else {
            extensions["omp"].as_array_mut().unwrap().push(json!("${roots.wayfinder}/extensions"));
            assert!(suite.get("pi").is_none());
        }
        assert_eq!(suite["extensions"],extensions);
    }
    assert_eq!(read(&root,"mcp.json"),json!({"mcpServers":{"codebase-memory-mcp":{"command":"codebase-memory-mcp","args":["--ui=true","--port=9749"]}}}));
}

// S2: each host consumes only its approved concrete paths. An array cannot
// distinguish the two hosts and must not remain as a compatibility branch.
#[tokio::test]
async fn suite_extensions_are_concrete_host_keyed_paths() {
    let root = TempDir::new("host keyed extensions");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    let server = common::TestServer::start(&config).await;
    let valid = json!({"id":"shared","name":"Shared","agents":["omp","pi"],"pi":{"codemode":"on"},"extensions":{"omp":["omp-specific.ts"],"pi":["pi-specific.ts"]}});
    let saved = common::request_json_over_tcp(server.addr(),"POST","/api/suites",Some(&valid.to_string())).await;
    assert_eq!(saved.status,200,"{}",saved.body);
    let result = common::get_over_tcp(server.addr(),"/api/suites/shared").await;
    let value: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(value["suite"]["extensions"],json!({"omp":["omp-specific.ts"],"pi":["pi-specific.ts"]}));
    for invalid in [json!([]),json!(["shared.ts"]),json!({"other":["x.ts"]})] {
        let mut input = valid.clone(); input["extensions"] = invalid;
        let rejected = common::request_json_over_tcp(server.addr(),"PUT","/api/suites/shared",Some(&input.to_string())).await;
        assert_eq!(rejected.status,400,"{}",rejected.body);
    }
}

// P39 cutover: old business selection must be rejected, including an empty
// array; silently accepting it leaves a second editable/runtime authority.
#[tokio::test]
async fn task_api_rejects_legacy_optional_extension_selection() {
    let root = TempDir::new("legacy extension cutover");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    let server = common::TestServer::start(&config).await;
    let mut input = json!({"name":"Only Suite","chatName":"{chat.id}","agent":"omp","taskType":"normal","model":"default","thinking":"default","skill":null,"prompt":"literal"});
    for old in [json!([]),json!(["omp-codebase-tools"])] {
        input["extensions"] = old;
        let rejected = common::request_json_over_tcp(server.addr(),"POST","/api/tasks",Some(&input.to_string())).await;
        assert_eq!(rejected.status,400,"{}",rejected.body);
    }
    input.as_object_mut().unwrap().remove("extensions");
    let saved = common::request_json_over_tcp(server.addr(),"POST","/api/tasks",Some(&input.to_string())).await;
    assert_eq!(saved.status,200,"{}",saved.body);
    let result: Value = serde_json::from_str(&saved.body).unwrap();
    assert!(result["tasks"][0].get("extensions").is_none());
    assert!(result["options"]["extensions"].as_array().unwrap().iter().all(|entry| entry["kind"] == "fixed"));
}

// P39: compare the current stable record before every first application;
// neither a newer environment nor a definition with the same ID is replaced.
#[test]
fn current_environment_and_definition_conflicts_are_explicit_and_non_destructive() {
    for (field, value) in [("suite",json!("new-user-suite")),("agent",json!("pi")),("extensions",json!(["user-added"]))] {
        let root = fixture();
        let mut current = read(&root,"tasks.json"); current["tasks"][0][field] = value;
        let bytes = current.to_string(); fs::write(root.path().join("tasks.json"), &bytes).unwrap();
        let failed = run(&root);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains(IDS[0]));
        assert_eq!(fs::read_to_string(root.path().join("tasks.json")).unwrap(),bytes);
        assert!(!root.path().join("suites.json").exists());
        assert!(!root.path().join("state/complete.json").exists());
    }
    let root = fixture();
    let before = fs::read(root.path().join("tasks.json")).unwrap();
    let user = json!({"suites":[{"id":"codebase-exploration","name":"User newer edit","agents":["omp"]}]}).to_string();
    fs::write(root.path().join("suites.json"),&user).unwrap();
    let failed = run(&root);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Suite"));
    assert_eq!(fs::read_to_string(root.path().join("suites.json")).unwrap(),user);
    assert_eq!(fs::read(root.path().join("tasks.json")).unwrap(),before);
    assert!(!root.path().join("mcp.json").exists());
    assert!(!root.path().join("state/complete.json").exists());
}

// P41: a real process is terminated after durable plan publication, not a
// hand-built completed state. A large test-owned MCP entry leaves an observable
// publication window; every spawned process is reaped on success and failure.
#[test]
fn process_interruption_restarts_from_its_owned_plan() {
    let root = fixture();
    fs::write(root.path().join("mcp.json"),json!({"mcpServers":{"unrelated":{"command":"unused","annotation":"M".repeat(8*1024*1024)}}}).to_string()).unwrap();
    let log = fs::File::create(root.path().join("interrupted.log")).unwrap();
    let errors = fs::File::create(root.path().join("interrupted-errors.log")).unwrap();
    let mut process = OwnedMigration(Command::new(env!("CARGO_BIN_EXE_migrate-initial-suites"))
        .arg(root.path().join("tasks.json")).arg(root.path().join("suites.json"))
        .arg(root.path().join("mcp.json")).arg(root.path().join("state")).arg(release())
        .stdout(log).stderr(errors).spawn().unwrap());
    let child = &mut process.0;
    let deadline = std::time::Instant::now()+std::time::Duration::from_secs(30);
    let observed = loop {
        if root.path().join("state/plan.json").is_file() { break true; }
        if child.try_wait().unwrap().is_some() || std::time::Instant::now() >= deadline { break false; }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let killed = child.kill();
    let exited = child.wait();
    assert!(observed,"{}",fs::read_to_string(root.path().join("interrupted-errors.log")).unwrap());
    assert!(killed.is_ok(),"migration exited before interruption: {killed:?}");
    assert!(exited.is_ok(),"owned process cleanup failed: {exited:?}");
    assert!(!root.path().join("state/complete.json").exists(),"interruption must occur before completion, not pass on an already finished run");
    let restarted = run(&root);
    assert!(restarted.status.success(),"{}",String::from_utf8_lossy(&restarted.stderr));
    assert_eq!(read(&root,"tasks.json")["tasks"].as_array().unwrap().len(),22);
    assert_eq!(read(&root,"tasks.json")["tasks"][0]["suite"],"codebase-exploration");
    assert_eq!(read(&root,"mcp.json")["mcpServers"]["unrelated"]["annotation"].as_str().unwrap().len(),8*1024*1024);
    assert!(root.path().join("state/complete.json").is_file());
}

// P41: deny the durable Task replacement, after definitions were published.
// A completion marker written after merely counting attempted writes would lie.
#[test]
fn interrupted_partial_application_never_marks_success_and_conflicting_edits_survive() {
    let root = fixture();
    let original = fs::read(root.path().join("tasks.json")).unwrap();
    let block = common::HistoryWriteBlock::new(&root.path().join("tasks.json"));
    let failed = run(&root);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("partial migration failed"));
    assert!(root.path().join("suites.json").is_file());
    assert!(root.path().join("mcp.json").is_file());
    assert!(root.path().join("state/plan.json").is_file());
    assert!(!root.path().join("state/complete.json").exists());
    assert_eq!(fs::read(root.path().join("tasks.json")).unwrap(), original);
    drop(block);
    let mut edited = read(&root,"tasks.json");
    edited["tasks"][0]["prompt"] = json!("concurrent new edit");
    let edited_bytes = edited.to_string();
    fs::write(root.path().join("tasks.json"), &edited_bytes).unwrap();
    let conflict = run(&root);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("conflict after partial migration"));
    assert_eq!(fs::read_to_string(root.path().join("tasks.json")).unwrap(), edited_bytes);
    assert!(!root.path().join("state/complete.json").exists());
    // Operator resolves the conflict to the exact recorded original; restart
    // resumes without recreating already published definitions or changing IDs.
    fs::write(root.path().join("tasks.json"), original).unwrap();
    let definitions = fs::read(root.path().join("suites.json")).unwrap();
    let recovered = run(&root);
    assert!(recovered.status.success(), "{}", String::from_utf8_lossy(&recovered.stderr));
    assert_eq!(fs::read(root.path().join("suites.json")).unwrap(), definitions);
    assert_eq!(read(&root,"tasks.json")["tasks"][2]["suite"],"code-implementation");
    assert!(root.path().join("state/complete.json").is_file());
}
