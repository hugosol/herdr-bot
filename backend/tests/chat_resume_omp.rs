//! Approved S2/S3 seams: real HTTP and production Herdr CLI adapter, with
//! controlled external Herdr and independent OMP native identity fixtures.
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
    let integration = root.path().join("required-omp-integration.ts");
    std::fs::write(&integration,"throw new Error('validation must not execute extensions')").unwrap();
    std::fs::write(&config.bots.metadata_file,format!("skills = []\n[[agents]]\nid = 'omp'\nlabel = 'OMP'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[models]]\nid = 'bot-model'\nlabel = 'Bot Model'\nprovider_model = 'bot-provider/bot-model'\nthinking = {{ omp = ['high'] }}\n[[extensions]]\nid = 'omp-state'\nlabel = 'Required integration'\nagent = 'omp'\nkind = 'fixed'\nentrypoint = '{}'\n",integration.display())).unwrap();
    std::fs::write(&config.sessions.history_file, json!([{"sessionId":"omp-old-3b9f-exact","sessionName":"Old OMP","agentType":"omp","cwd":root.path(),"sessionFile":file,"firstRecordedAt":1}]).to_string()).unwrap();
    config
}
async fn resume(config: &herdr_bot::Config) -> common::HttpResponse {
    let server = TestServer::start_app(herdr_bot::app_with_power_off(config, Arc::new(NeverPowerOff))).await;
    let bot = post_json_over_tcp(server.addr(), "/api/bots", &json!({"name":"Distinct Bot", "chatName":"Bot Chat", "agent":"omp", "model":"bot-model", "thinking":"high", "prompt":"BOT INITIAL MUST NEVER BE SUBMITTED", "extensions":[]}).to_string()).await;
    assert_eq!(bot.status,200,"{}",bot.body);
    post_json_over_tcp(server.addr(), "/api/chats/resume", r#"{"sessionId":"omp-old-3b9f-exact"}"#).await
}
fn native_file(root: &TempDir) -> std::path::PathBuf {
    let file = root.path().join("old OMP conversation.jsonl");
    std::fs::write(&file, format!("{}\n{}\n{}\n",json!({"type":"session","version":3,"id":"omp-old-3b9f-exact","cwd":root.path()}),json!({"type":"model_change","provider":"native-provider","modelId":"native-model"}),json!({"type":"thinking_level_change","thinkingLevel":"low"}))).unwrap();
    file
}
// P6/P10/P11/P13/P14/P15/P22. Source: ticket requires exact native file,
// recorded cwd, resume (not restore), no Bot settings/input, then identity and
// focus confirmation. A blank/new launch or restore flag would violate this;
// observe the complete external sequence, literal argv and actual child cwd.
#[tokio::test]
async fn omp_file_resumes_in_historical_cwd_without_bot_overrides_or_input_then_focuses() {
    let root = TempDir::new("OMP resume cwd with spaces");
    let herdr = HerdrFixture::new("omp-resume-file");
    let native = native_file(&root);
    let config = setup(&root,&herdr,Some(&native));
    herdr.set_behavior(json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":native}}));
    let reply = resume(&config).await;
    assert_eq!(reply.status,200,"{}",reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"],"restored");
    assert!(body["message"].as_str().unwrap().contains("已确认恢复同一 OMP 原生对话"));
    assert!(body["message"].as_str().unwrap().contains("不表示 Windows 窗口已置前"));
    let calls = herdr.calls();
    assert_eq!(calls.iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),vec!["agent_list","workspace_list","tab_create","agent_start","agent_get","agent_focus"]);
    let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
    assert_eq!(start["args"],json!(["--resume",native]));
    assert_eq!(start["agent_kind"],"omp");
    assert_eq!(body["target"]["paneId"],start["pane"]);
    assert_eq!(herdr.tabs()[0]["cwd"],json!(root.path()));
    let actual = common::read_json_lines(&herdr.path().join("started.jsonl"));
    assert!(common::same_path(Path::new(actual[0]["actual_cwd"].as_str().unwrap()),root.path()));
}

// P10/P11/P12: shared preconditions must remain active for OMP. Another
// conversation file or missing cwd cannot be permission to start a blank
// process or silently replace the directory. Source-less history uses native
// discovery, so Bot fixed-entrypoint availability is not a precondition.
#[tokio::test]
async fn omp_unusable_sources_or_cwd_fail_without_creating_a_target() {
    for scenario in ["wrong-file", "missing-cwd"] {
        let root = TempDir::new("OMP invalid restore source");
        let herdr = HerdrFixture::new("omp-invalid-source");
        let native = root.path().join("another.jsonl");
        std::fs::write(&native,format!("{}\n",json!({"type":"session","id":"another-omp-chat","cwd":root.path()}))).unwrap();
        let config = setup(&root,&herdr,(scenario == "wrong-file").then_some(native.as_path()));
        let (stage,code) = match scenario {
            "missing-cwd" => {
                let mut records: Value = serde_json::from_slice(&std::fs::read(&config.sessions.history_file).unwrap()).unwrap();
                records[0]["cwd"] = json!(root.path().join("deleted working directory"));
                std::fs::write(&config.sessions.history_file,records.to_string()).unwrap();
                ("cwd","invalid-cwd")
            }
            _ => ("session","native-session-mismatch"),
        };
        let reply = resume(&config).await;
        assert_eq!(reply.status,502,"{scenario}: {}",reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["stage"],stage);
        assert_eq!(body["code"],code);
        assert!(body["message"].as_str().unwrap().contains("OMP 恢复"));
        assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),["agent_list"]);
        assert!(herdr.tabs().is_empty());
    }
}

// P6/P11/P12/P22: readiness alone must not report success. Blank-process timeout
// is covered as unconfirmed in chat_resume_timeout.rs. Wrong identity/cwd,
// failed start or unconfirmed focus stays a visible failure;
// no further focus/input or replacement launch may conceal the failed stage.
#[tokio::test]
async fn omp_requires_same_native_identity_actual_cwd_and_confirmed_focus() {
    for (behavior,stage,last) in [
        (json!({"fail_tab_create":true}),"tab","tab_create"),
        (json!({"fail_agent_start":true}),"start","agent_start"),
        (json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":"wrong-omp-chat"}}),"confirm","agent_get"),
        (json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":"omp-old-3b9f-exact"},"fail_agent_focus":true}),"focus","agent_focus"),
        (json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":"omp-old-3b9f-exact"},"unconfirmed_focus":true}),"focus","agent_focus"),
    ] {
        let root = TempDir::new("OMP confirmation failure");
        let herdr = HerdrFixture::new("omp-confirm-failure");
        let config = setup(&root,&herdr,None);
        herdr.set_behavior(behavior);
        let reply = resume(&config).await;
        assert_eq!(reply.status,502,"{}",reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["outcome"],"failed");
        assert_eq!(body["stage"],stage);
        let calls = herdr.calls();
        assert_eq!(calls.last().unwrap()["kind"],last);
        assert_eq!(calls.iter().filter(|c| c["kind"] == "tab_create").count(),1);
        assert!(calls.iter().filter(|c| c["kind"] == "agent_start").count() <= 1);
        assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"));
    }
    // A correct historical file header cannot conceal the process's live cwd.
    let root = TempDir::new("OMP incorrect actual cwd");
    let herdr = HerdrFixture::new("omp-wrong-cwd");
    let native = native_file(&root);
    let config = setup(&root,&herdr,Some(&native));
    herdr.set_behavior(json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":native},"foreground_cwd":herdr.path()}));
    let reply = resume(&config).await;
    assert_eq!(reply.status,502,"{}",reply.body);
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["code"],"native-session-mismatch");
    assert!(!herdr.calls().iter().any(|c| c["kind"] == "agent_focus"));
}

// The ticket requires reuse of lookup-first, not a second OMP-only launcher.
// A matching target and every lookup/focus failure must prohibit new creation.
#[tokio::test]
async fn omp_existing_target_or_lookup_failure_never_starts_another_process() {
    for fault in ["none", "fail_agent_list", "invalid_agent_list", "fail_agent_focus"] {
        let root = TempDir::new("OMP lookup first");
        let herdr = HerdrFixture::new("omp-lookup-first");
        let config = setup(&root,&herdr,None);
        let mut behavior = json!({"agents":[{"agent":"omp","name":"old-live","workspace_id":"w-existing","tab_id":"t-existing","pane_id":"p-existing","agent_status":"working","agent_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":"omp-old-3b9f-exact"}}]});
        if fault != "none" { behavior[fault] = json!(true); }
        herdr.set_behavior(behavior);
        let reply = resume(&config).await;
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(reply.status,if fault == "none" { 200 } else { 502 },"{}",reply.body);
        assert_eq!(body["outcome"],if fault == "none" { "located" } else { "failed" });
        if fault == "none" { assert_eq!(body["target"]["paneId"],"p-existing"); }
        assert!(herdr.calls().iter().all(|c| matches!(c["kind"].as_str().unwrap(),"agent_list" | "agent_get" | "agent_focus")));
        assert!(herdr.tabs().is_empty());
    }
}

// P10: the same shared pipeline already supports ID fallback. Independent
// OMP samples distinguish exact ID resume from a bare picker or newest Chat.
#[tokio::test]
async fn omp_id_only_and_unavailable_file_resume_only_the_selected_native_id() {
    for with_missing_file in [false,true] {
        let root = TempDir::new("OMP ID fallback spaces");
        let herdr = HerdrFixture::new("omp-resume-id");
        let missing = root.path().join("unavailable OMP.jsonl");
        let config = setup(&root,&herdr,with_missing_file.then_some(missing.as_path()));
        herdr.set_behavior(json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":"omp-old-3b9f-exact"}}));
        let reply = resume(&config).await;
        assert_eq!(reply.status,200,"{}",reply.body);
        assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["outcome"],"restored");
        let calls = herdr.calls();
        let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
        assert_eq!(start["initialMessage"], "");
        assert!(!start["argv"].as_array().unwrap().iter().any(|arg| arg == "--initial-message"));
        assert_eq!(start["args"],json!(["--resume","omp-old-3b9f-exact"]));
        assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"));
    }
}
