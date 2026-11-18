//! Approved S2: public HTTP, persisted facts and controlled external host.
use super::*;

async fn manager(s: &Scenario, name: &str, host: &str) -> String {
    let response = post_json_over_tcp(s.server.addr(), "/api/tasks", &json!({"name":name,"taskType":"manager","agent":host,"chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","prompt":format!("FIXED-{name}")}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    value["tasks"].as_array().unwrap().iter().find(|task| task["name"] == name).unwrap()["id"].as_str().unwrap().to_owned()
}
fn native(s: &Scenario, host: &str, session: &str, zone: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":format!("herdr:{host}"),"agent":host,"kind":"id","value":session},"native_cwd":s.data.path().join(zone)}));
}
async fn detail(s: &Scenario, id: &str) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
fn nested(s: &Scenario, task: &str, input: &str) {
    fs::write(s.config.scheduler.bot_directory.join("nested.json"), json!({"id":"nested","name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"A":{"title":"Internal A","bot_id":"worker","input":"INNER-WORK","zone":"manager","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{"A":{"title":"Call","bot_id":"nested","manager_task_id":task,"input":input,"zone":"node","blocked_by":[]},"B":{"title":"Call again","bot_id":"nested","manager_task_id":task,"input":input,"zone":"node","blocked_by":[]}}}).to_string()).unwrap();
}

async fn role_request(s: &Scenario, route: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), route, &body.to_string()).await;
    assert_eq!(response.status, status, "{route}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

#[tokio::test]
async fn dynamic_zone_selects_only_nested_manager_and_preserves_internal_fixed_zones() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "DYNAMIC-CHILD");
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    map["nodes"]["A"]["zone"] = json!({"type":"dynamic"});
    fs::write(path, map.to_string()).unwrap();
    let started = s.start("dynamic-parent").await;
    let root = s.data.path().join("dynamic-child-root");
    fs::create_dir(&root).unwrap();
    let registered = post_json_over_tcp(s.server.addr(), "/api/zones", &json!({"id":"child-root","path":root}).to_string()).await;
    assert_eq!(registered.status, 200, "{}", registered.body);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"dynamic-child"},"native_cwd":root}));
    let dispatched = role_request(&s, "/api/workflow/manager/dispatch", json!({"sessionId":"dynamic-parent","nodes":["A"],"zoneIds":{"A":"child-root"}}), 200).await;
    assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
    let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(execution["zoneId"], "child-root");
    assert_eq!(execution["zone"], json!(root));
    let child = detail(&s, execution["childRunId"].as_str().unwrap()).await;
    assert_eq!(child["run"]["managerZoneId"], "child-root");
    assert_eq!(child["run"]["zone"], json!(root));
    assert_eq!(child["run"]["zoneBindings"]["manager"], json!(s.data.path()));
    assert_eq!(child["run"]["parent"]["runId"], started["id"]);
    assert!(dispatched["snapshot"]["zoneBindings"].get("child-root").is_none());
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"dynamic-internal-fixed"},"native_cwd":s.data.path()}));
    let internal = s.dispatch("dynamic-child", &["A"]).await;
    assert_eq!(internal["results"][0]["status"], "running", "{internal}");
    assert_eq!(internal["snapshot"]["nodes"]["A"]["execution"]["zoneId"], "manager");
    assert_eq!(internal["snapshot"]["nodes"]["A"]["execution"]["zone"], json!(s.data.path()));
}

// P21/P22: the same real Chat must address its child manager and exact parent
// attempt without a caller-chosen Run. Rejecting every mixed role breaks report;
// accepting all mixed roles would let a collision act on unrelated work.
#[tokio::test]
async fn dual_role_routes_report_to_parent_and_management_to_child_without_selectors() {
    let s = Scenario::with_collect_wait(1).await;
    nested(&s, &s.manager_task, "DUAL");
    let root = s.start("dual-root").await;
    native(&s, "omp", "dual-child", "node-zone");
    let dispatched = s.dispatch("dual-root", &["A"]).await;
    let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
    let child_id = execution["childRunId"].as_str().unwrap();
    for route in ["status", "collect"] {
        let value = role_request(&s, &format!("/api/workflow/manager/{route}"), json!({"sessionId":"dual-child"}), 200).await;
        assert_eq!(value["snapshot"]["id"], child_id);
    }
    let calls = s.herdr.calls();
    let context = role_request(&s, "/api/workflow/node/context", json!({"sessionId":"dual-child"}), 200).await;
    assert_eq!(context, json!({"runId":root["id"],"nodeId":"A","executionId":execution["id"],"autoApproved":true,"approved":false,"approvalSource":null}));
    assert_eq!(s.herdr.calls(), calls);
    assert!(detail(&s, root["id"].as_str().unwrap()).await["reports"].as_array().unwrap().is_empty());
    let report = role_request(&s, "/api/workflow/node/report", json!({"sessionId":"dual-child","body":"Progress, not complete","canComplete":false}), 200).await;
    assert_eq!(report["report"]["runId"], root["id"]);
    assert_eq!(report["report"]["nodeId"], "A");
    assert_eq!(report["report"]["executionId"], execution["id"]);
    assert_eq!(report["report"]["from"], execution["chat"]);
    assert_eq!(report["report"]["to"], root["run"]["manager"]);
    let calls = s.herdr.calls();
    for (route, body) in [("/api/workflow/manager/status", json!({"sessionId":"dual-child","runId":root["id"]})), ("/api/workflow/manager/collect", json!({"sessionId":"dual-child","nodeId":"A"})), ("/api/workflow/node/report", json!({"sessionId":"dual-child","body":"forge","canComplete":false,"executionId":"other"}))] {
        role_request(&s, route, body, 400).await;
    }
    assert_eq!(s.herdr.calls(), calls);
    assert!(detail(&s, child_id).await["reports"].as_array().unwrap().is_empty());
    // Existing resume consumer must recognize the newly authorized persisted
    // role union; full resume/restart lifecycle coverage belongs to ticket 08.
    let offset = s.herdr.calls().len();
    role_request(&s, "/api/chats/resume", json!({"sessionId":"dual-child"}), 200).await;
    let calls = s.herdr.calls();
    let start = calls[offset..].iter().find(|call| call["kind"] == "agent_start").unwrap();
    for role in ["manager", "node"] { assert!(start["args"].as_array().unwrap().contains(&json!(s.data.path().join(format!("omp-{role}-chat.ts"))))); }
}

// P22: an old child manager remains attached to its exact retained execution,
// not the same-named current node. A lookup by node name or newest child would
// misattribute this late report or let old management select the replacement.
#[tokio::test]
async fn retired_child_session_cannot_select_the_replacement_execution() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "RETRY-ATTRIBUTION");
    let root = s.start("attempt-root").await;
    native(&s, "omp", "attempt-old", "node-zone");
    let first = s.dispatch("attempt-root", &["A"]).await;
    let old = first["snapshot"]["nodes"]["A"]["execution"].clone();
    role_request(&s, "/api/workflow/manager/fail", json!({"sessionId":"attempt-root","nodeId":"A","reason":"User confirmed parent failure"}), 200).await;
    native(&s, "omp", "attempt-new", "node-zone");
    let retry = role_request(&s, "/api/workflow/manager/retry", json!({"sessionId":"attempt-root","nodeId":"A","confirmed":true,"reason":"Approved new attempt"}), 200).await;
    let current = &retry["snapshot"]["nodes"]["A"]["execution"];
    assert_ne!(current["id"], old["id"]);
    assert_ne!(current["childRunId"], old["childRunId"]);
    // P10: a retained child may report historical facts, but approval cannot
    // retarget its replacement or mutate either the old child or execution.
    let old_child = detail(&s, old["childRunId"].as_str().unwrap()).await;
    let context = role_request(&s, "/api/workflow/node/context", json!({"sessionId":"attempt-old"}), 200).await;
    assert_eq!(context["executionId"], old["id"]);
    let stale = role_request(&s, "/api/workflow/node/approve", json!({"sessionId":"attempt-old","executionId":old["id"],"source":"auto"}), 400).await;
    assert_eq!(stale["code"], "approval-execution-stale");
    let forged = role_request(&s, "/api/workflow/node/approve", json!({"sessionId":"attempt-old","executionId":current["id"],"source":"auto"}), 400).await;
    assert_eq!(forged["code"], "approval-execution-mismatch");
    assert_eq!(detail(&s, old["childRunId"].as_str().unwrap()).await, old_child);
    assert_eq!(detail(&s, root["id"].as_str().unwrap()).await["run"]["nodes"]["A"]["execution"], *current);
    let latest = role_request(&s, "/api/workflow/node/report", json!({"sessionId":"attempt-new","body":"NEW progress","canComplete":false}), 200).await;
    let late = role_request(&s, "/api/workflow/node/report", json!({"sessionId":"attempt-old","body":"OLD late progress","canComplete":false}), 200).await;
    assert_eq!(late["report"]["executionId"], old["id"]);
    assert_eq!(late["report"]["runId"], root["id"]);
    let status = role_request(&s, "/api/workflow/manager/status", json!({"sessionId":"attempt-root"}), 200).await;
    assert_eq!(status["pendingReports"]["A"], latest["report"]["id"]);
    assert_eq!(status["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], old);
    for (session, child) in [("attempt-old", &old["childRunId"]), ("attempt-new", &current["childRunId"])] {
        for route in ["status", "collect"] {
            assert_eq!(role_request(&s, &format!("/api/workflow/manager/{route}"), json!({"sessionId":session}), 200).await["snapshot"]["id"], *child);
        }
    }
}

#[tokio::test]
async fn startup_nested_dispatch_ignores_unreadable_history_and_binds_both_runs() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "NESTED-BUSINESS-INPUT");
    s.start("history-independent-parent").await;
    let historical = s.config.scheduler.run_directory.join("unreadable-history");
    fs::create_dir(&historical).unwrap();
    fs::write(historical.join("run.json"), b"{}").unwrap();
    native(&s, "omp", "history-independent-child", "node-zone");
    let result = s.dispatch("history-independent-parent", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "running", "{result}");
    assert_eq!(result["results"][0]["effects"]["associationSaved"], true);
    assert_eq!(result["results"][0]["effects"]["inputSubmitted"], true);
    let execution = &result["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(execution["chat"]["sessionId"], "history-independent-child");
    let child = detail(&s, execution["childRunId"].as_str().unwrap()).await;
    assert_eq!(child["run"]["manager"], execution["chat"]);
    let calls = s.herdr.calls();
    let prompts: Vec<_> = calls.iter().filter(|c| c["kind"] == "agent_prompt").collect();
    assert_eq!(prompts.len(), 2);
    let text = common::prompt_body(prompts[1]);
    assert!(text.contains("NESTED-BUSINESS-INPUT"));
    assert!(text.contains(execution["childRunId"].as_str().unwrap()));
    assert_eq!(fs::read(historical.join("run.json")).unwrap(), b"{}");
}

// P21: only the reciprocal execution link authorizes dual roles. Each mutation
// is a real persisted foreign association, not a private-code mutation. A blanket
// mixed-role exemption or missing Task/agent check would permit one of these.
#[tokio::test]
async fn invalid_nested_links_and_source_mismatches_are_rejected_without_effects() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "AUTH");
    let root = s.start("auth-root").await;
    native(&s, "omp", "auth-child", "node-zone");
    let valid = s.dispatch("auth-root", &["A"]).await;
    let child_id = valid["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
    let parent_path = s.config.scheduler.run_directory.join(root["id"].as_str().unwrap()).join("run.json");
    let child_path = s.config.scheduler.run_directory.join(child_id).join("run.json");
    let parent: Value = serde_json::from_slice(&fs::read(&parent_path).unwrap()).unwrap();
    let child: Value = serde_json::from_slice(&fs::read(&child_path).unwrap()).unwrap();
    for violation in ["missing-link","missing-child","wrong-child","missing-manager","wrong-execution","cross-run","same-run","duplicate-node","source-task","source-agent","duplicate-manager"] {
        let mut p = parent.clone(); let mut c = child.clone();
        match violation {
            "missing-link" => { c.as_object_mut().unwrap().remove("parent"); },
            "missing-child" => { p["nodes"]["A"]["execution"].as_object_mut().unwrap().remove("childRunId"); },
            "wrong-child" => p["nodes"]["A"]["execution"]["childRunId"] = json!(root["id"]),
            "missing-manager" => c["manager"] = Value::Null,
            "wrong-execution" => c["parent"]["executionId"] = json!("foreign"),
            "cross-run" => c["parent"]["runId"] = json!(child_id),
            "same-run" => { p["manager"] = p["nodes"]["A"]["execution"]["chat"].clone(); c["manager"] = Value::Null; },
            "duplicate-node" => p["nodes"]["B"] = p["nodes"]["A"].clone(),
            "source-task" => p["nodes"]["A"]["execution"]["taskId"] = json!("foreign-task"),
            "source-agent" => p["nodes"]["A"]["execution"]["agent"] = json!("pi"),
            "duplicate-manager" => p["manager"] = c["manager"].clone(),
            _ => unreachable!(),
        }
        fs::write(&parent_path, p.to_string()).unwrap(); fs::write(&child_path, c.to_string()).unwrap();
        let calls = s.herdr.calls();
        for route in ["status", "collect"] {
            role_request(&s, &format!("/api/workflow/manager/{route}"), json!({"sessionId":"auth-child"}), 403).await;
        }
        role_request(&s, "/api/workflow/node/context", json!({"sessionId":"auth-child"}), 403).await;
        role_request(&s, "/api/workflow/node/approve", json!({"sessionId":"auth-child","executionId":parent["nodes"]["A"]["execution"]["id"],"source":"auto"}), 403).await;
        role_request(&s, "/api/workflow/node/report", json!({"sessionId":"auth-child","body":"not authorized","canComplete":false}), 403).await;
        assert_eq!(s.herdr.calls(), calls, "{violation}");
        assert_eq!(fs::read_to_string(&parent_path).unwrap(), p.to_string());
        assert_eq!(fs::read_to_string(&child_path).unwrap(), c.to_string());
    }
    fs::write(parent_path, parent.to_string()).unwrap(); fs::write(child_path, child.to_string()).unwrap();
    role_request(&s, "/api/workflow/manager/status", json!({"sessionId":"auth-child"}), 200).await;
}

// P10/P11: absence is legal only when the child declaration permits it.
// A pass-through that fabricates a placeholder or skips child preflight would
// start the required case. Observe empty bindings, explicit missing ID, and no
// additional host start or child record after changing only requiredness.
#[tokio::test]
async fn optional_absence_passes_nested_interface_but_cannot_bypass_required_child() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "OPTIONAL");
    let maps = &s.config.scheduler.map_directory;
    let mut outer: Value = serde_json::from_slice(&fs::read(maps.join("work.json")).unwrap()).unwrap();
    outer["input_files"] = json!([{"id":"maybe","required":false}]);
    for id in ["A","B"] { outer["nodes"][id]["input_rule"] = json!([{"type":"run","id":"material","input_id":"maybe"}]); }
    fs::write(maps.join("work.json"), outer.to_string()).unwrap();
    let mut inner: Value = serde_json::from_slice(&fs::read(maps.join("inner.json")).unwrap()).unwrap();
    inner["input_files"] = json!([{"id":"material","required":false}]);
    inner["nodes"]["A"]["input_rule"] = json!([{"type":"run","id":"local","input_id":"material"}]);
    fs::write(maps.join("inner.json"), inner.to_string()).unwrap();
    s.start("optional-root").await;
    native(&s, "omp", "optional-child", "node-zone");
    let first = s.dispatch("optional-root", &["A"]).await;
    let child_id = first["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
    assert_eq!(detail(&s, child_id).await["run"]["fileInputs"], json!([]));
    native(&s, "omp", "optional-worker", "");
    let work = s.dispatch("optional-child", &["A"]).await;
    assert_eq!(work["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(work["snapshot"]["nodes"]["A"]["execution"]["inputs"], json!([]));
    inner["input_files"][0]["required"] = json!(true);
    fs::write(maps.join("inner.json"), inner.to_string()).unwrap();
    native(&s, "omp", "required-child", "node-zone");
    let started = s.dispatch("optional-root", &["B"]).await;
    assert_eq!(started["results"][0]["status"], "running");
    let child_id = started["snapshot"]["nodes"]["B"]["execution"]["childRunId"].as_str().unwrap();
    assert_eq!(detail(&s, child_id).await["run"]["inputsReady"], false);
    let calls = s.herdr.calls();
    let rejected = role_request(&s, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"required-child","fileInputs":[],"confirmed":true,"reason":"Review cannot supply the absent material"}), 409).await;
    assert!(rejected.to_string().contains("material"));
    let rejected = role_request(&s, "/api/workflow/manager/dispatch", json!({"sessionId":"required-child","nodes":["A"]}), 200).await;
    assert_eq!(rejected["results"][0]["status"], "rejected");
    assert_eq!(s.herdr.calls(), calls);
    let runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&runs.body).unwrap()["runs"].as_array().unwrap().len(), 3);
}

// P12: output source IDs and child interface IDs are deliberately different.
// Passing upstream IDs through unchanged would reject or misbind this actual
// registered file. Exercise report→complete→dispatch, not hand-built outputs.
#[tokio::test]
async fn upstream_output_uses_parent_local_id_for_the_child_interface() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "CONSUME");
    let maps = &s.config.scheduler.map_directory;
    let mut outer: Value = serde_json::from_slice(&fs::read(maps.join("work.json")).unwrap()).unwrap();
    outer["nodes"]["source"] = json!({"title":"Source","bot_id":"worker","input":"Produce","zone":"node","blocked_by":[],"output_rule":[{"id":"upstream-result"}]});
    outer["nodes"]["A"]["blocked_by"] = json!(["source"]);
    outer["nodes"]["A"]["input_rule"] = json!([{"type":"node","id":"child-material","node_id":"source","output_id":"upstream-result"}]);
    fs::write(maps.join("work.json"), outer.to_string()).unwrap();
    let mut inner: Value = serde_json::from_slice(&fs::read(maps.join("inner.json")).unwrap()).unwrap();
    inner["input_files"] = json!([{"id":"child-material","required":true}]);
    fs::write(maps.join("inner.json"), inner.to_string()).unwrap();
    s.start("mapping-root").await;
    native(&s, "omp", "mapping-source", "node-zone");
    s.dispatch("mapping-root", &["source"]).await;
    let path = s.data.path().join("produced.txt");
    fs::write(&path, "UPSTREAM-REAL-729").unwrap();
    role_request(&s, "/api/workflow/node/report", json!({"sessionId":"mapping-source","body":"Produced","canComplete":true,"outputs":[{"id":"upstream-result","path":path}]}), 200).await;
    approve_execution(&s, "mapping-source").await;
    role_request(&s, "/api/workflow/manager/complete", json!({"sessionId":"mapping-root","nodeId":"source","reason":"Accept actual file"}), 200).await;
    native(&s, "omp", "mapping-child", "node-zone");
    let dispatched = s.dispatch("mapping-root", &["A"]).await;
    let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(execution["inputs"], json!([{"id":"child-material","path":path}]));
    assert_eq!(detail(&s, execution["childRunId"].as_str().unwrap()).await["run"]["fileInputs"], json!([{"id":"child-material","path":path}]));
    assert_eq!(fs::read_to_string(path).unwrap(), "UPSTREAM-REAL-729");
}

// P12: local IDs, not upstream IDs, form each child interface. A resolver
// using a global same-name binding would send the first Run's path to the
// second Run, or lose the outer→shared rename. Real files and two nested levels
// distinguish both violations through public execution facts.
#[tokio::test]
async fn multilevel_local_ids_bind_only_the_owning_run() {
    let s = Scenario::new().await;
    nested(&s, &s.manager_task, "OUTER");
    let maps = &s.config.scheduler.map_directory;
    fs::write(s.config.scheduler.bot_directory.join("leaf.json"), json!({"id":"leaf","name":"Leaf","when":"","plan_guide":"","node_guide":"","map_file":"leaf.json"}).to_string()).unwrap();
    fs::write(maps.join("work.json"), json!({"title":"Root","description":"","input_files":[{"id":"outer","required":true}],"nodes":{"A":{"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"OUTER","zone":"node","blocked_by":[],"input_rule":[{"type":"run","id":"shared","input_id":"outer"}]}}}).to_string()).unwrap();
    fs::write(maps.join("inner.json"), json!({"title":"Middle","description":"","input_files":[{"id":"shared","required":true}],"nodes":{"A":{"title":"Call leaf","bot_id":"leaf","manager_task_id":s.manager_task,"input":"MIDDLE","zone":"manager","blocked_by":[],"input_rule":[{"type":"run","id":"shared","input_id":"shared"}]}}}).to_string()).unwrap();
    fs::write(maps.join("leaf.json"), json!({"title":"Leaf","description":"","input_files":[{"id":"shared","required":true}],"nodes":{"A":{"title":"Consume","bot_id":"worker","input":"LEAF","zone":"node","blocked_by":[],"input_rule":[{"type":"run","id":"local","input_id":"shared"}]}}}).to_string()).unwrap();
    let mut children = Vec::new();
    for (root_session, child_session, grand_session, worker_session, file, body) in [("r-one","c-one","g-one","w-one","one.txt","FIRST-418"),("r-two","c-two","g-two","w-two","two.txt","SECOND-905")] {
        let path = s.data.path().join(file);
        fs::write(&path, body).unwrap();
        native(&s, "omp", root_session, "");
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":"root","fileInputs":[{"id":"outer","path":path}]}).to_string()).await;
        assert_eq!(response.status, 201, "{}", response.body);
        native(&s, "omp", child_session, "node-zone");
        let parent = s.dispatch(root_session, &["A"]).await;
        let child_id = parent["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
        let child = detail(&s, child_id).await;
        assert_eq!(child["run"]["fileInputs"], json!([{"id":"shared","path":path}]));
        assert_eq!(child["run"]["inputsReady"], false, "parent preparation does not attest the child's business scope");
        let calls = s.herdr.calls();
        let rejected = role_request(&s, "/api/workflow/manager/dispatch", json!({"sessionId":child_session,"nodes":["A"]}), 200).await;
        assert_eq!(rejected["results"][0]["status"], "rejected");
        assert_eq!(s.herdr.calls(), calls);
        let _ = s.prepare_fixture(child_session).await;
        native(&s, "omp", grand_session, "");
        let middle = s.dispatch(child_session, &["A"]).await;
        let grand_id = middle["snapshot"]["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
        assert_eq!(detail(&s, grand_id).await["run"]["fileInputs"], json!([{"id":"shared","path":path}]));
        native(&s, "omp", worker_session, "node-zone");
        let leaf = s.dispatch(grand_session, &["A"]).await;
        assert_eq!(leaf["snapshot"]["nodes"]["A"]["execution"]["inputs"], json!([{"id":"local","path":path}]));
        assert_eq!(fs::read_to_string(&path).unwrap(), body);
        children.push(child_id.to_owned());
    }
    assert_ne!(children[0], children[1]);
}

// P04/P05/P06/P10/P21: independent launch literals for both hosts. A launcher
// that uses the root manager, overwrites inner zones, expands the graph or omits
// one role cannot satisfy the actual host invocation and public persisted facts.
#[tokio::test]
async fn nested_dispatch_uses_node_manager_business_zone_and_independent_runs() {
    for (host, name, business, session) in [("omp", "Review manager", "REVIEW 原始要求\n保留权限", "nested-review"), ("pi", "Ship manager", "SHIP exact request\nDo not execute", "nested-ship")] {
        let s = Scenario::new().await;
        let task = manager(&s, name, host).await;
        nested(&s, &task, business);
        let root = s.start("root-manager").await;
        let offset = s.herdr.calls().len();
        native(&s, host, session, "node-zone");
        let dispatched = s.dispatch("root-manager", &["A"]).await;
        assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running", "{dispatched}");
        let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
        let child_id = execution["childRunId"].as_str().expect("child run identity");
        let child = detail(&s, child_id).await;
        assert_eq!(child["run"]["managerTaskId"], task);
        assert_eq!(child["run"]["managerZoneId"], "node");
        assert_eq!(child["run"]["zone"], json!(s.data.path().join("node-zone")));
        assert_eq!(child["run"]["userInput"], business);
        assert_eq!(child["run"]["fileInputs"], json!([]));
        assert_eq!(child["run"]["parent"], json!({"runId":root["id"],"nodeId":"A","executionId":execution["id"]}));
        assert_eq!(child["run"]["manager"], execution["chat"]);
        assert_eq!(dispatched["snapshot"]["nodes"].as_object().unwrap().len(), 2);
        let calls = s.herdr.calls();
        let starts: Vec<_> = calls[offset..].iter().filter(|call| call["kind"] == "agent_start").collect();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0]["agent_kind"], host);
        for role in ["manager", "node"] { assert!(starts[0]["args"].as_array().unwrap().contains(&json!(s.data.path().join(format!("{host}-{role}-chat.ts")))), "{}", starts[0]); }
        let prompt = calls[offset..].iter().find(|call| call["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap();
        assert!(prompt.contains(business), "{prompt}");
        assert!(prompt.contains(&format!("FIXED-{name}")), "{prompt}");
        assert!(!prompt.contains("MANAGER-FIXED"), "root manager preset must not replace node manager");
        assert_eq!(s.herdr.tabs().last().unwrap()["cwd"], json!(s.data.path().join("node-zone")));
        native(&s, host, &format!("{session}-second"), "node-zone");
        let second = s.dispatch("root-manager", &["B"]).await;
        assert_eq!(second["snapshot"]["nodes"]["B"]["status"], "running", "{second}");
        assert_ne!(second["snapshot"]["nodes"]["B"]["execution"]["childRunId"], child_id);
        assert_ne!(second["snapshot"]["nodes"]["B"]["execution"]["id"], execution["id"]);
        native(&s, "omp", "inner-worker", "");
        let inner = s.dispatch(session, &["A"]).await;
        assert_eq!(inner["snapshot"]["id"], child_id);
        assert_eq!(inner["snapshot"]["nodes"]["A"]["status"], "running", "{inner}");
        assert_eq!(inner["snapshot"]["nodes"]["A"]["execution"]["zone"], json!(s.data.path()));
        assert_eq!(s.herdr.tabs().last().unwrap()["cwd"], json!(s.data.path()));
        assert_eq!(detail(&s, root["id"].as_str().unwrap()).await["run"]["nodes"]["A"]["execution"], *execution);
    }
}
