use super::*;

fn records(root: &std::path::Path) -> [Vec<u8>; 3] {
    ["run.json", "history.jsonl", "messages.jsonl"].map(|file| fs::read(root.join(file)).unwrap())
}

async fn delete(s: &Scenario, name: &str, history: bool) -> common::HttpResponse {
    post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{name}/delete"),
        &json!({"deleteHistory":history}).to_string()).await
}

#[tokio::test]
async fn retain_history_keeps_live_run_details_reports_and_resources_without_map() {
    let s = Scenario::new().await;
    let started = s.start("retain-manager").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retain-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("retain-manager", &["A"]).await;
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"retain-node","body":"Keep this result"}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let root = s.config.scheduler.run_directory.join(id);
    let before = records(&root);
    let resources = [&s.config.tasks.storage_file, &s.config.zones.registry_file];
    let resource_bytes = resources.map(|path| fs::read(path).unwrap());
    let bot = fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap();
    let calls = s.herdr.calls();
    let response = delete(&s, "work.json", false).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"files":[]}));
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
async fn delete_history_removes_live_and_persisted_statuses_but_not_unrelated_runs_or_resources() {
    let s = Scenario::new().await;
    let started = s.start("delete-manager").await;
    let live_id = started["id"].as_str().unwrap();
    let run_root = &s.config.scheduler.run_directory;
    for (id, map, status) in [("persisted-running", "work.json", "running"), ("persisted-paused", "work.json", "paused"), ("persisted-completed", "work.json", "completed"), ("other-run", "other.json", "running")] {
        let mut run = started["run"].clone();
        run["id"] = json!(id);
        run["map"] = json!(map);
        run["status"] = json!(status);
        let root = run_root.join(id);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("run.json"), run.to_string()).unwrap();
        fs::write(root.join("history.jsonl"), b"{\"action\":\"fixture\"}\n").unwrap();
        fs::write(root.join("messages.jsonl"), b"").unwrap();
        if id == "persisted-paused" {
            fs::write(root.join("history.jsonl"), b"damaged detail without newline").unwrap();
            fs::write(root.join("messages.jsonl"), b"broken report").unwrap();
        }
    }
    let unrelated = records(&run_root.join("other-run"));
    let resources = [&s.config.tasks.storage_file, &s.config.zones.registry_file];
    let before = resources.map(|path| fs::read(path).unwrap());
    let bot = fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap();
    fs::write(s.config.scheduler.map_directory.join("other.json"), b"{}").unwrap();
    // Broken definitions remain deletable; association is the filename, not JSON validity.
    fs::write(s.config.scheduler.map_directory.join("work.json"), b"not valid JSON\xff").unwrap();
    let calls = s.herdr.calls();
    let response = delete(&s, "work.json", true).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"files":["other.json"]}));
    for id in [live_id, "persisted-running", "persisted-paused", "persisted-completed"] {
        assert!(!run_root.join(id).exists(), "{id} history must be gone");
    }
    assert_eq!(records(&run_root.join("other-run")), unrelated);
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(list.status, 200, "{}", list.body);
    let list: Value = serde_json::from_str(&list.body).unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["id"], "other-run");
    assert_eq!(resources.map(|path| fs::read(path).unwrap()), before);
    assert_eq!(fs::read(s.config.scheduler.bot_directory.join("worker.json")).unwrap(), bot);
    assert_eq!(s.herdr.calls(), calls);
}

#[tokio::test]
async fn deletion_during_node_dispatch_prevents_late_state_or_history_resurrection() {
    let s = Scenario::new().await;
    let started = s.start("inflight-manager").await;
    let id = started["id"].as_str().unwrap();
    let release = s.data.path().join("delete-dispatch-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"inflight-node"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let pending = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"inflight-manager","nodes":["A"]}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 2).await);
    let calls = s.herdr.calls();
    let response = delete(&s, "work.json", true).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert!(!s.config.scheduler.run_directory.join(id).exists());
    assert_eq!(s.herdr.calls(), calls);
    fs::write(release, b"release").unwrap();
    let _response = tokio::time::timeout(Duration::from_secs(5), pending.response()).await.unwrap();
    assert!(!s.config.scheduler.run_directory.join(id).exists());
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&list.body).unwrap()["runs"], json!([]));
    assert_eq!(s.herdr.calls(), calls, "late failure cannot stop Chats or retry work");
}

#[tokio::test]
async fn deletion_during_manager_identity_does_not_reinsert_active_failure_or_recreate_records() {
    let s = Scenario::new().await;
    let release = s.data.path().join("delete-identity-release");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"deleted-startup"},"native_cwd":s.data.path(),"native_session_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let pending = common::OpenJsonRequest::submit(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().any(|c| c["kind"] == "agent_get")).await);
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    let list: Value = serde_json::from_str(&list.body).unwrap();
    let id = list["runs"][0]["id"].as_str().unwrap();
    let response = delete(&s, "work.json", true).await;
    assert_eq!(response.status, 200, "{}", response.body);
    fs::write(release, b"release").unwrap();
    let response = tokio::time::timeout(Duration::from_secs(5), pending.response()).await.unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(!s.config.scheduler.run_directory.join(id).exists());
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&list.body).unwrap()["runs"], json!([]));
    assert!(s.herdr.calls().iter().all(|c| c["kind"] != "agent_prompt" && c["kind"] != "agent_stop"));
}

#[tokio::test]
async fn failed_history_association_scan_reports_error_without_claiming_map_deletion() {
    let s = Scenario::new().await;
    let started = s.start("delete-scan-failure").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = records(&root);
    let unknown = s.config.scheduler.run_directory.join("unidentifiable-run");
    fs::create_dir(&unknown).unwrap();
    fs::write(unknown.join("run.json"), b"broken association").unwrap();
    let response = delete(&s, "work.json", true).await;
    assert_eq!(response.status, 500, "{}", response.body);
    let error: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(error["code"], "map-history-delete-failed");
    assert!(error["message"].as_str().unwrap().contains("Map 文件尚未删除"));
    assert!(s.config.scheduler.map_directory.join("work.json").is_file());
    assert_eq!(records(&root), before);
    assert_eq!(fs::read(unknown.join("run.json")).unwrap(), b"broken association");
}

#[tokio::test]
async fn map_delete_rejects_unsafe_names_missing_files_and_invalid_body_with_json_errors() {
    let s = Scenario::new().await;
    let original = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    for name in ["missing.json", "work.txt", "..%5Cwork.json", "work.json%3Astream"] {
        let response = delete(&s, name, true).await;
        assert_ne!(response.status, 200, "{name}");
        let error: Value = serde_json::from_str(&response.body).unwrap();
        assert!(error["message"].is_string(), "{name}: {error}");
    }
    for body in ["{}", "{\"deleteHistory\":\"true\"}", "{\"deleteHistory\":true,\"extra\":true}"] {
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", body).await;
        assert_ne!(response.status, 200);
        assert!(serde_json::from_str::<Value>(&response.body).unwrap()["message"].is_string());
    }
    assert_eq!(fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap(), original);
}
