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
    config.tasks.storage_file = root.path().join("tasks.json");
    config.tasks.metadata_file = root.path().join("options.toml");
    let integration = root.path().join("required-omp-integration.ts");
    std::fs::write(&integration,"throw new Error('validation must not execute extensions')").unwrap();
    std::fs::write(&config.tasks.metadata_file,format!("skills = []\n[[agents]]\nid = 'omp'\nlabel = 'OMP'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[models]]\nid = 'task-model'\nlabel = 'Task Model'\nprovider_model = 'task-provider/task-model'\nthinking = {{ omp = ['high'] }}\n[[extensions]]\nid = 'omp-state'\nlabel = 'Required integration'\nagent = 'omp'\nkind = 'fixed'\nentrypoint = '{}'\n",integration.display())).unwrap();
    std::fs::write(&config.sessions.history_file, json!([{"sessionId":"omp-old-3b9f-exact","sessionName":"Old OMP","agentType":"omp","cwd":root.path(),"sessionFile":file,"firstRecordedAt":1}]).to_string()).unwrap();
    config
}
async fn resume(config: &herdr_bot::Config) -> common::HttpResponse {
    resume_session(config, "omp-old-3b9f-exact").await
}
async fn resume_session(config: &herdr_bot::Config, session_id: &str) -> common::HttpResponse {
    let server = TestServer::start_app(herdr_bot::app_with_power_off(config, Arc::new(NeverPowerOff))).await;
    let task = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":"Distinct Task", "chatName":"Task Chat", "agent":"omp", "model":"task-model", "thinking":"high", "prompt":"TASK INITIAL MUST NEVER BE SUBMITTED", "extensions":[]}).to_string()).await;
    assert_eq!(task.status,200,"{}",task.body);
    post_json_over_tcp(server.addr(), "/api/chats/resume", &json!({"sessionId":session_id}).to_string()).await
}
fn native_file(root: &TempDir) -> std::path::PathBuf {
    let file = root.path().join("old OMP conversation.jsonl");
    std::fs::write(&file, format!("{}\n{}\n{}\n",json!({"type":"session","version":3,"id":"omp-old-3b9f-exact","cwd":root.path()}),json!({"type":"model_change","provider":"native-provider","modelId":"native-model"}),json!({"type":"thinking_level_change","thinkingLevel":"low"}))).unwrap();
    file
}
// P6/P10/P11/P13/P14/P15/P22. Source: ticket requires exact native file,
// recorded cwd, resume (not restore), no Task settings/input, then identity and
// focus confirmation. A blank/new launch or restore flag would violate this;
// observe the complete external sequence, literal argv and actual child cwd.
#[tokio::test]
async fn omp_file_resumes_in_historical_cwd_without_task_overrides_or_input_then_focuses() {
    let root = TempDir::new("OMP resume cwd with spaces");
    let herdr = HerdrFixture::new("omp-resume-file");
    let native = native_file(&root);
    let config = setup(&root,&herdr,Some(&native));
    // No usable live match remains after skipping this stale candidate. Resume
    // must still load the selected history, not stop or start a blank Chat.
    herdr.set_behavior(json!({
        "agents":[{"agent":"omp","workspace_id":"stale","tab_id":"stale:t1","pane_id":"stale:p1",
            "agent_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":root.path().join("missing.jsonl")}}],
        "native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":native}
    }));
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

// A different native conversation must fail before creating a target, even
// though an unavailable historical cwd can now use the explicit fallback.
#[tokio::test]
async fn omp_wrong_native_source_fails_without_creating_a_target() {
    let root = TempDir::new("OMP invalid restore source");
    let herdr = HerdrFixture::new("omp-invalid-source");
    let native = root.path().join("another.jsonl");
    std::fs::write(&native,format!("{}\n",json!({"type":"session","id":"another-omp-chat","cwd":root.path()}))).unwrap();
    let config = setup(&root,&herdr,Some(native.as_path()));
    let reply = resume(&config).await;
    assert_eq!(reply.status,502,"{}",reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["stage"],"session");
    assert_eq!(body["code"],"native-session-mismatch");
    assert!(body["message"].as_str().unwrap().contains("OMP 恢复"));
    assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(),["agent_list"]);
    assert!(herdr.tabs().is_empty());
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
        assert_eq!(start["args"],json!(["--resume","omp-old-3b9f-exact"]));
        assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"));
    }
}

const STARTUP_UUID: &str = "01a18564-c4a1-700b-81b2-4ffb35f043ed";
const STARTUP_FILENAME: &str = "2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl";

fn startup_history(root: &TempDir, herdr: &HerdrFixture, file: &Path, with_provenance: bool) -> herdr_bot::Config {
    let config = setup(root, herdr, Some(file));
    let mut entry = json!({
        "sessionId":STARTUP_UUID,"sessionName":"New OMP before first write",
        "agentType":"omp","cwd":root.path(),"sessionFile":file,"firstRecordedAt":1
    });
    if with_provenance { entry["startupIdentityPath"] = json!(file); }
    std::fs::write(&config.sessions.history_file, json!([entry]).to_string()).unwrap();
    config
}

fn startup_candidate(file: &Path) -> Value {
    json!({
        "agent":"omp","name":"startup-live","workspace_id":"w-startup",
        "tab_id":"t-startup","pane_id":"p-startup","agent_status":"working",
        "agent_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":file}
    })
}

// The saved startup path is permission to locate this exact still-running
// process before its first native write, not permission to launch another one.
#[tokio::test]
async fn omp_saved_startup_path_locates_missing_native_file_without_start_or_input() {
    let root = TempDir::new("OMP saved startup lookup");
    let herdr = HerdrFixture::new("omp-startup-located");
    let file = root.path().join(STARTUP_FILENAME);
    assert!(file.is_absolute());
    assert!(!file.exists());
    let config = startup_history(&root, &herdr, &file, true);
    let before = std::fs::read(&config.sessions.history_file).unwrap();
    herdr.set_behavior(json!({"agents":[startup_candidate(&file)]}));

    let reply = resume_session(&config, STARTUP_UUID).await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"], "located");
    assert_eq!(body["target"]["paneId"], "p-startup");
    assert_eq!(body["matchCount"], 1);
    let calls = herdr.calls();
    assert_eq!(calls.iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(), ["agent_list", "agent_focus"]);
    assert_eq!(calls[1]["argv"][2], "p-startup");
    assert!(herdr.tabs().is_empty());
    assert!(!file.exists());
    assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), before);
}

// A plausible UUID filename alone is not provenance. Nor may startup permission
// transfer to another path, source, agent, or saved UUID. A normal ID-based
// restoration remains available, but the unverified live candidate is not used.
#[tokio::test]
async fn omp_missing_live_candidate_requires_exact_saved_startup_provenance() {
    for fault in ["no-provenance", "registered-provenance", "different-path", "different-source", "different-agent", "different-uuid"] {
        let root = TempDir::new("OMP startup provenance boundary");
        let herdr = HerdrFixture::new("omp-startup-provenance");
        let saved_file = root.path().join(STARTUP_FILENAME);
        let config = startup_history(&root, &herdr, &saved_file, !matches!(fault, "no-provenance" | "registered-provenance"));
        if fault == "registered-provenance" {
            let server = TestServer::start(&config).await;
            let registered = post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({
                "sessionId":STARTUP_UUID,"kind":"register","data":{
                    "agentType":"omp","cwd":root.path(),"sessionFile":saved_file,
                    "startupIdentityPath":saved_file
                }
            }).to_string()).await;
            assert_eq!(registered.status, 200, "{}", registered.body);
        }
        let candidate_file = match fault {
            "different-path" => {
                let directory = root.path().join("other-native-directory");
                std::fs::create_dir(&directory).unwrap();
                directory.join(STARTUP_FILENAME)
            }
            "different-uuid" => root.path().join("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ee.jsonl"),
            _ => saved_file.clone(),
        };
        let mut candidate = startup_candidate(&candidate_file);
        if fault == "different-source" { candidate["agent_session"]["source"] = json!("untrusted:omp"); }
        if fault == "different-agent" { candidate["agent_session"]["agent"] = json!("pi"); }
        assert!(!saved_file.exists());
        assert!(!candidate_file.exists());
        herdr.set_behavior(json!({
            "agents":[candidate],
            "native_session":{"agent":"omp","source":"herdr:omp","kind":"id","value":STARTUP_UUID}
        }));

        let reply = resume_session(&config, STARTUP_UUID).await;
        assert_eq!(reply.status, 200, "{fault}: {}", reply.body);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["outcome"], "restored", "{fault}: {}", reply.body);
        assert_ne!(body["target"]["paneId"], "p-startup", "{fault}");
        let calls = herdr.calls();
        assert!(!calls.iter().any(|c| c["kind"] == "agent_focus" && c["argv"][2] == "p-startup"), "{fault}: {calls:?}");
        let start = calls.iter().find(|c| c["kind"] == "agent_start").unwrap();
        assert_eq!(start["args"], json!(["--resume", STARTUP_UUID]), "{fault}");
        assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"), "{fault}");
    }
}

// A startup mapping may locate its current process, but is never evidence
// that a newly launched restore process loaded the same conversation.
#[tokio::test]
async fn omp_startup_path_cannot_confirm_a_new_restore_process_without_native_file() {
    let root = TempDir::new("OMP startup strict restore");
    let herdr = HerdrFixture::new("omp-startup-strict-restore");
    let file = root.path().join(STARTUP_FILENAME);
    let config = startup_history(&root, &herdr, &file, true);
    herdr.set_behavior(json!({
        "agents":[],"native_cwd":root.path(),
        "native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":file}
    }));
    let before = std::fs::read(&config.sessions.history_file).unwrap();
    let reply = resume_session(&config, STARTUP_UUID).await;
    assert_eq!(reply.status, 502, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"], "unconfirmed");
    assert_eq!(body["stage"], "confirm");
    assert_eq!(body["progress"]["conversationConfirmed"], false);
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    assert!(!calls.iter().any(|call| call["kind"] == "agent_focus" || call["kind"] == "agent_prompt"));
    assert!(!file.exists());
    assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), before);
}

// Once the first write exists, its real header wins over the filename and
// startup provenance. A contradiction is a lookup failure, not a stale target
// to skip before silently creating a replacement process.
#[tokio::test]
async fn omp_startup_path_with_conflicting_native_header_rejects_lookup_without_history_change() {
    let root = TempDir::new("OMP startup header conflict");
    let herdr = HerdrFixture::new("omp-startup-header-conflict");
    let file = root.path().join(STARTUP_FILENAME);
    let config = startup_history(&root, &herdr, &file, true);
    let before = std::fs::read(&config.sessions.history_file).unwrap();
    std::fs::write(&file, format!("{}\n", json!({
        "type":"session","version":3,"id":"01a18564-c4a1-700b-81b2-4ffb35f043ee","cwd":root.path()
    }))).unwrap();
    herdr.set_behavior(json!({"agents":[startup_candidate(&file)]}));

    let reply = resume_session(&config, STARTUP_UUID).await;
    assert_eq!(reply.status, 502, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["outcome"], "failed");
    assert_eq!(body["stage"], "lookup");
    assert_eq!(body["code"], "identity-verification-failed");
    assert_eq!(herdr.calls().iter().map(|c| c["kind"].as_str().unwrap()).collect::<Vec<_>>(), ["agent_list"]);
    assert!(herdr.tabs().is_empty());
    assert_eq!(std::fs::read(&config.sessions.history_file).unwrap(), before);
}
