//! Approved seams: public workflow/definition HTTP, real files and controlled
//! Herdr CLI effects. No native role/Tab-lifetime claim (ticket 14 owns those).
use super::*;

async fn request(s: &Scenario, route: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), route, &body.to_string()).await;
    assert_eq!(response.status, status, "{route}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn action(s: &Scenario, session: &str, name: &str, extra: Value, status: u16) -> Value {
    let mut body = extra;
    body["sessionId"] = json!(session);
    request(s, &format!("/api/workflow/manager/{name}"), body, status).await
}
fn native(s: &Scenario, session: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
}
async fn read_run(s: &Scenario, id: &Value) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", id.as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn setup(s: &Scenario) {
    // Maps are files; Bot creation and node editing use their public API.
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"inside":{"title":"Internal work","bot_id":"worker","input":"INTERNAL","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    request(s, "/api/scheduler/bots/save", json!({"id":"nested","content":json!({"name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string(),"revision":null}), 200).await;
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{}}).to_string()).unwrap();
    for (id, content) in [
        ("call", json!({"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"CALL","zone":"node","blocked_by":[],"input_rule":[],"output_rule":[{"id":"result"},{"id":"missing"}],"auto_close":true})),
        ("consumer", json!({"title":"Consumer","bot_id":"worker","input":"CONSUME","zone":"node","blocked_by":["call"],"input_rule":[{"type":"node","id":"material","node_id":"call","output_id":"result"}],"output_rule":[]})),
    ] {
        let response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
        assert_eq!(response.status, 200, "{}", response.body);
        let draft: Value = serde_json::from_str(&response.body).unwrap();
        request(s, "/api/scheduler/maps/work.json/nodes/save", json!({"id":id,"content":content.to_string(),"revision":draft["revision"],"create":true}), 200).await;
    }
}

// P23/P24: a child with unfinished work OR all nodes completed but no explicit
// finish must not report readiness. Violation: ordinary-node report acceptance
// ignores child status. Observe HTTP rejection, no append and no host delivery.
#[tokio::test]
async fn nested_completion_requires_explicit_child_finish_before_ready_report() {
    let s = Scenario::new().await;
    setup(&s).await;
    let root = s.start("finish-root").await;
    native(&s, "finish-child");
    let dispatched = s.dispatch("finish-root", &["call"]).await;
    let child = dispatched["snapshot"]["nodes"]["call"]["execution"]["childRunId"].clone();
    let calls = s.herdr.calls();
    let rejected = request(&s, "/api/workflow/node/report", json!({"sessionId":"finish-child","body":"Too early","canComplete":true}), 400).await;
    assert_eq!(rejected["code"], "child-run-not-finished");
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(read_run(&s, &root["id"]).await["reports"], json!([]));
    let progress = request(&s, "/api/workflow/node/report", json!({"sessionId":"finish-child","body":"Progress only","canComplete":false}), 200).await;
    assert_eq!(progress["report"]["runId"], root["id"]);
    let rejected = action(&s, "finish-root", "complete", json!({"nodeId":"call","reason":"Progress is insufficient"}), 409).await;
    assert_eq!(rejected["code"], "report-not-completable");
    native(&s, "finish-inside");
    s.dispatch("finish-child", &["inside"]).await;
    submit_report(&s, "finish-inside", "Internal done", true).await;
    action(&s, "finish-child", "complete", json!({"nodeId":"inside","reason":"Internal accepted"}), 200).await;
    let before = read_run(&s, &root["id"]).await;
    let calls = s.herdr.calls();
    let rejected = request(&s, "/api/workflow/node/report", json!({"sessionId":"finish-child","body":"Still not finished","canComplete":true}), 400).await;
    assert_eq!(rejected["code"], "child-run-not-finished");
    assert_eq!(read_run(&s, &root["id"]).await, before);
    assert_eq!(s.herdr.calls(), calls);
    let finished = action(&s, "finish-child", "finish", json!({}), 200).await;
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(read_run(&s, &child).await["run"]["status"], "completed");
    assert_eq!(read_run(&s, &root["id"]).await, before);
    request(&s, "/api/workflow/node/report", json!({"sessionId":"finish-child","body":"Finished, no registered outputs","canComplete":true}), 200).await;
    let after = read_run(&s, &root["id"]).await;
    assert_eq!(after["run"]["nodes"]["call"]["status"], "running");
    assert_eq!(after["run"]["nodes"]["consumer"]["status"], "pending");
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
}

async fn finish_child(s: &Scenario, child: &str, inside: &str) {
    native(s, inside);
    s.dispatch(child, &["inside"]).await;
    submit_report(s, inside, "Internal delivery", true).await;
    action(s, child, "complete", json!({"nodeId":"inside","reason":"Accepted internal delivery"}), 200).await;
    action(s, child, "finish", json!({}), 200).await;
}

async fn add_node(s: &Scenario, id: &str, content: Value) {
    let response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let draft: Value = serde_json::from_str(&response.body).unwrap();
    request(s, "/api/scheduler/maps/work.json/nodes/save", json!({"id":id,"content":content.to_string(),"revision":draft["revision"],"create":true}), 200).await;
}

// P17/P18: frozen parent output IDs, report deltas and omission survive nested
// delivery; both target kinds bind the newest registered address. A live-Map
// validator, replace-all registry or recursive child-output lookup fails here.
#[tokio::test]
async fn nested_outputs_use_frozen_parent_rules_and_feed_both_downstream_kinds() {
    let s = Scenario::new().await;
    setup(&s).await;
    fs::write(s.config.scheduler.map_directory.join("sink.json"), json!({"title":"Sink","description":"","input_files":[{"id":"renamed","required":true}],"nodes":{"sink-work":{"title":"Sink work","bot_id":"worker","input":"Sink","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    request(&s, "/api/scheduler/bots/save", json!({"id":"sink","content":json!({"name":"Sink","when":"","plan_guide":"","node_guide":"","map_file":"sink.json"}).to_string(),"revision":null}), 200).await;
    add_node(&s, "map-consumer", json!({"title":"Map consumer","bot_id":"sink","manager_task_id":s.manager_task,"input":"MAP-CONSUME","zone":"node","blocked_by":["call"],"input_rule":[{"type":"node","id":"renamed","node_id":"call","output_id":"result"}]})).await;
    add_node(&s, "retained-consumer", json!({"title":"Retained consumer","bot_id":"worker","input":"Retained","zone":"node","blocked_by":["call"],"input_rule":[{"type":"node","id":"retained","node_id":"call","output_id":"missing"}]})).await;
    let root = s.start("output-root").await;
    native(&s, "output-child");
    let launched = s.dispatch("output-root", &["call"]).await;
    let execution = launched["snapshot"]["nodes"]["call"]["execution"].clone();
    finish_child(&s, "output-child", "output-inside").await;
    let draft = request(&s, "/api/scheduler/maps/work.json/nodes/edit", json!({"id":"call"}), 200).await;
    let mut content = draft["content"].clone();
    content["output_rule"] = json!([{"id":"result"},{"id":"missing"},{"id":"new-only"}]);
    request(&s, "/api/scheduler/maps/work.json/nodes/save", json!({"id":"call","content":content.to_string(),"revision":draft["revision"],"create":false}), 200).await;
    let old = s.data.path().join("old.txt");
    let new = s.data.path().join("new.txt");
    let retained = s.data.path().join("retained.txt");
    for (path, bytes) in [(&old,"OLD-371"),(&new,"NEW-849"),(&retained,"RETAINED-562")] { fs::write(path, bytes).unwrap(); }
    let calls = s.herdr.calls();
    for outputs in [json!([{"id":"new-only","path":new}]), json!([{"id":"result","path":old},{"id":"result","path":new}])] {
        request(&s, "/api/workflow/node/report", json!({"sessionId":"output-child","body":"Invalid","canComplete":true,"outputs":outputs}), 400).await;
    }
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(read_run(&s, &root["id"]).await["reports"], json!([]));
    for outputs in [Some(json!([{"id":"result","path":old},{"id":"missing","path":retained}])),Some(json!([{"id":"result","path":new}])),None] {
        let mut body = json!({"sessionId":"output-child","body":"Delivery delta","canComplete":true});
        if let Some(outputs) = outputs { body["outputs"] = outputs; }
        request(&s, "/api/workflow/node/report", body, 200).await;
    }
    let before = read_run(&s, &root["id"]).await;
    assert_eq!(before["reports"][2]["outputs"], json!([]));
    assert_eq!(before["run"]["nodes"]["call"]["execution"], execution);
    let blocked = s.dispatch("output-root", &["consumer","map-consumer"]).await;
    assert_eq!(blocked["snapshot"]["nodes"]["consumer"]["status"], "pending");
    assert_eq!(blocked["snapshot"]["nodes"]["map-consumer"]["status"], "pending");
    action(&s, "output-root", "complete", json!({"nodeId":"call","reason":"Accept parent delivery"}), 200).await;
    native(&s, "output-consumer");
    let ordinary = s.dispatch("output-root", &["consumer"]).await;
    assert_eq!(ordinary["snapshot"]["nodes"]["consumer"]["execution"]["inputs"], json!([{"id":"material","path":new}]));
    native(&s, "output-map-consumer");
    let mapped = s.dispatch("output-root", &["map-consumer"]).await;
    let consumer = &mapped["snapshot"]["nodes"]["map-consumer"]["execution"];
    assert_eq!(consumer["inputs"], json!([{"id":"renamed","path":new}]));
    assert_eq!(read_run(&s, &consumer["childRunId"]).await["run"]["fileInputs"], json!([{"id":"renamed","path":new}]));
    native(&s, "output-retained-consumer");
    let bound = s.dispatch("output-root", &["retained-consumer"]).await;
    assert_eq!(bound["snapshot"]["nodes"]["retained-consumer"]["execution"]["inputs"], json!([{"id":"retained","path":retained}]));
    assert_eq!(fs::read_to_string(new).unwrap(), "NEW-849");
    assert_eq!(fs::read_to_string(retained).unwrap(), "RETAINED-562");
    assert!(read_run(&s, &execution["childRunId"]).await["run"].get("outputFiles").is_none());
    wait_for_close(&s).await;
}

// P19: a previous child delivered the required file; its replacement finishes
// without outputs and is accepted. Dispatch must reject rather than inherit
// that old file or invent an address. Finish never requires all output IDs.
#[tokio::test]
async fn finished_child_without_current_output_cannot_reuse_previous_delivery() {
    let s = Scenario::new().await;
    setup(&s).await;
    let inner_path = s.config.scheduler.map_directory.join("inner.json");
    let mut inner: Value = serde_json::from_slice(&fs::read(&inner_path).unwrap()).unwrap();
    inner["input_files"] = json!([{"id":"material","required":false}]);
    fs::write(inner_path, inner.to_string()).unwrap();
    add_node(&s, "missing-map", json!({"title":"Missing Map consumer","bot_id":"nested","manager_task_id":s.manager_task,"input":"Consume","zone":"node","blocked_by":["call"],"input_rule":[{"type":"node","id":"material","node_id":"call","output_id":"result"}]})).await;
    let root = s.start("missing-root").await;
    native(&s, "missing-old-child");
    let first = s.dispatch("missing-root", &["call"]).await;
    finish_child(&s, "missing-old-child", "missing-old-inside").await;
    let old = s.data.path().join("previous-delivery.txt");
    fs::write(&old, "DO-NOT-INHERIT-631").unwrap();
    request(&s, "/api/workflow/node/report", json!({"sessionId":"missing-old-child","body":"Old delivery","canComplete":true,"outputs":[{"id":"result","path":old}]}), 200).await;
    action(&s, "missing-root", "fail", json!({"nodeId":"call","reason":"Explicit replacement"}), 200).await;
    native(&s, "missing-new-child");
    let retry = action(&s, "missing-root", "retry", json!({"nodeId":"call","confirmed":true,"reason":"Approved independent work"}), 200).await;
    assert_ne!(retry["snapshot"]["nodes"]["call"]["execution"]["id"], first["snapshot"]["nodes"]["call"]["execution"]["id"]);
    finish_child(&s, "missing-new-child", "missing-new-inside").await;
    request(&s, "/api/workflow/node/report", json!({"sessionId":"missing-new-child","body":"Finished with explicit limitation","canComplete":true}), 200).await;
    action(&s, "missing-root", "complete", json!({"nodeId":"call","reason":"Accept limitation independently of missing materials"}), 200).await;
    let calls = s.herdr.calls();
    let rejected = s.dispatch("missing-root", &["consumer","missing-map"]).await;
    for (index, id) in ["consumer","missing-map"].iter().enumerate() {
        assert_eq!(rejected["results"][index]["status"], "failed");
        assert_eq!(rejected["snapshot"]["nodes"][id]["failure"]["stage"], "resolve");
        assert!(rejected["snapshot"]["nodes"][id]["execution"].get("childRunId").is_none());
    }
    let launches = |calls: Vec<Value>| calls.into_iter().filter(|call| call["kind"] != "tab_close").collect::<Vec<_>>();
    assert_eq!(launches(s.herdr.calls()), launches(calls));
    let current = read_run(&s, &root["id"]).await;
    assert_eq!(current["run"]["nodes"]["call"]["status"], "completed");
    assert_eq!(current["reports"][1]["outputs"], json!([]));
    assert_eq!(fs::read_to_string(old).unwrap(), "DO-NOT-INHERIT-631");
    wait_for_close(&s).await;
}

async fn wait_for_close(s: &Scenario) {
    assert!(common::wait_until(Duration::from_secs(10), || common::read_json_lines(&s.herdr.path().join("closed.jsonl")).len() == 1).await, "owned close must finish at controlled host");
}

// P24/P26/P35: real CLI notification rejection occurs after durable save.
// Reads/finish must not retry it or complete the parent. The newest false
// report revokes readiness; only first legal parent complete closes its exact
// child manager Tab, retaining internal and unrelated Tabs.
#[tokio::test]
async fn nested_saved_notification_failure_latest_report_and_exact_close_are_independent() {
    let s = Scenario::new().await;
    setup(&s).await;
    let root = s.start("notify-root").await;
    native(&s, "notify-child");
    let launched = s.dispatch("notify-root", &["call"]).await;
    let execution = launched["snapshot"]["nodes"]["call"]["execution"].clone();
    finish_child(&s, "notify-child", "notify-inside").await;
    let child_before = read_run(&s, &execution["childRunId"]).await;
    let tabs_before = s.herdr.tabs();
    assert_eq!(tabs_before.len(), 3);
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
    s.herdr.set_behavior(json!({"fail_prompt":true}));
    let offset = s.herdr.calls().len();
    let saved = request(&s, "/api/workflow/node/report", json!({"sessionId":"notify-child","body":"Saved despite real host rejection","canComplete":true}), 200).await;
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["notification"]["status"], "failed");
    assert!(saved["notification"]["reason"].as_str().is_some_and(|reason| !reason.is_empty()));
    assert_eq!(saved["report"]["executionId"], execution["id"]);
    let read = read_run(&s, &root["id"]).await;
    assert_eq!(read["reports"], json!([saved["report"]]));
    assert_eq!(read["run"]["nodes"]["call"]["status"], "running");
    assert_eq!(read["run"]["nodes"]["consumer"]["status"], "pending");
    assert_eq!(read_run(&s, &execution["childRunId"]).await, child_before);
    action(&s, "notify-root", "status", json!({}), 200).await;
    action(&s, "notify-child", "finish", json!({}), 200).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls()[offset..].len(), 1, "failure/read/finish must not replay or relaunch");
    assert_eq!(s.herdr.calls()[offset]["kind"], "agent_prompt");
    assert_eq!(s.herdr.tabs(), tabs_before);
    s.herdr.set_behavior(json!({}));
    request(&s, "/api/workflow/node/report", json!({"sessionId":"notify-child","body":"Acceptance withdrawn","canComplete":false}), 200).await;
    let denied = action(&s, "notify-root", "complete", json!({"nodeId":"call","reason":"Must consult newest report"}), 409).await;
    assert_eq!(denied["code"], "report-not-completable");
    assert_eq!(s.herdr.tabs(), tabs_before);
    let newest = request(&s, "/api/workflow/node/report", json!({"sessionId":"notify-child","body":"Ready again","canComplete":true}), 200).await;
    let accepted = action(&s, "notify-root", "complete", json!({"nodeId":"call","reason":"Explicit business acceptance"}), 200).await;
    assert_eq!(accepted["snapshot"]["nodes"]["call"]["status"], "completed");
    wait_for_close(&s).await;
    let closed = common::read_json_lines(&s.herdr.path().join("closed.jsonl"));
    assert_eq!(closed, vec![json!({"target":execution["chat"]["tabId"]})]);
    for chat in [&root["run"]["manager"], &child_before["run"]["nodes"]["inside"]["execution"]["chat"]] {
        assert_ne!(closed[0]["target"], chat["tabId"], "no manager/internal close command");
    }
    let close_calls = s.herdr.calls();
    let close_calls: Vec<_> = close_calls.iter().filter(|call| call["kind"] == "tab_close").collect();
    assert_eq!(close_calls.len(), 1);
    assert_eq!(close_calls[0]["argv"], json!(["tab","close",execution["chat"]["tabId"]]));
    let repeated = action(&s, "notify-root", "complete", json!({"nodeId":"call","reason":"Idempotent repeat"}), 200).await;
    assert_eq!(repeated["changed"], false);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "tab_close").count(), 1);
    let final_read = read_run(&s, &root["id"]).await;
    assert_eq!(final_read["reports"].as_array().unwrap().len(), 3);
    assert_eq!(final_read["reports"][2], newest["report"]);
    assert_eq!(read_run(&s, &execution["childRunId"]).await, child_before);
}
