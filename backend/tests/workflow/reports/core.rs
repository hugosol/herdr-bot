// P24.01/P27.01/P28.01: independent literals require two append-only reports
// from the real current node identity, never a state transition or read receipt.
// A report attributed from caller-supplied metadata or cleared by reads violates
// this scenario at the HTTP, persisted-message and external-input seams.
#[tokio::test]
async fn node_reports_are_system_attributed_append_only_and_do_not_complete_work() {
    let s = Scenario::new().await;
    let started = s.start("manager-report").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-report"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-report", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let mut reports = Vec::new();
    for body in ["Result: C:\\results\\one.txt; draft only", "Result: C:\\results\\two.txt; reviewed"] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-report","body":body,"canComplete":body.ends_with("reviewed")}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], "submitted");
        assert_eq!(value["report"]["body"], body);
        reports.push(value["report"].clone());
    }
    assert_ne!(reports[0]["id"], reports[1]["id"]);
    for report in &reports {
        assert!(!report["id"].as_str().unwrap().is_empty());
        assert!(report["at"].as_u64().unwrap() >= started["run"]["createdAt"].as_u64().unwrap());
        assert_eq!(report["type"], "report");
        assert_eq!(report["runId"], id);
        assert_eq!(report["nodeId"], "A");
        let run: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(report["executionId"], run["nodes"]["A"]["execution"]["id"]);
        assert_eq!(report["from"]["sessionId"], "node-report");
        assert_eq!(report["to"]["sessionId"], "manager-report");
    }
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    for _ in 0..2 {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["reports"], json!(reports));
        assert_eq!(value["pendingReports"], json!({"A":reports[1]["id"]}));
    }
    let calls = s.herdr.calls();
    let inputs: Vec<_> = calls.iter().filter(|c| c["kind"] == "agent_prompt").collect();
    assert_eq!(inputs.len(), 4);
    for (input, report) in inputs[2..].iter().zip(&reports) {
        assert_eq!(input["target"], started["run"]["manager"]["agentName"]);
        assert!(input["text"].as_str().unwrap().contains(report["body"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains(report["id"].as_str().unwrap()));
        assert!(input["text"].as_str().unwrap().contains("node: A"));
    }
}

// P25.01: a real Windows read-only messages file must reject persistence.
// The external adapter remains real so any accidental prompt is observable.
#[tokio::test]
async fn report_save_failure_has_no_message_pending_item_or_host_delivery() {
    let s = Scenario::with_collect_wait(1).await;
    let started = s.start("manager-save-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-save-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-save-error", &["A"]).await;
    let path = s.config.scheduler.run_directory.join(id).join("messages.jsonl");
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-save-error","body":"must not persist or notify","canComplete":false,"outputs":[{"id":"result","path":"C:\\failed-candidate.txt"}]}).to_string()).await;
    fs::set_permissions(&path, original).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["code"], "report-save-failed");
    assert_eq!(value["saved"], false);
    assert_eq!(value["notification"]["status"], "not-attempted");
    assert!(fs::read(&path).unwrap().is_empty());
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reports"], json!([]));
    assert_eq!(value["pendingReports"], json!({}));
    // P25.02: a failed save must not produce even a transient collect wakeup.
    // The whole 1s backend deadline is observed, not just the empty report index.
    let start = std::time::Instant::now();
    let response = tokio::time::timeout(Duration::from_secs(3), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-save-error"}"#)).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let collected: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(collected["reason"], "timeout");
    assert!(start.elapsed() >= Duration::from_millis(900));
    assert_eq!(collected["pendingReports"], json!({}));
    assert_eq!(collected["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(s.herdr.calls(), calls);
    println!("P25.02 no-notificationWindowMs=1000 collect={collected} externalCalls={}", json!(calls));
}

// P26.01/P28.01/P68.02: the external prompt gate holds delivery after
// persistence. Saved reports must already be visible to the manager; a failed
// single attempt must not pause or resend during the 300 ms post-result window.
#[tokio::test]
async fn delayed_failed_notification_preserves_saved_report_and_pending_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-notify-error").await;
    let id = started["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-notify-error"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-notify-error", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(id);
    let before = fs::read(root.join("run.json")).unwrap();
    let release = s.data.path().join("notify-release");
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let addr = s.server.addr();
    let request = tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/node/report", &json!({"sessionId":"node-notify-error","body":"SENSITIVE-RESULT: saved before notification","canComplete":true,"outputs":[{"id":"result","path":"C:\\saved-despite-notification.txt"}]}).to_string()).await });
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 3).await;
    assert!(!request.is_finished());
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(reports.len(), 1);
    let report_id = reports[0]["id"].clone();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-notify-error"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let status: Value = serde_json::from_str(&response.body).unwrap();
    fs::write(release, b"release").unwrap();
    let response = request.await.unwrap();
    assert_eq!(status["pendingReports"], json!({"A":report_id}));
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["saved"], true);
    assert_eq!(value["notification"]["status"], "failed");
    assert!(!value["notification"]["reason"].as_str().unwrap().is_empty());
    assert_eq!(value["report"]["id"], report_id);
    assert_eq!(value["report"]["canComplete"], true);
    assert_eq!(value["report"]["outputs"], json!([{"id":"result","path":"C:\\saved-despite-notification.txt"}]));
    assert_eq!(status["reports"], json!(reports));
    let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(serde_json::from_str::<Value>(&detail.body).unwrap()["reports"], json!(reports));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    let calls = s.herdr.calls();
    assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
    println!("P26.01/P68.02 external calls after 300ms: {}", json!(calls));
}

// Chief ticket 01, approved S2: after the external gate observes notification,
// the literal report must already be readable. A file/operation lock spanning
// delivery would block the read/pause; disconnect rollback would remove it.
// Only the external Herdr process is controlled, never report persistence.
#[tokio::test]
async fn disconnected_report_caller_keeps_saved_fact_and_does_not_lock_run_during_delivery() {
    use tokio::io::AsyncWriteExt;
    let s = Scenario::new().await;
    let started = s.start("disconnect-report-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"disconnect-report-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("disconnect-report-manager", &["A"]).await;
    let release = s.data.path().join("disconnect-report-release");
    struct Release(std::path::PathBuf);
    impl Drop for Release {
        fn drop(&mut self) { fs::write(&self.0, b"release").expect("release owned prompt gate"); }
    }
    let gate = Release(release.clone());
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let body = json!({"sessionId":"disconnect-report-node","body":"Saved despite disconnected caller","canComplete":false,"outputs":[{"id":"result","path":"C:\\disconnect\\result.txt"}]}).to_string();
    let mut caller = tokio::net::TcpStream::connect(s.server.addr()).await.unwrap();
    caller.write_all(format!("POST /api/workflow/node/report HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", s.server.addr(), body.len()).as_bytes()).await.unwrap();
    common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 3).await;
    drop(caller);
    let route = format!("/api/workflow/runs/{}", started["id"].as_str().unwrap());
    let response = tokio::time::timeout(Duration::from_secs(2), get_over_tcp(s.server.addr(), &route)).await.expect("notification must not hold file coordination");
    assert_eq!(response.status, 200, "{}", response.body);
    let saved: Value = serde_json::from_str(&response.body).unwrap();
    let report = &saved["reports"][0];
    assert_eq!(saved["reports"].as_array().unwrap().len(), 1);
    assert_eq!(report["body"], "Saved despite disconnected caller");
    assert_eq!(report["outputs"], json!([{"id":"result","path":"C:\\disconnect\\result.txt"}]));
    assert_eq!(report["to"], started["run"]["manager"]);
    assert_eq!(saved["pendingReports"], json!({"A":report["id"]}));
    let paused = tokio::time::timeout(Duration::from_secs(2), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"disconnect-report-manager"}"#)).await.expect("notification must not hold Run operation lock");
    assert_eq!(paused.status, 200, "{}", paused.body);
    drop(gate);
    // This is a bounded development regression, not P23/P36 cross-host proof.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let response = get_over_tcp(s.server.addr(), &route).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let after: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(after["reports"], saved["reports"]);
    assert_eq!(after["pendingReports"], saved["pendingReports"]);
    assert_eq!(after["run"]["nodes"]["A"]["status"], "running");
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count(), 3);
}

// P24.01: neither a manager/unknown session nor forged ownership fields may
// produce a report or manager input. Business declarations are not ownership;
// sessionId belongs to the trusted public Session transport adapter.
#[tokio::test]
async fn report_rejects_non_node_identity_and_caller_supplied_ownership() {
    let s = Scenario::new().await;
    let started = s.start("manager-reject").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-reject"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-reject", &["A"]).await;
    let calls = s.herdr.calls();
    for (body, expected) in [
        (json!({"sessionId":"manager-reject","body":"manager cannot report","canComplete":false}), 403),
        (json!({"sessionId":"unassociated","body":"no binding","canComplete":false}), 403),
        (json!({"sessionId":"node-reject","body":"forged","canComplete":false,"runId":"another-run"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","canComplete":false,"nodeId":"B"}), 400),
        (json!({"sessionId":"node-reject","body":"forged","canComplete":false,"target":"another-chat"}), 400),
        (json!({"sessionId":"node-reject"}), 400),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
        assert_eq!(response.status, expected, "{}", response.body);
    }
    assert!(fs::read(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("messages.jsonl")).unwrap().is_empty());
    assert_eq!(s.herdr.calls(), calls);
}

// P61.01/P68.02: isolate the production diagnostic sink in a child process.
// Closing its stderr pipe is a real OS write failure, not a mocked logger.
// Independent literals prohibit report text in diagnostics; 300 ms of actual
// external calls after the result distinguishes one attempt from retry loops.
#[tokio::test]
async fn report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries() {
    const MODE: &str = "HERDR_REPORT_DIAGNOSTIC_TEST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        let s = Scenario::new().await;
        let started = s.start("manager-diagnostic").await;
        let id = started["id"].as_str().unwrap();
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-diagnostic"},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-diagnostic", &["A"]).await;
        let before = fs::read(s.config.scheduler.run_directory.join(id).join("run.json")).unwrap();
        let failed = mode != "success";
        s.herdr.set_behavior(json!({"fail_prompt":failed}));
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-diagnostic","body":"SENSITIVE-DIAGNOSTIC-BODY","canComplete":false}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["saved"], true);
        assert_eq!(value["notification"]["status"], if failed {"failed"} else {"submitted"});
        assert_eq!(value["diagnostic"]["status"], if mode == "broken" {"failed"} else {"written"});
        tokio::time::sleep(Duration::from_millis(300)).await;
        let root = s.config.scheduler.run_directory.join(id);
        assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), vec![value["report"].clone()]);
        assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
        let calls = s.herdr.calls();
        assert_eq!(calls.iter().filter(|c| c["kind"] == "agent_prompt").count(), 3);
        println!("REPORT-EVIDENCE {}", json!({"runId":id,"reportId":value["report"]["id"],"targetChat":started["run"]["manager"],"result":value["notification"]["status"],"externalCalls":calls,"observationMs":300}));
        return;
    }
    for mode in ["success", "failure", "broken"] {
        let cwd = TempDir::new("report diagnostic child cwd");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "report_delivery_diagnostics_are_body_free_and_sink_failure_never_retries", "--nocapture"])
            .env(MODE, mode).current_dir(cwd.path())
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped())
            .spawn().unwrap();
        if mode == "broken" { drop(child.stderr.take()); }
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{mode}\n{stdout}\n{stderr}");
        let evidence: Value = serde_json::from_str(stdout.lines().find_map(|line| line.strip_prefix("REPORT-EVIDENCE ")).unwrap()).unwrap();
        if mode != "broken" {
            assert!(!stderr.contains("SENSITIVE-DIAGNOSTIC-BODY"));
            let diagnostic: Value = serde_json::from_str(stderr.lines().find(|line| line.contains("workflow-report-delivery")).unwrap()).unwrap();
            assert_eq!(diagnostic["runId"], evidence["runId"]);
            assert_eq!(diagnostic["reportId"], evidence["reportId"]);
            assert_eq!(diagnostic["targetChat"], evidence["targetChat"]);
            assert_eq!(diagnostic["result"], evidence["result"]);
            assert!(!diagnostic["reason"].as_str().unwrap().is_empty());
        }
        println!("P61.01/P68.02 mode={mode} stderr={stderr} evidence={evidence}");
    }
}

