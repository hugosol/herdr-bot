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
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"inside":{"title":"Internal work","bot_id":"worker","input":"INTERNAL","zone":"node","blocked_by":[],"output_rule":[]}}}).to_string()).unwrap();
    request(s, "/api/scheduler/bots/save", json!({"id":"nested","content":json!({"name":"Nested","when":"","description":"","input_rule":"","output_rule":"","map_file":"inner.json"}).to_string(),"revision":null}), 200).await;
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

// P28/P29, S1 HTTP seam: independently finishing real internal work must not
// approve the parent. A plausible violation accepts parent approval before
// child finish, or copies the child's approved fact into its parent execution.
// Both frozen parent modes must reject early approval and still require their
// own approval, ready report and explicit parent-manager completion.
#[tokio::test]
async fn parent_approval_requires_finished_child_and_independent_frozen_mode() {
    for auto in [true, false] {
        let s = Scenario::new().await;
        setup(&s).await;
        let path = s.config.scheduler.map_directory.join("work.json");
        let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        map["nodes"]["call"]["auto_approved"] = json!(auto);
        map["nodes"]["call"]["output_rule"] = json!([]);
        map["nodes"]["consumer"]["input_rule"] = json!([]);
        fs::write(&path, map.to_string()).unwrap();
        let root = s.start("approval-root").await;
        native(&s, "approval-child");
        let launched = s.dispatch("approval-root", &["call"]).await;
        let execution = launched["snapshot"]["nodes"]["call"]["execution"].clone();
        let source = if auto { "auto" } else { "manual" };
        let approval = json!({"sessionId":"approval-child","executionId":execution["id"],"source":source});
        let context = request(&s, "/api/workflow/node/context", json!({"sessionId":"approval-child"}), 200).await;
        assert_eq!(context, json!({"runId":root["id"],"nodeId":"call","executionId":execution["id"],"autoApproved":auto,"approved":false,"approvalSource":null}));
        let calls = s.herdr.calls();
        let rejected = request(&s, "/api/workflow/node/approve", approval.clone(), 400).await;
        assert_eq!(rejected["code"], "child-run-not-finished");
        assert_eq!(s.herdr.calls(), calls);
        native(&s, "approval-inside");
        s.dispatch("approval-child", &["inside"]).await;
        approve_execution(&s, "approval-inside").await;
        submit_report(&s, "approval-inside", "Internal work delivered", true).await;
        action(&s, "approval-child", "complete", json!({"nodeId":"inside","reason":"Accept internal work"}), 200).await;
        assert_eq!(request(&s, "/api/workflow/node/approve", approval.clone(), 400).await["code"], "child-run-not-finished");
        action(&s, "approval-child", "finish", json!({}), 200).await;
        let child_before = read_run(&s, &execution["childRunId"]).await;
        assert_eq!(child_before["run"]["nodes"]["inside"]["execution"]["approved"], true);
        assert_eq!(read_run(&s, &root["id"]).await["run"]["nodes"]["call"]["execution"]["approved"], false);
        request(&s, "/api/workflow/node/report", json!({"sessionId":"approval-child","body":"Overall delivery","canComplete":true}), 200).await;
        assert_eq!(action(&s, "approval-root", "complete", json!({"nodeId":"call","reason":"Internal approval cannot replace overall approval"}), 409).await["code"], "node-not-approved");
        // Editing the live definition must not select a new parent mode.
        map["nodes"]["call"]["auto_approved"] = json!(!auto);
        fs::write(&path, map.to_string()).unwrap();
        let mut wrong = approval.clone();
        wrong["source"] = json!(if auto { "manual" } else { "auto" });
        assert_eq!(request(&s, "/api/workflow/node/approve", wrong, 400).await["code"], "approval-source-mismatch");
        let calls = s.herdr.calls();
        let approved = request(&s, "/api/workflow/node/approve", approval, 200).await;
        assert_eq!(approved["runId"], root["id"]);
        assert_eq!(approved["executionId"], execution["id"]);
        assert_eq!(approved["approvalSource"], source);
        assert_eq!(s.herdr.calls(), calls);
        assert_eq!(read_run(&s, &execution["childRunId"]).await, child_before);
        let parent = read_run(&s, &root["id"]).await;
        assert_eq!(parent["run"]["nodes"]["call"]["status"], "running");
        assert_eq!(parent["run"]["nodes"]["consumer"]["status"], "pending");
        let completed = action(&s, "approval-root", "complete", json!({"nodeId":"call","reason":"Accept overall delivery"}), 200).await;
        assert_eq!(completed["snapshot"]["nodes"]["call"]["status"], "completed");
        println!("P28/P29 S1 mode={source} parent={} execution={} child={} internalApproved=true parentInitiallyApproved=false overallApproved=true parentCompleted=true", root["id"], execution["id"], execution["childRunId"]);
        wait_for_close(&s).await;
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
    assert_eq!(rejected["code"], "node-not-approved");
    native(&s, "finish-inside");
    s.dispatch("finish-child", &["inside"]).await;
    // Chief ticket 01: the internal node still notifies its own manager,
    // never the outer manager, and does not complete the internal work itself.
    let internal = submit_report(&s, "finish-inside", "Internal done", true).await;
    assert_eq!(internal["runId"], child);
    assert_eq!(internal["to"]["sessionId"], "finish-child");
    let child_detail = read_run(&s, &child).await;
    assert_eq!(child_detail["reports"], json!([internal]));
    assert_eq!(child_detail["run"]["nodes"]["inside"]["status"], "running");
    let calls = s.herdr.calls();
    let notification = calls.iter().rev().find(|call| call["kind"] == "agent_prompt").unwrap();
    assert_eq!(notification["target"], child_detail["run"]["manager"]["agentName"]);
    assert!(notification["text"].as_str().unwrap().contains("Internal done"));
    assert_eq!(read_run(&s, &root["id"]).await["reports"], json!([progress["report"]]));
    approve_execution(&s, "finish-inside").await;
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
    approve_execution(s, inside).await;
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
    request(&s, "/api/scheduler/bots/save", json!({"id":"sink","content":json!({"name":"Sink","when":"","description":"","input_rule":"","output_rule":"","map_file":"sink.json"}).to_string(),"revision":null}), 200).await;
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
    approve_execution(&s, "output-child").await;
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

// A previous child delivered a required file; its replacement finishes
// without outputs. Overall approval does not inherit old deliveries: parent
// complete must reject the missing current output, leaving consumers blocked.
// Child finish itself still never requires parent output IDs.
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
    approve_execution(&s, "missing-new-child").await;
    let denied = action(&s, "missing-root", "complete", json!({"nodeId":"call","reason":"Old delivery cannot fill current requirements"}), 409).await;
    assert_eq!(denied["code"], "output-missing");
    let calls = s.herdr.calls();
    let rejected = s.dispatch("missing-root", &["consumer","missing-map"]).await;
    for id in ["consumer","missing-map"] {
        assert_eq!(rejected["snapshot"]["nodes"][id]["status"], "pending");
        assert!(rejected["snapshot"]["nodes"][id]["execution"].is_null());
    }
    let launches = |calls: Vec<Value>| calls.into_iter().filter(|call| call["kind"] != "tab_close").collect::<Vec<_>>();
    assert_eq!(launches(s.herdr.calls()), launches(calls));
    let current = read_run(&s, &root["id"]).await;
    assert_eq!(current["run"]["nodes"]["call"]["status"], "running");
    assert_eq!(current["reports"][1]["outputs"], json!([]));
    assert_eq!(fs::read_to_string(old).unwrap(), "DO-NOT-INHERIT-631");
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
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
    approve_execution(&s, "notify-child").await;
    let output = s.data.path().join("notify-delivery.txt");
    fs::write(&output, "OVERALL-DELIVERY-837").unwrap();
    let child_before = read_run(&s, &execution["childRunId"]).await;
    let tabs_before = s.herdr.tabs();
    assert_eq!(tabs_before.len(), 3);
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
    s.herdr.set_behavior(json!({"fail_prompt":true}));
    let offset = s.herdr.calls().len();
    let saved = request(&s, "/api/workflow/node/report", json!({"sessionId":"notify-child","body":"Saved despite real host rejection","canComplete":true,"outputs":[{"id":"result","path":output},{"id":"missing","path":output}]}), 200).await;
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

#[tokio::test]
async fn nested_end_node_handoff_finishes_child_but_still_requires_parent_report_and_acceptance() {
    let s = Scenario::new().await;
    setup(&s).await;
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner end","description":"","nodes":{"inside":{"title":"End conversation","bot_id":"worker","input":"USER HANDOFF","zone":"node","blocked_by":[],"end_node":true,"auto_close":true}}}).to_string()).unwrap();
    s.start("end-root").await;
    native(&s, "end-child");
    let dispatched = s.dispatch("end-root", &["call"]).await;
    let child_id = dispatched["snapshot"]["nodes"]["call"]["execution"]["childRunId"].clone();
    native(&s, "end-inside");
    let dispatched = s.dispatch("end-child", &["inside"]).await;
    assert_eq!(dispatched["canFinish"], true);
    request(&s, "/api/workflow/node/report", json!({"sessionId":"end-child","body":"Before explicit finish","canComplete":true}), 400).await;
    let finished = action(&s, "end-child", "finish", json!({}), 200).await;
    assert_eq!(finished["snapshot"]["finished"]["reason"], "handed-to-user");
    assert_eq!(finished["snapshot"]["nodes"]["inside"]["status"], "running");
    assert_eq!(action(&s, "end-root", "complete", json!({"nodeId":"call","reason":"No overall approval yet"}), 409).await["code"], "node-not-approved");
    approve_execution(&s, "end-child").await;
    assert_eq!(action(&s, "end-root", "complete", json!({"nodeId":"call","reason":"No report yet"}), 409).await["code"], "report-not-completable");
    let output = s.data.path().join("end-delivery.txt");
    fs::write(&output, "END-HANDOFF-482").unwrap();
    let report = request(&s, "/api/workflow/node/report", json!({"sessionId":"end-child","body":"Child handed to user","canComplete":true,"outputs":[{"id":"result","path":output},{"id":"missing","path":output}]}), 200).await;
    assert_eq!(report["report"]["nodeId"], "call");
    let accepted = action(&s, "end-root", "complete", json!({"nodeId":"call","reason":"Accepted child handoff"}), 200).await;
    assert_eq!(accepted["snapshot"]["nodes"]["call"]["status"], "completed");
    assert_eq!(read_run(&s, &child_id).await["run"], finished["snapshot"]);
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|call| call["kind"] == "tab_close")).await);
    let calls = s.herdr.calls();
    let closes: Vec<_> = calls.iter().filter(|call| call["kind"] == "tab_close").collect();
    assert_eq!(closes.len(), 1, "only the parent Map node auto-closes; never its end Chat");
    assert_eq!(closes[0]["argv"], json!(["tab", "close", dispatched["snapshot"]["manager"]["tabId"]]));
}
