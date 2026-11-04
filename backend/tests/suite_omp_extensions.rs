//! Approved seams S2/S3; S4 success is owned by test-real-herdr-suite-extensions.py.
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, path::Path};

async fn post(server: &TestServer, path: &str, value: Value) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), "POST", path, Some(&value.to_string())).await
}

// P46: the entry existed at preview but becomes genuinely unreadable before
// launch. Ignoring Suite extensions (or recovery's skip policy) would start Herdr.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_extension_rejects_public_launch_before_herdr() {
    let root = TempDir::new("unreadable Suite extension");
    let herdr = common::HerdrFixture::new("extension failure Herdr");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let package = root.path().join("existing extension package");
    fs::create_dir(&package).unwrap();
    let entry = package.join("entry.ts");
    fs::write(&entry, "export default function(pi) {}\n").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"],
        "extensions":{"omp":["existing extension package/entry.ts"]}
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected extension", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    use std::os::windows::fs::OpenOptionsExt;
    let _locked = fs::OpenOptions::new().read(true).share_mode(0).open(&entry).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("entry.ts"), "{}", launch.body);
    assert!(herdr.calls().is_empty(), "Suite failure must never start a partial/empty environment");
}

// P46: directory declarations include assets, not just their entry file. The
// factory could load successfully yet fail its tool later if this is ignored.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_bundled_asset_rejects_public_launch_before_herdr() {
    let root = TempDir::new("unreadable bundled extension asset");
    let herdr = common::HerdrFixture::new("extension asset failure Herdr");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let package = root.path().join("existing package");
    fs::create_dir_all(package.join("assets")).unwrap();
    fs::write(package.join("index.ts"), "export default function(pi) {}\n").unwrap();
    let asset = package.join("assets").join("prices.json");
    fs::write(&asset, "{\"unit\":19}").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "extensions":{"omp":["existing package"]}
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected extension", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    use std::os::windows::fs::OpenOptionsExt;
    let _locked = fs::OpenOptions::new().read(true).share_mode(0).open(&asset).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("prices.json"), "{}", launch.body);
    assert!(herdr.calls().is_empty());
}

// P46: all Suite resources now share one private publication tree. A file
// blocking its parent must prevent the entire environment reaching Herdr.
#[tokio::test]
async fn blocked_extension_assembly_rejects_public_launch_before_herdr() {
    let root = TempDir::new("blocked Suite extension assembly");
    let herdr = common::HerdrFixture::new("extension output failure Herdr");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let package = root.path().join("existing package");
    fs::create_dir(&package).unwrap();
    fs::write(package.join("entry.ts"), "export default function(pi) {}\n").unwrap();
    let blocked = config.suites.generated_directory.join("selected");
    fs::create_dir_all(blocked.parent().unwrap()).unwrap();
    fs::write(&blocked, "OWNED-OUTPUT-BLOCKER-492").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "extensions":{"omp":["existing package/entry.ts"]}
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected extension", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    let error: Value = serde_json::from_str(&launch.body).unwrap();
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("资源装配失败"), "{message}");
    assert!(message.contains(&blocked.to_string_lossy().to_string()), "{message}");
    assert!(herdr.calls().is_empty(), "Failed extension publication must not reach Herdr");
    assert_eq!(fs::read_to_string(&blocked).unwrap(), "OWNED-OUTPUT-BLOCKER-492");
}
