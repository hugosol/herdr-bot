//! Stable IDs are persisted; each launch resolves the current metadata snapshot.
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use herdr_bot::{suites::Suites, tasks::{Task, Tasks}};
use serde_json::json;
use std::{fs, path::Path, sync::Arc};

fn task() -> Task {
    serde_json::from_value(json!({"id":"task-1","name":"Web","chatName":"{task.name}","agent":"pi","taskType":"normal","model":"default","thinking":"default","skill":null,"prompt":"","suite":"web"})).unwrap()
}

#[test]
fn next_launch_and_recreated_runtime_use_latest_metadata_and_deduplicate_custom_sources() {
    let root = TempDir::new("latest Suite extension source");
    let definitions = root.path().join("suite authority");
    let metadata_dir = root.path().join("metadata authority");
    fs::create_dir(&definitions).unwrap();
    fs::create_dir(&metadata_dir).unwrap();
    let first = metadata_dir.join("first.ts");
    let second = metadata_dir.join("second.ts");
    fs::write(&first, "export default function(pi) {}\n").unwrap();
    fs::write(&second, "export default function(pi) {}\n").unwrap();
    let suite_file = definitions.join("suites.json");
    let suite_bytes = json!({"suites":[{"id":"web","name":"Web","agents":["pi"],"pi":{"codemode":"on"},"extensionOptions":{"pi":["pi-web-access"]},"extensions":{"pi":["../metadata authority/first.ts"]}}]}).to_string();
    fs::write(&suite_file, &suite_bytes).unwrap();
    let metadata_file = metadata_dir.join("options.toml");
    let mut metadata: toml::Value = toml::from_str(&fs::read_to_string(common::test_metadata_file()).unwrap()).unwrap();
    let entry = metadata["extensions"].as_array_mut().unwrap().iter_mut().find(|option| option["id"].as_str() == Some("pi-web-access")).unwrap();
    entry["entrypoint"] = toml::Value::String("first.ts".into());
    fs::write(&metadata_file, toml::to_string(&metadata).unwrap()).unwrap();
    let make_runtime = || Tasks::new(root.path().join("tasks.json"), metadata_file.clone(), Default::default(), Arc::new(Suites::new(suite_file.clone(), root.path().join("mcp.json"), root.path().join("generated"))));
    let runtime = make_runtime();
    let sources = |args: Vec<String>| -> Vec<String> { args.windows(2).filter(|pair| pair[0] == "--extension" && pair[1].ends_with(".ts")).map(|pair| pair[1].clone()).collect() };
    let first_path = first.to_string_lossy().into_owned();
    let second_path = second.to_string_lossy().into_owned();
    assert_eq!(sources(runtime.prepare_environment(&task()).unwrap()), vec![first_path.clone()]);
    let entry = metadata["extensions"].as_array_mut().unwrap().iter_mut().find(|option| option["id"].as_str() == Some("pi-web-access")).unwrap();
    entry["entrypoint"] = toml::Value::String("second.ts".into());
    fs::write(&metadata_file, toml::to_string(&metadata).unwrap()).unwrap();
    assert_eq!(sources(runtime.prepare_environment(&task()).unwrap()), vec![second_path.clone(), first_path.clone()]);
    assert_eq!(sources(make_runtime().prepare_environment(&task()).unwrap()), vec![second_path, first_path]);
    let before = fs::read_dir(root.path().join("generated/web")).unwrap().count();
    let entry = metadata["extensions"].as_array_mut().unwrap().iter_mut().find(|option| option["id"].as_str() == Some("pi-web-access")).unwrap();
    entry["entrypoint"] = toml::Value::String(".".into());
    // Even a valid pi.extensions manifest is not expanded by Herdr.
    fs::write(metadata_dir.join("package.json"), r#"{"pi":{"extensions":["second.ts"]}}"#).unwrap();
    fs::write(&metadata_file, toml::to_string(&metadata).unwrap()).unwrap();
    assert!(runtime.prepare_environment(&task()).unwrap_err().message().contains("具体文件"));
    assert_eq!(fs::read_dir(root.path().join("generated/web")).unwrap().count(), before);
    assert_eq!(fs::read_to_string(&suite_file).unwrap(), suite_bytes);
    for output in fs::read_dir(root.path().join("generated/web")).unwrap() {
        assert!(!output.unwrap().path().join("extensions").exists());
    }
}

#[tokio::test]
async fn missing_wrong_host_and_fixed_references_reject_save_preview_and_launch_without_herdr() {
    for invalid in ["missing-option", "omp-codebase-tools", "pi-suite"] {
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
        let declaration = json!({"id":"web","name":"Web","agents":["pi"],"pi":{"codemode":"on"},"extensionOptions":{"pi":[invalid]}});
        // Use public HTTP and retain the invalid reference on GET for repair.
        let reply = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&declaration.to_string())).await;
        assert_eq!(reply.status, 400, "{}", reply.body);
        assert!(!config.suites.definition_file.exists());
        fs::write(&config.suites.definition_file, json!({"suites":[declaration]}).to_string()).unwrap();
        let reply = common::get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert!(reply.body.contains(invalid));
        let mut task_input = serde_json::to_value(task()).unwrap();
        task_input.as_object_mut().unwrap().remove("id");
        let saved = request_json_over_tcp(server.addr(), "POST", "/api/tasks", Some(&task_input.to_string())).await;
        assert_eq!(saved.status, 400, "{}", saved.body);
        assert!(!config.tasks.storage_file.exists());
        fs::write(&config.tasks.storage_file, json!({"tasks":[task()]}).to_string()).unwrap();
        let input = json!({"zone":"owned","taskId":"task-1"});
        let zone = request_json_over_tcp(server.addr(), "POST", "/api/zones", Some(&json!({"id":"owned","path":root.path()}).to_string())).await;
        assert_eq!(zone.status, 200, "{}", zone.body);
        let preview = request_json_over_tcp(server.addr(), "POST", "/api/chats/preview", Some(&input.to_string())).await;
        assert_eq!(preview.status, 400, "{}", preview.body);
        let launch = request_json_over_tcp(server.addr(), "POST", "/api/chats", Some(&json!({"zone":"owned","taskId":"task-1","chatId":"abcdef","input":"reject"}).to_string())).await;
        assert_eq!(launch.status, 400, "{}", launch.body);
        assert!(herdr.calls().is_empty());
        assert!(!config.suites.generated_directory.exists());
    }
}
