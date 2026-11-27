//! Approved S1/S4: the selected release definitions through HTTP and actual files.
use super::*;

const MAP: &str = "plan_workflow.json";
const MATERIAL: &str = "meta-data/workflow-maps/plan_workflow/planning-deliverables.md";
const BOTS: [(&str, &str, &str, &str, &str); 4] = [
    ("bot-1794388558375394800-31524-0.json", "bot-map-plan-workflow", "创建map", "map_file", MAP),
    ("bot-1793304246857557100-58200-0.json", "outline-planning", "概要规划", "task_id", "task-18e318b13b71a8bc-0"),
    ("bot-1793305512701853300-58200-1.json", "plan-to-map", "概要方案转 Map", "task_id", "task-18e319d73777fc6c-1"),
    ("bot-1794222010819446100-10104-0.json", "bot-task-18e65aed740825f8-0", "prompt优化者——Map维度", "task_id", "task-18e65aed740825f8-0"),
];

fn repo() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

async fn read(s: &Scenario, route: &str) -> Value {
    let r = get_over_tcp(s.server.addr(), route).await;
    assert_eq!(r.status, 200, "{route}: {}", r.body);
    serde_json::from_str(&r.body).unwrap()
}

async fn post(s: &Scenario, route: &str, data: Value, status: u16) -> Value {
    let r = post_json_over_tcp(s.server.addr(), route, &data.to_string()).await;
    assert_eq!(r.status, status, "{route}: {}", r.body);
    serde_json::from_str(&r.body).unwrap()
}

fn install(s: &Scenario) -> Vec<u8> {
    // No substitutes for the shipped definitions or their actual bytes.
    for (file, ..) in BOTS {
        fs::copy(repo().join("meta-data/bots").join(file), s.config.scheduler.bot_directory.join(file)).unwrap();
    }
    fs::copy(repo().join("meta-data/tasks.json"), &s.config.tasks.storage_file).unwrap();
    let bytes = fs::read(repo().join("meta-data/workflow-maps").join(MAP)).unwrap();
    fs::write(s.config.scheduler.map_directory.join(MAP), &bytes).unwrap();
    bytes
}

// P33/P40–P46: the approved phase graph has no public file interface; mixing
// an old candidate into the release, changing a binding or dropping a handoff
// must disagree with service readback and the independent stable identities.
#[tokio::test]
async fn selected_flow_publishes_approved_graph_and_shared_knowledge_without_starting_work() {
    let s = Scenario::new().await;
    let bytes = install(&s);
    let file: Value = serde_json::from_slice(&bytes).unwrap();
    let map = read(&s, "/api/scheduler/maps/plan_workflow.json").await;
    assert_eq!(map["title"], "创建map");
    assert_eq!(map["group"], "meta-map");
    assert_eq!(map["description"], "将业务需求转化为新业务 Map，并完成关联提示词的分层优化；交付系统中已保存的配置，不启动生成的业务流程。");
    for key in ["map_input_rule", "map_output_rule", "input_requirements", "input_files", "materials"] {
        assert!(file.get(key).is_none(), "unexpected static field: {key}");
        assert!(map.get(key).is_none() || map[key].is_null(), "unexpected public rule: {key}");
    }
    let nodes = [
        ("outline", "概要规划与用户确认", "outline-planning", json!([]), json!([]), json!([{"id":"outline"}]), false, "概要规划"),
        ("generate_map", "概要方案转换为业务 Map", "plan-to-map", json!(["outline"]), json!([{"id":"outline","type":"node","node_id":"outline","output_id":"outline"}]), json!([{"id":"map"}]), true, "Map 转换"),
        ("optimize_map", "prompt优化者——Map维度", "bot-task-18e65aed740825f8-0", json!(["generate_map"]), json!([{"id":"map","type":"node","node_id":"generate_map","output_id":"map"},{"id":"outline","type":"node","node_id":"outline","output_id":"outline"}]), json!([{"id":"map"},{"id":"handoff"}]), false, "整图优化"),
    ];
    assert_eq!(map["nodes"].as_object().unwrap().len(), 3);
    for (id, title, bot, deps, inputs, outputs, approved, phase) in nodes {
        let node = &map["nodes"][id];
        assert_eq!(node["title"], title);
        assert_eq!(node["bot_id"], bot);
        assert_eq!(node["zone"], "hugosol/herdr-bot");
        assert_eq!(node["blocked_by"], deps);
        assert_eq!(node["input_rule"], inputs);
        assert_eq!(node["output_rule"], outputs);
        assert_eq!(node["auto_approved"], approved);
        assert_eq!(node["auto_close"], true);
        assert_eq!(node["end_node"], false);
        let conditional = format!("交付本阶段成果前，读取 ${{herdr-bot-path}}/{MATERIAL}「创建 Map 的整图交接」中的「{phase}」，核对本节点成果是否满足交接条件。");
        let expected = if id == "optimize_map" {
            format!("依据 outline 优化 map 对应的服务配置。允许修改目标 Map、关联 Task、Bot、子 Map 及共同文档中的提示词；共享对象的具体修改范围由用户在本 Chat 中确认。{conditional}")
        } else { conditional };
        assert_eq!(node["input"], expected);
    }
    for (_, id, name, target, binding) in BOTS {
        let bot = post(&s, "/api/scheduler/bots/edit", json!({"id":id}), 200).await;
        let content = &bot["content"];
        assert_eq!(content["name"], name);
        assert_eq!(content[target], binding);
        if target == "task_id" { assert_eq!(content["group"], "meta-bot"); }
        else { assert!(content["group"].is_null()); }
        for field in ["when", "description", "input_rule", "output_rule"] {
            assert!(!content[field].as_str().unwrap().is_empty(), "{id}: {field}");
        }
        assert!(content.get("plan_guide").is_none() && content.get("node_guide").is_none());
    }
    let tasks = read(&s, "/api/tasks").await;
    for (id, name, chat, skill) in [
        ("task-18e318b13b71a8bc-0", "概要规划", "[概要规划] {zone.name} · {chat.id}", json!("grill-with-docs")),
        ("task-18e319d73777fc6c-1", "概要方案转 Map", "[Plan to Map] {zone.name} · {chat.id}", Value::Null),
        ("task-18e65aed740825f8-0", "prompt优化者——Map维度", "[Map 提示词优化] {zone.name} · {chat.id}", json!("writing-for-agents")),
    ] {
        let mut row = tasks["tasks"].as_array().unwrap().iter().find(|t| t["id"] == id).unwrap().clone();
        let prompt = row.as_object_mut().unwrap().remove("prompt").unwrap();
        assert_eq!(row, json!({"id":id,"name":name,"group":"meta-task","chatName":chat,"agent":"omp","taskType":"normal","model":"gpt-6-astra","thinking":"medium","skill":skill,"suite":"codebase-exploration-omp"}));
        assert!(prompt.as_str().unwrap().contains(MATERIAL));
        assert!(!prompt.as_str().unwrap().contains("trip-readiness.json"));
    }
    let document = fs::read_to_string(repo().join(MATERIAL)).unwrap();
    for section in ["共同身份", "共同写入规则", "规划文档保存与交接", "创建 Map 的整图交接", "能力建设需求", "概要方案", "能力选用与调用", "保存与证据边界"] {
        assert!(document.lines().any(|line| line == format!("## {section}")), "{section}");
    }
    assert!(!repo().join("docs/planning-deliverables.md").exists());
    assert_eq!(read(&s, "/api/workflow/runs").await["runs"], json!([]));
    assert!(s.herdr.calls().is_empty(), "reading definitions is not starting the generated business");
    assert_eq!(fs::read(s.config.scheduler.map_directory.join(MAP)).unwrap(), bytes);
}

const MANAGER: &str = "selected-manager";
const ZONE: &str = "hugosol/herdr-bot";

fn identity(s: &Scenario, session: &str, manager: bool) {
    let cwd = if manager { s.data.path().to_owned() } else { s.data.path().join("node-zone") };
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":cwd}));
}

fn submissions(s: &Scenario) -> Vec<Value> {
    common::read_json_lines(&s.herdr.path().join("submitted.jsonl"))
}

// P33/P41/P42/P44/P46/P53/P58 at S1/S3/S4. The selected release is the only
// Map online; same-name legacy bytes stay offline. Run it with a deliberately
// non-parseable inventory, then compare standalone/node/manager submissions
// and the final reported file with the effective optimized service config.
// The controlled host proves delivery, not agent reading or model decisions.
#[tokio::test]
async fn selected_flow_executes_by_stable_identity_without_inventory_gate() {
    use sha2::{Digest, Sha256};
    let mut s = Scenario::new().await;
    fs::remove_file(s.config.scheduler.map_directory.join("work.json")).unwrap();
    fs::remove_file(s.config.scheduler.bot_directory.join("worker.json")).unwrap();
    let backup = s.data.path().join("offline-backup");
    fs::create_dir(&backup).unwrap();
    let old_map = br#"{"title":"Legacy selected flow","input_requirements":"old","nodes":{}}"#;
    let old_bot = br#"{"id":"bot-map-plan-workflow","name":"Legacy selected Bot","when":"old","plan_guide":"old","node_guide":"old","map_file":"plan_workflow.json"}"#;
    let old_map_path = backup.join(MAP);
    let old_bot_path = backup.join(BOTS[0].0);
    let pending = backup.join("not-selected.json");
    let pending_bytes = br#"{"title":"Unselected legacy Map","input_requirements":"unchanged","nodes":{}}"#;
    fs::write(&old_map_path, old_map).unwrap();
    fs::write(&old_bot_path, old_bot).unwrap();
    fs::write(&pending, pending_bytes).unwrap();
    let inventory = s.data.path().join("docs/workflow-migration-inventory.md");
    fs::create_dir(inventory.parent().unwrap()).unwrap();
    let inventory_bytes = b"# Conflict marker: not approved, all selected Maps pending\r\n| malformed";
    fs::write(&inventory, inventory_bytes).unwrap();
    let selected = install(&s);
    let reference = s.data.path().join(MATERIAL);
    fs::create_dir_all(reference.parent().unwrap()).unwrap();
    fs::copy(repo().join(MATERIAL), &reference).unwrap();
    let root = s.data.path().to_string_lossy().replace('\\', "/");
    fs::write(&s.config.prompts_file, format!("herdr-bot-path = '{root}'\nherdr-bot-api-url = 'http://127.0.0.1'\n")).unwrap();

    // A controlled suite supplies the original suite ID to the recording host;
    // it does not substitute the shipped Task, model selection or skill text.
    s.config.base_directory = s.data.path().to_owned();
    s.config.suites.definition_file = s.data.path().join("suites.json");
    s.config.suites.generated_directory = s.data.path().join("generated-suites");
    fs::write(&s.config.suites.definition_file, r#"{"suites":[{"id":"codebase-exploration-omp","name":"Controlled local suite · OMP","harness":"omp"}]}"#).unwrap();
    s.server = TestServer::start(&s.config).await;
    s.herdr.set_workspaces(json!([
        {"workspace_id":"w-manager","label":s.data.path().file_name().unwrap().to_str().unwrap()},
        {"workspace_id":"w-default","label":"bot-playground"}
    ]));
    let zone = s.data.path().join("node-zone");
    post(&s, "/api/zones", json!({"id":ZONE,"path":zone}), 200).await;
    assert_eq!(read(&s, "/api/scheduler/maps").await["files"], json!([MAP]));
    let listed = read(&s, "/api/scheduler/bots").await;
    let ids = listed["bots"].as_array().unwrap();
    assert_eq!(ids.len(), 4);
    assert!(ids.iter().any(|bot| bot["id"] == "bot-map-plan-workflow" && bot["map_file"] == MAP && bot["name"] == "创建map"));
    let tasks = read(&s, "/api/tasks").await;
    let material_pointer = format!("{root}/{MATERIAL}");
    let phases = [
        ("outline", "概要规划", "grill-with-docs", "draft-standalone-outline"),
        ("generate_map", "Map 转换", "", "draft-standalone-generate"),
        ("optimize_map", "整图优化", "writing-for-agents", "draft-standalone-optimize"),
    ];
    for ((_, phase, skill, session), (_, _, _, _, task)) in phases.into_iter().zip(BOTS.into_iter().skip(1)) {
        identity(&s, session, false);
        let before = submissions(&s).len();
        let preview = post(&s, "/api/chats/preview", json!({"zone":ZONE,"taskId":task}), 200).await;
        let request = format!("Independent input for {phase}");
        let chat = post(&s, "/api/chats", json!({"zone":ZONE,"taskId":task,"chatId":preview["chatId"],"input":request}), 202).await;
        let route = format!("/api/chats/{}", chat["id"].as_str().unwrap());
        tokio::time::timeout(Duration::from_secs(6), async {
            loop {
                let state = read(&s, &route).await;
                assert_ne!(state["stage"], "failed", "{state}");
                if state["stage"] == "succeeded" { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        let received = submissions(&s);
        assert_eq!(received.len(), before + 1);
        let prompt = common::prompt_body(&received[before]);
        assert!(prompt.contains(&material_pointer) && prompt.ends_with(&request), "{prompt}");
        assert!(!prompt.contains("创建 Map 的整图交接"), "the standalone Task does not get phase-specific instructions");
        assert_eq!(prompt.starts_with("/skill:"), !skill.is_empty());
        if !skill.is_empty() { assert!(prompt.starts_with(&format!("/skill:{skill} "))); }
    }

    let before = submissions(&s).len();
    identity(&s, MANAGER, true);
    let input = "Fixture only: prepare an inventory note; do not run the generated Map.";
    let manager_task = "task-18e2c2827c484128-0";
    assert!(tasks["tasks"].as_array().unwrap().iter().any(|task| task["id"] == manager_task && task["taskType"] == "manager"));
    let start = post(&s, "/api/scheduler/maps/plan_workflow.json/runs", json!({"managerTaskId":manager_task,"zone":"manager","userInput":input}), 201).await;
    let run = format!("/api/workflow/runs/{}", start["id"].as_str().unwrap());
    let current = read(&s, &run).await;
    assert_eq!(current["run"]["mapInput"], json!({"runInput":input,"files":[]}));
    assert_eq!(current["run"]["map"], MAP);
    let received = submissions(&s);
    assert_eq!(received.len(), before + 1);
    let manager_prompt = common::prompt_body(&received[before]);
    assert!(manager_prompt.contains(input));
    assert!(!manager_prompt.contains(MATERIAL) && !manager_prompt.contains("创建 Map 的整图交接"));
    post(&s, "/api/workflow/manager/prepare_inputs", json!({"sessionId":MANAGER,"fileInputs":[],"confirmed":true,"reason":"Reviewed the isolated text-only request"}), 200).await;

    let outline = zone.join("outline.md");
    let converted = zone.join("converted.json");
    let final_map = zone.join("optimized.json");
    let handoff = zone.join("handoff.md");
    let outline_bytes = b"# Isolated outline / r1\nAgreed: record the location, not the contents.\n";
    let handoff_bytes = b"Scope: one node input; no shared edits. Readback checked; business execution omitted.\r\n";
    let candidate = s.config.scheduler.map_directory.join("generated.json");
    let initial = json!({"title":"Generated test configuration","description":"Local sample only","nodes":{"record":{"title":"Record location","bot_id":"outline-planning","input":"Inventory existing notes.","zone":ZONE,"blocked_by":[]}}});
    let final_input = "Inventory existing notes; record their location, not their contents.";
    for ((node, phase, skill, _), session) in phases.into_iter().zip(["selected-outline", "selected-convert", "selected-optimize"]) {
        identity(&s, session, false);
        let before = submissions(&s).len();
        let dispatch = post(&s, "/api/workflow/manager/dispatch", json!({"sessionId":MANAGER,"nodes":[node]}), 200).await;
        assert_eq!(dispatch["results"][0]["status"], "running", "{dispatch}");
        let received = submissions(&s);
        assert_eq!(received.len(), before + 1);
        let prompt = common::prompt_body(&received[before]);
        assert!(prompt.contains(input) && prompt.contains(&material_pointer), "{prompt}");
        assert!(prompt.contains(&format!("「创建 Map 的整图交接」中的「{phase}」")), "{prompt}");
        assert!(!prompt.contains("各节点在准备交付本阶段成果时"), "the backend must not expand the business document");
        if !skill.is_empty() { assert!(prompt.starts_with(&format!("/skill:{skill} "))); }
        let outputs = match node {
            "outline" => {
                fs::write(&outline, outline_bytes).unwrap();
                json!([{"id":"outline","path":outline}])
            }
            "generate_map" => {
                assert!(prompt.contains(outline.to_str().unwrap()));
                fs::write(&candidate, initial.to_string()).unwrap();
                fs::copy(&candidate, &converted).unwrap();
                assert_eq!(read(&s, "/api/scheduler/maps/generated.json").await["nodes"]["record"]["input"], "Inventory existing notes.");
                json!([{"id":"map","path":converted}])
            }
            "optimize_map" => {
                for named in [&outline, &converted] { assert!(prompt.contains(named.to_str().unwrap()), "{prompt}"); }
                let endpoint = "/api/scheduler/maps/generated.json/nodes";
                let edit = post(&s, &format!("{endpoint}/edit"), json!({"id":"record"}), 200).await;
                let mut content = edit["content"].clone();
                content["input"] = json!(final_input);
                post(&s, &format!("{endpoint}/save"), json!({"id":"record","content":content.to_string(),"revision":edit["revision"],"create":false}), 200).await;
                fs::copy(&candidate, &final_map).unwrap();
                fs::write(&handoff, handoff_bytes).unwrap();
                json!([{"id":"map","path":final_map},{"id":"handoff","path":handoff}])
            }
            _ => unreachable!(),
        };
        approve_execution(&s, session).await;
        let report = post(&s, "/api/workflow/node/report", json!({"sessionId":session,"canComplete":true,"body":"Fixture artifact ready","outputs":outputs}), 200).await;
        assert_eq!(report["saved"], true);
        assert_eq!(report["report"]["outputs"], outputs);
        let complete = post(&s, "/api/workflow/manager/complete", json!({"sessionId":MANAGER,"nodeId":node,"reason":"Accepted the named fixture artifacts"}), 200).await;
        assert_eq!(complete["snapshot"]["nodes"][node]["status"], "completed");
    }
    post(&s, "/api/workflow/manager/finish", json!({"sessionId":MANAGER}), 200).await;
    let detail = read(&s, &run).await;
    assert_eq!(detail["run"]["status"], "completed");
    assert!(detail["run"]["mapOutput"]["files"].as_array().is_none_or(Vec::is_empty));
    assert_eq!(read(&s, "/api/workflow/runs").await["runs"].as_array().unwrap().len(), 1);
    let active = read(&s, "/api/scheduler/maps/generated.json").await;
    let final_bytes = fs::read(&final_map).unwrap();
    assert_eq!(fs::read(&candidate).unwrap(), final_bytes);
    assert_ne!(fs::read(&converted).unwrap(), final_bytes);
    let expected = json!({"title":"Generated test configuration","description":"Local sample only","nodes":{"record":{
        "title":"Record location","bot_id":"outline-planning","zone":"hugosol/herdr-bot","blocked_by":[],
        "input":"Inventory existing notes; record their location, not their contents.","input_rule":[],"output_rule":[],
        "auto_approved":true,"auto_close":false,"end_node":false
    }}});
    assert_eq!(active, expected);
    let material: Value = serde_json::from_slice(&final_bytes).unwrap();
    assert_eq!(material, expected);
    assert!(detail["reports"].as_array().unwrap().iter().any(|report| report["outputs"] == json!([
        {"id":"map","path":final_map},{"id":"handoff","path":handoff}
    ])));
    assert_eq!(fs::read(&outline).unwrap(), outline_bytes);
    assert_eq!(fs::read(&handoff).unwrap(), handoff_bytes);
    assert_eq!(fs::read(s.config.scheduler.map_directory.join(MAP)).unwrap(), selected);
    assert_eq!(fs::read(&old_map_path).unwrap(), old_map);
    assert_eq!(fs::read(&old_bot_path).unwrap(), old_bot);
    assert_eq!(fs::read(&pending).unwrap(), pending_bytes);
    assert_eq!(fs::read(&inventory).unwrap(), inventory_bytes);
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_start").count(), 7, "three standalone starts, one manager and three nodes; no generated business");
    println!("P53/P58 {run}; P41/P44 final optimized bytes sha256={:x}; saved config={active}", Sha256::digest(&final_bytes));
}

// P54/S4. Model the operator's explicit old-file authorization, with identical
// basenames online and a migrated Bot still referenced by an unprocessed Map.
// An online touch or wildcard removal must disagree with the complete byte set.
#[test]
fn selected_flow_cutover_removes_only_explicitly_confirmed_old_definition_copies() {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    let root = TempDir::new("selected definition cutover");
    let root = root.path();
    let official_map = fs::read(repo().join("meta-data/workflow-maps/plan_workflow.json")).unwrap();
    let official_bot = fs::read(repo().join("meta-data/bots").join(BOTS[0].0)).unwrap();
    let retained: BTreeMap<&str, &[u8]> = BTreeMap::from([
        ("online/maps/plan_workflow.json", official_map.as_slice()),
        ("online/bots/bot-1794388558375394800-31524-0.json", official_bot.as_slice()),
        ("offline/maps/unprocessed.json", br#"{"nodes":{"still-refers-to":{"bot_id":"bot-map-plan-workflow"}}}"#.as_slice()),
        ("offline/bots/unprocessed.json", b"UNPROCESSED BOT\r\n".as_slice()),
        ("offline/maps/abandoned.json", b"ABANDONED MAP\r\n".as_slice()),
        ("offline/bots/abandoned.json", b"ABANDONED BOT\r\n".as_slice()),
        ("shared/tasks.json", b"SHARED TASK SOURCE STAYS IN PLACE".as_slice()),
    ]);
    let removed: BTreeMap<&str, &[u8]> = BTreeMap::from([
        ("offline/maps/plan_workflow.json", b"OLD MAP\r\n".as_slice()),
        ("offline/bots/bot-1794388558375394800-31524-0.json", b"OLD BOT\r\n".as_slice()),
    ]);
    let before: BTreeMap<_, _> = retained.iter().chain(&removed).map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec())).collect();
    for (name, bytes) in &before {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        println!("P54 before {name}: {} bytes, sha256={:x}", bytes.len(), Sha256::digest(bytes));
    }
    let snapshot = || -> BTreeMap<String, Vec<u8>> {
        ["online/maps", "online/bots", "offline/maps", "offline/bots", "shared"].into_iter().flat_map(|directory| {
            fs::read_dir(root.join(directory)).unwrap().map(move |entry| {
                let entry = entry.unwrap();
                (format!("{directory}/{}", entry.file_name().to_str().unwrap()), fs::read(entry.path()).unwrap())
            })
        }).collect()
    };
    assert_eq!(snapshot(), before);
    // Literal file operations mirror the stopped-service manual runbook. The
    // expected set above is independent of the operands used here.
    fs::remove_file(root.join("offline/maps/plan_workflow.json")).unwrap();
    fs::remove_file(root.join("offline/bots/bot-1794388558375394800-31524-0.json")).unwrap();
    let after = snapshot();
    assert_eq!(after, retained.into_iter().map(|(name, bytes)| (name.to_owned(), bytes.to_vec())).collect());
    for (name, bytes) in after { println!("P54 retained {name}: sha256={:x}", Sha256::digest(&bytes)); }
    for name in removed.keys() { println!("P54 removed {name}"); }
}
