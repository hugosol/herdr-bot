//! P22/P23/P27/P28: public shipped definitions, real HTTP Map executions and files.
//! The controlled external host reads the real inputs; it does NOT prove an
//! agent follows a prompt. Native shared-Task scenarios own that evidence.
//! Independent oracle: abc / hello / empty, without approval credentials.
//! A stale Task binding, missing direct baseline, trim/swap, implicit addressing,
//! eager complete or global replace-text migration must disagree here.
use super::*;

fn root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_owned() }
fn source(path: &str) -> Value { serde_json::from_str(&fs::read_to_string(root().join(path)).unwrap()).unwrap() }
// The old archive retains retired report-sending instructions; compare all
// generic business rules and execution fields, excluding only that role footer.
fn without_report_footer(task: &Value) -> Value {
    let mut task = task.clone();
    let marker = match task["id"].as_str().unwrap() {
        "task-18e3415003d8cd00-5" => Some("\n\n作为工作流节点时"),
        "task-18e31d34797f1464-3" => Some("workflow 按注入角色"),
        _ => None,
    };
    if let Some(marker) = marker {
        task["prompt"] = json!(task["prompt"].as_str().unwrap().split_once(marker).expect("role footer must remain explicit").0);
    }
    task
}
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
    for fault in ["request-missing", "request-markdown", "missing-binding"] {
        migrated_workflow(Some(fault)).await;
    }
}

// P17/P34/P38, approved seam: shipped definition files and public HTTP.
// Oracle: business responsibilities listed below, not the retired report switch.
// A read-only inspection incorrectly kept manual, or an unconfirmed planning
// deliverable left automatic, must disagree with these literal expectations.
#[tokio::test]
async fn published_approval_responsibilities_round_trip_every_shipped_node() {
    let s = Scenario::new().await;
    let expected = [
        ("create-bot-wrapper.json", vec![("create_blank",true),("write_when",false),("write_plan_guide",false),("write_node_guide",false),("optimize_when",true),("optimize_plan_guide",true),("optimize_node_guide",true),("save_bot",true)]),
        ("create-bot-task-workflow--bc-61ce48b80a4c.json", vec![("w1",false),("w2",true),("wrap_bot",true),("optimize_bot",false)]),
        ("create-bot-v2.json", vec![("align_requirements",false),("design_and_create",false)]),
        ("create-task-v2.json", vec![("create_task",false),("create_bot_v2",true)]),
        ("create-task-script-type.json", vec![("create_script",false),("create_bot_v2",true)]),
        ("enhance-existing-bot--bc-301b25d9f8d6.json", vec![("w1",true),("w2",false),("w3",false),("w4",false),("w5",true)]),
        ("github-repository-exploration.json", vec![("W1",true),("W2",true),("W3",true)]),
        ("plan_workflow.json", vec![("outline",false),("generate_map",true),("optimize_map",false)]),
        ("tibo-monitor-then-c-drive-inspection.json", vec![("tibo-monitor",true),("c-drive-inspection",true)]),
    ];
    let paths: Vec<_> = fs::read_dir(root().join("meta-data/workflow-maps")).unwrap().map(|p|p.unwrap().path()).filter(|p|p.extension().is_some_and(|e|e == "json")).collect();
    assert_eq!(paths.len(), expected.len(), "Every published Map needs a responsibility decision");
    for (file, nodes) in expected {
        let definition = source(&format!("meta-data/workflow-maps/{file}"));
        assert_eq!(definition["nodes"].as_object().unwrap().len(), nodes.len());
        fs::write(s.config.scheduler.map_directory.join(file), definition.to_string()).unwrap();
        let response = get_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{file}")).await;
        assert_eq!(response.status, 200, "{file}: {}", response.body);
        let read: Value = serde_json::from_str(&response.body).unwrap();
        for (id, automatic) in nodes {
            let node = &definition["nodes"][id];
            assert!(node.get("report_policy").is_none(), "{file}/{id}: retired report switch");
            assert_eq!(node["auto_approved"], automatic, "{file}/{id}: shipped responsibility");
            assert_eq!(read["nodes"][id]["auto_approved"], automatic, "{file}/{id}: public readback");
            let end_node = file == "github-repository-exploration.json" && id == "W3";
            assert_eq!(node["end_node"].as_bool().unwrap_or(false), end_node, "{file}/{id}: shipped end-node responsibility");
            assert_eq!(read["nodes"][id]["end_node"], end_node, "{file}/{id}: public end-node readback");
            assert_eq!(read["nodes"][id]["input_rule"], node["input_rule"]);
            assert_eq!(read["nodes"][id]["output_rule"], node["output_rule"]);
            println!("P34 {file}/{id} auto_approved={automatic} public={}", read["nodes"][id]);
        }
    }
}

// P31 public producer seam: V1's caller must not demand retired credentials;
// shipped Task/Bot instructions must not restore report-payload confirmation.
// Existing strict request/save HTTP+real Bun cases own the material semantics.
#[tokio::test]
async fn published_producers_do_not_require_v1_credentials_or_report_approval() {
    let s = Scenario::new().await;
    let outer = source("meta-data/workflow-maps/create-bot-task-workflow--bc-61ce48b80a4c.json");
    let producer = outer["nodes"]["w2"]["input"].as_str().unwrap();
    for retired in ["authorization={", "confirmation={", "缺创建或材料授权时阻塞", "manual 报告发送确认"] {
        assert!(!outer.to_string().contains(retired), "V1 producer still requires {retired}: {producer}");
    }
    fs::write(&s.config.tasks.storage_file, source("meta-data/tasks.json").to_string()).unwrap();
    let tasks = get_over_tcp(s.server.addr(), "/api/tasks").await;
    assert_eq!(tasks.status, 200, "{}", tasks.body);
    for retired in ["report_policy", "报告发送确认", "报告发送分别授权"] {
        assert!(!tasks.body.contains(retired), "Published Task still requires {retired}");
    }
    for path in fs::read_dir(root().join("meta-data/bots")).unwrap() {
        let path = path.unwrap().path();
        if path.extension().is_none_or(|e|e != "json") { continue; }
        let bot: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        fs::write(s.config.scheduler.bot_directory.join(path.file_name().unwrap()), bot.to_string()).unwrap();
        let read = api(&s, "/api/scheduler/bots/edit", json!({"id":bot["id"]}), 200).await;
        assert!(!read.to_string().contains("report_policy"), "{} teaches a retired report switch", bot["id"]);
        for retired in ["报告发送确认", "报告发送分别授权"] {
            assert!(!read.to_string().contains(retired), "{} restores {retired}", bot["id"]);
        }
    }
    println!("P17/P31/P38 published producer and report instructions read through HTTP");
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
        if ["meta-data/tasks.json", "meta-data/suites.json", "meta-data/mcp.json", "meta-data/bots/bot-1793348060748020300-25960-2.json", "meta-data/workflow-maps/create-bot-wrapper.json", "meta-data/workflow-maps/create-bot-task-workflow--bc-61ce48b80a4c.json"].contains(&path.as_str()) { continue; }
        // The portable archive was captured with universal-newline reads.
        // Normalize checkout line endings only, never parsed field/text bytes.
        let original = original.as_str().unwrap();
        let actual = fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n");
        if path == "meta-data/task-options.toml" {
            // The original migration archive predates these independently shipped
            // model options. Keep the archive intact and account for only that delta.
            let expected = original.replacen(
                "[[models]]\nid = 'gpt-6.1-sol'",
                concat!(
                    "[[models]]\nid = 'gpt-6.2-sol'\nlabel = 'GPT-6.2-Sol'\n",
                    "provider_model = 'openai-codex/gpt-6.2-sol'\n",
                    "thinking = { omp = ['low', 'medium', 'high', 'xhigh', 'max'], ",
                    "pi = ['minimal', 'low', 'medium', 'high', 'xhigh', 'max'] }\n",
                    "[[models]]\nid = 'gpt-6.2-astra'\nlabel = 'GPT-6.2-Astra'\n",
                    "provider_model = 'openai-codex/gpt-6.2-astra'\n",
                    "thinking = { omp = ['low', 'medium', 'high', 'xhigh', 'max'], ",
                    "pi = ['minimal', 'low', 'medium', 'high', 'xhigh', 'max'] }\n",
                    "[[models]]\nid = 'gpt-6.1-sol'",
                ),
                1,
            );
            assert_eq!(actual, expected, "Unrelated definition changed: {path}");
            continue;
        }
        if !path.ends_with(".json") { assert_eq!(actual, original, "Unrelated definition changed: {path}"); continue; }
        let mut expected: Value = serde_json::from_str(original).unwrap();
        let mut current: Value = serde_json::from_str(&actual).unwrap();
        // This archive predates the independent approval cutover. Preserve the
        // business interfaces, not obsolete report-policy-specific assertions.
        // The literal responsibility test above owns every current mode.
        if path.starts_with("meta-data/workflow-maps/") {
            for map in [&mut expected, &mut current] {
                for node in map["nodes"].as_object_mut().unwrap().values_mut() {
                    for field in ["report_policy", "auto_approved"] { node.as_object_mut().unwrap().remove(field); }
                    if path.ends_with("plan_workflow.json") {
                        node["auto_close"] = json!(true);
                        node["end_node"] = json!(false);
                    }
                }
                if path.ends_with("plan_workflow.json") { map.as_object_mut().unwrap().remove("description"); }
            }
        }
        let changed_guides: &[&str] = match path.as_str() {
            "meta-data/bots/bot-1793304246857557100-58200-0.json" => &["node_guide"],
            "meta-data/bots/bot-1793343165820110000-25960-0.json" => &["when", "plan_guide", "node_guide"],
            "meta-data/bots/create-bot-wrapper.json" => &["plan_guide", "node_guide"],
            "meta-data/bots/revise-bot-guides.json" => &["plan_guide"],
            _ => &[],
        };
        for field in changed_guides { expected.as_object_mut().unwrap().remove(*field); current.as_object_mut().unwrap().remove(*field); }
        assert_eq!(current, expected, "Unrelated definition content changed: {path}");
    }
    for original in original_tasks["tasks"].as_array().unwrap() {
        if ["task-18e33fdc55177d1c-1", "task-18e3404478db1ef8-2", "task-18e340545eb481bc-3", "task-18e340f5287462ec-4"].contains(&original["id"].as_str().unwrap()) { continue; }
        let mut expected = original.clone();
        if let Some(id) = original["suite"].as_str() {
            let agent = original["agent"].as_str().unwrap();
            expected["suite"] = json!(format!("{id}-{agent}"));
        }
        // The independently published retro preset changed models after this
        // archive; the creation migration must preserve its current preset.
        if original["id"] == "task-retro" { expected["model"] = json!("gpt-6-astra"); }
        let mut current = tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == original["id"]).unwrap().clone();
        // Prompt wording is user-maintained, not a historical release contract.
        // Runtime preservation below compares complete current Task payloads
        // before and after the workflow, including their actual prompt text.
        expected.as_object_mut().unwrap().remove("prompt");
        current.as_object_mut().unwrap().remove("prompt");
        assert_eq!(current, expected, "Unrelated Task execution configuration changed");

    }
    let old_wrapper: Value = serde_json::from_str(before["meta-data/workflow-maps/create-bot-wrapper.json"].as_str().unwrap()).unwrap();
    let new_wrapper = source("meta-data/workflow-maps/create-bot-wrapper.json");
    for (id, node) in old_wrapper["nodes"].as_object().unwrap() {
        for field in ["input_rule", "output_rule", "blocked_by"] {
            let expected = if id == "save_bot" && field == "input_rule" {
                Value::Array(node[field].as_array().unwrap().iter().filter(|rule| !rule["id"].as_str().unwrap().ends_with("_handoff")).cloned().collect())
            } else { node[field].clone() };
            assert_eq!(new_wrapper["nodes"][id][field], expected);
        }
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
    assert_eq!(without_report_footer(tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == generic).unwrap()), without_report_footer(original_tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == generic).unwrap()));
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
    let replace_before = api(&s, "/api/scheduler/bots/edit", json!({"id":"replace-text"}), 200).await;
    let task_before = get_over_tcp(s.server.addr(), &format!("/api/tasks/{generic}")).await;
    assert_eq!(task_before.status, 200);
    let creator = api(&s, "/api/scheduler/bots/edit", json!({"id":"create-blank-bot"}), 200).await;
    assert_eq!(creator["content"]["script"], "create-blank-bot.ts");
    assert!(creator["content"].get("task_id").is_none());
    let saver = api(&s, "/api/scheduler/bots/edit", json!({"id":"save-created-bot-guides"}), 200).await;
    assert_eq!(saver["content"], json!({"group":"meta-bot","name":"创建流程三指南保存者","when":"","plan_guide":"","node_guide":"","script":"save-bot-guides.ts"}));
    let mut definition = source("meta-data/workflow-maps/create-bot-wrapper.json");
    for node in definition["nodes"].as_object_mut().unwrap().values_mut() { node["zone"] = json!("node"); }
    if fault == Some("missing-binding") { definition["nodes"]["save_bot"]["input_rule"].as_array_mut().unwrap().retain(|r| r["id"] != "when_text"); }
    fs::write(s.config.scheduler.map_directory.join("create-bot-wrapper.json"), definition.to_string()).unwrap();
    let target = api(&s, "/api/tasks", json!({"name":"Literal isolated target","taskType":"normal","agent":"pi","chatName":"{chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"Read only; no execution authorized."}), 200).await;
    let target = target["tasks"].as_array().unwrap().iter().find(|t| t["name"] == "Literal isolated target").unwrap().clone();
    let request = json!({"schema_version":1,"kind":"request","target":{"type":"task","task_id":target["id"]},"bot":{"id":"migration-result","name":"Literal migrated Bot"},"requirements":"Read only invoice differences","constraints":["Do not execute target"],"non_goals":[],"resource_limits":[]});
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
    for (field, text) in [("when","abc"),("plan_guide","hello"),("node_guide","")] {
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
            let handoff = json!({"schema_version":1,"kind":"handoff","stage":"guide","bot_id":"migration-result","target":locator,"field":field,"sources":sources,"constraints":["Do not execute target"],"unverified":["Controlled drafts do not prove agent compliance"]});
            fs::write(&handoff_path, handoff.to_string()).unwrap();
            api(&s, "/api/workflow/node/report", json!({"sessionId":session,"body":"Literal fixture delivery, not agent compliance","canComplete":true,"outputs":[{"id":"text","path":text_path},{"id":"handoff","path":handoff_path}]}), 200).await;
            approve_execution(&s, &session).await;
            complete(&s, &node).await;
            previous = Some(handoff);
        }
        final_texts.insert(field.into(), json!(text));
    }
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
    assert_eq!(saved_handoff["sources"], json!([{"binding_id":"baseline","direction":"input","role":"baseline"},{"binding_id":"when_text","direction":"input","role":"text"},{"binding_id":"plan_guide_text","direction":"input","role":"text"},{"binding_id":"node_guide_text","direction":"input","role":"text"}]));
    assert!(saved_handoff.get("confirmation_evidence").is_none());
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
    compare_independent_v2(&s, &run, &saved, &request_path).await;
    println!("P22/P23/P27/P28/P35 run={} baseline={baseline} saved={saved} created_report={created} saved_report={saved_report} unrelated={unrelated_before}", run["id"]);
}

// P35 S1: keep the actual two-node V2 definition, its own request protocol and
// a separate Run. A V1 alias, cross-call, inherited approval, report-as-approval
// or material credentials treated as generic approval must fail public reads.
// Host responses only establish transport; literal files are not model output.
async fn compare_independent_v2(s: &Scenario, v1: &Value, v1_saved: &Value, v1_request: &std::path::Path) {
    let v1_before = detail(s, &v1["id"]).await;
    let v1_request_before = fs::read(v1_request).unwrap();
    let worker = api(s, "/api/scheduler/bots/edit", json!({"id":"worker"}), 200).await;
    for file in ["bot-1794846408546638600-12332-0.json", "bot-1794850150349270100-12332-3.json"] {
        let mut bot = source(&format!("meta-data/bots/{file}"));
        assert!(["align-bot-requirements-v2", "bot-task-18e89651aa593290-1"].contains(&bot["id"].as_str().unwrap()));
        bot["task_id"] = worker["content"]["task_id"].clone();
        fs::write(s.config.scheduler.bot_directory.join(file), bot.to_string()).unwrap();
    }
    let contract_dir = s.data.path().join("node-zone/docs/contracts");
    fs::create_dir_all(&contract_dir).unwrap();
    fs::copy(root().join("docs/contracts/create-bot-v2-materials.md"), contract_dir.join("create-bot-v2-materials.md")).unwrap();
    let mut map = source("meta-data/workflow-maps/create-bot-v2.json");
    assert_eq!(map["nodes"].as_object().unwrap().len(), 2);
    for node in map["nodes"].as_object_mut().unwrap().values_mut() {
        node["zone"] = json!("node");
        for rule in node["input_rule"].as_array_mut().unwrap() {
            if rule["type"] == "file" { rule["zone_id"] = json!("node"); }
        }
    }
    fs::write(s.config.scheduler.map_directory.join("create-bot-v2.json"), map.to_string()).unwrap();
    let tasks = api(s, "/api/tasks", json!({"name":"Independent V2 target","taskType":"normal","agent":"pi","chatName":"{chat.id}","model":"gpt-6-luna","thinking":"high","skill":null,"prompt":"Only report read-only CSV differences; never write invoices."}), 200).await;
    let target = tasks["tasks"].as_array().unwrap().iter().find(|t|t["name"] == "Independent V2 target").unwrap().clone();
    assert_ne!(target["id"], v1_saved["target"]["task_id"]);
    let locator = json!({"type":"task","task_id":target["id"]});
    let request = json!({"schema_version":2,"kind":"request","target":locator,"requirements":"Read-only CSV differences, independent V2 fixture","constraints":["Never write invoices"],"non_goals":["Execution"],"resource_limits":[],"authorization":{"write_materials":true,"create_bot":true,"optimize_task_prompt":false},"confirmation":{"scope":"request","evidence":"Explicit isolated S1 V2 fixture委托; no native confirmation claimed"}});
    let request_path = s.data.path().join("v2-request.json");
    fs::write(&request_path, request.to_string()).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"v2-manager"},"native_cwd":s.data.path()}));
    let run = api(s, "/api/scheduler/maps/create-bot-v2.json/runs", json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":"Independent V2 only; no V1 delegation","fileInputs":[{"id":"request","path":request_path}]}), 201).await;
    assert_ne!(run["id"], v1["id"]);
    api(s, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"v2-manager","confirmed":true,"reason":"Review actual V2 request","fileInputs":[{"id":"request","path":request_path}]}), 200).await;
    let requirements_path = s.data.path().join("v2-requirements.md");
    let requirements = "# Confirmed V2 intent\nRead-only CSV differences. Never write invoices. No execution.\nMaterials and Bot creation permitted by isolated fixture; no Task.prompt optimization.\n";
    fs::write(&requirements_path, requirements).unwrap();
    let target_materials = json!({"schema_version":2,"kind":"target_materials","target":{"type":"task","task_id":target["id"],"configuration":target},"captured_at":"2026-11-17T00:00:00Z","references":[]});
    let target_path = s.data.path().join("v2-target.json");
    fs::write(&target_path, target_materials.to_string()).unwrap();
    let mut deliveries = vec![];
    for node in ["align_requirements", "design_and_create"] {
        let session = format!("v2-{node}");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone"),"consume_named_files":true}));
        assert_eq!(s.dispatch("v2-manager", &[node]).await["results"][0]["status"], "running");
        let before = detail(s, &run["id"]).await;
        let execution = &before["run"]["nodes"][node]["execution"];
        assert_eq!(execution["autoApproved"], false);
        assert_eq!(execution["approved"], false, "V2 business authorization is not execution approval");
        let consumed = common::read_json_lines(&s.herdr.path().join("consumed.jsonl"));
        let files = &consumed.last().unwrap()["files"];
        assert_eq!(files["material_contract"], fs::read_to_string(contract_dir.join("create-bot-v2-materials.md")).unwrap());
        let outputs = if node == "align_requirements" {
            assert_eq!(serde_json::from_str::<Value>(files["request"].as_str().unwrap()).unwrap(), request);
            json!([{"id":"bot_requirements","path":requirements_path},{"id":"target_materials","path":target_path}])
        } else {
            assert_eq!(files["bot_requirements"], requirements);
            assert_eq!(serde_json::from_str::<Value>(files["target_materials"].as_str().unwrap()).unwrap(), target_materials);
            let bot = json!({"name":"Independent V2 Bot","when":"Read-only CSV comparison","plan_guide":"Arrange independent read-only comparison","node_guide":"Provide two explicitly readable CSV files","task_id":target["id"]});
            api(s, "/api/scheduler/bots/save", json!({"id":"v2-result","content":bot.to_string(),"revision":null}), 200).await;
            let read = api(s, "/api/scheduler/bots/edit", json!({"id":"v2-result"}), 200).await;
            assert_eq!(read["content"], bot);
            let mut expected_bot = bot;
            expected_bot["id"] = json!("v2-result");
            assert_eq!(serde_json::from_str::<Value>(read["revision"]["original"].as_str().unwrap()).unwrap(), expected_bot);
            let target_read = get_over_tcp(s.server.addr(), &format!("/api/tasks/{}", target["id"].as_str().unwrap())).await;
            assert_eq!(target_read.status, 200);
            assert_eq!(serde_json::from_str::<Value>(&target_read.body).unwrap()["task"], target);
            let saved = json!({"schema_version":2,"kind":"saved","bot":expected_bot,"revision":read["revision"],"target":target_materials["target"]});
            let handoff = json!({"schema_version":2,"kind":"handoff","stage":"saved","bot_id":"v2-result","target":locator,"confirmation_evidence":["Literal S1 fixture intent, complete candidate and save authorization; no native UI claim"],"sources":[{"binding_id":"bot_requirements","direction":"input","role":"requirements"},{"binding_id":"target_materials","direction":"input","role":"target_materials"}],"changes":["New Bot only; Task unchanged"],"checks":{"bot_read_back":true,"target_read_back":true,"task_prompt_changed":false,"non_target_unchanged":true,"semantics_preserved":true},"unverified":["Controlled S1 drafts; native loading and model business effects unverified"]});
            let saved_path = s.data.path().join("v2-saved.json");
            let handoff_path = s.data.path().join("v2-handoff.json");
            fs::write(&saved_path, saved.to_string()).unwrap();
            fs::write(&handoff_path, handoff.to_string()).unwrap();
            deliveries = vec![("saved",saved),("handoff",handoff)];
            json!([{"id":"saved","path":saved_path},{"id":"handoff","path":handoff_path}])
        };
        let report = api(s, "/api/workflow/node/report", json!({"sessionId":session,"body":"Independent V2 literal S1 delivery","canComplete":true,"outputs":outputs}), 200).await;
        assert_eq!(detail(s, &run["id"]).await["run"]["nodes"][node]["execution"]["approved"], false);
        let denied = api(s, "/api/workflow/manager/complete", json!({"sessionId":"v2-manager","nodeId":node,"reason":"Report alone is not approval"}), 409).await;
        assert_eq!(denied["code"], "node-not-approved");
        approve_execution(s, &session).await;
        let approved = detail(s, &run["id"]).await;
        assert_eq!(approved["run"]["nodes"][node]["execution"]["approvalSource"], "manual");
        assert_eq!(approved["run"]["nodes"][node]["status"], "running");
        assert!(approved["reports"].as_array().unwrap().contains(&report["report"]));
        if node == "design_and_create" {
            for (id, expected) in &deliveries { assert_eq!(material(&report["report"], id), *expected); }
        }
        api(s, "/api/workflow/manager/complete", json!({"sessionId":"v2-manager","nodeId":node,"reason":"Accept independent V2 real files and complete readback"}), 200).await;
    }
    api(s, "/api/workflow/manager/finish", json!({"sessionId":"v2-manager"}), 200).await;
    let v2 = detail(s, &run["id"]).await;
    assert_eq!(v2["run"]["status"], "completed");
    assert_eq!(v2["run"]["nodes"].as_object().unwrap().len(), 2);
    assert_eq!(detail(s, &v1["id"]).await, v1_before);
    assert_eq!(fs::read(v1_request).unwrap(), v1_request_before);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(request_path).unwrap()).unwrap(), request);
    println!("P35 independent_v1={v1_before} independent_v2={v2} v2_request={request} v2_deliveries={deliveries:?}");
}
