//! S2 Task/Suite reference contracts. All expectations are independent literals.
mod common;
use common::{TempDir, TestServer, get_over_tcp, request_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

fn config(root: &TempDir) -> herdr_bot::Config {
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("central-mcp.json");
    fs::write(&config.suites.definition_file, json!({"suites":[
        {"id":"shared","name":"Shared tools","agents":["omp"],"mcp":["search"]},
        {"id":"pi-only","name":"Pi tools","agents":["pi"],"pi":{"codemode":"on"}}
    ]}).to_string()).unwrap();
    fs::write(&config.suites.mcp_file, r#"{"mcpServers":{"search":{"command":"controlled-search"}}}"#).unwrap();
    config
}
fn task(name: &str, suite: Value) -> Value {
    json!({"name":name,"chatName":"{task.name}","agent":"omp","taskType":"normal","model":"default","thinking":"default","skill":null,"prompt":"Keep my trigger","suite":suite})
}
async fn send(server: &TestServer, method: &str, path: &str, body: &Value) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), method, path, Some(&body.to_string())).await
}
fn body(response: &common::HttpResponse) -> Value { serde_json::from_str(&response.body).unwrap() }

// P1/P2/P17: the approved S2 seam is Task POST/PUT/GET. A plausible violation
// accepts an array or silently substitutes an incompatible/missing selection.
// Distinct IDs must round-trip the same literal central reference unchanged.
#[tokio::test]
async fn task_accepts_one_compatible_suite_or_none_and_shares_stable_references() {
    let root = TempDir::new("suite task selection");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let first = send(&server, "POST", "/api/tasks", &task("First", json!("shared"))).await;
    assert_eq!(first.status, 200, "{}", first.body);
    let id1 = body(&first)["tasks"][0]["id"].as_str().unwrap().to_owned();
    let second = send(&server, "POST", "/api/tasks", &task("Second", json!("shared"))).await;
    assert_eq!(second.status, 200, "{}", second.body);
    let id2 = body(&second)["tasks"][1]["id"].as_str().unwrap().to_owned();
    assert_ne!(id1, id2);
    for id in [&id1, &id2] {
        let read = get_over_tcp(server.addr(), &format!("/api/tasks/{id}")).await;
        assert_eq!(read.status, 200, "{}", read.body);
        assert_eq!(body(&read)["task"]["suite"], "shared");
        assert_eq!(body(&read)["task"]["prompt"], "Keep my trigger");
    }
    let before = body(&get_over_tcp(server.addr(), "/api/tasks").await)["tasks"].clone();
    for invalid in [json!(["shared", "pi-only"]), json!("pi-only"), json!("missing"), json!("")] {
        for (method, path) in [("POST", "/api/tasks".to_owned()), ("PUT", format!("/api/tasks/{id1}"))] {
            let rejected = send(&server, method, &path, &task("Invalid", invalid.clone())).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            assert_eq!(body(&get_over_tcp(server.addr(), "/api/tasks").await)["tasks"], before);
        }
    }
    let none = send(&server, "PUT", &format!("/api/tasks/{id1}"), &task("First", Value::Null)).await;
    assert_eq!(none.status, 200, "{}", none.body);
    assert_eq!(body(&get_over_tcp(server.addr(), &format!("/api/tasks/{id1}")).await)["task"]["suite"], Value::Null);
}

// P1/P46: re-reading at save and launch must catch an external deletion or
// host change after preview. A save-only validator would still accept launch.
#[tokio::test]
async fn deleted_or_incompatible_suite_after_preview_never_starts_an_empty_environment() {
    for changed in [json!({"suites":[]}), json!({"suites":[{"id":"shared","name":"Now Pi","agents":["pi"],"pi":{"codemode":"on"}}]})] {
        let root = TempDir::new("changed suite before launch");
        let mut config = config(&root);
        let herdr = common::HerdrFixture::new("changed suite Herdr");
        config.herdr.executable = herdr.executable().to_path_buf();
        let server = TestServer::start(&config).await;
        let input = task("Stable source", json!("shared"));
        let saved = send(&server, "POST", "/api/tasks", &input).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let original = body(&saved)["tasks"][0].clone();
        let id = original["id"].as_str().unwrap();
        let registered = send(&server, "POST", "/api/zones", &json!({"id":"zone", "path":root.path()})).await;
        assert_eq!(registered.status, 200, "{}", registered.body);
        let preview = send(&server, "POST", "/api/chats/preview", &json!({"zone":"zone", "taskId":id})).await;
        assert_eq!(preview.status, 200, "{}", preview.body);
        fs::write(&config.suites.definition_file, changed.to_string()).unwrap();
        let rejected = send(&server, "PUT", &format!("/api/tasks/{id}"), &input).await;
        assert_eq!(rejected.status, 400, "{}", rejected.body);
        let launch = send(&server, "POST", "/api/chats", &json!({"zone":"zone", "taskId":id, "chatId":body(&preview)["chatId"], "input":"must not start"})).await;
        assert_eq!(launch.status, 400, "{}", launch.body);
        assert_eq!(body(&launch)["code"], "task-suite-invalid");
        assert!(launch.body.contains("shared"));
        assert_eq!(body(&get_over_tcp(server.addr(), &format!("/api/tasks/{id}")).await)["task"], original);
        assert!(herdr.calls().is_empty());
    }
}

// P17/P18: Suite MCP declarations contain central IDs, not copied native
// settings. A save that validates only structure would accept a deleted ID.
#[tokio::test]
async fn suite_saves_validate_central_mcp_ids_without_copying_settings() {
    let root = TempDir::new("suite central MCP saves");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let central = fs::read(&config.suites.mcp_file).unwrap();
    let literal = json!({"id":"reference-only","name":"Reference only","agents":["omp"],"mcp":["search"]});
    let saved = send(&server, "POST", "/api/suites", &literal).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = get_over_tcp(server.addr(), "/api/suites/reference-only").await;
    assert_eq!(body(&read)["suite"]["mcp"], json!(["search"]));
    assert!(!read.body.contains("controlled-search"));
    assert_eq!(fs::read(&config.suites.mcp_file).unwrap(), central);
    let before = fs::read(&config.suites.definition_file).unwrap();
    fs::write(&config.suites.mcp_file, r#"{"mcpServers":{}}"#).unwrap();
    let rejected = send(&server, "PUT", "/api/suites/reference-only", &literal).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(rejected.body.contains("search"));
    let mut missing = literal.clone();
    missing["id"] = json!("new-missing");
    let rejected = send(&server, "POST", "/api/suites", &missing).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert_eq!(fs::read(&config.suites.definition_file).unwrap(), before);
}

// P33: a direct deletion must list every referencing stable Task and leave
// both the Suite and references readable, including an externally edited Task.
#[tokio::test]
async fn referenced_suite_deletion_lists_tasks_and_preserves_references() {
    let root = TempDir::new("referenced suite deletion");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    for name in ["First reference", "Second reference"] {
        let saved = send(&server, "POST", "/api/tasks", &task(name, json!("shared"))).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
    }
    let before = body(&get_over_tcp(server.addr(), "/api/tasks").await)["tasks"].clone();
    let suite_before = body(&get_over_tcp(server.addr(), "/api/suites/shared").await);
    let rejected = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/shared", None).await;
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    assert_eq!(body(&rejected)["code"], "suite-in-use");
    for entry in before.as_array().unwrap() {
        assert!(rejected.body.contains(entry["id"].as_str().unwrap()), "{}", rejected.body);
        assert!(rejected.body.contains(entry["name"].as_str().unwrap()), "{}", rejected.body);
        assert_eq!(body(&get_over_tcp(server.addr(), &format!("/api/tasks/{}", entry["id"].as_str().unwrap())).await)["task"], *entry);
    }
    assert_eq!(body(&get_over_tcp(server.addr(), "/api/suites/shared").await), suite_before);
    for entry in before.as_array().unwrap() {
        let response = request_json_over_tcp(server.addr(), "DELETE", &format!("/api/tasks/{}", entry["id"].as_str().unwrap()), None).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    let mut external = before[0].clone();
    external["name"] = json!("External reference");
    fs::write(&config.tasks.storage_file, json!({"tasks":[external]}).to_string()).unwrap();
    let external_rejected = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/shared", None).await;
    assert_eq!(external_rejected.status, 409, "{}", external_rejected.body);
    assert!(external_rejected.body.contains("External reference"));
    assert_eq!(body(&get_over_tcp(server.addr(), "/api/tasks").await)["tasks"], json!([external]));
}

// P45: both requests have connected and sent all but their last byte before
// the barrier releases them. No sleeps, private hooks or test-only API. The
// existing shared FileStore scope is expected to satisfy this criterion already.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_delete_and_new_reference_cannot_leave_a_dangling_task() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn gated_request(addr: std::net::SocketAddr, method: &str, path: &str, payload: &str, gate: &tokio::sync::Barrier) -> u16 {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}", payload.len());
        let split = request.len() - 1;
        stream.write_all(&request.as_bytes()[..split]).await.unwrap();
        stream.flush().await.unwrap();
        gate.wait().await;
        stream.write_all(&request.as_bytes()[split..]).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        String::from_utf8(raw).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap()
    }
    let root = TempDir::new("concurrent suite reference");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    for round in 0..24 {
        let suite = json!({"id":format!("race-{round}"),"name":"Race tools","agents":["omp"]});
        let created = send(&server, "POST", "/api/suites", &suite).await;
        assert_eq!(created.status, 200, "{}", created.body);
        let payload = task("Concurrent reference", suite["id"].clone()).to_string();
        let (method, task_path) = if round % 2 == 0 {
            ("POST", "/api/tasks".to_owned())
        } else {
            let unselected = send(&server, "POST", "/api/tasks", &task("No Suite yet", Value::Null)).await;
            assert_eq!(unselected.status, 200, "{}", unselected.body);
            let list = body(&unselected);
            let id = list["tasks"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap();
            ("PUT", format!("/api/tasks/{id}"))
        };
        let path = format!("/api/suites/race-{round}");
        let gate = tokio::sync::Barrier::new(2);
        let (save, delete) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(gated_request(server.addr(), method, &task_path, &payload, &gate), gated_request(server.addr(), "DELETE", &path, "", &gate))
        }).await.expect("overlapping requests complete without deadlock");
        assert!((save == 200 && delete == 409) || (save == 400 && delete == 200), "save={save}, delete={delete}");
        let persisted_suites: Value = serde_json::from_slice(&fs::read(&config.suites.definition_file).unwrap()).unwrap();
        let read = body(&get_over_tcp(server.addr(), "/api/tasks").await)["tasks"].clone();
        let persisted_tasks: Value = match fs::read(&config.tasks.storage_file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({"tasks":[]}),
            Err(error) => panic!("{error}"),
        };
        assert_eq!(persisted_tasks["tasks"], read);
        for task in read.as_array().unwrap().iter().filter(|task| !task["suite"].is_null()) {
            assert!(persisted_suites["suites"].as_array().unwrap().iter().any(|suite| suite["id"] == task["suite"]), "dangling reference: {task}");
        }
        let suite_read = get_over_tcp(server.addr(), &path).await;
        assert_eq!(suite_read.status, if save == 200 { 200 } else { 404 });
    }
}

// P18/P46: deleting an external central MCP entry must remain observable. A
// stale cached catalog or an empty-environment fallback would accept launch.
#[tokio::test]
async fn missing_central_mcp_is_readable_but_save_and_launch_are_refused() {
    let root = TempDir::new("missing suite MCP");
    let mut config = config(&root);
    let herdr = common::HerdrFixture::new("missing suite MCP Herdr");
    config.herdr.executable = herdr.executable().to_path_buf();
    let server = TestServer::start(&config).await;
    let saved = send(&server, "POST", "/api/tasks", &task("MCP source", json!("shared"))).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let id = body(&saved)["tasks"][0]["id"].as_str().unwrap().to_owned();
    let registered = send(&server, "POST", "/api/zones", &json!({"id":"zone", "path":root.path()})).await;
    assert_eq!(registered.status, 200, "{}", registered.body);
    let preview = send(&server, "POST", "/api/chats/preview", &json!({"zone":"zone", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let bytes = fs::read(&config.tasks.storage_file).unwrap();
    let suites = fs::read(&config.suites.definition_file).unwrap();
    fs::write(&config.suites.mcp_file, r#"{"mcpServers":{}}"#).unwrap();
    let read = get_over_tcp(server.addr(), &format!("/api/tasks/{id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(body(&read)["task"]["suite"], "shared");
    let rejected = send(&server, "PUT", &format!("/api/tasks/{id}"), &task("MCP source", json!("shared"))).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(rejected.body.contains("search"), "{}", rejected.body);
    let launch = send(&server, "POST", "/api/chats", &json!({"zone":"zone", "taskId":id, "chatId":body(&preview)["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert_eq!(body(&launch)["code"], "task-suite-invalid");
    assert!(launch.body.contains("search"), "{}", launch.body);
    assert!(herdr.calls().is_empty(), "invalid reference must not reach Herdr");
    assert_eq!(fs::read(&config.tasks.storage_file).unwrap(), bytes);
    assert_eq!(fs::read(&config.suites.definition_file).unwrap(), suites);
}

