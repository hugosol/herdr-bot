//! Approved S2/S3 seams: HTTP, real persistent files and the production CLI adapter.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
struct NeverPowerOff;
impl herdr_bot::PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> { panic!("history must not power off") }
}
fn config(root: &TempDir, herdr: &HerdrFixture) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.herdr.executable = herdr.executable().to_owned();
    config.sessions.history_file = root.path().join("history.json");
    config.bots.storage_file = root.path().join("bots.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.bots.metadata_file = root.path().join("catalog.toml");
    let integration = root.path().join("required-pi.ts");
    std::fs::write(&integration, "// Controlled mandatory integration entrypoint").unwrap();
    std::fs::write(&config.bots.metadata_file, format!("skills = []\n[[agents]]\nid = 'pi'\nlabel = 'Pi'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'pi-state'\nlabel = 'Required integration'\nagent = 'pi'\nkind = 'fixed'\nentrypoint = '{}'\n", integration.display())).unwrap();
    config
}
async fn start(config: &herdr_bot::Config) -> TestServer {
    TestServer::start_app(herdr_bot::app_with_power_off(config, Arc::new(NeverPowerOff))).await
}
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
async fn register(server: &TestServer, root: &TempDir, id: &str, origin: Option<&str>) {
    post(server, "/api/sessions/events", json!({"sessionId":id,"kind":"register","data":{
        "cwd":root.path(),"sessionName":"Same name","agentType":"pi","originalWorkspaceId":origin
    }})).await;
}
fn native(herdr: &HerdrFixture, root: &TempDir, id: &str) {
    herdr.set_behavior(json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":id},"native_cwd":root.path()}));
}
async fn create_chat(server: &TestServer, zone: &str, bot: &str) {
    let preview = post(server, "/api/chats/preview", json!({"zone":zone,"botId":bot})).await;
    let accepted = post(server, "/api/chats", json!({"zone":zone,"botId":bot,"chatId":preview["chatId"],"expectedName":preview["name"],"input":"test"})).await;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let view = get(server, &format!("/api/chats/{}", accepted["id"].as_str().unwrap())).await;
        assert_ne!(view["stage"], "failed", "{view}");
        if view["stage"] == "succeeded" { return; }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
async fn wait_identities(server: &TestServer, expected: &[&str]) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let history = get(server, "/api/sessions/history").await;
        let mut ids = history["sessions"].as_array().unwrap().iter().map(|r| r["sessionId"].as_str().unwrap()).collect::<Vec<_>>();
        ids.sort_unstable();
        if ids == expected { return history; }
        assert!(tokio::time::Instant::now() < deadline, "expected literal identities {expected:?}, got {history}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
// P7/P21: normal Chat launch and connect collect the same full ID once, while
// two IDs sharing name and cwd remain distinct before/after deletion and restart.
#[tokio::test]
async fn normal_launch_and_connect_keep_literal_identity_set_and_original_location() {
    let root = TempDir::new("normal collection identities");
    let herdr = HerdrFixture::new("normal-collection-identities");
    let mut config = config(&root, &herdr);
    config.herdr.workspace_name = "other".into();
    let server = start(&config).await;
    let zones = post(&server, "/api/zones", json!({"path":root.path()})).await;
    let zone = zones["zones"][0]["path"].as_str().unwrap();
    let bots = post(&server, "/api/bots", json!({"name":"Same name","chatName":"Same name","agent":"pi","model":"default","thinking":"default","prompt":"","extensions":[]})).await;
    let bot = bots["bots"][0]["id"].as_str().unwrap();
    native(&herdr, &root, "native-full-001");
    create_chat(&server, zone, bot).await;
    let history = wait_identities(&server, &["native-full-001"]).await;
    assert_eq!(history["sessions"][0]["originalWorkspaceId"], "w-other");
    let memo = common::request_json_over_tcp(server.addr(), "PUT", "/api/sessions/history/memo",
        Some(&json!({"sessionId":"native-full-001","memo":"  shared note\n第二行  "}).to_string())).await;
    assert_eq!(memo.status, 200, "{}", memo.body);
    create_chat(&server, zone, bot).await;
    // Collection is asynchronous; this read waits for both confirmed agent_gets.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count() < 2 {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    register(&server, &root, "native-full-001", Some("w-target")).await;
    wait_identities(&server, &["native-full-001"]).await;
    native(&herdr, &root, "native-full-002");
    create_chat(&server, zone, bot).await;
    wait_identities(&server, &["native-full-001", "native-full-002"]).await;
    register(&server, &root, "native-full-002", None).await;
    for id in ["native-full-001", "native-full-002"] {
        post(&server, "/api/sessions/events", json!({"sessionId":id,"kind":"unregister"})).await;
    }
    let before = wait_identities(&server, &["native-full-001", "native-full-002"]).await;
    drop(server);
    config.herdr.workspace_name = "bot-playground".into();
    let server = start(&config).await;
    let after = wait_identities(&server, &["native-full-001", "native-full-002"]).await;
    assert_eq!(before, after);
    for row in after["sessions"].as_array().unwrap() {
        if row["sessionId"] == "native-full-001" {
            assert_eq!(row["memo"], "  shared note\n第二行  ");
        } else {
            assert!(row.get("memo").is_none());
        }
        assert_eq!(row["originalWorkspaceId"], "w-other");
        assert_eq!(row["sessionName"], "Same name");
    }
    let restored = post(&server, "/api/chats/resume", json!({"sessionId":"native-full-002"})).await;
    assert_eq!(restored["outcome"], "restored");
    assert_eq!(restored["fallback"], false);
    assert_eq!(herdr.tabs().last().unwrap()["workspace"], "w-other");
    assert_eq!(get(&server, "/api/sessions/history").await, after);
}

// P21: independently prewritten old-format bytes have no new metadata. Resume
// must work without rewriting or backfilling the historical document.
#[tokio::test]
async fn old_format_history_remains_readable_and_resumable_without_rewriting() {
    let root = TempDir::new("old history format");
    let herdr = HerdrFixture::new("old-history-format");
    let config = config(&root, &herdr);
    let old = format!(r#"[{{"sessionId":"legacy-native-001","sessionName":"Legacy","agentType":"pi","cwd":{},"sessionFile":null,"firstRecordedAt":123}}]"#, json!(root.path()));
    std::fs::write(&config.sessions.history_file, &old).unwrap();
    let server = start(&config).await;
    wait_identities(&server, &["legacy-native-001"]).await;
    native(&herdr, &root, "legacy-native-001");
    let result = post(&server, "/api/chats/resume", json!({"sessionId":"legacy-native-001"})).await;
    assert_eq!(result["outcome"], "restored");
    assert_eq!(result["fallback"], true);
    assert_eq!(herdr.tabs()[0]["workspace"], "w-target");
    drop(server);
    let server = start(&config).await;
    let history = wait_identities(&server, &["legacy-native-001"]).await;
    assert!(history["sessions"][0].get("originalWorkspaceId").is_none());
    assert_eq!(std::fs::read_to_string(&config.sessions.history_file).unwrap(), old);
}

// P7: locating a target in a different Workspace must not replace the original
// location or touch native content when the current registration is deleted.
#[tokio::test]
async fn locating_elsewhere_keeps_original_location_and_native_content() {
    let root = TempDir::new("history located elsewhere");
    let herdr = HerdrFixture::new("history-located-elsewhere");
    let config = config(&root, &herdr);
    let server = start(&config).await;
    let file = root.path().join("native.jsonl");
    let bytes = b"{\"type\":\"session\",\"id\":\"native-full-001\"}\n{\"type\":\"message\",\"content\":\"Historical content\"}\n";
    std::fs::write(&file, bytes).unwrap();
    post(&server, "/api/sessions/events", json!({"sessionId":"native-full-001","kind":"register","data":{"cwd":root.path(),"agentType":"pi","sessionFile":file,"originalWorkspaceId":"w-original"}})).await;
    herdr.set_behavior(json!({"agents":[{"agent":"pi","name":"live","workspace_id":"w-elsewhere","tab_id":"t-live","pane_id":"p-live","agent_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":"native-full-001"}}]}));
    let result = post(&server, "/api/chats/resume", json!({"sessionId":"native-full-001"})).await;
    assert_eq!(result["outcome"], "located");
    assert_eq!(result["target"]["workspaceId"], "w-elsewhere");
    assert!(herdr.tabs().is_empty());
    post(&server, "/api/sessions/events", json!({"sessionId":"native-full-001","kind":"unregister"})).await;
    drop(server);
    let server = start(&config).await;
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-original");
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}

// P9: an unavailable or malformed original inventory is not evidence of deletion.
// Observe the actual external command boundary: no fallback query, Tab or start.
#[tokio::test]
async fn original_workspace_query_failure_never_falls_back_or_starts() {
    for malformed in [false, true] {
        let root = TempDir::new("original workspace query failure");
        let herdr = HerdrFixture::new("original-query-failure");
        let config = config(&root, &herdr);
        let server = start(&config).await;
        register(&server, &root, "native-full-001", Some("w-original")).await;
        if malformed {
            herdr.set_workspaces(json!([{"label":"unknown ID"}]));
        } else {
            herdr.set_behavior(json!({"fail_workspace_list":true}));
        }
        let reply = post_json_over_tcp(server.addr(), "/api/chats/resume", r#"{"sessionId":"native-full-001"}"#).await;
        assert_eq!(reply.status, 502, "{}", reply.body);
        let result: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(result["outcome"], "failed");
        assert_eq!(result["stage"], "workspace");
        assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(), ["agent_list", "workspace_list"]);
        assert!(herdr.tabs().is_empty());
        assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-original");
    }
}

// P8/P7: a deleted ID must not match its former label or a focused decoy.
// The independent inventory leaves only w-target as the configured default.
#[tokio::test]
async fn deleted_original_falls_back_without_overwriting_history_even_after_restart() {
    let root = TempDir::new("deleted original workspace");
    let herdr = HerdrFixture::new("deleted-original-workspace");
    let config = config(&root, &herdr);
    let server = start(&config).await;
    register(&server, &root, "native-full-001", Some("w-deleted")).await;
    herdr.set_workspaces(json!([
        {"workspace_id":"w-target","label":"bot-playground"},
        {"workspace_id":"w-replacement","label":"Former original","focused":true}
    ]));
    native(&herdr, &root, "native-full-001");
    let result = post(&server, "/api/chats/resume", json!({"sessionId":"native-full-001"})).await;
    assert_eq!(result["outcome"], "restored");
    assert_eq!(result["fallback"], true);
    assert!(result["message"].as_str().unwrap().contains("原 Workspace 已删除"));
    assert_eq!(herdr.tabs()[0]["workspace"], "w-target");
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-deleted");
    drop(server);
    let server = start(&config).await;
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-deleted");
    assert_eq!(get(&server, "/api/sessions/history").await["total"], 1);
}

// P7/P21: expected original ID is an independent literal. A persistence omission
// or default-only implementation fails after a fresh application reads the file.
#[tokio::test]
async fn connect_history_preserves_original_workspace_across_restart_and_resumes_there() {
    let root = TempDir::new("history original workspace");
    let herdr = HerdrFixture::new("history-original-workspace");
    let config = config(&root, &herdr);
    let server = start(&config).await;
    register(&server, &root, "native-full-001", Some("w-other")).await;
    register(&server, &root, "native-full-001", Some("w-target")).await;
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-other");
    post(&server, "/api/sessions/events", json!({"sessionId":"native-full-001","kind":"unregister"})).await;
    drop(server);
    let server = start(&config).await;
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-other");
    herdr.set_workspaces(json!([
        {"workspace_id":"w-other","label":"Renamed original"},
        {"workspace_id":"w-same-label","label":"other"}
    ]));
    native(&herdr, &root, "native-full-001");
    let result = post(&server, "/api/chats/resume", json!({"sessionId":"native-full-001"})).await;
    assert_eq!(result["outcome"], "restored");
    assert_eq!(result["fallback"], false);
    assert_eq!(herdr.tabs()[0]["workspace"], "w-other");
    assert_eq!(get(&server, "/api/sessions/history").await["sessions"][0]["originalWorkspaceId"], "w-other");
}
