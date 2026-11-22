//! Ticket 08 / S2: real HTTP, isolated definitions/storage, production Herdr CLI
//! adapter with a controlled external executable. No model or native-host claims.
#[path = "../common/mod.rs"]
mod common;
use common::{HerdrFixture, HttpResponse, TempDir, get_over_tcp, post_json_over_tcp};
#[path = "chief_delegation/process.rs"]
mod process;
#[path = "chief_delegation/map.rs"]
mod map;
#[path = "chief_delegation/reports.rs"]
mod reports;
#[path = "chief_delegation/receipts.rs"]
mod receipts;
#[path = "chief_delegation/role_recovery.rs"]
mod role_recovery;
use process::TestServer;
use serde_json::{Value, json};
use std::{fs, time::Duration};

fn body(response: HttpResponse, status: u16) -> Value {
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn post(server: &TestServer, path: &str, input: Value) -> Value {
    body(
        post_json_over_tcp(server.addr(), path, &input.to_string()).await,
        200,
    )
}
async fn terminal(server: &TestServer, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let view = body(
            get_over_tcp(server.addr(), &format!("/api/chats/{id}")).await,
            200,
        );
        if matches!(view["stage"].as_str(), Some("succeeded" | "failed")) {
            return view;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "creation did not finish: {view}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
struct Fixture {
    server: TestServer,
    config: herdr_bot::Config,
    worker: Value,
    herdr: HerdrFixture,
    root: TempDir,
}
impl Fixture {
    async fn new() -> Self {
        Self::with_host(false, "pi").await
    }

    async fn with_host(app: bool, agent: &str) -> Self {
        let root = TempDir::new("chief delegation S1");
        let herdr = HerdrFixture::new("chief delegation CLI");
        let mut config = common::config(&root.path().join("unused.bat"), root.path());
        config.base_directory = root.path().to_owned();
        config.data_directory = root.path().join("data");
        config.tasks.storage_file = root.path().join("tasks.json");
        config.zones.registry_file = root.path().join("zones.json");
        config.sessions.history_file = root.path().join("history.json");
        config.prompts_file = root.path().join("prompts.toml");
        config.scheduler.bot_directory = root.path().join("bots");
        config.scheduler.map_directory = root.path().join("maps");
        config.scheduler.run_directory = root.path().join("runs");
        config.suites.definition_file = root.path().join("suites.json");
        config.suites.mcp_file = root.path().join("mcp.json");
        config.herdr.executable = herdr.executable().to_owned();
        if app {
            fs::write(herdr.path().join("chat-app-mode"), "").unwrap();
            config.herdr.backend = herdr_bot::config::HerdrBackend::App;
            config.herdr.app_executable = herdr.executable().to_owned();
            config.herdr.core_executable = herdr.executable().to_owned();
            // App deployments need neither official TUI state entrypoint.
            let mut catalog: toml::Value = toml::from_str(&fs::read_to_string(&config.tasks.metadata_file).unwrap()).unwrap();
            for extension in catalog["extensions"].as_array_mut().unwrap() {
                if matches!(extension["id"].as_str(), Some("omp-herdr-agent-state" | "pi-herdr-agent-state")) {
                    extension["entrypoint"] = toml::Value::String("not-installed-tui-state.ts".into());
                }
            }
            config.tasks.metadata_file = root.path().join("app-task-options.toml");
            fs::write(&config.tasks.metadata_file, toml::to_string(&catalog).unwrap()).unwrap();
        }
        fs::create_dir(root.path().join("bots")).unwrap();
        fs::create_dir(root.path().join("work")).unwrap();
        fs::create_dir(root.path().join("materials")).unwrap();
        fs::write(
            root.path().join("materials/brief.txt"),
            "Independent source material 731",
        )
        .unwrap();
        let server = TestServer::start(&config).await;
        for (id, dir) in [("work-zone", "work"), ("material-zone", "materials")] {
            post(
                &server,
                "/api/zones",
                json!({"id":id,"path":root.path().join(dir)}),
            )
            .await;
        }
        let mut tasks = Vec::new();
        for (name, role, prompt) in [
            ("entry", "chief", "Chief preset"),
            ("worker", "normal", "Worker preset"),
        ] {
            let chat_name = if app && role == "chief" { "Same chief title" } else { "work {chat.id}" };
            let result=post(&server,"/api/tasks",json!({"name":name,"taskType":role,"chatName":chat_name,"agent":agent,"model":"default","prompt":prompt})).await;
            tasks.push(
                result["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["name"] == name)
                    .unwrap()
                    .clone(),
            );
        }
        fs::write(root.path().join("bots/worker.json"),json!({"id":"brief-bot","name":"Brief analyst","when":"Summarize a brief","plan_guide":"Use supplied material","node_guide":"Do not invent findings","task_id":tasks[1]["id"]}).to_string()).unwrap();
        let fixture = Self {
            root,
            herdr,
            config,
            server,
            worker: tasks[1].clone(),
        };
        fixture.start_chief("chief-a", &tasks[0]).await;
        if !app {
            fixture.identity("worker-a");
        }
        fixture
    }

    fn is_app(&self) -> bool {
        self.config.herdr.backend == herdr_bot::config::HerdrBackend::App
    }

    async fn start_chief(&self, session_id: &str, task: &Value) -> Value {
        if !self.is_app() {
            self.identity(session_id);
        }
        let preview = post(&self.server, "/api/chats/preview", json!({"zone":"work-zone","taskId":task["id"]})).await;
        let started = body(post_json_over_tcp(self.server.addr(), "/api/chats",
            &json!({"zone":"work-zone","taskId":task["id"],"chatId":preview["chatId"]}).to_string()).await, 202);
        let id = started["id"].as_str().unwrap();
        if self.is_app() {
            let tab = self.app_tab(id).await;
            self.app_event(&tab, session_id, "launch_identity").await;
            self.app_event(&tab, session_id, "launch_ready").await;
        }
        let finished = terminal(&self.server, id).await;
        assert_eq!(finished["stage"], "succeeded", "{finished}");
        post(&self.server, "/api/sessions/events", json!({"sessionId":session_id,"kind":"register","data":{
            "cwd":self.root.path().join("work"),"agentType":task["agent"]
        }})).await;
        finished
    }

    async fn app_tab(&self, id: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let view = body(get_over_tcp(self.server.addr(), &format!("/api/chats/{id}")).await, 200);
                if view["stage"] == "initializing" {
                    return common::read_json_lines(&self.herdr.path().join("app-tabs.jsonl"))
                        .into_iter().find(|tab| tab["tab"] == view["tabId"]).expect("created App Tab");
                }
                assert_ne!(view["stage"], "failed", "{view}");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }).await.expect("App Chat initializes")
    }

    async fn app_event(&self, tab: &Value, session_id: &str, kind: &str) {
        let argv = &tab["options"]["argv"];
        post(&self.server, "/api/sessions/events", json!({"sessionId":session_id,"kind":kind,"data":{
            "launchId":argv[6],"targetId":argv[7],"agentType":argv[8],"cwd":tab["options"]["folder"],"pid":43127
        }})).await;
    }
    fn identity(&self, id: &str) {
        let file = self.root.path().join(format!("{id}.jsonl"));
        fs::write(&file,json!({"type":"session","version":3,"id":id,"timestamp":"2026-11-12T00:00:00.000Z","cwd":self.root.path().join("work")}).to_string()+"\n").unwrap();
        self.herdr.set_behavior(
            json!({"native_session":{"source":format!("herdr:{}", self.worker["agent"].as_str().unwrap()),"agent":self.worker["agent"],"kind":"path","value":file}}),
        );
    }
    fn request(&self) -> Value {
        json!({"sessionId":"chief-a","botId":"brief-bot","zoneId":"work-zone","input":"Summarize only the supplied brief; do not publish.","materials":[{"id":"brief","zoneId":"material-zone","path":"brief.txt"}]})
    }
    async fn preview(&self) -> Value {
        post(
            &self.server,
            "/api/chief/delegations/preview",
            self.request(),
        )
        .await
    }
}

// The preview is the approved S2 public boundary for explicit business facts.
// Missing/escaped material, empty input and wrong target roles must not be
// "helpfully" substituted with a guessed file, Task, zone or another Bot.
#[tokio::test]
async fn preview_rejects_incomplete_or_invalid_business_targets_and_materials() {
    let f = Fixture::new().await;
    let before = f.herdr.calls().len();
    for change in [
        "empty-input",
        "missing-zone",
        "missing-bot",
        "duplicate-material",
        "escaped-material",
        "missing-material",
    ] {
        let mut request = f.request();
        match change {
            "empty-input" => request["input"] = json!("  "),
            "missing-zone" => request["zoneId"] = json!("not-registered"),
            "missing-bot" => request["botId"] = json!("not-defined"),
            "duplicate-material" => {
                let duplicate = request["materials"][0].clone();
                request["materials"].as_array_mut().unwrap().push(duplicate);
            }
            "escaped-material" => request["materials"][0]["path"] = json!("../work/secret.txt"),
            _ => request["materials"][0]["path"] = json!("missing.txt"),
        }
        let response = post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations/preview",
            &request.to_string(),
        )
        .await;
        assert!(response.status >= 400, "{change}: {}", response.body);
    }
    for role in ["manager", "chief"] {
        let mut task = f.worker.clone();
        task.as_object_mut().unwrap().remove("id");
        task["taskType"] = json!(role);
        body(
            common::request_json_over_tcp(
                f.server.addr(),
                "PUT",
                &format!("/api/tasks/{}", f.worker["id"].as_str().unwrap()),
                Some(&task.to_string()),
            )
            .await,
            200,
        );
        let response = post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations/preview",
            &f.request().to_string(),
        )
        .await;
        assert_eq!(body(response, 409)["code"], "delegation-task-role");
    }
    fs::write(f.root.path().join("bots/worker.json"),json!({"id":"brief-bot","name":"Map only","when":"Flow","plan_guide":"Flow","node_guide":"Flow","map_file":"flow.json"}).to_string()).unwrap();
    let response = post_json_over_tcp(
        f.server.addr(),
        "/api/chief/delegations/preview",
        &f.request().to_string(),
    )
    .await;
    assert!(
        response.status >= 400,
        "Map Bot cannot silently become a Task dispatch"
    );
    assert_eq!(f.herdr.calls().len(), before);
}

// A malformed official native reference is a concrete adapter failure, not an
// absent identity to poll. Keep its original category instead of swallowing it
// for the 30-second identity budget and eventually claiming a generic timeout.
#[tokio::test]
async fn malformed_native_reference_preserves_failure_without_repeated_identity_commands() {
    let f = Fixture::new().await;
    let preview = f.preview().await;
    f.herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"omp","kind":"id","value":"not-a-pi-chat"}}));
    let before = f
        .herdr
        .calls()
        .iter()
        .filter(|call| call["kind"] == "agent_get")
        .count();
    let accepted = body(
        post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations",
            &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string(),
        )
        .await,
        202,
    );
    let observed = tokio::time::timeout(
        Duration::from_secs(5),
        terminal(&f.server, accepted["id"].as_str().unwrap()),
    )
    .await;
    if observed.is_err() {
        // Release the owned creation cleanly even on the old looping violation;
        // then assert the original bounded observation, not this cleanup result.
        f.identity("worker-a");
        terminal(&f.server, accepted["id"].as_str().unwrap()).await;
    }
    let view = observed.expect("malformed reference must return its concrete failure promptly");
    assert_eq!(view["stage"], "failed");
    assert_eq!(view["failure"]["code"], "herdr-unexpected-response");
    assert!(
        view["failure"]["message"]
            .as_str()
            .unwrap()
            .contains("native reference agent/source does not match")
    );
    assert_eq!(
        f.herdr
            .calls()
            .iter()
            .filter(|call| call["kind"] == "agent_get")
            .count()
            - before,
        1,
        "no repeated command after concrete identity failure"
    );
}

// Independent outcome: the existing Chat lifecycle distinguishes a rejected
// submission from an accepted input whose response is unreadable. A wrapper that
// reports success, drops failure details or guesses sender identity must fail.
#[tokio::test]
async fn startup_failures_keep_real_association_and_uncertain_submission_feedback() {
    for fault in [
        "fail_agent_start",
        "fail_prompt",
        "invalid_prompt_response",
        "wrong-cwd",
        "history-write",
    ] {
        let f = Fixture::new().await;
        let preview = f.preview().await;
        let mut behavior: Value = serde_json::from_str(
            &fs::read_to_string(f.herdr.path().join("behavior.json")).unwrap(),
        )
        .unwrap();
        if fault == "wrong-cwd" {
            behavior["native_cwd"] = json!(f.root.path().join("materials"));
        } else if fault != "history-write" {
            behavior[fault] = json!(true);
        }
        f.herdr.set_behavior(behavior);
        let _block = (fault == "history-write")
            .then(|| common::HistoryWriteBlock::new(&f.config.sessions.history_file));
        let before = common::read_json_lines(&f.herdr.path().join("submitted.jsonl")).len();
        let accepted = body(
            post_json_over_tcp(
                f.server.addr(),
                "/api/chief/delegations",
                &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string(),
            )
            .await,
            202,
        );
        let id = accepted["id"].as_str().unwrap();
        let view = terminal(&f.server, id).await;
        assert_eq!(view["stage"], "failed", "{fault}: {view}");
        assert_eq!(view["inputSubmitted"], false);
        let record = body(
            get_over_tcp(f.server.addr(), &format!("/api/chief/delegations/{id}")).await,
            200,
        );
        assert_eq!(
            record["creation"]["failure"], view["failure"],
            "{fault}: failure remains recoverable"
        );
        let submits =
            common::read_json_lines(&f.herdr.path().join("submitted.jsonl")).len() - before;
        if fault == "invalid_prompt_response" {
            assert_eq!(
                submits, 1,
                "the real adapter must preserve an accepted external effect"
            );
            assert_eq!(view["failure"]["code"], "herdr-unexpected-response");
            assert!(
                view["failure"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("不能据此断定未发送")
            );
        } else {
            assert_eq!(submits, 0, "{fault}: no business input accepted");
        }
        if matches!(fault, "fail_prompt" | "invalid_prompt_response") {
            assert_eq!(record["sessionId"], "worker-a");
        } else {
            assert!(
                record.get("sessionId").is_none(),
                "{fault}: no invented sender identity"
            );
        }
    }
}

// S1/P23/P37: public dispatch and history, plus the selected host's terminal
// boundary. The gate holds the first real business input: absent or late
// history/provenance fails the public reads, and argv prefill fails independently.
#[tokio::test]
async fn sender_is_durable_before_external_input_is_accepted() {
    for (app, agent) in [(true, "omp"), (true, "pi"), (false, "omp"), (false, "pi")] {
        let f = Fixture::with_host(app, agent).await;
        let current = body(get_over_tcp(f.server.addr(), "/api/chief").await, 200);
        assert_eq!(current["current"]["sessionId"], "chief-a");
        assert_eq!(current["current"]["phase"], "registered");
        let before = f.herdr.calls().len();
        let mut denied = f.request();
        denied["sessionId"] = json!("unrelated-chat");
        let denied = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations/preview", &denied.to_string()).await, 409);
        assert_eq!(denied["code"], "chief-not-current");
        let preview = f.preview().await;
        assert_eq!(f.herdr.calls().len(), before, "preview/denial must not launch work");
        assert_eq!(preview["facts"]["bot"]["id"], "brief-bot");
        assert_eq!(preview["facts"]["task"]["id"], f.worker["id"]);
        let literal = "Summarize only the supplied brief; do not publish.";
        let prompt = preview["facts"]["prompt"].as_str().unwrap();
        assert!(prompt.starts_with(&format!("Worker preset\n\n{literal}\n\n")));
        let release = f.root.path().join("release-prompt");
        let settings = f.herdr.path().join("behavior.json");
        let mut behavior = if app { json!({}) } else { serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap() };
        behavior["prompt_gate"] = json!({"wait_for_file":release,"timeout_ms":10000});
        f.herdr.set_behavior(behavior);
        let accepted = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({
            "sessionId":"chief-a","previewId":preview["previewId"]
        }).to_string()).await, 202);
        let id = accepted["id"].as_str().unwrap();
        let app_tab = if app {
            let tab = f.app_tab(id).await;
            assert_eq!(tab["space"], "space-default");
            assert!(tab["options"].get("task").is_none());
            assert!(!tab["options"]["argv"].to_string().contains(literal));
            assert!(!tab["options"]["argv"].to_string().contains("not-installed-tui-state"));
            f.app_event(&tab, "worker-a", "launch_identity").await;
            assert_eq!(common::read_json_lines(&f.herdr.path().join("submitted.jsonl")).len(), 1);
            f.app_event(&tab, "worker-a", "launch_ready").await;
            Some(tab)
        } else { None };
        let sent_kind = if app { "core_send" } else { "agent_prompt" };
        let observed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(call) = f.herdr.calls().into_iter().find(|call| call["kind"] == sent_kind && call["text"] == prompt) {
                    let record = get_over_tcp(f.server.addr(), "/api/chief/delegations?sessionId=worker-a").await;
                    let history = get_over_tcp(f.server.addr(), "/api/sessions/history").await;
                    // The existing durable history format carries the source
                    // Task; the public history list deliberately omits it.
                    let persisted = fs::read(&f.config.sessions.history_file);
                    return (call, record, history, persisted);
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }).await;
        let submitted_before_release = common::read_json_lines(&f.herdr.path().join("submitted.jsonl")).len();
        fs::write(&release, "release owned external process").unwrap();
        let (call, record, history, persisted) = observed.expect("selected host must reach the input gate");
        let finished = terminal(&f.server, id).await;
        assert_eq!(finished["stage"], "succeeded", "{finished}");
        assert_eq!(finished["inputSubmitted"], true);
        assert_eq!(submitted_before_release, 1, "only the chief input was accepted before release");
        let record = body(record, 200);
        assert_eq!(record["delegations"][0]["id"], id);
        assert_eq!(record["delegations"][0]["sessionId"], "worker-a");
        assert_eq!(record["delegations"][0]["facts"], preview["facts"]);
        let history = body(history, 200);
        let worker = history["sessions"].as_array().unwrap().iter().find(|chat| chat["sessionId"] == "worker-a").expect("worker history must be public before input");
        let persisted: Vec<Value> = serde_json::from_slice(&persisted.unwrap()).unwrap();
        let stored_worker = persisted.iter().find(|chat| chat["sessionId"] == "worker-a").expect("worker history on disk before input");
        assert_eq!(stored_worker["sourceTaskId"], f.worker["id"]);
        assert_eq!(worker["agentType"], agent);
        assert!(common::same_path(std::path::Path::new(worker["cwd"].as_str().unwrap()), &f.root.path().join("work")));
        if let Some(tab) = app_tab {
            assert_eq!(call["terminal"], tab["terminal"]);
            assert_eq!(finished["tabId"], tab["tab"]);
            assert_eq!(worker["originalWorkspaceId"], "space-default");
        } else {
            let starts = common::read_json_lines(&f.herdr.path().join("started.jsonl"));
            assert_eq!(call["target"], starts[1]["name"]);
            assert_eq!(finished["paneId"], starts[1]["pane_id"]);
            let invocations = common::read_json_lines(&f.herdr.path().join("invocations.jsonl"));
            assert!(invocations.iter().filter(|row| row["argv"][1] == "create" || row["argv"][1] == "start")
                .all(|row| !row["argv"].to_string().contains(literal)));
        }
        let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
        assert_eq!(submitted.len(), 2);
        assert_eq!(submitted[1]["text"], prompt);
    }
}

#[tokio::test]
async fn initial_association_storage_failure_rejects_without_any_external_launch() {
    let f = Fixture::new().await;
    let preview = f.preview().await;
    let _block = common::HistoryWriteBlock::new(
        &f.config
            .sessions
            .history_file
            .with_extension("delegations.json"),
    );
    let before = f.herdr.calls().len();
    let rejected = body(
        post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations",
            &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string(),
        )
        .await,
        500,
    );
    assert_eq!(rejected["code"], "delegation-storage-failed");
    assert_eq!(
        f.herdr.calls().len(),
        before,
        "a failed durable reservation never starts a Chat"
    );
}

// Existing authority is the independent source: removing chief-a and launching
// chief-b revokes old operations, even if an old preview remains in memory.
// The real qualifier stays on the path; no internal authority double is used.
#[tokio::test]
async fn revoked_chief_and_forged_payload_cannot_execute_a_saved_preview() {
    let f = Fixture::new().await;
    let preview = f.preview().await;
    let current = body(get_over_tcp(f.server.addr(), "/api/chief").await, 200);
    post(
        &f.server,
        "/api/chief/release",
        json!({"claimId":current["current"]["claimId"]}),
    )
    .await;
    f.identity("chief-b");
    let tasks = body(get_over_tcp(f.server.addr(), "/api/tasks").await, 200);
    let chief = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["taskType"] == "chief")
        .unwrap();
    let name = post(
        &f.server,
        "/api/chats/preview",
        json!({"zone":"work-zone","taskId":chief["id"]}),
    )
    .await;
    let started = body(
        post_json_over_tcp(
            f.server.addr(),
            "/api/chats",
            &json!({"zone":"work-zone","taskId":chief["id"],"chatId":name["chatId"]}).to_string(),
        )
        .await,
        202,
    );
    assert_eq!(
        terminal(&f.server, started["id"].as_str().unwrap()).await["stage"],
        "succeeded"
    );
    post(&f.server,"/api/sessions/events",json!({"sessionId":"chief-b","kind":"register","data":{"cwd":f.root.path().join("work"),"agentType":"pi"}})).await;
    let before = f.herdr.calls().len();
    let old_preview = body(
        post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations/preview",
            &f.request().to_string(),
        )
        .await,
        409,
    );
    assert_eq!(old_preview["code"], "chief-not-current");
    for session in ["chief-a", "chief-b", "unrelated-chat"] {
        let denied = body(
            post_json_over_tcp(
                f.server.addr(),
                "/api/chief/delegations",
                &json!({"sessionId":session,"previewId":preview["previewId"]}).to_string(),
            )
            .await,
            409,
        );
        assert_eq!(denied["code"], "chief-not-current");
    }
    let mut request = f.request();
    request["sessionId"] = json!("chief-b");
    let fresh = post(&f.server, "/api/chief/delegations/preview", request).await;
    body(post_json_over_tcp(f.server.addr(),"/api/chief/delegations",&json!({"sessionId":"chief-b","previewId":fresh["previewId"],"input":"unapproved replacement"}).to_string()).await,400);
    assert_eq!(
        f.herdr.calls().len(),
        before,
        "rejected calls never reach Herdr"
    );
}

// Independent outcome: approval covers the actual target and location, not only
// stable IDs. A plausible violation caches the old preview yet launches after a
// Bot retarget, Task edit or zone rebind. Observe rejection with no Herdr effect.
#[tokio::test]
async fn approval_rejects_changed_bot_task_or_zone_semantics_before_external_effects() {
    for change in ["bot", "task", "zone", "material-zone"] {
        let f = Fixture::new().await;
        let preview = f.preview().await;
        match change {
            "bot" => {
                let changed=post(&f.server,"/api/tasks",json!({"name":"different work","chatName":"different {chat.id}","agent":"pi","model":"default","prompt":"Different business"})).await;
                let task = changed["tasks"].as_array().unwrap().last().unwrap();
                let mut bot: Value = serde_json::from_str(
                    &fs::read_to_string(f.root.path().join("bots/worker.json")).unwrap(),
                )
                .unwrap();
                bot["task_id"] = task["id"].clone();
                fs::write(f.root.path().join("bots/worker.json"), bot.to_string()).unwrap();
            }
            "task" => {
                let mut task = f.worker.clone();
                task.as_object_mut().unwrap().remove("id");
                task["prompt"] = json!("Changed fixed instruction, not approved");
                body(
                    common::request_json_over_tcp(
                        f.server.addr(),
                        "PUT",
                        &format!("/api/tasks/{}", f.worker["id"].as_str().unwrap()),
                        Some(&task.to_string()),
                    )
                    .await,
                    200,
                );
            }
            _ => {
                let zone = if change == "zone" {
                    "work-zone"
                } else {
                    "material-zone"
                };
                fs::create_dir(f.root.path().join("changed location")).unwrap();
                fs::write(
                    f.root.path().join("changed location/brief.txt"),
                    "different material",
                )
                .unwrap();
                body(
                    common::request_json_over_tcp(
                        f.server.addr(),
                        "PUT",
                        "/api/zones",
                        Some(
                            &json!({"id":zone,"path":f.root.path().join("changed location")})
                                .to_string(),
                        ),
                    )
                    .await,
                    200,
                );
            }
        }
        let before = f.herdr.calls().len();
        let rejected = body(
            post_json_over_tcp(
                f.server.addr(),
                "/api/chief/delegations",
                &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string(),
            )
            .await,
            409,
        );
        assert_eq!(
            rejected["code"], "delegation-preview-stale",
            "{change}: {rejected}"
        );
        assert_eq!(
            f.herdr.calls().len(),
            before,
            "{change}: approval drift must not launch old or new work"
        );
    }
}

// Approved seam: S2 HTTP preview/commit/read and the external Herdr effect.
// Independent outcome: ticket 08 requires the explicitly named Bot, input, cwd,
// material and real native identity, with no fabricated Run or business completion.
// Plausible violation: reuse ordinary launch but lose provenance, change input,
// or return before associating the actual Chat. The worker never reports completion.
#[tokio::test]
async fn task_bot_launches_independent_chat_with_previewed_input_and_durable_sender_identity() {
    let f = Fixture::new().await;
    let before = f.herdr.calls().len();
    let preview = f.preview().await;
    assert_eq!(f.herdr.calls().len(), before, "preview cannot launch work");
    let facts = &preview["facts"];
    assert_eq!(facts["request"], f.request());
    assert_eq!(facts["bot"]["id"], "brief-bot");
    assert_eq!(facts["task"]["id"], f.worker["id"]);
    assert!(common::same_path(
        std::path::Path::new(facts["cwd"].as_str().unwrap()),
        &f.root.path().join("work")
    ));
    assert_eq!(facts["materials"][0]["id"], "brief");
    assert!(common::same_path(
        std::path::Path::new(facts["materials"][0]["path"].as_str().unwrap()),
        &f.root.path().join("materials/brief.txt")
    ));
    let prompt = facts["prompt"].as_str().unwrap();
    assert!(
        prompt
            .starts_with("Worker preset\n\nSummarize only the supplied brief; do not publish.\n\n")
    );
    assert!(
        prompt.contains("brief-bot") && prompt.contains("chief-a") && prompt.contains("brief.txt")
    );
    let accepted = body(
        post_json_over_tcp(
            f.server.addr(),
            "/api/chief/delegations",
            &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string(),
        )
        .await,
        202,
    );
    let id = accepted["id"].as_str().unwrap();
    let creation = terminal(&f.server, id).await;
    assert_eq!(creation["stage"], "succeeded", "{creation}");
    assert_eq!(creation["inputSubmitted"], true);
    assert!(creation.get("runId").is_none() && creation.get("executionId").is_none());
    let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
    assert_eq!(
        submitted.last().unwrap()["text"],
        facts["prompt"],
        "only the full previewed payload may be sent"
    );
    let tabs = f.herdr.tabs();
    assert!(common::same_path(
        std::path::Path::new(tabs.last().unwrap()["cwd"].as_str().unwrap()),
        &f.root.path().join("work")
    ));
    let record = body(
        get_over_tcp(f.server.addr(), &format!("/api/chief/delegations/{id}")).await,
        200,
    );
    assert_eq!(record["sessionId"], "worker-a");
    assert_eq!(record["facts"], *facts);
    assert_eq!(record["creation"]["stage"], "succeeded");
    assert!(
        record.get("businessCompleted").is_none(),
        "startup does not observe a business outcome"
    );
    f.identity("worker-b");
    let ordinary = post(
        &f.server,
        "/api/chats/preview",
        json!({"zone":"work-zone","taskId":f.worker["id"]}),
    )
    .await;
    let ordinary=body(post_json_over_tcp(f.server.addr(),"/api/chats",&json!({"zone":"work-zone","taskId":f.worker["id"],"chatId":ordinary["chatId"],"input":"Separate ordinary work"}).to_string()).await,202);
    assert_eq!(
        terminal(&f.server, ordinary["id"].as_str().unwrap()).await["stage"],
        "succeeded"
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let history = body(
                get_over_tcp(f.server.addr(), "/api/sessions/history").await,
                200,
            );
            if history["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|chat| chat["sessionId"] == "worker-b")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("ordinary same-Task native identity must be independently recorded");
    let unrelated = body(
        get_over_tcp(f.server.addr(), "/api/chief/delegations?sessionId=worker-b").await,
        200,
    );
    assert_eq!(
        unrelated["delegations"],
        json!([]),
        "an actual same-Task Chat has no delegation"
    );
    // New backend instance re-reads real storage; Resume must retain the native
    // sender without creating another delegation or replaying business input.
    f.identity("worker-a");
    drop(f.server);
    let recovered = TestServer::start(&f.config).await;
    let prompts_before = f
        .herdr
        .calls()
        .iter()
        .filter(|call| call["kind"] == "agent_prompt")
        .count();
    let resumed = post(
        &recovered,
        "/api/chats/resume",
        json!({"sessionId":"worker-a"}),
    )
    .await;
    assert!(
        ["restored", "located"].contains(&resumed["outcome"].as_str().unwrap()),
        "{resumed}"
    );
    assert_eq!(
        f.herdr
            .calls()
            .iter()
            .filter(|call| call["kind"] == "agent_prompt")
            .count(),
        prompts_before,
        "Resume does not replay initial input"
    );
    let after = body(
        get_over_tcp(
            recovered.addr(),
            "/api/chief/delegations?sessionId=worker-a",
        )
        .await,
        200,
    );
    assert_eq!(
        after["delegations"],
        json!([record]),
        "association survives backend replacement and actual Resume"
    );
}
