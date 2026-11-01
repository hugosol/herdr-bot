//! Ticket 06, approved S1 seam: the shipped planning definition is readable,
//! editable and service-visible, without starting a business Run. P27/P28's
//! real Chat, business translation and human-confirmation evidence belong to 09.
use super::*;

// Independent ticket expectation: generate_map consumes outline's named outline
// output, while the original two-node dependency and report policies survive.
// A migration that leaves log-based handoff (no source declaration), drops the
// reference on save, changes a binding or starts work must fail this scenario.
#[tokio::test]
async fn planning_definition_round_trips_named_handoff_without_starting_work() {
    let s = Scenario::new().await;
    let definition = include_str!("../../../meta-data/workflow-maps/plan_workflow.json");
    fs::write(s.config.scheduler.map_directory.join("plan_workflow.json"), definition).unwrap();
    let endpoint = "/api/scheduler/maps/plan_workflow.json";
    let response = get_over_tcp(s.server.addr(), endpoint).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let original: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(original["nodes"].as_object().unwrap().len(), 2);
    assert_eq!(original["nodes"]["outline"]["bot_id"], "outline-planning");
    assert_eq!(original["nodes"]["generate_map"]["bot_id"], "plan-to-map");
    assert_eq!(original["nodes"]["outline"]["blocked_by"], json!([]));
    assert_eq!(original["nodes"]["generate_map"]["blocked_by"], json!(["outline"]));
    assert_eq!(original["nodes"]["outline"]["report_policy"], "manual");
    assert_eq!(original["nodes"]["generate_map"]["report_policy"], "auto");
    assert_eq!(original["nodes"]["generate_map"]["inputs"], json!({
        "outline": {"source":"node", "node_id":"outline", "output":"outline"}
    }));

    for id in ["outline", "generate_map"] {
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
    assert_eq!(catalog["titles"]["plan_workflow.json"], "plan_workflow");
    let runs = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs.status, 200, "{}", runs.body);
    assert_eq!(serde_json::from_str::<Value>(&runs.body).unwrap()["runs"], json!([]));
    assert!(s.herdr.calls().is_empty(), "definition operations cannot start Chats");
}
