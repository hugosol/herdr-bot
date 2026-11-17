// P1: the contract requires a strict boolean, default true. A permissive
// parser could accept null/strings or overwrite the saved definition on error.
#[tokio::test]
async fn map_auto_approved_defaults_round_trips_and_rejects_invalid_saves_without_writing() {
    let s = Scenario::new().await;
    let map_response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(map_response.status, 200);
    let map: Value = serde_json::from_str(&map_response.body).unwrap();
    assert_eq!(map["nodes"]["A"]["auto_approved"], true);
    let template = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
    assert_eq!(template.status, 200);
    assert_eq!(serde_json::from_str::<Value>(&template.body).unwrap()["content"]["auto_approved"], true);
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    assert_eq!(edit.status, 200);
    let draft: Value = serde_json::from_str(&edit.body).unwrap();
    let path = s.config.scheduler.map_directory.join("work.json");
    let original = fs::read(&path).unwrap();
    for policy in [json!("true"), json!("false"), Value::Null, json!(0), json!({}), json!({"true":null}), json!({"false":null}), json!([])] {
        let mut content = draft["content"].clone();
        content["auto_approved"] = policy;
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        let error: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(error["code"], "node-invalid");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    let mut content = draft["content"].clone();
    content["auto_approved"] = json!(true);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let readback = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(serde_json::from_str::<Value>(&readback.body).unwrap()["nodes"]["A"]["auto_approved"], true);
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    let draft: Value = serde_json::from_str(&edit.body).unwrap();
    content["auto_approved"] = json!(false);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap()["nodes"]["A"]["auto_approved"], false);
    let readback = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(serde_json::from_str::<Value>(&readback.body).unwrap()["nodes"]["A"]["auto_approved"], false);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["nodes"]["A"]["auto_approved"], false);
    let mut invalid: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    invalid["nodes"]["A"]["auto_approved"] = json!("invalid");
    fs::write(&path, invalid.to_string()).unwrap();
    let rejected = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(rejected.status, 500, "{}", rejected.body);
    assert_eq!(serde_json::from_str::<Value>(&rejected.body).unwrap()["code"], "map-read-failed");
}

