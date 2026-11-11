//! Ticket 17 enabling checks, not P39-P45 real-model acceptance (owned by 21).
//! Approved seam: shipped Map/Bot via public definition save/read and preflight.
//! Expected: eight nodes, one direct baseline for every consumer, three separate
//! writer/optimizer pairs, one seven-material save. A missing baseline edge or
//! Task-creating node must disagree with these independent literal expectations.
use super::*;

#[tokio::test]
async fn create_bot_wrapper_publishes_complete_direct_material_interface() {
    let s = Scenario::new().await;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let path = root.join("meta-data/workflow-maps/create-bot-wrapper.json");
    assert!(path.is_file(), "publish the existing-target wrapper Map");
    let definition: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let input_shape: Vec<Value> = definition["input_files"].as_array().unwrap().iter()
        .map(|input| json!({"id": input["id"], "required": input["required"]})).collect();
    assert_eq!(input_shape, vec![json!({"id":"request","required":true}), json!({"id":"materials","required":false})]);
    assert_eq!(definition["nodes"].as_object().unwrap().len(), 8);
    assert_eq!(definition["nodes"]["create_blank"]["input_rule"], json!([
        {"id":"request","type":"run","input_id":"request"},
        {"id":"materials","type":"run","input_id":"materials"}
    ]));
    assert_eq!(definition["nodes"]["create_blank"]["output_rule"], json!([{"id":"baseline"},{"id":"handoff"}]));
    let bindings = [("create_blank","create-blank-bot"),("write_when","write-bot-when"),("write_plan_guide","write-bot-plan-guide"),("write_node_guide","write-bot-node-guide"),("optimize_when","optimize-prompt"),("optimize_plan_guide","optimize-prompt"),("optimize_node_guide","optimize-prompt"),("save_bot","replace-text")];
    for (id, bot) in bindings { assert_eq!(definition["nodes"][id]["bot_id"], bot); }
    for field in ["when","plan_guide","node_guide"] {
        let writer = format!("write_{field}");
        let optimizer = format!("optimize_{field}");
        let baseline = json!({"id":"baseline","type":"node","node_id":"create_blank","output_id":"baseline"});
        assert_eq!(definition["nodes"][&writer]["input_rule"], json!([baseline.clone()]));
        assert_eq!(definition["nodes"][&writer]["blocked_by"], json!(["create_blank"]));
        assert_eq!(definition["nodes"][&writer]["report_policy"], "manual");
        assert_eq!(definition["nodes"][&optimizer]["input_rule"], json!([baseline,
            {"id":"text","type":"node","node_id":writer,"output_id":"text"},
            {"id":"handoff","type":"node","node_id":writer,"output_id":"handoff"}]));
        assert_eq!(definition["nodes"][&optimizer]["blocked_by"], json!([writer]));
        for id in [&writer,&optimizer] { assert_eq!(definition["nodes"][id]["output_rule"], json!([{"id":"text"},{"id":"handoff"}])); }
    }
    let mut final_inputs = vec![json!({"id":"baseline","type":"node","node_id":"create_blank","output_id":"baseline"})];
    for field in ["when","plan_guide","node_guide"] { for role in ["text","handoff"] {
        final_inputs.push(json!({"id":format!("{field}_{role}"),"type":"node","node_id":format!("optimize_{field}"),"output_id":role}));
    }}
    assert_eq!(definition["nodes"]["save_bot"]["input_rule"], json!(final_inputs));
    assert_eq!(definition["nodes"]["save_bot"]["output_rule"], json!([{"id":"saved"},{"id":"handoff"}]));
    // The wrapper's shipped binding and all shared Task identities are checked
    // unchanged before using fixture-owned tasks/resources for host preflight.
    let wrapper: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/bots/create-bot-wrapper.json")).unwrap()).unwrap();
    assert_eq!(wrapper["id"], "create-bot-wrapper");
    assert_eq!(wrapper["map_file"], "create-bot-wrapper.json");
    assert!(wrapper.get("task_id").is_none());
    let shared = [
        ("bot-1793348060748020300-25960-2.json","create-blank-bot","task-18e33fdc55177d1c-1"),
        ("bot-1793348357474956000-25960-3.json","write-bot-when","task-18e3404478db1ef8-2"),
        ("bot-1793348521344242500-25960-4.json","write-bot-plan-guide","task-18e340545eb481bc-3"),
        ("bot-1793349245294351300-25960-8.json","write-bot-node-guide","task-18e340f5287462ec-4"),
        ("bot-1793309187850682300-58200-3.json","optimize-prompt","task-18e31d34797f1464-3"),
        ("bot-1793349624876930800-25960-9.json","replace-text","task-18e3415003d8cd00-5")];
    let worker = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"worker"}"#).await;
    let worker: Value = serde_json::from_str(&worker.body).unwrap();
    for (file,id,task) in shared {
        let mut bot: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/bots").join(file)).unwrap()).unwrap();
        assert_eq!(bot["id"], id);
        assert_eq!(bot["task_id"], task);
        bot.as_object_mut().unwrap().remove("id");
        bot["task_id"] = worker["content"]["task_id"].clone();
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"content":bot.to_string(),"revision":null}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
    }
    fs::write(s.config.scheduler.map_directory.join("create-bot-wrapper.json"), r#"{"title":"Wrapper","description":"","nodes":{}}"#).unwrap();
    let endpoint = "/api/scheduler/maps/create-bot-wrapper.json";
    let interface = get_over_tcp(s.server.addr(), &format!("{endpoint}/inputs")).await;
    let interface: Value = serde_json::from_str(&interface.body).unwrap();
    let declared = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/inputs"), &json!({"input_files":definition["input_files"],"input_requirements":definition["input_requirements"].as_str().unwrap_or(""),"revision":interface["revision"]}).to_string()).await;
    assert_eq!(declared.status, 200, "{}", declared.body);
    for id in ["create_blank", "write_when", "write_plan_guide", "write_node_guide", "optimize_when", "optimize_plan_guide", "optimize_node_guide", "save_bot"] {
        let mut node = definition["nodes"][id].clone();
        node["zone"] = json!("node");
        let draft = get_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/new")).await;
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/save"), &json!({"id":id,"content":node.to_string(),"revision":draft["revision"],"create":true}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let read = get_over_tcp(s.server.addr(), endpoint).await;
        assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["nodes"][id], node);
    }
    let mut wrapper_content = wrapper.clone();
    wrapper_content.as_object_mut().unwrap().remove("id");
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"create-bot-wrapper","content":wrapper_content.to_string(),"revision":null}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"create-bot-wrapper"}"#).await;
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["content"], wrapper_content);
    let request = s.data.path().join("request.md");
    fs::write(&request, "Target: normal Task invoice-review. Create a wrapper, not a Task. No reference materials.").unwrap();
    let addr = s.server.addr();
    let preflight = |inputs: Value| { let request = json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Create only the authorized Bot","fileInputs":inputs}); async move { post_json_over_tcp(addr, &format!("{endpoint}/preflight"), &request.to_string()).await } };
    assert_eq!(preflight(json!([])).await.status, 200);
    let valid = preflight(json!([{"id":"request","path":request}])).await;
    assert_eq!(valid.status, 200, "{}", valid.body);
    let missing = preflight(json!([{"id":"request","path":request},{"id":"materials","path":s.data.path().join("absent.md")}])).await;
    assert_eq!(missing.status, 400, "{}", missing.body);
    assert!(s.herdr.calls().is_empty(), "definition publication cannot start business work");
}
