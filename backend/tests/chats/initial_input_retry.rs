//! S1: real creation/status and session-event HTTP, an injected monotonic
//! clock, and separate external host attempt and terminal-input records.
use super::*;
use herdr_bot::clock::ManualClock;
use std::sync::Arc;

const INPUT: &str = "Only this initial instruction";
const SESSION: &str = "retry-native-session";

struct NoPowerOff;
impl herdr_bot::PowerOff for NoPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> {
        panic!("Chat launch must not authorize power-off")
    }
}

struct LaunchFixture {
    server: TestServer,
    clock: Arc<ManualClock>,
    host: HerdrFixture,
    _storage: TempDir,
    id: String,
    report: Value,
    app: bool,
}

impl LaunchFixture {
    async fn start(app: bool, agent: &str, behavior: Value) -> Self {
        let host = HerdrFixture::new("initial-input-retry");
        let storage = TempDir::new("initial-input-storage");
        host.set_behavior(behavior);
        let clock = Arc::new(ManualClock::new());
        let mut config = config_with(&host, &storage);
        config.sessions.history_file = storage.path().join("history.json");
        if app {
            fs::write(host.path().join("chat-app-mode"), "").unwrap();
            config.herdr.backend = herdr_bot::config::HerdrBackend::App;
            config.herdr.app_executable = host.executable().into();
            config.herdr.core_executable = host.executable().into();
        }
        let sessions = Arc::new(herdr_bot::sessions::SessionsState::with_policy_and_clock(
            config.sessions.policy(), clock.clone(),
        ));
        let server = TestServer::start_app(herdr_bot::app_with_sessions(&config, Arc::new(NoPowerOff), sessions)).await;
        let cwd = storage.path().join("alpha");
        fs::create_dir(&cwd).unwrap();
        let zone = register(&server, &cwd).await;
        let mut task = default_task();
        task["agent"] = json!(agent);
        let task = save_task(&server, task).await;
        let id = accept_creation(&server, &zone, task["id"].as_str().unwrap(), INPUT).await;
        let init = wait_stage(&server, &id, "initializing").await;
        assert_eq!(init["inputSubmitted"], false);
        let (launch, target) = if app {
            let tabs = common::read_json_lines(&host.path().join("app-tabs.jsonl"));
            let argv = &tabs[0]["options"]["argv"];
            (argv[6].clone(), argv[7].clone())
        } else {
            let correlations = common::read_json_lines(&host.path().join("tab-launches.jsonl"));
            (correlations[0]["launch"].clone(), correlations[0]["target"].clone())
        };
        let report = json!({"sessionId": SESSION, "data": {
            "launchId": launch, "targetId": target, "agentType": agent, "cwd": cwd, "pid": 43127,
        }});
        Self { server, clock, host, _storage: storage, id, report, app }
    }

    async fn event(&self, kind: &str) -> u16 {
        let mut report = self.report.clone();
        report["kind"] = json!(kind);
        self.send_event(report).await
    }

    async fn send_event(&self, report: Value) -> u16 {
        post_json_over_tcp(self.server.addr(), "/api/sessions/events", &report.to_string()).await.status
    }

    async fn ready(&self) {
        assert_eq!(self.event("launch_identity").await, 200);
        assert_eq!(self.event("launch_ready").await, 200);
    }

    async fn stage(&self, key: &str) -> Value {
        wait_stage(&self.server, &self.id, key).await
    }

    fn attempts(&self) -> Vec<Value> {
        calls_of_kind(&self.host, if self.app { "core_send" } else { "agent_prompt" })
    }

    fn external_input(&self) -> Vec<Value> {
        common::read_json_lines(&self.host.path().join("submitted.jsonl"))
    }

    fn assert_one_creation(&self) {
        if self.app {
            assert_eq!(common::read_json_lines(&self.host.path().join("app-tabs.jsonl")).len(), 1);
            assert!(calls_of_kind(&self.host, "agent_start").is_empty());
        } else {
            assert_eq!(self.host.tabs().len(), 1);
            assert_eq!(calls_of_kind(&self.host, "tab_create").len(), 1);
            assert_eq!(calls_of_kind(&self.host, "agent_start").len(), 1);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn initial_retry_runs_five_times_one_second_after_each_rejection() {
    // P20/P24–P26/P50/P52: advancing time while the first send awaits its
    // rejection and publishing duplicate/out-of-order ready events must not
    // make the second attempt early, reset its deadline, or start another job.
    let f = LaunchFixture::start(false, "omp", json!({"manual_launch_events": true, "block_prompt": true,
        "prompt_gate": {"wait_for_file": "release-prompt"}})).await;
    assert_eq!(f.event("launch_ready").await, 200); // ready arrives before identity
    assert!(f.attempts().is_empty());
    f.ready().await;
    assert!(wait_until(Duration::from_secs(5), || f.attempts().len() == 1).await);
    f.clock.advance(Duration::from_secs(20));
    assert_eq!(creation(&f.server, &f.id).await["stage"], "submitting");
    fs::write(f.host.path().join("release-prompt"), "").unwrap();

    let waiting = f.stage("retrying").await;
    assert_eq!(waiting["inputOutcome"], "not-submitted");
    assert_eq!(waiting["inputSubmitted"], false);
    assert!(waiting["retryReason"].as_str().unwrap().contains("controlled modal"));
    assert!(f.external_input().is_empty(), "blocked input must not reach the terminal");
    f.clock.advance(Duration::from_millis(999));
    for kind in ["launch_ready", "launch_identity", "launch_ready"] {
        assert_eq!(f.event(kind).await, 200);
    }
    assert_eq!(creation(&f.server, &f.id).await, waiting);
    assert!(!wait_until(Duration::from_millis(250), || f.attempts().len() > 1).await);

    for attempt in 2..=6 {
        let reason = format!("modal attempt {attempt}");
        f.host.set_behavior(json!({"manual_launch_events": true, "block_prompt": true, "block_reason": reason}));
        f.clock.advance(Duration::from_millis(1));
        assert!(wait_until(Duration::from_secs(5), || f.attempts().len() == attempt).await);
        if attempt < 6 {
            let next = f.stage("retrying").await;
            assert!(next["retryReason"].as_str().unwrap().contains(&reason));
            f.clock.advance(Duration::from_millis(999));
            assert!(!wait_until(Duration::from_millis(100), || f.attempts().len() > attempt).await);
        }
    }
    let failed = f.stage("failed").await;
    assert_eq!(failed["inputOutcome"], "not-submitted");
    assert!(failed["failure"]["message"].as_str().unwrap().contains("modal attempt 6"));
    assert_eq!(failed["tabId"], waiting["tabId"]);
    let attempts = f.attempts();
    assert_eq!(attempts.len(), 6);
    assert!(attempts.iter().all(|attempt| attempt["target"] == attempts[0]["target"]));
    for attempt in attempts {
        assert_eq!(attempt["text"], INPUT);
        assert_eq!(attempt["argv"].as_array().unwrap().len(), 4, "no force or extra input flags");
    }
    f.clock.advance(Duration::from_secs(6));
    assert!(!wait_until(Duration::from_millis(250), || f.attempts().len() > 6).await);
    assert_eq!(creation(&f.server, &f.id).await, failed);
    assert!(f.external_input().is_empty());
    f.assert_one_creation();
}

#[tokio::test(flavor = "multi_thread")]
async fn initial_retry_in_app_uses_the_same_core_terminal_and_writes_only_once() {
    // P24/P25/P52: core's pre-write error is a different wire contract from
    // TUI's. Unblocking it allows the original target to receive one input.
    let f = LaunchFixture::start(true, "pi", json!({"block_prompt":true})).await;
    f.ready().await;
    let waiting = f.stage("retrying").await;
    assert_eq!(waiting["inputOutcome"], "not-submitted");
    assert!(waiting["retryReason"].as_str().unwrap().contains("controlled core modal"));
    assert!(f.external_input().is_empty());
    for attempt in 2..=5 {
        f.clock.advance(Duration::from_secs(1));
        assert!(wait_until(Duration::from_secs(5), || f.attempts().len() == attempt).await);
        f.stage("retrying").await;
        assert!(f.external_input().is_empty());
    }
    f.host.set_behavior(json!({}));
    f.clock.advance(Duration::from_secs(1));
    let sent = f.stage("succeeded").await;
    assert_eq!(sent["inputSubmitted"], true);
    assert!(sent.get("inputOutcome").is_none());
    assert_eq!(sent["tabId"], waiting["tabId"]);
    let attempts = f.attempts();
    assert_eq!(attempts.len(), 6);
    assert!(attempts.iter().all(|attempt| attempt == &attempts[0]));
    f.clock.advance(Duration::from_secs(6));
    assert!(!wait_until(Duration::from_millis(250), || f.attempts().len() > 6).await);
    assert_eq!(f.external_input(), vec![json!({"terminal":"t1","text":INPUT})]);
    f.assert_one_creation();
}

#[tokio::test(flavor = "multi_thread")]
async fn app_resume_locates_confirmed_launch_after_initial_input_rejection() {
    let mut f = LaunchFixture::start(true, "pi", json!({"block_prompt":true})).await;
    let cwd = f.report["data"]["cwd"].clone();
    let native = Path::new(cwd.as_str().unwrap()).join("native.jsonl");
    fs::write(&native, format!("{}\n", json!({"type":"session","id":SESSION,"cwd":cwd}))).unwrap();
    f.report["data"]["sessionFile"] = json!(native);
    f.ready().await;

    // Exhaust the confirmed pre-write refusals without waiting on the wall clock.
    for attempt in 2..=6 {
        let waiting = f.stage("retrying").await;
        assert_eq!(waiting["inputOutcome"], "not-submitted");
        f.clock.advance(Duration::from_secs(1));
        assert!(wait_until(Duration::from_secs(5), || f.attempts().len() == attempt).await);
    }
    let failed = f.stage("failed").await;
    assert_eq!(failed["inputOutcome"], "not-submitted");
    assert!(f.external_input().is_empty());

    // Connect-bot registration supplies history independently of the failed creation.
    let address = f.server.addr();
    let registration = post_json_over_tcp(address, "/api/sessions/events", &json!({
        "sessionId":SESSION,"kind":"register","data":{"cwd":cwd,"agentType":"pi","sessionFile":native}
    }).to_string()).await;
    assert_eq!(registration.status, 200, "{}", registration.body);
    let history = get_over_tcp(address, "/api/sessions/history").await;
    assert_eq!(history.status, 200, "{}", history.body);
    let history: Value = serde_json::from_str(&history.body).unwrap();
    assert_eq!(history["sessions"][0]["sessionId"], SESSION);

    let request = json!({"sessionId":SESSION}).to_string();
    let response = tokio::time::timeout(
        Duration::from_secs(8),
        post_json_over_tcp(address, "/api/chats/resume", &request),
    ).await.expect("Resume should locate the live target without starting another agent");
    assert_eq!(response.status, 200, "{}", response.body);
    let resumed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(resumed["outcome"], "located", "{resumed}");
    assert_eq!(resumed["target"], json!({"workspaceId":"space-default","tabId":"tab-1","paneId":"pane-1"}));
    f.assert_one_creation();
    assert_eq!(f.attempts().len(), 6, "Resume must not replay the initial input");
}

#[tokio::test(flavor = "multi_thread")]
async fn initial_retry_cancels_for_ended_replaced_session_or_replaced_owner() {
    // P28: a confirmed exit cancels immediately; an observed session change
    // in the same core process also revokes the launch. A silently replaced
    // foreground process must fail the final host inspection at expiry.
    for (app, agent, change) in [(false, "pi", "ended"), (true, "omp", "session"), (true, "pi", "owner"), (false, "omp", "official-session")] {
        let mut behavior = json!({"manual_launch_events":true,"block_prompt":true});
        if change == "official-session" {
            behavior["native_session"] = json!({"source":"herdr:omp","agent":"omp","kind":"id","value":SESSION});
        }
        let f = LaunchFixture::start(app, agent, behavior).await;
        f.ready().await;
        let waiting = f.stage("retrying").await;
        f.clock.advance(Duration::from_millis(500));
        match change {
            "ended" => assert_eq!(f.event("launch_ended").await, 200),
            "session" => {
                let mut replacement = f.report.clone();
                replacement["sessionId"] = json!("replacement-in-original-process");
                replacement["kind"] = json!("launch_ready");
                assert_eq!(f.send_event(replacement.clone()).await, 409);
                assert_eq!(creation(&f.server, &f.id).await, waiting, "unmatched ready cannot cancel");
                replacement["kind"] = json!("launch_identity");
                assert_eq!(f.send_event(replacement).await, 409, "never rebind to the new owner");
            }
            "owner" => {
                f.host.set_behavior(json!({"block_prompt":true,"launch_pid":43128}));
                f.clock.advance(Duration::from_millis(500));
            }
            "official-session" => {
                f.host.set_behavior(json!({"block_prompt":true,"native_session":{
                    "source":"herdr:omp","agent":"omp","kind":"id","value":"replacement-native-session"
                }}));
                f.clock.advance(Duration::from_millis(500));
            }
            _ => unreachable!(),
        }
        let failed = wait_stage_within(&f.server, &f.id, "failed", Duration::from_secs(3)).await;
        assert_eq!(failed["failure"]["code"], "initial-input-retry-cancelled", "{change}: {failed}");
        assert_eq!(failed["inputOutcome"], "not-submitted");
        assert_eq!(failed["tabId"], waiting["tabId"]);
        f.clock.advance(Duration::from_secs(12));
        assert!(!wait_until(Duration::from_millis(200), || f.attempts().len() > 1).await, "{change}");
        assert!(f.external_input().is_empty(), "{change}");
        f.assert_one_creation();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn initial_retry_never_replays_unknown_input_effects() {
    // P27/P50: the external terminal can receive input even if the response
    // is lost, the write is partial, the ack has no agent, or work did not start.
    for (app, agent, behavior, visible_reason, external_text) in [
        (false, "omp", json!({"prompt_fault":"disconnect"}), "connection closed", INPUT),
        (false, "pi", json!({"prompt_fault":"partial-write"}), "writing a prefix", "Only"),
        (false, "omp", json!({"prompt_fault":"ambiguous-ack"}), "result.agent", INPUT),
        (true, "pi", json!({"started":false}), "started:false", INPUT),
    ] {
        let f = LaunchFixture::start(app, agent, behavior).await;
        f.ready().await;
        let failed = wait_stage_within(&f.server, &f.id, "failed", Duration::from_secs(3)).await;
        assert_eq!(failed["inputOutcome"], "unknown", "{failed}");
        assert_eq!(failed["inputSubmitted"], false);
        assert!(failed.get("retryReason").is_none());
        let message = failed["failure"]["message"].as_str().unwrap();
        assert!(message.contains("投递结果未确认") && message.contains(visible_reason), "{message}");
        let writes = f.external_input();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0]["text"], external_text);
        f.clock.advance(Duration::from_secs(6));
        assert!(!wait_until(Duration::from_millis(250), || f.attempts().len() > 1).await);
        assert_eq!(f.external_input(), writes);
        assert_eq!(creation(&f.server, &f.id).await, failed);
        f.assert_one_creation();
    }
}
