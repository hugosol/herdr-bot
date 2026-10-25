//! Approved seams S2/S4: public HTTP and the external Herdr process.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, sync::Arc, time::Duration};
struct NeverPowerOff;
impl herdr_bot::PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> { panic!("history must not power off") }
}
async fn start(herdr: &HerdrFixture, root: &TempDir) -> TestServer {
    TestServer::start_app(herdr_bot::app_with_power_off(&configuration(herdr, root), Arc::new(NeverPowerOff))).await
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
fn configuration(herdr: &HerdrFixture, root: &TempDir) -> herdr_bot::Config {
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.shutdown.countdown_seconds = herdr_bot::CountdownSeconds::new(600).unwrap();
    config.herdr.executable = herdr.executable().to_path_buf();
    config.herdr.workspace_name = "bot-playground".into();
    config.zones.registry_file = root.path().join("zones.json");
    config.bots.storage_file = root.path().join("bots.json");
    config.sessions.history_file = root.path().join("history.json");
    config
}
async fn setup(herdr: &HerdrFixture, root: &TempDir) -> (TestServer, String, String) {
    let server = start(herdr, root).await;
    let zone = root.path().join("project");
    fs::create_dir_all(&zone).unwrap();
    let zones = post(&server, "/api/zones", json!({"path":zone})).await;
    let zone = zones["zones"][0]["path"].as_str().unwrap().to_owned();
    let bots = post(&server, "/api/bots", json!({
        "name":"Pi native history", "chatName":"Pi Zone Name", "agent":"pi",
        "model":"default","thinking":"default","skill":null,"prompt":"","extensions":[]
    })).await;
    let id = bots["bots"].as_array().unwrap().iter().find(|t| t["name"] == "Pi native history").unwrap()["id"].as_str().unwrap().to_owned();
    (server, zone, id)
}
async fn create(server: &TestServer, zone: &str, bot: &str) -> Value {
    let preview = post(server, "/api/chats/preview", json!({"zone":zone,"botId":bot})).await;
    let accepted = post(server, "/api/chats", json!({"zone":zone,"botId":bot,"chatId":preview["chatId"],"expectedName":preview["name"],"input":"/settings"})).await;
    let path = format!("/api/chats/{}", accepted["id"].as_str().unwrap());
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
    let attached = root.path().join("newest-decoy.records");
    fs::create_dir(&attached).unwrap();
    fs::write(attached.join("tool.bin"), b"\0old-pi-attached\xff\r\n").unwrap();
    fs::create_dir(attached.join("empty")).unwrap();
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}));
    let (server, zone, bot) = setup(&herdr, &root).await;
    let created = create(&server, &zone, &bot).await;
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
    assert_eq!(fs::read(attached.join("tool.bin")).unwrap(), b"\0old-pi-attached\xff\r\n");
    assert_eq!(fs::read_dir(&attached).unwrap().count(), 2);
    assert_eq!(fs::read_dir(attached.join("empty")).unwrap().count(), 0);
}

async fn wait_get(herdr: &HerdrFixture) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !herdr.calls().iter().any(|c| c["kind"] == "agent_get") {
        assert!(tokio::time::Instant::now() < deadline, "identity read never started");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// P30: header not yet written is transient only inside the total budget.
// Advance the production Tokio clock to 29s; a later exact header must still
// collect, without requiring or repeating initial input/model completion.
#[tokio::test]
async fn pi_exact_header_can_arrive_before_the_total_deadline() {
    let herdr = HerdrFixture::new("pi-late-header");
    let root = TempDir::new("pi-late-header");
    let native = root.path().join("exact-late.jsonl");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}));
    let (server, zone, bot) = setup(&herdr, &root).await;
    assert_eq!(create(&server, &zone, &bot).await["stage"], "succeeded");
    wait_get(&herdr).await;
    assert_eq!(get(&server, "/api/sessions/history").await["total"], 0);
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(29)).await;
    fs::write(&native, b"{\"type\":\"session\",\"id\":\"pi-header-at-29s\",\"cwd\":\"C:/late/exact\"}\n").unwrap();
    tokio::time::resume();
    assert_eq!(wait_history(&server, "pi-header-at-29s").await["cwd"], "C:/late/exact");
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 1);
}

// P22/P30: missing references, absent exact headers, and a still-running
// external query all expire at the same 30s budget. Providing a valid identity
// after expiry must not restart reads, save, or resubmit/stop the agent.
#[tokio::test]
async fn pi_identity_budget_ends_reads_and_never_collects_late_identity() {
    for failure in ["missing", "header-unreadable", "query-pending"] {
        let herdr = HerdrFixture::new(failure);
        let root = TempDir::new(failure);
        let native = root.path().join("late.jsonl");
        let release = root.path().join("release-query");
        let behavior = match failure {
            "header-unreadable" => json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}),
            "query-pending" => json!({"native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}),
            _ => json!({}),
        };
        herdr.set_behavior(behavior);
        let (server, zone, bot) = setup(&herdr, &root).await;
        let created = create(&server, &zone, &bot).await;
        assert_eq!(created["stage"], "succeeded");
        wait_get(&herdr).await;
        post(&server, "/api/sessions/events", json!({"sessionId":"current-timeout-control","kind":"register","data":{"cwd":"C:/current"}})).await;
        post(&server, "/api/sessions/events", json!({"sessionId":"current-timeout-control","kind":"state","data":{"value":"working"}})).await;
        post(&server, "/api/shutdown/monitor", json!({"sessionId":"current-timeout-control","selected":true})).await;
        let mut before = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
        let before = before.next_frame().await.json();
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::resume();
        let reads_at_expiry = herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count();
        fs::write(&native, b"{\"type\":\"session\",\"id\":\"pi-too-late\",\"cwd\":\"C:/late\"}\n").unwrap();
        herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}));
        fs::write(release, b"release").unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(get(&server, "/api/sessions/history").await["total"], 1, "{failure}");
        let mut after = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
        let after = after.next_frame().await.json();
        assert_eq!(after["sessions"], before["sessions"]);
        assert_eq!(after["shutdown"]["monitoredSessionIds"], json!(["current-timeout-control"]));
        assert_eq!(after["shutdown"]["phase"], "none");
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count(), reads_at_expiry, "{failure}: reads after deadline");
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 1);
        let view = get(&server, &format!("/api/chats/{}", created["id"].as_str().unwrap())).await;
        assert_eq!(view, created);
    }
}

// P17/P26: while the first exact query is unresolved, another creation,
// register and a fresh SSE snapshot must advance. Collection cannot change
// keepAlive, monitoring or countdown intent or establish current registration.
#[tokio::test(flavor="multi_thread")]
async fn pi_pending_identity_does_not_hold_creation_or_current_state() {
    let herdr = HerdrFixture::new("pi-independent");
    let root = TempDir::new("pi-independent");
    let release = root.path().join("release");
    herdr.set_behavior(json!({"native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}));
    let (server, zone, bot) = setup(&herdr, &root).await;
    let first = create(&server, &zone, &bot).await;
    assert_eq!(first["stage"], "succeeded");
    wait_get(&herdr).await;
    let first_agent = first["agentName"].as_str().unwrap();
    herdr.set_behavior(json!({"native_session_gate":{"match":first_agent,"wait_for_file":release,"timeout_ms":60000},"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-second-independent"},"native_cwd":"C:/second"}));
    assert_eq!(create(&server, &zone, &bot).await["stage"], "succeeded");
    wait_history(&server, "pi-second-independent").await;
    post(&server, "/api/sessions/events", json!({"sessionId":"current-untouched","kind":"register","data":{"cwd":"C:/current","agentType":"pi"}})).await;
    post(&server, "/api/sessions/events", json!({"sessionId":"current-untouched","kind":"state","data":{"value":"working"}})).await;
    post(&server, "/api/shutdown/monitor", json!({"sessionId":"current-untouched","selected":true})).await;
    let mut observation = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let before = observation.next_frame().await.json();
    assert_eq!(before["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(before["sessions"][0]["online"], true);
    assert!(!release.exists());
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-first-independent"},"native_cwd":"C:/first"}));
    fs::write(release, b"release").unwrap();
    wait_history(&server, "pi-first-independent").await;
    let mut after = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let after = after.next_frame().await.json();
    assert_eq!(after["sessions"], before["sessions"]);
    assert_eq!(after["shutdown"]["monitoredSessionIds"], json!(["current-untouched"]));
    assert_eq!(after["shutdown"]["phase"], "none");
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 2);
}

// P22/P26/P30: a real write failure must be terminal for this collection,
// independent of the current member's keepAlive and armed completion watcher.
// Restoring storage without another event exposes any hidden save retry.
#[tokio::test(flavor="multi_thread")]
async fn pi_save_failure_does_not_retry_or_change_current_intent() {
    let herdr = HerdrFixture::new("pi-save-failure");
    let root = TempDir::new("pi-save-failure");
    let release = root.path().join("release");
    herdr.set_behavior(json!({"native_session_gate":{"wait_for_file":release,"timeout_ms":60000},"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-save-once"},"native_cwd":"C:/save-once"}));
    let (server, zone, bot) = setup(&herdr, &root).await;
    post(&server, "/api/sessions/events", json!({"sessionId":"current-save-control","kind":"register","data":{"cwd":"C:/current","agentType":"pi"}})).await;
    post(&server, "/api/sessions/events", json!({"sessionId":"current-save-control","kind":"state","data":{"value":"working"}})).await;
    post(&server, "/api/shutdown/monitor", json!({"sessionId":"current-save-control","selected":true})).await;
    let mut before = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let before = before.next_frame().await.json();
    let blocker = root.path().join("history.json.tmp");
    fs::create_dir(&blocker).unwrap();
    assert_eq!(create(&server, &zone, &bot).await["stage"], "succeeded");
    wait_get(&herdr).await;
    fs::write(release, b"release").unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(get(&server, "/api/sessions/history").await["total"], 1);
    fs::remove_dir(blocker).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let history = get(&server, "/api/sessions/history").await;
    assert_eq!(history["total"], 1);
    assert_eq!(history["sessions"][0]["sessionId"], "current-save-control");
    let mut after = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let after = after.next_frame().await.json();
    assert_eq!(after["sessions"], before["sessions"]);
    assert_eq!(after["shutdown"]["monitoredSessionIds"], json!(["current-save-control"]));
    assert_eq!(after["shutdown"]["phase"], "none");
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count(), 1);
    drop(server);
    let server = start(&herdr, &root).await;
    let mut restarted = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let restarted = restarted.next_frame().await.json();
    assert_eq!(restarted["sessions"], json!([]));
    assert_eq!(restarted["shutdown"]["monitoredSessionIds"], json!([]));
    assert_eq!(restarted["shutdown"]["phase"], "none");
    assert_eq!(get(&server, "/api/sessions/history").await["total"], 1);
}

// P26: an already-running countdown keeps the same arrangement/deadline
// across successful collection, missing identity and a real save failure.
#[tokio::test]
async fn pi_history_outcomes_never_cancel_or_reschedule_an_existing_countdown() {
    for outcome in ["success", "timeout", "save-failed"] {
        let herdr = HerdrFixture::new(outcome);
        let root = TempDir::new(outcome);
        let release = root.path().join("release");
        herdr.set_behavior(json!({"native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}));
        let (server, zone, bot) = setup(&herdr, &root).await;
        let started = common::post_over_tcp(server.addr(), "/api/shutdown/start").await;
        assert!((200..300).contains(&started.status), "{}", started.body);
        let mut before = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
        let before = before.next_frame().await.json();
        assert_eq!(before["shutdown"]["phase"], "counting-down");
        if outcome == "save-failed" { fs::create_dir(root.path().join("history.json.tmp")).unwrap(); }
        assert_eq!(create(&server, &zone, &bot).await["stage"], "succeeded");
        wait_get(&herdr).await;
        if outcome == "timeout" {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(30)).await;
            tokio::task::yield_now().await;
            tokio::time::resume();
        } else {
            herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-countdown-outcome"},"native_cwd":"C:/countdown"}));
            fs::write(&release, b"release").unwrap();
            if outcome == "success" { wait_history(&server, "pi-countdown-outcome").await; }
            else { tokio::time::sleep(Duration::from_millis(400)).await; }
        }
        let mut after = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
        let after = after.next_frame().await.json();
        for field in ["phase", "arrangementId", "deadline", "monitoredSessionIds"] {
            assert_eq!(after["shutdown"][field], before["shutdown"][field], "{outcome}: {field}");
        }
        assert_eq!(after["sessions"], json!([]));
        let cancelled = common::post_over_tcp(server.addr(), "/api/shutdown/cancel").await;
        assert!((200..300).contains(&cancelled.status), "{}", cancelled.body);
    }
}
// P02: startup and submit rejection must not collect a zone record. A later
// explicit register is independently allowed; same ID is not blacklisted.
#[tokio::test(flavor="multi_thread")]
async fn pi_unconfirmed_start_and_submit_never_collect_but_register_still_can() {
    for failure in ["fail_agent_start", "fail_initial_message"] {
        let herdr = HerdrFixture::new(failure);
        let root = TempDir::new(failure);
        let mut behavior = json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-unconfirmed-id"},"native_cwd":"C:/native/unconfirmed"});
        behavior[failure] = json!(true);
        herdr.set_behavior(behavior);
        let (server, zone, bot) = setup(&herdr, &root).await;
        assert_eq!(create(&server, &zone, &bot).await["stage"], "failed");
        assert_eq!(get(&server, "/api/sessions/history").await["total"], 0);
        assert!(!herdr.calls().iter().any(|c| c["kind"] == "agent_get"));
        post(&server, "/api/sessions/events", json!({"sessionId":"pi-unconfirmed-id","kind":"register","data":{"cwd":"C:/native/unconfirmed","sessionName":"Accepted connect","agentType":"pi"}})).await;
        assert_eq!(wait_history(&server, "pi-unconfirmed-id").await["sessionName"], "Accepted connect");
        assert_eq!(get(&server, "/api/sessions/history").await["total"], 1);
    }
}

// P04/P09: connect may precede or follow zone collection. The independent
// expected name is the Chat name, not whichever metadata arrived last.
#[tokio::test(flavor="multi_thread")]
async fn pi_zone_name_wins_chath_source_orders_and_survives_restart() {
    let herdr = HerdrFixture::new("pi-name-priority");
    let root = TempDir::new("pi-name-priority-storage");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-priority-id"},"native_cwd":"C:/native/priority"}));
    let (server, zone, bot) = setup(&herdr, &root).await;
    post(&server, "/api/sessions/events", json!({"sessionId":"pi-priority-id","kind":"register","data":{"cwd":"C:/connect/first","sessionName":"Connect first"}})).await;
    assert_eq!(create(&server, &zone, &bot).await["stage"], "succeeded");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if wait_history(&server, "pi-priority-id").await["sessionName"] == "Pi Zone Name" { break; }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    post(&server, "/api/sessions/events", json!({"sessionId":"pi-priority-id","kind":"register","data":{"cwd":"C:/connect/second","sessionName":"Connect later"}})).await;
    assert_eq!(wait_history(&server, "pi-priority-id").await["sessionName"], "Pi Zone Name");
    drop(server);
    let server = start(&herdr, &root).await;
    post(&server, "/api/sessions/events", json!({"sessionId":"pi-priority-id","kind":"register","data":{"cwd":"C:/connect/third","sessionName":"Connect after restart"}})).await;
    let history = get(&server, "/api/sessions/history").await;
    assert_eq!(history["total"], 1);
    assert_eq!(history["sessions"][0]["sessionName"], "Pi Zone Name");
}

