//! Public definition/workflow HTTP, real persistence and production host argv.
use super::*;

async fn action(s: &Scenario, session: &str, name: &str, extra: Value, expected: u16) -> Value {
    let mut body = extra;
    body["sessionId"] = json!(session);
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, expected, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
fn end_map(s: &Scenario, other: bool) {
    let mut nodes = json!({"end":{"title":"User conversation","bot_id":"worker","input":"END-BUSINESS-LITERAL","input_rule":[{"id":"brief","type":"file","zone_id":"node","path":"brief.txt"}],"zone":"node","blocked_by":[],"end_node":true,"auto_close":true,"auto_approved":false}});
    if other { nodes["work"] = json!({"title":"Work","bot_id":"worker","input":"PREPARE","zone":"node","blocked_by":[]}); }
    fs::write(s.data.path().join("node-zone/brief.txt"), "BUSINESS-MATERIAL").unwrap();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"End map","description":"","nodes":nodes}).to_string()).unwrap();
}
fn native(s: &Scenario, host: &str, session: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":format!("herdr:{host}"),"agent":host,"kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
}
fn assert_no_node_role(s: &Scenario, offset: usize, host: &str) {
    let calls = s.herdr.calls();
    let starts: Vec<_> = calls[offset..].iter().filter(|call| call["kind"] == "agent_start").collect();
    assert_eq!(starts.len(), 1);
    let args = starts[0]["args"].as_array().unwrap();
    assert!(args.contains(&json!("--no-extensions")));
    assert!(!args.contains(&json!(s.data.path().join(format!("{host}-node-chat.ts")))));
    assert!(!args.contains(&json!(s.data.path().join(format!("{host}-manager-chat.ts")))));
}

#[tokio::test]
async fn end_node_waits_for_every_node_then_explicit_finish_preserves_open_running_chat() {
    let s = Scenario::new().await;
    end_map(&s, true);
    let started = s.start("end-manager").await;
    let blocked = s.dispatch("end-manager", &["end"]).await;
    assert_eq!(blocked["results"][0]["status"], "rejected");
    assert_eq!(blocked["snapshot"]["nodes"]["end"]["status"], "pending");
    assert_eq!(blocked["ready"], json!(["work"]));
    action(&s, "end-manager", "finish", json!({}), 409).await;
    native(&s, "omp", "end-work");
    s.dispatch("end-manager", &["work"]).await;
    submit_report(&s, "end-work", "Prepared", true).await;
    let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", r#"{"sessionId":"end-work"}"#).await;
    let context: Value = serde_json::from_str(&context.body).unwrap();
    let approved = post_json_over_tcp(s.server.addr(), "/api/workflow/node/approve", &json!({"sessionId":"end-work","executionId":context["executionId"],"source":"auto"}).to_string()).await;
    assert_eq!(approved.status, 200, "{}", approved.body);
    let completed = action(&s, "end-manager", "complete", json!({"nodeId":"work","reason":"Accepted work"}), 200).await;
    assert_eq!(completed["ready"], json!(["end"]));
    assert_eq!(completed["canFinish"], false);
    action(&s, "end-manager", "finish", json!({}), 409).await;
    native(&s, "omp", "end-chat");
    let offset = s.herdr.calls().len();
    let dispatched = s.dispatch("end-manager", &["end"]).await;
    assert_eq!(dispatched["canFinish"], true);
    let execution = dispatched["snapshot"]["nodes"]["end"]["execution"].clone();
    assert_eq!(execution["endNode"], true);
    assert_eq!(execution["approved"], false);
    assert!(execution["approvalSource"].is_null());
    assert_eq!(execution["autoClose"], false);
    assert_eq!(execution["inputs"][0]["path"], json!(s.data.path().join("node-zone").join("brief.txt")));
    assert_no_node_role(&s, offset, "omp");
    let calls = s.herdr.calls();
    let prompt = calls[offset..].iter().find(|call| call["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap();
    for literal in ["NODE-FIXED", "END-BUSINESS-LITERAL", "MANAGER-USER-SECRET", "brief.txt"] { assert!(prompt.contains(literal), "{prompt}"); }
    for forbidden in ["运行说明", "当前状态", "执行历史", "报告记录", "[Report policy control]", "[Output rules]"] { assert!(!prompt.contains(forbidden), "{prompt}"); }
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"end-chat","body":"No workflow authority","canComplete":true}"#).await;
    assert_eq!(report.status, 403, "{}", report.body);
    let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", r#"{"sessionId":"end-chat"}"#).await;
    assert_eq!(context.status, 403, "{}", context.body);
    let approval = post_json_over_tcp(s.server.addr(), "/api/workflow/node/approve", &json!({"sessionId":"end-chat","executionId":execution["id"],"source":"manual"}).to_string()).await;
    assert_eq!(approval.status, 403, "{}", approval.body);
    action(&s, "end-manager", "complete", json!({"nodeId":"end","reason":"Not a workflow completion"}), 409).await;
    let finished = action(&s, "end-manager", "finish", json!({}), 200).await;
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["reason"], "handed-to-user");
    assert_eq!(finished["snapshot"]["nodes"]["end"]["status"], "running");
    assert!(finished["snapshot"]["nodes"]["end"]["completion"].is_null());
    assert_eq!(finished["snapshot"]["nodes"]["end"]["execution"], execution);
    let persisted: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(persisted, finished["snapshot"]);
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
}

#[tokio::test]
async fn end_node_start_and_persisted_resume_do_not_require_node_extension_for_either_host() {
    for host in ["omp", "pi"] {
        let s = Scenario::new().await;
        end_map(&s, false);
        let response = get_over_tcp(s.server.addr(), "/api/tasks").await;
        let tasks: Value = serde_json::from_str(&response.body).unwrap();
        let mut task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "node").unwrap().clone();
        let id = task.as_object_mut().unwrap().remove("id").unwrap();
        task["agent"] = json!(host);
        let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{}", id.as_str().unwrap()), Some(&task.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        fs::remove_file(s.data.path().join(format!("{host}-node-chat.ts"))).unwrap();
        s.start("end-manager").await;
        native(&s, host, "end-resume");
        let offset = s.herdr.calls().len();
        let dispatched = s.dispatch("end-manager", &["end"]).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        assert_no_node_role(&s, offset, host);
        action(&s, "end-manager", "finish", json!({}), 200).await;
        let restored = TestServer::start_app(herdr_bot::app(&s.config)).await;
        let offset = s.herdr.calls().len();
        let response = post_json_over_tcp(restored.addr(), "/api/chats/resume", r#"{"sessionId":"end-resume"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["outcome"], "restored");
        assert!(!value["degradations"].as_array().unwrap().iter().any(|item| item["reason"].as_str().unwrap().contains("node-chat")));
        assert_no_node_role(&s, offset, host);
        assert!(!s.herdr.calls()[offset..].iter().any(|call| call["kind"] == "agent_prompt" || call["kind"] == "agent_input"));
    }
}

#[tokio::test]
async fn end_node_failed_dispatch_never_enables_finish() {
    for flag in ["fail_agent_start", "fail_prompt", "invalid_prompt_response"] {
        let s = Scenario::new().await;
        end_map(&s, false);
        s.start("end-manager").await;
        let mut behavior = json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"failed-end-chat"},"native_cwd":s.data.path().join("node-zone")});
        behavior[flag] = json!(true);
        s.herdr.set_behavior(behavior);
        let failed = s.dispatch("end-manager", &["end"]).await;
        assert_eq!(failed["snapshot"]["nodes"]["end"]["status"], "failed", "{flag}: {failed}");
        assert_eq!(failed["snapshot"]["nodes"]["end"]["execution"]["endNode"], true);
        assert_eq!(failed["canFinish"], false);
        action(&s, "end-manager", "finish", json!({}), 409).await;
    }
}

#[tokio::test]
async fn end_node_definition_rejects_multiple_downstream_and_non_task_targets() {
    let s = Scenario::new().await;
    end_map(&s, true);
    let path = s.config.scheduler.map_directory.join("work.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for variant in ["multiple", "downstream", "script", "map", "manager-task"] {
        let mut definition = original.clone();
        match variant {
            "multiple" => definition["nodes"]["work"]["end_node"] = json!(true),
            "downstream" => definition["nodes"]["work"]["blocked_by"] = json!(["end"]),
            "script" => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Script","when":"","plan_guide":"","node_guide":"","script":"rust-create"}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); },
            "map" => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Map","when":"","plan_guide":"","node_guide":"","map_file":"work.json"}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); definition["nodes"]["end"]["manager_task_id"] = json!(s.manager_task); },
            _ => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Manager task","when":"","plan_guide":"","node_guide":"","task_id":s.manager_task}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); },
        }
        fs::write(&path, definition.to_string()).unwrap();
        let before = s.herdr.calls();
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"business"}).to_string()).await;
        assert_ne!(response.status, 201, "{variant}: {}", response.body);
        assert_eq!(s.herdr.calls(), before);
    }
}

#[tokio::test]
async fn end_node_public_edits_round_trip_and_reject_a_second_end_without_writing() {
    let s = Scenario::new().await;
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"A"}"#).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let mut draft: Value = serde_json::from_str(&response.body).unwrap();
    let mut content = draft["content"].clone();
    assert_eq!(content["end_node"], false);
    content["end_node"] = json!(true);
    // A is not a leaf: the existing C dependency must not be silently removed.
    draft["content"] = json!(content.to_string());
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &draft.to_string()).await;
    assert_ne!(response.status, 200, "{}", response.body);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"C"}"#).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let mut draft: Value = serde_json::from_str(&response.body).unwrap();
    let mut content = draft["content"].clone();
    content["end_node"] = json!(true);
    content["auto_approved"] = json!(false);
    content["auto_close"] = json!(true);
    draft["content"] = json!(content.to_string());
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &draft.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let saved: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(saved["nodes"]["C"]["end_node"], true);
    assert_eq!(saved["nodes"]["C"]["auto_close"], true);
    let path = s.config.scheduler.map_directory.join("work.json");
    let before = fs::read(&path).unwrap();
    let response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
    let mut draft: Value = serde_json::from_str(&response.body).unwrap();
    content["blocked_by"] = json!([]);
    draft["id"] = json!("second-end");
    draft["content"] = json!(content.to_string());
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &draft.to_string()).await;
    assert_ne!(response.status, 200, "{}", response.body);
    assert_eq!(fs::read(&path).unwrap(), before);
}
