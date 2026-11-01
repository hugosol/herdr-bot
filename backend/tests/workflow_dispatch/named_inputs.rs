//! S1: public Map save/read and Run dispatch; external host captures actual input.
use super::*;

async fn draft(s: &Scenario) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn save(s: &Scenario, draft: &Value, inputs: Value) -> common::HttpResponse {
    let mut content = draft["content"].clone();
    content["inputs"] = inputs;
    post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await
}

// P1/P2/P3: literals from the approved source forms. Dropping inputs or turning
// a source reference into a dependency would disagree with the public readback.
#[tokio::test]
async fn named_inputs_round_trip_without_dependencies_or_implicit_launch() {
    let s = Scenario::new().await;
    let inputs = json!({
        "same_volume":{"source":"file","path":"C:\\handoff\\not-created.csv"},
        "other_volume":{"source":"file","path":"Z:\\external\\result.txt"},
        "outside_zone":{"source":"file","path":"C:\\outside-any-zone\\brief.md"},
        "summary":{"source":"node","node_id":"B","output":"review"}
    });
    let response = save(&s, &draft(&s).await, inputs.clone()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    let map: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(map["nodes"]["A"]["inputs"], inputs);
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
        json!({"brief":{"source":"node","node_id":"typo","output":"review"}}),
        json!({"brief":{"source":"file","path":"relative/brief.md"}}),
        json!({"brief":{"source":"file","path":"C:brief.md"}}),
        json!({"brief":{"source":"file","path":"\\brief.md"}}),
        json!({"brief":{"source":"file","path":"C:\\brief\u{0}.md"}}),
        json!({"brief":{"source":"file","path":12}}),
        json!({"brief":{"source":"node","node_id":false,"output":"review"}}),
        json!({"brief":{"source":"node","node_id":"B","output":[]}}),
        json!({"brief":{"source":"file","path":"C:\\brief.md","node_id":"B","output":"review"}}),
        json!({"brief":{"path":"C:\\brief.md"}}),
        json!({"brief":{"source":"FILE","path":"C:\\brief.md"}}),
        json!({"brief":{"source":false,"path":"C:\\brief.md"}}),
        json!({"brief":{"source":"node","node_id":"B"}}),
        json!({"brief":["file","C:\\brief.md"]}),
        json!({"brief":null}), json!([]), Value::Null,
    ] {
        let response = save(&s, &draft, inputs.clone()).await;
        assert_eq!(response.status, 500, "candidate={inputs} response={}", response.body);
        let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
        assert_eq!(read.status, 200, "{}", read.body);
        assert_eq!(read.body, original, "invalid candidate changed original: {inputs}");
    }
    assert!(s.herdr.calls().is_empty());
}

// P1: unchanged legacy source form is still saved and explicitly dispatched;
// there is no requirement to invent a file input or preallocate a result.
#[tokio::test]
async fn legacy_node_without_file_inputs_saves_and_dispatches() {
    let s = Scenario::new().await;
    let draft = draft(&s).await;
    assert!(draft["content"].get("inputs").is_none());
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":draft["content"].to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert!(s.herdr.calls().is_empty());
    s.start("legacy-named-manager").await;
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(), 1);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"legacy-named-node"},"native_cwd":s.data.path().join("node-zone")}));
    let result = s.dispatch("legacy-named-manager", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "running");
    assert!(!s.data.path().join(".workflow").exists());
    assert_eq!(fs::read_dir(s.data.path().join("node-zone")).unwrap().count(), 0);
}

// P3: existing editing consumers must not leave a dangling named source.
#[tokio::test]
async fn deleting_a_named_input_source_is_rejected_without_cascading() {
    let s = Scenario::new().await;
    let saved = save(&s, &draft(&s).await, json!({"brief":{"source":"node","node_id":"C","output":"summary"}})).await;
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
    let inputs = json!({"brief":{"source":"file","path":file},"remote":{"source":"file","path":"Z:\\business\\not-created.txt"}});
    let response = save(&s, &draft(&s).await, inputs).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let first = s.start("named-manager-first").await;
    let second = s.start("named-manager-second").await;
    assert_ne!(first["id"], second["id"]);
    assert_eq!(s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count(), 2, "Run creation launches only managers");
    let upstream_file = s.data.path().join("upstream.jsonl");
    fs::write(&upstream_file, format!("{}\n{}\n", json!({"type":"session","id":"named-upstream","cwd":s.data.path().join("node-zone")}), json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"UPSTREAM-CHAT-SENSITIVE-7358"}]}}))).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":upstream_file},"native_cwd":s.data.path().join("node-zone")}));
    assert_eq!(s.dispatch("named-manager-first", &["B"]).await["results"][0]["status"], "running");
    for (manager, started, identity) in [("named-manager-first", &first, "named-node-first"), ("named-manager-second", &second, "named-node-second")] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
        let result = s.dispatch(manager, &["A"]).await;
        assert_eq!(result["results"][0]["status"], "running");
        let calls = s.herdr.calls();
        let text = calls.iter().rev().find(|c| c["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap();
        let materials = s.data.path().join(".workflow").join(started["id"].as_str().unwrap());
        assert!(text.contains("MANAGER-USER-SECRET"), "Run background missing: {text}");
        assert!(text.contains("NODE-INPUT\n第二行"), "business input missing: {text}");
        assert!(text.contains(file.to_str().unwrap()), "direct file address missing: {text}");
        assert!(text.contains("Z:\\business\\not-created.txt"));
        assert!(text.contains(materials.to_str().unwrap()), "manager-zone Run materials location missing: {text}");
        assert!(!materials.starts_with(s.data.path().join("node-zone")));
        for secret in ["FILE-BODY-SENSITIVE-9204", "UPSTREAM-CHAT-SENSITIVE-7358", "BOT-NODE-SECRET", "MAP-SECRET"] {
            assert!(!text.contains(secret), "unexpected body injected: {secret}");
        }
        println!("P4/P24 dispatchRequest={} result={result} actualHostInput={text}", json!({"sessionId":manager,"nodes":["A"]}));
    }
    assert!(!s.data.path().join(".workflow").exists(), "location is guidance, not a directory/output preallocation protocol");
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
    let response = save(&s, &draft, json!({"brief":{"source":"file","path":"Z:\\external\\brief.md"}})).await;
    fs::set_permissions(&path, permissions).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], "node-write-failed");
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(read.body, original);
    assert!(s.herdr.calls().is_empty());
}
