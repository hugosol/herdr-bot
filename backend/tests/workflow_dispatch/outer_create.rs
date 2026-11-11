//! Enabling seam: public definition write/read and preflight, not P45/P46's
//! real outer execution or independent/workflow pairs (owned by 24).
//! Publishes the outer creation, wrapper and final Bot optimization stages through
//! public APIs; optional and unreadable materials exercise real preflight behavior.
use super::*;

#[tokio::test]
async fn outer_create_publishes_task_then_wrapper_without_changing_legacy_entries() {
    let s = Scenario::new().await;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let file = "create-bot-task-workflow--bc-61ce48b80a4c.json";
    let definition: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/workflow-maps").join(file)).unwrap()).unwrap();
    let nodes = definition["nodes"].as_object().unwrap();

    for (file,id,task) in [
        ("bot-1793301029285548400-31876-1.json","create-bot-task","task-18e315be1cf0ef10-0"),
        ("bot-1793309187911298500-58200-4.json","create-bot","task-18e31d347ccbeb4c-4"),
        ("bot-1793309187825217800-58200-2.json","create-task","task-18e31d30fe69a51c-2"),
        ("bot-1793309187850682300-58200-3.json","optimize-prompt","task-18e31d34797f1464-3")
    ] {
        let bot: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/bots").join(file)).unwrap()).unwrap();
        assert_eq!(bot["id"],id);
        assert_eq!(bot["task_id"],task);
        assert!(bot.get("map_file").is_none());
    }
    let worker = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"worker"}"#).await;
    let worker: Value = serde_json::from_str(&worker.body).unwrap();
    for id in ["create-task","optimize-prompt","bot-task-18e66273ae718d14-1"] {
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"content":worker["content"].to_string(),"revision":null}).to_string()).await;
        assert_eq!(saved.status,200,"{}",saved.body);
    }
    // Keep the shipped child topology; only its host-facing Tasks and zones
    // are fixture-owned. No real Suite or native model behavior is claimed.
    let mut child: Value = serde_json::from_str(&fs::read_to_string(root.join("meta-data/workflow-maps/create-bot-wrapper.json")).unwrap()).unwrap();
    for node in child["nodes"].as_object_mut().unwrap().values_mut() {
        node["bot_id"] = json!("worker");
        node["zone"] = json!("node");
    }
    fs::write(s.config.scheduler.map_directory.join("create-bot-wrapper.json"), child.to_string()).unwrap();
    let bot = json!({"name":"Wrapper","when":"Existing targets only","plan_guide":"","node_guide":"","map_file":"create-bot-wrapper.json"});
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"create-bot-wrapper","content":bot.to_string(),"revision":null}).to_string()).await;
    assert_eq!(saved.status,200,"{}",saved.body);
    fs::write(s.config.scheduler.map_directory.join(file), r#"{"title":"Outer","description":"","nodes":{}}"#).unwrap();
    let endpoint = format!("/api/scheduler/maps/{file}");
    let read = get_over_tcp(s.server.addr(), &format!("{endpoint}/inputs")).await;
    let read: Value = serde_json::from_str(&read.body).unwrap();
    let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/inputs"), &json!({"input_files":definition["input_files"],"input_requirements":definition["input_requirements"].as_str().unwrap_or(""),"revision":read["revision"]}).to_string()).await;
    assert_eq!(saved.status,200,"{}",saved.body);
    for id in ["w1","w2","wrap_bot","optimize_bot"] {
        let mut node = nodes[id].clone();
        node["zone"] = json!("node");
        if id == "wrap_bot" { node["manager_task_id"] = json!(s.manager_task); }
        let draft = get_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/new")).await;
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/save"), &json!({"id":id,"content":node.to_string(),"revision":draft["revision"],"create":true}).to_string()).await;
        assert_eq!(saved.status,200,"{}",saved.body);
        let read = get_over_tcp(s.server.addr(), &endpoint).await;
        assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["nodes"][id],node);
    }
    let material = s.data.path().join("reference.txt");
    fs::write(&material,"Explicit resource limitation: read-only invoice review.").unwrap();
    for inputs in [json!([]),json!([{"id":"materials","path":material}])] {
        let response = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/preflight"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Create an invoice review Task then wrap it","fileInputs":inputs}).to_string()).await;
        assert_eq!(response.status,200,"{}",response.body);
    }
    fs::remove_file(&material).unwrap();
    let rejected = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/preflight"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Create","fileInputs":[{"id":"materials","path":material}]}).to_string()).await;
    assert_eq!(rejected.status,400,"{}",rejected.body);
    assert!(s.herdr.calls().is_empty(),"publishing and preflight cannot execute business work");
}
