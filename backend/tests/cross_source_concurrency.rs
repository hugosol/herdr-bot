//! S2/P26 combined source partition. Public HTTP + SSE, real isolated write
//! failure, external Herdr gate; only the existing PowerOff boundary is replaced.
mod common;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure, clock::ManualClock, sessions::SessionsState};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};

#[derive(Default)]
struct RecordingPowerOff(AtomicUsize);
impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(()) // Safety boundary: never calls any OS power API.
    }
}
async fn post(server: &TestServer, path: &str, body: Value) -> Value {
    let response = post_json_over_tcp(server.addr(), path, &body.to_string()).await;
    assert!([200, 202].contains(&response.status), "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn history(server: &TestServer) -> Value {
    let response = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn snapshot(server: &TestServer) -> Value {
    let mut stream = common::sse::SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    stream.next_event("snapshot").await.json()
}
async fn event(server: &TestServer, id: &str, kind: &str, data: Value) -> Value {
    post(server, "/api/sessions/events", json!({"sessionId":id,"kind":kind,"data":data})).await
}
async fn create_pending(server: &TestServer, fixture: &HerdrFixture, zone: &str, template: &str, count: usize) {
    let preview = post(server, "/api/bots/preview", json!({"zone":zone,"templateId":template})).await;
    let accepted = post(server, "/api/bots", json!({"zone":zone,"templateId":template,"botId":preview["botId"],"input":"/settings"})).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let response = get_over_tcp(server.addr(), &format!("/api/bots/{}", accepted["id"].as_str().unwrap())).await;
            let view: Value = serde_json::from_str(&response.body).unwrap();
            assert_ne!(view["stage"], "failed", "{view}");
            if view["stage"] == "succeeded" && fixture.calls().iter().filter(|c| c["kind"] == "agent_get").count() == count { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("creation succeeds and exact identity query reaches external gate");
}
async fn wait_name(server: &TestServer, name: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let view = history(server).await;
            assert_eq!(view["total"], 1, "same full native identity must remain unique");
            if view["sessions"][0]["sessionName"] == name { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("independent history read observes completed zone persistence");
}

// Independent expectations: ADR-0006 keeps monitoring through offline/recovery,
// accepted register refreshes keepAlive even if history fails, and ADR-0005
// consumes monitoring once, never retimes an active countdown. A history failure
// that disarms registration, restores persisted intent, or resets a deadline
// fails these literal public observations. Zone tasks stay in the real path.
#[tokio::test]
async fn cross_source_write_failures_preserve_keep_alive_monitoring_and_countdown_for_pi_and_omp() {
    for (agent, id) in [("pi", "pi-357dc800-2546-4095-baaa-000000000011"), ("omp", "omp-357dc800-2546-4095-baaa-000000000012")] {
        let root = TempDir::new("cross-source-intent");
        let fixture = HerdrFixture::new("cross-source-herdr");
        let mut config = common::config(Path::new("unused.bat"), root.path());
        config.herdr.executable = fixture.executable().to_path_buf();
        config.herdr.workspace_name = "bot-playground".into();
        config.zones.registry_file = root.path().join("zones.json");
        config.bot_templates.storage_file = root.path().join("templates.json");
        config.sessions.history_file = root.path().join("history.json");
        config.shutdown.countdown_seconds = herdr_bot::CountdownSeconds::new(3600).unwrap();
        let clock = Arc::new(ManualClock::new());
        let sessions = Arc::new(SessionsState::with_policy_and_clock(config.sessions.policy(), clock.clone()));
        let power = Arc::new(RecordingPowerOff::default());
        let server = TestServer::start_app(herdr_bot::app_with_sessions(&config, power.clone(), sessions)).await;
        let zone = root.path().join("zone"); fs::create_dir(&zone).unwrap();
        let zones = post(&server, "/api/zones", json!({"path":zone})).await;
        let zone = zones["zones"][0]["path"].as_str().unwrap();
        let templates = post(&server, "/api/bot-templates", json!({"name":"Cross source", "botName":"Zone first literal", "agent":agent,"model":"default","thinking":"default","skill":null,"prompt":"","extensions":[]})).await;
        let first = templates["templates"][0]["id"].as_str().unwrap();
        let renamed = post(&server, "/api/bot-templates", json!({"name":"Cross source renamed", "botName":"Zone final literal", "agent":agent,"model":"default","thinking":"default","skill":null,"prompt":"","extensions":[]})).await;
        let renamed = renamed["templates"].as_array().unwrap().iter().find(|t| t["name"] == "Cross source renamed").unwrap()["id"].as_str().unwrap();
        assert_eq!(event(&server,id,"register",json!({"cwd":"C:/current","sessionName":"Connect first literal","agentType":agent})).await["historySaved"],true);
        let first_time = history(&server).await["sessions"][0]["firstRecordedAt"].clone();
        event(&server,id,"state",json!({"value":"working"})).await;
        post(&server,"/api/shutdown/monitor",json!({"sessionId":id,"selected":true})).await;
        let blocker = root.path().join("history.json.tmp");
        for (round, template, name) in [(1,first,"Zone first literal"), (2,renamed,"Zone final literal")] {
            let release = root.path().join(format!("release-{round}"));
            fixture.set_behavior(json!({"native_session":{"source":format!("herdr:{agent}"),"agent":agent,"kind":"id","value":id},"native_cwd":"C:/native","native_session_gate":{"wait_for_file":release,"timeout_ms":40000}}));
            create_pending(&server,&fixture,zone,template,round).await;
            clock.advance(Duration::from_secs(9));
            let intent_before_failure = snapshot(&server).await;
            fs::create_dir(&blocker).unwrap();
            let failed = event(&server,id,"register",json!({"cwd":"C:/current","sessionName":"Failed connect rename","agentType":agent})).await;
            assert_eq!(failed["outcome"],"registered");
            assert_eq!(failed["historySaved"],false);
            assert_eq!(failed["historyWarning"]["code"],"history-save-failed");
            clock.advance(Duration::from_secs(2));
            let current = snapshot(&server).await;
            for field in ["phase", "monitoredSessionIds", "arrangementId", "deadline"] {
                assert_eq!(current["shutdown"][field],intent_before_failure["shutdown"][field],"failed history preserves shutdown {field}");
            }
            assert_eq!(current["sessions"].as_array().unwrap().len(),1);
            assert_eq!(current["sessions"][0]["online"],true,"accepted failed-history register renews keepAlive");
            assert_eq!(current["sessions"][0]["state"], if round == 1 { "working" } else { "awaiting-input" });
            if round == 1 {
                assert_eq!(current["shutdown"]["monitoredSessionIds"],json!([id]));
                assert_eq!(current["shutdown"]["phase"],"none");
            } else {
                assert_eq!(current["shutdown"]["phase"],"counting-down");
                assert_eq!(current["shutdown"]["monitoredSessionIds"],json!([]));
            }
            fs::remove_dir(&blocker).unwrap();
            fs::write(&release,b"identity ready").unwrap();
            wait_name(&server,name).await;
            assert_eq!(history(&server).await["sessions"][0]["firstRecordedAt"],first_time);
            let after = snapshot(&server).await;
            assert_eq!(after["sessions"],current["sessions"],"zone history never mutates current registration");
            for field in ["phase", "monitoredSessionIds", "arrangementId", "deadline"] {
                assert_eq!(after["shutdown"][field],current["shutdown"][field],"history never changes shutdown {field}");
            }
            if round == 1 {
                clock.advance(Duration::from_secs(9));
                let offline = snapshot(&server).await;
                assert_eq!(offline["sessions"][0]["online"],false);
                assert_eq!(offline["shutdown"]["monitoredSessionIds"],json!([id]));
                event(&server,id,"heartbeat",json!({})).await;
                assert_eq!(snapshot(&server).await["sessions"][0]["online"],true);
                event(&server,id,"state",json!({"value":"awaiting-input"})).await;
                assert_eq!(snapshot(&server).await["shutdown"]["phase"],"counting-down");
            }
        }
        let saved = history(&server).await;
        assert_eq!(saved["sessions"][0]["sessionName"],"Zone final literal");
        assert_eq!(power.0.load(Ordering::SeqCst),0);
        drop(server);
        let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config,power.clone())).await;
        assert_eq!(history(&restarted).await,saved);
        let current = snapshot(&restarted).await;
        assert_eq!(current["sessions"],json!([]));
        assert_eq!(current["shutdown"]["phase"],"none");
        assert_eq!(current["shutdown"]["monitoredSessionIds"],json!([]));
        assert_eq!(power.0.load(Ordering::SeqCst),0);
    }
}
