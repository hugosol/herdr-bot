// Approved S2 HTTP seam, P30.01. Contract literal: no notifications and
// no running nodes returns no-running, not an infinite wait; collect omits
// report bodies. A ready node alone is not a separate wake-up rule.
#[tokio::test]
async fn collect_without_active_execution_returns_reason_and_body_free_snapshot() {
    let s = Scenario::new().await;
    s.start("manager-collect-empty").await;
    let response = tokio::time::timeout(Duration::from_secs(2), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", &json!({"sessionId":"manager-collect-empty"}).to_string())).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reason"], "no-running");
    assert_eq!(value["snapshot"]["status"], "running");
    assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "pending");
    assert_eq!(value["pendingReports"], json!({}));
    assert!(value.get("reports").is_none(), "collect must not duplicate report bodies");
}

// S2/S3/S4, P26.02/P28.03/P31.01. A gated failing external delivery
// must not gate collect. Two reports before any waiter merge into one check;
// a later report must trigger another check without clearing the pending work.
#[tokio::test]
async fn collect_retains_merges_notifications_before_failed_delivery_and_keeps_pending_work() {
    let s = Scenario::new().await;
    let started = s.start("manager-collect-reports").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-collect-reports"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-collect-reports", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let release = s.data.path().join("collect-delivery-release");
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":10000}}));
    let mut requests = Vec::new();
    for body in ["SECRET-FIRST", "SECRET-SECOND"] {
        let addr = s.server.addr();
        requests.push(tokio::spawn(async move { post_json_over_tcp(addr, "/api/workflow/node/report", &json!({"sessionId":"node-collect-reports","body":body,"canComplete":false}).to_string()).await }));
        let expected = requests.len() + 2;
        common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == expected).await;
    }
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    let collect = || post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-collect-reports"}"#);
    let response = tokio::time::timeout(Duration::from_secs(2), collect()).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let first: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(first["reason"], "notification");
    assert_eq!(first["pendingReports"], json!({"A":reports[1]["id"]}));
    assert!(!response.body.contains("SECRET-"));
    assert!(requests.iter().all(|request| !request.is_finished()));
    fs::write(&release, "release").unwrap();
    for request in requests {
        let response = request.await.unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["notification"]["status"], "failed");
    }
    // paused collects check snapshots repeatedly without acknowledging handoff.
    assert_eq!(post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"manager-collect-reports"}"#).await.status, 200);
    for _ in 0..2 {
        let value: Value = serde_json::from_str(&collect().await.body).unwrap();
        assert_eq!(value["reason"], "paused");
        assert_eq!(value["pendingReports"], first["pendingReports"]);
        assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "running");
    }
    assert_eq!(post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", r#"{"sessionId":"manager-collect-reports"}"#).await.status, 200);
    let waiting = collect();
    tokio::pin!(waiting);
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut waiting).await.is_err(), "merged notifications must not remain unconsumed; ready B does not force return");
    let third = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"node-collect-reports","body":"SECRET-THIRD","canComplete":true}"#).await;
    let third: Value = serde_json::from_str(&third.body).unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), waiting).await.unwrap();
    let next: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(next["reason"], "notification");
    assert_eq!(next["pendingReports"], json!({"A":third["report"]["id"]}));
    assert_eq!(next["snapshot"]["nodes"]["A"]["status"], "running");
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P26.02/P28.03/P31.01 no-retryWindowMs=150 first={first} next={next} externalCalls={}", json!(calls));
}

// S2/S4, P30.01/P32.01/P33.01/P67.02/P68.05 (HTTP partitions).
// Contract: default 90 minutes, configured normal expiry and disconnect do not
// mutate facts or initiate host calls. A stale receiver consuming the next
// notification would make the post-disconnect request miss that notification.
#[tokio::test]
async fn collect_configured_expiry_and_tcp_disconnect_leave_facts_and_next_notification_intact() {
    let defaults: herdr_bot::SchedulerConfig = toml::from_str("").unwrap();
    assert_eq!(defaults.collect_wait_seconds.get(), 5400);
    assert!(toml::from_str::<herdr_bot::SchedulerConfig>("collect_wait_seconds = 0").is_err());
    assert!(toml::from_str::<herdr_bot::SchedulerConfig>("collect_wait_seconds = -1").is_err());
    let s = Scenario::with_collect_wait(1).await;
    let started = s.start("manager-collect-expiry").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-collect-expiry"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-collect-expiry", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    let start = std::time::Instant::now();
    let response = tokio::time::timeout(Duration::from_secs(3), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-collect-expiry"}"#)).await.unwrap();
    assert!(start.elapsed() >= Duration::from_millis(900));
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reason"], "timeout");
    assert_eq!(value["snapshot"]["status"], "running");
    assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert_eq!(s.herdr.calls(), calls);
    // Read actual response headers to establish the request is waiting before
    // closing TCP. No diagnostic-only waiter count is added to the API.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for _ in 0..3 {
        let mut stream = tokio::net::TcpStream::connect(s.server.addr()).await.unwrap();
        let body = r#"{"sessionId":"manager-collect-expiry"}"#;
        stream.write_all(format!("POST /api/workflow/manager/collect HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", s.server.addr(), body.len()).as_bytes()).await.unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut byte)).await.unwrap().unwrap();
            headers.push(byte[0]);
        }
        assert!(String::from_utf8(headers).unwrap().starts_with("HTTP/1.1 200"));
        drop(stream);
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert_eq!(s.herdr.calls(), calls);
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"node-collect-expiry","body":"Result after three TCP disconnects","canComplete":true}"#).await;
    let report: Value = serde_json::from_str(&report.body).unwrap();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-collect-expiry"}"#).await;
    let next: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(next["reason"], "notification");
    assert_eq!(next["pendingReports"], json!({"A":report["report"]["id"]}));
    println!("P32.01/P33.01/P67.02/P68.05 HTTP-only deadlineSeconds=1 disconnects=3 no-effectsWindowMs=150 expiry={value} next={next} externalCalls={}", json!(s.herdr.calls()));
}

// S2, P30.01: pause and final running-node handoff must release an already
// waiting collect. Holding an operation/file lock across the wait deadlocks the
// action; treating state-change wakeups as reports returns the wrong reason.
#[tokio::test]
async fn collect_wait_releases_on_pause_and_last_handoff_and_completed_is_immediate() {
    let s = Scenario::new().await;
    s.start("manager-collect-state").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-collect-state-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-collect-state", &["A"]).await;
    submit_report(&s, "node-collect-state-A", "Handoff ready before waiting for state changes", true).await;
    let checked = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-collect-state"}"#).await;
    assert_eq!(serde_json::from_str::<Value>(&checked.body).unwrap()["reason"], "notification");
    let collect = || post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"manager-collect-state"}"#);
    let waiting = collect();
    tokio::pin!(waiting);
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut waiting).await.is_err());
    let paused = tokio::time::timeout(Duration::from_secs(2), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", r#"{"sessionId":"manager-collect-state"}"#)).await.unwrap();
    assert_eq!(paused.status, 200);
    let response = tokio::time::timeout(Duration::from_secs(2), waiting).await.unwrap();
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reason"], "paused");
    assert_eq!(value["snapshot"]["status"], "paused");
    assert_eq!(post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", r#"{"sessionId":"manager-collect-state"}"#).await.status, 200);
    let waiting = collect();
    tokio::pin!(waiting);
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut waiting).await.is_err());
    let complete = tokio::time::timeout(Duration::from_secs(2), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", r#"{"sessionId":"manager-collect-state","nodeId":"A","reason":"Checked actual handoff"}"#)).await.unwrap();
    assert_eq!(complete.status, 200);
    let response = tokio::time::timeout(Duration::from_secs(2), waiting).await.unwrap();
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reason"], "no-running");
    assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(value["pendingReports"], json!({}));
    for node in ["B", "C"] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("node-collect-state-{node}")},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("manager-collect-state", &[node]).await;
        submit_report(&s, &format!("node-collect-state-{node}"), "Remaining handoff ready", true).await;
        assert_eq!(post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-collect-state","nodeId":node,"reason":"Accepted"}).to_string()).await.status, 200);
    }
    assert_eq!(post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", r#"{"sessionId":"manager-collect-state"}"#).await.status, 200);
    for _ in 0..2 {
        let response = tokio::time::timeout(Duration::from_secs(2), collect()).await.unwrap();
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["reason"], "completed");
        assert_eq!(value["snapshot"]["status"], "completed");
        assert_eq!(value["pendingReports"], json!({}));
        assert!(value.get("reports").is_none());
    }
}

// S2/S4, P33.01/P68.05 backend partition: a real read failure after HTTP
// headers cannot become a normal timeout snapshot. The next fresh request must
// still work; observe all records and host calls over a bounded failure window.
#[tokio::test]
async fn collect_late_read_failure_breaks_transport_without_fake_snapshot_or_state_changes() {
    let s = Scenario::with_collect_wait(1).await;
    let started = s.start("manager-collect-broken").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-collect-broken"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-collect-broken", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(s.server.addr()).await.unwrap();
    let body = r#"{"sessionId":"manager-collect-broken"}"#;
    stream.write_all(format!("POST /api/workflow/manager/collect HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", s.server.addr(), body.len()).as_bytes()).await.unwrap();
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut byte)).await.unwrap().unwrap();
        headers.push(byte[0]);
    }
    assert!(String::from_utf8(headers).unwrap().starts_with("HTTP/1.1 200"));
    fs::write(root.join("messages.jsonl"), b"{broken\n").unwrap();
    let mut bytes = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut bytes)).await.unwrap();
    fs::write(root.join("messages.jsonl"), &before[2]).unwrap();
    assert!(bytes.is_empty(), "late failure must not manufacture a JSON snapshot or normal chunk terminator: {bytes:?}; read={read:?}");
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert_eq!(s.herdr.calls(), calls);
    let response = tokio::time::timeout(Duration::from_secs(3), post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", body)).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["reason"], "timeout");
    assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(s.herdr.calls(), calls);
    println!("P33.01/P68.05 backend-only late-errorPayloadBytes=0 no-effectsWindowMs=150 next={value} externalCalls={}", json!(calls));
}

// S2: transport identity remains system-owned. Collect accepts no business
// arguments; forged scope and a node/unknown identity cannot inspect/check it.
#[tokio::test]
async fn collect_rejects_unassociated_node_and_forged_scope_without_external_calls() {
    let s = Scenario::new().await;
    s.start("manager-collect-identity").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-collect-identity"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-collect-identity", &["A"]).await;
    let calls = s.herdr.calls();
    for (body, code, status) in [
        (json!({"sessionId":"unknown"}), "manager-unassociated", 403),
        (json!({"sessionId":"node-collect-identity"}), "manager-unassociated", 403),
        (json!({"sessionId":"manager-collect-identity","runId":"forged"}), "invalid-json", 400),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", &body.to_string()).await;
        assert_eq!(response.status, status, "{}", response.body);
        if status == 403 { assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], code); }
    }
    assert_eq!(s.herdr.calls(), calls);
}

