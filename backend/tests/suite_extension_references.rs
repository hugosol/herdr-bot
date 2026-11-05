//! Suite source paths are persisted; launches independently resolve current declarations.
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use herdr_bot::{suites::Suites, tasks::{Task, Tasks}};
use serde_json::json;
use std::{fs, path::Path, sync::Arc};

fn task() -> Task {
    serde_json::from_value(json!({"id":"task-1","name":"Web","chatName":"{task.name}","agent":"pi","taskType":"normal","model":"default","thinking":"default","skill":null,"prompt":"","suite":"web"})).unwrap()
}

#[test]
fn next_launch_and_recreated_runtime_use_latest_suite_paths_and_deduplicate_sources() {
    let root = TempDir::new("latest Suite extension source");
    let definitions = root.path().join("suite authority");
    let sources_dir = root.path().join("source entries");
    fs::create_dir(&definitions).unwrap();
    fs::create_dir(&sources_dir).unwrap();
    let first = sources_dir.join("first.ts");
    let second = sources_dir.join("second.ts");
    fs::write(&first, "export default function(pi) {}\n").unwrap();
    fs::write(&second, "export default function(pi) {}\n").unwrap();
    let suite_file = definitions.join("suites.json");
    let mut declaration = json!({"suites":[{"id":"web","name":"Web","agents":["pi"],"pi":{"codemode":"on"},"extensions":{"pi":["../source entries/first.ts", first]}}]});
    fs::write(&suite_file, declaration.to_string()).unwrap();
    let make_runtime = || Tasks::new(root.path().join("tasks.json"), common::test_metadata_file(), Default::default(), Arc::new(Suites::new(suite_file.clone(), root.path().join("mcp.json"), root.path().join("generated"))));
    let runtime = make_runtime();
    let sources = |args: Vec<String>| -> Vec<String> { args.windows(2).filter(|pair| pair[0] == "--extension" && pair[1].ends_with(".ts")).map(|pair| pair[1].clone()).collect() };
    assert_eq!(sources(runtime.prepare_environment(&task()).unwrap()), vec![first.to_string_lossy().into_owned()]);
    declaration["suites"][0]["extensions"]["pi"] = json!(["../source entries/second.ts"]);
    fs::write(&suite_file, declaration.to_string()).unwrap();
    let expected = vec![second.to_string_lossy().into_owned()];
    assert_eq!(sources(runtime.prepare_environment(&task()).unwrap()), expected);
    assert_eq!(sources(make_runtime().prepare_environment(&task()).unwrap()), expected);
    let before = fs::read_dir(root.path().join("generated/web")).unwrap().count();
    declaration["suites"][0]["extensions"]["pi"] = json!(["../source entries"]);
    // Pi directories are rejected even when a native manifest names a file.
    fs::write(sources_dir.join("package.json"), r#"{"pi":{"extensions":["second.ts"]}}"#).unwrap();
    let bytes = declaration.to_string();
    fs::write(&suite_file, &bytes).unwrap();
    assert!(runtime.prepare_environment(&task()).unwrap_err().message().contains("具体文件"));
    assert_eq!(fs::read_dir(root.path().join("generated/web")).unwrap().count(), before);
    assert_eq!(fs::read_to_string(&suite_file).unwrap(), bytes);
    for output in fs::read_dir(root.path().join("generated/web")).unwrap() {
        assert!(!output.unwrap().path().join("extensions").exists());
    }
}

#[test]
fn connect_bot_loads_once_with_or_without_suite_for_each_host_and_execution_role() {
    use herdr_bot::tasks::{Agent, catalog::ExecutionRole};
    let root = TempDir::new("fixed connect launch boundary");
    let suite_file = root.path().join("suites.json");
    fs::write(&suite_file, json!({"suites":[{"id":"web","name":"Selected","agents":["omp","pi"],"pi":{"codemode":"on"}}]}).to_string()).unwrap();
    let runtime = Tasks::new(root.path().join("tasks.json"), common::test_metadata_file(), Default::default(),
        Arc::new(Suites::new(suite_file, root.path().join("mcp.json"), root.path().join("generated"))));
    for agent in [Agent::Omp, Agent::Pi] {
        for suite in [None, Some("web")] {
            let mut selected = task();
            selected.agent = agent;
            selected.env.suite = suite.map(str::to_owned);
            for role in [ExecutionRole::Standalone, ExecutionRole::Manager, ExecutionRole::Node] {
                let mut preset = runtime.catalog().unwrap().resolve_for_role(&selected, role).unwrap();
                preset.env.suite_args = runtime.prepare_environment(&selected).unwrap();
                let connect = preset.env.extensions.iter().find(|extension| extension.id == format!("{}-connect-bot", agent.label())).unwrap();
                let expected = connect.entrypoint.to_string_lossy().into_owned();
                let args = herdr_bot::chats::agent_launch_args(&preset.env, None);
                assert_eq!(args.windows(2).filter(|pair| pair[0] == "--extension" && pair[1] == expected).count(), 1,
                    "{agent:?} {suite:?} {role:?}: {args:?}");
                assert!(preset.env.extensions.iter().all(|extension| extension.agent == agent));
                for extension in &preset.env.extensions {
                    if extension.id.ends_with("-manager-chat") { assert_eq!(role, ExecutionRole::Manager); }
                    if extension.id.ends_with("-node-chat") { assert_eq!(role, ExecutionRole::Node); }
                }
            }
        }
    }
}

#[tokio::test]
async fn missing_entries_allow_definition_task_save_and_preview_but_reject_launch_without_herdr() {
    for invalid in ["missing.ts", "missing directory/entry.ts"] {
        let root = TempDir::new("invalid shared reference");
        let herdr = common::HerdrFixture::new("invalid reference Herdr");
        let mut config = common::config(Path::new("unused.bat"), root.path());
        config.tasks.storage_file = root.path().join("tasks.json");
        config.zones.registry_file = root.path().join("zones.json");
        config.suites.definition_file = root.path().join("suites.json");
        config.suites.mcp_file = root.path().join("mcp.json");
        config.suites.generated_directory = root.path().join("generated");
        config.herdr.executable = herdr.executable().to_path_buf();
        let server = TestServer::start(&config).await;
        let declaration = json!({"id":"web","name":"Web","agents":["pi"],"pi":{"codemode":"on"},"extensions":{"pi":[invalid]}});
        // Saving records declarations without requiring installed source resources.
        let reply = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&declaration.to_string())).await;
        assert_eq!(reply.status, 200, "{}", reply.body);
        let reply = common::get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert!(reply.body.contains(invalid));
        let mut task_input = serde_json::to_value(task()).unwrap();
        task_input.as_object_mut().unwrap().remove("id");
        let saved = request_json_over_tcp(server.addr(), "POST", "/api/tasks", Some(&task_input.to_string())).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        fs::write(&config.tasks.storage_file, json!({"tasks":[task()]}).to_string()).unwrap();
        let input = json!({"zone":"owned","taskId":"task-1"});
        let zone = request_json_over_tcp(server.addr(), "POST", "/api/zones", Some(&json!({"id":"owned","path":root.path()}).to_string())).await;
        assert_eq!(zone.status, 200, "{}", zone.body);
        let preview = request_json_over_tcp(server.addr(), "POST", "/api/chats/preview", Some(&input.to_string())).await;
        // Preview resolves the Task and Suite declarations; resource readability is checked at launch.
        assert_eq!(preview.status, 200, "{}", preview.body);
        let preview: serde_json::Value = serde_json::from_str(&preview.body).unwrap();
        let launch = request_json_over_tcp(server.addr(), "POST", "/api/chats", Some(&json!({"zone":"owned","taskId":"task-1","chatId":preview["chatId"],"input":"reject"}).to_string())).await;
        assert_eq!(launch.status, 400, "{}", launch.body);
        assert!(herdr.calls().is_empty());
        assert!(!config.suites.generated_directory.exists());
    }
}
