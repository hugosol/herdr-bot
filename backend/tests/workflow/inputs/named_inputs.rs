//! S1: public Map save/read and Run dispatch; external host captures actual input.
use super::*;

async fn draft(s: &Scenario) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn save(s: &Scenario, draft: &Value, inputs: Value) -> common::HttpResponse {
    let mut content = draft["content"].clone();
    content["input_rule"] = inputs;
    post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await
}

// P1/P2/P3: literals from the approved source forms. Dropping inputs or turning
// a source reference into a dependency would disagree with the public readback.
#[tokio::test]
async fn named_inputs_round_trip_without_dependencies_or_implicit_launch() {
    let s = Scenario::new().await;
    let inputs = json!([{"id":"manager_file","type":"file","path":"handoff/not-created.csv"}, {"id":"node_file","type":"file","path":"external/result.txt","zone_id":"node"}, {"id":"unregistered_file","type":"file","path":"brief.md","zone_id":"unregistered"}, {"id":"summary","type":"node","source_node_ref":"B","node_output_ref":"review"}]);
    let response = save(&s, &draft(&s).await, inputs.clone()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    let map: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(map["nodes"]["A"]["input_rule"], inputs);
    assert_eq!(map["nodes"]["A"]["blocked_by"], json!([]));
    assert_eq!(map["nodes"]["C"]["blocked_by"], json!(["A","B"]));
    assert!(s.herdr.calls().is_empty(), "saving and reading cannot start Chats");
    println!("P1/P2/P3 save={} read={} hostCalls={:?}", response.body, read.body, s.herdr.calls());
}

// P1/P2/P3: a source must be explicit and well typed, and references must name
// an existing node. Accepting/coercing any candidate would replace the original.
#[tokio::test]
async fn invalid_named_inputs_preserve_original_map() {
    let s = Scenario::new().await;
    let original = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body;
    let draft = draft(&s).await;
    for inputs in [
        json!([{"id":"brief","type":"node","source_node_ref":"typo","node_output_ref":"review"}]),
        json!([{"id":"brief","type":"file","path":"../brief.md"}]),
        json!([{"id":"brief","type":"file","path":"C:/absolute/brief.md"}]),
        json!([{"id":"brief","type":"file","path":"/absolute/brief.md"}]),
        json!([{"id":"brief","type":"file","path":"relative\\brief.md"}]),
        json!([{"id":"brief","type":"file","path":"brief.md","zone_id":12}]),
        json!([{"id":"brief","type":"file","path":"C:brief.md"}]),
        json!([{"id":"brief","type":"file","path":"\\brief.md"}]),
        json!({"brief":{"source":"file","path":"C:\\brief\u{0}.md"}}),
        json!([{"id":"brief","type":"file","path":12}]),
        json!([{"id":"brief","type":"node","source_node_ref":false,"node_output_ref":"review"}]),
        json!([{"id":"brief","type":"node","source_node_ref":"B","node_output_ref":[]}]),
        json!([{"id":"brief","type":"file","path":"brief.md","source_node_ref":"B","node_output_ref":"review"}]),
        json!({"brief":{"path":"C:\\brief.md"}}),
        json!([{"id":"brief","type":"FILE","path":"C:\\brief.md"}]),
        json!([{"id":"brief","type":false,"path":"C:\\brief.md"}]),
        json!([{"id":"brief","type":"node","source_node_ref":"B"}]),
        json!({"brief":["file","C:\\brief.md"]}),
        json!({"brief":null}), Value::Null,
        json!([{"id":"","type":"file","path":"brief.md"}]),
        json!([{"id":"brief","type":"file","path":"one"},{"id":"brief","type":"file","path":"two"}]),
        json!([{"id":"brief","type":"node","source_node_ref":"B","node_output_ref":"undeclared"}]),
    ] {
        let response = save(&s, &draft, inputs.clone()).await;
        assert_eq!(response.status, 500, "candidate={inputs} response={}", response.body);
        let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
        assert_eq!(read.status, 200, "{}", read.body);
        assert_eq!(read.body, original, "invalid candidate changed original: {inputs}");
    }
    assert!(s.herdr.calls().is_empty());
}

// P1: omitted rules default to empty arrays without inventing file inputs.
#[tokio::test]
async fn omitted_rules_default_to_empty_arrays_and_dispatch() {
    let s = Scenario::new().await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["A"].as_object_mut().unwrap().remove("output_rule");
    fs::write(path, map.to_string()).unwrap();
    let draft = draft(&s).await;
    assert_eq!(draft["content"]["input_rule"], json!([]));
    assert_eq!(draft["content"]["output_rule"], json!([]));
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":draft["content"].to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert!(s.herdr.calls().is_empty());
    s.start("legacy-named-manager").await;
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(), 1);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"legacy-named-node"},"native_cwd":s.data.path().join("node-zone")}));
    let result = s.dispatch("legacy-named-manager", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "running");
    let execution = &result["snapshot"]["nodes"]["A"]["execution"];
    for field in ["inputRule", "outputRule", "inputs"] {
        assert_eq!(execution[field], json!([]), "{field}");
    }
    assert!(!s.data.path().join(".manager-zone").join(".workflow").exists());
    assert_eq!(fs::read_dir(s.data.path().join("node-zone")).unwrap().count(), 0);
}

#[tokio::test]
async fn output_rule_ids_and_referenced_declarations_are_validated_atomically() {
    let s = Scenario::new().await;
    let saved = save(&s, &draft(&s).await, json!([{"id":"brief","type":"node","source_node_ref":"B","node_output_ref":"review"}])).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"B"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let draft: Value = serde_json::from_str(&response.body).unwrap();
    let before = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body;
    for outputs in [
        json!([]),
        json!([{"id":""}]),
        json!([{"id":" "}]),
        json!([{"id":"review"},{"id":"review"}]),
        json!([["review"]]),
        json!({"review":{}}),
        Value::Null,
    ] {
        let mut content = draft["content"].clone();
        content["output_rule"] = outputs;
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"B","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body, before);
    }
    assert!(s.herdr.calls().is_empty());
}

// P3: existing editing consumers must not leave a dangling named source.
#[tokio::test]
async fn deleting_a_named_input_source_is_rejected_without_cascading() {
    let s = Scenario::new().await;
    let saved = save(&s, &draft(&s).await, json!([{"id":"brief","type":"node","source_node_ref":"C","node_output_ref":"summary"}])).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let current = draft(&s).await;
    let checked = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/check-delete", &json!({"id":"C"}).to_string()).await;
    assert_eq!(checked.status, 500, "{}", checked.body);
    assert_eq!(serde_json::from_str::<Value>(&checked.body).unwrap()["code"], "node-referenced");
    let deleted = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/delete", &json!({"id":"C","revision":current["revision"]}).to_string()).await;
    assert_eq!(deleted.status, 500, "{}", deleted.body);
    assert_eq!(serde_json::from_str::<Value>(&deleted.body).unwrap()["code"], "node-referenced");
    assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body, saved.body);
}

// P4/P24: background and business input are independent literals. An upstream
// native Chat and a direct file contain different sensitive markers; copying
// either body instead of just addresses would be observable at the real host.
#[tokio::test]
async fn dispatch_receives_background_direct_addresses_and_run_scoped_material_location() {
    let s = Scenario::new().await;
    let file = s.data.path().join("private-source.txt");
    fs::write(&file, "FILE-BODY-SENSITIVE-9204").unwrap();
    let remote = s.data.path().join("node-zone").join("remote.txt");
    fs::write(&remote, "REMOTE-FILE-SENSITIVE").unwrap();
    let inputs = json!([{"id":"brief","type":"file","path":"private-source.txt"}, {"id":"remote","type":"file","path":"remote.txt","zone_id":"node"}]);
    let rules = s.data.path().join("delivery-rules.md");
    fs::write(&rules, "BUSINESS-RULES-BODY-4935").unwrap();
    fs::write(&s.config.prompts_file, format!("business_rules = '{}'\n", rules.display())).unwrap();
    let mut editor = draft(&s).await;
    editor["content"]["input"] = json!("NODE-INPUT\n第二行\n交付本阶段成果前，读取 ${business_rules}，核对本节点成果是否满足交接条件。");
    let response = save(&s, &editor, inputs).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let definition = fs::read(&map_path).unwrap();
    let first = s.start("named-manager-first").await;
    let second = s.start("named-manager-second").await;
    assert_ne!(first["id"], second["id"]);
    let submitted = || common::read_json_lines(&s.herdr.path().join("submitted.jsonl"));
    let managers = submitted();
    assert_eq!(managers.len(), 2, "Run creation launches only managers");
    for submission in managers {
        let text = submission["text"].as_str().unwrap();
        assert!(text.contains("业务执行和成果检查由节点承担"));
        assert!(!text.contains("delivery-rules.md") && !text.contains("BUSINESS-RULES-BODY-4935") && !text.contains("MAP-SECRET"));
    }
    let upstream_file = s.data.path().join("upstream.jsonl");
    fs::write(&upstream_file, format!("{}\n{}\n", json!({"type":"session","id":"named-upstream","cwd":s.data.path().join("node-zone")}), json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"UPSTREAM-CHAT-SENSITIVE-7358"}]}}))).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":upstream_file},"native_cwd":s.data.path().join("node-zone")}));
    assert_eq!(s.dispatch("named-manager-first", &["B"]).await["results"][0]["status"], "running");
    for (manager, started, identity) in [("named-manager-first", &first, "named-node-first"), ("named-manager-second", &second, "named-node-second")] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
        let result = s.dispatch(manager, &["A"]).await;
        assert_eq!(result["results"][0]["status"], "running");
        let submissions = submitted();
        let text = submissions.last().unwrap()["text"].as_str().unwrap();
        let materials = s.data.path().join(".manager-zone").join(".workflow").join(started["id"].as_str().unwrap());
        assert!(text.starts_with("/skill:teach NODE-FIXED\n\n"), "implementation prompt/Skill missing: {text}");
        assert!(text.contains("[Run background / runInput]\nMANAGER-USER-SECRET\n[/Run background / runInput]"), "Run background missing: {text}");
        assert!(text.contains(&format!("[Business input]\nNODE-INPUT\n第二行\n交付本阶段成果前，读取 {}，核对本节点成果是否满足交接条件。", rules.display())), "condition and reference missing: {text}");
        assert!(text.contains("[Output rules]\n[{\"id\":\"A\"},{\"id\":\"B\"},{\"id\":\"otherVolume\"},{\"id\":\"result\"},{\"id\":\"brief\"},{\"id\":\"shared\"}]\n[/Output rules]"));
        assert!(text.contains(file.to_str().unwrap()), "direct file address missing: {text}");
        assert!(text.contains(remote.to_str().unwrap()));
        assert!(text.contains(materials.to_str().unwrap()), "manager-zone Run materials location missing: {text}");
        assert!(!materials.starts_with(s.data.path().join("node-zone")));
        for secret in ["FILE-BODY-SENSITIVE-9204", "UPSTREAM-CHAT-SENSITIVE-7358", "BOT-WHEN-SECRET", "BOT-PLAN-SECRET", "BOT-NODE-SECRET", "BOT-OUTPUT-SECRET", "MAP-SECRET", "BUSINESS-RULES-BODY-4935"] {
            assert!(!text.contains(secret), "unexpected body injected: {secret}");
        }
        println!("P4/P24 dispatchRequest={} result={result} actualHostInput={text}", json!({"sessionId":manager,"nodes":["A"]}));
    }
    assert_eq!(fs::read(&map_path).unwrap(), definition);
    for run in [&first, &second] {
        assert_eq!(fs::read(s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("map.json")).unwrap(), definition);
    }
    assert!(!s.data.path().join(".manager-zone").join(".workflow").exists(), "location is guidance, not a directory/output preallocation protocol");
    assert_eq!(fs::read_to_string(file).unwrap(), "FILE-BODY-SENSITIVE-9204");
}

// P1: filesystem denies publication, not a bad candidate; the API must expose
// failure and keep the original definition available after the fault is removed.
#[tokio::test]
async fn named_input_storage_failure_keeps_original_definition() {
    let s = Scenario::new().await;
    let original = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body;
    let draft = draft(&s).await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let response = save(&s, &draft, json!([{"id":"brief","type":"file","path":"external/brief.md"}])).await;
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], "node-write-failed");
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(read.body, original);
    assert!(s.herdr.calls().is_empty());
}
