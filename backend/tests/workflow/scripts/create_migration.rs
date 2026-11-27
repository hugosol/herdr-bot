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
        ("create-bot-wrapper.json", vec![("create_blank",true),("write_when",false),("write_plan_guide",false),("write_node_guide",false),("write_output_rule",false),("optimize_when",true),("optimize_plan_guide",true),("optimize_node_guide",true),("optimize_output_rule",true),("save_bot",true)]),
        ("create-bot-task-workflow--bc-61ce48b80a4c.json", vec![("w1",false),("w2",true),("wrap_bot",true),("optimize_bot",false)]),
        ("create-bot-v2.json", vec![("align_requirements",false),("design_and_create",false)]),
        ("create-task-v2.json", vec![("create_task",false),("create_bot_v2",true)]),
        ("create-task-script-type.json", vec![("create_script",false),("create_bot_v2",true)]),
        ("enhance-existing-bot--bc-301b25d9f8d6.json", vec![("w1",true),("w2",false),("w3",false),("w4",false),("w6",false),("w5",true)]),
        ("github-repository-exploration.json", vec![("W1",true),("W2",true),("W3",true)]),
        ("plan_workflow.json", vec![("outline",false),("generate_map",true),("optimize_map",false)]),
        ("tibo-monitor-then-c-drive-inspection.json", vec![("tibo-monitor",true),("c-drive-inspection",true)]),
    ];
    let paths: Vec<_> = fs::read_dir(root().join("meta-data/workflow-maps")).unwrap().map(|p|p.unwrap().path()).filter(|p|p.extension().is_some_and(|e|e == "json")).collect();
    assert_eq!(paths.len(), expected.len(), "Every published Map needs a responsibility decision");
    for (file, nodes) in expected {
        let definition = source(&format!("meta-data/workflow-maps/{file}"));
        assert_eq!(definition["nodes"].as_object().unwrap().len(), nodes.len());
        // Test the node contract under a legal, test-owned Map envelope. A
        // published node's responsibility is checked below against the original
        // asset; this does not treat unselected Maps as migrated definitions.
        let fixture = json!({"title":"Approval responsibility fixture","description":"","nodes":definition["nodes"],"map_input_rule":{"files":[
            {"id":"request","required":false}, {"id":"materials","required":false},
            {"id":"clone_request","required":false}
        ]}});
        fs::write(s.config.scheduler.map_directory.join(file), fixture.to_string()).unwrap();
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
        if bot.get("output_rule").is_none() {
            // Definitions outside the authoring cutover are deliberately not
            // installed for execution; their historical wording is still checked.
            for retired in ["report_policy", "报告发送确认", "报告发送分别授权"] {
                assert!(!bot.to_string().contains(retired), "{} restores {retired}", bot["id"]);
            }
            continue;
        }
        fs::write(s.config.scheduler.bot_directory.join(path.file_name().unwrap()), bot.to_string()).unwrap();
        let read = api(&s, "/api/scheduler/bots/edit", json!({"id":bot["id"]}), 200).await;
        assert!(!read.to_string().contains("report_policy"), "{} teaches a retired report switch", bot["id"]);
        for retired in ["报告发送确认", "报告发送分别授权"] {
            assert!(!read.to_string().contains(retired), "{} restores {retired}", bot["id"]);
        }
    }
    println!("P17/P31/P38 published producer and report instructions read through HTTP");
}

fn rename_archived_source_refs(map: &mut Value) {
    for node in map["nodes"].as_object_mut().unwrap().values_mut() {
        let Some(rules) = node.get_mut("input_rule").and_then(Value::as_array_mut) else { continue; };
        for rule in rules {
            let fields: &[(&str, &str)] = match rule["type"].as_str() {
                Some("run") => &[("input_id", "run_file_ref")],
                Some("node") => &[("node_id", "source_node_ref"), ("output_id", "node_output_ref")],
                _ => continue,
            };
            let rule = rule.as_object_mut().unwrap();
            for &(old, new) in fields {
                let value = rule.remove(old).unwrap();
                rule.insert(new.into(), value);
            }
        }
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
            let expected = expected.replacen(
                "native_name = 'retro'\n",
                concat!("native_name = 'retro'\n", "[[skills]]\nid = 'design-taste-frontend'\nlabel = '/design-taste-frontend'\nnative_name = 'design-taste-frontend'\n"),
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
            // Update the comparison expectation without rewriting the historical archive.
            rename_archived_source_refs(&mut expected);
            for map in [&mut expected, &mut current] {
                for node in map["nodes"].as_object_mut().unwrap().values_mut() {
                    for field in ["report_policy", "auto_approved"] { node.as_object_mut().unwrap().remove(field); }
                    if path.ends_with("plan_workflow.json") {
                        node["auto_close"] = json!(true);
                        node["end_node"] = json!(false);
                        // The selected-flow release owns the new phase instructions;
                        // preserve every other field in this unrelated history comparison.
                        node.as_object_mut().unwrap().remove("input");
                    }
                }
                if path.ends_with("plan_workflow.json") { map.as_object_mut().unwrap().remove("description"); }
            }
            if path.ends_with("plan_workflow.json") {
                expected.as_object_mut().unwrap().remove("input_requirements");
            }
        }
        if path.starts_with("meta-data/bots/") && current.get("output_rule").is_some() {
            // Direct authoring consumers have intentionally redistributed their old
            // text. The historical archive still checks every unaffected identity,
            // group, binding and non-index field rather than blessing a new snapshot.
            for field in ["when", "plan_guide", "node_guide"] { expected.as_object_mut().unwrap().remove(field); }
            for field in ["when", "description", "input_rule", "output_rule"] { current.as_object_mut().unwrap().remove(field); }
            if matches!(current["id"].as_str(), Some("write-bot-plan-guide" | "write-bot-node-guide" | "revise-bot-guides")) {
                let display = match current["id"].as_str().unwrap() {
                    "write-bot-plan-guide" => "Bot.description 编写者",
                    "write-bot-node-guide" => "Bot.input_rule 编写者",
                    _ => "独立修订既有 Bot 能力摘要",
                };
                expected["name"] = json!(display);
            }
        }
        if path.ends_with("bot-1793343165820110000-25960-0.json") {
            for bot in [&mut expected, &mut current] { for field in ["when", "plan_guide", "node_guide"] { bot.as_object_mut().unwrap().remove(field); } }
        }
        if path.ends_with("enhance-existing-bot--bc-301b25d9f8d6.json") {
            // The independent revision graph test checks its four-lane wiring.
            for map in [&mut expected, &mut current] { for field in ["title", "description", "nodes"] { map.as_object_mut().unwrap().remove(field); } }
            let old = expected.as_object_mut().unwrap();
            old.remove("input_files");
            old.remove("input_requirements");
            current.as_object_mut().unwrap().remove("map_input_rule");
        }
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
    // The public definition test independently verifies every current writer,
    // optimizer and save binding. This historical check protects the unchanged
    // entry and terminal material interface across the field cutover.
    let mut old_wrapper: Value = serde_json::from_str(before["meta-data/workflow-maps/create-bot-wrapper.json"].as_str().unwrap()).unwrap();
    rename_archived_source_refs(&mut old_wrapper);
    let new_wrapper = source("meta-data/workflow-maps/create-bot-wrapper.json");
    for (id, fields) in [("create_blank", &["bot_id", "input_rule", "output_rule", "blocked_by"][..]), ("save_bot", &["output_rule"][..])] {
        for field in fields { assert_eq!(new_wrapper["nodes"][id][field], old_wrapper["nodes"][id][field]); }
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
    // Only the deployed authoring definitions are online. Keep unselected
    // old-format definitions unchanged in the isolated offline archive.
    for file in ["create-bot-wrapper.json", "enhance-existing-bot--bc-301b25d9f8d6.json"] {
        fs::copy(root().join("meta-data/workflow-maps").join(file), s.config.scheduler.map_directory.join(file)).unwrap();
    }
    let worker = api(&s, "/api/scheduler/bots/edit", json!({"id":"worker"}), 200).await;
    let backup = s.data.path().join("offline-bots");
    fs::create_dir(&backup).unwrap();
    for path in fs::read_dir(root().join("meta-data/bots")).unwrap() {
        let path = path.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") { continue; }
        let mut bot: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        if !matches!(bot["id"].as_str(), Some("write-bot-when" | "write-bot-plan-guide" | "write-bot-node-guide" | "write-bot-output-rule" | "optimize-prompt" | "create-blank-bot" | "save-created-bot-guides" | "replace-text" | "create-bot-wrapper" | "prepare-bot-revision" | "revise-bot-guides")) {
            fs::copy(&path, backup.join(path.file_name().unwrap())).unwrap();
            continue;
        }
        if ["write-bot-when","write-bot-plan-guide","write-bot-node-guide","write-bot-output-rule","optimize-prompt"].contains(&bot["id"].as_str().unwrap()) { bot["task_id"] = worker["content"]["task_id"].clone(); }
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
    assert_eq!(saver["content"], json!({"group":"meta-bot","name":"创建流程四字段保存者","when":"","description":"","input_rule":"","output_rule":"","script":"save-bot-guides.ts"}));
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
    assert_eq!(baseline["bot"], json!({"id":"migration-result","name":"Literal migrated Bot","when":"","description":"","input_rule":"","output_rule":"","task_id":target["id"]}));
    assert_eq!(detail(&s, &run["id"]).await["run"]["nodes"]["create_blank"]["status"], "running");
    complete(&s, "create_blank").await;
    let locator = json!({"type":"task","task_id":target["id"]});
    let mut final_texts = serde_json::Map::new();
    for (field, suffix, text) in [("when","when","abc"),("description","plan_guide","hello"),("input_rule","node_guide",""),("output_rule","output_rule","Read-only report")] {
        let mut previous = None;
        for stage in ["write", "optimize"] {
            let node = format!("{stage}_{suffix}");
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
    assert_eq!(saved_handoff["sources"], json!([{"binding_id":"baseline","direction":"input","role":"baseline"},{"binding_id":"when_text","direction":"input","role":"text"},{"binding_id":"description_text","direction":"input","role":"text"},{"binding_id":"input_rule_text","direction":"input","role":"text"},{"binding_id":"output_rule_text","direction":"input","role":"text"}]));
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

// P35 S1: V2 uses its own request and public identity output. Controlled
// transport proves file delivery and explicit approval, not model quality.
async fn compare_independent_v2(s: &Scenario, v1: &Value, v1_saved: &Value, v1_request: &std::path::Path) {
    let v1_before = detail(s, &v1["id"]).await;
    let v1_request_before = fs::read(v1_request).unwrap();
    let v2 = selected_v2_workflow(s).await;
    let created = api(s, "/api/scheduler/bots/edit", json!({"id":"v2-result"}), 200).await;
    assert_ne!(created["content"]["task_id"], v1_saved["target"]["task_id"]);
    assert_eq!(detail(s, &v1["id"]).await, v1_before);
    assert_eq!(fs::read(v1_request).unwrap(), v1_request_before);
    println!("P35 independent_v1={v1_before} independent_v2={v2}");
}

#[tokio::test]
async fn selected_v2_delivers_request_to_both_nodes_and_returns_saved_bot_identity() {
    let s = Scenario::new().await;
    selected_v2_workflow(&s).await;
}

async fn install_selected_v2(s: &Scenario, bot_files: &[&str]) {
    fs::write(&s.config.prompts_file, format!("herdr-bot-path = '{}'\n", root().display())).unwrap();
    let worker = api(s, "/api/scheduler/bots/edit", json!({"id":"worker"}), 200).await;
    for file in ["bot-1794846408546638600-12332-0.json", "bot-1794850150349270100-12332-3.json"].into_iter().chain(bot_files.iter().copied()) {
        let mut bot = source(&format!("meta-data/bots/{file}"));
        if bot.get("task_id").is_some() {
            bot["task_id"] = worker["content"]["task_id"].clone();
        }
        fs::write(s.config.scheduler.bot_directory.join(file), bot.to_string()).unwrap();
    }
    let mut map = source("meta-data/workflow-maps/create-bot-v2.json");
    for node in map["nodes"].as_object_mut().unwrap().values_mut() {
        node["zone"] = json!("node");
    }
    fs::write(s.config.scheduler.map_directory.join("create-bot-v2.json"), map.to_string()).unwrap();
}

async fn create_v2_target(s: &Scenario) -> Value {
    let tasks = api(s, "/api/tasks", json!({"name":"Independent V2 target","taskType":"normal","agent":"pi","chatName":"{chat.id}","model":"gpt-6-luna","thinking":"high","skill":null,"prompt":"Only report read-only CSV differences; never write invoices."}), 200).await;
    let target = tasks["tasks"].as_array().unwrap().iter().find(|t|t["name"] == "Independent V2 target").unwrap();
    let read = get_over_tcp(s.server.addr(), &format!("/api/tasks/{}", target["id"].as_str().unwrap())).await;
    assert_eq!(read.status, 200, "{}", read.body);
    let reread = serde_json::from_str::<Value>(&read.body).unwrap()["task"].clone();
    assert_eq!(reread, *target);
    reread
}

async fn selected_v2_workflow(s: &Scenario) -> Value {
    install_selected_v2(s, &[]).await;
    let target = create_v2_target(s).await;
    let request_path = s.data.path().join("v2-request.json");
    fs::write(&request_path, json!({"type":"task","ref":target["id"]}).to_string()).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"v2-manager"},"native_cwd":s.data.path()}));
    let input = "Read-only CSV comparison; preserve the target Task and do not run it.";
    let run = api(s, "/api/scheduler/maps/create-bot-v2.json/runs", json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":input,"fileInputs":[{"id":"request","path":request_path}]}), 201).await;
    run_v2_consumers(s, &run, &target, &request_path, input).await
}

async fn run_v2_consumers(s: &Scenario, run: &Value, target: &Value, request_path: &std::path::Path, input: &str) -> Value {
    let request = json!({"type":"task","ref":target["id"]});
    api(s, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"v2-manager","confirmed":true,"reason":"Review actual V2 target","fileInputs":[{"id":"request","path":request_path}]}), 200).await;
    let requirements_path = s.data.path().join("v2-requirements.md");
    let requirements = "# Confirmed V2 intent\nRead-only CSV differences. Never write invoices. No execution.\nBot creation permitted by isolated fixture; no Task.prompt optimization.\n";
    fs::write(&requirements_path, requirements).unwrap();
    let identity_path = s.data.path().join("created-bot.json");
    let identity = json!({"bot_id":"v2-result"});
    for node in ["align_requirements", "design_and_create"] {
        let session = format!("v2-{node}");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone"),"consume_named_files":true}));
        assert_eq!(s.dispatch("v2-manager", &[node]).await["results"][0]["status"], "running");
        let calls = s.herdr.calls();
        let prompt = common::prompt_body(calls.iter().rfind(|call| call["kind"] == "agent_prompt").unwrap());
        assert!(prompt.contains(&format!("[Run background / runInput]\n{input}\n[/Run background / runInput]")), "{prompt}");
        let before = detail(s, &run["id"]).await;
        let execution = &before["run"]["nodes"][node]["execution"];
        assert_eq!(execution["autoApproved"], false);
        assert_eq!(execution["approved"], false);
        let consumed = common::read_json_lines(&s.herdr.path().join("consumed.jsonl"));
        let files = &consumed.last().unwrap()["files"];
        let expected_inputs = if node == "align_requirements" {
            json!({"request":request.to_string()})
        } else {
            json!({"request":request.to_string(),"bot_requirements":requirements})
        };
        assert_eq!(*files, expected_inputs, "Each consumer receives its actual complete file inputs");
        let outputs = if node == "align_requirements" {
            json!([{"id":"bot_requirements","path":requirements_path}])
        } else {
            let bot = json!({"name":"Independent V2 Bot","when":"Read-only CSV comparison","description":"Arrange independent read-only comparison","input_rule":"Provide two explicitly readable CSV files","output_rule":"","task_id":target["id"]});
            api(s, "/api/scheduler/bots/save", json!({"id":"v2-result","content":bot.to_string(),"revision":null}), 200).await;
            let read = api(s, "/api/scheduler/bots/edit", json!({"id":"v2-result"}), 200).await;
            assert_eq!(read["content"], bot);
            let mut expected_bot = bot;
            expected_bot["id"] = json!("v2-result");
            assert_eq!(serde_json::from_str::<Value>(read["revision"]["original"].as_str().unwrap()).unwrap(), expected_bot);
            let target_read = get_over_tcp(s.server.addr(), &format!("/api/tasks/{}", target["id"].as_str().unwrap())).await;
            assert_eq!(target_read.status, 200);
            assert_eq!(serde_json::from_str::<Value>(&target_read.body).unwrap()["task"], *target);
            fs::write(&identity_path, identity.to_string()).unwrap();
            json!([{"id":"created_bot","path":identity_path}])
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
            assert_eq!(material(&report["report"], "created_bot"), identity);
        }
        api(s, "/api/workflow/manager/complete", json!({"sessionId":"v2-manager","nodeId":node,"reason":"Accept V2 actual files and complete readback"}), 200).await;
    }
    api(s, "/api/workflow/manager/finish", json!({"sessionId":"v2-manager"}), 409).await;
    assert_eq!(detail(s, &run["id"]).await["run"]["status"], "running");
    api(s, "/api/workflow/manager/finish", json!({"sessionId":"v2-manager","mapOutput":{"files":[{"id":"created_bot","path":identity_path}]}}), 200).await;
    let v2 = detail(s, &run["id"]).await;
    assert_eq!(v2["run"]["status"], "completed");
    assert_eq!(v2["run"]["mapOutput"]["files"], json!([{"id":"created_bot","path":identity_path}]));
    let delivered: Value = serde_json::from_slice(&fs::read(v2["run"]["mapOutput"]["files"][0]["path"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(delivered, identity);
    let located = api(s, "/api/scheduler/bots/edit", json!({"id":delivered["bot_id"]}), 200).await;
    assert_eq!(located["content"]["task_id"], target["id"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(request_path).unwrap()).unwrap(), request);
    v2
}

// The shipped caller supplies a Task locator, not a replacement requirements
// file. Public Run state and real named-file consumers must retain both inputs.
#[tokio::test]
async fn create_migration_shipped_task_v2_delivers_created_task_and_nested_bot_identity() {
    let s = Scenario::new().await;
    install_selected_v2(&s, &["bot-1793309187825217800-58200-2.json", "bot-1794855295140784100-12332-6.json"]).await;
    let mut map = source("meta-data/workflow-maps/create-task-v2.json");
    for node in map["nodes"].as_object_mut().unwrap().values_mut() {
        node["zone"] = json!("node");
    }
    map["nodes"]["create_bot_v2"]["manager_task_id"] = json!(s.manager_task);
    fs::write(s.config.scheduler.map_directory.join("create-task-v2.json"), map.to_string()).unwrap();

    let input = " \tRead-only CSV comparison; deliver a Task and its Bot without running either.\r\nNever write invoices.  ";
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"task-v2-manager"},"native_cwd":s.data.path()}));
    let root = api(&s, "/api/scheduler/maps/create-task-v2.json/runs", json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":input}), 201).await;
    input_resolution::identity(&s, "task-v2-author");
    let launched = s.dispatch("task-v2-manager", &["create_task"]).await;
    assert_eq!(launched["results"][0]["status"], "running", "{launched}");
    assert_eq!(launched["snapshot"]["nodes"]["create_task"]["execution"]["inputs"], json!([]));
    assert_eq!(launched["snapshot"]["nodes"]["create_task"]["execution"]["autoApproved"], false);

    let target = create_v2_target(&s).await;
    let task_identity = json!({"task_id":target["id"]});
    let task_path = s.data.path().join("created-task.json");
    let request_path = s.data.path().join("v2-request.json");
    fs::write(&task_path, task_identity.to_string()).unwrap();
    fs::write(&request_path, json!({"type":"task","ref":task_identity["task_id"]}).to_string()).unwrap();
    approve_execution(&s, "task-v2-author").await;
    let produced = api(&s, "/api/workflow/node/report", json!({"sessionId":"task-v2-author","body":"Saved the Task and read back its actual identity","canComplete":true,"outputs":[{"id":"created_task","path":task_path},{"id":"request","path":request_path}]}), 200).await;
    assert_eq!(material(&produced["report"], "created_task"), task_identity);
    api(&s, "/api/workflow/manager/complete", json!({"sessionId":"task-v2-manager","nodeId":"create_task","reason":"Accept verified Task and its request"}), 200).await;

    input_resolution::identity(&s, "v2-manager");
    let nested = s.dispatch("task-v2-manager", &["create_bot_v2"]).await;
    assert_eq!(nested["results"][0]["status"], "running", "{nested}");
    let execution = &nested["snapshot"]["nodes"]["create_bot_v2"]["execution"];
    let child = detail(&s, &execution["childRunId"]).await;
    assert_eq!(child["run"]["parent"], json!({"runId":root["id"],"nodeId":"create_bot_v2","executionId":execution["id"]}));
    assert_eq!(child["run"]["mapInput"], json!({"runInput":input,"files":[{"id":"request","path":request_path}]}));
    let finished_child = run_v2_consumers(&s, &child["run"], &target, &request_path, input).await;
    let bot_output = finished_child["run"]["mapOutput"]["files"][0].clone();
    let waiting = detail(&s, &root["id"]).await;
    assert_eq!(waiting["run"]["nodes"]["create_bot_v2"]["status"], "running");
    assert_eq!(waiting["reports"], json!([produced["report"]]));

    approve_execution(&s, "v2-manager").await;
    let delivered = api(&s, "/api/workflow/node/report", json!({"sessionId":"v2-manager","body":"Deliver the finished child Run’s Bot","canComplete":true}), 200).await;
    assert_eq!(delivered["report"]["outputs"], json!([bot_output]));
    assert_eq!(material(&delivered["report"], "created_bot"), json!({"bot_id":"v2-result"}));
    api(&s, "/api/workflow/manager/complete", json!({"sessionId":"task-v2-manager","nodeId":"create_bot_v2","reason":"Accept the actual nested Bot"}), 200).await;
    let task_output = json!({"id":"created_task","path":task_path});
    api(&s, "/api/workflow/manager/finish", json!({"sessionId":"task-v2-manager","mapOutput":{"files":[task_output]}}), 409).await;
    let outputs = json!([task_output, bot_output]);
    api(&s, "/api/workflow/manager/finish", json!({"sessionId":"task-v2-manager","mapOutput":{"files":outputs}}), 200).await;
    let finished = detail(&s, &root["id"]).await;
    assert_eq!(finished["run"]["status"], "completed");
    assert_eq!(finished["run"]["mapOutput"]["files"], outputs);
}
