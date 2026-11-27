//! Approved seams S2 HTTP, S3 controlled Herdr, S4 actual records.
//! Expected sequence: all three records exist before external start, native
//! identity is persisted before prompt. A prompt-first implementation violates
//! P13 even if its final response happens to contain a session ID.
#[path = "../common/mod.rs"]
mod common;
#[path = "recovery/app_roles.rs"]
mod app_roles;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

async fn fixture() -> (
    TempDir,
    HerdrFixture,
    herdr_bot::Config,
    TestServer,
    String,
    Vec<String>,
) {
    fixture_for_agent("omp").await
}

async fn fixture_for_agent(agent: &str) -> (TempDir, HerdrFixture, herdr_bot::Config, TestServer, String, Vec<String>) {
    fixture_with_host(agent, false).await
}

async fn fixture_with_host(agent: &str, app: bool) -> (TempDir, HerdrFixture, herdr_bot::Config, TestServer, String, Vec<String>) {
    let data = TempDir::new("workflow start records");
    let herdr = HerdrFixture::new("workflow manager external");
    let zone_path = if app {
        let dir = data.path().join("alpha");
        fs::create_dir(&dir).unwrap();
        fs::write(herdr.path().join("chat-app-mode"), "").unwrap();
        dir
    } else { data.path().to_owned() };
    let fixtures = common::repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations/success.bat"),
        &fixtures.join("operations"),
    );
    config.tasks.storage_file = data.path().join("tasks.json");
    config.sessions.history_file = data.path().join("chat-history.json");
    config.prompts_file = data.path().join("prompts.local.toml");
    let mut catalog: toml::Value =
        toml::from_str(&fs::read_to_string(&config.tasks.metadata_file).unwrap()).unwrap();
    for entry in catalog["extensions"].as_array_mut().unwrap() {
        let path = data
            .path()
            .join(format!("{}.ts", entry["id"].as_str().unwrap()));
        fs::write(&path, "export default function () {}\n").unwrap();
        entry["entrypoint"] = toml::Value::String(path.to_string_lossy().into_owned());
    }
    config.tasks.metadata_file = data.path().join("options.toml");
    fs::write(
        &config.tasks.metadata_file,
        toml::to_string(&catalog).unwrap(),
    )
    .unwrap();
    config.zones.registry_file = data.path().join("zones.json");
    config.scheduler.map_directory = data.path().join("maps");
    config.scheduler.bot_directory = data.path().join("bots");
    config.scheduler.run_directory = data.path().join("nondefault-runs");
    config.scheduler.run_instructions = data.path().join("runtime-instructions.md");
    fs::copy(
        fixtures.parent().unwrap().join("docs/workflow-runtime.md"),
        &config.scheduler.run_instructions,
    )
    .unwrap();
    config.herdr.executable = herdr.executable().to_path_buf();
    if app {
        config.herdr.backend = herdr_bot::config::HerdrBackend::App;
        config.herdr.app_executable = herdr.executable().into();
        config.herdr.core_executable = herdr.executable().into();
    }
    herdr.set_workspaces(json!([{"workspace_id":"w-manager", "label":data.path().file_name().unwrap().to_str().unwrap()}]));
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp", "agent":"omp", "kind":"id", "value":"manager-native-04"}, "native_cwd":data.path()}));
    fs::create_dir_all(&config.scheduler.map_directory).unwrap();
    fs::create_dir_all(&config.scheduler.bot_directory).unwrap();
    let server = TestServer::start(&config).await;
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({"id":"manager","path":zone_path}).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "{}", response.body);
    let zone = serde_json::from_str::<Value>(&response.body).unwrap()["zones"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut ids = Vec::new();
    for role in ["manager", "node"] {
        let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":if role == "node" {"normal"} else {"manager"},"agent":agent,"chatName":"{chat.id}","model":"default","thinking":"default","skill":null,"prompt":"FIXED-MANAGER-ROLE"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        ids.push(
            body["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == role)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    fs::write(config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"","plan_guide":"","node_guide":"","task_id":ids[1]}).to_string()).unwrap();
    fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"MAP-BODY-NOT-INJECTED","nodes":{"A":{"title":"A","bot_id":"worker","input":"NODE-INPUT-NOT-INJECTED","zone":zone,"blocked_by":[]}}}).to_string()).unwrap();
    (data, herdr, config, server, zone, ids)
}

// S1: public Run/start and session-event HTTP; actual native header, public
// Run/detail, durable Chat history, and receive-side command at the external
// App/core boundary. A legacy TUI-only launch or a launch that sends from the
// create reply/identity event would fail before the first business turn.
#[tokio::test]
async fn app_root_run_waits_for_native_readiness_and_persists_before_business_input() {
    let (data, herdr, config, server, zone, ids) = fixture_with_host("omp", true).await;
    let cwd = data.path().join("alpha");
    let native_file = cwd.join("root-session.jsonl");
    fs::write(&native_file, format!("{}\n", json!({"type":"session","id":"app-root-omp","cwd":cwd}))).unwrap();
    let addr = server.addr();
    let manager = ids[0].clone();
    let start = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":manager,"zone":zone,"userInput":"Root business before any other turn"}).to_string()).await
    });
    let tab = app_tab(&herdr, 1).await;
    assert_eq!(tab["space"], "space-current");
    assert_eq!(tab["options"]["folder"], json!(cwd));
    let argv = tab["options"]["argv"].as_array().expect("App must start the actual agent in its new tab");
    assert_eq!(argv[8], "omp");
    assert!(!argv.iter().any(|arg| arg.as_str().is_some_and(|arg| arg.contains("Root business"))));
    let event = |kind| json!({"kind":kind,"sessionId":"app-root-omp","data":{
        "launchId":argv[6],"targetId":argv[7],"agentType":"omp","cwd":cwd,"pid":43127,"sessionFile":native_file,
    }}).to_string();
    let root = fs::read_dir(&config.scheduler.run_directory).unwrap().next().unwrap().unwrap().path();
    let run_id = root.file_name().unwrap().to_str().unwrap();
    let detail = format!("/api/workflow/runs/{run_id}");
    fs::write(herdr.path().join("observe-app-send.json"),
        json!([root.join("run.json"), config.sessions.history_file]).to_string()).unwrap();
    let initial: Value = serde_json::from_str(&get_over_tcp(addr, &detail).await.body).unwrap();
    assert!(initial["run"]["manager"].is_null());
    let send = || herdr.calls().into_iter().filter(|call| call["kind"] == "core_send").collect::<Vec<_>>();
    let identity = post_json_over_tcp(addr, "/api/sessions/events", &event("launch_identity")).await;
    assert_eq!(identity.status, 200, "{}", identity.body);
    assert!(!start.is_finished());
    assert!(send().is_empty());
    let ready = post_json_over_tcp(addr, "/api/sessions/events", &event("launch_ready")).await;
    assert_eq!(ready.status, 200, "{}", ready.body);
    let started = start.await.unwrap();
    assert_eq!(started.status, 201, "{}", started.body);
    let saved: Value = serde_json::from_str(&get_over_tcp(addr, &detail).await.body).unwrap();
    let manager = &saved["run"]["manager"];
    assert_eq!(manager["sessionId"], "app-root-omp");
    assert_eq!(manager["tabId"], tab["tab"]);
    assert_eq!(manager["paneId"], tab["pane"]);
    let history: Value = serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let row = history.as_array().unwrap().iter().find(|row| row["sessionId"] == "app-root-omp").unwrap();
    assert_eq!(row["sourceTaskId"], ids[0]);
    assert_eq!(row["sessionFile"], json!(native_file));
    assert_eq!(row["originalWorkspaceId"], "space-current");
    let sent = send();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["terminal"], tab["terminal"]);
    let observed_run: Value = serde_json::from_str(sent[0]["observedFiles"][0].as_str().unwrap()).unwrap();
    assert_eq!(observed_run["manager"], *manager);
    let observed_history: Value = serde_json::from_str(sent[0]["observedFiles"][1].as_str().unwrap()).unwrap();
    assert_eq!(observed_history, history);
    assert!(sent[0]["text"].as_str().unwrap().ends_with("Root business before any other turn"));
}

// S1: exact saved Run target, not just a successful App focus reply. Unknown
// execution IDs must not focus the manager or create another terminal.
#[tokio::test]
async fn app_run_locate_checks_the_saved_target_and_actual_pane_focus() {
    let (data, herdr, _, server, zone, ids) = fixture_with_host("pi", true).await;
    let pending = start_fixture_run(&server, ids[0].clone(), zone);
    let tab = app_launch(&server, &herdr, 1, "pi", "app-locate-manager", &data.path().join("alpha")).await;
    let run = run_reply(pending.await.unwrap(), 201);
    let route = format!("/api/workflow/runs/{}/locate", run["id"].as_str().unwrap());
    let wrong = run_reply(post_json_over_tcp(server.addr(), &route, r#"{"executionId":"unknown-execution"}"#).await, 404);
    assert_eq!(wrong["code"], "run-chat-not-found");
    assert!(!herdr.path().join("app-focused.json").exists());
    let located = run_reply(post_json_over_tcp(server.addr(), &route, "{}").await, 200);
    assert_eq!(located["outcome"], "located");
    assert_eq!(located["target"]["paneId"], tab["pane"]);
    assert_eq!(fs::read_to_string(herdr.path().join("app-focused.json")).unwrap(), tab.to_string());
    herdr.set_behavior(json!({"unconfirmed_focus":true}));
    let failed = run_reply(post_json_over_tcp(server.addr(), &route, "{}").await, 500);
    assert_eq!(failed["code"], "run-chat-focus-failed");
    assert_eq!(common::read_json_lines(&herdr.path().join("app-tabs.jsonl")).len(), 1);
}

#[tokio::test]
async fn tui_run_locate_does_not_focus_a_same_identity_decoy() {
    let (data, herdr, _, server, zone, ids) = fixture().await;
    let pending = run_reply(start_fixture_run(&server, ids[0].clone(), zone).await.unwrap(), 201);
    let tab = common::read_json_lines(&herdr.path().join("started.jsonl")).pop().unwrap();
    let mut live = tab.clone();
    live["agent_session"] = json!({"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-native-04"});
    let mut decoy = live.clone();
    decoy["workspace_id"] = json!("a-decoy-workspace");
    decoy["tab_id"] = json!("a-decoy-tab");
    decoy["pane_id"] = json!("a-decoy-pane");
    herdr.set_behavior(json!({"agents":[decoy, live],"native_cwd":data.path()}));
    let url = format!("/api/workflow/runs/{}/locate", pending["id"].as_str().unwrap());
    let located = run_reply(post_json_over_tcp(server.addr(), &url, "{}").await, 200);
    assert_eq!(located["target"]["paneId"], tab["pane_id"]);
    let focused = herdr.path().join("focused.json");
    assert_eq!(fs::read_to_string(&focused).unwrap(), tab["pane_id"].to_string());
    fs::remove_file(&focused).unwrap();
    herdr.set_behavior(json!({"agents":[decoy]}));
    let wrong = run_reply(post_json_over_tcp(server.addr(), &url, "{}").await, 404);
    assert_eq!(wrong["code"], "run-chat-not-found");
    assert!(!focused.exists(), "an exact identity in a different Pane was not authorized");
    assert_eq!(herdr.tabs().len(), 1);
}

// S1/P40/P52: a real saved report survives an explicit external send failure.
// Counting the initial submits plus the rejected manager-terminal notification
// distinguishes one delivery from no attempt as well as from automatic replay.
#[tokio::test]
async fn app_run_report_failure_preserves_public_readback_without_replaying() {
    use std::time::{Duration, Instant};
    let (data, herdr, config, server, zone, ids) = fixture_with_host("omp", true).await;
    let cwd = data.path().join("alpha");
    let pending = start_fixture_run(&server, ids[0].clone(), zone);
    let manager_tab = app_launch(&server, &herdr, 1, "omp", "report-manager", &cwd).await;
    let started = run_reply(pending.await.unwrap(), 201);
    prepare_run(&server, "report-manager").await;
    let run_id = started["id"].as_str().unwrap();
    let root = config.scheduler.run_directory.join(run_id);
    fs::write(herdr.path().join("observe-app-send.json"),
        json!([root.join("run.json"), config.sessions.history_file, root.join("messages.jsonl")]).to_string()).unwrap();
    let dispatch = dispatch_app_node(&server, "report-manager", "A");
    let node_tab = app_launch(&server, &herdr, 2, "omp", "report-worker", &cwd).await;
    let dispatched = run_reply(dispatch.await.unwrap(), 200);
    assert_eq!(dispatched["results"][0]["status"], "running");
    let sends = || herdr.calls().into_iter().filter(|call| call["kind"] == "core_send").collect::<Vec<_>>();
    let startup = sends();
    assert_eq!(startup.len(), 2);
    assert_eq!(startup[1]["terminal"], node_tab["terminal"]);
    assert!(startup[1]["text"].as_str().unwrap().contains("NODE-INPUT-NOT-INJECTED"));
    let observed_run: Value = serde_json::from_str(startup[1]["observedFiles"][0].as_str().unwrap()).unwrap();
    let execution = &observed_run["nodes"]["A"]["execution"];
    assert_eq!(execution["chat"]["sessionId"], "report-worker");
    assert_eq!(execution["chat"]["paneId"], node_tab["pane"]);
    let observed_history: Value = serde_json::from_str(startup[1]["observedFiles"][1].as_str().unwrap()).unwrap();
    assert!(observed_history.as_array().unwrap().iter().any(|row| row["sessionId"] == "report-worker" && row["sourceTaskId"] == ids[1]));
    let node_argv = node_tab["options"]["argv"].as_array().unwrap();
    assert!(node_argv.iter().any(|arg| arg.as_str().is_some_and(|value| value.ends_with("omp-node-chat.ts"))));

    herdr.set_behavior(json!({"fail_app_send":true}));
    let report = run_reply(post_json_over_tcp(server.addr(), "/api/workflow/node/report", &json!({
        "sessionId":"report-worker","body":"Saved despite refusing delivery","canComplete":false
    }).to_string()).await, 200);
    assert_eq!(report["saved"], true);
    assert_eq!(report["notification"]["status"], "failed");
    assert_eq!(report["report"]["executionId"], execution["id"]);
    assert_eq!(report["report"]["to"]["sessionId"], "report-manager");
    let attempted = sends();
    assert_eq!(attempted.len(), 3);
    assert_eq!(attempted[2]["terminal"], manager_tab["terminal"]);
    assert!(attempted[2]["text"].as_str().unwrap().ends_with("Saved despite refusing delivery"));
    let persisted_at_attempt = attempted[2]["observedFiles"][2].as_str().unwrap().lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()).collect::<Vec<_>>();
    assert_eq!(persisted_at_attempt, vec![report["report"].clone()]);
    let started = Instant::now();
    loop {
        let detail = run_reply(get_over_tcp(server.addr(), &format!("/api/workflow/runs/{run_id}")).await, 200);
        assert_eq!(detail["reports"], json!([report["report"]]));
        assert_eq!(detail["run"]["nodes"]["A"]["execution"], *execution);
        assert_eq!(sends(), attempted);
        assert_eq!(common::read_json_lines(&herdr.path().join("app-tabs.jsonl")).len(), 2);
        if started.elapsed() > Duration::from_secs(6) { break; }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

// S1/P23/P39/P41: the child manager has a dual role at one actual Pane;
// the end Chat is a separate user handoff, not a reporting node. Observe the
// complete parent/child link at the initial terminal interaction, not only in
// dispatch results, and use the saved execution identity for the focus action.
#[tokio::test]
async fn app_nested_manager_and_end_chat_keep_separate_run_roles() {
    let (data, herdr, config, server, zone, ids) = fixture_with_host("pi", true).await;
    let cwd = data.path().join("alpha");
    fs::write(config.scheduler.bot_directory.join("nested.json"), json!({
        "id":"nested","name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"
    }).to_string()).unwrap();
    fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"Root","description":"","nodes":{
        "N":{"title":"Nested","bot_id":"nested","manager_task_id":ids[0],"input":"Nested business input","zone":zone,"blocked_by":[]}
    }}).to_string()).unwrap();
    fs::write(config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{
        "E":{"title":"Handoff","bot_id":"worker","input":"User takes it from here","zone":zone,"blocked_by":[],"end_node":true}
    }}).to_string()).unwrap();
    let pending = start_fixture_run(&server, ids[0].clone(), zone);
    app_launch(&server, &herdr, 1, "pi", "parent-manager", &cwd).await;
    let started = run_reply(pending.await.unwrap(), 201);
    let parent_id = started["id"].as_str().unwrap();
    prepare_run(&server, "parent-manager").await;

    let nested = dispatch_app_node(&server, "parent-manager", "N");
    let parent_file = config.scheduler.run_directory.join(parent_id).join("run.json");
    app_tab(&herdr, 2).await;
    let begun: Value = serde_json::from_slice(&fs::read(&parent_file).unwrap()).unwrap();
    let child_id = begun["nodes"]["N"]["execution"]["childRunId"].as_str().unwrap();
    let child_file = config.scheduler.run_directory.join(child_id).join("run.json");
    let observations = herdr.path().join("observe-app-send.json");
    fs::write(&observations, json!([parent_file, child_file, config.sessions.history_file]).to_string()).unwrap();
    let tab = app_launch(&server, &herdr, 2, "pi", "nested-manager", &cwd).await;
    let nested = run_reply(nested.await.unwrap(), 200);
    assert_eq!(nested["results"][0]["status"], "running");
    let sends = || herdr.calls().into_iter().filter(|call| call["kind"] == "core_send").collect::<Vec<_>>();
    let sent = sends();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1]["terminal"], tab["terminal"]);
    assert!(sent[1]["text"].as_str().unwrap().contains("[Child Run manager context]"));
    assert!(sent[1]["text"].as_str().unwrap().contains("Nested business input"));
    let observed = |index: usize| serde_json::from_str::<Value>(sent[1]["observedFiles"][index].as_str().unwrap()).unwrap();
    let parent = observed(0);
    let execution = &parent["nodes"]["N"]["execution"];
    let child = observed(1);
    assert_eq!(child["parent"], json!({"runId":parent_id,"nodeId":"N","executionId":execution["id"]}));
    assert_eq!(child["manager"], execution["chat"]);
    assert_eq!(child["manager"]["sessionId"], "nested-manager");
    assert_eq!(child["manager"]["paneId"], tab["pane"]);
    assert!(observed(2).as_array().unwrap().iter().any(|row| row["sessionId"] == "nested-manager" && row["sourceTaskId"] == ids[0]));
    let extensions = |tab: &Value| tab["options"]["argv"].as_array().unwrap().iter().filter_map(Value::as_str)
        .filter_map(|argument| std::path::Path::new(argument).file_name()?.to_str().map(str::to_owned)).collect::<Vec<_>>();
    for extension in ["pi-manager-chat.ts", "pi-node-chat.ts"] { assert!(extensions(&tab).iter().any(|name| name == extension)); }
    let role = run_reply(post_json_over_tcp(server.addr(), "/api/workflow/node/context", r#"{"sessionId":"nested-manager"}"#).await, 200);
    assert_eq!(role["runId"], parent_id);
    assert_eq!(role["executionId"], execution["id"]);
    prepare_run(&server, "nested-manager").await;

    let end = dispatch_app_node(&server, "nested-manager", "E");
    let end_tab = app_launch(&server, &herdr, 3, "pi", "user-end-chat", &cwd).await;
    let ended = run_reply(end.await.unwrap(), 200);
    let end_execution = &ended["snapshot"]["nodes"]["E"]["execution"];
    assert_eq!(end_execution["endNode"], true);
    assert_eq!(end_execution["approved"], false);
    assert_eq!(end_execution["chat"]["sessionId"], "user-end-chat");
    assert!(extensions(&end_tab).iter().all(|name| !matches!(name.as_str(), "pi-manager-chat.ts" | "pi-node-chat.ts")));
    let sent = sends();
    assert_eq!(sent.len(), 3);
    let input = sent[2]["text"].as_str().unwrap();
    assert!(input.contains("User takes it from here") && !input.contains("[Result approval control]"));
    let before_end: Value = serde_json::from_str(sent[2]["observedFiles"][1].as_str().unwrap()).unwrap();
    assert_eq!(&before_end["nodes"]["E"]["execution"], end_execution);
    run_reply(post_json_over_tcp(server.addr(), "/api/workflow/node/report",
        r#"{"sessionId":"user-end-chat","body":"Not a reporting node","canComplete":true}"#).await, 403);
    let finished = run_reply(post_json_over_tcp(server.addr(), "/api/workflow/manager/finish",
        r#"{"sessionId":"nested-manager"}"#).await, 200);
    assert_eq!(finished["snapshot"]["finished"]["reason"], "handed-to-user");
    assert_eq!(finished["snapshot"]["nodes"]["E"]["status"], "running");
    let located = run_reply(post_json_over_tcp(server.addr(), &format!("/api/workflow/runs/{child_id}/locate"),
        &json!({"executionId":end_execution["id"]}).to_string()).await, 200);
    assert_eq!(located["target"]["paneId"], end_tab["pane"]);
    assert_eq!(fs::read_to_string(herdr.path().join("app-focused.json")).unwrap(), end_tab.to_string());
    assert_eq!(sends(), sent);
}

// S1/P23/P39: real chief preview/confirmation chooses a Run, not an
// independent Task Chat. At terminal receipt the delegation must already grant
// the same native manager the authority represented by the saved Run/history.
#[tokio::test]
async fn app_delegated_map_binds_manager_before_approved_input() {
    let (data, herdr, config, server, zone, ids) = fixture_with_host("omp", true).await;
    let addr = server.addr();
    let cwd = data.path().join("alpha");
    let chief = run_reply(post_json_over_tcp(addr, "/api/tasks", &json!({
        "name":"Delegation entry","taskType":"chief","agent":"omp","chatName":"Chief {chat.id}",
        "model":"default","thinking":"default","skill":null,"prompt":"Clarify only"
    }).to_string()).await, 200);
    let chief_id = &chief["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "Delegation entry").unwrap()["id"];
    let preview = run_reply(post_json_over_tcp(addr, "/api/chats/preview",
        &json!({"zone":zone,"taskId":chief_id}).to_string()).await, 200);
    let accepted = run_reply(post_json_over_tcp(addr, "/api/chats",
        &json!({"zone":zone,"taskId":chief_id,"chatId":preview["chatId"]}).to_string()).await, 202);
    app_launch(&server, &herdr, 1, "omp", "delegating-chief", &cwd).await;
    run_reply(post_json_over_tcp(addr, "/api/sessions/events", &json!({"sessionId":"delegating-chief","kind":"register",
        "data":{"cwd":cwd,"agentType":"omp"}}).to_string()).await, 200);
    let stage_url = format!("/api/chats/{}", accepted["id"].as_str().unwrap());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let stage = run_reply(get_over_tcp(addr, &stage_url).await, 200);
            if stage["stage"] == "succeeded" { break; }
            assert_ne!(stage["stage"], "failed", "{stage}");
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }).await.unwrap();
    fs::write(config.scheduler.bot_directory.join("flow.json"), json!({
        "id":"flow","name":"Delegated workflow","when":"Use the workflow","plan_guide":"Explicit input","node_guide":"Explicit client",
        "map_file":"work.json"
    }).to_string()).unwrap();
    let preview = run_reply(post_json_over_tcp(addr, "/api/chief/delegations/preview", &json!({
        "sessionId":"delegating-chief","botId":"flow","managerTaskId":ids[0],"zoneId":zone,
        "input":"Approved delegated business input only","materials":[]
    }).to_string()).await, 200);
    let run_id = preview["previewId"].as_str().unwrap();
    let body = json!({"sessionId":"delegating-chief","previewId":run_id}).to_string();
    let run_file = config.scheduler.run_directory.join(run_id).join("run.json");
    fs::write(herdr.path().join("observe-app-send.json"), json!([
        run_file, config.sessions.history_file, config.sessions.history_file.with_extension("delegations.json")
    ]).to_string()).unwrap();
    let commit = tokio::spawn(async move { post_json_over_tcp(addr, "/api/chief/delegations", &body).await });
    let tab = app_launch(&server, &herdr, 2, "omp", "delegated-manager", &cwd).await;
    let started = run_reply(commit.await.unwrap(), 201);
    assert_eq!(started["id"], run_id);
    let sends = herdr.calls().into_iter().filter(|call| call["kind"] == "core_send").collect::<Vec<_>>();
    assert_eq!(sends.len(), 2);
    assert_eq!(sends[1]["terminal"], tab["terminal"]);
    assert_eq!(sends[1]["text"], preview["facts"]["prompt"]);
    assert!(sends[1]["text"].as_str().unwrap().contains("Approved delegated business input only"));
    let observed = |index: usize| serde_json::from_str::<Value>(sends[1]["observedFiles"][index].as_str().unwrap()).unwrap();
    assert_eq!(observed(0)["manager"], started["run"]["manager"]);
    assert_eq!(observed(0)["manager"]["sessionId"], "delegated-manager");
    assert!(observed(2).as_array().unwrap().iter().any(|record| record["runId"] == run_id && record["sessionId"] == "delegated-manager"));
    assert!(observed(1).as_array().unwrap().iter().any(|row| row["sessionId"] == "delegated-manager" && row["sourceTaskId"] == ids[0]));
    let sender = run_reply(get_over_tcp(addr, "/api/chief/delegations?sessionId=delegated-manager").await, 200);
    assert_eq!(sender["delegations"][0]["runId"], run_id);
    assert_eq!(sender["delegations"][0]["creation"]["inputSubmitted"], true);

    // P42: Resume the exact approved Map delegation; it is still only a
    // manager launch, not an independent delegated or ordinary node Chat.
    let runs_before = run_reply(get_over_tcp(addr, "/api/workflow/runs").await, 200);
    assert_eq!(runs_before["runs"].as_array().unwrap().len(), 1);
    assert_eq!(runs_before["runs"][0]["run"]["manager"]["sessionId"], "delegated-manager");
    let native = cwd.join("delegated-manager.jsonl");
    app_roles::app_role_event(&server, &tab, "delegated-manager", &native, "launch_ended").await;
    app_roles::restore_app_role(&server, &herdr, 3, "delegated-manager", &native, &["manager-chat"]).await;
    assert_eq!(run_reply(get_over_tcp(addr, "/api/workflow/runs").await, 200), runs_before);
    assert_eq!(run_reply(get_over_tcp(addr, "/api/chief/delegations?sessionId=delegated-manager").await, 200), sender);
    assert_eq!(herdr.calls().into_iter().filter(|call| call["kind"] == "core_send").collect::<Vec<_>>(), sends);
}

// S1 (P43–P45): the public manager completion, controlled App/core CLI
// and its independent receipt/layout/process records. A Space-wide close
// would affect the manager on the first pass; without App confirmation the
// same exact Tab request would be refused when it is the last one.
#[tokio::test]
async fn precise_auto_close_app_targets_tab_and_confirms_only_tab_scope() {
    use std::time::Duration;
    for (last_tab, rejection) in [
        (false, None),
        (true, None),
        (false, Some(("change_app_core", "run-chat-lookup-failed", "core 实例已改变"))),
        (false, Some(("fail_app_tab_close", "herdr-command-failed", "controlled App close refused"))),
    ] {
        let (data, herdr, config, server, zone, ids) = fixture_with_host("omp", true).await;
        let cwd = data.path().join("alpha");
        fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"","nodes":{
            "A":{"title":"A","bot_id":"worker","input":"Approved close target","zone":zone,"blocked_by":[],"auto_close":true}
        }}).to_string()).unwrap();
        let start = start_fixture_run(&server, ids[0].clone(), zone);
        let manager = app_launch(&server, &herdr, 1, "omp", "close-app-manager", &cwd).await;
        let started = run_reply(start.await.unwrap(), 201);
        prepare_run(&server, "close-app-manager").await;
        let dispatch = dispatch_app_node(&server, "close-app-manager", "A");
        let node = app_launch(&server, &herdr, 2, "omp", "close-app-worker", &cwd).await;
        let dispatched = run_reply(dispatch.await.unwrap(), 200);
        assert_eq!(dispatched["results"][0]["status"], "running");
        let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
        assert_eq!(execution["chat"]["tabId"], node["tab"]);
        assert_eq!(execution["chat"]["paneId"], node["pane"]);
        assert_eq!(node["space"], manager["space"]);
        run_reply(post_json_over_tcp(server.addr(), "/api/workflow/node/approve", &json!({
            "sessionId":"close-app-worker","executionId":execution["id"],"source":"auto"
        }).to_string()).await, 200);
        run_reply(post_json_over_tcp(server.addr(), "/api/workflow/node/report", r#"{"sessionId":"close-app-worker","body":"Ready for acceptance","canComplete":true}"#).await, 200);
        if last_tab {
            // An independent layout change before completion; no Run or Chat
            // record is edited. The target is now the Space's only Tab.
            fs::write(herdr.path().join("app-removed-tabs.jsonl"), format!("{}\n", json!({"tab":manager["tab"],"space":manager["space"],"spaceRemoved":false}))).unwrap();
        }
        fs::write(herdr.path().join("app-unrelated-terminal"), "").unwrap();
        let observe = |args: &[&str]| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&herdr.path(), &mut hash);
            let endpoint = if cfg!(windows) {
                format!(r"\\.\pipe\herdr-core-fixture-{:016x}.sock", std::hash::Hasher::finish(&hash))
            } else { herdr.path().join("core.sock").to_string_lossy().into_owned() };
            let output = std::process::Command::new(herdr.executable()).env("HC_SOCKET", endpoint).args(args).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        };
        let state_of = |terminal: &Value| observe(&["get", terminal.as_str().unwrap(), "--json"])["state"].clone();
        let records = |name: &str| common::read_json_lines(&herdr.path().join(name));
        let before = herdr.calls().len();
        let before_invocations = records("invocations.jsonl").len();
        let mutations = || records("invocations.jsonl").into_iter().skip(before_invocations)
            .filter(|row| !matches!(row["argv"][0].as_str(), Some("ls" | "get" | "server"))).count();
        let close_call = json!({"kind":"app_tab_close","request":{"id":node["tab"],"confirmed":true},"terminal":node["terminal"]});
        let read_run = format!("/api/workflow/runs/{}", started["id"].as_str().unwrap());
        let complete_request = r#"{"sessionId":"close-app-manager","nodeId":"A","reason":"Only this worker was accepted"}"#;
        if let Some((behavior, _, _)) = rejection {
            herdr.set_behavior(json!({(behavior): true}));
        }
        let complete = run_reply(post_json_over_tcp(server.addr(), "/api/workflow/manager/complete", complete_request).await, 200);
        assert_eq!(complete["snapshot"]["nodes"]["A"]["status"], "completed");
        if let Some((behavior, code, reason)) = rejection {
            let detail = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let detail = run_reply(get_over_tcp(server.addr(), &read_run).await, 200);
                    if detail["run"]["nodes"]["A"]["autoCloseFailure"].is_object() { break detail; }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }).await.expect("public Run must expose the App close failure");
            let saved = &detail["run"]["nodes"]["A"];
            assert_eq!(saved["status"], "completed");
            assert_eq!(saved["completion"], complete["snapshot"]["nodes"]["A"]["completion"]);
            assert_eq!(saved["autoCloseFailure"]["code"], code);
            assert!(saved["autoCloseFailure"]["message"].as_str().unwrap().contains(reason), "{saved}");
            assert!(records("app-close-receipts.jsonl").is_empty());
            assert!(records("app-removed-tabs.jsonl").is_empty());
            assert_eq!(state_of(&node["terminal"]), "live");
            let duplicate = run_reply(post_json_over_tcp(server.addr(), "/api/workflow/manager/complete", complete_request).await, 200);
            assert_eq!(duplicate["changed"], false);
            assert_eq!(duplicate["snapshot"]["nodes"]["A"], *saved);
            let calls = herdr.calls();
            if behavior == "change_app_core" {
                assert_eq!(calls.len(), before, "short terminal addresses cannot authorize another core");
                assert_eq!(mutations(), 0);
            } else {
                assert_eq!(&calls[before..], [close_call], "a refusal must not trigger another command");
                assert_eq!(mutations(), 1);
            }
            continue;
        }
        assert!(common::wait_until(Duration::from_secs(3), || !records("app-close-receipts.jsonl").is_empty()).await,
            "no App close receipt: {:?}", &herdr.calls()[before..]);
        let receipts = records("app-close-receipts.jsonl");
        assert_eq!(receipts, [json!({"tab":node["tab"],"terminal":node["terminal"]})]);
        let layout = observe(&["ls"]);
        let space = layout["spaces"].as_array().unwrap().iter().find(|space| space["id"] == node["space"]);
        if last_tab {
            assert!(space.is_none(), "the now-empty Space may close");
        } else {
            assert_eq!(space.unwrap()["tabs"].as_array().unwrap().iter().map(|t| &t["id"]).collect::<Vec<_>>(), [&manager["tab"]]);
        }
        assert_eq!(state_of(&node["terminal"]), "live", "the receipt and layout loss are not a terminal exit");
        fs::write(herdr.path().join("app-release-terminal-exit"), "").unwrap();
        assert_eq!(state_of(&node["terminal"]), "dead");
        assert_eq!(state_of(&manager["terminal"]), "live");
        assert_eq!(state_of(&json!("t-unrelated")), "live");
        let calls = herdr.calls();
        assert_eq!(&calls[before..], [close_call]);
        assert_eq!(mutations(), 1, "no separate space-close, park, settle, or fallback command");
        let detail = run_reply(get_over_tcp(server.addr(), &read_run).await, 200);
        assert_eq!(detail["run"]["nodes"]["A"], complete["snapshot"]["nodes"]["A"]);
        assert_eq!(detail["run"]["nodes"]["A"]["execution"]["approved"], true);
        assert_eq!(detail["run"]["nodes"]["A"]["execution"]["id"], execution["id"]);
    }
}

async fn prepare_run(server: &TestServer, session: &str) {
    run_reply(post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs", &json!({
        "sessionId":session,"fileInputs":[],"confirmed":true,"reason":"Verified Run fixture inputs"
    }).to_string()).await, 200);
}

fn dispatch_app_node(server: &TestServer, manager: &str, node: &str) -> tokio::task::JoinHandle<common::HttpResponse> {
    let addr = server.addr();
    let body = json!({"sessionId":manager,"nodes":[node]}).to_string();
    tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &body).await })
}

fn start_fixture_run(server: &TestServer, manager: String, zone: String) -> tokio::task::JoinHandle<common::HttpResponse> {
    let addr = server.addr();
    tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":manager,"zone":zone,"userInput":"App Run requirement"}).to_string()).await
    })
}

async fn app_tab(herdr: &HerdrFixture, n: usize) -> Value {
    let log = herdr.path().join("app-tabs.jsonl");
    let mut found = None;
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        found = fs::read_to_string(&log).ok().and_then(|text| serde_json::from_str(text.lines().nth(n - 1)?).ok());
        found.is_some()
    }).await, "App did not create tab {n}");
    found.unwrap()
}

async fn app_launch(server: &TestServer, herdr: &HerdrFixture, n: usize, agent: &str, session: &str, cwd: &std::path::Path) -> Value {
    let tab = app_tab(herdr, n).await;
    let argv = tab["options"]["argv"].as_array().unwrap();
    assert_eq!(argv[8], agent);
    let file = cwd.join(format!("{session}.jsonl"));
    fs::write(&file, format!("{}\n", json!({"type":"session","id":session,"cwd":cwd}))).unwrap();
    for kind in ["launch_identity", "launch_ready"] {
        run_reply(post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({"kind":kind,"sessionId":session,"data":{
            "launchId":argv[6],"targetId":argv[7],"agentType":agent,"cwd":cwd,"pid":43127,"sessionFile":file,
        }}).to_string()).await, 200);
    }
    tab
}

fn run_reply(response: common::HttpResponse, status: u16) -> Value {
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// Chief P3 / S2: normal remains the node target, manager the Run target.
// A broad "not normal means manager" (or "not manager means normal") check
// would accept chief. Observe rejection at both public preflight and start;
// the controlled Herdr boundary must never be called for these invalid roles.
#[tokio::test]
async fn chief_is_neither_a_node_target_nor_a_run_manager() {
    let (_data, herdr, _config, server, zone, ids) = fixture().await;
    let input = json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Explicit role separation"});
    let valid = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/preflight", &input.to_string()).await;
    assert_eq!(valid.status, 200, "normal node and manager must remain valid: {}", valid.body);
    for (index, original, invalid, target) in [
        (0, "manager", "normal", "manager"),
        (0, "manager", "chief", "manager"),
        (1, "normal", "manager", "task"),
        (1, "normal", "chief", "task"),
    ] {
        let route = format!("/api/tasks/{}", ids[index]);
        let mut task = json!({"name":"chief-bot", "taskType":invalid, "agent":"omp",
            "chatName":"{chat.id}", "model":"default", "thinking":"default", "skill":null,"prompt":""});
        let changed = common::request_json_over_tcp(server.addr(), "PUT", &route, Some(&task.to_string())).await;
        assert_eq!(changed.status, 200, "{}", changed.body);
        for action in ["preflight", "runs"] {
            let rejected = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/work.json/{action}"), &input.to_string()).await;
            assert_eq!(rejected.status, 400, "{target} {invalid}: {}", rejected.body);
            let body: Value = serde_json::from_str(&rejected.body).unwrap();
            assert!(body["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "task-role-invalid" && issue["target"] == target && issue["taskId"] == ids[index]), "{body}");
        }
        task["taskType"] = json!(original);
        let restored = common::request_json_over_tcp(server.addr(), "PUT", &route, Some(&task.to_string())).await;
        assert_eq!(restored.status, 200, "{}", restored.body);
    }
    assert!(herdr.calls().is_empty());
}

// S2, nesting P08: descriptions roundtrip literally and empty descriptions stay
// omitted. Invalid descriptions and duplicate IDs reject before publication;
// a second Map distinguishes local uniqueness from global uniqueness.
#[tokio::test]
async fn named_input_declarations_are_local_and_duplicate_save_preserves_map() {
    let (_data, herdr, config, server, _zone, _ids) = fixture().await;
    let route = "/api/scheduler/maps/work.json/inputs";
    let draft = get_over_tcp(server.addr(), route).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let original: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body).unwrap();
    let declarations = json!([{"id":"brief","required":true,"description":"Invoice **source** <plain text>\nKeep the original date."},{"id":"appendix","required":false}]);
    let mut submitted = declarations.clone();
    submitted[1]["description"] = json!("");
    let saved = post_json_over_tcp(server.addr(), route, &json!({"map_input_rule":{"files":submitted,"description":"Review the invoice date and obtain explicit scope approval."},"revision":draft["revision"]}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    let map: Value = serde_json::from_str(&read.body).unwrap();
    let mut expected = original;
    expected["map_input_rule"] = json!({"files":declarations,"description":"Review the invoice date and obtain explicit scope approval."});
    assert_eq!(map, expected);
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), expected);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(config.scheduler.map_directory.join("work.json")).unwrap()).unwrap(), expected);
    let edited = get_over_tcp(server.addr(), route).await;
    assert_eq!(edited.status, 200, "{}", edited.body);
    let draft: Value = serde_json::from_str(&edited.body).unwrap();
    assert_eq!(draft["map_input_rule"], expected["map_input_rule"]);
    let persisted = fs::read(config.scheduler.map_directory.join("work.json")).unwrap();
    for description in [Value::Null, json!(42), json!(true), json!([]), json!({})] {
        let mut invalid = declarations.clone();
        invalid[0]["description"] = description.clone();
        let rejected = post_json_over_tcp(server.addr(), route, &json!({"map_input_rule":{"files":invalid,"description":"Must not replace general requirements."},"revision":draft["revision"]}).to_string()).await;
        assert_eq!(rejected.status, 422, "description {description}: {}", rejected.body);
        assert_eq!(fs::read(config.scheduler.map_directory.join("work.json")).unwrap(), persisted);
        assert_eq!(get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body, read.body);
    }
    let rejected = post_json_over_tcp(server.addr(), route, &json!({"map_input_rule":{"files":[{"id":"brief","required":true},{"id":"brief","required":false}],"description":"Review the invoice date and obtain explicit scope approval."},"revision":draft["revision"]}).to_string()).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(rejected.body.contains("brief"));
    assert_eq!(get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await.body, read.body);
    fs::write(config.scheduler.map_directory.join("other.json"), serde_json::to_vec(&map).unwrap()).unwrap();
    let other = get_over_tcp(server.addr(), "/api/scheduler/maps/other.json").await;
    assert_eq!(other.status, 200, "{}", other.body);
    assert_eq!(serde_json::from_str::<Value>(&other.body).unwrap()["map_input_rule"]["files"], declarations);
    assert!(herdr.calls().is_empty());
}

// S2, nesting P08/P09/P10: independent business markers are deliberately not
// file discovery hints. Dropping bindings or sharing a same-ID binding across
// Runs disagrees with the public, restarted readback of two real input files.
#[tokio::test]
async fn root_runs_preserve_named_bindings_separately_from_business_text() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let map_path = config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["map_input_rule"]["files"] = json!([{"id":"brief","required":true},{"id":"appendix","required":false}]);
    fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
    fs::write(config.scheduler.map_directory.join("other.json"), serde_json::to_vec(&map).unwrap()).unwrap();
    let mut expected_runs = Vec::new();
    for (index, name) in ["work.json", "work.json", "other.json"].iter().enumerate() {
        let file = data.path().join(format!("actual brief {index}.txt"));
        fs::write(&file, format!("independent material {index}")).unwrap();
        let bindings = json!([{"id":"brief","path":file}]);
        let business = format!("BUSINESS-{index}: brief is a word, not a file protocol");
        let input = json!({"managerTaskId":ids[0],"zone":zone,"userInput":business,"fileInputs":bindings});
        let preflight = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/{name}/preflight"), &input.to_string()).await;
        assert_eq!(preflight.status, 200, "{}", preflight.body);
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("named-manager-{index}")},"native_cwd":data.path()}));
        let started = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/{name}/runs"), &input.to_string()).await;
        assert_eq!(started.status, 201, "{}", started.body);
        let id = serde_json::from_str::<Value>(&started.body).unwrap()["id"].as_str().unwrap().to_owned();
        expected_runs.push((id, bindings, business));
    }
    drop(server);
    let server = TestServer::start(&config).await;
    for (id, bindings, business) in expected_runs {
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(detail.status, 200, "{}", detail.body);
        let detail: Value = serde_json::from_str(&detail.body).unwrap();
        assert_eq!(detail["run"]["fileInputs"], bindings);
        assert_eq!(detail["run"]["inputsReady"], false, "readable startup bindings are not semantic confirmation, including after reload");
        assert_eq!(detail["run"]["userInput"], business);
    }
}

// Ticket 13 fixture regression: an order file is not a declared Run input
// merely because the business text or a node's file rule mentions it.
#[tokio::test]
async fn handoff_order_requires_a_declared_map_input_before_manager_preparation() {
    let (data, herdr, _config, server, zone, ids) = fixture().await;
    let order = data.path().join("order.json");
    fs::write(&order, br#"{"quantity":7,"unitPriceCents":1200,"discountPercent":null}"#).unwrap();
    let started = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Discuss this order; discount is still unknown."}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    let first: Value = serde_json::from_str(&started.body).unwrap();
    let bindings = json!([{"id":"order","path":order}]);
    let rejected = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs",
        &json!({"sessionId":"manager-native-04","fileInputs":bindings,"confirmed":true,"reason":"Quantity and unit price are known; the worker will discuss the missing discount."}).to_string()).await;
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    let rejection: Value = serde_json::from_str(&rejected.body).unwrap();
    assert_eq!(rejection["code"], "run-inputs-invalid");
    assert!(rejection["message"].as_str().unwrap().contains("order"), "{rejection}");
    let unchanged = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{}", first["id"].as_str().unwrap())).await;
    let unchanged: Value = serde_json::from_str(&unchanged.body).unwrap();
    assert_eq!(unchanged["run"]["inputsReady"], false);
    assert_eq!(unchanged["run"]["fileInputs"], json!([]));

    let declarations = json!([{"id":"order","required":true,"description":"Original order; discount may be null for discussion."}]);
    let draft = get_over_tcp(server.addr(), "/api/scheduler/maps/work.json/inputs").await;
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let saved = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/inputs",
        &json!({"map_input_rule":{"files":declarations,"description":"Quantity and unit price must be known. Missing discount is the worker's discussion responsibility, not a prerequisite to start."},"revision":draft["revision"]}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let published = get_over_tcp(server.addr(), "/api/scheduler/maps/work.json").await;
    let published: Value = serde_json::from_str(&published.body).unwrap();
    assert_eq!(published["map_input_rule"]["files"], declarations);
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"handoff-input-manager"},"native_cwd":data.path()}));
    let corrected = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Discuss this order; discount is still unknown.","fileInputs":bindings}).to_string()).await;
    assert_eq!(corrected.status, 201, "{}", corrected.body);
    let corrected: Value = serde_json::from_str(&corrected.body).unwrap();
    let prepared = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs",
        &json!({"sessionId":"handoff-input-manager","fileInputs":bindings,"confirmed":true,"reason":"Read the literal order: quantity and price are known; discount remains null for worker discussion."}).to_string()).await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{}", corrected["id"].as_str().unwrap())).await;
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert_eq!(detail["run"]["inputsReady"], true);
    assert_eq!(detail["run"]["fileInputs"], bindings);
    assert!(detail["run"]["nodes"]["A"]["execution"].is_null());
    assert_eq!(fs::read(&order).unwrap(), br#"{"quantity":7,"unitPriceCents":1200,"discountPercent":null}"#);
    println!("handoff fixture public HTTP: undeclared order -> 409; declared order -> prepared without discount, dispatch or model");
}

// S2, nesting P11: duplicate and undeclared IDs must be rejected by
// both public entries before any external effect or Run allocation. Business
// text containing an ID/path must not fill a missing binding.
#[tokio::test]
async fn invalid_root_input_ids_reject_preflight_and_start_without_host_effects() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let map_path = config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["map_input_rule"]["files"] = json!([{"id":"brief","required":true}]);
    fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
    let file = data.path().join("brief.txt");
    fs::write(&file, "material").unwrap();
    for (bindings, code, input_id) in [
        (json!([{"id":"brief","path":file},{"id":"alien","path":file}]), "run-input-undeclared", "alien"),
        (json!([{"id":"brief","path":file},{"id":"brief","path":file}]), "run-input-duplicate", "brief"),
    ] {
        for action in ["preflight", "runs"] {
            let rejected = post_json_over_tcp(server.addr(), &format!("/api/scheduler/maps/work.json/{action}"), &json!({"managerTaskId":ids[0],"zone":zone,"userInput":format!("brief {}",file.display()),"fileInputs":bindings}).to_string()).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            let body: Value = serde_json::from_str(&rejected.body).unwrap();
            assert!(body["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == code && issue["inputId"] == input_id && issue["target"] == "run-input"), "{body}");
            assert!(herdr.calls().is_empty());
            assert!(!config.scheduler.run_directory.exists());
        }
    }
}

// S2, nesting P10: omission and explicit empty bindings both mean absence,
// including undeclared interfaces. A manufactured default address fails this
// public persisted readback even when starting the manager succeeds.
#[tokio::test]
async fn roots_without_files_and_omitted_optional_inputs_start_with_empty_bindings() {
    for (declarations, bindings) in [(json!(null), None), (json!([]), Some(json!([]))), (json!([{"id":"optional","required":false}]), None)] {
        let (_data, _herdr, config, server, zone, ids) = fixture().await;
        let map_path = config.scheduler.map_directory.join("work.json");
        let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
        if !declarations.is_null() { map["map_input_rule"]["files"] = declarations; }
        fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
        let mut input = json!({"managerTaskId":ids[0],"zone":zone,"userInput":"optional: no file was supplied"});
        if let Some(bindings) = bindings { input["fileInputs"] = bindings; }
        let started = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs", &input.to_string()).await;
        assert_eq!(started.status, 201, "{}", started.body);
        let body: Value = serde_json::from_str(&started.body).unwrap();
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{}",body["id"].as_str().unwrap())).await;
        assert_eq!(serde_json::from_str::<Value>(&detail.body).unwrap()["run"]["fileInputs"], json!([]));
    }
}

// P13/P15/P17: external start/prompt gates expose the actual temporal facts.
// Independent trigger literal comes from the catalog's declared mapping, not
// agent_launch_args. Status is called over HTTP with native transport identity.
#[tokio::test]
async fn manager_trigger_and_status_follow_committed_identity() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    // P16 uses distinct nonempty document markers, unlike P2's empty documents.
    fs::write(config.scheduler.bot_directory.join("worker.json"), json!({
        "id":"worker", "name":"worker", "when":"BOT-WHEN-NOT-INJECTED",
        "plan_guide":"BOT-PLAN-NOT-INJECTED", "node_guide":"BOT-NODE-NOT-INJECTED", "task_id":ids[1]
    }).to_string()).unwrap();
    let task = json!({"name":"manager","taskType":"manager","agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":"FIXED-ROLE-MARKER"});
    let update = common::request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/tasks/{}", ids[0]),
        Some(&task.to_string()),
    )
    .await;
    assert_eq!(update.status, 200, "{}", update.body);
    let start_release = data.path().join("start-release");
    let prompt_release = data.path().join("prompt-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"trigger-native-04"},"native_cwd":data.path(),"agent_start_gate":{"wait_for_file":start_release,"timeout_ms":10000},"prompt_gate":{"wait_for_file":prompt_release,"timeout_ms":10000}}));
    let addr = server.addr();
    let manager = ids[0].clone();
    let request = tokio::spawn(async move {
        post_json_over_tcp(
            addr,
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":manager,"zone":zone,"userInput":"LAST-USER-MARKER"})
                .to_string(),
        )
        .await
    });
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_start")
    })
    .await;
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert!(original["manager"].is_null());
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
    assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
    let start = herdr
        .calls()
        .into_iter()
        .find(|c| c["kind"] == "agent_start")
        .unwrap();
    assert_eq!(
        &start["args"].as_array().unwrap()[..4],
        &[
            json!("--model"),
            json!("openai-codex/gpt-6.1-sol"),
            json!("--thinking"),
            json!("high")
        ]
    );
    fs::write(start_release, b"release").unwrap();
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_prompt")
    })
    .await;
    let committed: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(committed["manager"]["sessionId"], "trigger-native-04");
    let prompt = herdr
        .calls()
        .into_iter()
        .find(|c| c["kind"] == "agent_prompt")
        .unwrap();
    let body = common::prompt_body(&prompt);
    assert!(body.starts_with("/skill:teach FIXED-ROLE-MARKER\n\n"));
    assert!(body.ends_with("\n\nLAST-USER-MARKER"));
    assert!(body.contains(original["id"].as_str().unwrap()));
    for required_path in [
        config.scheduler.map_directory.join("work.json"),
        config.scheduler.run_instructions.clone(),
        root.join("run.json"),
        root.join("history.jsonl"),
        root.join("messages.jsonl"),
        data.path().join(".manager-zone").join(".workflow").join(original["id"].as_str().unwrap()),
    ] {
        assert!(body.contains(&required_path.display().to_string()), "missing {}", required_path.display());
    }
    let status = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"trigger-native-04"}).to_string(),
    )
    .await;
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(
        serde_json::from_str::<Value>(&status.body).unwrap()["snapshot"],
        committed
    );
    let denied = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"unassociated"}).to_string(),
    )
    .await;
    assert_eq!(denied.status, 403);
    let invented = post_json_over_tcp(
        server.addr(),
        "/api/workflow/manager/status",
        &json!({"sessionId":"trigger-native-04","runId":original["id"]}).to_string(),
    )
    .await;
    assert_eq!(invented.status, 400);
    fs::write(prompt_release, b"release").unwrap();
    assert_eq!(request.await.unwrap().status, 201);
    let saved = common::get_over_tcp(server.addr(), &format!("/api/tasks/{}", ids[0])).await;
    assert_eq!(
        serde_json::from_str::<Value>(&saved.body).unwrap()["task"]["prompt"],
        "FIXED-ROLE-MARKER"
    );
}

// The literal identities are independent expected values from canonical OMP
// filenames. Keep the native files absent: creating empty files would bypass
// the startup timing condition and exercise a malformed-header failure instead.
#[tokio::test]
async fn missing_canonical_omp_files_commit_manager_and_node_identity_before_instructions() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let manager_file = data.path().join(
        "2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl",
    );
    let node_file = data.path().join(
        "2026-10-29T02-02-17-097Z_01a18564-c889-700c-92c3-5aa0460154fe.jsonl",
    );
    let release = data.path().join("missing-native-manager-prompt-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":manager_file},
        "native_cwd":data.path(),
        "prompt_gate":{"wait_for_file":release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let manager_task = ids[0].clone();
    let manager_zone = zone.clone();
    let request = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/scheduler/maps/work.json/runs", &json!({
            "managerTaskId":manager_task,"zone":manager_zone,"userInput":"missing native startup"
        }).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|call| call["kind"] == "agent_prompt")
    }).await, "manager must reach the external first-instruction gate without a native file");
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap().next().unwrap().unwrap().path();
    let committed: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(committed["manager"]["sessionId"], "01a18564-c4a1-700b-81b2-4ffb35f043ed");
    assert_eq!(committed["managerTaskId"], ids[0]);
    let history: Value =
        serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let manager = history.as_array().unwrap().iter()
        .find(|row| row["sessionId"] == "01a18564-c4a1-700b-81b2-4ffb35f043ed").unwrap();
    assert_eq!(manager["sourceTaskId"], ids[0]);
    assert_eq!(manager["agentType"], "omp");
    assert_eq!(manager["cwd"], json!(data.path()));
    assert_eq!(manager["sessionFile"], json!(manager_file));
    assert_eq!(manager["startupIdentityPath"], json!(manager_file));
    assert!(!manager_file.exists(), "startup must not manufacture the native file");
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 201, "{}", response.body);
    let started: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(started["run"]["manager"]["sessionId"], "01a18564-c4a1-700b-81b2-4ffb35f043ed");
    let status = post_json_over_tcp(server.addr(), "/api/workflow/manager/status",
        &json!({"sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed"}).to_string()).await;
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(serde_json::from_str::<Value>(&status.body).unwrap()["snapshot"]["id"], committed["id"]);

    let prepared = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs", &json!({"sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed","fileInputs":[],"confirmed":true,"reason":"Reviewed fixture inputs before native identity checks"}).to_string()).await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let node_release = data.path().join("missing-native-node-prompt-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":node_file},
        "native_cwd":data.path(),
        "prompt_gate":{"wait_for_file":node_release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let dispatch = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({
            "sessionId":"01a18564-c4a1-700b-81b2-4ffb35f043ed","nodes":["A"]
        }).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2
    }).await, "node must reach its first-instruction gate without a native file");
    let saved: Value =
        serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "01a18564-c889-700c-92c3-5aa0460154fe");
    assert_eq!(saved["nodes"]["A"]["execution"]["zone"], json!(data.path()));
    let history: Value =
        serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let node = history.as_array().unwrap().iter()
        .find(|row| row["sessionId"] == "01a18564-c889-700c-92c3-5aa0460154fe").unwrap();
    assert_eq!(node["sourceTaskId"], ids[1]);
    assert_eq!(node["agentType"], "omp");
    assert_eq!(node["cwd"], json!(data.path()));
    assert_eq!(node["sessionFile"], json!(node_file));
    assert_eq!(node["startupIdentityPath"], json!(node_file));
    let native = common::read_json_lines(&herdr.path().join("started.jsonl")).pop().unwrap();
    assert_eq!(native["actual_cwd"], json!(data.path()));
    assert_eq!(herdr.tabs().last().unwrap()["cwd"], json!(data.path()));
    assert!(!manager_file.exists());
    assert!(!node_file.exists());
    fs::write(node_release, b"release").unwrap();
    let dispatched = dispatch.await.unwrap();
    assert_eq!(dispatched.status, 200, "{}", dispatched.body);
    let result: Value = serde_json::from_str(&dispatched.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
    assert!(!manager_file.exists(), "manager file must remain owned by OMP");
    assert!(!node_file.exists(), "node file must remain owned by OMP");
}

#[tokio::test]
async fn missing_pi_node_file_still_submits_initial_instructions_after_association() {
    let (data, herdr, config, server, zone, ids) = fixture_for_agent("pi").await;
    herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"pi-manager"},"native_cwd":data.path()}));
    let started = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    let run: Value = serde_json::from_str(&started.body).unwrap();
    let prepared = post_json_over_tcp(server.addr(), "/api/workflow/manager/prepare_inputs", &json!({"sessionId":"pi-manager","fileInputs":[],"confirmed":true,"reason":"Reviewed fixture inputs before native identity checks"}).to_string()).await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let file = data.path().join("2026-10-29T06-31-42-214Z_01a1865b-7185-74a2-af0e-9d7eb972c0ee.jsonl");
    let release = data.path().join("pi-node-input-release");
    herdr.set_behavior(json!({
        "native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":file},
        "native_cwd":data.path(), "prompt_gate":{"wait_for_file":release,"timeout_ms":10000}
    }));
    let addr = server.addr();
    let request = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/manager/dispatch",
            &json!({"sessionId":"pi-manager","nodes":["A"]}).to_string()).await
    });
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2
    }).await, "pi node never received initial instructions while its native file was absent");
    let root = config.scheduler.run_directory.join(run["id"].as_str().unwrap());
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "01a1865b-7185-74a2-af0e-9d7eb972c0ee");
    let history: Value = serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    let node = history.as_array().unwrap().iter().find(|entry| entry["sessionId"] == "01a1865b-7185-74a2-af0e-9d7eb972c0ee").unwrap();
    assert_eq!(node["startupIdentityPath"], json!(file));
    assert_eq!(node["agentType"], "pi");
    assert!(!file.exists(), "herdr-bot must not manufacture native files");
    fs::write(release, "release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let result: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
}

#[tokio::test]
async fn existing_omp_file_header_takes_precedence_over_canonical_filename() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let file = data.path().join("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl");
    let contents = format!("{}\n", json!({"type":"session","id":"actual-header-identity","cwd":data.path()}));
    fs::write(&file, &contents).unwrap();
    herdr.set_behavior(json!({"native_session":{"agent":"omp","source":"herdr:omp","kind":"path","value":file}}));
    let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs", &json!({
        "managerTaskId":ids[0],"zone":zone,"userInput":""
    }).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    let started: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(started["run"]["manager"]["sessionId"], "actual-header-identity");
    let history: Value = serde_json::from_slice(&fs::read(&config.sessions.history_file).unwrap()).unwrap();
    assert_eq!(history[0]["sessionId"], "actual-header-identity");
    assert_eq!(fs::read_to_string(&file).unwrap(), contents);
}

// A plausible but unsafe implementation extracts any filename suffix, or
// falls back after every read/header error. Only official missing canonical
// paths with a reported working directory qualify for the startup exception.
#[tokio::test]
async fn invalid_native_paths_and_existing_header_errors_never_submit_instructions() {
    for case in [
        "uuid-only", "invalid-timestamp", "invalid-uuid", "wrong-extension",
        "relative-path", "empty-file", "invalid-json", "wrong-header-type",
        "missing-header-id", "unreadable-directory", "wrong-source", "wrong-agent", "missing-cwd",
    ] {
        let (data, herdr, config, server, zone, ids) = fixture().await;
        let canonical = data.path().join(
            "2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl",
        );
        let file = match case {
            "uuid-only" => data.path().join("01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            "invalid-timestamp" => data.path().join("2026-99-99T99-99-99-999Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            "invalid-uuid" => data.path().join("2026-10-29T02-02-16-097Z_not-a-uuid.jsonl"),
            "wrong-extension" => data.path().join("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.txt"),
            "relative-path" => std::path::PathBuf::from("2026-10-29T02-02-16-097Z_01a18564-c4a1-700b-81b2-4ffb35f043ed.jsonl"),
            _ => canonical,
        };
        let bytes = match case {
            "empty-file" => Some(""),
            "invalid-json" => Some("not json\n"),
            "wrong-header-type" => Some("{\"type\":\"message\",\"id\":\"not-a-session\"}\n"),
            "missing-header-id" => Some("{\"type\":\"session\"}\n"),
            _ => None,
        };
        if let Some(bytes) = bytes {
            fs::write(&file, bytes).unwrap();
        } else if case == "unreadable-directory" {
            fs::create_dir(&file).unwrap();
        }
        herdr.set_behavior(json!({
            "native_session":{
                "source":if case == "wrong-source" { "custom:omp" } else { "herdr:omp" },
                "agent":if case == "wrong-agent" { "pi" } else { "omp" },
                "kind":"path","value":file
            },
            "native_cwd":if case == "missing-cwd" { Value::Null } else { json!(data.path()) }
        }));
        let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string()).await;
        assert_eq!(response.status, 500, "{case}: {}", response.body);
        let failure: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(failure["startupError"]["stage"], "identity", "{case}: {}", response.body);
        assert_eq!(failure["startupError"]["effects"]["inputAttempted"], false, "{case}");
        let calls = herdr.calls();
        assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_get").count(), 1, "{case}: invalid identity is not a pending identity");
        assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"), "{case}");
        let root = config.scheduler.run_directory.join(failure["id"].as_str().unwrap());
        let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
        assert!(saved["manager"].is_null(), "{case}");
        if let Some(bytes) = bytes {
            assert_eq!(fs::read(&file).unwrap(), bytes.as_bytes(), "{case}");
        } else if case == "unreadable-directory" {
            assert!(file.is_dir());
        } else if case != "relative-path" {
            assert!(!file.exists(), "{case}: rejection must not manufacture a native file");
        }
    }
}

// Herdr can acknowledge agent start before publishing agent_session. Waiting
// for that read-only fact must not duplicate the agent or submit input early.
#[tokio::test]
async fn manager_waits_for_late_native_identity_without_restarting_or_replaying_input() {
    let (data, herdr, _config, server, zone, ids) = fixture().await;
    herdr.set_behavior(json!({
        "native_session_pending_reads":2,
        "native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"late-manager-identity"},
        "native_cwd":data.path()
    }));
    let response = post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Initialize once after identity"}).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["run"]["manager"]["sessionId"], "late-manager-identity");
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    let prompt = calls.iter().position(|call| call["kind"] == "agent_prompt").unwrap();
    assert!(calls[..prompt].iter().filter(|call| call["kind"] == "agent_get").count() >= 3);
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_prompt").count(), 1);
}

#[tokio::test]
async fn manager_identity_wait_expires_without_restarting_or_submitting_input() {
    let (_data, herdr, _config, server, zone, ids) = fixture().await;
    herdr.set_behavior(json!({"native_session":null,"manual_launch_events":true}));
    let started = std::time::Instant::now();
    let response = tokio::time::timeout(std::time::Duration::from_secs(40),
        post_json_over_tcp(server.addr(), "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":"Do not send without identity"}).to_string())
    ).await.expect("identity acquisition must terminate within its bounded startup budget");
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(started.elapsed() >= std::time::Duration::from_secs(30), "missing identity must get the full acquisition window");
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["startupError"]["stage"], "identity");
    assert_eq!(body["startupError"]["effects"]["agentStarted"], true);
    assert_eq!(body["startupError"]["effects"]["inputAttempted"], false);
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"));
}

// P13: an initialization error must precede every external call; an identity
// error can leave a real Chat, but cannot submit an instruction. Neither error
// is recorded as a persisted business failure or retried autonomously.
#[tokio::test]
async fn initialization_and_identity_failures_stop_before_initial_instructions() {
    for stage in ["initialize", "identity", "submit"] {
        let (data, herdr, config, server, zone, ids) = fixture().await;
        if stage == "initialize" {
            fs::write(&config.scheduler.run_directory, b"not a directory").unwrap();
        }
        if stage == "identity" {
            herdr.set_behavior(json!({"fail_agent_get":true}));
        }
        if stage == "submit" {
            herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"submit-failure-chat"},"native_cwd":data.path(),"fail_prompt":true}));
        }
        let response = post_json_over_tcp(
            server.addr(),
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
        )
        .await;
        assert_eq!(response.status, 500, "{}", response.body);
        let detail: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(detail["startupError"]["stage"], stage);
        assert_eq!(detail["startupError"]["effects"]["inputSubmitted"], false);
        assert_eq!(detail["startupError"]["effects"]["inputAttempted"], stage == "submit");
        let calls = herdr.calls();
        if stage == "initialize" {
            assert!(calls.is_empty());
        } else {
            assert_eq!(
                calls.iter().filter(|c| c["kind"] == "agent_start").count(),
                1
            );
            assert_eq!(
                calls.iter().filter(|c| c["kind"] == "agent_prompt").count(),
                usize::from(stage == "submit")
            );
            let root = config
                .scheduler
                .run_directory
                .join(detail["id"].as_str().unwrap());
            assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
            assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
        }
        let refreshed = get_over_tcp(
            server.addr(),
            &format!("/api/workflow/runs/{}", detail["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(refreshed.status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&refreshed.body).unwrap()["startupError"],
            detail["startupError"]
        );
        assert_eq!(herdr.calls(), calls);
    }
}

// P13/P45: the external identity gate distinguishes persisted linkage from
// a plausible prompt-first bug; association failure must preserve external refs.
#[tokio::test]
async fn association_failure_never_submits_and_exposes_real_started_effects() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let release = data.path().join("identity-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"association-failure-chat"},"native_cwd":data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = server.addr();
    let request = tokio::spawn(async move {
        post_json_over_tcp(
            addr,
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
        )
        .await
    });
    common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|c| c["kind"] == "agent_get")
    })
    .await;
    let root = fs::read_dir(&config.scheduler.run_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(root.join("history.jsonl").is_file());
    assert!(root.join("messages.jsonl").is_file());
    let original = fs::read(root.join("run.json")).unwrap();
    let id = serde_json::from_slice::<Value>(&original).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(herdr.calls().iter().all(|c| c["kind"] != "agent_prompt"));
    // The S4 file boundary deterministically rejects whole-file replacement.
    fs::remove_file(root.join("run.json")).unwrap();
    fs::create_dir(root.join("run.json")).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 500);
    let failure: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(failure["id"], id, "{}", response.body);
    assert_eq!(failure["startupError"]["stage"], "association");
    assert_eq!(
        failure["startupError"]["effects"]["sessionId"],
        "association-failure-chat"
    );
    assert_eq!(failure["startupError"]["effects"]["agentStarted"], true);
    assert_eq!(failure["startupError"]["effects"]["inputSubmitted"], false);
    assert!(herdr.calls().iter().all(|c| c["kind"] != "agent_prompt"));
    let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(
        serde_json::from_str::<Value>(&detail.body).unwrap()["startupError"],
        failure["startupError"]
    );
    fs::remove_dir(root.join("run.json")).unwrap();
    fs::write(root.join("run.json"), original).unwrap();
}

// A failed Chat-history write follows a committed Run association, but must
// still stop before initial input. Only the startup diagnostic is process-local.
#[tokio::test]
async fn manager_history_failure_retains_saved_identity_without_initial_prompt() {
    let (data, herdr, config, server, zone, ids) = fixture().await;
    let release = data.path().join("history-identity-release");
    herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-history-failed"},"native_cwd":data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let request = common::OpenJsonRequest::submit(
        server.addr(),
        "/api/scheduler/maps/work.json/runs",
        &json!({"managerTaskId":ids[0],"zone":zone,"userInput":""}).to_string(),
    ).await;
    assert!(common::wait_until(std::time::Duration::from_secs(5), || {
        herdr.calls().iter().any(|call| call["kind"] == "agent_get")
    }).await);
    let history = &config.sessions.history_file;
    let original_history = fs::read(history).ok();
    if history.exists() { fs::remove_file(history).unwrap(); }
    fs::create_dir_all(history).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.response().await;
    assert_eq!(response.status, 500, "{}", response.body);
    let failure: Value = serde_json::from_str(&response.body).unwrap();
    let error = &failure["startupError"];
    assert_eq!(error["stage"], "history");
    assert_eq!(error["effects"]["sessionId"], "manager-history-failed");
    assert_eq!(error["effects"]["agentStarted"], true);
    assert_eq!(error["effects"]["associationSaved"], true);
    assert_eq!(error["effects"]["inputAttempted"], false);
    assert_eq!(error["effects"]["inputSubmitted"], false);
    let id = failure["id"].as_str().unwrap();
    let root = config.scheduler.run_directory.join(id);
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["manager"]["sessionId"], "manager-history-failed");
    assert!(saved.get("startupError").is_none());
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
    assert!(history.is_dir(), "failed history publication must not replace the fault");
    let calls = herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "agent_start").count(), 1);
    assert!(calls.iter().all(|call| call["kind"] != "agent_prompt"));
    let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    assert_eq!(serde_json::from_str::<Value>(&detail.body).unwrap()["startupError"], *error);
    let restarted = TestServer::start(&config).await;
    let detail = get_over_tcp(restarted.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert!(detail.get("startupError").is_none() || detail["startupError"].is_null());
    assert_eq!(detail["run"]["manager"], saved["manager"]);
    assert_eq!(herdr.calls(), calls);
    fs::remove_dir(history).unwrap();
    if let Some(bytes) = original_history { fs::write(history, bytes).unwrap(); }
}

#[tokio::test]
async fn valid_empty_bot_documents_start_manager_and_preserve_literal_requirements() {
    let (_data, herdr, config, server, zone, ids) = fixture().await;
    for requirements in ["", "# 本次独立标记\n\n- 保留  空格\n末尾\n"] {
        let response = post_json_over_tcp(
            server.addr(),
            "/api/scheduler/maps/work.json/runs",
            &json!({"managerTaskId":ids[0],"zone":zone,"userInput":requirements}).to_string(),
        )
        .await;
        assert_eq!(response.status, 201, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        let id = body["run"]["id"].as_str().unwrap();
        assert_eq!(body["run"]["userInput"], requirements);
        assert_eq!(body["run"]["manager"]["sessionId"], "manager-native-04");
        assert_eq!(body["run"]["manager"]["workspaceId"], "w-manager");
        assert_eq!(body["run"]["managerTaskId"], ids[0]);
        let root = config.scheduler.run_directory.join(id);
        let persisted: Value =
            serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
        assert_eq!(persisted["userInput"], requirements);
        assert_eq!(fs::read(root.join("messages.jsonl")).unwrap(), b"");
        assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), b"");
        assert_eq!(persisted["nodes"]["A"]["status"], "pending");
        assert!(persisted["nodes"]["A"].get("ready").is_none());
        let calls = herdr.calls();
        let prompt = common::prompt_body(calls
            .iter()
            .rev()
            .find(|c| c["kind"] == "agent_prompt")
            .unwrap());
        assert!(prompt.starts_with("FIXED-MANAGER-ROLE\n\nRun ID: "));
        assert!(prompt.contains("work.json"));
        assert!(!prompt.contains("MAP-BODY-NOT-INJECTED"));
        assert!(!prompt.contains("NODE-INPUT-NOT-INJECTED"));
        if requirements.is_empty() {
            assert!(!prompt.ends_with("\n\n"));
        } else {
            assert!(prompt.ends_with("\n\n# 本次独立标记\n\n- 保留  空格\n末尾\n"));
        }
        let count = calls.iter().filter(|c| c["kind"] == "agent_start").count();
        let detail = get_over_tcp(server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(detail.status, 200, "{}", detail.body);
        assert_eq!(
            serde_json::from_str::<Value>(&detail.body).unwrap()["run"]["userInput"],
            requirements
        );
        assert_eq!(
            herdr
                .calls()
                .iter()
                .filter(|c| c["kind"] == "agent_start")
                .count(),
            count
        );
    }
}
