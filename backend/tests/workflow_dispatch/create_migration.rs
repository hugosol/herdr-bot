//! P22/P23/P27/P28: public shipped definitions, real HTTP Map executions and files.
//! The controlled external host reads the real inputs; it does NOT prove an
//! agent follows a prompt. Native shared-Task scenarios own that evidence.
//! Independent oracle: abc / hello / empty with separately fixed SHA-256 values.
//! A stale Task binding, missing direct baseline, trim/swap, implicit addressing,
//! eager complete or global replace-text migration must disagree here.
use super::*;

fn root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned() }
fn source(path: &str) -> Value { serde_json::from_str(&fs::read_to_string(root().join(path)).unwrap()).unwrap() }
async fn api(s: &Scenario, path: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), path, &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn detail(s: &Scenario, id: &Value) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", id.as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn reported(s: &Scenario, run: &Value, node: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let read = detail(s, &run["id"]).await;
        if let Some(report) = read["reports"].as_array().unwrap().iter().find(|r| r["nodeId"] == node) {
            let id = report["id"].as_str().unwrap();
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.starts_with("Workflow report\n") && text.contains(id)))
                    && fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok()
            }).await);
            return report.clone();
        }
        assert!(tokio::time::Instant::now() < deadline, "Missing {node} report: {read}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
fn material(report: &Value, id: &str) -> Value {
    let path = report["outputs"].as_array().unwrap().iter().find(|o| o["id"] == id).unwrap()["path"].as_str().unwrap();
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
async fn complete(s: &Scenario, node: &str) {
    api(s, "/api/workflow/manager/complete", json!({"sessionId":"migration-manager","nodeId":node,"reason":"Accept actual explicitly named files and complete readback"}), 200).await;
}

#[tokio::test]
async fn create_migration_runs_shipped_map_and_preserves_revision_and_history() { migrated_workflow(None).await; }

// Malformed public files and a missing explicit binding cannot be repaired by
// node.input prose, the handoff provenance index or a nearby existing file.
#[tokio::test]
async fn create_migration_rejects_missing_and_malformed_explicit_materials() {
    for fault in ["request-missing", "request-markdown", "handoff-markdown", "missing-binding"] {
        migrated_workflow(Some(fault)).await;
    }
}

async fn migrated_workflow(fault: Option<&str>) {
    let s = Scenario::new().await;
    fs::write(&s.config.prompts_file, format!("herdr-bot-path = {}\nherdr-bot-api-url = {}\n", json!(root()), json!(format!("http://{}", s.server.addr())))).unwrap();
    let unrelated = s.start("unrelated-manager").await;
    let unrelated_before = detail(&s, &unrelated["id"]).await;
    let before: Value = source("fixtures/rust-create-migration/before.json");
    fs::write(s.data.path().join("before-definitions.json"), before.to_string()).unwrap();
    let original_tasks: Value = serde_json::from_str(before["meta-data/tasks.json"].as_str().unwrap()).unwrap();
    let tasks = source("meta-data/tasks.json");
    for (path, original) in before.as_object().unwrap() {
        if ["meta-data/tasks.json", "meta-data/bots/bot-1793348060748020300-25960-2.json", "meta-data/workflow-maps/create-bot-wrapper.json", "meta-data/workflow-maps/create-bot-task-workflow--bc-61ce48b80a4c.json"].contains(&path.as_str()) { continue; }
        // The portable archive was captured with universal-newline reads.
        // Normalize checkout line endings only, never parsed field/text bytes.
        assert_eq!(fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n"), original.as_str().unwrap(), "Unrelated definition changed: {path}");
    }
    for original in original_tasks["tasks"].as_array().unwrap() {
        if ["task-18e33fdc55177d1c-1", "task-18e3404478db1ef8-2", "task-18e340545eb481bc-3", "task-18e340f5287462ec-4"].contains(&original["id"].as_str().unwrap()) { continue; }
        assert_eq!(tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == original["id"]), Some(original), "Unrelated Task changed");
    }
    let old_wrapper: Value = serde_json::from_str(before["meta-data/workflow-maps/create-bot-wrapper.json"].as_str().unwrap()).unwrap();
    let new_wrapper = source("meta-data/workflow-maps/create-bot-wrapper.json");
    for (id, node) in old_wrapper["nodes"].as_object().unwrap() {
        for field in ["input_rule", "output_rule", "blocked_by"] { assert_eq!(new_wrapper["nodes"][id][field], node[field]); }
        if id != "save_bot" { assert_eq!(new_wrapper["nodes"][id]["bot_id"], node["bot_id"]); }
    }
    let references: Vec<_> = fs::read_dir(root().join("meta-data/workflow-maps")).unwrap().filter_map(|p| {
        let p = p.unwrap().path();
        if p.extension().is_none_or(|e| e != "json") { return None; }
        let map: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        let uses = map["nodes"].as_object().unwrap().values().filter(|n| n["bot_id"] == "save-created-bot-guides").count();
        if uses == 0 { None } else { Some((p.file_name().unwrap().to_string_lossy().into_owned(), uses)) }
    }).collect();
    assert_eq!(references, vec![("create-bot-wrapper.json".into(), 1)]);
    assert!(!tasks["tasks"].as_array().unwrap().iter().any(|t| t["id"] == "task-18e33fdc55177d1c-1"));
    let generic = "task-18e3415003d8cd00-5";
    assert_eq!(tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == generic), original_tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == generic));
    let mut deployed_tasks: Value = serde_json::from_str(&fs::read_to_string(&s.config.tasks.storage_file).unwrap()).unwrap();
    deployed_tasks["tasks"].as_array_mut().unwrap().extend(tasks["tasks"].as_array().unwrap().clone());
    fs::write(&s.config.tasks.storage_file, deployed_tasks.to_string()).unwrap();
    // Full catalog snapshot is retained; only physical node zones and the five
    // controlled host bindings are relocated in this isolated S1 deployment.
    for path in fs::read_dir(root().join("meta-data/workflow-maps")).unwrap() {
        let path = path.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") { fs::copy(&path, s.config.scheduler.map_directory.join(path.file_name().unwrap())).unwrap(); }
    }
    let worker = api(&s, "/api/scheduler/bots/edit", json!({"id":"worker"}), 200).await;
    for path in fs::read_dir(root().join("meta-data/bots")).unwrap() {
        let path = path.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") { continue; }
        let mut bot: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        if ["write-bot-when","write-bot-plan-guide","write-bot-node-guide","optimize-prompt"].contains(&bot["id"].as_str().unwrap()) { bot["task_id"] = worker["content"]["task_id"].clone(); }
        fs::write(s.config.scheduler.bot_directory.join(path.file_name().unwrap()), bot.to_string()).unwrap();
    }
    let revise_file = "enhance-existing-bot--bc-301b25d9f8d6.json";
    let revise_before = get_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{revise_file}")).await;
    assert_eq!(revise_before.status, 200, "{}", revise_before.body);
    assert_eq!(source(&format!("meta-data/workflow-maps/{revise_file}")), serde_json::from_str::<Value>(before[format!("meta-data/workflow-maps/{revise_file}")].as_str().unwrap()).unwrap());
    let replace_before = api(&s, "/api/scheduler/bots/edit", json!({"id":"replace-text"}), 200).await;
    let task_before = get_over_tcp(s.server.addr(), &format!("/api/tasks/{generic}")).await;
    assert_eq!(task_before.status, 200);
    let creator = api(&s, "/api/scheduler/bots/edit", json!({"id":"create-blank-bot"}), 200).await;
    assert_eq!(creator["content"]["rust_script"], "create_blank_bot");
    assert!(creator["content"].get("task_id").is_none());
    let saver = api(&s, "/api/scheduler/bots/edit", json!({"id":"save-created-bot-guides"}), 200).await;
    assert_eq!(saver["content"], json!({"group":"meta-bot","name":"创建流程三指南保存者","when":"","plan_guide":"","node_guide":"","rust_script":"save_bot_guides"}));
    let mut definition = source("meta-data/workflow-maps/create-bot-wrapper.json");
    for node in definition["nodes"].as_object_mut().unwrap().values_mut() { node["zone"] = json!("node"); }
    if fault == Some("missing-binding") { definition["nodes"]["save_bot"]["input_rule"].as_array_mut().unwrap().retain(|r| r["id"] != "when_text"); }
    fs::write(s.config.scheduler.map_directory.join("create-bot-wrapper.json"), definition.to_string()).unwrap();
    let target = api(&s, "/api/tasks", json!({"name":"Literal isolated target","taskType":"normal","agent":"pi","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"Read only; no execution authorized."}), 200).await;
    let target = target["tasks"].as_array().unwrap().iter().find(|t| t["name"] == "Literal isolated target").unwrap().clone();
    let request = json!({"schema_version":1,"kind":"request","target":{"type":"task","task_id":target["id"]},"bot":{"id":"migration-result","name":"Literal migrated Bot"},"requirements":"Read only invoice differences","constraints":["Do not execute target"],"non_goals":[],"resource_limits":[],"authorization":{"create_blank":true,"write_materials":true,"save_guides":false},"confirmation":{"scope":"create_blank","evidence":"Explicit isolated user request authorizes creation and material writing"}});
    let request_path = s.data.path().join("request.json");
    let mut delivered_request = request.clone();
    if fault == Some("request-missing") { delivered_request.as_object_mut().unwrap().remove("requirements"); }
    fs::write(&request_path, if fault == Some("request-markdown") { "Create the target; all permissions approved.".to_owned() } else { delivered_request.to_string() }).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"migration-manager"},"native_cwd":s.data.path()}));
    let run = api(&s, "/api/scheduler/maps/create-bot-wrapper.json/runs", json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":"Isolated literal approvals only","fileInputs":[{"id":"request","path":request_path}]}), 201).await;
    api(&s, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"migration-manager","confirmed":true,"reason":"Exact v1 request reviewed","fileInputs":[{"id":"request","path":request_path}]}), 200).await;
    let dispatch = s.dispatch("migration-manager", &["create_blank"]).await;
    assert_eq!(dispatch["results"][0]["status"], "running", "{dispatch}");
    let created = reported(&s, &run, "create_blank").await;
    if fault.is_some_and(|f| f.starts_with("request-")) {
        assert_eq!(created["canComplete"], false, "{created}");
        let read = detail(&s, &run["id"]).await;
        assert_eq!(read["run"]["nodes"]["create_blank"]["execution"]["lifecycle"]["failure"]["effects"]["write"], "not_attempted");
        assert!(created["outputs"].as_array().unwrap().is_empty());
        api(&s, "/api/scheduler/bots/edit", json!({"id":"migration-result"}), 409).await;
        assert_eq!(api(&s, "/api/scheduler/bots/edit", json!({"id":"replace-text"}), 200).await, replace_before);
        assert_eq!(detail(&s, &unrelated["id"]).await, unrelated_before);
        println!("P27 refused fault={fault:?} report={created} failure={}", read["run"]["nodes"]["create_blank"]["execution"]["lifecycle"]["failure"]);
        return;
    }
    assert_eq!(created["canComplete"], true, "{created}");
    let baseline = material(&created, "baseline");
    let created_handoff = material(&created, "handoff");
    assert_eq!(created_handoff["stage"], "created");
    assert_eq!(created_handoff["bot_id"], "migration-result");
    assert_eq!(created_handoff["checks"], json!({"bot_read_back":true,"target_unchanged":true}));
    assert!(created_handoff.get("bot").is_none());
    assert!(created_handoff["target"].get("configuration").is_none());
    assert_eq!(baseline["request"], request);
    assert_eq!(baseline["target"]["configuration"], target);
    assert_eq!(baseline["bot"], json!({"id":"migration-result","name":"Literal migrated Bot","when":"","plan_guide":"","node_guide":"","task_id":target["id"]}));
    assert_eq!(detail(&s, &run["id"]).await["run"]["nodes"]["create_blank"]["status"], "running");
    complete(&s, "create_blank").await;
    let locator = json!({"type":"task","task_id":target["id"]});
    let mut final_texts = serde_json::Map::new();
    for (field, text, digest) in [("when","abc","ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),("plan_guide","hello","2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),("node_guide","","e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")] {
        let mut previous = None;
        for stage in ["write", "optimize"] {
            let node = format!("{stage}_{field}");
            let session = format!("literal-{node}");
            s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone"),"consume_named_files":true}));
            let dispatched = s.dispatch("migration-manager", &[&node]).await;
            assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
            let consumed = common::read_json_lines(&s.herdr.path().join("consumed.jsonl"));
            let consumed = &consumed.last().unwrap()["files"];
            assert_eq!(serde_json::from_str::<Value>(consumed["baseline"].as_str().unwrap()).unwrap(), baseline);
            if let Some(prior) = &previous { assert_eq!(consumed["text"], text); assert_eq!(&serde_json::from_str::<Value>(consumed["handoff"].as_str().unwrap()).unwrap(), prior); }
            let text_path = s.data.path().join(format!("{node}.txt"));
            let handoff_path = s.data.path().join(format!("{node}.json"));
            fs::write(&text_path, text).unwrap();
            let mut sources = vec![json!({"binding_id":"baseline","direction":"input","role":"baseline"}),json!({"binding_id":"text","direction":"output","role":"text"})];
            if stage == "optimize" { sources.extend([json!({"binding_id":"text","direction":"input","role":"text"}),json!({"binding_id":"handoff","direction":"input","role":"handoff"})]); }
            let handoff = json!({"schema_version":1,"kind":"handoff","stage":"guide","bot_id":"migration-result","target":locator,"field":field,"sources":sources,"confirmation":{"evidence":format!("Independent literal user confirmation of {field}: {text:?}; authorize this field save"),"text_sha256":digest,"full_text":true,"save_authorized":true,"semantics_preserved":true},"constraints":["Do not execute target"],"unverified":["Controlled drafts do not prove agent compliance"]});
            fs::write(&handoff_path, handoff.to_string()).unwrap();
            api(&s, "/api/workflow/node/report", json!({"sessionId":session,"body":"Literal fixture delivery, not agent compliance","canComplete":true,"outputs":[{"id":"text","path":text_path},{"id":"handoff","path":handoff_path}]}), 200).await;
            complete(&s, &node).await;
            previous = Some(handoff);
        }
        final_texts.insert(field.into(), json!(text));
    }
    if fault == Some("handoff-markdown") { fs::write(s.data.path().join("optimize_when.json"), "Approved. Read neighboring optimize_when.txt as the text.").unwrap(); }
    assert_eq!(s.dispatch("migration-manager", &["save_bot"]).await["results"][0]["status"], "running");
    let saved_report = reported(&s, &run, "save_bot").await;
    if fault.is_some() {
        assert_eq!(saved_report["canComplete"], false, "{saved_report}");
        let actual = api(&s, "/api/scheduler/bots/edit", json!({"id":"migration-result"}), 200).await;
        assert_eq!(actual["revision"], baseline["revision"]);
        assert_eq!(serde_json::from_str::<Value>(actual["revision"]["original"].as_str().unwrap()).unwrap(), baseline["bot"]);
        assert!(saved_report["outputs"].as_array().unwrap().is_empty());
        assert_eq!(api(&s, "/api/scheduler/bots/edit", json!({"id":"replace-text"}), 200).await, replace_before);
        assert_eq!(detail(&s, &unrelated["id"]).await, unrelated_before);
        println!("P27 refused fault={fault:?} report={saved_report}");
        return;
    }
    assert_eq!(saved_report["canComplete"], true, "{saved_report}");
    let saved = material(&saved_report, "saved");
    let saved_handoff = material(&saved_report, "handoff");
    assert_eq!(saved_handoff["stage"], "saved");
    assert_eq!(saved_handoff["bot_id"], "migration-result");
    assert_eq!(saved_handoff["checks"], json!({"bot_read_back":true,"target_unchanged":true}));
    assert!(saved_handoff.get("bot").is_none());
    assert!(saved_handoff["target"].get("configuration").is_none());
    assert_eq!(saved_handoff["sources"].as_array().unwrap().len(), 7);
    let mut expected = baseline["bot"].clone();
    expected.as_object_mut().unwrap().extend(final_texts);
    assert_eq!(saved["bot"], expected);
    assert_ne!(saved["revision"], baseline["revision"]);
    assert_eq!(saved["target"], baseline["target"]);
    let actual = api(&s, "/api/scheduler/bots/edit", json!({"id":"migration-result"}), 200).await;
    assert_eq!(serde_json::from_str::<Value>(actual["revision"]["original"].as_str().unwrap()).unwrap(), expected);
    assert_eq!(actual["revision"], saved["revision"]);
    assert_eq!(detail(&s, &run["id"]).await["run"]["nodes"]["save_bot"]["status"], "running");
    complete(&s, "save_bot").await;
    api(&s, "/api/workflow/manager/finish", json!({"sessionId":"migration-manager"}), 200).await;
    assert_eq!(detail(&s, &run["id"]).await["run"]["status"], "completed");
    assert_eq!(api(&s, "/api/scheduler/bots/edit", json!({"id":"replace-text"}), 200).await, replace_before);
    assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/tasks/{generic}")).await.body, task_before.body);
    assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{revise_file}")).await.body, revise_before.body);
    assert_eq!(detail(&s, &unrelated["id"]).await, unrelated_before);
    println!("P22/P23/P27/P28 run={} baseline={baseline} saved={saved} created_report={created} saved_report={saved_report} unrelated={unrelated_before}", run["id"]);
}
