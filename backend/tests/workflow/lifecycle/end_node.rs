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
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut definition: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    definition["map_output_rule"] = json!({"files":[{"id":"release"}]});
    fs::write(&map_path, definition.to_string()).unwrap();
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
    // P21 (S1/S4): issuing the handoff is not a substitute for the required file.
    let rejected = action(&s, "end-manager", "finish", json!({}), 409).await;
    assert_eq!(rejected["code"], "output-missing");
    assert_eq!(rejected["snapshot"]["status"], "running");
    assert!(rejected["snapshot"]["finished"].is_null());
    let release = s.data.path().join("prepared-before-finish.txt");
    fs::write(&release, b"USER-HANDOFF-RELEASE").unwrap();
    let output = json!({"files":[{"id":"release","path":release}]});
    let finished = action(&s, "end-manager", "finish", json!({"mapOutput":output}), 200).await;
    assert_eq!(finished["snapshot"]["mapOutput"], output);
    assert_eq!(fs::read(release).unwrap(), b"USER-HANDOFF-RELEASE");
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["reason"], "handed-to-user");
    assert_eq!(finished["snapshot"]["nodes"]["end"]["status"], "running");
    assert!(finished["snapshot"]["nodes"]["end"]["completion"].is_null());
    assert_eq!(finished["snapshot"]["nodes"]["end"]["execution"], execution);
    let persisted: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(persisted, finished["snapshot"]);
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
}

async fn ready_to_finish(s: &Scenario, rule: Option<Value>) -> Value {
    end_map(s, false);
    if let Some(rule) = rule {
        let path = s.config.scheduler.map_directory.join("work.json");
        let mut definition: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        definition["map_output_rule"] = rule;
        fs::write(path, definition.to_string()).unwrap();
    }
    let started = s.start("public-finish-manager").await;
    native(s, "omp", "public-finish-end");
    assert_eq!(s.dispatch("public-finish-manager", &["end"]).await["canFinish"], true);
    started
}

async fn finish_detail(s: &Scenario, id: &str) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str::<Value>(&response.body).unwrap()["run"].clone()
}

// P15/P16/P17 (S1/S4): with the graph already finishable, break exactly
// one file condition at a time. Then submit valid files and a different valid
// set. A terminal rewrite or any validation bypass changes public facts/bytes.
#[tokio::test]
async fn public_finish_rejects_each_bad_binding_and_preserves_first_delivery() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    let started = ready_to_finish(&s, Some(json!({"files":[{"id":"report"},{"id":"receipt"}]}))).await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let paths = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| root.join(name));
    let before = paths.each_ref().map(|path| fs::read(path).unwrap());
    let current = finish_detail(&s, id).await;
    let calls = s.herdr.calls();
    let report = s.data.path().join("written-report.txt");
    let receipt = s.data.path().join("written-receipt.txt");
    let alternate = s.data.path().join("alternate.txt");
    let denied = s.data.path().join("denied.txt");
    for (path, bytes) in [(&report,"REPORT-031"),(&receipt,"RECEIPT-041"),(&alternate,"ALTERNATE-051"),(&denied,"EXISTS-BUT-LOCKED")] {
        fs::write(path, bytes).unwrap();
    }
    let held = fs::OpenOptions::new().read(true).share_mode(0).open(&denied).unwrap();
    let correct = json!({"id":"report","path":report});
    let good_receipt = json!({"id":"receipt","path":receipt});
    for (code, files) in [
        ("invalid-output-id", json!([correct,good_receipt,correct])),
        ("undeclared-output", json!([correct,good_receipt,{"id":"foreign","path":receipt}])),
        ("output-missing", json!([correct])),
        ("invalid-output-path", json!([correct,{"id":"receipt","path":"relative.txt"}])),
        ("output-unreadable", json!([correct,{"id":"receipt","path":s.data.path()}])),
        ("output-unreadable", json!([correct,{"id":"receipt","path":s.data.path().join("not-written.txt")}])),
        ("output-unreadable", json!([correct,{"id":"receipt","path":denied}])),
    ] {
        let rejected = action(&s, "public-finish-manager", "finish", json!({"mapOutput":{"files":files}}), 409).await;
        assert_eq!(rejected["code"], code);
        assert_eq!(rejected["changed"], false);
        assert_eq!(rejected["snapshot"], current);
        assert!(rejected["snapshot"]["finished"].is_null());
        assert_eq!(finish_detail(&s, id).await, current);
        assert_eq!(paths.each_ref().map(|path| fs::read(path).unwrap()), before);
    }
    drop(held);
    for invalid in [Value::Null, json!([[correct,good_receipt]]), json!({"files":[["report",report]]}), json!({"files":[],"unexpected":true})] {
        action(&s, "public-finish-manager", "finish", json!({"mapOutput":invalid}), 400).await;
        assert_eq!(paths.each_ref().map(|path| fs::read(path).unwrap()), before);
    }
    let output = json!({"files":[correct,good_receipt]});
    action(&s, "public-finish-end", "finish", json!({"mapOutput":output}), 403).await;
    let finished = action(&s, "public-finish-manager", "finish", json!({"mapOutput":output}), 200).await;
    assert_eq!(finished["snapshot"]["mapOutput"], output);
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["actor"]["sessionId"], "public-finish-manager");
    let saved = paths.each_ref().map(|path| fs::read(path).unwrap());
    let history = common::read_json_lines(&paths[1]);
    assert_eq!(history.iter().filter(|event| event["action"] == "finish").count(), 1);
    assert_eq!(history.last().unwrap()["id"], finished["snapshot"]["finished"]["id"]);
    let repeated = action(&s, "public-finish-manager", "finish", json!({"mapOutput":{"files":[{"id":"report","path":alternate},good_receipt]}}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(finish_detail(&s, id).await, finished["snapshot"]);
    assert_eq!(repeated["snapshot"], finished["snapshot"]);
    assert_eq!(paths.each_ref().map(|path| fs::read(path).unwrap()), saved);
    assert_eq!(fs::read(report).unwrap(), b"REPORT-031");
    assert_eq!(fs::read(receipt).unwrap(), b"RECEIPT-041");
    assert_eq!(fs::read(alternate).unwrap(), b"ALTERNATE-051");
    assert_eq!(s.herdr.calls(), calls);
}

#[tokio::test]
async fn public_finish_without_file_rule_accepts_no_output_parameter() {
    let s = Scenario::new().await;
    let started = ready_to_finish(&s, None).await;
    let id = started["id"].as_str().unwrap();
    let calls = s.herdr.calls();
    let finished = action(&s, "public-finish-manager", "finish", json!({}), 200).await;
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["mapOutput"], json!({"files":[]}));
    assert_eq!(finish_detail(&s, id).await, finished["snapshot"]);
    let saved = fs::read(s.config.scheduler.run_directory.join(id).join("run.json")).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&saved).unwrap(), finished["snapshot"]);
    assert_eq!(s.herdr.calls(), calls);
}

// P29 (S1/S4): deny a real replace, then deny only the independent history
// append. Only the latter may durably publish the finish/output fact.
#[tokio::test]
async fn public_finish_preserves_truth_across_current_save_and_history_failures() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    let started = ready_to_finish(&s, Some(json!({"files":[{"id":"release"}]}))).await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let state = root.join("run.json");
    let history = root.join("history.jsonl");
    let saved_state = fs::read(&state).unwrap();
    let saved_history = fs::read(&history).unwrap();
    let release = s.data.path().join("pre-existing-effect.txt");
    fs::write(&release, b"PRE-EXISTING-PUBLIC-EFFECT").unwrap();
    let output = json!({"files":[{"id":"release","path":release}]});
    let calls = s.herdr.calls();
    let held = fs::OpenOptions::new().read(true).share_mode(3).open(&state).unwrap();
    let failed = action(&s, "public-finish-manager", "finish", json!({"mapOutput":output}), 500).await;
    assert_eq!(failed["code"], "finish-failed");
    assert_eq!(finish_detail(&s, id).await["status"], "running");
    assert_eq!(fs::read(&state).unwrap(), saved_state);
    assert_eq!(fs::read(&history).unwrap(), saved_history);
    assert_eq!(fs::read(&release).unwrap(), b"PRE-EXISTING-PUBLIC-EFFECT");
    drop(held);
    let held = fs::OpenOptions::new().read(true).share_mode(1).open(&history).unwrap();
    let partial = action(&s, "public-finish-manager", "finish", json!({"mapOutput":output}), 200).await;
    assert_eq!(partial["code"], "history-append-failed");
    assert_eq!(partial["stateCommitted"], true);
    assert_eq!(partial["changed"], true);
    assert!(partial["historyError"].as_str().unwrap().contains("history.jsonl"));
    let detail = finish_detail(&s, id).await;
    assert_eq!(detail["status"], "completed");
    assert_eq!(detail["mapOutput"], output);
    assert_eq!(detail, partial["snapshot"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(state).unwrap()).unwrap(), detail);
    assert_eq!(fs::read(&history).unwrap(), saved_history);
    drop(held);
    let repeated = action(&s, "public-finish-manager", "finish", json!({}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], detail);
    assert_eq!(fs::read(history).unwrap(), saved_history);
    assert_eq!(fs::read(release).unwrap(), b"PRE-EXISTING-PUBLIC-EFFECT");
    assert_eq!(s.herdr.calls(), calls);
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
            "script" => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Script","when":"","description":"","input_rule":"","output_rule":"","script":"rust-create"}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); },
            "map" => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Map","when":"","description":"","input_rule":"","output_rule":"","map_file":"work.json"}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); definition["nodes"]["end"]["manager_task_id"] = json!(s.manager_task); },
            _ => { fs::write(s.config.scheduler.bot_directory.join("invalid-end.json"), json!({"id":"invalid-end","name":"Manager task","when":"","description":"","input_rule":"","output_rule":"","task_id":s.manager_task}).to_string()).unwrap(); definition["nodes"]["end"]["bot_id"] = json!("invalid-end"); },
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
