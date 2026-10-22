//! Approved seams S2/S4: public HTTP and the external Herdr process.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, time::Duration};

async fn get(server: &TestServer, path: &str) -> Value {
    let response = get_over_tcp(server.addr(), path).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn post(server: &TestServer, path: &str, body: Value) -> Value {
    let response = post_json_over_tcp(server.addr(), path, &body.to_string()).await;
    assert!((200..300).contains(&response.status), "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn setup(herdr: &HerdrFixture, root: &TempDir) -> (TestServer, String, String) {
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.herdr.executable = herdr.executable().to_path_buf();
    config.herdr.workspace_name = "bot-playground".into();
    config.zones.registry_file = root.path().join("zones.json");
    config.bot_templates.storage_file = root.path().join("templates.json");
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start(&config).await;
    let zone = root.path().join("project");
    fs::create_dir_all(&zone).unwrap();
    let zones = post(&server, "/api/zones", json!({"path":zone})).await;
    let zone = zones["zones"][0]["path"].as_str().unwrap().to_owned();
    let templates = post(&server, "/api/bot-templates", json!({
        "name":"Pi native history", "botName":"Pi Zone Name", "agent":"pi",
        "model":"default","thinking":"default","skill":null,"prompt":"","extensions":[]
    })).await;
    let id = templates["templates"].as_array().unwrap().iter().find(|t| t["name"] == "Pi native history").unwrap()["id"].as_str().unwrap().to_owned();
    (server, zone, id)
}
async fn create(server: &TestServer, zone: &str, template: &str) -> Value {
    let preview = post(server, "/api/bots/preview", json!({"zone":zone,"templateId":template})).await;
    let accepted = post(server, "/api/bots", json!({"zone":zone,"templateId":template,"botId":preview["botId"],"expectedName":preview["name"],"input":"/settings"})).await;
    let path = format!("/api/bots/{}", accepted["id"].as_str().unwrap());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let view = get(server, &path).await;
        if view["stage"] == "succeeded" || view["stage"] == "failed" { return view; }
        assert!(tokio::time::Instant::now() < deadline, "creation did not finish: {view}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}
async fn wait_history(server: &TestServer, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let history = get(server, "/api/sessions/history").await;
        if let Some(row) = history["sessions"].as_array().unwrap().iter().find(|r| r["sessionId"] == id) { return row.clone(); }
        assert!(tokio::time::Instant::now() < deadline, "missing exact native id {id}: {history}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

// P01/P05/P17/P25: contract says the exact created native conversation is
// collected without connect. A connect-only implementation or latest-file guess
// misses this independently specified ID. A blank prefix and malformed body
// ensure only header metadata is needed, not a first-line assumption/body import.
#[tokio::test(flavor="multi_thread")]
async fn pi_without_connect_collects_exact_header_and_leaves_current_and_old_files_untouched() {
    let herdr = HerdrFixture::new("pi-history");
    let root = TempDir::new("pi-history-storage");
    let native = root.path().join("exact-native.jsonl");
    let bytes = b"\n{\"type\":\"session\",\"id\":\"pi-native-full-001\",\"cwd\":\"C:/native/actual\"}\nnot session JSON body\n";
    fs::write(&native, bytes).unwrap();
    let old = root.path().join("newest-decoy.jsonl");
    fs::write(&old, b"{\"type\":\"session\",\"id\":\"wrong-decoy\"}\n").unwrap();
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}));
    let (server, zone, template) = setup(&herdr, &root).await;
    let created = create(&server, &zone, &template).await;
    assert_eq!(created["stage"], "succeeded");
    let row = wait_history(&server, "pi-native-full-001").await;
    assert_eq!(row["cwd"], "C:/native/actual");
    assert_eq!(row["sessionName"], "Pi Zone Name");
    assert_eq!(row["agentType"], "pi");
    assert_eq!(get(&server, "/api/sessions/history").await["total"], 1);
    let mut observation = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    assert_eq!(observation.next_frame().await.json()["sessions"], json!([]));
    assert_eq!(fs::read(native).unwrap(), bytes);
    assert_eq!(fs::read(old).unwrap(), b"{\"type\":\"session\",\"id\":\"wrong-decoy\"}\n");
}
