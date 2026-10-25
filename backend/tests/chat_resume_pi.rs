//! Approved seams S2/S3: real HTTP, real temporary paths, production CLI adapter.
mod common;
use std::{path::Path, sync::Arc};
use common::{HerdrFixture, TempDir, TestServer, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
struct NeverPowerOff;
impl PowerOff for NeverPowerOff { fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("resume must not power off") } }
fn setup(root: &TempDir, herdr: &HerdrFixture, file: Option<&Path>) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.herdr.executable = herdr.executable().to_owned();
    config.sessions.history_file = root.path().join("history.json");
    config.bots.storage_file = root.path().join("bots.json");
    config.bots.metadata_file = root.path().join("options.toml");
    let integration = root.path().join("required-pi-integration.ts");
    std::fs::write(&integration,"throw new Error('must not execute during validation')").unwrap();
    std::fs::write(&config.bots.metadata_file,format!("skills = []\n[[agents]]\nid = 'pi'\nlabel = 'Pi'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[models]]\nid = 'bot-model'\nlabel = 'Bot Model'\nprovider_model = 'bot-provider/bot-model'\nthinking = {{ pi = ['high'] }}\n[[extensions]]\nid = 'pi-state'\nlabel = 'Required integration'\nagent = 'pi'\nkind = 'fixed'\nentrypoint = '{}'\n",integration.display())).unwrap();
    std::fs::write(&config.sessions.history_file, json!([{"sessionId":"pi-old-exact","sessionName":"Old Pi","agentType":"pi","cwd":root.path(),"sessionFile":file,"firstRecordedAt":1}]).to_string()).unwrap();
    config
}
async fn resume(config: &herdr_bot::Config) -> common::HttpResponse {
    let server = TestServer::start_app(herdr_bot::app_with_power_off(config, Arc::new(NeverPowerOff))).await;
    let bot = post_json_over_tcp(server.addr(), "/api/bots", &json!({"name":"Distinct Bot", "chatName":"Bot Chat", "agent":"pi", "model":"bot-model", "thinking":"high", "prompt":"BOT INITIAL MUST NEVER BE SUBMITTED", "extensions":[]}).to_string()).await;
    assert_eq!(bot.status,200,"{}",bot.body);
    post_json_over_tcp(server.addr(), "/api/chats/resume", r#"{"sessionId":"pi-old-exact"}"#).await
}
// P6/P8/P10/P11/P13/P14/P15/P22. The independent session header identifies
// the history. A plausible violation would start a fresh Chat or apply a Bot
// preset; observe the external argv, no input, loaded identity and final focus.
#[tokio::test]
async fn pi_history_loads_file_in_recorded_cwd_then_focuses_without_input_or_bot_overrides() {
    let root = TempDir::new("Pi resume with spaces");
    let herdr = HerdrFixture::new("pi-resume-file");
    let native = root.path().join("old conversation.jsonl");
    std::fs::write(&native, format!("{}\n{}\n{}\n",json!({"type":"session","id":"pi-old-exact","cwd":root.path()}),json!({"type":"model_change","provider":"native-provider","modelId":"native-model"}),json!({"type":"thinking_level_change","thinkingLevel":"low"}))).unwrap();
    let config = setup(&root,&herdr,Some(&native));
    herdr.set_behavior(json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"path","value":native},"native_cwd":root.path()}));
    let reply = resume(&config).await;
    assert_eq!(reply.status,200,"{}",reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"],"restored");
    assert_eq!(body["fallback"],true);
    assert!(body["message"].as_str().unwrap().contains("默认 Workspace"));
    assert!(body["message"].as_str().unwrap().contains("不表示 Windows 窗口已置前"));
    let calls = herdr.calls();
    assert_eq!(calls.iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),vec!["agent_list","workspace_list","tab_create","agent_start","agent_get","agent_focus"]);
    assert_eq!(herdr.tabs()[0]["cwd"],json!(root.path()));
    assert_eq!(herdr.tabs()[0]["workspace"],"w-target");
    let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
    let args = start["args"].as_array().unwrap();
    assert!(args.windows(2).any(|w| w == [json!("--session"),json!(native)]));
    assert!(!args.iter().any(|a| ["--model","--thinking","--continue","--resume"].contains(&a.as_str().unwrap_or(""))));
    assert_eq!(body["target"]["paneId"],start["pane"]);
    assert_eq!(start["args"], json!(["--session", native]));
    let actual = common::read_json_lines(&herdr.path().join("started.jsonl"));
    assert!(common::same_path(Path::new(actual[0]["actual_cwd"].as_str().unwrap()),root.path()));
}

// P10/P12/P14: never load a different file or replace it with a blank launch.
// Missing Bot fixed entries are not a precondition for source-less discovery.
#[tokio::test]
async fn wrong_session_file_fails_before_creation() {
    let root = TempDir::new("Pi invalid restoration settings");
    let herdr = HerdrFixture::new("pi-invalid-settings");
    let native = root.path().join("another.jsonl");
    std::fs::write(&native,format!("{}\n",json!({"type":"session","id":"not-the-selected-chat","cwd":root.path()}))).unwrap();
    let config = setup(&root,&herdr,Some(native.as_path()));
    let reply = resume(&config).await;
    assert_eq!(reply.status,502,"{}",reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["code"], "native-session-mismatch");
    assert_eq!(herdr.calls().len(),1);
    assert_eq!(herdr.calls()[0]["kind"],"agent_list");
    assert!(herdr.tabs().is_empty());
}

// P9: absence, same-name ambiguity and external lookup errors must not
// allocate a substitute workspace or create a Tab.
#[tokio::test]
async fn default_workspace_must_resolve_uniquely_before_creating_any_target() {
    for (workspaces,behavior,code) in [
        (json!([]),json!({}),"workspace-not-found"),
        (json!([{"workspace_id":"w1","label":"bot-playground"},{"workspace_id":"w2","label":"bot-playground"}]),json!({}),"workspace-ambiguous"),
        (json!([]),json!({"fail_workspace_list":true}),"herdr-command-failed"),
    ] {
        let root = TempDir::new("Pi workspace failure");
        let herdr = HerdrFixture::new("pi-workspace-failure");
        let config = setup(&root,&herdr,None);
        herdr.set_workspaces(workspaces);
        herdr.set_behavior(behavior);
        let reply = resume(&config).await;
        assert_eq!(reply.status,502,"{}",reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["stage"],"workspace");
        assert_eq!(body["code"],code);
        assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),["agent_list","workspace_list"]);
        assert!(herdr.tabs().is_empty());
    }
}

// P11/P12: missing cwd and invalid agent metadata are visible failures,
// never permission to infer an agent from a JSON file or replace the cwd.
#[tokio::test]
async fn missing_cwd_or_unrecognized_agent_type_never_starts_a_process() {
    for (kind,cwd,code) in [(json!("pi"),json!("Z:/definitely-missing-resume-cwd"),"invalid-cwd"),(Value::Null,Value::Null,"unsupported-agent"),(json!("unknown"),Value::Null,"unsupported-agent"),(json!("claude"),Value::Null,"unsupported-agent")] {
        let root = TempDir::new("Pi invalid history");
        let herdr = HerdrFixture::new("pi-invalid-history");
        let config = setup(&root,&herdr,None);
        let mut records: Value = serde_json::from_slice(&std::fs::read(&config.sessions.history_file).unwrap()).unwrap();
        records[0]["agentType"] = kind;
        records[0]["cwd"] = cwd;
        std::fs::write(&config.sessions.history_file,records.to_string()).unwrap();
        let reply = resume(&config).await;
        assert!(reply.status >= 400,"{}",reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["code"],code);
        assert!(!body["message"].as_str().unwrap().is_empty());
        assert!(herdr.calls().iter().all(|c| c["kind"] == "agent_list"));
        assert!(herdr.tabs().is_empty());
    }
}

// P6/P12/P22: interactive readiness alone is not identity confirmation;
// neither another conversation nor a refused focus is success. A blank process
// that times out is covered as unconfirmed in chat_resume_timeout.rs.
#[tokio::test]
async fn failed_launch_wrong_identity_and_focus_failure_cannot_be_success() {
    for (behavior,stage,expected_last) in [
        (json!({"fail_tab_create":true}),"tab","tab_create"),
        (json!({"fail_agent_start":true}),"start","agent_start"),
        (json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":"different-conversation"}}),"confirm","agent_get"),
        (json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":"pi-old-exact"},"fail_agent_focus":true}),"focus","agent_focus"),
    ] {
        let root = TempDir::new("Pi confirmation failure");
        let herdr = HerdrFixture::new("pi-confirm-failure");
        let config = setup(&root,&herdr,None);
        herdr.set_behavior(behavior);
        let reply = resume(&config).await;
        assert_eq!(reply.status,502,"{}",reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["outcome"],"failed");
        assert_eq!(body["stage"],stage);
        assert_eq!(herdr.calls().last().unwrap()["kind"],expected_last);
        assert_eq!(herdr.calls().iter().filter(|c| c["kind"] == "tab_create").count(),1);
        assert!(!herdr.calls().iter().any(|c| c["kind"] == "agent_prompt"));
    }
}

// P11: the file header's historical cwd cannot stand in for the actual
// foreground cwd. A process launched elsewhere must not be called restored.
#[tokio::test]
async fn pi_file_identity_does_not_hide_an_incorrect_actual_target_cwd() {
    let root = TempDir::new("Pi wrong actual cwd");
    let herdr = HerdrFixture::new("pi-wrong-cwd");
    let native = root.path().join("old.jsonl");
    std::fs::write(&native,format!("{}\n",json!({"type":"session","id":"pi-old-exact","cwd":root.path()}))).unwrap();
    let config = setup(&root,&herdr,Some(&native));
    herdr.set_behavior(json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"path","value":native},"foreground_cwd":herdr.path()}));
    let reply = resume(&config).await;
    assert_eq!(reply.status,502,"{}",reply.body);
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["stage"],"confirm");
    assert!(!herdr.calls().iter().any(|c| c["kind"] == "agent_focus"));
}

// P14/P15, S2/S3: independent missing-file sample and retained original Workspace.
// Violation: exact-ID loading works but silently hides why the file was replaced.
// Observe both the actual load reference and the public degradation facts.
#[tokio::test]
async fn unavailable_file_reports_exact_id_strategy_without_changing_the_result() {
    let root = TempDir::new("Pi visible exact ID fallback");
    let herdr = HerdrFixture::new("pi-visible-id-fallback");
    let missing = root.path().join("unavailable.jsonl");
    let config = setup(&root, &herdr, Some(&missing));
    let mut records: Value = serde_json::from_slice(&std::fs::read(&config.sessions.history_file).unwrap()).unwrap();
    records[0]["originalWorkspaceId"] = json!("w-target");
    std::fs::write(&config.sessions.history_file, records.to_string()).unwrap();
    herdr.set_behavior(json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":"pi-old-exact"}}));
    let reply = resume(&config).await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"], "restored");
    let items = body["degradations"].as_array().expect("visible degradation facts");
    assert_eq!(items.len(), 2);
    assert_eq!(items[1]["element"], "extensions");
    assert_eq!(items[0]["element"], "session-file");
    assert!(items[0]["reason"].as_str().unwrap().contains(missing.to_str().unwrap()));
    assert!(items[0]["strategy"].as_str().unwrap().contains("pi-old-exact"));
    assert!(body["message"].as_str().unwrap().contains("精确原生 ID"));
    let calls = herdr.calls();
    let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
    assert!(start["args"].as_array().unwrap().windows(2).any(|w| w == [json!("--session"), json!("pi-old-exact")]));
    assert!(!missing.exists(), "Resume must not repair the native file");
}

// P10: a missing file may fall back to the exact ID, not an arbitrary newest
// session. These two inputs are independent from the file-preference sample.
#[tokio::test]
async fn pi_id_only_and_unavailable_file_restore_the_exact_native_id() {
    for with_missing_file in [false,true] {
        let root = TempDir::new("Pi ID fallback spaces");
        let herdr = HerdrFixture::new("pi-resume-id");
        let missing = root.path().join("missing.jsonl");
        let config = setup(&root,&herdr,with_missing_file.then_some(missing.as_path()));
        herdr.set_behavior(json!({"native_session":{"agent":"pi","source":"herdr:pi","kind":"id","value":"pi-old-exact"}}));
        let reply = resume(&config).await;
        assert_eq!(reply.status,200,"{}",reply.body);
        assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["outcome"],"restored");
        let calls = herdr.calls();
        let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
        assert_eq!(start["initialMessage"], "");
        assert!(!start["argv"].as_array().unwrap().iter().any(|arg| arg == "--initial-message"));
        assert!(start["args"].as_array().unwrap().windows(2).any(|w| w == [json!("--session"),json!("pi-old-exact")]));
        assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"));
    }
}
