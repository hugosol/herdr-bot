//! Approved S2/S3: real TCP requests and the production CLI adapter.
mod common;
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use common::{HerdrFixture, TempDir, TestServer, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("Resume must not power off") }
}
fn setup(root: &TempDir, herdr: &HerdrFixture, kind: &str) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.herdr.executable = herdr.executable().to_owned();
    config.sessions.history_file = root.path().join("history.json");
    config.tasks.storage_file = root.path().join("tasks.json");
    config.tasks.metadata_file = root.path().join("options.toml");
    let integration = root.path().join("fixed-integration.ts");
    std::fs::write(&integration, "// controlled fixed integration").unwrap();
    std::fs::write(&config.tasks.metadata_file, format!("skills = []\n[[agents]]\nid = '{kind}'\nlabel = '{kind}'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'fixed'\nlabel = 'Required integration'\nagent = '{kind}'\nkind = 'fixed'\nentrypoint = '{}'\n", integration.display())).unwrap();
    std::fs::write(&config.sessions.history_file, json!([{"sessionId":"timeout-native","agentType":kind,"cwd":root.path(),"firstRecordedAt":1}]).to_string()).unwrap();
    config
}
async fn resume(addr: SocketAddr) -> common::HttpResponse {
    post_json_over_tcp(addr, "/api/chats/resume", r#"{"sessionId":"timeout-native"}"#).await
}
async fn wait_call(herdr: &HerdrFixture, kind: &str, count: usize) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while herdr.calls().iter().filter(|c| c["kind"] == kind).count() < count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("external control stage must be reached");
}
// P17/P18/P22: timeout while waiting for a mutating CLI response is not a
// safe failure either. Source: retain only identifiers actually returned; a
// Workspace lookup cannot invent the Tab/Pane IDs of a timed-out create. A
// plausible violation reports definite failure or loses the known destination.
#[tokio::test]
async fn pi_and_omp_mutating_command_timeouts_expose_only_observed_destination() {
    for kind in ["pi", "omp"] {
        for stage in ["tab", "start"] {
            let root = TempDir::new("Resume mutating timeout");
            let herdr = HerdrFixture::new("resume-command-timeout");
            let config = setup(&root, &herdr, kind);
            let mut behavior = json!({});
            behavior[if stage == "tab" { "tab_create_gate" } else { "agent_start_gate" }] = json!({"wait_for_file":root.path().join("never-release"),"timeout_ms":120000});
            herdr.set_behavior(behavior);
            let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
            let addr = server.addr();
            let first = tokio::spawn(resume(addr));
            wait_call(&herdr, if stage == "tab" { "tab_create" } else { "agent_start" }, 1).await;
            busy(addr).await;
            // External gate establishes the wait before advancing only the
            // application's clock; no wall-clock race chooses the interleaving.
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(if stage == "tab" { 21 } else { 41 })).await;
            tokio::time::resume();
            let reply = first.await.unwrap();
            assert_eq!(reply.status, 502, "{kind}/{stage}: {}", reply.body);
            let body: Value = serde_json::from_str(&reply.body).unwrap();
            assert_eq!(body["outcome"], "unconfirmed", "{kind}/{stage}: {body}");
            assert_eq!(body["stage"], stage);
            assert!(body["message"].as_str().unwrap().contains("启动结果未确认"));
            assert_eq!(body["target"]["workspaceId"], "w-target");
            assert_eq!(body["progress"], json!({"tabCreated":if stage == "start" { json!(true) } else { Value::Null },"startSubmitted":stage == "start","conversationConfirmed":false,"focused":false}));
            if stage == "tab" {
                assert_eq!(body["target"], json!({"workspaceId":"w-target"}));
                assert!(body["agentName"].is_null());
                assert!(herdr.tabs().is_empty());
            } else {
                let tab = &herdr.tabs()[0];
                assert_eq!(body["target"]["tabId"], tab["tab_id"]);
                assert_eq!(body["target"]["paneId"], tab["pane_id"]);
                assert_eq!(body["agentName"], herdr.calls().last().unwrap()["name"]);
                assert_eq!(herdr.tabs().len(), 1);
            }
            assert!(herdr.calls().iter().all(|c| ["agent_list","workspace_list","tab_create","agent_start"].contains(&c["kind"].as_str().unwrap())));
            // Qualification is released even when the CLI itself timed out.
            herdr.set_behavior(json!({"fail_agent_list":true}));
            let retry = resume(addr).await;
            assert_eq!(retry.status, 502);
            assert_eq!(serde_json::from_str::<Value>(&retry.body).unwrap()["stage"], "lookup");
            assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "tab_create").count(), 1);
        }
    }
}

async fn busy(addr: SocketAddr) {
    let reply = tokio::time::timeout(Duration::from_secs(3), resume(addr)).await.expect("duplicates must not queue");
    assert_eq!(reply.status, 409, "{}", reply.body);
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["code"], "resume-in-progress");
}
fn assert_retained(herdr: &HerdrFixture) {
    assert_eq!(herdr.tabs().len(), 1);
    assert_eq!(common::read_json_lines(&herdr.path().join("started.jsonl")).len(), 1);
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|c| c["kind"] == "tab_create").count(), 1);
    assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_start").count(), 1);
    assert!(calls.iter().all(|c| ["agent_list","workspace_list","tab_create","agent_start","agent_get","agent_focus"].contains(&c["kind"].as_str().unwrap())), "no close, terminate or input commands: {calls:?}");
}
// P17-P19/P22 + P16 timeout interleaving. Source: a submitted launch without
// native identity is uncertain, not failed or restored. A plausible violation
// reports success after agent.start, discards the target, leaks eligibility or
// replays startup on retry. Keep real polling/locking/CLI execution in the path;
// only the external native reference is withheld, then attached to the SAME
// externally created target. Explicit lookup gates distinguish concurrent retry
// rejection from queued execution, and actual records prove target retention.
#[tokio::test]
async fn pi_and_omp_timeout_retain_target_release_eligibility_and_retry_by_lookup_not_replay() {
    for kind in ["pi", "omp"] {
        let root = TempDir::new("Resume uncertain startup");
        let herdr = HerdrFixture::new("resume-timeout");
        let config = setup(&root, &herdr, kind);
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
        let addr = server.addr();
        let mut first = tokio::spawn(resume(addr));
        tokio::select! {
            _ = wait_call(&herdr, "agent_get", 1) => {},
            reply = &mut first => { let reply = reply.unwrap(); panic!("before confirmation: {} {}", reply.status, reply.body); }
        }
        busy(addr).await;
        let reply = first.await.unwrap();
        assert_eq!(reply.status, 502, "{kind}: {}", reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["outcome"], "unconfirmed", "{kind}: {body}");
        assert_eq!(body["code"], "restore-unconfirmed");
        assert_eq!(body["stage"], "confirm");
        assert!(body["message"].as_str().unwrap().contains("启动结果未确认"));
        assert!(!body["message"].as_str().unwrap().contains("阶段失败"));
        let mut target = common::read_json_lines(&herdr.path().join("started.jsonl"))[0].clone();
        for (public, external) in [("workspaceId","workspace_id"),("tabId","tab_id"),("paneId","pane_id")] {
            assert_eq!(body["target"][public], target[external]);
        }
        assert_eq!(body["agentName"], target["name"]);
        assert_eq!(body["progress"], json!({"tabCreated":true,"startSubmitted":true,"conversationConfirmed":false,"focused":false}));
        assert!(!herdr.calls().iter().any(|c| c["kind"] == "agent_focus"));
        assert_retained(&herdr);
        let queries = herdr.calls().iter().filter(|c| c["kind"] == "agent_list").count();
        assert_eq!(queries, 1, "busy operation did not query or queue");
        target["agent_session"] = json!({"agent":kind,"source":format!("herdr:{kind}"),"kind":"id","value":"timeout-native"});
        // P19/P20: query and focus failures still stop a new operation without
        // replaying launch. Bad candidate identities are now skipped separately.
        for (fault, stage, code) in [("query","lookup","herdr-command-failed"), ("focus","focus","herdr-command-failed")] {
            let mut behavior = json!({"agents":[target.clone()]});
            if fault == "query" { behavior["fail_agent_list"] = json!(true); }
            else { behavior["fail_agent_focus"] = json!(true); }
            herdr.set_behavior(behavior);
            let reply = resume(addr).await;
            assert_eq!(reply.status, 502, "{kind}/{fault}: {}", reply.body);
            let failed: Value = serde_json::from_str(&reply.body).unwrap();
            assert_eq!(failed["stage"], stage);
            assert_eq!(failed["code"], code);
            assert_retained(&herdr);
        }
        let release = root.path().join("release-retry");
        herdr.set_behavior(json!({"agents":[target],"agent_list_gate":{"wait_for_file":release}}));
        let retry = tokio::spawn(resume(addr));
        wait_call(&herdr, "agent_list", 4).await;
        busy(addr).await;
        std::fs::write(&release, b"release").unwrap();
        let reply = retry.await.unwrap();
        assert_eq!(reply.status, 200, "{}", reply.body);
        let located: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(located["outcome"], "located");
        assert_eq!(located["target"], body["target"]);
        assert_eq!(std::fs::read_to_string(herdr.path().join("focused.json")).unwrap(), target["pane_id"].to_string());
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "agent_list").count(), 4);
        assert_retained(&herdr);
    }
}
