mod common;

use std::{path::Path, sync::Arc};
use common::{TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("test must never power off") }
}

async fn register(server: &TestServer, id: &str, name: &str) -> Value {
    let response = post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({
        "sessionId":id, "kind":"register", "data":{"cwd":"C:/literal/project", "sessionName":name, "agentType":"omp"}
    }).to_string()).await;
    assert_eq!(response.status, 200, "history failure must not reject accepted connect: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

struct ServerClock(std::sync::atomic::AtomicU64);
impl herdr_bot::clock::Clock for ServerClock {
    fn now(&self) -> std::time::Instant { std::time::Instant::now() }
    fn unix_millis(&self) -> u64 { self.0.load(std::sync::atomic::Ordering::SeqCst) }
}

// S2, P11/P20: failed attempts must not allocate a first time; a plausible bug
// stores a failed attempt in memory or replaces the first timestamp on re-register.
// Predetermined wall times and an independent restarted GET distinguish both.
#[tokio::test]
async fn explicit_register_recovers_new_and_existing_records_without_rewriting_first_time() {
    let root = TempDir::new("connect-history-time");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let clock = Arc::new(ServerClock(std::sync::atomic::AtomicU64::new(1_700_000_001_000)));
    let sessions = Arc::new(herdr_bot::sessions::SessionsState::with_policy_and_clock(config.sessions.policy(), clock.clone()));
    let server = TestServer::start_app(herdr_bot::app_with_sessions(&config, Arc::new(NeverPowerOff), sessions)).await;
    let blocked = common::HistoryWriteBlock::new(&config.sessions.history_file);
    assert_eq!(register(&server, "new-native-id", "Never saved").await["historySaved"], false);
    drop(blocked);
    clock.0.store(1_700_000_002_000, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(register(&server, "new-native-id", "First saved").await["historySaved"], true);
    let blocked = common::HistoryWriteBlock::new(&config.sessions.history_file);
    clock.0.store(1_700_000_003_000, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(register(&server, "new-native-id", "Rejected update").await["historySaved"], false);
    let saved: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(saved["sessions"][0]["sessionName"], "First saved");
    assert_eq!(saved["sessions"][0]["firstRecordedAt"], 1_700_000_002_000u64);
    drop(blocked);
    clock.0.store(1_700_000_004_000, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(register(&server, "new-native-id", "Recovered update").await["historySaved"], true);
    register(&server, "new-native-id", "Recovered update").await;
    drop(server);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let result: Value = serde_json::from_str(&get_over_tcp(restarted.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(result["sessions"], json!([{"sessionId":"new-native-id", "sessionName":"Recovered update", "agentType":"omp", "cwd":"C:/literal/project", "firstRecordedAt":1_700_000_002_000u64}]));
}

// S2, P21: neither an exclusive OS file lock nor invalid JSON is an empty
// collection. A read-error-to-empty fallback would overwrite the saved bytes on
// register; the HTTP errors, exact bytes and restarted read below detect it.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_and_invalid_storage_are_errors_and_never_overwritten() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = TempDir::new("connect-history-unreadable");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    register(&server, "preserved-native-id", "Exact original").await;
    let original = std::fs::read(&config.sessions.history_file).unwrap();
    let expected: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/sessions/history").await.body).unwrap();
    let locked = std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&config.sessions.history_file).unwrap();
    let response = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(response.status, 500);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], "history-unreadable");
    assert_eq!(register(&server, "preserved-native-id", "Must not replace").await["historySaved"], false);
    drop(locked);
    assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), original);
    let mut invalid_documents = vec![b"invalid history content\n".to_vec()];
    for legacy_source in [json!("opaque-original-id"), Value::Null] {
        let mut legacy: Value = serde_json::from_slice(&original).unwrap();
        legacy[0]["sourceBotId"] = legacy_source;
        // A new association alongside the retired key must not hide unmigrated data.
        legacy[0]["sourceTaskId"] = json!("opaque-original-id");
        invalid_documents.push(serde_json::to_vec(&legacy).unwrap());
    }
    for invalid in invalid_documents {
        std::fs::write(&config.sessions.history_file, &invalid).unwrap();
        let response = get_over_tcp(server.addr(), "/api/sessions/history").await;
        assert_eq!(response.status, 500);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], "history-unreadable");
        assert_eq!(register(&server, "unexpected-new-id", "Must not save").await["historySaved"], false);
        assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), invalid);
    }
    std::fs::write(&config.sessions.history_file, original).unwrap();
    drop(server);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let actual: Value = serde_json::from_str(&get_over_tcp(restarted.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(actual, expected);
}

// S2/P22: after the only register fails, restore writable storage and send no
// events. Advancing the runtime's timer boundary exposes scheduled retries: any
// such write would materialize history without the required explicit register.
#[tokio::test]
async fn failed_history_has_no_autonomous_write_retry() {
    let root = TempDir::new("connect-history-no-retry");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let blocked = common::HistoryWriteBlock::new(&config.sessions.history_file);
    assert_eq!(register(&server, "never-background-retry", "Not yet saved").await["historySaved"], false);
    drop(blocked);
    tokio::time::pause();
    for _ in 0..12 {
        tokio::time::advance(std::time::Duration::from_secs(3600)).await;
        tokio::task::yield_now().await;
        assert!(!config.sessions.history_file.exists(), "no timer may retry the failed write");
    }
    tokio::time::resume();
    assert_eq!(register(&server, "never-background-retry", "Explicit retry").await["historySaved"], true);
    let response: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(response["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(response["sessions"][0]["sessionName"], "Explicit retry");
}

// S2, P18/P19/P21: a readable destination that denies replacement makes a real write fail. A plausible
// violation is returning the ordinary success receipt, leaving the caller unable to warn.
// The independent expected receipt distinguishes registration from persistence.
#[tokio::test]
async fn accepted_connect_distinguishes_history_failure_and_preserves_previous_data() {
    let root = TempDir::new("connect-history-failure");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    assert_eq!(register(&server, "native-old-id", "Saved name").await["historySaved"], true);
    let before: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/sessions/history").await.body).unwrap();
    let blocked = common::HistoryWriteBlock::new(&config.sessions.history_file);
    let failed = register(&server, "native-old-id", "Failed name").await;
    assert_eq!(failed["outcome"], "registered");
    assert_eq!(failed["historySaved"], false);
    assert_eq!(failed["historyWarning"]["code"], "history-save-failed");
    assert!(failed["historyWarning"]["message"].as_str().is_some_and(|s| !s.is_empty()));
    drop(server);
    drop(blocked);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let after: Value = serde_json::from_str(&get_over_tcp(restarted.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(after, before, "independent restarted read must retain exact previous fields");
}
