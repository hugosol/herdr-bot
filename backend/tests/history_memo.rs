//! Public browser HTTP boundary; persistent legacy fixtures and controlled Herdr inventory.
mod common;
use axum::{body::{Body, to_bytes}, extract::connect_info::MockConnectInfo, http::Request};
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, request_json_over_tcp};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use tower::ServiceExt;
struct NeverPowerOff;
impl herdr_bot::PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> { panic!("memo never powers off") }
}
fn config(root: &TempDir, herdr: &HerdrFixture) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    config.herdr.executable = herdr.executable().to_owned();
    std::fs::write(&config.sessions.history_file, json!([
        {"sessionId":"native-one","sessionName":"Legacy","agentType":"pi","cwd":root.path(),"firstRecordedAt":1,"originalWorkspaceId":"w-target"},
        {"sessionId":"native-two","sessionName":"Legacy","agentType":"pi","cwd":root.path(),"firstRecordedAt":2}
    ]).to_string()).unwrap();
    config
}
async fn read(server: &TestServer) -> Value {
    let response = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn save(server: &TestServer, id: &str, memo: &str) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), "PUT", "/api/sessions/history/memo",
        Some(&json!({"sessionId":id,"memo":memo}).to_string())).await
}
#[tokio::test]
async fn legacy_memo_is_optional_and_lan_browser_can_save_exact_text_and_clear_it() {
    let root = TempDir::new("memo legacy LAN");
    let herdr = HerdrFixture::new("memo legacy LAN");
    let config = config(&root, &herdr);
    let app = herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff));
    let server = TestServer::start_app(app.clone()).await;
    let before = read(&server).await;
    assert!(before["sessions"].as_array().unwrap().iter().all(|row| row.get("memo").is_none()));
    let lan = app.layer(MockConnectInfo("192.0.2.123:45678".parse::<std::net::SocketAddr>().unwrap()));
    let text = "  第一行\nsecond line\r\n  ";
    let response = lan.oneshot(Request::builder().method("PUT").uri("/api/sessions/history/memo")
        .header("content-type", "application/json")
        .body(Body::from(json!({"sessionId":"native-one","memo":text}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), 200);
    let result: Value = serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(result, json!({"sessionId":"native-one","memo":text}));
    drop(server);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let rows = read(&restarted).await;
    assert_eq!(rows["sessions"][1]["memo"], text);
    assert!(rows["sessions"][0].get("memo").is_none());
    assert_eq!(save(&restarted, "native-one", "").await.status, 200);
    assert_eq!(read(&restarted).await["sessions"][1]["memo"], "");
}
#[tokio::test]
async fn missing_identity_and_failed_writes_preserve_existing_history() {
    let root = TempDir::new("memo persistence failure");
    let herdr = HerdrFixture::new("memo persistence failure");
    let config = config(&root, &herdr);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    assert_eq!(save(&server, "native-one", "saved").await.status, 200);
    let before = read(&server).await;
    let missing = save(&server, "native-one-extra", "must not create").await;
    assert_eq!(missing.status, 404);
    assert_eq!(serde_json::from_str::<Value>(&missing.body).unwrap()["code"], "unknown-chat");
    assert_eq!(read(&server).await, before);
    let blocker = root.path().join("history.json.tmp");
    std::fs::create_dir(&blocker).unwrap();
    let failed = save(&server, "native-one", "unsaved draft\nretained by client").await;
    assert_eq!(failed.status, 500);
    assert_eq!(serde_json::from_str::<Value>(&failed.body).unwrap()["code"], "history-save-failed");
    assert_eq!(read(&server).await, before);
    std::fs::remove_dir(blocker).unwrap();
    assert_eq!(save(&server, "native-one", "retry").await.status, 200);
    std::fs::write(&config.sessions.history_file, b"broken history").unwrap();
    assert_eq!(save(&server, "native-one", "must not replace").await.status, 500);
    assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), b"broken history");
}
#[tokio::test]
async fn workspace_labels_use_one_inventory_and_fail_without_losing_saved_ids_or_memos() {
    let root = TempDir::new("history inventory labels");
    let herdr = HerdrFixture::new("history inventory labels");
    let config = config(&root, &herdr);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    assert_eq!(save(&server, "native-one", "note").await.status, 200);
    herdr.set_workspaces(json!([{"workspace_id":"w-target","label":"Renamed workspace"}]));
    let first = read(&server).await;
    assert_eq!(first["sessions"][1]["originalWorkspaceName"], "Renamed workspace");
    assert!(first["sessions"][0].get("originalWorkspaceName").is_none());
    assert_eq!(herdr.calls().iter().filter(|call| call["kind"] == "workspace_list").count(), 1);
    herdr.set_behavior(json!({"fail_workspace_list":true}));
    let degraded = read(&server).await;
    assert!(degraded["sessions"][1].get("originalWorkspaceName").is_none());
    assert_eq!(degraded["sessions"][1]["originalWorkspaceId"], "w-target");
    assert_eq!(degraded["sessions"][1]["memo"], "note");
    herdr.set_behavior(json!({}));
    herdr.set_workspaces(json!([]));
    assert_eq!(read(&server).await, degraded);
    std::fs::write(&config.sessions.history_file, b"[]").unwrap();
    let count = herdr.calls().len();
    assert_eq!(read(&server).await["total"], 0);
    assert_eq!(herdr.calls().len(), count, "empty pages never query Herdr");
}
