//! P16 at approved S2/S3: independent TCP requests, real CLI adapter, external gates/records.
mod common;
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use common::{HerdrFixture, TempDir, TestServer, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("Resume must not power off") }
}
fn setup(root: &TempDir, herdr: &HerdrFixture) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.herdr.executable = herdr.executable().to_owned();
    config.sessions.history_file = root.path().join("history.json");
    config.bots.storage_file = root.path().join("bots.json");
    config.bots.metadata_file = root.path().join("options.toml");
    let integration = root.path().join("required-pi-integration.ts");
    std::fs::write(&integration, "// controlled integration entrypoint").unwrap();
    std::fs::write(&config.bots.metadata_file, format!("skills = []\n[[agents]]\nid = 'pi'\nlabel = 'Pi'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'pi-state'\nlabel = 'Required integration'\nagent = 'pi'\nkind = 'fixed'\nentrypoint = '{}'\n", integration.display())).unwrap();
    std::fs::write(&config.sessions.history_file, json!([
        {"sessionId":"native-one","sessionName":"First Chat","agentType":"pi","cwd":root.path(),"firstRecordedAt":1},
        {"sessionId":"native-two","sessionName":"Second Chat","agentType":"pi","cwd":root.path(),"firstRecordedAt":2}
    ]).to_string()).unwrap();
    config
}
async fn resume(addr: SocketAddr, id: &str) -> common::HttpResponse {
    post_json_over_tcp(addr, "/api/chats/resume", &json!({"sessionId":id}).to_string()).await
}
async fn wait_call(herdr: &HerdrFixture, kind: &str, count: usize) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while herdr.calls().iter().filter(|call| call["kind"] == kind).count() < count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("external CLI must reach the controlled gate");
}
async fn assert_busy(addr: SocketAddr) {
    let reply = tokio::time::timeout(Duration::from_secs(3), resume(addr, "native-one"))
        .await.expect("duplicate must be rejected before the owner is released, never queued");
    assert_eq!(reply.status, 409, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["code"], "resume-in-progress");
    assert!(body["message"].as_str().unwrap().contains("不会排队"));
}
fn target(id: &str) -> Value {
    json!({"agent":"pi","name":"existing","workspace_id":"w-existing","tab_id":"w-existing:t1","pane_id":"w-existing:p1","focused":false,
        "agent_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":id}})
}

// P16: normal location and each explicit external failure must release eligibility.
// A leaked permit would keep a new user operation busy; a delayed rejected request
// would add another query or creation after the held operation finishes.
#[tokio::test]
async fn located_and_explicit_lookup_focus_or_start_failures_release_for_a_new_user_request() {
    for outcome in ["located", "fail_agent_list", "fail_agent_focus", "fail_agent_start"] {
        let root = TempDir::new(outcome);
        let herdr = HerdrFixture::new(outcome);
        let config = setup(&root, &herdr);
        let release = root.path().join("release");
        let mut behavior = json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-one"},"native_cwd":root.path()});
        if outcome != "fail_agent_start" {
            behavior["agents"] = json!([target("native-one")]);
            behavior["agent_list_gate"] = json!({"wait_for_file":release});
        } else {
            behavior["agent_start_gate"] = json!({"wait_for_file":release});
        }
        if outcome != "located" { behavior[outcome] = json!(true); }
        herdr.set_behavior(behavior);
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
        let addr = server.addr();
        let first = tokio::spawn(resume(addr, "native-one"));
        wait_call(&herdr, if outcome == "fail_agent_start" { "agent_start" } else { "agent_list" }, 1).await;
        assert_busy(addr).await;
        std::fs::write(&release, b"release").unwrap();
        let reply = first.await.unwrap();
        assert_eq!(reply.status, if outcome == "located" { 200 } else { 502 }, "{outcome}: {}", reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["outcome"], if outcome == "located" { "located" } else { "failed" });
        if outcome != "located" {
            assert_eq!(body["stage"], match outcome { "fail_agent_list" => "lookup", "fail_agent_focus" => "focus", _ => "start" });
        }
        herdr.set_behavior(json!({"agents":[target("native-one")]}));
        let fresh = resume(addr, "native-one").await;
        assert_eq!(fresh.status, 200, "{outcome}: {}", fresh.body);
        assert_eq!(serde_json::from_str::<Value>(&fresh.body).unwrap()["outcome"], "located");
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_list").count(), 2);
        assert_eq!(herdr.tabs().len(), usize::from(outcome == "fail_agent_start"));
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), usize::from(outcome == "fail_agent_start"));
    }
}

// P16 is keyed by Chat and app instance, not a global mutex or a distributed lock.
// While the first Chat is held at start, another Chat in this same instance and
// the same Chat in a separate instance must both complete before its release.
#[tokio::test]
async fn other_chats_and_separate_instances_are_not_blocked_by_an_active_resume() {
    let root = TempDir::new("Independent Resume scopes");
    let herdr = HerdrFixture::new("independent-resume-scopes");
    let config = setup(&root, &herdr);
    let release = root.path().join("release");
    herdr.set_behavior(json!({"agent_start_gate":{"wait_for_file":release},
        "native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-one"},"native_cwd":root.path()}));
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let addr = server.addr();
    let first = tokio::spawn(resume(addr, "native-one"));
    wait_call(&herdr, "agent_start", 1).await;
    herdr.set_behavior(json!({"agents":[target("native-one"), target("native-two")],
        "native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-one"},"native_cwd":root.path()}));
    let second_instance = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    for (requester, id) in [(addr, "native-two"), (second_instance.addr(), "native-one")] {
        let reply = tokio::time::timeout(Duration::from_secs(3), resume(requester, id)).await.expect("unrelated operations must not wait for the held Chat");
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["outcome"], "located");
    }
    std::fs::write(&release, b"release").unwrap();
    let reply = first.await.unwrap();
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(herdr.tabs().len(), 1);
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 1);
}

// Independent source: P16 allows one accepted operation, rejects duplicates rather
// than queuing them. Missing an early guard or dropping it before start would let
// these independent callers reach a second list/create/start at the held gates.
#[tokio::test]
async fn duplicate_requests_during_lookup_and_start_are_rejected_not_queued_then_fresh_request_can_run() {
    let root = TempDir::new("Resume concurrency");
    let herdr = HerdrFixture::new("resume-concurrency");
    let config = setup(&root, &herdr);
    let lookup_release = root.path().join("release-lookup");
    let start_release = root.path().join("release-start");
    herdr.set_behavior(json!({
        "agent_list_gate":{"wait_for_file":lookup_release},
        "agent_start_gate":{"wait_for_file":start_release},
        "native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-one"},
        "native_cwd":root.path()
    }));
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let addr = server.addr();
    let mut first = tokio::spawn(resume(addr, "native-one"));
    wait_call(&herdr, "agent_list", 1).await;
    assert_busy(addr).await;
    assert_eq!(herdr.calls().len(), 1, "busy requests must not even query Herdr");
    std::fs::write(&lookup_release, b"release").unwrap();
    tokio::select! {
        _ = wait_call(&herdr, "agent_start", 1) => {},
        reply = &mut first => {
            let reply = reply.unwrap();
            panic!("Resume ended before start: {} {}", reply.status, reply.body);
        }
    }
    assert_busy(addr).await;
    assert_eq!(herdr.tabs().len(), 1);
    std::fs::write(&start_release, b"release").unwrap();
    let reply = first.await.unwrap();
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["outcome"], "restored");
    assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),
        vec!["agent_list", "workspace_list", "tab_create", "agent_start", "agent_get", "agent_focus"]);
    herdr.set_behavior(json!({"agents":[target("native-one")]}));
    let fresh = resume(addr, "native-one").await;
    assert_eq!(fresh.status, 200, "{}", fresh.body);
    assert_eq!(serde_json::from_str::<Value>(&fresh.body).unwrap()["outcome"], "located");
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_list").count(), 2);
    assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(), 1);
    assert_eq!(herdr.tabs().len(), 1, "rejected requests leave no delayed restoration copy");
}
