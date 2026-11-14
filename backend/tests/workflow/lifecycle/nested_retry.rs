//! P29/P30: approved public HTTP, real persisted records/files and controlled
//! external Herdr. Native model retry/resume responsibility belongs to ticket 15.
use super::*;

async fn request(s: &Scenario, route: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), route, &body.to_string()).await;
    assert_eq!(response.status, status, "{route}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn action(s: &Scenario, session: &str, name: &str, mut body: Value, status: u16) -> Value {
    body["sessionId"] = json!(session);
    request(s, &format!("/api/workflow/manager/{name}"), body, status).await
}
fn native(s: &Scenario, session: &str) {
    input_resolution::identity(s, session);
}
fn records(s: &Scenario, id: &Value) -> [Vec<u8>; 3] {
    let root = s.config.scheduler.run_directory.join(id.as_str().unwrap());
    ["run.json", "history.jsonl", "messages.jsonl"].map(|file| fs::read(root.join(file)).unwrap())
}
async fn setup(s: &Scenario) {
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"A":{"title":"Same node ID, child scope","bot_id":"worker","input":"INNER","zone":"node","blocked_by":[],"output_rule":[{"id":"result"},{"id":"missing"}]}}}).to_string()).unwrap();
    request(s, "/api/scheduler/bots/save", json!({"id":"nested","content":json!({"name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string(),"revision":null}), 200).await;
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{
        "A":{"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"CALL","zone":"node","blocked_by":[],"output_rule":[{"id":"result"},{"id":"missing"}]},
        "consumer":{"title":"Consumer","bot_id":"worker","input":"CONSUME","zone":"node","blocked_by":["A"],"input_rule":[{"id":"material","type":"node","node_id":"A","output_id":"result"}]},
        "missing":{"title":"No fallback","bot_id":"worker","input":"NO FALLBACK","zone":"node","blocked_by":["A"],"input_rule":[{"id":"material","type":"node","node_id":"A","output_id":"missing"}]}
    }}).to_string()).unwrap();
}
async fn finish_child(s: &Scenario, session: &str, worker: &str, outputs: Value) {
    native(s, worker);
    s.dispatch(session, &["A"]).await;
    request(s, "/api/workflow/node/report", json!({"sessionId":worker,"body":"Internal delivery","canComplete":true,"outputs":outputs}), 200).await;
    action(s, session, "complete", json!({"nodeId":"A","reason":"Accept internal result"}), 200).await;
    action(s, session, "finish", json!({}), 200).await;
}
async fn report(s: &Scenario, session: &str, ready: bool, outputs: Value) -> Value {
    let value = request(s, "/api/workflow/node/report", json!({"sessionId":session,"body":format!("Delivery by {session}"),"canComplete":ready,"outputs":outputs}), 200).await;
    assert_eq!(value["saved"], true);
    assert_eq!(value["notification"]["status"], "submitted");
    let delivered = common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).pop().unwrap();
    assert_eq!(delivered["target"], value["report"]["to"]["agentName"]);
    assert!(delivered["text"].as_str().unwrap().contains(value["report"]["id"].as_str().unwrap()));
    value["report"].clone()
}

// Expected P29: confirmation creates independent execution/child/Chat, while
// recover and Chat resume preserve the original. Violation: recycle child or
// erase its records/side effects. Observe all identities, bytes and host input.
#[tokio::test]
async fn confirmed_nested_retry_preserves_old_child_and_effects_unlike_recover_or_chat_resume() {
    let s = Scenario::new().await;
    setup(&s).await;
    let root = s.start("p29-root").await;
    native(&s, "p29-old");
    let first = s.dispatch("p29-root", &["A"]).await;
    let old = first["snapshot"]["nodes"]["A"]["execution"].clone();
    let effect = s.data.path().join("old-external-effect.txt");
    fs::write(&effect, "SIDE-EFFECT-731").unwrap();
    action(&s, "p29-root", "fail", json!({"nodeId":"A","reason":"Partial business effect confirmed"}), 200).await;
    let before = records(&s, &root["id"]);
    let calls = s.herdr.calls();
    for (confirmed, reason) in [(false, "Not authorized"), (true, "  ")] {
        let rejected = action(&s, "p29-root", "retry", json!({"nodeId":"A","confirmed":confirmed,"reason":reason}), 409).await;
        assert_eq!(rejected["code"], "retry-confirmation-required");
        assert_eq!(records(&s, &root["id"]), before);
    }
    assert_eq!(s.herdr.calls(), calls);
    let recovered = action(&s, "p29-root", "recover", json!({"nodeId":"A","confirmed":true,"reason":"Original Chat can continue"}), 200).await;
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["execution"], old);
    assert_eq!(s.herdr.calls(), calls);
    let parent_before_resume = records(&s, &root["id"]);
    let child_before_resume = records(&s, &old["childRunId"]);
    native(&s, "p29-old");
    request(&s, "/api/chats/resume", json!({"sessionId":"p29-old"}), 200).await;
    assert_eq!(records(&s, &root["id"]), parent_before_resume);
    assert_eq!(records(&s, &old["childRunId"]), child_before_resume);
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count(), 2, "resume must not replay initial input");
    action(&s, "p29-root", "fail", json!({"nodeId":"A","reason":"Original cannot continue; effect inspected"}), 200).await;
    let child_before_retry = records(&s, &old["childRunId"]);
    native(&s, "p29-new");
    let retried = action(&s, "p29-root", "retry", json!({"nodeId":"A","confirmed":true,"reason":"Authorize new independent work after effect inspection"}), 200).await;
    let node = &retried["snapshot"]["nodes"]["A"];
    let current = &node["execution"];
    assert_ne!(current["id"], old["id"]);
    assert_ne!(current["childRunId"], old["childRunId"]);
    assert_eq!(current["chat"]["sessionId"], "p29-new");
    assert_ne!(current["chat"], old["chat"]);
    assert_eq!(node["previousExecutions"][0]["execution"], old);
    assert_eq!(node["previousExecutions"][0]["failure"]["message"], "Original cannot continue; effect inspected");
    assert_eq!(records(&s, &old["childRunId"]), child_before_retry);
    let child: Value = serde_json::from_slice(&records(&s, &current["childRunId"])[0]).unwrap();
    assert_eq!(child["parent"], json!({"runId":root["id"],"nodeId":"A","executionId":current["id"]}));
    assert_eq!(child["manager"], current["chat"]);
    assert_eq!(fs::read_to_string(effect).unwrap(), "SIDE-EFFECT-731");
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count(), 3);
    let history = common::read_json_lines(&s.config.scheduler.run_directory.join(root["id"].as_str().unwrap()).join("history.jsonl"));
    assert!(history.iter().any(|event| event["action"] == "retry" && event["previousExecution"] == old));
}

// Expected P30: late true reports keep history/notification but never qualify
// current completion or supply its missing output. Violation: latest by node,
// recursive child registry, old execution fallback, or cross-Run global lookup.
// Observe real consumer initialization plus real file contents and disk logs.
#[tokio::test]
async fn interleaved_nested_reports_isolate_pending_completion_and_actual_consumers() {
    let s = Scenario::new().await;
    setup(&s).await;
    let old_file = s.data.path().join("old.txt");
    let new_file = s.data.path().join("new.txt");
    let decoy = s.data.path().join("other-scope.txt");
    for (path, text) in [(&old_file, "OLD-413"), (&new_file, "NEW-829"), (&decoy, "WRONG-SCOPE-957")] { fs::write(path, text).unwrap(); }
    let root = s.start("p30-root").await;
    native(&s, "p30-old");
    let old = s.dispatch("p30-root", &["A"]).await["snapshot"]["nodes"]["A"]["execution"].clone();
    finish_child(&s, "p30-old", "p30-old-worker", json!([])).await;
    let old_outputs = json!([{"id":"result","path":old_file},{"id":"missing","path":old_file}]);
    let first = report(&s, "p30-old", true, old_outputs.clone()).await;
    action(&s, "p30-root", "fail", json!({"nodeId":"A","reason":"Old result rejected"}), 200).await;
    native(&s, "p30-new");
    let retry = action(&s, "p30-root", "retry", json!({"nodeId":"A","confirmed":true,"reason":"Inspected old child and effects; replace independently"}), 200).await;
    let current = retry["snapshot"]["nodes"]["A"]["execution"].clone();
    let late = report(&s, "p30-old", true, old_outputs.clone()).await;
    let status = action(&s, "p30-root", "status", json!({}), 200).await;
    assert!(status["pendingReports"].get("A").is_none());
    assert_eq!(action(&s, "p30-root", "complete", json!({"nodeId":"A","reason":"Old true must not qualify"}), 409).await["code"], "report-not-completable");
    let progress = report(&s, "p30-new", false, json!([])).await;
    let later = report(&s, "p30-old", true, old_outputs.clone()).await;
    let collect = action(&s, "p30-root", "collect", json!({}), 200).await;
    assert_eq!(collect["pendingReports"]["A"], progress["id"]);
    assert_eq!(action(&s, "p30-root", "complete", json!({"nodeId":"A","reason":"Current false overrides old true"}), 409).await["code"], "report-not-completable");
    // Same node/output names in a different Run and in the current child may
    // not supply the parent's missing output.
    let other = s.start("p30-other-root").await;
    native(&s, "p30-other-child");
    s.dispatch("p30-other-root", &["A"]).await;
    report(&s, "p30-other-child", false, json!([{"id":"missing","path":decoy}])).await;
    finish_child(&s, "p30-new", "p30-new-worker", json!([{"id":"result","path":decoy},{"id":"missing","path":decoy}])).await;
    let ready = report(&s, "p30-new", true, json!([{"id":"result","path":new_file}])).await;
    let last = report(&s, "p30-old", true, old_outputs).await;
    let collect = action(&s, "p30-root", "collect", json!({}), 200).await;
    assert_eq!(collect["pendingReports"]["A"], ready["id"]);
    action(&s, "p30-root", "complete", json!({"nodeId":"A","reason":"Accept new execution only"}), 200).await;
    let calls = s.herdr.calls();
    let denied = s.dispatch("p30-root", &["missing"]).await;
    assert_eq!(denied["results"][0]["failure"]["stage"], "resolve");
    assert_eq!(denied["results"][0]["effects"]["agentStarted"], false);
    assert_eq!(s.herdr.calls(), calls);
    native(&s, "p30-consumer");
    let consumed = s.dispatch("p30-root", &["consumer"]).await;
    let inputs = &consumed["snapshot"]["nodes"]["consumer"]["execution"]["inputs"];
    assert_eq!(*inputs, json!([{"id":"material","path":new_file}]));
    let text = input_resolution::initial_input(&s);
    assert_eq!(input_resolution::named_input(&text), format!("material: {}\n", new_file.display()));
    assert_eq!(fs::read_to_string(inputs[0]["path"].as_str().unwrap()).unwrap(), "NEW-829");
    let expected = vec![first, late, progress, later, ready, last];
    let reports = common::read_json_lines(&s.config.scheduler.run_directory.join(root["id"].as_str().unwrap()).join("messages.jsonl"));
    assert_eq!(reports, expected);
    for index in [0, 1, 3, 5] { assert_eq!(reports[index]["executionId"], old["id"]); }
    for index in [2, 4] { assert_eq!(reports[index]["executionId"], current["id"]); }
    assert_ne!(other["id"], root["id"]);
    assert_eq!(fs::read_to_string(old_file).unwrap(), "OLD-413");
    assert_eq!(action(&s, "p30-root", "status", json!({}), 200).await["pendingReports"], json!({}));
}
