//! S2/S3: public Chat launch publishes a native incremental LSP plugin.
//! S4 startup/absence evidence belongs to test-real-herdr-suite-lsp.py, not this fixture.
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, time::{Duration, Instant}};

async fn post(server: &TestServer, path: &str, value: Value) -> Value {
    let response = request_json_over_tcp(server.addr(), "POST", path, Some(&value.to_string())).await;
    assert!(matches!(response.status, 200 | 202), "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// Expected behavior: Suite declarations enter the native plugin config stack,
// below user/project sources. Plausible violation: skills are handed off but
// LSP is silently discarded. Observe the delivered plugin, not private helpers.
#[tokio::test]
async fn omp_chat_delivers_lsp_declarations_as_incremental_native_plugin() {
    let root = TempDir::new("native lsp plugin");
    let herdr = common::HerdrFixture::new("native lsp Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated resources with spaces");
    config.herdr.executable = herdr.executable().to_path_buf();
    let servers = json!({"rust-analyzer":{"command":"rust-analyzer", "args":[],
        "fileTypes":[".rs"], "rootMarkers":["Cargo.toml","rust-analyzer.toml"]}});
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"native-lsp", "name":"Native LSP", "agents":["omp"], "lsp":servers
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let task = post(&server, "/api/tasks", json!({"name":"Native LSP", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"Inspect language service status.", "suite":"native-lsp"})).await;
    post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await;
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":task["tasks"][0]["id"]})).await;
    post(&server, "/api/chats", json!({"zone":"owned", "taskId":task["tasks"][0]["id"],
        "chatId":preview["chatId"], "input":"Inspect status."})).await;
    let deadline = Instant::now() + Duration::from_secs(15);
    let call = loop {
        if let Some(call) = herdr.calls().into_iter().find(|call| call["kind"] == "agent_start") { break call; }
        assert!(Instant::now() < deadline, "Herdr did not receive public Chat launch");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let args = call["args"].as_array().unwrap();
    let plugin = args.windows(2).find(|pair| pair[0] == "--plugin-dir")
        .expect("Suite LSP must reach native plugin discovery, not be discarded");
    let directory = std::path::Path::new(plugin[1].as_str().unwrap());
    assert!(directory.starts_with(&config.suites.generated_directory));
    let delivered: Value = serde_json::from_slice(&fs::read(directory.join("lsp.json")).unwrap()).unwrap();
    assert_eq!(delivered, json!({"servers":servers}));
    assert!(!root.path().join("lsp.json").exists(), "do not overwrite native project configuration");
}
