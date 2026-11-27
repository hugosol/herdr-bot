//! Approved public definition seams: planner Task/Bot definitions and Maps
//! can be read and edited without starting business work. Approved production
//! planning assets are verified separately by the selected-flow migration.
use super::*;

// S1/S4: new-format capability summaries and their actual Task targets remain
// separately readable. The test-owned Bot fixtures do not migrate production copy.
#[tokio::test]
async fn planner_presets_and_bot_summaries_are_publicly_readable_without_launch() {
    let s = Scenario::new().await;
    fs::write(&s.config.tasks.storage_file, include_str!("../../../../meta-data/tasks.json")).unwrap();
    let shipped: Value = serde_json::from_str(include_str!("../../../../meta-data/tasks.json")).unwrap();
    let legacy = r#"{"id":"legacy-planner","name":"Legacy planner","when":"Plan","plan_guide":"Plan first","node_guide":"Explain scope","task_id":"task-18e318b13b71a8bc-0"}"#;
    let path = s.config.scheduler.bot_directory.join("legacy-planner.json");
    fs::write(&path, legacy).unwrap();
    let rejected = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"legacy-planner"}"#).await;
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    assert!(rejected.body.contains("plan_guide"), "{}", rejected.body);
    assert_eq!(fs::read_to_string(path).unwrap(), legacy);

    for bot in [
        json!({"id":"fixture-outline","name":"Outline fixture","when":"Plan a process","description":"Clarify the scope","input_rule":"Business request","output_rule":"Outline","task_id":"task-18e318b13b71a8bc-0"}),
        json!({"id":"fixture-translate","name":"Translate fixture","when":"Convert an outline","description":"Build a Map definition","input_rule":"Approved outline","output_rule":"Map JSON","task_id":"task-18e319d73777fc6c-1"}),
    ] {
        let id = bot["id"].as_str().unwrap();
        fs::write(s.config.scheduler.bot_directory.join(format!("{id}.json")), bot.to_string()).unwrap();
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
    let bot = json!({"name":"Order review process","when":"Review orders","description":"Use the existing process","input_rule":"Provide source and review result","output_rule":"","map_file":"work.json"});
    let created = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"review-process","content":bot.to_string(),"revision":null}).to_string()).await;
    assert_eq!(created.status, 200, "{}", created.body);
    let interface = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/inputs").await;
    assert_eq!(interface.status, 200, "{}", interface.body);
    let interface: Value = serde_json::from_str(&interface.body).unwrap();
    let declared = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/inputs", &json!({"map_input_rule":{"files":[{"id":"source","required":true}]},"map_output_rule":{"files":[{"id":"review_result"}]},"revision":interface["revision"]}).to_string()).await;
    assert_eq!(declared.status, 200, "{}", declared.body);
    let child_declared = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    assert_ne!(child_declared, child_before);
    let nodes = [
        ("review", json!({"title":"Review orders","bot_id":"review-process","manager_task_id":s.manager_task,"input":"Review the named source in EUR; deliver only the review summary.","zone":"node","blocked_by":[],"input_rule":[{"id":"source","type":"file","zone_id":"node","path":"source.txt"}],"output_rule":[{"id":"review_result"}],"auto_approved":false,"auto_close":false})),
        ("check", json!({"title":"Check summary","bot_id":"worker","input":"Check the named summary against the authorized EUR scope.","zone":"node","blocked_by":["review"],"input_rule":[{"id":"summary","type":"node","node_id":"review","output_id":"review_result"}],"output_rule":[{"id":"checked"}],"auto_approved":true,"auto_close":false})),
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
    for (id, expected) in &nodes {
        let mut expected = expected.clone();
        expected["end_node"] = json!(false);
        assert_eq!(read["nodes"][id], expected);
    }
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

// S1/S4: editing and saving each node preserves the new-format named handoff
// without starting work. Retired rule fields do not become online definitions.
#[tokio::test]
async fn planning_definition_round_trips_named_handoff_without_starting_work() {
    let s = Scenario::new().await;
    let legacy = r#"{"title":"Legacy plan","description":"","input_requirements":"Describe the change","nodes":{}}"#;
    let path = s.config.scheduler.map_directory.join("legacy-plan.json");
    fs::write(&path, legacy).unwrap();
    let rejected = get_over_tcp(s.server.addr(), "/api/scheduler/maps/legacy-plan.json").await;
    assert_eq!(rejected.status, 500, "{}", rejected.body);
    assert!(rejected.body.contains("input_requirements"), "{}", rejected.body);
    assert_eq!(fs::read_to_string(path).unwrap(), legacy);

    let definition = json!({"title":"Planning handoff fixture","description":"","map_input_rule":{"description":"Describe the change","files":[]},"nodes":{
        "outline":{"title":"Outline","bot_id":"worker","input":"Draft an outline","zone":"node","blocked_by":[],"output_rule":[{"id":"outline"}]},
        "translate":{"title":"Translate","bot_id":"worker","input":"Translate the approved outline","zone":"node","blocked_by":["outline"],"input_rule":[{"type":"node","id":"outline","node_id":"outline","output_id":"outline"}],"output_rule":[{"id":"map"}]},
        "check":{"title":"Check","bot_id":"worker","input":"Check the approved outline and Map","zone":"node","blocked_by":["translate"],"input_rule":[{"type":"node","id":"outline","node_id":"outline","output_id":"outline"},{"type":"node","id":"map","node_id":"translate","output_id":"map"}],"output_rule":[{"id":"map"},{"id":"handoff"}]}
    }});
    fs::write(s.config.scheduler.map_directory.join("plan_workflow.json"), definition.to_string()).unwrap();
    let endpoint = "/api/scheduler/maps/plan_workflow.json";
    let response = get_over_tcp(s.server.addr(), endpoint).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let original: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(original["map_input_rule"], definition["map_input_rule"]);
    for (id, node) in definition["nodes"].as_object().unwrap() {
        for field in ["blocked_by", "output_rule"] {
            assert_eq!(original["nodes"][id][field], node[field]);
        }
        assert_eq!(original["nodes"][id]["input_rule"], node.get("input_rule").cloned().unwrap_or(json!([])));
    }
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
