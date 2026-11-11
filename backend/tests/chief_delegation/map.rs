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
    let observed = body(observed.expect("Map prompt gate must be reached"), 200);
    assert_eq!(observed["delegations"][0]["runId"], started["id"]);
    assert_eq!(observed["delegations"][0]["sessionId"], "flow-manager");
    assert_eq!(submitted, 1, "only initial chief input preceded release");
    let manager = json!({"sessionId":"flow-manager","nodes":["review"]});
    let blocked = post(&f.server, "/api/workflow/manager/dispatch", manager.clone()).await;
    assert_eq!(blocked["snapshot"]["nodes"]["review"]["status"], "pending");
    assert_eq!(blocked["results"][0]["status"], "rejected");
    post(&f.server, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"flow-manager","fileInputs":started["run"]["fileInputs"],"confirmed":true,"reason":"Explicit independent review of brief binding"})).await;
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
        let started = body(post_json_over_tcp(f.server.addr(), "/api/chief/delegations", &json!({"sessionId":"chief-a","previewId":preview["previewId"]}).to_string()).await, 500);
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
async fn map_preview_requires_explicit_manager_and_all_required_named_materials() {
    let (f, _, request) = map_fixture().await;
    let before = f.herdr.calls().len();
    for missing in ["manager", "required-file", "wrong-manager-role", "undeclared-file"] {
        let mut input = request.clone();
        match missing {
            "manager" => { input.as_object_mut().unwrap().remove("managerTaskId"); }
            "required-file" => input["materials"] = json!([]),
            "wrong-manager-role" => input["managerTaskId"] = f.worker["id"].clone(),
            _ => input["materials"][0]["id"] = json!("not-declared"),
        }
        let response = post_json_over_tcp(f.server.addr(), "/api/chief/delegations/preview", &input.to_string()).await;
        assert!(response.status >= 400, "{missing}: {}", response.body);
        assert_eq!(f.herdr.calls().len(), before);
    }
}

async fn map_fixture() -> (Fixture, Value, Value) {
    let f = Fixture::new().await;
    let tasks = post(&f.server, "/api/tasks", json!({"name":"flow manager","taskType":"manager","chatName":"manager {chat.id}","agent":"pi","model":"default","prompt":"MANAGER-FIXED-731"})).await;
    let manager = tasks["tasks"].as_array().unwrap().iter().find(|t| t["name"] == "flow manager").unwrap().clone();
    post(&f.server, "/api/zones", json!({"id":"review-zone","path":f.root.path().join("materials")})).await;
    fs::create_dir(f.root.path().join("maps")).unwrap();
    fs::write(f.root.path().join("maps/flow.json"), json!({"title":"Review flow","description":"Independent flow","input_files":[{"id":"brief","required":true}],"nodes":{"review":{"title":"Review","bot_id":"brief-bot","input":"Review the named brief without publishing","zone":"review-zone","input_rule":[{"type":"run","id":"source","input_id":"brief"}],"blocked_by":[]}}}).to_string()).unwrap();
    fs::write(f.root.path().join("bots/flow.json"), json!({"id":"flow-bot","name":"Review flow","when":"Review a brief","plan_guide":"Supply brief","node_guide":"Review","map_file":"flow.json"}).to_string()).unwrap();
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
    assert_eq!(run["userInput"], request["input"]);
    assert!(run.get("parent").is_none(), "chief is not a parent node: {run}");
    assert_eq!(run["manager"]["sessionId"], "flow-manager");
    assert_eq!(run["inputsReady"], false, "launch cannot approve workflow inputs");
    assert_eq!(run["nodes"]["review"]["status"], "pending");
    assert!(common::same_path(std::path::Path::new(run["zoneBindings"]["review-zone"].as_str().unwrap()), &f.root.path().join("materials")));
    assert_eq!(run["fileInputs"][0]["id"], "brief");
    assert!(common::same_path(std::path::Path::new(run["fileInputs"][0]["path"].as_str().unwrap()), &f.root.path().join("materials/brief.txt")));
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
    let detail = body(get_over_tcp(f.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await, 200);
    assert_eq!(detail["run"], *run);
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
            fs::write(f.root.path().join("bots/nested.json"), json!({"id":"nested-bot","name":"Nested review","when":"Nested work","plan_guide":"Review","node_guide":"Review","map_file":"child.json"}).to_string()).unwrap();
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
                bot["plan_guide"] = json!("DIFFERENT PLANNING REQUIREMENTS");
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
