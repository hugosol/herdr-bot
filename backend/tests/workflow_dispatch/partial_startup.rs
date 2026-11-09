//! P31 approved seams: public definition/run HTTP, real temporary persistence,
//! and the external controlled Herdr adapter. Reconciliation is observation,
//! never authorization to replay startup or erase a registered child.
use super::*;

async fn setup(session: &str) -> (Scenario, Value) {
    let s = Scenario::new().await;
    fs::write(s.config.scheduler.bot_directory.join("nested.json"), json!({"id":"nested","name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"A":{"title":"Work","bot_id":"worker","input":"WORK","zone":"manager","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{"A":{"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"CHILD-INPUT-731","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    let root = s.start(session).await;
    (s, root)
}
async fn action(s: &Scenario, route: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{route}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
fn disk(s: &Scenario, id: &str) -> Value {
    serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(id).join("run.json")).unwrap()).unwrap()
}
async fn child_detail(s: &Scenario, id: &str) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// Expected from P31: a refused Chat start does not undo child registration.
// Plausible violation: not_created clears the parent attempt just because no
// native Chat exists, orphaning its already durable child. Observe all four
// boundaries before/after reconciliation, including actual started records.
#[tokio::test]
async fn partial_child_registered_without_chat_cannot_be_reconciled_away() {
    let (s, root) = setup("partial-root").await;
    s.herdr.set_behavior(json!({"fail_agent_start":true}));
    let dispatched = s.dispatch("partial-root", &["A"]).await;
    let result = &dispatched["results"][0];
    assert_eq!(result["failure"]["stage"], "start");
    assert_eq!(result["effects"]["childRecordsInitialized"], true);
    assert_eq!(result["effects"]["agentStarted"], false);
    let execution = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    let child = execution["childRunId"].as_str().unwrap();
    assert_eq!(disk(&s, child)["parent"]["runId"], root["id"]);
    assert!(child_detail(&s, child).await["run"]["manager"].is_null());
    let calls = s.herdr.calls();
    let started = common::read_json_lines(&s.herdr.path().join("started.jsonl"));
    assert_eq!(started.len(), 1, "only the root manager actually started");
    let uncertain = action(&s, "reconcile_dispatch", json!({"sessionId":"partial-root","nodeId":"A","finding":"uncertain","reason":"Child exists; Chat start rejected"}), 200).await;
    assert_eq!(uncertain["snapshot"]["nodes"]["A"]["status"], "failed");
    let absent = action(&s, "reconcile_dispatch", json!({"sessionId":"partial-root","nodeId":"A","finding":"not_created","reason":"No native Chat"}), 409).await;
    assert_eq!(absent["code"], "dispatch-child-already-known");
    assert_eq!(disk(&s, root["id"].as_str().unwrap()), uncertain["snapshot"]);
    assert_eq!(disk(&s, root["id"].as_str().unwrap())["nodes"]["A"]["execution"], execution);
    assert_eq!(child_detail(&s, child).await["run"], disk(&s, child));
    assert_eq!(s.herdr.calls(), calls);
    action(&s, "retry", json!({"sessionId":"partial-root","nodeId":"A","confirmed":true,"reason":"Try blindly"}), 409).await;
    assert_eq!(s.herdr.calls(), calls);
}

// P31: a real Windows replacement denial after native startup leaves the
// child unassociated. Created reconciliation must repair BOTH original links;
// parent-only repair would falsely report success but leave both roles unusable.
#[tokio::test]
async fn partial_child_association_reconciliation_preserves_the_original_chat_without_input_replay() {
    use futures_util::FutureExt;
    let (s, root) = setup("association-root").await;
    let release = s.data.path().join("identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"association-child"},"native_cwd":s.data.path().join("node-zone"),"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let request = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", r#"{"sessionId":"association-root","nodes":["A"]}"#).await;
    let observation = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_get").count() == 2).await);
        let parent = disk(&s, root["id"].as_str().unwrap());
        let child = parent["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap().to_owned();
        let lock = common::HistoryWriteBlock::new(&s.config.scheduler.run_directory.join(&child).join("run.json"));
        (child, lock)
    }).catch_unwind().await;
    fs::write(&release, "release").unwrap();
    let response = request.response().await;
    let (child, lock) = observation.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    drop(lock);
    assert_eq!(response.status, 200, "{}", response.body);
    let failed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(failed["results"][0]["failure"]["stage"], "child-association");
    assert_eq!(failed["results"][0]["effects"]["sessionId"], "association-child");
    assert_eq!(failed["results"][0]["effects"]["inputAttempted"], false);
    assert!(disk(&s, &child)["manager"].is_null());
    let before = s.herdr.calls();
    let repaired = action(&s, "reconcile_dispatch", json!({"sessionId":"association-root","nodeId":"A","finding":"created","reason":"Read exact original host identity after releasing disk lock"}), 200).await;
    let chat = &repaired["snapshot"]["nodes"]["A"]["execution"]["chat"];
    assert_eq!(chat["sessionId"], "association-child");
    assert_eq!(disk(&s, &child)["manager"], *chat);
    assert_eq!(child_detail(&s, &child).await["run"], disk(&s, &child));
    assert_eq!(disk(&s, root["id"].as_str().unwrap()), repaired["snapshot"]);
    let status = action(&s, "status", json!({"sessionId":"association-child"}), 200).await;
    assert_eq!(status["snapshot"]["id"], child);
    let after = s.herdr.calls();
    assert!(after[before.len()..].iter().all(|call| call["kind"] == "agent_get"));
    assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(), 2);
    assert_eq!(after.iter().filter(|call| call["kind"] == "agent_prompt").count(), 1, "only root input was submitted");
}

// P31: child preflight rejects a newly required interface before registration.
// Unlike a registered child with no Chat, this confirmed absence may restore
// pending, but reconciliation itself still performs no dispatch or input.
#[tokio::test]
async fn partial_child_not_created_is_distinct_from_registered_without_chat() {
    let (s, root) = setup("absent-root").await;
    let map = s.config.scheduler.map_directory.join("inner.json");
    let mut definition: Value = serde_json::from_slice(&fs::read(&map).unwrap()).unwrap();
    definition["input_files"] = json!([{"id":"missing-material","required":true}]);
    fs::write(&map, definition.to_string()).unwrap();
    let calls = s.herdr.calls();
    let failed = s.dispatch("absent-root", &["A"]).await;
    assert_eq!(failed["results"][0]["failure"]["stage"], "child-preflight");
    assert!(failed["results"][0]["effects"].get("childRunId").is_none());
    assert!(failed["snapshot"]["nodes"]["A"]["execution"].get("childRunId").is_none());
    assert_eq!(fs::read_dir(&s.config.scheduler.run_directory).unwrap().count(), 1);
    let observed = action(&s, "reconcile_dispatch", json!({"sessionId":"absent-root","nodeId":"A","finding":"uncertain","reason":"Observe preflight rejection"}), 200).await;
    assert_eq!(observed["snapshot"], failed["snapshot"]);
    let absent = action(&s, "reconcile_dispatch", json!({"sessionId":"absent-root","nodeId":"A","finding":"not_created","reason":"Preflight rejected before registration; independent run catalog has only parent"}), 200).await;
    assert_eq!(absent["snapshot"]["nodes"]["A"]["status"], "pending");
    assert!(absent["snapshot"]["nodes"]["A"]["execution"].is_null());
    assert_eq!(disk(&s, root["id"].as_str().unwrap()), absent["snapshot"]);
    let listed = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(listed.status, 200, "{}", listed.body);
    assert_eq!(serde_json::from_str::<Value>(&listed.body).unwrap()["runs"].as_array().unwrap().len(), 1);
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(), 1);
    assert_eq!(common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).len(), 1);
}

// The owned run root denies creating child directories, not replacing files
// inside the already initialized parent. This produces a real OS failure after
// durable child registration without a product fault-injection interface.
struct DenyChildDirectories(std::path::PathBuf);
impl DenyChildDirectories {
    fn change(path: &std::path::Path, remove: bool) {
        let script = r#"$ErrorActionPreference='Stop'; $p=$env:P31_ACL_PATH; $acl=[System.IO.Directory]::GetAccessControl($p); $sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User; $rule=[System.Security.AccessControl.FileSystemAccessRule]::new($sid,[System.Security.AccessControl.FileSystemRights]::CreateDirectories,[System.Security.AccessControl.AccessControlType]::Deny); if ($env:P31_ACL_REMOVE -eq 'true') {$acl.RemoveAccessRuleSpecific($rule)} else {$acl.AddAccessRule($rule)}; [System.IO.Directory]::SetAccessControl($p,$acl)"#;
        let output = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", script]).env("P31_ACL_PATH", path).env("P31_ACL_REMOVE", remove.to_string()).output().unwrap();
        assert!(output.status.success(), "ACL operation: {} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
    fn new(path: &std::path::Path) -> Self {
        Self::change(path, false);
        Self(path.to_owned())
    }
}
impl Drop for DenyChildDirectories {
    fn drop(&mut self) { Self::change(&self.0, true); }
}

// P31: initialized records are not a prerequisite for acknowledging a durable
// child registration. A list based only on readable run.json would lose it;
// an absent-Chat reconciliation would erase it. Neither may launch a host.
#[tokio::test]
async fn partial_child_registration_survives_failed_record_initialization() {
    let (s, root) = setup("initialize-root").await;
    let calls = s.herdr.calls();
    let denial = DenyChildDirectories::new(&s.config.scheduler.run_directory);
    let failed = s.dispatch("initialize-root", &["A"]).await;
    drop(denial);
    assert_eq!(failed["results"][0]["failure"]["stage"], "child-initialize");
    assert_eq!(failed["results"][0]["effects"]["childRecordsInitialized"], false);
    let child = failed["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
    assert_eq!(fs::read(s.config.scheduler.run_directory.join(child).join("run.json")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert_eq!(disk(&s, root["id"].as_str().unwrap()), failed["snapshot"]);
    let detail = child_detail(&s, child).await;
    assert!(detail["run"].is_null());
    assert_eq!(detail["startupError"]["stage"], "child-initialize");
    assert!(detail["readError"].is_string());
    let response = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let listed: Value = serde_json::from_str(&response.body).unwrap();
    assert!(listed["runs"].as_array().unwrap().iter().any(|run| run["id"] == child && run["run"].is_null() && run["readError"].is_string()));
    let observed = action(&s, "reconcile_dispatch", json!({"sessionId":"initialize-root","nodeId":"A","finding":"uncertain","reason":"Child registration exists but disk initialization failed"}), 200).await;
    assert_eq!(observed["snapshot"], failed["snapshot"]);
    let rejected = action(&s, "reconcile_dispatch", json!({"sessionId":"initialize-root","nodeId":"A","finding":"not_created","reason":"No Chat"}), 409).await;
    assert_eq!(rejected["code"], "dispatch-child-already-known");
    action(&s, "reconcile_dispatch", json!({"sessionId":"initialize-root","nodeId":"A","finding":"created","reason":"No identity exists"}), 409).await;
    assert_eq!(s.herdr.calls(), calls);
}

// P31 independent oracle: explicit host refusal has no submitted effect;
// malformed acknowledgement AFTER the host consumed input is unknown, not
// rejected. Reconciliation reads identity only and cannot turn either into
// permission to retry or claim a successful submission.
#[tokio::test]
async fn partial_child_submission_distinguishes_rejected_and_unknown_without_replaying() {
    for (flag, outcome, submitted) in [("fail_prompt", "rejected", 1), ("invalid_prompt_response", "unknown", 2)] {
        let (s, root) = setup("submit-root").await;
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"submit-child"},"native_cwd":s.data.path().join("node-zone"),flag:true}));
        let failed = s.dispatch("submit-root", &["A"]).await;
        let result = &failed["results"][0];
        assert_eq!(result["failure"]["stage"], "submit");
        assert_eq!(result["effects"]["inputAttempted"], true);
        assert_eq!(result["effects"]["inputSubmitted"], false);
        assert_eq!(result["effects"]["inputOutcome"], outcome);
        let child = failed["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
        let chat = &failed["snapshot"]["nodes"]["A"]["execution"]["chat"];
        assert_eq!(disk(&s, child)["manager"], *chat);
        assert_eq!(child_detail(&s, child).await["run"], disk(&s, child));
        assert_eq!(common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).len(), submitted);
        let calls = s.herdr.calls();
        let unknown = action(&s, "reconcile_dispatch", json!({"sessionId":"submit-root","nodeId":"A","finding":"uncertain","reason":"No verified input acknowledgement"}), 200).await;
        assert_eq!(unknown["snapshot"], failed["snapshot"]);
        action(&s, "retry", json!({"sessionId":"submit-root","nodeId":"A","confirmed":true,"reason":"Blind retry"}), 409).await;
        // A known Chat is NOT evidence that its trigger input was accepted.
        let observed = action(&s, "reconcile_dispatch", json!({"sessionId":"submit-root","nodeId":"A","finding":"created","reason":"Observe the original Chat only"}), 200).await;
        assert_eq!(observed["snapshot"]["nodes"]["A"]["status"], "failed");
        assert_eq!(observed["snapshot"]["nodes"]["A"]["failure"], failed["snapshot"]["nodes"]["A"]["failure"]);
        action(&s, "retry", json!({"sessionId":"submit-root","nodeId":"A","confirmed":true,"reason":"Still blind retry"}), 409).await;
        assert_eq!(disk(&s, root["id"].as_str().unwrap()), observed["snapshot"]);
        let after = s.herdr.calls();
        assert!(after[calls.len()..].iter().all(|call| call["kind"] == "agent_get"));
        assert_eq!(common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).len(), submitted);
        assert_eq!(common::read_json_lines(&s.herdr.path().join("started.jsonl")).len(), 2);
    }
}
