//! Ticket 18 enabling seam: shipped definitions -> public save/read/preflight.
//! Independent expectation: five nodes, read-only baseline producer, three
//! independent manual writers and one directly bound seven-material save.
//! Violation: retaining the Task.prompt optimizer prerequisite or dropping the
//! baseline edge must fail even if the service accepts the remaining graph.
//! P40-P45 real-model scenarios and dedicated Task pairing belong to ticket 22.
use super::*;

#[tokio::test]
async fn revise_bot_guides_publishes_independent_five_node_material_interface() {
    let s = Scenario::new().await;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let file = "enhance-existing-bot--bc-301b25d9f8d6.json";
    let definition: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/workflow-maps").join(file)).unwrap()).unwrap();
    assert_eq!(definition["input_files"], json!([{"id":"request","required":true},{"id":"materials","required":false}]));
    assert_eq!(definition["nodes"].as_object().unwrap().len(), 5);
    assert_eq!(definition["nodes"]["w1"]["bot_id"], "prepare-bot-revision");
    assert_eq!(definition["nodes"]["w1"]["input_rule"], json!([
        {"id":"request","type":"run","input_id":"request"},
        {"id":"materials","type":"run","input_id":"materials"}
    ]));
    assert_eq!(definition["nodes"]["w1"]["output_rule"], json!([{"id":"baseline"},{"id":"handoff"}]));
    let baseline = json!({"id":"baseline","type":"node","node_id":"w1","output_id":"baseline"});
    let mut final_inputs = vec![baseline.clone()];
    for (id, field, bot) in [("w2","when","write-bot-when"),("w3","plan_guide","write-bot-plan-guide"),("w4","node_guide","write-bot-node-guide")] {
        let node = &definition["nodes"][id];
        assert_eq!(node["bot_id"], bot);
        assert_eq!(node["input_rule"], json!([baseline.clone()]));
        assert_eq!(node["blocked_by"], json!(["w1"]));
        assert_eq!(node["report_policy"], "manual");
        assert_eq!(node["output_rule"], json!([{"id":"text"},{"id":"handoff"}]));
        for role in ["text","handoff"] {
            final_inputs.push(json!({"id":format!("{field}_{role}"),"type":"node","node_id":id,"output_id":role}));
        }
    }
    assert_eq!(definition["nodes"]["w5"]["bot_id"], "replace-text");
    assert_eq!(definition["nodes"]["w5"]["blocked_by"], json!(["w2","w3","w4"]));
    assert_eq!(definition["nodes"]["w5"]["input_rule"], json!(final_inputs));
    assert_eq!(definition["nodes"]["w5"]["output_rule"], json!([{"id":"saved"},{"id":"handoff"}]));
    let tasks: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/tasks.json")).unwrap()).unwrap();
    let task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["id"] == "task-revise-bot-baseline-v1").expect("publish stable revision-only Task for owner 22 pairing");
    assert_eq!(task["taskType"], "normal");
    let worker = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"worker"}"#).await;
    let worker: Value = serde_json::from_str(&worker.body).unwrap();
    for (id, source) in [("prepare-bot-revision","prepare-bot-revision.json"),("revise-bot-guides","revise-bot-guides.json")] {
        let mut bot: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/bots").join(source)).unwrap()).unwrap();
        assert_eq!(bot["id"], id);
        if id == "prepare-bot-revision" { assert_eq!(bot["task_id"], "task-revise-bot-baseline-v1"); }
        else { assert_eq!(bot["map_file"], file); }
        bot.as_object_mut().unwrap().remove("id");
        if id == "prepare-bot-revision" { bot["task_id"] = worker["content"]["task_id"].clone(); }
        if id == "revise-bot-guides" {
            fs::write(s.config.scheduler.map_directory.join(file), r#"{"title":"Revision","description":"","nodes":{}}"#).unwrap();
        }
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"content":bot.to_string(),"revision":null}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let read = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", &json!({"id":id}).to_string()).await;
        assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["content"], bot);
    }
    // Host resources remain test-owned; this is not deployed Suite evidence.
    for id in ["write-bot-when","write-bot-plan-guide","write-bot-node-guide","replace-text"] {
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"revision":null,"content":worker["content"].to_string()}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
    }
    let endpoint = format!("/api/scheduler/maps/{file}");
    let interface = get_over_tcp(s.server.addr(), &format!("{endpoint}/inputs")).await;
    let interface: Value = serde_json::from_str(&interface.body).unwrap();
    let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/inputs"), &json!({"input_files":definition["input_files"],"revision":interface["revision"]}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    for id in ["w1","w2","w3","w4","w5"] {
        let mut node = definition["nodes"][id].clone();
        node["zone"] = json!("node");
        let draft = get_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/new")).await;
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/save"), &json!({"id":id,"content":node.to_string(),"revision":draft["revision"],"create":true}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let read = get_over_tcp(s.server.addr(), &endpoint).await;
        assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["nodes"][id], node);
    }
    let request = s.data.path().join("revision-request.md");
    let material = s.data.path().join("reference.md");
    fs::write(&request, "Revise existing Bot worker only. Keep its Task unchanged. No reference material required.").unwrap();
    fs::write(&material, "Preserve the original authorization boundary.").unwrap();
    for (inputs, status) in [(json!([]),400), (json!([{"id":"request","path":request}]),200), (json!([{"id":"request","path":request},{"id":"materials","path":material}]),200), (json!([{"id":"request","path":request},{"id":"materials","path":s.data.path().join("missing.md")}]),400)] {
        let response = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/preflight"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Revise guides, not the target","fileInputs":inputs}).to_string()).await;
        assert_eq!(response.status, status, "{}", response.body);
    }
    assert!(s.herdr.calls().is_empty(), "publication and preflight must not run business work");
}
