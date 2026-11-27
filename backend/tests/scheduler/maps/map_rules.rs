use super::*;

const MAP: &str = "/api/scheduler/maps/work.json";
const RULES: &str = "/api/scheduler/maps/work.json/inputs";
const NODES: &str = "/api/scheduler/maps/work.json/nodes/save";

// S1/S4, P5: the approved rule format allows both sides to use `brief`,
// while omitting a whole rule differs from declaring a rule with no files.
// A flat schema, shared ID set, or loss of defaults breaks the round trip.
#[tokio::test]
async fn public_rules_round_trip_with_independent_ids_and_defaults() {
    let s = Scenario::new().await;
    let addr = s.server.addr();
    let path = s.config.scheduler.map_directory.join("work.json");
    fs::write(&path, r#"{"title":"Literal map","description":"A rule scenario","nodes":{}}"#).unwrap();
    let absent = json!({"title":"Literal map","description":"A rule scenario","nodes":{}});
    assert_eq!(serde_json::from_str::<Value>(&get_over_tcp(addr, MAP).await.body).unwrap(), absent);

    let draft = get_over_tcp(addr, RULES).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    assert!(draft.get("map_input_rule").is_none());
    assert!(draft.get("map_output_rule").is_none());
    let saved = post_json_over_tcp(addr, RULES, &json!({
        "revision":draft["revision"],
        "map_input_rule":{"files":[{"id":"brief","required":true},{"id":"appendix","required":false,"description":"Extra facts"}]},
        "map_output_rule":{"files":[{"id":"brief"}]}
    }).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let expected = json!({
        "title":"Literal map", "description":"A rule scenario", "nodes":{},
        "map_input_rule":{"description":"","files":[{"id":"brief","required":true},{"id":"appendix","required":false,"description":"Extra facts"}]},
        "map_output_rule":{"description":"","files":[{"id":"brief"}]}
    });
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), expected);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(), expected);
    assert_eq!(serde_json::from_str::<Value>(&get_over_tcp(addr, MAP).await.body).unwrap(), expected);

    let draft: Value = serde_json::from_str(&get_over_tcp(addr, RULES).await.body).unwrap();
    let saved = post_json_over_tcp(addr, RULES, &json!({
        "revision":draft["revision"], "map_output_rule":{"description":"Hand the Chat back to the user."}
    }).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let expected = json!({"title":"Literal map","description":"A rule scenario","nodes":{},
        "map_output_rule":{"description":"Hand the Chat back to the user.","files":[]}});
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), expected);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(), expected);
    assert_eq!(serde_json::from_str::<Value>(&get_over_tcp(addr, MAP).await.body).unwrap(), expected);
}

// S1/S4, P7: the outer node uses its local input ID (not its source ID) and
// asks for only one of two public outputs. Matching a private ID must fail.
#[tokio::test]
async fn parent_node_saves_only_same_named_public_bindings_and_an_output_subset() {
    let s = Scenario::new().await;
    let addr = s.server.addr();
    let maps = &s.config.scheduler.map_directory;
    let bots = &s.config.scheduler.bot_directory;
    let mut bot: Value = serde_json::from_slice(&fs::read(bots.join("worker.json")).unwrap()).unwrap();
    bot.as_object_mut().unwrap().remove("task_id");
    bot["id"] = json!("child-interface");
    bot["map_file"] = json!("analysis.json");
    fs::write(bots.join("child-interface.json"), bot.to_string()).unwrap();
    fs::write(maps.join("analysis.json"), r#"{
        "title":"Independent child","description":"","map_input_rule":{"files":[{"id":"public-brief","required":true}]},
        "map_output_rule":{"files":[{"id":"published"},{"id":"appendix"}]},
        "nodes":{"private-node":{"title":"Private analysis","bot_id":"worker","input":"","zone":{"type":"dynamic"},"blocked_by":[],
            "input_rule":[{"id":"private-source","type":"run","run_file_ref":"public-brief"}],"output_rule":[{"id":"internal-result"}]}}
    }"#).unwrap();
    let path = maps.join("work.json");
    fs::write(&path, r#"{"title":"Parent","description":"","nodes":{}}"#).unwrap();
    let revision = |original| json!({"file":"work.json","original":original});
    let call = json!({"title":"Call child","bot_id":"child-interface","manager_task_id":s.manager_task,"input":"","zone":"node","blocked_by":[],
        "input_rule":[{"id":"public-brief","type":"file","path":"different-source.md"}],"output_rule":[{"id":"published"}]});
    let saved = post_json_over_tcp(addr, NODES, &json!({"id":"call","content":call.to_string(),"revision":revision(fs::read_to_string(&path).unwrap()),"create":true}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let expected = json!({"title":"Parent","description":"","nodes":{"call":{
        "title":"Call child","bot_id":"child-interface","manager_task_id":s.manager_task,"input":"","zone":"node","blocked_by":[],
        "input_rule":[{"id":"public-brief","type":"file","path":"different-source.md"}],"output_rule":[{"id":"published"}],
        "auto_approved":true,"auto_close":false,"end_node":false
    }}});
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap(), expected);
    let original = fs::read_to_string(&path).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&original).unwrap(), expected);
    for (rule, binding, invalid_id) in [("input_rule", json!({"id":"private-source","type":"file","path":"different-source.md"}), "private-source"),
        ("output_rule", json!({"id":"internal-result"}), "internal-result")] {
        let mut invalid = call.clone();
        invalid[rule] = json!([binding]);
        let rejected = post_json_over_tcp(addr, NODES, &json!({"id":"call","content":invalid.to_string(),"revision":revision(original.clone()),"create":false}).to_string()).await;
        assert!(rejected.status >= 400, "accepted {invalid_id}: {}", rejected.body);
        assert!(rejected.body.contains(invalid_id), "{}", rejected.body);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(serde_json::from_str::<Value>(&get_over_tcp(addr, MAP).await.body).unwrap(), expected);
    }
}

// S1/S4, P5/P29: a permissive Option/struct decoder could accept null or
// positional arrays. Every literal below must be rejected before writing.
#[tokio::test]
async fn malformed_map_rules_reject_save_without_changing_the_definition() {
    let s = Scenario::new().await;
    let addr = s.server.addr();
    let path = s.config.scheduler.map_directory.join("work.json");
    let original = r#"{
      "title":"Keep layout","description":"original text","map_output_rule":{"description":"original agreement"},"nodes":{}
    }
"#;
    fs::write(&path, original).unwrap();
    let before = get_over_tcp(addr, MAP).await;
    assert_eq!(before.status, 200, "{}", before.body);
    let cases = [
        r#"{"input_requirements":"legacy"}"#,
        r#"{"input_files":[]}"#,
        r#"{"unexpected":true}"#,
        r#"{"map_input_rule":null}"#,
        r#"{"map_output_rule":null}"#,
        r#"{"map_input_rule":["positional",[]]}"#,
        r#"{"map_output_rule":["positional",[]]}"#,
        r#"{"map_input_rule":{"description":null}}"#,
        r#"{"map_output_rule":{"description":42}}"#,
        r#"{"map_input_rule":{"files":null}}"#,
        r#"{"map_output_rule":{"files":{}}}"#,
        r#"{"map_input_rule":{"files":[["positional",true]]}}"#,
        r#"{"map_output_rule":{"files":[["positional"]]}}"#,
        r#"{"map_input_rule":{"files":[{"id":"brief"}]}}"#,
        r#"{"map_input_rule":{"files":[{"id":"brief","required":null}]}}"#,
        r#"{"map_input_rule":{"files":[{"id":"brief","required":"false"}]}}"#,
        r#"{"map_input_rule":{"files":[{"id":"brief","required":true,"description":null}]}}"#,
        r#"{"map_output_rule":{"files":[{"id":"result","description":null}]}}"#,
        r#"{"map_output_rule":{"files":[{"id":"result","required":false}]}}"#,
        r#"{"map_output_rule":{"files":[{"id":"result","path":"fixed.txt"}]}}"#,
        r#"{"map_input_rule":{"why":"unknown"}}"#,
        r#"{"map_output_rule":{"why":"unknown"}}"#,
        r#"{"map_input_rule":{"files":[{"id":" ","required":false}]}}"#,
        r#"{"map_output_rule":{"files":[{"id":""}]}}"#,
        r#"{"map_input_rule":{"files":[{"id":"brief","required":true},{"id":"brief","required":false}]}}"#,
        r#"{"map_output_rule":{"files":[{"id":"report"},{"id":"report"}]}}"#,
    ];
    for raw in cases {
        let mut candidate: Value = serde_json::from_str(raw).unwrap();
        candidate["revision"] = json!({"file":"work.json","original":original});
        let response = post_json_over_tcp(addr, RULES, &candidate.to_string()).await;
        assert!(matches!(response.status, 400 | 422), "{raw}: {} {}", response.status, response.body);
        assert_eq!(fs::read_to_string(&path).unwrap(), original, "{raw}");
        let current = get_over_tcp(addr, MAP).await;
        assert_eq!(current.status, 200, "{raw}: {}", current.body);
        assert_eq!(current.body, before.body, "{raw}");
    }
    let positional = json!([{}, {}, {"file":"work.json","original":original}]);
    let response = post_json_over_tcp(addr, RULES, &positional.to_string()).await;
    assert_eq!(response.status, 422, "{}", response.body);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert_eq!(get_over_tcp(addr, MAP).await.body, before.body);

    // The public definition reader independently rejects the same shapes from disk.
    for raw in cases {
        let mut candidate: Value = serde_json::from_str(raw).unwrap();
        candidate["title"] = json!("Unchecked file");
        candidate["description"] = json!("");
        candidate["nodes"] = json!({});
        fs::write(s.config.scheduler.map_directory.join("unchecked.json"), candidate.to_string()).unwrap();
        let response = get_over_tcp(addr, "/api/scheduler/maps/unchecked.json").await;
        assert!(response.status >= 400, "read accepted {raw}: {}", response.body);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}

// S1/S4, P6/P29: a material edge is not a graph edge. Save a dynamic consumer
// of all approved sources with no blocked_by entries, then try invalid edits.
#[tokio::test]
async fn node_save_preserves_sources_zone_and_policy_without_inventing_dependencies() {
    let s = Scenario::new().await;
    let addr = s.server.addr();
    let path = s.config.scheduler.map_directory.join("work.json");
    fs::write(&path, r#"{
      "title":"Wiring","description":"","map_input_rule":{"files":[{"id":"public","required":false}]},
      "nodes":{"provider":{"title":"Provide","bot_id":"worker","input":"","zone":"node","blocked_by":[],"output_rule":[{"id":"private"}]}}
    }"#).unwrap();
    let node = json!({"title":"Consume","bot_id":"worker","input":"","zone":{"type":"dynamic"},"blocked_by":[],
        "input_rule":[{"id":"direct","type":"file","path":"source/data.txt","zone_id":"node"},
            {"id":"internal","type":"node","source_node_ref":"provider","node_output_ref":"private"},
            {"id":"bound","type":"run","run_file_ref":"public"}],"output_rule":[{"id":"result"}],
        "auto_approved":false,"auto_close":true});
    let submit = |content: Value, original: &str, create| json!({"id":"consumer","content":content.to_string(),"revision":{"file":"work.json","original":original},"create":create}).to_string();
    let saved = post_json_over_tcp(addr, NODES, &submit(node.clone(), &fs::read_to_string(&path).unwrap(), true)).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let expected = json!({"title":"Wiring","description":"","map_input_rule":{"description":"","files":[{"id":"public","required":false}]},"nodes":{
        "provider":{"title":"Provide","bot_id":"worker","input":"","zone":"node","blocked_by":[],"input_rule":[],"output_rule":[{"id":"private"}],"auto_approved":true,"auto_close":false,"end_node":false},
        "consumer":{"title":"Consume","bot_id":"worker","input":"","zone":{"type":"dynamic"},"blocked_by":[],
            "input_rule":[{"id":"direct","type":"file","path":"source/data.txt","zone_id":"node"},
                {"id":"internal","type":"node","source_node_ref":"provider","node_output_ref":"private"},
                {"id":"bound","type":"run","run_file_ref":"public"}],"output_rule":[{"id":"result"}],
            "auto_approved":false,"auto_close":true,"end_node":false}
    }});
    let original = fs::read_to_string(&path).unwrap();
    for body in [&saved.body, &get_over_tcp(addr, MAP).await.body, &original] {
        assert_eq!(serde_json::from_str::<Value>(body).unwrap(), expected);
    }
    for (field, value) in [
        ("zone", json!(["dynamic"])), ("zone", json!({"type":"dynamic","zone_id":"node"})),
        ("input", json!(null)), ("parameters", json!({})), ("misspelled", json!(true)),
        ("auto_close", json!("true")), ("blocked_by", json!(["consumer"])), ("blocked_by", json!(["missing"])),
        ("input_rule", json!([{"id":"direct","type":"file","path":"../elsewhere.txt"}])),
        ("input_rule", json!([{"id":"internal","type":"node","source_node_ref":"provider","node_output_ref":"missing"}])),
        ("input_rule", json!([{"id":"bound","type":"run","run_file_ref":"missing"}])),
        ("input_rule", json!([{"id":"bound","type":"run","input_id":"public"}])),
        ("input_rule", json!([{"id":"internal","type":"node","node_id":"provider","node_output_ref":"private"}])),
        ("input_rule", json!([{"id":"internal","type":"node","source_node_ref":"provider","output_id":"private"}])),
        ("output_rule", json!([{"id":"duplicate"},{"id":"duplicate"}])),
    ] {
        let mut invalid = node.clone();
        invalid[field] = value;
        let response = post_json_over_tcp(addr, NODES, &submit(invalid.clone(), &original, false)).await;
        assert!(response.status >= 400, "accepted {invalid}: {}", response.body);
        assert_eq!(fs::read_to_string(&path).unwrap(), original, "{invalid}");
        assert_eq!(get_over_tcp(addr, MAP).await.body, saved.body, "{invalid}");
    }
    assert!(s.herdr.calls().is_empty());
}
