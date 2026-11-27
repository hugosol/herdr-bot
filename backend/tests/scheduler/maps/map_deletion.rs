use super::*;

fn records(root: &std::path::Path) -> [Vec<u8>; 4] {
    ["run.json", "history.jsonl", "messages.jsonl", "map.json"].map(|file| fs::read(root.join(file)).unwrap())
}

#[tokio::test]
async fn retain_history_keeps_live_run_details_reports_and_resources_without_map() {
    let s = Scenario::new().await;
    let started = s.start("retain-manager").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retain-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("retain-manager", &["A"]).await;
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"retain-node","body":"Keep this result","canComplete":true}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let root = s.config.scheduler.run_directory.join(id);
    let before = records(&root);
    let resources = [&s.config.tasks.storage_file, &s.config.zones.registry_file];
    let resource_bytes = resources.map(|path| fs::read(path).unwrap());
    let bot = fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap();
    let calls = s.herdr.calls();
    let legacy = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", r#"{"deleteHistory":true}"#).await;
    assert_eq!(legacy.status, 400, "obsolete deleteHistory must be rejected: {}", legacy.body);
    let snapshot = fs::read(root.join("map.json")).unwrap();
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", "{}").await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(fs::read(root.join("map.json")).unwrap(), snapshot);
    let listed: Value = serde_json::from_str(&get_over_tcp(s.server.addr(), "/api/workflow/runs").await.body).unwrap();
    assert_eq!(listed["runs"][0]["id"], id);
    let graph = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}/map")).await;
    assert_eq!(graph.status, 200, "{}", graph.body);
    assert_eq!(serde_json::from_str::<Value>(&graph.body).unwrap()["nodes"]["A"]["title"], "Worker A");
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["files"], json!([]));
    assert!(!s.config.scheduler.map_directory.join("work.json").exists());
    assert_eq!(records(&root), before);
    let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert_eq!(detail["run"]["status"], "running");
    assert_eq!(detail["run"]["nodes"]["A"]["status"], "running");
    assert_eq!(detail["reports"][0]["body"], "Keep this result");
    assert_eq!(resources.map(|path| fs::read(path).unwrap()), resource_bytes);
    assert_eq!(fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap(), bot);
    assert_eq!(s.herdr.calls(), calls, "deletion must never stop or otherwise call Chat hosts");
}


#[tokio::test]
async fn map_delete_rejects_unsafe_names_missing_files_and_invalid_body_with_json_errors() {
    let s = Scenario::new().await;
    let file = s.config.scheduler.map_directory.join("work.json");
    let original = fs::read(&file).unwrap();
    for name in ["missing.json", "work.txt", "..%5Cwork.json", "work.json%3Astream"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{name}/delete"), "{}").await;
        assert_ne!(response.status, 200, "{name}");
        let error: Value = serde_json::from_str(&response.body).unwrap();
        assert!(error["message"].is_string(), "{name}: {error}");
    }
    for body in ["[]", "null", r#"{"deleteHistory":false}"#, r#"{"extra":true}"#] {
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", body).await;
        assert_eq!(response.status, 400, "{body}: {}", response.body);
        assert!(serde_json::from_str::<Value>(&response.body).unwrap()["message"].is_string());
    }
    assert_eq!(fs::read(&file).unwrap(), original);
    fs::write(&file, b"broken Map JSON").unwrap();
    let discarded = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", "{}").await;
    assert_eq!(discarded.status, 200, "{}", discarded.body);
    assert!(!file.exists());
}
