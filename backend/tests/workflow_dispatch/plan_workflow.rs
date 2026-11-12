//! Approved public definition seams: shipped planner presets, guides and Maps
//! remain readable, editable and service-visible without starting business work.
//! This enabling coverage does not replace real-model planning acceptance.
use super::*;

// Enabling check for nesting P03 (real model selection is owned by 23).
// Approved seams: public Task/Bot definition readback and Map save/preflight.
// A deployment that drops a planner's binding or business prompt must disagree
// with the complete shipped object, not with a particular prompt phrase.
#[tokio::test]
async fn shipped_planner_presets_and_guides_are_publicly_readable_without_launch() {
    let s = Scenario::new().await;
    fs::write(&s.config.tasks.storage_file, include_str!("../../../meta-data/tasks.json")).unwrap();
    let shipped: Value = serde_json::from_str(include_str!("../../../meta-data/tasks.json")).unwrap();
    for definition in [
        include_str!("../../../meta-data/bots/bot-1793304246857557100-58200-0.json"),
        include_str!("../../../meta-data/bots/bot-1793305512701853300-58200-1.json"),
    ] {
        let bot: Value = serde_json::from_str(definition).unwrap();
        let id = bot["id"].as_str().unwrap();
        fs::write(s.config.scheduler.bot_directory.join(format!("{id}.json")), definition).unwrap();
        let read = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", &json!({"id":id}).to_string()).await;
        assert_eq!(read.status, 200, "{}", read.body);
        let mut expected_bot = bot.clone();
        expected_bot.as_object_mut().unwrap().remove("id");
        assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["content"], expected_bot);
        let task_id = bot["task_id"].as_str().unwrap();
        let expected = shipped["tasks"].as_array().unwrap().iter().find(|task| task["id"] == task_id).unwrap();
        let read = get_over_tcp(s.server.addr(), "/api/tasks").await;
        assert_eq!(read.status, 200, "{}", read.body);
        let read: Value = serde_json::from_str(&read.body).unwrap();
        let actual = read["tasks"].as_array().unwrap().iter().find(|task| task["id"] == task_id).unwrap();
        assert_eq!(actual, expected);
    }
    let runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs.status, 200, "{}", runs.body);
    assert_eq!(serde_json::from_str::<Value>(&runs.body).unwrap()["runs"], json!([]));
    assert!(s.herdr.calls().is_empty());
}

// Independent planning example: one child call followed by an ordinary Task.
// A flattened graph, invented mapping fields, manager copied into the Bot, or
// a save that loses local material IDs is distinguishable through public reads.
// This validates the translation shape, not a real model's capability choice.
#[tokio::test]
async fn mixed_target_planning_shape_saves_and_preflights_without_running_business() {
    let s = Scenario::new().await;
    fs::write(s.data.path().join("node-zone/source.txt"), "ORDER-SOURCE-617\nEUR 12.50").unwrap();
    let child_before = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    fs::write(s.config.scheduler.map_directory.join("planned.json"), r#"{"title":"Confirmed order review","description":"Only authorized order review","nodes":{}}"#).unwrap();
    let bot = json!({"name":"Order review process","when":"Review orders","plan_guide":"Use the existing process","node_guide":"Provide source and review result","map_file":"work.json"});
    let created = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"review-process","content":bot.to_string(),"revision":null}).to_string()).await;
    assert_eq!(created.status, 200, "{}", created.body);
    let interface = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/inputs").await;
    assert_eq!(interface.status, 200, "{}", interface.body);
    let interface: Value = serde_json::from_str(&interface.body).unwrap();
    let declared = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/inputs", &json!({"input_files":[{"id":"source","required":true}],"input_requirements":"","revision":interface["revision"]}).to_string()).await;
    assert_eq!(declared.status, 200, "{}", declared.body);
    let child_declared = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    assert_ne!(child_declared, child_before);
    let nodes = [
        ("review", json!({"title":"Review orders","bot_id":"review-process","manager_task_id":s.manager_task,"input":"Review the named source in EUR; deliver only the review summary.","zone":"node","blocked_by":[],"input_rule":[{"id":"source","type":"file","zone_id":"node","path":"source.txt"}],"output_rule":[{"id":"review_result"}],"report_policy":"manual","auto_close":false})),
        ("check", json!({"title":"Check summary","bot_id":"worker","input":"Check the named summary against the authorized EUR scope.","zone":"node","blocked_by":["review"],"input_rule":[{"id":"summary","type":"node","node_id":"review","output_id":"review_result"}],"output_rule":[{"id":"checked"}],"report_policy":"auto","auto_close":false})),
    ];
    for (id, node) in &nodes {
        let draft = get_over_tcp(s.server.addr(), "/api/scheduler/maps/planned.json/nodes/new").await;
        assert_eq!(draft.status, 200, "{}", draft.body);
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/planned.json/nodes/save", &json!({"id":id,"content":node.to_string(),"revision":draft["revision"],"create":true}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
    }
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/planned.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    let read: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(read["nodes"].as_object().unwrap().len(), 2, "child nodes stay in their own Map");
    for (id, expected) in &nodes { assert_eq!(&read["nodes"][id], expected); }
    assert_eq!(read["nodes"]["review"]["input_rule"][0]["id"], "source");
    assert_eq!(read["nodes"]["check"]["input_rule"][0]["output_id"], "review_result");
    let preflight = || async { post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/planned.json/preflight", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Root-only business request"}).to_string()).await };
    let valid = preflight().await;
    assert_eq!(valid.status, 200, "{}", valid.body);
    // Keep all three convincing guides unchanged but remove the actual target.
    fs::remove_file(s.config.scheduler.map_directory.join("work.json")).unwrap();
    let rejected = preflight().await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    fs::write(s.config.scheduler.map_directory.join("work.json"), &child_declared).unwrap();
    assert_eq!(fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap(), child_declared);
    let bot_read = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"review-process"}"#).await;
    assert_eq!(bot_read.status, 200, "{}", bot_read.body);
    assert_eq!(serde_json::from_str::<Value>(&bot_read.body).unwrap()["content"], bot);
    let runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs.status, 200, "{}", runs.body);
    assert_eq!(serde_json::from_str::<Value>(&runs.body).unwrap()["runs"], json!([]));
    assert!(s.herdr.calls().is_empty(), "saving or checking a plan must not execute it");
}

// Editing and saving each shipped node must preserve the complete definition
// without starting work.
#[tokio::test]
async fn planning_definition_round_trips_named_handoff_without_starting_work() {
    let s = Scenario::new().await;
    let definition = include_str!("../../../meta-data/workflow-maps/plan_workflow.json");
    fs::write(s.config.scheduler.map_directory.join("plan_workflow.json"), definition).unwrap();
    let endpoint = "/api/scheduler/maps/plan_workflow.json";
    let response = get_over_tcp(s.server.addr(), endpoint).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let original: Value = serde_json::from_str(&response.body).unwrap();
    for id in original["nodes"].as_object().unwrap().keys() {
        let draft = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/edit"), &json!({"id":id}).to_string()).await;
        assert_eq!(draft.status, 200, "{}", draft.body);
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let saved = post_json_over_tcp(s.server.addr(), &format!("{endpoint}/nodes/save"), &json!({
            "id":id, "content":draft["content"].to_string(), "revision":draft["revision"], "create":false
        }).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), original);
    }
    let readback = get_over_tcp(s.server.addr(), endpoint).await;
    assert_eq!(readback.status, 200, "{}", readback.body);
    assert_eq!(serde_json::from_str::<Value>(&readback.body).unwrap(), original);
    let catalog = get_over_tcp(s.server.addr(), "/api/scheduler/maps").await;
    assert_eq!(catalog.status, 200, "{}", catalog.body);
    let catalog: Value = serde_json::from_str(&catalog.body).unwrap();
    assert!(catalog["files"].as_array().unwrap().contains(&json!("plan_workflow.json")));
    let runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs.status, 200, "{}", runs.body);
    assert_eq!(serde_json::from_str::<Value>(&runs.body).unwrap()["runs"], json!([]));
    assert!(s.herdr.calls().is_empty(), "definition operations cannot start Chats");
}
