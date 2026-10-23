//! Approved S2/S4: real HTTP router and external Herdr process; no private history seam.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::Arc, time::Duration};
use herdr_bot::{PowerOff, PowerOffFailure};
struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("history must never power off") }
}
async fn read(server: &TestServer, path: &str) -> Value {
    let response = get_over_tcp(server.addr(), path).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn current(server: &TestServer) -> Value {
    let mut stream = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    stream.next_event("snapshot").await.json()
}
async fn post(server: &TestServer, path: &str, body: Value) -> Value {
    let response = post_json_over_tcp(server.addr(), path, &body.to_string()).await;
    assert!([200, 202].contains(&response.status), "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
fn configuration(root: &TempDir, herdr: &HerdrFixture) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.zones.registry_file = root.path().join("zones.json");
    config.bots.storage_file = root.path().join("bots.json");
    config.sessions.history_file = root.path().join("history.json");
    config.herdr.workspace_name = "bot-playground".into();
    config.herdr.executable = herdr.executable().to_path_buf();
    config
}
async fn setup(server: &TestServer, root: &TempDir) -> (String, String) {
    let zone = root.path().join("zone");
    fs::create_dir_all(&zone).unwrap();
    let zones = post(server, "/api/zones", json!({"path":zone})).await;
    let zone = zones["zones"][0]["path"].as_str().unwrap().to_owned();
    let bots = post(server, "/api/bots", json!({"name":"OMP history", "chatName":"OMP exact {chat.id}", "agent":"omp", "model":"default", "thinking":"default", "skill":null, "prompt":"", "extensions":[]})).await;
    (zone, bots["bots"][0]["id"].as_str().unwrap().to_owned())
}
async fn create(server: &TestServer, zone: &str, bot: &str) -> Value {
    let preview = post(server, "/api/chats/preview", json!({"zone":zone, "botId":bot})).await;
    post(server, "/api/chats", json!({"zone":zone,"botId":bot,"chatId":preview["chatId"],"input":"/settings"})).await
}
async fn terminal(server: &TestServer, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let view = read(server, &format!("/api/chats/{id}")).await;
            if view["stage"] == "succeeded" || view["stage"] == "failed" { return view; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("creation reaches terminal stage")
}
async fn history_count(server: &TestServer, count: usize) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let result = read(server, "/api/sessions/history").await;
            if result["sessions"].as_array().unwrap().len() == count { return result; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("independent history read reaches exact set")
}
// P01/P05/P25: the independent literal is the second-line native header, not
// creation/tab/alias or the newest file. Reading only physical line one (title),
// importing body or using the decoy ID would violate the criterion and fail here.
#[tokio::test(flavor="multi_thread")]
async fn omp_without_connect_records_exact_header_after_title_without_changing_native_files() {
    let root = TempDir::new("omp-zone-history");
    let herdr = HerdrFixture::new("omp-zone-history-herdr");
    let file = root.path().join("native.jsonl");
    let native = b"{\"type\":\"title\",\"title\":\"title before header\"}\n{\"type\":\"session\",\"version\":3,\"id\":\"omp-exact-full-native-id-001\",\"cwd\":\"C:/native/actual-cwd\"}\nnot JSON conversation body must not be read\n";
    fs::write(&file, native).unwrap();
    let old = root.path().join("newer-decoy.jsonl");
    fs::write(&old, b"{\"type\":\"session\",\"id\":\"do-not-import\",\"cwd\":\"C:/decoy\"}\n").unwrap();
    let sidecar = root.path().join("newer-decoy.records");
    fs::create_dir_all(sidecar.join("empty")).unwrap();
    fs::write(sidecar.join("tool.bin"),b"\0old-attached-record\xff\r\n").unwrap();
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":file}}));
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&configuration(&root,&herdr), Arc::new(NeverPowerOff))).await;
    let (zone, bot) = setup(&server,&root).await;
    let accepted = create(&server,&zone,&bot).await;
    let done = terminal(&server, accepted["id"].as_str().unwrap()).await;
    assert_eq!(done["stage"],"succeeded");
    let history = history_count(&server,1).await;
    assert_eq!(history["sessions"][0]["sessionId"],"omp-exact-full-native-id-001");
    assert_eq!(history["sessions"][0]["cwd"],"C:/native/actual-cwd");
    assert_eq!(history["sessions"][0]["sessionName"],done["name"]);
    assert_eq!(history["sessions"][0]["agentType"],"omp");
    assert_eq!(current(&server).await["sessions"], json!([]));
    assert_eq!(fs::read(&file).unwrap(),native);
    assert_eq!(fs::read(&old).unwrap(),b"{\"type\":\"session\",\"id\":\"do-not-import\",\"cwd\":\"C:/decoy\"}\n");
    assert_eq!(fs::read(sidecar.join("tool.bin")).unwrap(),b"\0old-attached-record\xff\r\n");
    assert!(sidecar.join("empty").is_dir());
}

// P02: rejected start or unacknowledged initial submission must not contribute
// a zone name. A later independent register of the same ID remains eligible.
#[tokio::test(flavor="multi_thread")]
async fn failed_omp_start_and_submission_do_not_record_zone_but_register_can_record_same_identity() {
    for failure in ["fail_agent_start", "fail_prompt"] {
        let root = TempDir::new("omp-failed-create-history");
        let herdr = HerdrFixture::new("omp-failed-create-herdr");
        let mut behavior = json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"omp-rejected-native-id"},"native_cwd":"C:/native"});
        behavior[failure] = json!(true);
        herdr.set_behavior(behavior);
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&configuration(&root,&herdr), Arc::new(NeverPowerOff))).await;
        let (zone,bot) = setup(&server,&root).await;
        let accepted = create(&server,&zone,&bot).await;
        assert_eq!(terminal(&server,accepted["id"].as_str().unwrap()).await["stage"],"failed");
        assert_eq!(read(&server,"/api/sessions/history").await["total"],0);
        assert!(!herdr.calls().iter().any(|c|c["kind"]=="agent_get"));
        post(&server,"/api/sessions/events",json!({"sessionId":"omp-rejected-native-id","kind":"register","data":{"cwd":"C:/accepted","sessionName":"Independent connect","agentType":"omp"}})).await;
        let history = history_count(&server,1).await;
        assert_eq!(history["sessions"][0]["sessionId"],"omp-rejected-native-id");
        assert_eq!(history["sessions"][0]["sessionName"],"Independent connect");
    }
}

// P17/P26/P30: hold the external identity read unfinished, not merely slow.
// Creation success and a second creation/register/current snapshot must still
// advance. No lock, UI connection or model completion may own this task.
#[tokio::test(flavor="multi_thread")]
async fn pending_omp_identity_does_not_hold_creation_registration_or_current_state() {
    let root = TempDir::new("omp-detached-history");
    let herdr = HerdrFixture::new("omp-detached-herdr");
    let release = root.path().join("release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"omp-pending-exact-id"},"native_cwd":"C:/pending/native", "native_session_gate":{"wait_for_file":release,"timeout_ms":40000}}));
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&configuration(&root,&herdr), Arc::new(NeverPowerOff))).await;
    let (zone, bot) = setup(&server,&root).await;
    let accepted = create(&server,&zone,&bot).await;
    let first = terminal(&server,accepted["id"].as_str().unwrap()).await;
    assert_eq!(first["stage"],"succeeded");
    tokio::time::timeout(Duration::from_secs(3), async {
        while !herdr.calls().iter().any(|c| c["kind"] == "agent_get") { tokio::time::sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
    assert!(!release.exists());
    assert_eq!(read(&server,"/api/sessions/history").await["total"],0);
    let second = create(&server,&zone,&bot).await;
    assert_eq!(terminal(&server,second["id"].as_str().unwrap()).await["stage"],"succeeded");
    post(&server,"/api/sessions/events",json!({"sessionId":"unrelated-current","kind":"register","data":{"cwd":"C:/current","sessionName":"Unrelated", "agentType":"omp"}})).await;
    post(&server,"/api/sessions/events",json!({"sessionId":"unrelated-current","kind":"state","data":{"value":"working"}})).await;
    post(&server,"/api/shutdown/monitor",json!({"sessionId":"unrelated-current","selected":true})).await;
    let snapshot = current(&server).await;
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"],json!(["unrelated-current"]));
    assert_eq!(snapshot["shutdown"]["phase"],"none");
    assert_eq!(snapshot["sessions"].as_array().unwrap().len(),1);
    assert_eq!(snapshot["sessions"][0]["sessionId"],"unrelated-current");
    assert!(!release.exists());
    fs::write(&release,b"ready").unwrap();
    let history = history_count(&server,2).await;
    let pending = history["sessions"].as_array().unwrap().iter().find(|r| r["sessionId"]=="omp-pending-exact-id").unwrap();
    assert_eq!(pending["cwd"],"C:/pending/native");
    assert!(pending["sessionName"].as_str().unwrap().starts_with("OMP exact "));
    let after = current(&server).await;
    assert_eq!(after["sessions"],snapshot["sessions"]);
    assert_eq!(after["shutdown"]["phase"],snapshot["shutdown"]["phase"]);
    assert_eq!(after["shutdown"]["monitoredSessionIds"],snapshot["shutdown"]["monitoredSessionIds"]);
}

// P30: the source of the 29s/30s boundary is the approved contract. The real
// external process/filesystem remain in-path; only the production Tokio clock
// advances. An identity getter with a fresh 30s budget on every retry fails this.
#[tokio::test]
async fn omp_header_at_29_seconds_is_recorded_before_the_total_budget_expires() {
    let root = TempDir::new("omp-header-29s");
    let herdr = HerdrFixture::new("omp-header-29s-herdr");
    let native = root.path().join("late.jsonl");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":native}}));
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&configuration(&root,&herdr),Arc::new(NeverPowerOff))).await;
    let (zone,bot) = setup(&server,&root).await;
    let accepted = create(&server,&zone,&bot).await;
    assert_eq!(terminal(&server,accepted["id"].as_str().unwrap()).await["stage"],"succeeded");
    wait_get(&herdr).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(29)).await;
    fs::write(native,b"{\"type\":\"title\",\"title\":\"metadata first\"}\n{\"type\":\"session\",\"id\":\"omp-header-at-29-seconds\",\"cwd\":\"C:/late/native\"}\n").unwrap();
    tokio::time::resume();
    let history = history_count(&server,1).await;
    assert_eq!(history["sessions"][0]["sessionId"],"omp-header-at-29-seconds");
    assert_eq!(history["sessions"][0]["cwd"],"C:/late/native");
}

async fn wait_get(herdr: &HerdrFixture) {
    tokio::time::timeout(Duration::from_secs(3),async {
        while !herdr.calls().iter().any(|c|c["kind"]=="agent_get") { tokio::time::sleep(Duration::from_millis(10)).await; }
    }).await.unwrap();
}

// P22/P26/P30: restoration after expiry must not initiate reads, writes, connect,
// resubmission or agent shutdown. Current keepAlive and armed watcher are a
// separate authority and must be byte-for-byte semantically unchanged.
#[tokio::test]
async fn omp_total_deadline_ends_missing_unreadable_and_pending_identity_without_changing_current_intent() {
    for failure in ["missing", "header", "pending-query"] {
        let root = TempDir::new("omp-identity-budget");
        let herdr = HerdrFixture::new("omp-identity-budget-herdr");
        let native = root.path().join("late.jsonl");
        let release = root.path().join("release");
        let behavior = match failure {
            "header" => json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":native}}),
            "pending-query" => json!({"native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}),
            _ => json!({}),
        };
        herdr.set_behavior(behavior);
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&configuration(&root,&herdr),Arc::new(NeverPowerOff))).await;
        let (zone,bot) = setup(&server,&root).await;
        let accepted = create(&server,&zone,&bot).await;
        let done = terminal(&server,accepted["id"].as_str().unwrap()).await;
        assert_eq!(done["stage"],"succeeded");
        wait_get(&herdr).await;
        post(&server,"/api/sessions/events",json!({"sessionId":"omp-current-budget-control","kind":"register","data":{"cwd":"C:/current","agentType":"omp"}})).await;
        post(&server,"/api/sessions/events",json!({"sessionId":"omp-current-budget-control","kind":"state","data":{"value":"working"}})).await;
        post(&server,"/api/shutdown/monitor",json!({"sessionId":"omp-current-budget-control","selected":true})).await;
        let before = current(&server).await;
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        tokio::time::resume();
        let reads = herdr.calls().iter().filter(|c|c["kind"]=="agent_get").count();
        fs::write(&native,b"{\"type\":\"session\",\"id\":\"too-late-omp\",\"cwd\":\"C:/late\"}\n").unwrap();
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":native}}));
        fs::write(release,b"release").unwrap();
        tokio::time::sleep(Duration::from_millis(350)).await;
        let history = read(&server,"/api/sessions/history").await;
        assert_eq!(history["total"],1,"{failure}");
        assert_eq!(history["sessions"][0]["sessionId"],"omp-current-budget-control");
        assert_eq!(herdr.calls().iter().filter(|c|c["kind"]=="agent_get").count(),reads,"{failure}: no reads after deadline");
        assert_eq!(herdr.calls().iter().filter(|c|c["kind"]=="agent_prompt").count(),1);
        assert_eq!(read(&server,&format!("/api/chats/{}",done["id"].as_str().unwrap())).await,done);
        let after = current(&server).await;
        assert_eq!(after["sessions"],before["sessions"]);
        assert_eq!(after["shutdown"]["monitoredSessionIds"],json!(["omp-current-budget-control"]));
        assert_eq!(after["shutdown"]["phase"],"none");
    }
}

// P22/P26/P30: real isolated filesystem failure followed by restoration with no
// new event exposes background save retry. Restart checks history/current split.
#[tokio::test]
async fn omp_save_failure_is_one_shot_and_does_not_persist_current_shutdown_intent() {
    let root = TempDir::new("omp-one-shot-save");
    let herdr = HerdrFixture::new("omp-one-shot-save-herdr");
    let release = root.path().join("release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"omp-not-saved"},"native_cwd":"C:/native","native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}));
    let config = configuration(&root,&herdr);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config,Arc::new(NeverPowerOff))).await;
    let (zone,bot) = setup(&server,&root).await;
    post(&server,"/api/sessions/events",json!({"sessionId":"omp-current-save-control","kind":"register","data":{"cwd":"C:/current","agentType":"omp"}})).await;
    post(&server,"/api/sessions/events",json!({"sessionId":"omp-current-save-control","kind":"state","data":{"value":"working"}})).await;
    post(&server,"/api/shutdown/monitor",json!({"sessionId":"omp-current-save-control","selected":true})).await;
    let before = current(&server).await;
    let blocker = root.path().join("history.json.tmp");
    fs::create_dir(&blocker).unwrap();
    let accepted = create(&server,&zone,&bot).await;
    let done = terminal(&server,accepted["id"].as_str().unwrap()).await;
    assert_eq!(done["stage"],"succeeded");
    wait_get(&herdr).await;
    fs::write(release,b"release").unwrap();
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert_eq!(read(&server,"/api/sessions/history").await["total"],1);
    fs::remove_dir(blocker).unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(3600)).await;
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    tokio::time::resume();
    assert_eq!(read(&server,"/api/sessions/history").await["total"],1);
    assert_eq!(herdr.calls().iter().filter(|c|c["kind"]=="agent_get").count(),1);
    let after = current(&server).await;
    assert_eq!(after["sessions"],before["sessions"]);
    assert_eq!(after["shutdown"]["monitoredSessionIds"],json!(["omp-current-save-control"]));
    assert_eq!(after["shutdown"]["phase"],"none");
    drop(server);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config,Arc::new(NeverPowerOff))).await;
    let snapshot = current(&restarted).await;
    assert_eq!(snapshot["sessions"],json!([]));
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"],json!([]));
    assert_eq!(snapshot["shutdown"]["phase"],"none");
    assert_eq!(read(&restarted,"/api/sessions/history").await["total"],1);
}

// P26: history success, identity failure and save failure must not cancel,
// restart or retime an already-existing countdown. A panic-only power adapter
// and one-hour countdown make real shutdown impossible in this isolated test.
#[tokio::test]
async fn omp_history_outcomes_do_not_retime_or_persist_an_existing_countdown() {
    for outcome in ["success", "identity-timeout", "save-failure"] {
        let root = TempDir::new("omp-existing-countdown");
        let herdr = HerdrFixture::new("omp-existing-countdown-herdr");
        let release = root.path().join("release");
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"omp-countdown-history"},"native_cwd":"C:/native","native_session_gate":{"wait_for_file":release,"timeout_ms":60000}}));
        let mut config = configuration(&root,&herdr);
        config.shutdown.countdown_seconds = herdr_bot::CountdownSeconds::new(3600).unwrap();
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&config,Arc::new(NeverPowerOff))).await;
        let (zone,bot) = setup(&server,&root).await;
        let started = common::post_over_tcp(server.addr(),"/api/shutdown/start").await;
        assert_eq!(started.status,200,"{}",started.body);
        let before = current(&server).await;
        assert_eq!(before["shutdown"]["phase"],"counting-down");
        let accepted = create(&server,&zone,&bot).await;
        assert_eq!(terminal(&server,accepted["id"].as_str().unwrap()).await["stage"],"succeeded");
        wait_get(&herdr).await;
        if outcome == "identity-timeout" {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(30)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            tokio::time::resume();
        } else {
            if outcome == "save-failure" { fs::create_dir(root.path().join("history.json.tmp")).unwrap(); }
            fs::write(&release,b"release").unwrap();
            if outcome == "success" { history_count(&server,1).await; }
            else { tokio::time::sleep(Duration::from_millis(350)).await; }
        }
        let after = current(&server).await;
        assert_eq!(after["shutdown"]["phase"],"counting-down","{outcome}");
        assert_eq!(after["shutdown"]["arrangementId"],before["shutdown"]["arrangementId"]);
        assert_eq!(after["shutdown"]["deadline"],before["shutdown"]["deadline"]);
        assert_eq!(after["shutdown"]["monitoredSessionIds"],json!([]));
        common::post_over_tcp(server.addr(),"/api/shutdown/cancel").await;
        fs::write(&release,b"cleanup").unwrap();
        drop(server);
        let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config,Arc::new(NeverPowerOff))).await;
        assert_eq!(current(&restarted).await["shutdown"]["phase"],"none");
    }
}


