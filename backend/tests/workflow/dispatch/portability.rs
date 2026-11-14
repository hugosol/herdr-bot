use super::*;

fn edit_map(s: &Scenario, edit: impl FnOnce(&mut Value)) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut map);
    fs::write(path, serde_json::to_vec_pretty(&map).unwrap()).unwrap();
}

#[tokio::test]
async fn dispatch_uses_start_snapshot_after_rebind_and_unregister() {
    let s = Scenario::new().await;
    let started = s.start("manager-frozen-zones").await;
    let original = s.data.path().join("node-zone");
    let other = s.data.path().join("new-node-zone");
    fs::create_dir(&other).unwrap();
    let response = common::request_json_over_tcp(s.server.addr(), "PUT", "/api/zones", Some(&json!({"id":"node","path":other}).to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let response = common::request_json_over_tcp(s.server.addr(), "DELETE", "/api/zones", Some(&json!({"id":"node"}).to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-frozen-A"},"native_cwd":original}));
    let dispatched = s.dispatch("manager-frozen-zones", &["A"]).await;
    assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
    let record: Value = serde_json::from_str(&fs::read_to_string(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(record["zoneBindings"]["node"], json!(original));
    assert_eq!(record["nodes"]["A"]["execution"]["zone"], json!(original));
    assert!(s.herdr.calls().iter().any(|call| call["kind"] == "tab_create" && call["cwd"] == json!(original)), "external cwd must remain frozen: {:?}", s.herdr.calls());
    let rejected = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/preflight", &json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":""}).to_string()).await;
    assert_eq!(rejected.status, 400, "new runs must re-resolve registration: {}", rejected.body);
}

#[tokio::test]
async fn frozen_directory_disappearance_rejects_dispatch_without_launch() {
    let s = Scenario::new().await;
    s.start("manager-gone-zone").await;
    fs::remove_dir(s.data.path().join("node-zone")).unwrap();
    let before = s.herdr.calls();
    let result = s.dispatch("manager-gone-zone", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "failed", "{result}");
    assert_eq!(s.herdr.calls(), before);
}

#[tokio::test]
async fn omitted_file_zone_uses_manager_and_explicit_zone_uses_its_snapshot() {
    let s = Scenario::new().await;
    fs::write(s.data.path().join("source.txt"), "manager input").unwrap();
    fs::write(s.data.path().join("node-zone").join("source.txt"), "different node input").unwrap();
    edit_map(&s, |map| map["nodes"]["A"]["input_rule"] = json!([
        {"type":"file","id":"default","path":"source.txt"},
        {"type":"file","id":"explicit","path":"source.txt","zone_id":"node"}
    ]));
    s.start("manager-relative-input").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-relative-input"},"native_cwd":s.data.path().join("node-zone")}));
    let result = s.dispatch("manager-relative-input", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "running", "{result}");
    let input = s.herdr.calls().into_iter().filter(|call| call["kind"] == "agent_prompt").last().unwrap()["text"].as_str().unwrap().to_owned();
    assert!(input.contains(&format!("default: {}", s.data.path().join("source.txt").display())), "{input}");
    assert!(input.contains(&format!("explicit: {}", s.data.path().join("node-zone").join("source.txt").display())), "{input}");
}

#[tokio::test]
async fn absolute_escaping_and_backslash_file_rules_are_rejected_before_run_allocation() {
    let s = Scenario::new().await;
    for path in ["../escape.txt", "/absolute.txt", "C:/absolute.txt", "folder\\\\file.txt", "folder/../escape.txt"] {
        edit_map(&s, |map| map["nodes"]["A"]["input_rule"] = json!([{ "type":"file", "id":"input", "path":path }]));
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":""}).to_string()).await;
        assert_eq!(response.status, 400, "{path}: {}", response.body);
        assert!(s.herdr.calls().is_empty(), "unsafe file definition must not launch: {path}");
    }
}

#[tokio::test]
async fn file_disappearing_after_start_fails_before_node_prompt() {
    let s = Scenario::new().await;
    edit_map(&s, |map| map["nodes"]["A"]["input_rule"] = json!([{ "type":"file", "id":"input", "path":"not-created.txt" }]));
    s.start("manager-late-file").await;
    let before = s.herdr.calls();
    let result = s.dispatch("manager-late-file", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "failed", "{result}");
    assert_eq!(s.herdr.calls(), before, "file checks must precede node launch");
}

#[tokio::test]
async fn historical_absolute_input_rules_remain_readable_without_binding_snapshot() {
    let s = Scenario::new().await;
    let started = s.start("manager-historical-input").await;
    let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
    let mut run: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    run.as_object_mut().unwrap().remove("managerZoneId");
    run.as_object_mut().unwrap().remove("zoneBindings");
    run["nodes"]["A"]["execution"] = json!({"id":"historical-execution","taskId":"historical-task","agent":"omp","zone":s.data.path(),"inputRule":[{"type":"file","id":"old","path":s.data.path().join("historical.txt")}],"outputRule":[],"inputs":[],"reportPolicy":"auto","autoClose":false,"chat":null});
    fs::write(path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let response: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(response["run"]["nodes"]["A"]["execution"]["inputRule"][0]["path"], json!(s.data.path().join("historical.txt")));
}

#[tokio::test]
async fn node_dictionary_reads_latest_and_does_not_expand_run_user_input() {
    let s = Scenario::new().await;
    edit_map(&s, |map| {
        map["nodes"]["A"]["input"] = json!("node ${location}");
        map["nodes"]["B"]["input"] = json!("next ${location}");
    });
    let mut tasks: Value = serde_json::from_str(&fs::read_to_string(&s.config.tasks.storage_file).unwrap()).unwrap();
    for task in tasks["tasks"].as_array_mut().unwrap() { task["prompt"] = json!("fixed ${location}"); }
    fs::write(&s.config.tasks.storage_file, serde_json::to_vec_pretty(&tasks).unwrap()).unwrap();
    fs::write(&s.config.prompts_file, "location = 'first'\n").unwrap();
    let started = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":"literal ${not_in_dictionary}"}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    let manager_prompt = s.herdr.calls().into_iter().find(|call| call["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap().to_owned();
    assert!(manager_prompt.contains("fixed first"), "{manager_prompt}");
    assert!(manager_prompt.contains("literal ${not_in_dictionary}"), "{manager_prompt}");
    fs::write(&s.config.prompts_file, "location = 'second'\n").unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-latest-dictionary"},"native_cwd":s.data.path().join("node-zone")}));
    let result = s.dispatch("manager-05", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "running", "{result}");
    let prompt = s.herdr.calls().into_iter().filter(|call| call["kind"] == "agent_prompt").last().unwrap()["text"].as_str().unwrap().to_owned();
    assert!(prompt.contains("node second"), "{prompt}");
    assert!(prompt.contains("fixed second"), "{prompt}");
    assert!(prompt.contains("literal ${not_in_dictionary}"), "{prompt}");
    edit_map(&s, |map| map["nodes"]["B"]["input"] = json!("${missing_node_key}"));
    fs::write(&s.config.prompts_file, "location = 'third'\n").unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-missing-dictionary"},"native_cwd":s.data.path().join("node-zone")}));
    let prompts_before = s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count();
    let rejected = s.dispatch("manager-05", &["B"]).await;
    assert_eq!(rejected["results"][0]["status"], "failed", "{rejected}");
    assert!(rejected["results"][0]["failure"]["message"].as_str().unwrap().contains("node.input"));
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count(), prompts_before);
}
