//! Approved S2/S3/S4 seams: real HTTP, external controlled host and disk records.
//! P8/P10/P13/P14/P15/P16: a prompt-first launch or a copied manager input
//! would violate independent literal expectations at the host boundary.
mod common;
#[path = "workflow_dispatch/recovery.rs"]
mod recovery;
use common::{HerdrFixture, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, time::Duration};

struct Scenario {
    data: TempDir,
    herdr: HerdrFixture,
    config: herdr_bot::Config,
    server: TestServer,
    zone: String,
    manager_task: String,
}
impl Scenario {
    async fn new() -> Self {
        let data = TempDir::new("dispatch records");
        let herdr = HerdrFixture::new("dispatch external");
        let fixtures = common::repo_fixtures_dir();
        let mut config = common::config(&fixtures.join("operations/success.bat"), &fixtures.join("operations"));
        config.tasks.storage_file = data.path().join("tasks.json");
        let mut catalog: toml::Value = toml::from_str(&fs::read_to_string(&config.tasks.metadata_file).unwrap()).unwrap();
        for entry in catalog["extensions"].as_array_mut().unwrap() {
            let path = data.path().join(format!("{}.ts", entry["id"].as_str().unwrap()));
            fs::write(&path, "export default function () {}\n").unwrap();
            entry["entrypoint"] = toml::Value::String(path.to_string_lossy().into_owned());
        }
        config.tasks.metadata_file = data.path().join("options.toml");
        fs::write(&config.tasks.metadata_file, toml::to_string(&catalog).unwrap()).unwrap();
        config.zones.registry_file = data.path().join("zones.json");
        config.scheduler.map_directory = data.path().join("maps");
        config.scheduler.bot_directory = data.path().join("bots");
        config.scheduler.run_directory = data.path().join("runs");
        config.scheduler.run_instructions = std::path::absolute(fixtures.parent().unwrap().join("docs/workflow-runtime.md")).unwrap();
        config.herdr.executable = herdr.executable().to_owned();
        herdr.set_workspaces(json!([{"workspace_id":"w-manager","label":data.path().file_name().unwrap().to_str().unwrap()}]));
        herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"manager-05"},"native_cwd":data.path()}));
        fs::create_dir_all(&config.scheduler.map_directory).unwrap();
        fs::create_dir_all(&config.scheduler.bot_directory).unwrap();
        let server = TestServer::start(&config).await;
        let node_zone = data.path().join("node-zone");
        fs::create_dir(&node_zone).unwrap();
        for path in [data.path(), node_zone.as_path()] {
            assert_eq!(post_json_over_tcp(server.addr(), "/api/zones", &json!({"path":path}).to_string()).await.status, 200);
        }
        let zone = data.path().to_string_lossy().into_owned();
        let mut ids = Vec::new();
        for role in ["manager", "node"] {
            let response = post_json_over_tcp(server.addr(), "/api/tasks", &json!({"name":role,"taskType":role,"agent":"omp","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":"teach","prompt":if role == "node" {"NODE-FIXED"} else {"MANAGER-FIXED"},"extensions":[]}).to_string()).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let body: Value = serde_json::from_str(&response.body).unwrap();
            ids.push(body["tasks"].as_array().unwrap().iter().find(|t| t["name"] == role).unwrap()["id"].as_str().unwrap().to_owned());
        }
        fs::write(config.scheduler.bot_directory.join("worker.json"), json!({"id":"worker","name":"worker","when":"BOT-WHEN-SECRET","plan_guide":"BOT-PLAN-SECRET","node_guide":"BOT-NODE-SECRET","task_id":ids[1]}).to_string()).unwrap();
        fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"map","description":"MAP-SECRET","nodes":{
            "A":{"title":"Worker A","bot_id":"worker","input":"NODE-INPUT\n第二行","zone":node_zone,"blocked_by":[]},
            "B":{"title":"Worker B","bot_id":"worker","input":"B-INPUT","zone":node_zone,"blocked_by":[]},
            "C":{"title":"Worker C","bot_id":"worker","input":"C-INPUT","zone":node_zone,"blocked_by":["A","B"]}
        }}).to_string()).unwrap();
        Self { data, herdr, config, server, zone, manager_task:ids.remove(0) }
    }
    async fn start(&self, identity: &str) -> Value {
        self.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":self.data.path()}));
        let response = post_json_over_tcp(self.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":self.manager_task,"zone":self.zone,"userInput":"MANAGER-USER-SECRET"}).to_string()).await;
        assert_eq!(response.status, 201, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    async fn dispatch(&self, identity: &str, nodes: &[&str]) -> Value {
        let response = post_json_over_tcp(self.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":identity,"nodes":nodes}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
}

// Approved S2/S3/S4 seams. P18.03/P20.03/P36.01/P68.04: pause only
// changes dispatch eligibility. A plausible violation stops A or blocks its
// report/complete along with B's dispatch. Keep real HTTP, persistence and host
// adapter calls in path; observe active A, literal rejection, first history and
// no external calls during the bounded 150ms pause window.
#[tokio::test]
async fn pause_blocks_new_dispatch_but_preserves_active_chat_handoff_and_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-pause").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-pause-A"},"native_cwd":s.data.path().join("node-zone")}));
    assert_eq!(s.dispatch("manager-pause", &["A"]).await["results"][0]["status"], "running");
    for event in [json!({"sessionId":"node-pause-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":"node-pause-A","kind":"state","data":{"value":"working"}})] {
        assert_eq!(post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await.status, 200);
    }
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &json!({"sessionId":"manager-pause"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let paused: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(paused["changed"], true);
    assert_eq!(paused["snapshot"]["status"], "paused");
    assert_eq!(paused["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(paused["ready"], json!([]));
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["action"], "pause");
    assert_eq!(events[1]["before"], "running");
    assert_eq!(events[1]["after"], "paused");
    assert_eq!(events[1]["actor"], started["run"]["manager"]);
    assert!(events[1]["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(events[1]["at"].as_u64().is_some());
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &json!({"sessionId":"manager-pause"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], paused["snapshot"]);
    let rejected = s.dispatch("manager-pause", &["B"]).await;
    assert_eq!(rejected["results"], json!([{"nodeId":"B","status":"rejected","reasons":["Run 状态为 paused"]}]));
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls, "pause must not stop or launch Chats");
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-pause-A","body":"A ready; cleanup continues while paused"}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let report: Value = serde_json::from_str(&report.body).unwrap();
    assert_eq!(report["saved"], true);
    assert_eq!(report["notification"]["status"], "submitted");
    let calls_after_report = s.herdr.calls();
    assert_eq!(calls_after_report.len(), calls.len() + 1, "only the requested report notification is sent");
    let complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-pause","nodeId":"A","reason":"Accepted while Chat remains working"}).to_string()).await;
    assert_eq!(complete.status, 200, "{}", complete.body);
    let complete: Value = serde_json::from_str(&complete.body).unwrap();
    assert_eq!(complete["snapshot"]["status"], "paused");
    assert_eq!(complete["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(complete["pendingReports"], json!({}));
    assert_eq!(complete["ready"], json!([]));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls_after_report);
    println!("P18.03/P20.03/P36.01/P68.04 observationWindowMs=150 pause={paused} rejected={rejected} report={report} complete={complete} callsBeforeReport={calls:?} callsAfterReport={calls_after_report:?} history={events:?}");
}

// P20.03/P37.01/P68.04 at S2/S3/S4. Contract source: resume changes
// paused -> running only. A plausible violation starts ready B automatically,
// or repeated resume overwrites the first transition. After real pause/resume,
// observe literal pending/ready facts, byte-identical repeated facts/history,
// and unchanged external calls for 150ms before explicitly dispatching B.
#[tokio::test]
async fn resume_restores_only_eligibility_until_explicit_dispatch_and_preserves_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-resume").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let body = json!({"sessionId":"manager-resume"}).to_string();
    let paused = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &body).await;
    assert_eq!(paused.status, 200, "{}", paused.body);
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", &body).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let resumed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(resumed["changed"], true);
    assert_eq!(resumed["snapshot"]["status"], "running");
    assert_eq!(resumed["ready"], json!(["A", "B"]));
    assert_eq!(resumed["snapshot"]["nodes"], started["run"]["nodes"]);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["action"], "resume");
    assert_eq!(events[1]["before"], "paused");
    assert_eq!(events[1]["after"], "running");
    assert_eq!(events[1]["actor"], started["run"]["manager"]);
    assert!(events[1]["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(events[1]["at"].as_u64().is_some());
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", &body).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], resumed["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &body).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], resumed[field]); }
    assert_eq!(s.herdr.calls(), calls, "resume must not start ready work");
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-resume-B"},"native_cwd":s.data.path().join("node-zone")}));
    let explicit = s.dispatch("manager-resume", &["B"]).await;
    assert_eq!(explicit["results"][0]["status"], "running");
    assert_eq!(explicit["snapshot"]["nodes"]["A"]["status"], "pending");
    let after = s.herdr.calls();
    assert_eq!(after.iter().filter(|call| call["kind"] == "agent_start").count(), 2, "only manager and explicit B start");
    println!("P20.03/P37.01/P68.04 observationWindowMs=150 resume={resumed} query={query} explicit={explicit} callsBeforeDispatch={calls:?} callsAfterDispatch={after:?} history={events:?}");
}

// P35.02 at S2/S3/S4: the approved finish condition is all nodes completed,
// even while paused and a Chat is working. A plausible violation requires
// resume/Chat exit, or permits terminal pause/resume. Reach this state through
// real dispatch/complete/pause (not a hand-built snapshot), then compare the
// first finish facts/history and all host calls after terminal requests.
#[tokio::test]
async fn paused_run_finishes_with_active_chat_and_terminal_eligibility_actions_are_rejected() {
    let s = Scenario::new().await;
    let started = s.start("manager-paused-finish").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let body = json!({"sessionId":"manager-paused-finish"}).to_string();
    for node in ["A", "B", "C"] {
        let identity = format!("node-paused-finish-{node}");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
        assert_eq!(s.dispatch("manager-paused-finish", &[node]).await["results"][0]["status"], "running");
        if node == "C" {
            for event in [json!({"sessionId":identity,"kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":identity,"kind":"state","data":{"value":"working"}})] {
                assert_eq!(post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await.status, 200);
            }
            let paused = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &body).await;
            assert_eq!(paused.status, 200, "{}", paused.body);
        }
        let complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-paused-finish","nodeId":node,"reason":"Ready for handoff; Chat keeps working"}).to_string()).await;
        assert_eq!(complete.status, 200, "{}", complete.body);
        if node == "C" {
            let complete: Value = serde_json::from_str(&complete.body).unwrap();
            assert_eq!(complete["snapshot"]["status"], "paused");
            assert_eq!(complete["canFinish"], true);
        }
    }
    let calls = s.herdr.calls();
    let finished = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &body).await;
    assert_eq!(finished.status, 200, "{}", finished.body);
    let finished: Value = serde_json::from_str(&finished.body).unwrap();
    assert_eq!(finished["changed"], true);
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["actor"], started["run"]["manager"]);
    assert_eq!(finished["canFinish"], false);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 8);
    assert_eq!(events[7]["action"], "finish");
    assert_eq!(events[7]["before"], "paused");
    assert_eq!(events[7]["after"], "completed");
    for action in ["pause", "resume"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body).await;
        assert_eq!(response.status, 409, "{}", response.body);
        let rejected: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(rejected["code"], "run-terminal");
        assert_eq!(rejected["changed"], false);
        assert_eq!(rejected["snapshot"], finished["snapshot"]);
    }
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &body).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], finished["snapshot"]);
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &body).await;
    assert_eq!(query.status, 200, "{}", query.body);
    assert_eq!(serde_json::from_str::<Value>(&query.body).unwrap()["snapshot"], finished["snapshot"]);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P35.02 observationWindowMs=150 finish={finished} repeat={repeated} history={events:?} calls={calls:?}");
}

// Same approved HTTP/host/record seams: eligibility actions belong only to
// the uniquely associated manager; unknown/node identities and self-reported
// ownership must not change facts or call the host. Expectations are literal
// role/validation outcomes, not derived from the runtime's response.
#[tokio::test]
async fn pause_resume_reject_non_manager_and_caller_supplied_ownership_without_effects() {
    let s = Scenario::new().await;
    let started = s.start("manager-eligibility-owner").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-eligibility-owner"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-eligibility-owner", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for action in ["pause", "resume"] {
        for (body, expected) in [
            (json!({"sessionId":"node-eligibility-owner"}), 403),
            (json!({"sessionId":"unassociated"}), 403),
            (json!({"sessionId":"manager-eligibility-owner","runId":started["id"]}), 400),
            (json!({"sessionId":"manager-eligibility-owner","nodeId":"A"}), 400),
            (json!({"sessionId":"manager-eligibility-owner","role":"manager"}), 400),
        ] {
            let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body.to_string()).await;
            assert_eq!(response.status, expected, "{}", response.body);
            println!("pause/resume caller guard action={action} expected={expected} result={}", response.body);
        }
    }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls(), calls);
}

// Approved seams: S2 HTTP manager actions/status, S3 external host calls,
// S4 persisted current facts/history. P17.02/P18.02/P20.02/P29.01/P68.03:
// A/B -> C independently requires both predecessors completed. A plausible
// violation is treating A's report or completion as permission to launch C.
// Observe literal blocked/ready fields, unchanged host calls over 150ms, and
// explicit completion's first reason/history through the real HTTP boundary.
#[tokio::test]
async fn explicit_complete_unlocks_only_satisfied_dependencies_and_preserves_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-complete").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let initial = fs::read(root.join("run.json")).unwrap();
    let initial_calls = s.herdr.calls();
    let pending = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"not started"}).to_string()).await;
    assert_eq!(pending.status, 409, "{}", pending.body);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), initial);
    assert!(common::read_json_lines(&root.join("history.jsonl")).is_empty());
    assert_eq!(s.herdr.calls(), initial_calls);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-complete-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-complete", &["A"]).await;
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"A artifact is ready; Chat still cleaning up"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let completed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(completed["changed"], true);
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["completion"]["reason"], "A artifact is ready; Chat still cleaning up");
    assert_eq!(completed["ready"], json!(["B"]));
    assert_eq!(completed["blocked"], json!({"A":["node 状态为 completed，仅 pending 可派发"],"B":[],"C":["前置 B 尚未 completed"]}));
    assert_eq!(completed["canFinish"], false);
    assert_eq!(s.herdr.calls(), calls);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["action"], "complete");
    assert_eq!(events[1]["before"], "running");
    assert_eq!(events[1]["after"], "completed");
    assert_eq!(events[1]["actor"], started["run"]["manager"]);
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"MUST NOT OVERWRITE"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], completed["snapshot"]);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let rejected = s.dispatch("manager-complete", &["C"]).await;
    assert_eq!(rejected["results"], json!([{"nodeId":"C","status":"rejected","reasons":["前置 B 尚未 completed"]}]));
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-complete"}).to_string()).await;
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], completed[field]); }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P17.02/P18.02/P20.02/P68.03: observationWindowMs=150 calls={calls:?} complete={completed} query={query}");
}

// P17.02/P18.02/P20.02/P35.01: finish cannot infer handoff from live
// Chat state. Independent graph expectations: C is ready only after A and B;
// all completed permits finish, then no node is dispatchable. A plausible
// violation overwrites the first finish or permits complete in terminal state.
#[tokio::test]
async fn finish_requires_every_handoff_and_terminal_actions_preserve_success() {
    let s = Scenario::new().await;
    let started = s.start("manager-finish").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let unfinished = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(unfinished.status, 409, "{}", unfinished.body);
    assert!(common::read_json_lines(&root.join("history.jsonl")).is_empty());
    for node in ["A", "B", "C"] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("node-finish-{node}")},"native_cwd":s.data.path().join("node-zone")}));
        let dispatched = s.dispatch("manager-finish", &[node]).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-finish","nodeId":node,"reason":format!("{node} handoff ready while Chat active")}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        if node == "B" {
            assert_eq!(value["ready"], json!(["C"]));
            assert_eq!(value["blocked"]["C"], json!([]));
            assert_eq!(value["canFinish"], false);
        }
        if node == "C" { assert_eq!(value["canFinish"], true); }
    }
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let finished: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(finished["changed"], true);
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["actor"], started["run"]["manager"]);
    assert_eq!(finished["ready"], json!([]));
    assert_eq!(finished["canFinish"], false);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 7);
    assert_eq!(events[6]["action"], "finish");
    assert_eq!(events[6]["before"], "running");
    assert_eq!(events[6]["after"], "completed");
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], finished["snapshot"]);
    let terminal_complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-finish","nodeId":"A","reason":"overwrite terminal"}).to_string()).await;
    assert_eq!(terminal_complete.status, 409, "{}", terminal_complete.body);
    let rejected = s.dispatch("manager-finish", &["A", "C"]).await;
    assert_eq!(rejected["results"], json!([
        {"nodeId":"A","status":"rejected","reasons":["Run 状态为 completed","node 状态为 completed，仅 pending 可派发"]},
        {"nodeId":"C","status":"rejected","reasons":["Run 状态为 completed","node 状态为 completed，仅 pending 可派发"]}
    ]));
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], finished[field]); }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P17.02/P18.02/P20.02/P35.01: observationWindowMs=150 calls={calls:?} finish={finished} query={query}");
}

// P28.02/P29.01/P68.03: reports, observed end-of-turn and session exit
// must leave A running, whereas explicit complete clears its actionable report.
// A plausible violation infers lifecycle completion or reopens work on a late
// report. Session events are real HTTP inputs, not evidence of native omp exit.
#[tokio::test]
async fn reports_and_observed_chat_lifecycle_never_replace_explicit_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-lifecycle").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-lifecycle-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-lifecycle", &["A"]).await;
    let first_run = fs::read(root.join("run.json")).unwrap();
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"A artifact ready; cleanup continues"}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let report: Value = serde_json::from_str(&report.body).unwrap();
    let calls = s.herdr.calls();
    for event in [
        json!({"sessionId":"node-lifecycle-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}),
        json!({"sessionId":"node-lifecycle-A","kind":"state","data":{"value":"awaiting-input"}}),
        json!({"sessionId":"node-lifecycle-A","kind":"unregister"})
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        tokio::time::sleep(Duration::from_millis(150)).await;
        let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
        assert_eq!(query.status, 200, "{}", query.body);
        let query: Value = serde_json::from_str(&query.body).unwrap();
        assert_eq!(query["snapshot"]["nodes"]["A"]["status"], "running");
        assert_eq!(query["snapshot"]["status"], "running");
        assert_eq!(query["pendingReports"], json!({"A":report["report"]["id"]}));
        assert_eq!(query["ready"], json!(["B"]));
        assert_eq!(fs::read(root.join("run.json")).unwrap(), first_run);
        assert_eq!(s.herdr.calls(), calls);
        let rejected = s.dispatch("manager-lifecycle", &["C"]).await;
        assert_eq!(rejected["results"][0]["reasons"], json!(["前置 A 尚未 completed","前置 B 尚未 completed"]));
        assert_eq!(s.herdr.calls(), calls);
        println!("P29.01/P68.03: observationWindowMs=150 event={event} calls={calls:?} query={query}");
    }
    // Register active work again: handoff and finish must not demand an exit.
    for event in [json!({"sessionId":"node-lifecycle-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}),json!({"sessionId":"node-lifecycle-A","kind":"state","data":{"value":"working"}})] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    for node in ["A", "B", "C"] {
        if node != "A" {
            s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("node-lifecycle-{node}")},"native_cwd":s.data.path().join("node-zone")}));
            assert_eq!(s.dispatch("manager-lifecycle", &[node]).await["results"][0]["status"], "running");
        }
        let calls_before_complete = s.herdr.calls();
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-lifecycle","nodeId":node,"reason":"Handoff accepted; Chat may keep working"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["pendingReports"], json!({}));
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(s.herdr.calls(), calls_before_complete);
        if node == "A" {
            let late = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"Post-handoff cleanup note"}).to_string()).await;
            assert_eq!(late.status, 200, "{}", late.body);
            let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
            let query: Value = serde_json::from_str(&query.body).unwrap();
            assert_eq!(query["pendingReports"], json!({}));
            assert_eq!(query["snapshot"], value["snapshot"]);
        }
        println!("P28.02/P68.03: postCompleteObservationWindowMs=150 calls={calls_before_complete:?} complete={value}");
    }
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let finished = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let late = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"Post-finish cleanup finished"}).to_string()).await;
    assert_eq!(late.status, 200, "{}", late.body);
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let query = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(query["run"]["status"], "completed");
    assert_eq!(query["run"]["nodes"]["A"]["status"], "completed");
    assert_eq!(query["pendingReports"], json!({}));
    assert_eq!(query["reports"].as_array().unwrap().len(), 3);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), finished);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls(), calls);
    println!("P28.02/P29.01/P68.03: terminalObservationWindowMs=150 calls={calls:?} detail={query}");
}

// P24.01/P27.01/P28.01: independent literals require two append-only reports
// from the real current node identity, never a state transition or read receipt.
// A report attributed from caller-supplied metadata or cleared by reads violates
// this scenario at the HTTP, persisted-message and external-input seams.
#[tokio::test]
async fn node_reports_are_system_attributed_append_only_and_do_not_complete_work() {
    let s = Scenario::new().await;
    let started = s.start("manager-report").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-report"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-report", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let mut reports = Vec::new();
    for body in ["Result: C:\\results\\one.txt; draft only", "Result: C:\\results\\two.txt; reviewed"] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-report","body":body}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], "submitted");
        assert_eq!(value["report"]["body"], body);
        reports.push(value["report"].clone());
    }
    assert_ne!(reports[0]["id"], reports[1]["id"]);
    for report in &reports {
        assert!(!report["id"].as_str().unwrap().is_empty());
        assert!(report["at"].as_u64().unwrap() >= started["run"]["createdAt"].as_u64().unwrap());
        assert_eq!(report["type"], "report");
        assert_eq!(report["runId"], id);
        assert_eq!(report["nodeId"], "A");
        let run: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(report["executionId"], run["nodes"]["A"]["execution"]["id"]);
        assert_eq!(report["source"]["sessionId"], "node-report");
        assert_eq!(report["target"]["sessionId"], "manager-report");
    }
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    for _ in 0..2 {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["reports"], json!(reports));
        assert_eq!(value["pendingReports"], json!({"A":reports[1]["id"]}));
    }
    let calls = s.herdr.calls();
    let inputs: Vec<_> = calls.iter().filter(|c| c["kind"] == "agent_prompt").collect();
    assert_eq!(inputs.len(), 4);
    for (input, report) in inputs[2..].iter().zip(&reports) {
        assert_eq!(input["target"], started["run"]["manager"]["agentName"]);
        assert!(input["text"].as_str().unwrap().contains(report["body"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains(report["id"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains("node: A"));
    }
}

// P25.01: a real Windows read-only messages file must reject persistence.
// The external adapter remains real so any accidental prompt is observable.
#[tokio::test]
async fn report_save_failure_has_no_message_pending_item_or_host_delivery() {
    let s = Scenario::new().await;
    let started = s.start("manager-save-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-save-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-save-error", &["A"]).await;
    let path = s.config.scheduler.run_directory.join(id).join("messages.jsonl");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-save-error","body":"must not persist or notify"}).to_string()).await;
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["code"], "report-save-failed");
    assert_eq!(value["saved"], false);
    assert_eq!(value["notification"]["status"], "not-attempted");
    assert!(fs::read(&path).unwrap().is_empty());
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reports"], json!([]));
    assert_eq!(value["pendingReports"], json!({}));
    assert_eq!(s.herdr.calls(), calls);
}

// P26.01/P28.01/P68.02: the external prompt gate holds delivery after
// persistence. Saved reports must already be visible to the manager; a failed
// single attempt must not pause or resend during the 300 ms post-result window.
#[tokio::test]
async fn delayed_failed_notification_preserves_saved_report_and_pending_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-notify-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-notify-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-notify-error", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let release = s.data.path().join("notify-release");
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/node/report", &json!({"sessionId":"node-notify-error","body":"SENSITIVE-RESULT: saved before notification"}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 3).await;
    assert!(!request.is_finished());
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(reports.len(), 1);
    let report_id = reports[0]["id"].clone();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-notify-error"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let status: Value = serde_json::from_str(&response.body).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(status["pendingReports"], json!({"A":report_id}));
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["saved"], true);
    assert_eq!(value["notification"]["status"], "failed");
    assert!(!value["notification"]["reason"].as_str().unwrap().is_empty());
    assert_eq!(value["report"]["id"], report_id);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    let calls = s.herdr.calls();
    assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
    println!("P26.01/P68.02 external calls after 300ms: {}", json!(calls));
}

// P24.01: neither a manager/unknown session nor forged ownership fields may
// produce a report or manager input. The model-facing body is the only business
// argument; sessionId belongs to the trusted public Session transport adapter.
#[tokio::test]
async fn report_rejects_non_node_identity_and_caller_supplied_ownership() {
    let s = Scenario::new().await;
    let started = s.start("manager-reject").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-reject"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-reject", &["A"]).await;
    let calls = s.herdr.calls();
    for (body, expected) in [
        (json!({"sessionId":"manager-reject","body":"manager cannot report"}), 403),
        (json!({"sessionId":"unassociated","body":"no binding"}), 403),
        (json!({"sessionId":"node-reject","body":"forged","runId":"another-run"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","nodeId":"B"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","target":"another-chat"}), 400),
        (json!({"sessionId":"node-reject"}), 400),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
        assert_eq!(response.status, expected, "{}", response.body);
    }
    assert!(fs::read(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("messages.jsonl")).unwrap().is_empty());
    assert_eq!(s.herdr.calls(), calls);
}

// P61.01/P68.02: isolate the production diagnostic sink in a child process.
// Closing its stderr pipe is a real OS write failure, not a mocked logger.
// Independent literals prohibit report text in diagnostics; 300 ms of actual
// external calls after the result distinguishes one attempt from retry loops.
#[tokio::test]
async fn report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries() {
    const MODE: &str = "HERDR_REPORT_DIAGNOSTIC_TEST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        let s = Scenario::new().await;
        let started = s.start("manager-diagnostic").await;
        let id = started["id"].as_str().unwrap();
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-diagnostic"},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-diagnostic", &["A"]).await;
        let before = fs::read(s.config.scheduler.run_directory.join(id).join("run.json")).unwrap();
        let failed = mode != "success";
        s.herdr.set_behavior(json!({"fail_prompt":failed}));
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-diagnostic","body":"SENSITIVE-DIAGNOSTIC-BODY"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], if failed {"failed"} else {"submitted"});
        assert_eq!(value["diagnostic"]["status"], if mode == "broken" {"failed"} else {"written"});
        tokio::time::sleep(Duration::from_millis(300)).await;
        let root = s.config.scheduler.run_directory.join(id);
        assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), vec![value["report"].clone()]);
        assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
        println!("REPORT-EVIDENCE {}", json!({"runId":id,"reportId":value["report"]["id"],"targetChat":started["run"]["manager"],"result":value["notification"]["status"],"externalCalls":calls,"observationMs":300}));
        return;
    }
    for mode in ["success", "failure", "broken"] {
        let cwd = TempDir::new("report diagnostic child cwd");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries", "--nocapture"])
            .env(MODE, mode).current_dir(cwd.path())
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
            .spawn().unwrap();
        if mode == "broken" { drop(child.stderr.take()); }
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{mode}\n{stdout}\n{stderr}");
        let evidence: Value = serde_json::from_str(stdout.lines().find_map(|line| line.strip_prefix("REPORT-EVIDENCE ")).unwrap()).unwrap();
        if mode != "broken" {
            assert!(!stderr.contains("SENSITIVE-DIAGNOSTIC-BODY"));
            let diagnostic: Value = serde_json::from_str(stderr.lines().find(|line| line.contains("workflow-report-delivery")).unwrap()).unwrap();
            assert_eq!(diagnostic["runId"], evidence["runId"]);
            assert_eq!(diagnostic["reportId"], evidence["reportId"]);
            assert_eq!(diagnostic["targetChat"], evidence["targetChat"]);
            assert_eq!(diagnostic["result"], evidence["result"]);
            assert!(!diagnostic["reason"].as_str().unwrap().is_empty());
        }
        println!("P61.01/P68.02 mode={mode} stderr={stderr} evidence={evidence}");
    }
}

#[tokio::test]
async fn node_dispatch_commits_identity_before_literal_trigger_and_uses_manager_workspace() {
    let s = Scenario::new().await;
    let started = s.start("manager-05").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let release = s.data.path().join("prompt-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-native-05"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-05","nodes":["A"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 2).await;
    let saved: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(saved["nodes"]["A"]["execution"]["chat"]["sessionId"], "node-native-05");
    let calls = s.herdr.calls();
    let prompt = calls.iter().filter(|c| c["kind"] == "agent_prompt").nth(1).unwrap();
    assert_eq!(prompt["text"], format!("/skill:teach NODE-FIXED\n\nNode ID: A\nNode: Worker A\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\nNODE-INPUT\n第二行", s.config.scheduler.run_instructions.display(),root.join("run.json").display(),root.join("history.jsonl").display(),root.join("messages.jsonl").display()));
    let start = calls.iter().filter(|c| c["kind"] == "agent_start").nth(1).unwrap();
    assert_eq!(&start["args"].as_array().unwrap()[..4], &[json!("--model"),json!("openai-codex/gpt-6.1-sol"),json!("--thinking"),json!("high")]);
    let tab = s.herdr.tabs().pop().unwrap();
    assert_eq!(tab["workspace"], "w-manager");
    let native = common::read_json_lines(&s.herdr.path().join("started.jsonl")).pop().unwrap();
    assert_eq!(native["actual_cwd"], s.data.path().join("node-zone").to_string_lossy().as_ref());
    assert_eq!(native["workspace_id"], "w-manager");
    assert_eq!(tab["cwd"], s.data.path().join("node-zone").to_string_lossy().as_ref());
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let result: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(result["results"][0]["status"], "running");
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
    let definition: Value = serde_json::from_slice(&fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap()).unwrap();
    assert_eq!(definition["nodes"]["A"]["input"], "NODE-INPUT\n第二行");
    assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await.status, 200);
}

// P11/P18/P19/P20/P38: one rejected target, one external start failure,
// one success; repeated dispatch must not create another Chat. The gate makes
// the failure deterministic without replacing the production host adapter.
#[tokio::test]
async fn mixed_batch_preserves_failures_success_and_other_run_without_automatic_actions() {
    let s = Scenario::new().await;
    let first = s.start("manager-first").await;
    let second = s.start("manager-second").await;
    let other_path = s.config.scheduler.run_directory.join(second["id"].as_str().unwrap()).join("run.json");
    let other_bytes = fs::read(&other_path).unwrap();
    let release = s.data.path().join("start-release");
    s.herdr.set_behavior(json!({"fail_agent_start":true,"agent_start_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"manager-first","nodes":["C","A","B"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count() == 3).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-B-native"}}));
    fs::write(release,b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status,200,"{}",response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(batch["results"][0]["status"],"rejected");
    assert_eq!(batch["results"][1]["status"],"failed");
    assert_eq!(batch["results"][1]["failure"]["stage"],"start");
    assert_eq!(batch["results"][2]["status"],"running");
    assert_eq!(batch["snapshot"]["status"],"running");
    let root = s.config.scheduler.run_directory.join(first["id"].as_str().unwrap());
    let disk: Value = serde_json::from_slice(&fs::read(root.join("run.json")).unwrap()).unwrap();
    assert_eq!(disk["nodes"]["A"]["failure"],batch["results"][1]["failure"]);
    assert!(disk["nodes"]["A"]["failure"]["message"].as_str().unwrap().contains("受控夹具拒绝启动"));
    let history = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(history.len(),2);
    assert_eq!(history[0]["nodeId"],"A");
    assert_eq!(history[0]["status"],"failed");
    assert_eq!(history[1]["nodeId"],"B");
    assert_eq!(history[1]["status"],"running");
    let calls = s.herdr.calls();
    let repeated = s.dispatch("manager-first", &["A","B"]).await;
    assert_eq!(repeated["results"][0]["status"],"rejected");
    assert_eq!(repeated["results"][1]["status"],"rejected");
    assert_eq!(repeated["snapshot"],disk);
    let queried = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-first"}).to_string()).await;
    assert_eq!(serde_json::from_str::<Value>(&queried.body).unwrap()["snapshot"],disk);
    let other = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-second"}).to_string()).await;
    assert_eq!(serde_json::from_str::<Value>(&other.body).unwrap()["snapshot"],second["run"]);
    assert_eq!(fs::read(other_path).unwrap(),other_bytes);
    // Bounded negative observation: no retry, terminate, pause or hidden launch
    // during requests plus the following 100ms; not an infinite-time claim.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(s.herdr.calls(),calls);
    assert_eq!(fs::read_to_string(root.join("history.jsonl")).unwrap().lines().count(),2);
}

// P13.02/P45.02: identity and submit failures leave the external Chat intact;
// identity failure must never submit. These criteria already pass the launch
// implementation, so verify them without manufacturing an unrelated red.
#[tokio::test]
async fn identity_and_submit_failures_keep_external_effects_in_query_and_disk() {
    for stage in ["identity", "submit"] {
        let s = Scenario::new().await;
        let started = s.start("manager-errors").await;
        s.herdr.set_behavior(if stage == "identity" { json!({"fail_agent_get":true}) } else { json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-submit-failed"},"fail_prompt":true}) });
        let batch = s.dispatch("manager-errors", &["A"]).await;
        let failure = &batch["results"][0]["failure"];
        assert_eq!(failure["stage"],stage);
        assert_eq!(failure["effects"]["agentStarted"],true);
        assert_eq!(failure["effects"]["inputSubmitted"],false);
        assert_eq!(failure["effects"]["associationSaved"],stage == "submit");
        assert_eq!(failure["effects"]["inputAttempted"],stage == "submit");
        assert!(failure["effects"]["agentName"].is_string());
        let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
        let disk: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(disk["nodes"]["A"]["failure"],*failure);
        if stage == "submit" {
            assert_eq!(disk["nodes"]["A"]["execution"]["chat"]["sessionId"],"node-submit-failed");
        }
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(),if stage == "identity" {1} else {2});
        assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(),2);
        let queried = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-errors"}).to_string()).await;
        assert_eq!(serde_json::from_str::<Value>(&queried.body).unwrap()["snapshot"],disk);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(s.herdr.calls(),calls);
    }
}

#[tokio::test]
async fn association_write_failure_exposes_native_chat_without_submitting_or_claiming_rollback() {
    let s = Scenario::new().await;
    let started = s.start("manager-association").await;
    let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
    let original = fs::read(&path).unwrap();
    let release = s.data.path().join("identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-association-failed"},"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr,"/api/workflow/manager/dispatch",&json!({"sessionId":"manager-association","nodes":["A"]}).to_string()).await });
    common::wait_until(Duration::from_secs(5),|| s.herdr.calls().iter().filter(|c| c["kind"] == "agent_get").count() == 2).await;
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(release,b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(response.status,200,"{}",response.body);
    let batch: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(batch["results"][0]["failure"]["stage"],"association");
    assert_eq!(batch["results"][0]["effects"]["sessionId"],"node-association-failed");
    assert_eq!(batch["results"][0]["effects"]["agentStarted"],true);
    assert_eq!(batch["results"][0]["effects"]["associationSaved"],false);
    assert!(batch["results"][0]["persistenceError"].is_string());
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
    assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(),2);
    // Storage remains unavailable: response/diagnostic carry effects, and the
    // snapshot must not adopt the uncommitted candidate. No fake disk success.
    assert_eq!(batch["snapshot"],started["run"]);
    fs::remove_dir(&path).unwrap();
    fs::write(path,original).unwrap();
}

#[tokio::test]
async fn missing_manager_workspace_never_falls_back_or_creates_a_tab() {
    let s = Scenario::new().await;
    s.start("manager-workspace").await;
    s.herdr.set_workspaces(json!([{"workspace_id":"w-default","label":"bot-playground","focused":true}]));
    let before = s.herdr.tabs();
    let batch = s.dispatch("manager-workspace", &["A"]).await;
    assert_eq!(batch["results"][0]["failure"]["stage"],"workspace");
    assert_eq!(batch["results"][0]["effects"]["agentStarted"],false);
    assert_eq!(s.herdr.tabs(),before);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_start").count(),1);
}

// A new Tab is insufficient: replaying the manager native identity must fail
// without sending node instructions, even though the host created another pane.
#[tokio::test]
async fn new_tab_with_reused_native_identity_is_not_accepted_as_independent_chat() {
    let s = Scenario::new().await;
    s.start("same-native").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"same-native"}}));
    let batch = s.dispatch("same-native", &["A"]).await;
    assert_eq!(batch["results"][0]["status"],"failed");
    assert_eq!(batch["results"][0]["failure"]["stage"],"identity");
    assert_eq!(batch["results"][0]["effects"]["sessionId"],"same-native");
    assert_eq!(s.herdr.tabs().len(),2);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(),1);
}

#[tokio::test]
async fn manager_query_derives_ready_targets_and_rejects_unassociated_dispatch() {
    let s = Scenario::new().await;
    s.start("manager-ready").await;
    let status = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/status",&json!({"sessionId":"manager-ready"}).to_string()).await;
    assert_eq!(status.status,200,"{}",status.body);
    let query: Value = serde_json::from_str(&status.body).unwrap();
    assert_eq!(query["ready"],json!(["A","B"]));
    assert_eq!(query["blocked"]["C"],json!(["前置 A 尚未 completed","前置 B 尚未 completed"]));
    assert_eq!(query["canFinish"],false);
    let calls = s.herdr.calls();
    for body in [json!({"sessionId":"stranger","nodes":["A"]}),json!({"sessionId":"manager-ready","nodes":["A"],"runId":"invented"})] {
        let response = post_json_over_tcp(s.server.addr(),"/api/workflow/manager/dispatch",&body.to_string()).await;
        assert!(response.status == 403 || response.status == 400,"{}",response.body);
    }
    assert_eq!(s.herdr.calls(),calls);
}
