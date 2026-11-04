//! Approved seams: S2 public launch, S3 deployment configuration, S4 real Herdr
//! (the latter is exercised by scripts/test-real-herdr-suite-skills.py).
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

async fn post(server: &TestServer, path: &str, value: Value) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), "POST", path, Some(&value.to_string())).await
}

// P46: after preview, an existing SKILL.md becomes unreadable under a real
// Windows sharing lock. Independent source: fail-closed launch, not a missing
// path approximation. A stale availability check would launch an empty env.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_selected_skill_rejects_public_launch_before_herdr() {
    let root = TempDir::new("unreadable selected skill");
    let herdr = common::HerdrFixture::new("skill failure Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let skill = root.path().join("unreadable skill");
    fs::create_dir(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "---\nname: selected\ndescription: Selected skill\n---\nSelected body").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "skills":["unreadable skill"]
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected skill", "chatName":"{task.name}",
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
    let _locked = fs::OpenOptions::new().read(true).share_mode(0).open(skill.join("SKILL.md")).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("unreadable skill"), "{}", launch.body);
    assert!(herdr.calls().is_empty(), "no partial/empty startup is permitted");
}

// P46/S2+S3: an output location blocked by a regular file must fail closed,
// even with readable selected input. Checking resources alone would pass.
#[tokio::test]
async fn blocked_assembly_directory_rejects_public_launch_before_herdr() {
    let root = TempDir::new("blocked skill assembly");
    let herdr = common::HerdrFixture::new("blocked assembly Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("blocked");
    config.herdr.executable = herdr.executable().to_path_buf();
    fs::create_dir(root.path().join("readable skill")).unwrap();
    fs::write(root.path().join("readable skill/SKILL.md"), "---\nname: selected\ndescription: Selected skill\n---\nSelected body").unwrap();
    fs::write(&config.suites.generated_directory, "not a directory").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "skills":["readable skill"]
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected skill", "chatName":"{task.name}",
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
    assert!(launch.body.contains("selected"), "{}", launch.body);
    assert!(herdr.calls().is_empty());
    assert_eq!(fs::read_to_string(&config.suites.generated_directory).unwrap(), "not a directory");
}
