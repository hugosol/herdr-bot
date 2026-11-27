use super::*;

// S2 gate leaves the real Run persistence and CLI startup path in place. A
// late/backfilled sender association would be missing while input is blocked;
// bypassing manager preparation or copying its cwd would alter node effects.
#[tokio::test]
async fn map_sender_is_saved_before_input_and_internal_node_keeps_workflow_gates() {
    let (f, _, request) = map_fixture().await;
    let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
    let release = f.root.path().join("release-map-prompt");
    let mut behavior: Value = serde_json::from_slice(&fs::read(f.herdr.path().join("behavior.json")).unwrap()).unwrap();
    behavior["prompt_gate"] = json!({"wait_for_file":release,"timeout_ms":10000});
    f.herdr.set_behavior(behavior);
    let addr = f.server.addr();
    let commit = json!({"sessionId":"chief-a","previewId":preview["previewId"]});
    let started = tokio::spawn(async move { post_json_over_tcp(addr, "/api/chief/delegations", &commit.to_string()).await });
    let observed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if f.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"] == preview["facts"]["prompt"]) { break; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        get_over_tcp(addr, "/api/chief/delegations?sessionId=flow-manager").await
    }).await;
    let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl")).len();
    fs::write(&release, "release owned Map prompt").unwrap();
    let started = body(started.await.unwrap(), 201);
    assert_eq!(preview["facts"]["name"], "[M]Review flow");
    assert_eq!(started["run"]["name"], "[M]Review flow");
    let calls = f.herdr.calls();
    assert_eq!(calls.iter().filter(|call| call["kind"] == "tab_create").last().unwrap()["label"], "[M]Review flow");
    let history = body(get_over_tcp(addr, "/api/sessions/history").await, 200);
    let manager_history = history["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == "flow-manager").unwrap();
    assert_eq!(manager_history["sessionName"], "[M]Review flow");
    let observed = body(observed.expect("Map prompt gate must be reached"), 200);
    assert_eq!(observed["delegations"][0]["runId"], started["id"]);
    assert_eq!(observed["delegations"][0]["sessionId"], "flow-manager");
    assert_eq!(submitted, 1, "only initial chief input preceded release");
    let manager = json!({"sessionId":"flow-manager","nodes":["review"]});
    let blocked = post(&f.server, "/api/workflow/manager/dispatch", manager.clone()).await;
    assert_eq!(blocked["snapshot"]["nodes"]["review"]["status"], "pending");
    assert_eq!(blocked["results"][0]["status"], "rejected");
    post(&f.server, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"flow-manager","fileInputs":started["run"]["mapInput"]["files"],"confirmed":true,"reason":"Explicit independent review of brief binding"})).await;
    f.identity("internal-reviewer");
    fs::write(f.root.path().join("internal-reviewer.jsonl"), json!({"type":"session","version":3,"id":"internal-reviewer","timestamp":"2026-11-12T00:00:00.000Z","cwd":f.root.path().join("materials")}).to_string()+"\n").unwrap();
    let mut behavior: Value = serde_json::from_slice(&fs::read(f.herdr.path().join("behavior.json")).unwrap()).unwrap();
    behavior["native_cwd"] = json!(f.root.path().join("materials"));
    f.herdr.set_behavior(behavior);
    let dispatched = post(&f.server, "/api/workflow/manager/dispatch", manager).await;
    let execution = &dispatched["snapshot"]["nodes"]["review"]["execution"];
    assert_eq!(execution["chat"]["sessionId"], "internal-reviewer", "{dispatched}");
    assert!(common::same_path(std::path::Path::new(execution["zone"].as_str().unwrap()), &f.root.path().join("materials")));
    assert_eq!(execution["inputs"][0]["id"], "source");
    let tabs = f.herdr.tabs();
    assert!(common::same_path(std::path::Path::new(tabs.last().unwrap()["cwd"].as_str().unwrap()), &f.root.path().join("materials")));
    let context = post(&f.server, "/api/workflow/node/context", json!({"sessionId":"internal-reviewer"})).await;
    assert_eq!(context["runId"], started["id"]);
    assert_eq!(context["nodeId"], "review");
    assert_eq!(body(get_over_tcp(addr, "/api/chief/delegations?sessionId=internal-reviewer").await, 200)["delegations"], json!([]));
    body(post_json_over_tcp(addr, "/api/workflow/manager/complete", &json!({"sessionId":"flow-manager","nodeId":"review","reason":"No handoff report exists"}).to_string()).await, 409);
}

// These Map paths reuse the production Run protocol. A failure must keep the
// real Run/Chat effects and rejected/unknown input outcome instead of launching
// a stand-alone manager or reporting business success.
#[tokio::test]
async fn map_startup_failure_retains_run_provenance_and_real_submission_outcome() {
    for fault in ["fail_agent_start", "fail_prompt", "invalid_prompt_response", "run-initialize"] {
        let (f, _, request) = map_fixture().await;
        let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
        if fault == "run-initialize" {
            fs::write(f.root.path().join("runs"), "block Run directory creation").unwrap();
        } else {
            let mut behavior: Value = serde_json::from_slice(&fs::read(f.herdr.path().join("behavior.json")).unwrap()).unwrap();
            behavior[fault] = json!(true);
            f.herdr.set_behavior(behavior);
        }
        let before = f.herdr.calls().len();
        let submitted = || common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
        let count = submitted().len();
        let started = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 500);
        let after = submitted();
        assert_eq!(after.len() - count, usize::from(fault == "invalid_prompt_response"));
        if fault == "invalid_prompt_response" {
            assert_eq!(after.last().unwrap()["text"], preview["facts"]["prompt"]);
        }
        let record = body(get_over_tcp(f.server.addr(), &format!("/api/chief/delegations/{}", preview["previewId"].as_str().unwrap())).await, 200);
        assert_eq!(record["runId"], preview["previewId"]);
        if fault != "run-initialize" { assert_eq!(record["runId"], started["id"]); }
        assert_eq!(record["creation"]["stage"], "failed");
        assert_eq!(record["creation"]["inputSubmitted"], false);
        if fault == "run-initialize" {
            assert_eq!(f.herdr.calls().len(), before, "no manager without Run records");
            assert!(record.get("sessionId").is_none());
        } else if fault == "fail_agent_start" {
            assert_eq!(started["startupError"]["stage"], "start");
            assert!(record.get("sessionId").is_none());
        } else {
            assert_eq!(record["sessionId"], "flow-manager");
            assert_eq!(started["run"]["manager"]["sessionId"], "flow-manager");
            assert_eq!(started["startupError"]["effects"]["inputOutcome"], if fault == "fail_prompt" { "rejected" } else { "unknown" });
        }
    }
}

#[tokio::test]
async fn map_accepts_missing_required_materials_but_rejects_invalid_supplied_materials() {
    let (f, _, request) = map_fixture().await;
    let before = f.herdr.calls().len();
    for missing in ["manager", "wrong-manager-role", "undeclared-file", "unreadable-file"] {
        let mut input = request.clone();
        match missing {
            "manager" => { input.as_object_mut().unwrap().remove("managerTaskId"); }
            "unreadable-file" => input["materials"][0]["path"] = json!("missing-input.txt"),
            "wrong-manager-role" => input["managerTaskId"] = f.worker["id"].clone(),
            _ => input["materials"][0]["id"] = json!("not-declared"),
        }
        let response = post_json_over_tcp(f.server.addr(), "/api/chief/delegations/preview", &input.to_string()).await;
        assert!(response.status >= 400, "{missing}: {}", response.body);
        assert_eq!(f.herdr.calls().len(), before);
    }
    let mut incomplete = request.clone();
    incomplete["materials"] = json!([]);
    let preview = post(&f.server, "/api/chief/delegations/preview", incomplete).await;
    assert_eq!(f.herdr.calls().len(), before);
    let started = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 201);
    assert_eq!(started["run"]["mapInput"], json!({"runInput":request["input"],"files":[]}));
    assert_eq!(started["run"]["inputsReady"], false);
    assert_eq!(started["run"]["manager"]["sessionId"], "flow-manager");
    assert_eq!(started["run"]["nodes"]["review"]["status"], "pending");
}

pub(super) async fn map_fixture() -> (Fixture, Value, Value) {
    let f = Fixture::new().await;
    let tasks = post(&f.server, "/api/tasks", json!({"name":"flow manager","taskType":"manager","chatName":"manager {chat.id}","agent":"pi","model":"default","prompt":"MANAGER-FIXED-731"})).await;
    let manager = tasks["tasks"].as_array().unwrap().iter().find(|t| t["name"] == "flow manager").unwrap().clone();
    post(&f.server, "/api/zones", json!({"id":"review-zone","path":f.root.path().join("materials")})).await;
    fs::create_dir(f.root.path().join("maps")).unwrap();
    fs::write(f.root.path().join("maps/flow.json"), json!({"title":"Review flow","description":"Independent flow","map_input_rule":{"files":[{"id":"brief","required":true}]},"nodes":{"review":{"title":"Review","bot_id":"brief-bot","input":"Review the named brief without publishing","zone":"review-zone","input_rule":[{"type":"run","id":"source","run_file_ref":"brief"}],"blocked_by":[]}}}).to_string()).unwrap();
    fs::write(f.root.path().join("bots/flow.json"), json!({"id":"flow-bot","name":"Review flow","when":"Review a brief","description":"Supply brief","input_rule":"Review","output_rule":"","map_file":"flow.json"}).to_string()).unwrap();
    let mut request = f.request();
    request["botId"] = json!("flow-bot");
    request["managerTaskId"] = manager["id"].clone();
    f.herdr.set_workspaces(json!([{"workspace_id":"flow-workspace","label":"work"}]));
    f.identity("flow-manager");
    (f, manager, request)
}

// Approved seam S2: preview/commit, public Run and sender provenance, external
// CLI. Ticket 09 independently requires a real root Run, explicit manager/input
// and named material, with manager and node in different zones. A manager-only
// Chat or copying the manager zone into nodes must fail these observations.
#[tokio::test]
async fn map_bot_starts_real_root_run_and_preserves_explicit_manager_and_node_zones() {
    let (f, manager, request) = map_fixture().await;
    let before = f.herdr.calls().len();
    let preview = post(&f.server, "/api/chief/delegations/preview", request.clone()).await;
    assert_eq!(f.herdr.calls().len(), before);
    assert_eq!(preview["facts"]["request"], request);
    assert_eq!(preview["facts"]["task"]["id"], manager["id"]);
    assert_eq!(preview["facts"]["map"]["file"], "flow.json");
    let accepted = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 201);
    let run = &accepted["run"];
    assert_eq!(run["map"], "flow.json");
    assert_eq!(run["managerTaskId"], manager["id"]);
    assert_eq!(run["managerZoneId"], "work-zone");
    assert_eq!(run["mapInput"]["runInput"], request["input"]);
    assert!(run.get("parent").is_none(), "chief is not a parent node: {run}");
    assert_eq!(run["manager"]["sessionId"], "flow-manager");
    assert_eq!(run["inputsReady"], false, "launch cannot approve workflow inputs");
    assert_eq!(run["nodes"]["review"]["status"], "pending");
    assert!(common::same_path(std::path::Path::new(run["zoneBindings"]["review-zone"].as_str().unwrap()), &f.root.path().join("materials")));
    assert_eq!(run["mapInput"]["files"][0]["id"], "brief");
    assert!(common::same_path(std::path::Path::new(run["mapInput"]["files"][0]["path"].as_str().unwrap()), &f.root.path().join("materials/brief.txt")));
    let record = body(get_over_tcp(f.server.addr(), &format!("/api/chief/delegations/{}", preview["previewId"].as_str().unwrap())).await, 200);
    assert_eq!(record["runId"], run["id"]);
    assert_eq!(record["creation"]["stage"], "succeeded");
    assert_eq!(record["creation"]["inputSubmitted"], true);
    assert_eq!(record["sessionId"], "flow-manager");
    assert_eq!(record["facts"], preview["facts"]);
    let sender = body(get_over_tcp(f.server.addr(), "/api/chief/delegations?sessionId=flow-manager").await, 200);
    assert_eq!(sender["delegations"], json!([record]));
    let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
    assert_eq!(submitted.last().unwrap()["text"], preview["facts"]["prompt"], "only the complete approved payload may be submitted");
    let text = submitted.last().unwrap()["text"].as_str().unwrap();
    assert!(text.contains("你负责本次 Run 的输入准备、节点编排、通用交接判断与流程收束"));
    assert!(text.contains("本次工作由 chief 委派。按提供的业务要求完成工作，并向当前 chief 交接实际结果"));
    assert!(text.contains("brief: ") && text.contains("brief.txt"));
    assert!(!text.contains("Independent flow") && !text.contains("Supply brief"));
    let detail = body(get_over_tcp(f.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await, 200);
    assert_eq!(detail["run"], *run);
}

// P22/P24/P25/P29/P34 (S1/S4): the caller asks for just one of two
// public files. A stale grant, a report-supplied address, or a finish that
// accepted only the caller's subset would disagree with the public facts.
#[tokio::test]
async fn map_reports_only_the_requested_finished_run_outputs() {
    let (f, _, mut request) = map_fixture().await;
    let addr = f.server.addr();
    let map_path = f.root.path().join("maps/flow.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["review"]["end_node"] = json!(true);
    map["map_output_rule"] = json!({"files":[{"id":"disposition"},{"id":"review-record"}]});
    fs::write(&map_path, map.to_string()).unwrap();
    let before = f.herdr.calls();
    request["outputRule"] = json!([{"id":"internal-only"}]);
    let invalid = body(post_json_over_tcp(addr, "/api/chief/delegations/preview", &request.to_string()).await, 409);
    assert_eq!(invalid["code"], "undeclared-output");
    assert_eq!(f.herdr.calls(), before);

    request["outputRule"] = json!([{"id":"disposition"}]);
    let previous = post(&f.server, "/api/chief/delegations/preview", request.clone()).await;
    assert_eq!(previous["facts"]["request"]["outputRule"], json!([{"id":"disposition"}]));
    let old_id = previous["previewId"].as_str().unwrap();
    let mut changed = map.clone();
    changed["map_output_rule"]["files"] = json!([{"id":"review-record"}]);
    fs::write(&map_path, changed.to_string()).unwrap();
    let commit = json!({"sessionId":"chief-a","previewId":old_id}).to_string();
    let stale = body(post_json_over_tcp(addr, "/api/chief/delegations", &commit).await, 409);
    assert_eq!(stale["code"], "delegation-preview-stale");
    assert!(stale["message"].as_str().unwrap().contains("undeclared-output"), "{stale}");
    body(get_over_tcp(addr, &format!("/api/chief/delegations/{old_id}")).await, 404);
    assert_eq!(f.herdr.calls(), before);
    fs::write(&map_path, map.to_string()).unwrap();
    let reused = body(post_json_over_tcp(addr, "/api/chief/delegations", &commit).await, 409);
    assert_eq!(reused["code"], "delegation-preview-missing");
    assert_eq!(f.herdr.calls(), before);

    let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
    let id = preview["previewId"].as_str().unwrap();
    let started = body(post_json_over_tcp(addr, "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":id}).to_string()).await, 201);
    let delegation = body(get_over_tcp(addr, &format!("/api/chief/delegations/{id}")).await, 200);
    assert_eq!(delegation["facts"], preview["facts"]);
    assert_eq!(delegation["runId"], started["id"]);
    let submitted = common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
    let prompt = submitted.last().unwrap()["text"].as_str().unwrap();
    assert_eq!(prompt, preview["facts"]["prompt"].as_str().unwrap());
    assert!(prompt.contains("[Requested file outputs]\n[{\"id\":\"disposition\"}]\n[/Requested file outputs]"));
    assert!(prompt.contains("正式交付时按本次要求交接文件成果；Map 的公开文件成果取自已完成 Run。"));

    let route = format!("/api/workflow/runs/{}", started["id"].as_str().unwrap());
    let mut interim = Vec::new();
    for result in ["failure", "blocked"] {
        let mut report = reports::content(result);
        report.as_object_mut().unwrap().remove("outputs");
        let saved = post(&f.server, "/api/chief/reports", json!({"sessionId":"flow-manager","body":report})).await;
        assert_eq!(saved["report"]["body"]["result"], result);
        assert_eq!(saved["report"]["body"]["outputs"], json!([]));
        interim.push(saved["report"].clone());
    }
    let success = json!({"sessionId":"flow-manager","body":reports::content("success")});
    let before_reporting = f.herdr.calls();
    let early = body(post_json_over_tcp(addr, "/api/chief/reports", &success.to_string()).await, 400);
    assert_eq!(early["code"], "map-run-not-finished");
    assert_eq!(f.herdr.calls(), before_reporting);
    assert_eq!(body(get_over_tcp(addr, "/api/chief/reports").await, 200)["reports"], json!(interim));
    assert_eq!(body(get_over_tcp(addr, &route).await, 200)["run"], started["run"], "chief reports did not advance the Run");

    post(&f.server, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"flow-manager","fileInputs":started["run"]["mapInput"]["files"],"confirmed":true,"reason":"Approved inputs"})).await;
    let end_dir = f.root.path().join("materials");
    let end_history = f.root.path().join("review-end.jsonl");
    fs::write(&end_history, json!({"type":"session","version":3,"id":"review-end","timestamp":"2026-11-12T00:00:00.000Z","cwd":end_dir}).to_string()+"\n").unwrap();
    f.herdr.set_behavior(json!({"native_cwd":end_dir,"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":end_history}}));
    let dispatched = post(&f.server, "/api/workflow/manager/dispatch", json!({"sessionId":"flow-manager","nodes":["review"]})).await;
    assert_eq!(dispatched["canFinish"], true, "{dispatched}");

    let canonical = f.root.path().join("work/status-731.txt");
    let ancillary = f.root.path().join("work/detail-904.txt");
    let decoy = f.root.path().join("work/other-address.txt");
    fs::write(&canonical, b"Official public disposition\r\n").unwrap();
    fs::write(&ancillary, b"Map's other mandatory public file").unwrap();
    fs::write(&decoy, b"Different output, not the Run's").unwrap();
    let deliver = json!({"id":"disposition","path":canonical});
    let other = json!({"id":"review-record","path":ancillary});
    let subset = json!({"sessionId":"flow-manager","mapOutput":{"files":[deliver]}});
    let missing = body(post_json_over_tcp(addr, "/api/workflow/manager/finish", &subset.to_string()).await, 409);
    assert_eq!(missing["code"], "output-missing");
    assert_eq!(missing["snapshot"]["status"], "running");
    let all = json!({"files":[other,deliver]});
    let before_finish = f.herdr.calls();
    let finished = post(&f.server, "/api/workflow/manager/finish", json!({"sessionId":"flow-manager","mapOutput":all})).await;
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["mapOutput"], all);
    assert_eq!(body(get_over_tcp(addr, "/api/chief/reports").await, 200)["reports"], json!(interim));
    assert_eq!(f.herdr.calls(), before_finish, "finish never submits a chief report");

    let mut override_report = success.clone();
    override_report["body"]["outputs"] = json!([{"id":"disposition","path":decoy}]);
    let denied = body(post_json_over_tcp(addr, "/api/chief/reports", &override_report.to_string()).await, 400);
    assert_eq!(denied["code"], "map-output-override");
    assert_eq!(body(get_over_tcp(addr, "/api/chief/reports").await, 200)["reports"], json!(interim));
    assert_eq!(f.herdr.calls(), before_finish);
    fs::remove_file(&canonical).unwrap();
    let missing_file = body(post_json_over_tcp(addr, "/api/chief/reports", &success.to_string()).await, 400);
    assert_eq!(missing_file["code"], "output-unreadable");
    assert_eq!(f.herdr.calls(), before_finish);
    assert_eq!(body(get_over_tcp(addr, "/api/chief/reports").await, 200)["reports"], json!(interim));
    fs::write(&canonical, b"Official public disposition\r\n").unwrap();
    let saved = post(&f.server, "/api/chief/reports", success).await;
    let mut expected = reports::content("success");
    expected["outputs"] = json!([deliver]);
    assert_eq!(saved["report"]["body"], expected);
    let readback = body(get_over_tcp(addr, &format!("/api/chief/reports/{}", saved["report"]["id"].as_str().unwrap())).await, 200);
    assert_eq!(readback["body"], expected);
    assert_eq!(fs::read(readback["body"]["outputs"][0]["path"].as_str().unwrap()).unwrap(), b"Official public disposition\r\n");
    assert_eq!(body(get_over_tcp(addr, &route).await, 200)["run"]["mapOutput"], all);
}

// Ticket 09 confirmation must reject old content, never silently run changed
// Map work. The expected outcome is an explicit re-confirmation error and no
// CLI effect, not simply a successful launch with the newest definitions.
#[tokio::test]
async fn map_confirmation_rejects_changed_map_manager_node_binding_and_material_meaning() {
    for change in ["map", "node-task", "node-zone", "manager", "material-bytes", "bot", "nested-map", "direct-material", "deleted-material", "dictionary"] {
        let (f, manager, request) = map_fixture().await;
        if change == "nested-map" {
            let path = f.root.path().join("maps/flow.json");
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            fs::write(f.root.path().join("maps/child.json"), map.to_string()).unwrap();
            fs::write(f.root.path().join("bots/nested.json"), json!({"id":"nested-bot","name":"Nested review","when":"Nested work","description":"Review","input_rule":"Review","output_rule":"","map_file":"child.json"}).to_string()).unwrap();
            map["nodes"]["review"]["bot_id"] = json!("nested-bot");
            map["nodes"]["review"]["manager_task_id"] = manager["id"].clone();
            map["nodes"]["review"]["input_rule"][0]["id"] = json!("brief");
            fs::write(path, map.to_string()).unwrap();
        } else if change == "direct-material" {
            fs::write(f.root.path().join("materials/internal-reference.txt"), "APPROVED REFERENCE").unwrap();
            let path = f.root.path().join("maps/flow.json");
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            map["nodes"]["review"]["input_rule"].as_array_mut().unwrap().push(json!({"type":"file","id":"reference","zone_id":"review-zone","path":"internal-reference.txt"}));
            fs::write(path, map.to_string()).unwrap();
        }
        if change == "dictionary" {
            fs::write(&f.config.prompts_file, "review_scope = 'Review without publishing'\n").unwrap();
            let path = f.root.path().join("maps/flow.json");
            let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            map["nodes"]["review"]["input"] = json!("${review_scope}");
            fs::write(path, map.to_string()).unwrap();
        }
        let preview = post(&f.server, "/api/chief/delegations/preview", request).await;
        match change {
            "dictionary" => fs::write(&f.config.prompts_file, "review_scope = 'Publish immediately without review'\n").unwrap(),
            "bot" => {
                let path = f.root.path().join("bots/flow.json");
                let mut bot: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                bot["description"] = json!("DIFFERENT PLANNING REQUIREMENTS");
                fs::write(path, bot.to_string()).unwrap();
            }
            "direct-material" => fs::write(f.root.path().join("materials/internal-reference.txt"), "UNAPPROVED REFERENCE").unwrap(),
            "deleted-material" => fs::remove_file(f.root.path().join("materials/brief.txt")).unwrap(),
            "map" | "nested-map" => {
                let path = f.root.path().join(if change == "nested-map" { "maps/child.json" } else { "maps/flow.json" });
                let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                map["nodes"]["review"]["input"] = json!("Publish without review: NOT APPROVED");
                fs::write(path, map.to_string()).unwrap();
            }
            "manager" | "node-task" => {
                let mut task = if change == "manager" { manager } else { f.worker.clone() };
                let id = task.as_object_mut().unwrap().remove("id").unwrap();
                task["prompt"] = json!("DIFFERENT FIXED BUSINESS INSTRUCTION");
                body(common::request_json_over_tcp(f.server.addr(), "PUT", &format!("/api/tasks/{}", id.as_str().unwrap()), Some(&task.to_string())).await, 200);
            }
            "node-zone" => {
                fs::create_dir(f.root.path().join("new-node-location")).unwrap();
                fs::write(f.root.path().join("new-node-location/brief.txt"), "Different source").unwrap();
                body(common::request_json_over_tcp(f.server.addr(), "PUT", "/api/zones", Some(&json!({"id":"review-zone","path":f.root.path().join("new-node-location")}).to_string())).await, 200);
            }
            _ => fs::write(f.root.path().join("materials/brief.txt"), "DIFFERENT CONTENT NOT APPROVED").unwrap(),
        }
        let before = f.herdr.calls().len();
        let response = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 409);
        assert_eq!(response["code"], "delegation-preview-stale", "{change}: {response}");
        assert!(response["message"].as_str().unwrap().contains("确认"));
        assert_eq!(f.herdr.calls().len(), before, "{change}: no unapproved launch");
        assert_eq!(body(get_over_tcp(f.server.addr(), "/api/workflow/runs").await, 200)["runs"], json!([]));
    }
}
