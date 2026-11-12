use super::*;

async fn request(s: &Scenario, path: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), path, &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn creator(s: &Scenario) {
    request(s, "/api/scheduler/bots/save", json!({"id":"rust-create","revision":null,"content":json!({"name":"Rust creator","when":"","plan_guide":"","node_guide":"","rust_script":"create_blank_bot"}).to_string()}), 200).await;
}
async fn target_task(s: &Scenario) -> Value {
    let tasks = request(s, "/api/tasks", json!({"name":"Only read invoices","taskType":"normal","agent":"pi","chatName":"Invoice {chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"只读，不修改。"}), 200).await;
    tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "Only read invoices").unwrap().clone()
}
fn business_request(target: Value) -> Value {
    json!({"schema_version":1,"kind":"request","target":target,"bot":{"id":"invoice-review","name":"Invoice reviewer","group":"isolated"},"requirements":"只读审阅","constraints":["不执行目标"],"non_goals":["修复文件"],"resource_limits":[],"authorization":{"create_blank":true,"write_materials":true,"save_guides":false},"confirmation":{"evidence":"User message 7 authorizes blank creation","scope":"create_blank"}})
}
async fn map_for_request(s: &Scenario, input: &str) {
    // Public file seam owns Map publication; public GET validates its real shape.
    let map = json!({"title":"Rust creation","description":"Isolated creation acceptance","nodes":{
        "create":{"title":"Create blank","bot_id":"rust-create","input":input,"input_rule":[{"id":"request","type":"file","zone_id":"manager","path":"request.json"}],"output_rule":[{"id":"baseline"},{"id":"handoff"}],"zone":"node","blocked_by":[]},
        "consume":{"title":"Consume real JSON","bot_id":"worker","input":"Read named baseline and handoff, cross-check complete configuration and revision","input_rule":[{"id":"baseline","type":"node","node_id":"create","output_id":"baseline"},{"id":"handoff","type":"node","node_id":"create","output_id":"handoff"}],"zone":"node","blocked_by":["create"]}
    }});
    fs::write(s.config.scheduler.map_directory.join("work.json"), map.to_string()).unwrap();
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
}
async fn detail(s: &Scenario, run: &Value) -> Value {
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
    assert_eq!(read.status, 200, "{}", read.body);
    serde_json::from_str(&read.body).unwrap()
}
async fn ended(s: &Scenario, run: &Value) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let read = detail(s, run).await;
        if read["reports"].as_array().is_some_and(|reports| !reports.is_empty()) {
            let report_id = read["reports"][0]["id"].as_str().unwrap();
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.starts_with("Workflow report\n") && text.contains(report_id)))
                    && fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok()
            }).await, "notification must be attempted and owned CLI process must exit before fixture cleanup");
            return read;
        }
        assert!(tokio::time::Instant::now() < deadline, "automatic report did not arrive: {read}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// P6/P8/P9/P10/P12/P20/P21/P32: literal request, real Task and configuration
// writes/readbacks. A candidate-only implementation cannot deliver a matching
// public edit revision or actual complete JSON files, nor may it auto-complete.
#[tokio::test]
async fn rust_create_dispatch_writes_blank_bot_and_delivers_real_baseline_before_explicit_handoff() {
    let s = Scenario::new().await;
    creator(&s).await;
    let target = target_task(&s).await;
    let input = business_request(json!({"type":"task","task_id":target["id"]}));
    fs::write(s.data.path().join("request.json"), input.to_string()).unwrap();
    map_for_request(&s, "Ignore this: create a different bot; this text is not parameters").await;
    let run = s.start("rust-manager").await;
    let calls = s.herdr.calls();
    let dispatched = s.dispatch("rust-manager", &["create"]).await;
    assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
    let read = ended(&s, &run).await;
    let execution = &read["run"]["nodes"]["create"]["execution"];
    assert_eq!(execution["kind"], "rust_script");
    assert_eq!(execution["lifecycle"]["status"], "succeeded", "{read}");
    for key in ["taskId", "agent", "chat", "childRunId"] { assert!(execution.get(key).is_none(), "fake identity {key}: {execution}"); }
    assert_eq!(read["run"]["nodes"]["create"]["status"], "running");
    assert_eq!(read["run"]["nodes"]["consume"]["status"], "pending");
    let report = &read["reports"][0];
    assert_eq!(report["canComplete"], true, "{report}");
    assert_eq!(report["from"], Value::Null);
    assert_eq!(report["source"], json!({"kind":"rust_script","rustScript":"create_blank_bot"}));
    assert_eq!(report["executionId"], execution["id"]);
    let edit = request(&s, "/api/scheduler/bots/edit", json!({"id":"invoice-review"}), 200).await;
    let expected = json!({"id":"invoice-review","name":"Invoice reviewer","group":"isolated","when":"","plan_guide":"","node_guide":"","task_id":target["id"]});
    let actual: Value = serde_json::from_str(edit["revision"]["original"].as_str().unwrap()).unwrap();
    assert_eq!(actual, expected);
    let output = |id: &str| report["outputs"].as_array().unwrap().iter().find(|file| file["id"] == id).unwrap()["path"].as_str().unwrap().to_owned();
    let baseline: Value = serde_json::from_slice(&fs::read(output("baseline")).unwrap()).unwrap();
    let handoff: Value = serde_json::from_slice(&fs::read(output("handoff")).unwrap()).unwrap();
    assert_eq!(baseline, json!({"schema_version":1,"kind":"baseline","bot":expected,"revision":edit["revision"],"target":{"type":"task","task_id":target["id"],"configuration":target},"request":input}));
    assert_eq!(handoff["stage"], "created");
    assert!(handoff.get("bot").is_none() && handoff.get("revision").is_none());
    assert_eq!(handoff["checks"], json!({"bot_read_back":true,"target_unchanged":true}));
    let status = request(&s, "/api/workflow/manager/status", json!({"sessionId":"rust-manager"}), 200).await;
    assert!(!status["ready"].as_array().unwrap().contains(&json!("consume")));
    assert!(s.herdr.calls()[calls.len()..].iter().all(|call| call["kind"] == "agent_prompt"), "only manager report notification is allowed");
    let collected = request(&s, "/api/workflow/manager/collect", json!({"sessionId":"rust-manager"}), 200).await;
    assert_eq!(collected["pendingReports"]["create"], report["id"]);
    let complete = request(&s, "/api/workflow/manager/complete", json!({"sessionId":"rust-manager","nodeId":"create","reason":"Verified actual baseline and blank bot"}), 200).await;
    assert!(complete["ready"].as_array().unwrap().contains(&json!("consume")));
    assert_eq!(complete["snapshot"]["nodes"]["consume"]["status"], "pending");
    // This is a controlled external consumer, not a real manager/model. Its
    // process reads the actual named files handed to the dispatched Task.
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"rust-downstream-consumer"},"native_cwd":s.data.path().join("node-zone"),"consume_named_json":true}));
    let consumer = s.dispatch("rust-manager", &["consume"]).await;
    assert_eq!(consumer["results"][0]["status"], "running", "{consumer}");
    let consumed = fs::read_to_string(s.herdr.path().join("consumed.jsonl")).expect("downstream process must actually consume named files");
    let consumed: Value = serde_json::from_str(consumed.lines().last().unwrap()).unwrap();
    assert_eq!(consumed["files"]["baseline"], baseline);
    assert_eq!(consumed["files"]["handoff"], handoff);
    assert_eq!(consumed["files"]["baseline"]["revision"], edit["revision"]);
    assert_eq!(consumer["snapshot"]["nodes"]["consume"]["execution"]["inputs"], report["outputs"]);
    println!("Rust creation actual run={} execution={} report={report} baseline={baseline} consumed={consumed}", run["id"], execution["id"]);
}


// P11, approved S1 HTTP/files and the existing external-host diagnostic seam.
// Oracle: real successful creation survives a rejected notification unchanged.
// Plausible violations: notify-before-save, rollback, lost outputs, or a hidden
// retry. Hold only Herdr's prompt, read the real fact/files before releasing it,
// then sample identities, effects and host calls for a full one-second window.
#[tokio::test]
async fn rust_create_saved_report_survives_immediate_and_delayed_notification_rejection() {
    const MODE: &str = "HERDR_SCRIPT_NOTIFICATION_ISOLATION_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        use std::os::windows::fs::OpenOptionsExt;
        let s = Scenario::new().await;
        creator(&s).await;
        let target = target_task(&s).await;
        let input = business_request(json!({"type":"task","task_id":target["id"]}));
        fs::write(s.data.path().join("request.json"), input.to_string()).unwrap();
        map_for_request(&s, "").await;
        let run = s.start("notification-isolation-manager").await;
        let before_calls = s.herdr.calls();
        let release = s.data.path().join("release-script-notification");
        let delayed = mode == "delayed";
        let mut behavior = json!({"fail_prompt":true});
        if delayed {
            behavior["prompt_gate"] = json!({"wait_for_file":release,"timeout_ms":10000});
        }
        s.herdr.set_behavior(behavior);
        let dispatched = s.dispatch("notification-isolation-manager", &["create"]).await;
        let entered = common::wait_until(Duration::from_secs(5), || {
            s.herdr.calls()[before_calls.len()..].iter().any(|call| call["kind"] == "agent_prompt")
        }).await;
        // Collect observations without panicking while the owned CLI is held;
        // always release and confirm its exit before checking the observations.
        let during = tokio::time::timeout(Duration::from_secs(2), detail(&s, &run)).await;
        let during_files = during.as_ref().ok().and_then(|read| read["reports"][0]["outputs"].as_array()).map(|outputs| {
            outputs.iter().map(|output| fs::read(output["path"].as_str().unwrap())).collect::<Vec<_>>()
        });
        let host_held = fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_err();
        fs::write(&release, b"release owned notification").unwrap();
        let after = ended(&s, &run).await;
        assert!(entered, "notification must enter the external host within 5 seconds");
        assert_eq!(dispatched["results"][0]["status"], "running");
        let during = during.expect("saved report must be readable while delivery is pending");
        if delayed {
            assert!(host_held, "the real external notification must still be held during readback");
        }
        assert_eq!(during["reports"].as_array().unwrap().len(), 1);
        assert_eq!(after["reports"], during["reports"]);
        let report = after["reports"][0].clone();
        assert!(!report["id"].as_str().unwrap().is_empty());
        assert!(!report["body"].as_str().unwrap().is_empty());
        assert_eq!(report["canComplete"], true);
        assert_eq!(report["to"], run["run"]["manager"]);
        let execution = after["run"]["nodes"]["create"]["execution"].clone();
        assert_eq!(execution["id"], dispatched["snapshot"]["nodes"]["create"]["execution"]["id"]);
        assert_eq!(execution["lifecycle"]["status"], "succeeded");
        assert_eq!(report["executionId"], execution["id"]);
        assert_eq!(report["source"], json!({"kind":"rust_script","rustScript":"create_blank_bot"}));
        let outputs = report["outputs"].as_array().unwrap();
        assert_eq!(outputs.iter().map(|output| output["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["baseline", "handoff"]);
        let bytes = during_files.expect("both output bindings must be readable before notification ends").into_iter().map(Result::unwrap).collect::<Vec<_>>();
        for (output, original) in outputs.iter().zip(&bytes) {
            assert_eq!(fs::read(output["path"].as_str().unwrap()).unwrap(), *original);
        }
        let edit = request(&s, "/api/scheduler/bots/edit", json!({"id":"invoice-review"}), 200).await;
        let expected = json!({"id":"invoice-review","name":"Invoice reviewer","group":"isolated","when":"","plan_guide":"","node_guide":"","task_id":target["id"]});
        assert_eq!(serde_json::from_str::<Value>(edit["revision"]["original"].as_str().unwrap()).unwrap(), expected);
        let baseline: Value = serde_json::from_slice(&bytes[0]).unwrap();
        assert_eq!(baseline, json!({"schema_version":1,"kind":"baseline","bot":expected,"revision":edit["revision"],"target":{"type":"task","task_id":target["id"],"configuration":target},"request":input}));
        let handoff: Value = serde_json::from_slice(&bytes[1]).unwrap();
        assert_eq!(handoff["stage"], "created");
        assert_eq!(handoff["checks"], json!({"bot_read_back":true,"target_unchanged":true}));
        let calls = s.herdr.calls();
        assert_eq!(calls.len(), before_calls.len() + 1, "one notification, no script Chat or repeated business dispatch");
        let notification = calls.last().unwrap();
        assert_eq!(notification["kind"], "agent_prompt");
        assert_eq!(notification["target"], report["to"]["agentName"]);
        assert_eq!(notification["text"], format!("Workflow report\nRun: {}\nnode: create\nReport: {}\n\n{}", run["id"].as_str().unwrap(), report["id"].as_str().unwrap(), report["body"].as_str().unwrap()));
        let observed_at = tokio::time::Instant::now();
        let mut samples = 0;
        while observed_at.elapsed() < Duration::from_secs(1) {
            let read = detail(&s, &run).await;
            assert_eq!(read["reports"], after["reports"], "no report retry or rollback");
            assert_eq!(read["run"], after["run"], "no new execution or automatic lifecycle change");
            assert_eq!(s.herdr.calls(), calls, "no notification retry or external business dispatch");
            assert_eq!(request(&s, "/api/scheduler/bots/edit", json!({"id":"invoice-review"}), 200).await, edit, "no business rewrite");
            for (output, original) in outputs.iter().zip(&bytes) {
                assert_eq!(fs::read(output["path"].as_str().unwrap()).unwrap(), *original);
            }
            samples += 1;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let observed_ms = observed_at.elapsed().as_millis();
        assert!(observed_ms >= 1000 && samples > 1);
        assert_eq!(after["run"]["nodes"]["create"]["status"], "running");
        assert!(after["run"]["nodes"]["create"]["previousExecutions"].as_array().is_none_or(|facts| facts.is_empty()));
        let dispatches = common::read_json_lines(&s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("history.jsonl"));
        assert_eq!(dispatches.iter().filter(|event| event["action"] == "dispatch").count(), 1);
        assert!(fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok(), "owned notification CLI has exited before cleanup");
        println!("SCRIPT-NOTIFICATION-EVIDENCE {}", json!({"mode":mode,"runId":run["id"],"reportId":report["id"],"executionId":execution["id"],"targetChat":report["to"],"saved":true,"report":report,"baseline":baseline,"handoff":handoff,"dispatches":1,"observationMs":observed_ms,"samples":samples,"externalCalls":calls,"notificationProcessExited":true}));
        return;
    }
    // Reuse the existing report diagnostic child-process fixture. Capturing the
    // real stderr sink proves failure feedback independently of the saved fact.
    for mode in ["immediate", "delayed"] {
        let cwd = TempDir::new("script notification diagnostic child");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "rust_create::rust_create_saved_report_survives_immediate_and_delayed_notification_rejection", "--nocapture"])
            .env(MODE, mode).current_dir(cwd.path()).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{mode}\n{stdout}\n{stderr}");
        let evidence: Value = serde_json::from_str(stdout.lines().find_map(|line| line.strip_prefix("SCRIPT-NOTIFICATION-EVIDENCE ")).unwrap()).unwrap();
        let diagnostics = stderr.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).filter(|entry| entry["event"] == "workflow-report-delivery").collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 1, "one result for the single saved report");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["runId"], evidence["runId"]);
        assert_eq!(diagnostic["reportId"], evidence["reportId"]);
        assert_eq!(diagnostic["targetChat"], evidence["targetChat"]);
        assert_eq!(diagnostic["result"], "failed", "business success must not imply notification submission or receipt");
        assert_eq!(diagnostic["reason"], "herdr-command-failed");
        assert_eq!(evidence["saved"], true);
        assert_eq!(evidence["notificationProcessExited"], true);
        println!("P11 childExited=true diagnostic={diagnostic} evidence={evidence}");
    }
}

// P3/P6/P12: a real Windows file-open gate holds the business target read,
// not notification or a mocked Rust collaborator. The HTTP dispatch response,
// current running lifecycle and complete rejection are observed BEFORE release.
// Mutating live Map rules/input then distinguishes freezing from late lookup.
#[tokio::test]
async fn rust_create_returns_while_business_target_read_is_held_and_freezes_named_rules() {
    use futures_util::FutureExt;
    let s = Scenario::new().await;
    creator(&s).await;
    let target = target_task(&s).await;
    let path = s.config.tasks.storage_file.clone();
    let original = fs::read(&path).unwrap();
    let mut input = business_request(json!({"type":"task","task_id":target["id"]}));
    input.as_object_mut().unwrap().remove("bot");
    fs::write(s.data.path().join("request.json"), input.to_string()).unwrap();
    map_for_request(&s, "First node.input must not become parameters").await;
    let run = s.start("held-rust-manager").await;
    let calls = s.herdr.calls();
    let gate = super::history_integrity::file_gate::Gate::new(&path);
    let addr = s.server.addr();
    let dispatch = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", r#"{"sessionId":"held-rust-manager","nodes":["create"]}"#).await });
    let observation = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), || gate.reached()).await, "business must actually reach held target file");
        let response = tokio::time::timeout(Duration::from_secs(2), dispatch).await.expect("dispatch must return during business wait").unwrap();
        assert_eq!(response.status, 200, "{}", response.body);
        let dispatched: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(dispatched["results"][0]["status"], "running");
        let during = detail(&s, &run).await;
        assert_eq!(during["run"]["nodes"]["create"]["execution"]["lifecycle"]["status"], "running");
        assert_eq!(during["reports"], json!([]));
        assert_eq!(s.herdr.calls(), calls, "no node Chat or host work during Rust business");
        request(&s, "/api/workflow/manager/complete", json!({"sessionId":"held-rust-manager","nodeId":"create","reason":"No report yet"}), 409).await;
        let status = request(&s, "/api/workflow/manager/status", json!({"sessionId":"held-rust-manager"}), 200).await;
        assert!(!status["ready"].as_array().unwrap().contains(&json!("consume")));
        let root = s.config.scheduler.map_directory.join("work.json");
        let mut map: Value = serde_json::from_slice(&fs::read(&root).unwrap()).unwrap();
        map["nodes"]["create"]["input"] = json!("CONTRAST: create a script target and overwrite existing Bot instead");
        map["nodes"]["create"]["input_rule"] = json!([]);
        map["nodes"]["create"]["output_rule"] = json!([]);
        fs::write(root, map.to_string()).unwrap();
    }).catch_unwind().await;
    drop(gate);
    let read = ended(&s, &run).await;
    if let Err(error) = observation { std::panic::resume_unwind(error); }
    assert_eq!(read["reports"][0]["canComplete"], true, "{read}");
    let execution = &read["run"]["nodes"]["create"]["execution"];
    assert_eq!(execution["inputRule"][0]["id"], "request");
    assert_eq!(execution["outputRule"], json!([{"id":"baseline"},{"id":"handoff"}]));
    let output = &read["reports"][0]["outputs"][0]["path"];
    let baseline: Value = serde_json::from_slice(&fs::read(output.as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(baseline["target"]["configuration"], target);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(baseline["bot"], json!({"id":format!("bot-{}", target["id"].as_str().unwrap()),"name":"Only read invoices","when":"","plan_guide":"","node_guide":"","task_id":target["id"]}));
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&list.body).unwrap()["runs"].as_array().unwrap().len(), 1, "target must not execute");
}

// P6/P20/P21: a real Map target, including its full node rules/configuration,
// is preserved and delivered; default identity comes from the exact filename.
// The request reaches the capability through an explicitly renamed Run input.
#[tokio::test]
async fn rust_create_map_target_preserves_complete_definition_and_named_run_source() {
    let s = Scenario::new().await;
    creator(&s).await;
    let target = json!({"title":"Exact target","group":"isolated","description":"Do not execute","input_requirements":"Read exact materials","input_files":[{"id":"source","required":true,"description":"Original source"}],"nodes":{"inspect":{"title":"Inspect","bot_id":"worker","input":"Only inspect","input_rule":[{"id":"source","type":"run","input_id":"source"}],"output_rule":[{"id":"result"}],"report_policy":"manual","auto_close":false,"zone":"node","blocked_by":[]}}});
    let target_path = s.config.scheduler.map_directory.join("exact.target.json");
    fs::write(&target_path, target.to_string()).unwrap();
    let original = fs::read(&target_path).unwrap();
    let mut input = business_request(json!({"type":"map","map_file":"exact.target.json"}));
    input.as_object_mut().unwrap().remove("bot");
    let path = s.data.path().join("incoming.json");
    fs::write(&path, input.to_string()).unwrap();
    map_for_request(&s, "").await;
    let root = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&root).unwrap()).unwrap();
    map["input_files"] = json!([{"id":"submission","required":true}]);
    map["nodes"]["create"]["input_rule"] = json!([{"id":"request","type":"run","input_id":"submission"}]);
    fs::write(root, map.to_string()).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"map-rust-manager"},"native_cwd":s.data.path()}));
    let run = request(&s, "/api/scheduler/maps/work.json/runs", json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Create definition only","fileInputs":[{"id":"submission","path":path}]}), 201).await;
    s.dispatch("map-rust-manager", &["create"]).await;
    let read = ended(&s, &run).await;
    assert_eq!(read["reports"][0]["canComplete"], true, "{read}");
    let execution = &read["run"]["nodes"]["create"]["execution"];
    assert_eq!(execution["inputs"], json!([{"id":"request","path":path}]));
    let baseline: Value = serde_json::from_slice(&fs::read(read["reports"][0]["outputs"][0]["path"].as_str().unwrap()).unwrap()).unwrap();
    let full_target = get_over_tcp(s.server.addr(), "/api/scheduler/maps/exact.target.json").await;
    assert_eq!(full_target.status, 200, "{}", full_target.body);
    assert_eq!(baseline["target"]["configuration"], serde_json::from_str::<Value>(&full_target.body).unwrap());
    assert_eq!(baseline["bot"], json!({"id":"bot-map-exact.target","name":"Exact target","when":"","plan_guide":"","node_guide":"","map_file":"exact.target.json"}));
    let edit = request(&s, "/api/scheduler/bots/edit", json!({"id":"bot-map-exact.target"}), 200).await;
    assert_eq!(baseline["revision"], edit["revision"]);
    assert_eq!(fs::read(target_path).unwrap(), original);
    assert_eq!(baseline["request"], input);
    let actual_runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&actual_runs.body).unwrap()["runs"].as_array().unwrap().len(), 1);
}

// P7/P20/P33: malformed/missing/ambiguous/conflicting requests must leave the
// independently captured complete definitions and actual files untouched. A
// false success or a hidden write is caught by both report and full readback.
#[tokio::test]
async fn rust_create_rejects_bad_requests_without_business_writes_or_success_outputs() {
    for case in ["invalid-json", "duplicate-key", "nested-duplicate", "missing-field", "unknown-field", "null-bot", "null-id", "unknown-version", "unauthorized", "script-target", "mixed-target", "missing-target", "unknown-task", "manager-task", "missing-map", "path-map", "identity-conflict", "binding-conflict", "ambiguous-catalog"] {
        let s = Scenario::new().await;
        creator(&s).await;
        let target = target_task(&s).await;
        let mut input = business_request(json!({"type":"task","task_id":target["id"]}));
        match case {
            "missing-field" => { input.as_object_mut().unwrap().remove("requirements"); },
            "unknown-field" => input["hidden_parameters"] = json!({"run_target":true}),
            "null-bot" => input["bot"] = Value::Null,
            "null-id" => input["bot"]["id"] = Value::Null,
            "unknown-version" => input["schema_version"] = json!(2),
            "unauthorized" => input["authorization"]["create_blank"] = json!(false),
            "script-target" => input["target"] = json!({"type":"rust_script","rust_script":"create_blank_bot"}),
            "mixed-target" => input["target"]["map_file"] = json!("work.json"),
            "missing-target" => { input["target"].as_object_mut().unwrap().remove("task_id"); },
            "unknown-task" => input["target"]["task_id"] = json!("nonexistent-task"),
            "manager-task" => input["target"]["task_id"] = json!(s.manager_task),
            "missing-map" => input["target"] = json!({"type":"map","map_file":"missing.json"}),
            "path-map" => input["target"] = json!({"type":"map","map_file":"../work.json"}),
            "identity-conflict" => input["bot"]["id"] = json!("worker"),
            "binding-conflict" => {
                let edit = request(&s, "/api/scheduler/bots/edit", json!({"id":"worker"}), 200).await;
                input["target"]["task_id"] = edit["content"]["task_id"].clone();
            },
            _ => {},
        }
        let raw = match case {
            "invalid-json" => "not a JSON document".into(),
            "duplicate-key" => format!("{{\"schema_version\":1,{}", &input.to_string()[1..]),
            "nested-duplicate" => input.to_string().replace("\"scope\":\"create_blank\"", "\"scope\":\"create_blank\",\"scope\":\"create_blank\""),
            _ => input.to_string(),
        };
        fs::write(s.data.path().join("request.json"), &raw).unwrap();
        map_for_request(&s, "MUST NOT compensate for missing or invalid structured data").await;
        let run = s.start(&format!("reject-{case}")).await;
        if case == "ambiguous-catalog" { fs::write(s.config.scheduler.bot_directory.join("duplicate.json"), fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap()).unwrap(); }
        let before_tasks = get_over_tcp(s.server.addr(), "/api/tasks").await.body;
        let before_bots = get_over_tcp(s.server.addr(), "/api/scheduler/bots").await.body;
        let before_map = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
        let before_files: std::collections::BTreeMap<_, _> = fs::read_dir(&s.config.scheduler.bot_directory).unwrap().map(|entry| { let path = entry.unwrap().path(); (path.file_name().unwrap().to_owned(), fs::read(&path).unwrap()) }).collect();
        s.dispatch(&format!("reject-{case}"), &["create"]).await;
        let read = ended(&s, &run).await;
        assert_eq!(read["reports"][0]["canComplete"], false, "case={case}: {read}");
        assert_eq!(read["reports"][0]["outputs"], json!([]), "case={case}");
        assert_eq!(read["run"]["nodes"]["create"]["status"], "running");
        assert_eq!(read["run"]["nodes"]["create"]["execution"]["lifecycle"]["status"], "failed", "case={case}");
        let expected_code = match case {
            "duplicate-key" | "nested-duplicate" | "ambiguous-catalog" => "ambiguous-request",
            "missing-field" | "missing-target" => "missing-field",
            "unauthorized" => "authorization-required",
            "identity-conflict" => "identity-conflict",
            "unknown-task" | "manager-task" | "missing-map" | "path-map" | "binding-conflict" => "target-conflict",
            _ => "invalid-format",
        };
        let failure = &read["run"]["nodes"]["create"]["execution"]["lifecycle"]["failure"];
        assert_eq!(failure["code"], expected_code, "case={case}: {failure}");
        println!("Rust refusal case={case} run={} report={} failure={failure}", run["id"], read["reports"][0]["id"]);
        assert_eq!(get_over_tcp(s.server.addr(), "/api/tasks").await.body, before_tasks, "case={case}");
        assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/bots").await.body, before_bots, "case={case}");
        assert_eq!(fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap(), before_map);
        let after_files: std::collections::BTreeMap<_, _> = fs::read_dir(&s.config.scheduler.bot_directory).unwrap().map(|entry| { let path = entry.unwrap().path(); (path.file_name().unwrap().to_owned(), fs::read(&path).unwrap()) }).collect();
        assert_eq!(after_files, before_files, "case={case}");
        assert_eq!(fs::read_to_string(s.data.path().join("request.json")).unwrap(), raw);
        assert!(!s.data.path().join(".manager-zone").exists(), "rejected request cannot publish success materials: {case}");
        request(&s, "/api/workflow/manager/complete", json!({"sessionId":format!("reject-{case}"),"nodeId":"create","reason":"False must never complete"}), 409).await;
    }
}

// P8: real filesystem failures, never a mocked business writer. The target
// open is held to place each fault after dispatch admission. A two-gate case
// corrupts target readback only AFTER initial full target read and before save.
#[tokio::test]
async fn rust_create_write_readback_and_material_failures_report_actual_effects() {
    for case in ["write", "readback", "materials", "collision", "handoff-collision"] {
        let s = Scenario::new().await;
        creator(&s).await;
        let target = target_task(&s).await;
        fs::write(s.data.path().join("request.json"), business_request(json!({"type":"task","task_id":target["id"]})).to_string()).unwrap();
        map_for_request(&s, "No input inference").await;
        let run = s.start(&format!("fault-{case}")).await;
        let original_tasks = fs::read(&s.config.tasks.storage_file).unwrap();
        let task_gate = super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file);
        let dispatched = s.dispatch(&format!("fault-{case}"), &["create"]).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        assert!(common::wait_until(Duration::from_secs(5), || task_gate.reached()).await);
        let saved_bots = s.data.path().join("held-bots");
        let bot_gate = if case == "readback" { Some(super::history_integrity::file_gate::Gate::new(&s.config.scheduler.bot_directory.join("worker.json"))) } else { None };
        if case == "write" {
            fs::rename(&s.config.scheduler.bot_directory, &saved_bots).unwrap();
            fs::write(&s.config.scheduler.bot_directory, b"real file blocks legal Bot directory").unwrap();
        }
        if case == "materials" { fs::write(s.data.path().join(".manager-zone"), b"real file blocks authorized materials root").unwrap(); }
        let collision = s.data.path().join(".manager-zone").join(".workflow").join(run["id"].as_str().unwrap()).join(dispatched["snapshot"]["nodes"]["create"]["execution"]["id"].as_str().unwrap()).join(if case == "handoff-collision" { "handoff.json" } else { "baseline.json" });
        if matches!(case, "collision" | "handoff-collision") { fs::create_dir_all(collision.parent().unwrap()).unwrap(); fs::write(&collision, b"UNOWNED MATERIAL MUST NOT BE OVERWRITTEN").unwrap(); }
        drop(task_gate);
        if let Some(gate) = bot_gate {
            let reached = common::wait_until(Duration::from_secs(5), || gate.reached()).await;
            if reached { fs::write(&s.config.tasks.storage_file, b"target became unreadable JSON after initial target read").unwrap(); }
            drop(gate);
            assert!(reached, "save must reach the real Bot directory read before target fault");
        }
        let read = ended(&s, &run).await;
        if matches!(case, "collision" | "handoff-collision") { assert_eq!(fs::read(&collision).unwrap(), b"UNOWNED MATERIAL MUST NOT BE OVERWRITTEN"); }
        if case == "write" { fs::remove_file(&s.config.scheduler.bot_directory).unwrap(); fs::rename(&saved_bots, &s.config.scheduler.bot_directory).unwrap(); }
        fs::write(&s.config.tasks.storage_file, &original_tasks).unwrap();
        let lifecycle = &read["run"]["nodes"]["create"]["execution"]["lifecycle"];
        assert_eq!(lifecycle["status"], "failed", "{case}: {read}");
        assert_eq!(read["reports"][0]["canComplete"], false);
        if case == "handoff-collision" {
            assert_eq!(read["reports"][0]["outputs"].as_array().unwrap().len(), 1);
            assert_eq!(read["reports"][0]["outputs"][0]["id"], "baseline");
            let baseline: Value = serde_json::from_slice(&fs::read(read["reports"][0]["outputs"][0]["path"].as_str().unwrap()).unwrap()).unwrap();
            assert_eq!(baseline["bot"]["id"], "invoice-review");
            assert_eq!(baseline["target"]["configuration"], target);
            assert_eq!(lifecycle["failure"]["effects"]["materials"][0]["status"], "succeeded");
            assert_eq!(lifecycle["failure"]["effects"]["materials"][1]["status"], "failed");
        } else { assert_eq!(read["reports"][0]["outputs"], json!([])); }
        assert_eq!(lifecycle["failure"]["code"], match case { "write" => "write-failed", "readback" => "read-back-failed", _ => "material-write-failed" });
        assert_eq!(lifecycle["failure"]["effects"]["write"], if case == "write" { "failed" } else { "succeeded" });
        assert_eq!(lifecycle["failure"]["effects"]["readBack"], match case { "write" => "not_attempted", "readback" => "failed", _ => "succeeded" });
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"invoice-review"}"#).await;
        if case == "write" { assert_ne!(response.status, 200); } else {
            assert_eq!(response.status, 200, "{}", response.body);
            let edit: Value = serde_json::from_str(&response.body).unwrap();
            assert_eq!(edit["content"], json!({"name":"Invoice reviewer","group":"isolated","when":"","plan_guide":"","node_guide":"","task_id":target["id"]}));
            assert!(!edit["revision"]["original"].as_str().unwrap().is_empty());
        }
        request(&s, "/api/workflow/manager/complete", json!({"sessionId":format!("fault-{case}"),"nodeId":"create","reason":"Partial effects do not grant completion"}), 409).await;
    }
}

// P33: a real active Run protects its actual Map, not a synthetic flag. The
// creation capability must use its protection check and preserve every byte.
#[tokio::test]
async fn rust_create_refuses_binding_a_definition_used_by_a_real_running_run() {
    let s = Scenario::new().await;
    creator(&s).await;
    fs::write(s.data.path().join("request.json"), business_request(json!({"type":"map","map_file":"work.json"})).to_string()).unwrap();
    map_for_request(&s, "No runtime definition modification authorized").await;
    let before_map = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    let before_bots = get_over_tcp(s.server.addr(), "/api/scheduler/bots").await.body;
    let run = s.start("protected-manager").await;
    s.dispatch("protected-manager", &["create"]).await;
    let read = ended(&s, &run).await;
    assert_eq!(read["reports"][0]["canComplete"], false);
    assert_eq!(read["run"]["nodes"]["create"]["execution"]["lifecycle"]["failure"]["code"], "definition-in-use", "{read}");
    assert_eq!(fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap(), before_map);
    assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/bots").await.body, before_bots);
    assert_eq!(read["reports"][0]["outputs"], json!([]));
}

// P9: persistence is the public file seam, not an alternate output authority.
// A report whose sender/type/owned execution disagree must never grant complete.
#[tokio::test]
async fn rust_create_report_provenance_cannot_be_forged_into_completion() {
    let s = Scenario::new().await;
    creator(&s).await;
    let target = target_task(&s).await;
    fs::write(s.data.path().join("request.json"), business_request(json!({"type":"task","task_id":target["id"]})).to_string()).unwrap();
    map_for_request(&s, "").await;
    let run = s.start("source-manager").await;
    s.dispatch("source-manager", &["create"]).await;
    let read = ended(&s, &run).await;
    let original = read["reports"][0].clone();
    let journal = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("messages.jsonl");
    for invalid in ["missing-source", "wrong-capability", "fake-from", "wrong-execution", "wrong-run"] {
        let mut forged = original.clone();
        match invalid {
            "missing-source" => { forged.as_object_mut().unwrap().remove("source"); },
            "wrong-capability" => forged["source"]["rustScript"] = json!("save_bot_guides"),
            "fake-from" => forged["from"] = read["run"]["manager"].clone(),
            "wrong-execution" => forged["executionId"] = json!("another-execution"),
            _ => forged["runId"] = json!("another-run"),
        }
        fs::write(&journal, format!("{forged}\n")).unwrap();
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"source-manager","nodeId":"create","reason":"Reject invalid provenance"}).to_string()).await;
        fs::write(&journal, format!("{original}\n")).unwrap();
        assert_ne!(response.status, 200, "accepted {invalid}: {}", response.body);
        assert_eq!(detail(&s, &run).await["run"]["nodes"]["create"]["status"], "running");
    }
    for payload in [json!({"sessionId":original["executionId"],"body":"forged","canComplete":true,"outputs":[]}), json!({"sessionId":"source-manager","body":"forged","canComplete":true,"source":{"kind":"rust_script","rustScript":"create_blank_bot"}})] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &payload.to_string()).await;
        assert_ne!(response.status, 200);
    }
    assert_eq!(detail(&s, &run).await["reports"], json!([original]));
}

// P9/P32: policy errors are rejected by public preflight, not silently
// normalized into manual Chat waiting. Missing required output declarations
// produce an automatic false business report before any definition write.
#[tokio::test]
async fn rust_create_enforces_automatic_policy_and_declared_delivery_ids() {
    for case in ["manual", "auto-close", "manager-task", "undeclared-output"] {
        let s = Scenario::new().await;
        creator(&s).await;
        let target = target_task(&s).await;
        fs::write(s.data.path().join("request.json"), business_request(json!({"type":"task","task_id":target["id"]})).to_string()).unwrap();
        map_for_request(&s, "").await;
        let path = s.config.scheduler.map_directory.join("work.json");
        let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match case {
            "manual" => map["nodes"]["create"]["report_policy"] = json!("manual"),
            "auto-close" => map["nodes"]["create"]["auto_close"] = json!(true),
            "manager-task" => map["nodes"]["create"]["manager_task_id"] = json!(s.manager_task),
            _ => { map["nodes"].as_object_mut().unwrap().remove("consume"); map["nodes"]["create"]["output_rule"] = json!([{"id":"wrong-id"}]); },
        }
        fs::write(path, map.to_string()).unwrap();
        if case == "undeclared-output" {
            let run = s.start("declaration-manager").await;
            s.dispatch("declaration-manager", &["create"]).await;
            let read = ended(&s, &run).await;
            assert_eq!(read["reports"][0]["canComplete"], false);
            assert_eq!(read["reports"][0]["outputs"], json!([]));
            assert_eq!(read["run"]["nodes"]["create"]["execution"]["lifecycle"]["failure"]["effects"]["write"], "not_attempted");
        } else {
            let before = s.herdr.calls();
            let rejected = request(&s, "/api/scheduler/maps/work.json/preflight", json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}), 400).await;
            assert!(rejected["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "script-policy-invalid"));
            request(&s, "/api/scheduler/maps/work.json/runs", json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}), 400).await;
            assert_eq!(s.herdr.calls(), before);
        }
        let catalog = get_over_tcp(s.server.addr(), "/api/scheduler/bots").await;
        assert!(!serde_json::from_str::<Value>(&catalog.body).unwrap()["bots"].as_array().unwrap().iter().any(|bot| bot["id"] == "invoice-review"));
    }
}

// Approved seam: public S1 HTTP/file interface. Independent contract literal:
// rust_script=create_blank_bot is runnable, exclusive, and unknown IDs reject.
// A permissive parser or a rejected write replacing the original is observable
// by reading the complete public catalog after each attempted save.
#[tokio::test]
async fn rust_create_target_round_trips_and_rejected_edits_preserve_definition() {
    let s = Scenario::new().await;
    let content = json!({"name":"Rust blank creator","when":"","plan_guide":"","node_guide":"","rust_script":"create_blank_bot"});
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"rust-create","content":content.to_string(),"revision":null}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let saved: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(saved["rust_script"], "create_blank_bot");
    assert!(saved.get("task_id").is_none() && saved.get("map_file").is_none());
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"rust-create"}"#).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let edit: Value = serde_json::from_str(&response.body).unwrap();
    for invalid in [json!({"rust_script":"execute_code"}), json!({"rust_script":"create_blank_bot","task_id":"fake"}), json!({"rust_script":"create_blank_bot","map_file":"work.json"})] {
        let mut rejected = content.clone();
        rejected.as_object_mut().unwrap().extend(invalid.as_object().unwrap().clone());
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"rust-create","content":rejected.to_string(),"revision":edit["revision"]}).to_string()).await;
        assert_ne!(response.status, 200, "{}", response.body);
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"rust-create"}"#).await;
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), edit);
    }
}
