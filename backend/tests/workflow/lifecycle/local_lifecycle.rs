//! P27/P28/P47 approved seams: public definition/workflow HTTP, real durable
//! records and controlled external Herdr CLI effects (not native agent quality).
use super::*;

async fn request(s: &Scenario, route: &str, body: Value, status: u16) -> Value {
    let response = tokio::time::timeout(Duration::from_secs(10), post_json_over_tcp(s.server.addr(), route, &body.to_string())).await.expect("public action must make bounded progress");
    assert_eq!(response.status, status, "{route}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn action(s: &Scenario, session: &str, name: &str, mut body: Value, status: u16) -> Value {
    body["sessionId"] = json!(session);
    request(s, &format!("/api/workflow/manager/{name}"), body, status).await
}
fn native(s: &Scenario, session: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
}
async fn detail(s: &Scenario, id: &Value) -> Value {
    let response = tokio::time::timeout(Duration::from_secs(10), get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", id.as_str().unwrap()))).await.expect("Run read must progress");
    assert_eq!(response.status, 200, "{}", response.body);
    let detail: Value = serde_json::from_str(&response.body).unwrap();
    let disk: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(id.as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(detail["run"], disk, "public and independently read durable facts must agree");
    detail
}
async fn setup(s: &Scenario) {
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{
        "inside":{"title":"Inside","bot_id":"worker","input":"INSIDE","zone":"node","blocked_by":[]},
        "sibling":{"title":"Sibling","bot_id":"worker","input":"SIBLING","zone":"node","blocked_by":[]}
    }}).to_string()).unwrap();
    request(s, "/api/scheduler/bots/save", json!({"id":"nested","content":json!({"name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string(),"revision":null}), 200).await;
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{}}).to_string()).unwrap();
    for (id, content) in [
        ("call", json!({"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"CALL","zone":"node","blocked_by":[]})),
        ("branch", json!({"title":"Independent branch","bot_id":"worker","input":"BRANCH","zone":"node","blocked_by":[]})),
        ("later", json!({"title":"Later","bot_id":"worker","input":"LATER","zone":"node","blocked_by":[]})),
    ] {
        let response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
        assert_eq!(response.status, 200, "{}", response.body);
        let draft: Value = serde_json::from_str(&response.body).unwrap();
        request(s, "/api/scheduler/maps/work.json/nodes/save", json!({"id":id,"content":content.to_string(),"revision":draft["revision"],"create":true}), 200).await;
    }
}
async fn report(s: &Scenario, session: &str, body: &str) -> Value {
    request(s, "/api/workflow/node/report", json!({"sessionId":session,"body":body,"canComplete":true}), 200).await
}

// P27 independent outcome: pause only changes the parent's eligibility. A
// cascading pause would reject the child's NEW dispatch or its explicit finish;
// an auto-resume dispatcher would add host effects without an explicit action.
#[tokio::test]
async fn parent_pause_rejects_local_dispatch_retry_but_child_work_and_handoff_continue() {
    let s = Scenario::new().await;
    setup(&s).await;
    let root = s.start("pause-root").await;
    native(&s, "pause-child");
    let launched = s.dispatch("pause-root", &["call"]).await;
    let child = launched["snapshot"]["nodes"]["call"]["execution"]["childRunId"].clone();
    native(&s, "pause-failed");
    s.dispatch("pause-root", &["branch"]).await;
    action(&s, "pause-root", "fail", json!({"nodeId":"branch","reason":"Controlled local execution failure"}), 200).await;
    action(&s, "pause-root", "pause", json!({}), 200).await;
    let before = detail(&s, &root["id"]).await;
    let calls = s.herdr.calls();
    // Dispatch is a per-target batch response (HTTP 200), unlike retry's
    // action-level 409. Rejection and zero host effects are the criterion.
    assert_eq!(action(&s, "pause-root", "dispatch", json!({"nodes":["later"]}), 200).await["results"], json!([{"nodeId":"later","status":"rejected","reasons":["Run 状态为 paused"]}]));
    assert_eq!(action(&s, "pause-root", "retry", json!({"nodeId":"branch","confirmed":true,"reason":"Explicit retry"}), 409).await["code"], "run-paused");
    assert_eq!(detail(&s, &root["id"]).await, before);
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(detail(&s, &child).await["run"]["status"], "running");
    for (node, identity) in [("inside", "pause-inside"), ("sibling", "pause-sibling")] {
        native(&s, identity);
        let dispatched = s.dispatch("pause-child", &[node]).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        assert_eq!(dispatched["snapshot"]["nodes"][node]["execution"]["chat"]["sessionId"], identity);
        report(&s, identity, "Child work while parent paused").await;
        action(&s, "pause-child", "complete", json!({"nodeId":node,"reason":"Accepted"}), 200).await;
    }
    action(&s, "pause-child", "finish", json!({}), 200).await;
    report(&s, "pause-child", "Finished child while parent paused").await;
    let parent = detail(&s, &root["id"]).await;
    assert_eq!(parent["run"]["status"], "paused");
    assert_eq!(parent["run"]["nodes"]["call"]["status"], "running");
    action(&s, "pause-root", "complete", json!({"nodeId":"call","reason":"Explicit parent acceptance"}), 200).await;
    let calls = s.herdr.calls();
    let resumed = action(&s, "pause-root", "resume", json!({}), 200).await;
    assert_eq!(resumed["snapshot"]["status"], "running");
    assert_eq!(resumed["snapshot"]["nodes"]["branch"]["status"], "failed");
    assert_eq!(resumed["snapshot"]["nodes"]["later"]["status"], "pending");
    assert_eq!(s.herdr.calls(), calls, "resume is eligibility only");
    native(&s, "pause-retry");
    let retried = action(&s, "pause-root", "retry", json!({"nodeId":"branch","confirmed":true,"reason":"Explicitly authorized after resume"}), 200).await;
    assert_eq!(retried["result"]["status"], "running");
    assert_eq!(detail(&s, &child).await["run"]["status"], "completed");
    let submitted = common::read_json_lines(&s.herdr.path().join("submitted.jsonl"));
    assert!(submitted.iter().any(|entry| entry["text"].as_str().is_some_and(|text| text.contains("Node ID: inside"))));
    assert!(submitted.iter().any(|entry| entry["text"].as_str().is_some_and(|text| text.contains("Finished child while parent paused"))));
}

// P28: an actual host submission refusal in a grandchild may fail that node,
// never its two ancestors or their independent branches. Literal states alone
// would miss a global stop: start and submit three real sibling/independent Chats.
#[tokio::test]
async fn descendant_host_failure_keeps_ancestors_and_independent_sessions_running() {
    let s = Scenario::new().await;
    setup(&s).await;
    request(&s, "/api/scheduler/bots/save", json!({"id":"deep","content":json!({"name":"Deep","when":"","plan_guide":"","node_guide":"","map_file":"leaf.json"}).to_string(),"revision":null}), 200).await;
    fs::write(s.config.scheduler.map_directory.join("leaf.json"), json!({"title":"Leaf","description":"","nodes":{"work":{"title":"Leaf work","bot_id":"worker","input":"LEAF-WORK","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{
        "inside":{"title":"Deep call","bot_id":"deep","manager_task_id":s.manager_task,"input":"DEEP","zone":"node","blocked_by":[]},
        "sibling":{"title":"Sibling","bot_id":"worker","input":"SIBLING","zone":"node","blocked_by":[]}
    }}).to_string()).unwrap();
    let root = s.start("failure-root").await;
    let independent = s.start("failure-independent").await;
    native(&s, "failure-child");
    let launched = s.dispatch("failure-root", &["call"]).await;
    let child = launched["snapshot"]["nodes"]["call"]["execution"]["childRunId"].clone();
    native(&s, "failure-grandchild");
    let launched = s.dispatch("failure-child", &["inside"]).await;
    let grandchild = launched["snapshot"]["nodes"]["inside"]["execution"]["childRunId"].clone();
    let root_before = detail(&s, &root["id"]).await;
    let child_before = detail(&s, &child).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"failure-leaf"},"native_cwd":s.data.path().join("node-zone"),"fail_prompt":true}));
    let failed = s.dispatch("failure-grandchild", &["work"]).await;
    assert_eq!(failed["results"][0]["status"], "failed");
    assert_eq!(failed["results"][0]["failure"]["stage"], "submit");
    assert_eq!(failed["results"][0]["effects"]["inputOutcome"], "rejected");
    let leaf = detail(&s, &grandchild).await;
    assert_eq!(leaf["run"]["status"], "running");
    assert_eq!(leaf["run"]["nodes"]["work"]["status"], "failed");
    assert_eq!(detail(&s, &root["id"]).await, root_before);
    assert_eq!(detail(&s, &child).await, child_before);
    let accepted_before = common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).len();
    for (manager, node, identity) in [("failure-child", "sibling", "failure-sibling"), ("failure-root", "branch", "failure-branch"), ("failure-independent", "branch", "failure-other")] {
        native(&s, identity);
        let advanced = s.dispatch(manager, &[node]).await;
        assert_eq!(advanced["results"][0]["status"], "running");
        assert_eq!(advanced["snapshot"]["status"], "running");
        assert_eq!(advanced["snapshot"]["nodes"][node]["execution"]["chat"]["sessionId"], identity);
        report(&s, identity, "Independent branch delivered").await;
        action(&s, manager, "complete", json!({"nodeId":node,"reason":"Sibling independently accepted"}), 200).await;
    }
    assert_eq!(detail(&s, &independent["id"]).await["run"]["nodes"]["branch"]["status"], "completed");
    assert_eq!(detail(&s, &root["id"]).await["run"]["nodes"]["call"]["status"], "running");
    assert_eq!(detail(&s, &child).await["run"]["nodes"]["inside"]["status"], "running");
    assert_eq!(detail(&s, &grandchild).await, leaf, "no automatic recovery");
    assert_eq!(common::read_json_lines(&s.herdr.path().join("submitted.jsonl")).len(), accepted_before + 6, "three actual initial submissions and three notifications");
    assert!(!s.herdr.path().join("closed.jsonl").exists(), "no cascading termination");
}

// P47 independent expected outcome: child handoff and an independent launch
// progress while parent startup is blocked, then parent complete progresses
// while both report notifications remain blocked. A parent/child lock inversion
// or report holding an operation permit across host delivery fails a bounded
// action, even if a later final-state-only test would eventually pass.
#[tokio::test]
async fn parent_child_handoff_and_independent_run_progress_across_slow_host_effects() {
    use futures_util::FutureExt;
    let s = Scenario::new().await;
    setup(&s).await;
    let root = s.start("concurrent-root").await;
    let independent = s.start("concurrent-independent").await;
    let gates = ["launch-release", "inner-release", "outer-release"].map(|name| s.data.path().join(name));
    let mut tasks = Vec::new();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"concurrent-child"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":gates[0],"timeout_ms":30000}}));
    let count = s.herdr.calls().len();
    let parent = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", r#"{"sessionId":"concurrent-root","nodes":["call"]}"#).await;
    tasks.push(tokio::spawn(parent.response()));
    let observation = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls()[count..].iter().any(|call| call["kind"] == "agent_prompt")).await, "parent initial input must reach external gate");
        let during = detail(&s, &root["id"]).await;
        let child = during["run"]["nodes"]["call"]["execution"]["childRunId"].clone();
        assert_eq!(during["run"]["nodes"]["call"]["status"], "pending");
        for (node, identity) in [("sibling", "concurrent-sibling"), ("inside", "concurrent-inside")] {
            native(&s, identity);
            let child_dispatch = tokio::time::timeout(Duration::from_secs(5), s.dispatch("concurrent-child", &[node])).await.expect("child dispatch must progress before parent input gate release");
            assert_eq!(child_dispatch["results"][0]["status"], "running");
            if node == "sibling" {
                report(&s, identity, "Sibling finished").await;
                action(&s, "concurrent-child", "complete", json!({"nodeId":node,"reason":"Accepted"}), 200).await;
            }
        }
        s.herdr.set_behavior(json!({"prompt_gate":{"match":"INNER-SLOW","wait_for_file":gates[1],"timeout_ms":30000}}));
        let inner = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"concurrent-inside","body":"INNER-SLOW","canComplete":true}"#).await;
        tasks.push(tokio::spawn(inner.response()));
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.contains("INNER-SLOW")))).await);
        assert_eq!(detail(&s, &child).await["reports"][1]["body"], "INNER-SLOW");
        action(&s, "concurrent-child", "complete", json!({"nodeId":"inside","reason":"Saved report is sufficient despite blocked notification"}), 200).await;
        assert_eq!(action(&s, "concurrent-child", "finish", json!({}), 200).await["snapshot"]["status"], "completed");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"concurrent-other"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"match":"OUTER-SLOW","wait_for_file":gates[2],"timeout_ms":30000}}));
        let outer = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"concurrent-child","body":"OUTER-SLOW","canComplete":true}"#).await;
        tasks.push(tokio::spawn(outer.response()));
        assert!(tokio::time::timeout(Duration::from_millis(200), &mut tasks[2]).await.is_err(), "outward report waits for in-flight parent dispatch");
        let other = tokio::time::timeout(Duration::from_secs(5), s.dispatch("concurrent-independent", &["branch"])).await.expect("independent Run must launch before parent gate release");
        assert_eq!(other["results"][0]["status"], "running");
        assert_eq!(detail(&s, &independent["id"]).await["run"]["nodes"]["branch"]["execution"]["chat"]["sessionId"], "concurrent-other");
        fs::write(&gates[0], "release").unwrap();
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.contains("OUTER-SLOW")))).await);
        assert_eq!(detail(&s, &root["id"]).await["reports"][0]["body"], "OUTER-SLOW");
        assert_eq!(action(&s, "concurrent-root", "complete", json!({"nodeId":"call","reason":"Explicit acceptance during slow notifications"}), 200).await["snapshot"]["nodes"]["call"]["status"], "completed");
        assert!(!tasks[1].is_finished() && !tasks[2].is_finished(), "both external notification gates must still be held");
        assert!(!gates[1].exists() && !gates[2].exists());
        assert_eq!(detail(&s, &child).await["run"]["status"], "completed");
        assert_eq!(detail(&s, &root["id"]).await["run"]["nodes"]["call"]["status"], "completed");
    }).catch_unwind().await;
    // No assertion may skip release/drain. Keep every cleanup error alongside
    // the original panic instead of swallowing it or replacing it with cleanup.
    let mut cleanup = Vec::new();
    for gate in &gates {
        if let Err(error) = fs::write(gate, "release") { cleanup.push(error.to_string()); }
    }
    for task in tasks {
        match tokio::time::timeout(Duration::from_secs(35), task).await {
            Ok(Ok(response)) if response.status == 200 => {
                match serde_json::from_str::<Value>(&response.body) {
                    Ok(value) if value.get("saved") == Some(&json!(true)) => {
                        if value["notification"]["status"] != "submitted" { cleanup.push(response.body); }
                    }
                    Ok(value) if value["results"][0]["status"] == "running" => {},
                    _ => cleanup.push(response.body),
                }
            }
            result => cleanup.push(format!("accepted request drain failed: {result:?}")),
        }
    }
    if let Err(primary) = observation {
        let message = primary.downcast_ref::<String>().map(String::as_str).or_else(|| primary.downcast_ref::<&str>().copied()).unwrap_or("non-string assertion failure");
        panic!("primary observation: {message}; cleanup errors: {cleanup:?}");
    }
    assert!(cleanup.is_empty(), "cleanup errors: {cleanup:?}");
}
