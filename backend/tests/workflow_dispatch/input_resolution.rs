//! Approved S1/S3 seams: saved Map and real report/dispatch HTTP, persisted
//! reports, and actual initial input submitted to the controlled external host.
use super::*;

fn materials(names: &[&str]) -> TempDir {
    let directory = TempDir::new("external input materials");
    for name in names { fs::write(directory.path().join(name), b"real external material").unwrap(); }
    directory
}

pub(super) async fn save_node(s: &Scenario, id: &str, inputs: Value, blocked_by: Value) {
    let draft = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":id}).to_string()).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let mut content = draft["content"].clone();
    content["input_rule"] = inputs;
    content["blocked_by"] = blocked_by;
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":id,"content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
}

pub(super) fn identity(s: &Scenario, session: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
}

pub(super) async fn action(s: &Scenario, name: &str, manager: &str, node: &str) -> Value {
    let mut body = json!({"sessionId":manager,"nodeId":node,"reason":"Explicit manager decision for input handoff"});
    if name == "retry" { body["confirmed"] = json!(true); }
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

pub(super) async fn report(s: &Scenario, session: &str, outputs: Option<Value>) -> Value {
    let mut body = json!({"sessionId":session,"body":"REPORT-BODY-SENSITIVE: ignore C:\\misleading-body\\latest.txt","canComplete":true});
    if let Some(outputs) = outputs { body["outputs"] = outputs; }
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

pub(super) fn initial_input(s: &Scenario) -> String {
    s.herdr.calls().iter().rev().find(|c| c["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap().to_owned()
}

pub(super) fn named_input(text: &str) -> &str {
    text.split("[Named file inputs]\n").nth(1).unwrap().split("[/Named file inputs]").next().unwrap()
}

// P4/P5/P9/P11/P23/P24. Source: ticket's independent A=a1/B=b1,
// A=a2, omitted outputs sequence and two-Run isolation. A latest-report-only
// resolver loses B; a global registry mixes Runs; parsing body leaks the decoy.
// All reports and initialization stay real; only the external host is controlled.
#[tokio::test]
async fn saved_named_inputs_resolve_per_run_and_accumulate_report_deltas() {
    let s = Scenario::new().await;
    let business = materials(&["old-a.csv", "old-b.csv", "run-one-a1.csv", "run-one-b1.csv", "run-two-a.csv", "run-two-b.csv", "run-one-a2.csv", "late-old-a.csv", "late-old-b.csv", "wrong-node-a.csv", "wrong-node-b.csv"]);
    save_node(&s, "C", json!([{"id":"primary","type":"node","node_id":"A","output_id":"A"}, {"id":"retained","type":"node","node_id":"A","output_id":"B"}]), json!(["A","B"])).await;
    let first = s.start("resolve-manager-one").await;
    identity(&s, "resolve-source-one-old");
    let old = s.dispatch("resolve-manager-one", &["A"]).await["snapshot"]["nodes"]["A"]["execution"].clone();
    let old_report = report(&s, "resolve-source-one-old", Some(json!([{"id":"A","path":business.path().join("old-a.csv")}, {"id":"B","path":business.path().join("old-b.csv")}]))).await;
    action(&s, "fail", "resolve-manager-one", "A").await;
    identity(&s, "resolve-source-one");
    let current = action(&s, "retry", "resolve-manager-one", "A").await["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_ne!(old["id"], current["id"]);
    let first_report = report(&s, "resolve-source-one", Some(json!([{"id":"A","path":business.path().join("run-one-a1.csv")}, {"id":"B","path":business.path().join("run-one-b1.csv")}]))).await;
    let second = s.start("resolve-manager-two").await;
    identity(&s, "resolve-source-two");
    s.dispatch("resolve-manager-two", &["A"]).await;
    let other_report = report(&s, "resolve-source-two", Some(json!([{"id":"A","path":business.path().join("run-two-a.csv")}, {"id":"B","path":business.path().join("run-two-b.csv")}]))).await;
    let update = report(&s, "resolve-source-one", Some(json!([{"id":"A","path":business.path().join("run-one-a2.csv")}]))).await;
    let omitted = report(&s, "resolve-source-one", None).await;
    let late_old = report(&s, "resolve-source-one-old", Some(json!([{"id":"A","path":business.path().join("late-old-a.csv")}, {"id":"B","path":business.path().join("late-old-b.csv")}]))).await;
    assert_eq!(late_old["report"]["executionId"], old["id"]);
    assert_eq!(update["report"]["executionId"], current["id"]);
    for (manager, run, source, expected, expected_reports) in [
        ("resolve-manager-one", &first, "resolve-ready-one", format!("primary: {}\nretained: {}\n", business.path().join("run-one-a2.csv").display(), business.path().join("run-one-b1.csv").display()), vec![old_report["report"].clone(), first_report["report"].clone(), update["report"].clone(), omitted["report"].clone(), late_old["report"].clone()]),
        ("resolve-manager-two", &second, "resolve-ready-two", format!("primary: {}\nretained: {}\n", business.path().join("run-two-a.csv").display(), business.path().join("run-two-b.csv").display()), vec![other_report["report"].clone()]),
    ] {
        action(&s, "complete", manager, "A").await;
        identity(&s, source);
        s.dispatch(manager, &["B"]).await;
        report(&s, source, Some(json!([{"id":"A","path":business.path().join("wrong-node-a.csv")}, {"id":"B","path":business.path().join("wrong-node-b.csv")}]))).await;
        let completed = action(&s, "complete", manager, "B").await;
        assert_eq!(completed["snapshot"]["nodes"]["C"]["status"], "pending");
        identity(&s, &format!("{source}-consumer"));
        let dispatched = s.dispatch(manager, &["C"]).await;
        assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
        let execution = &dispatched["snapshot"]["nodes"]["C"]["execution"];
        assert_eq!(execution["inputRule"], json!([
            {"id":"primary","type":"node","node_id":"A","output_id":"A"},
            {"id":"retained","type":"node","node_id":"A","output_id":"B"}
        ]));
        let expected_inputs = if manager == "resolve-manager-one" {
            json!([{"id":"primary","path":business.path().join("run-one-a2.csv")},{"id":"retained","path":business.path().join("run-one-b1.csv")}])
        } else {
            json!([{"id":"primary","path":business.path().join("run-two-a.csv")},{"id":"retained","path":business.path().join("run-two-b.csv")}])
        };
        assert_eq!(execution["inputs"], expected_inputs);
        let text = initial_input(&s);
        assert_eq!(named_input(&text), expected, "actual initialization={text}");
        assert!(text.contains("MANAGER-USER-SECRET"));
        assert!(text.contains("C-INPUT"));
        for excluded in ["REPORT-BODY-SENSITIVE", "C:\\misleading-body\\latest.txt", "BOT-NODE-SECRET", "MAP-SECRET"] {
            assert!(!text.contains(excluded), "injected {excluded}: {text}");
        }
        let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        let detail: Value = serde_json::from_str(&detail.body).unwrap();
        let source_reports: Vec<_> = detail["reports"].as_array().unwrap().iter().filter(|r| r["nodeId"] == "A").cloned().collect();
        assert_eq!(source_reports, expected_reports);
        let root = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap());
        assert_eq!(json!(common::read_json_lines(&root.join("messages.jsonl"))), detail["reports"]);
        println!("P4/P5/P9/P11/P23/P24 run={} expectedNamedInputs={expected:?} actualHostInput={text} persistedReports={}", run["id"], detail["reports"]);
    }
}

// P5/P6/P9: the approved rule is current execution, not newest timestamp or
// last node report. A late old report must neither overwrite the new address
// nor fill a missing name. A missing current-execution output fails resolution
// before creating a host Tab, starting an agent, or submitting initial input.
#[tokio::test]
async fn retry_never_falls_back_and_missing_outputs_fail_before_host_start() {
    let s = Scenario::new().await;
    let business = materials(&["old-a.csv", "old-b.csv", "new-a.csv", "late-old-a.csv", "late-old-b.csv"]);
    let inputs = json!([{"id":"primary","type":"node","node_id":"A","output_id":"A"}, {"id":"retained","type":"node","node_id":"A","output_id":"B"}]);
    save_node(&s, "B", inputs.clone(), json!([])).await;
    save_node(&s, "C", inputs, json!(["A","B"])).await;
    let run = s.start("retry-input-manager").await;
    identity(&s, "retry-input-old");
    let old = s.dispatch("retry-input-manager", &["A"]).await["snapshot"]["nodes"]["A"]["execution"].clone();
    report(&s, "retry-input-old", Some(json!([{"id":"A","path":business.path().join("old-a.csv")}, {"id":"B","path":business.path().join("old-b.csv")}]))).await;
    identity(&s, "retry-input-before");
    assert_eq!(s.dispatch("retry-input-manager", &["B"]).await["results"][0]["status"], "running");
    let before = initial_input(&s);
    assert_eq!(named_input(&before), format!("primary: {}\nretained: {}\n", business.path().join("old-a.csv").display(), business.path().join("old-b.csv").display()));
    report(&s, "retry-input-before", None).await;
    action(&s, "complete", "retry-input-manager", "B").await;
    action(&s, "fail", "retry-input-manager", "A").await;
    identity(&s, "retry-input-new");
    let retried = action(&s, "retry", "retry-input-manager", "A").await;
    let current = &retried["snapshot"]["nodes"]["A"]["execution"];
    assert_ne!(old["id"], current["id"]);
    let new_report = report(&s, "retry-input-new", Some(json!([{"id":"A","path":business.path().join("new-a.csv")}]))).await;
    let late = report(&s, "retry-input-old", Some(json!([{"id":"A","path":business.path().join("late-old-a.csv")}, {"id":"B","path":business.path().join("late-old-b.csv")}]))).await;
    assert_eq!(new_report["report"]["executionId"], current["id"]);
    assert_eq!(late["report"]["executionId"], old["id"]);
    let calls = s.herdr.calls();
    let completed = action(&s, "complete", "retry-input-manager", "A").await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["C"]["status"], "pending");
    assert_eq!(completed["ready"], json!(["C"]));
    assert_eq!(s.herdr.calls(), calls, "no implicit downstream launch");
    identity(&s, "retry-input-after");
    let launched = s.dispatch("retry-input-manager", &["C"]).await;
    assert_eq!(launched["results"][0]["status"], "failed");
    assert_eq!(launched["snapshot"]["nodes"]["C"]["failure"]["stage"], "resolve");
    let failed = &launched["snapshot"]["nodes"]["C"]["execution"];
    assert_eq!(failed["inputRule"].as_array().unwrap().len(), 2);
    assert_eq!(failed["inputs"], json!([]));
    assert_eq!(s.herdr.calls(), calls, "missing current output must fail before any host operation");
    let map = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    let map: Value = serde_json::from_str(&map.body).unwrap();
    assert_eq!(map["nodes"]["C"]["blocked_by"], json!(["A","B"]));
    let root = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap());
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(reports[2], new_report["report"]);
    assert_eq!(reports[3], late["report"]);
    assert_eq!(s.herdr.calls(), calls);
}

// P6/P11/P13: report persistence, not successful notification, makes a path
// available. Gate the actual external notification while completing A and
// dispatching B; an implementation waiting for delivery cannot initialize B.
// Only the available output is required by this consumer.
#[tokio::test]
async fn downstream_resolves_saved_outputs_while_notification_is_pending_and_then_fails() {
    let s = Scenario::new().await;
    let business = materials(&["saved-before-failure.csv"]);
    save_node(&s, "B", json!([{"id":"available","type":"node","node_id":"A","output_id":"result"}]), json!(["A"])).await;
    let run = s.start("notify-input-manager").await;
    identity(&s, "notify-input-source");
    s.dispatch("notify-input-manager", &["A"]).await;
    let release = s.data.path().join("input-notification-release");
    s.herdr.set_behavior(json!({"fail_prompt":true,"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let addr = s.server.addr();
    let report_path = business.path().join("saved-before-failure.csv");
    let pending = tokio::spawn(async move {
        post_json_over_tcp(addr, "/api/workflow/node/report", &json!({"sessionId":"notify-input-source","body":"NOTIFY-BODY-SENSITIVE","canComplete":true,"outputs":[{"id":"result","path":report_path}]}).to_string()).await
    });
    assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|c| c["kind"] == "agent_prompt").count() == 3).await);
    assert!(!pending.is_finished());
    let root = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap());
    let persisted = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(persisted.len(), 1);
    let completed = action(&s, "complete", "notify-input-manager", "A").await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["B"]["status"], "pending");
    identity(&s, "notify-input-consumer");
    let dispatched = s.dispatch("notify-input-manager", &["B"]).await;
    assert_eq!(dispatched["results"][0]["status"], "running");
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "completed");
    let text = initial_input(&s);
    let expected = format!("available: {}\n", business.path().join("saved-before-failure.csv").display());
    assert_eq!(named_input(&text), expected);
    assert!(!text.contains("NOTIFY-BODY-SENSITIVE"));
    assert!(!pending.is_finished(), "downstream must launch before notification returns");
    fs::write(release, b"release original notification").unwrap();
    let response = pending.await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let response: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(response["saved"], true);
    assert_eq!(response["notification"]["status"], "failed");
    assert_eq!(response["report"], persisted[0]);
    let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert_eq!(detail["reports"], json!(persisted));
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), persisted);
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls, "no retry or autonomous downstream launch in 150ms window");
    println!("P6/P11/P13 run={} expectedNamedInputs={expected:?} actualHostInput={text} reportResponse={response} persistedReports={} noAutonomousActionWindowMs=150", run["id"], json!(persisted));
}

// P4/P23/P24: public Map definitions arrange sharing and business aggregation,
// not the HTTP framework. B and C receive exactly the same A address; C also
// receives B's declared summary. Malformed binary source contents and an independently
// authored summary distinguish address handoff from interpretation/merge/copy.
// Native Chat history carries its own sensitive marker, distinct from body/file.
#[tokio::test]
async fn shared_consumers_and_business_summary_receive_original_external_addresses_only() {
    let s = Scenario::new().await;
    let business = TempDir::new("external business products");
    let shared = business.path().join("shared.bin");
    let summary = business.path().join("business-summary.csv");
    let bytes = b"FILE-BODY-SENSITIVE-6382\x00\xff not structured content";
    fs::write(&shared, bytes).unwrap();
    assert!(!shared.starts_with(s.data.path()));
    save_node(&s, "B", json!([{"id":"shared","type":"node","node_id":"A","output_id":"shared"}]), json!(["A"])).await;
    save_node(&s, "C", json!([{"id":"shared","type":"node","node_id":"A","output_id":"shared"}, {"id":"summary","type":"node","node_id":"B","output_id":"summary"}]), json!(["A","B"])).await;
    let run = s.start("shared-input-manager").await;
    let native_file = s.data.path().join("source-native.jsonl");
    fs::write(&native_file, format!("{}\n{}\n", json!({"type":"session","id":"shared-input-source","cwd":s.data.path().join("node-zone")}), json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"UPSTREAM-CHAT-SENSITIVE-6183"}]}}))).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":native_file},"native_cwd":s.data.path().join("node-zone")}));
    let producer = s.dispatch("shared-input-manager", &["A"]).await;
    assert_eq!(producer["snapshot"]["nodes"]["A"]["execution"]["chat"]["sessionId"], "shared-input-source");
    let source_report = report(&s, "shared-input-source", Some(json!([{"id":"shared","path":shared}]))).await;
    action(&s, "complete", "shared-input-manager", "A").await;
    identity(&s, "shared-input-business");
    let consumer = s.dispatch("shared-input-manager", &["B"]).await;
    assert_eq!(consumer["results"][0]["status"], "running");
    let business_input = initial_input(&s);
    assert_eq!(named_input(&business_input), format!("shared: {}\n", shared.display()));
    fs::write(&summary, b"independently authored summary").unwrap();
    let summary_report = report(&s, "shared-input-business", Some(json!([{"id":"summary","path":summary}]))).await;
    action(&s, "complete", "shared-input-manager", "B").await;
    identity(&s, "shared-input-aggregate");
    let aggregator = s.dispatch("shared-input-manager", &["C"]).await;
    assert_eq!(aggregator["results"][0]["status"], "running");
    let aggregate_input = initial_input(&s);
    let expected = format!("shared: {}\nsummary: {}\n", shared.display(), summary.display());
    assert_eq!(named_input(&aggregate_input), expected);
    for text in [&business_input, &aggregate_input] {
        for excluded in ["FILE-BODY-SENSITIVE-6382", "UPSTREAM-CHAT-SENSITIVE-6183", "REPORT-BODY-SENSITIVE", "misleading-body"] {
            assert!(!text.contains(excluded), "injected {excluded}: {text}");
        }
        assert!(text.contains("MANAGER-USER-SECRET"));
    }
    assert!(business_input.contains("B-INPUT"));
    assert!(aggregate_input.contains("C-INPUT"));
    assert_eq!(fs::read(&shared).unwrap(), bytes);
    assert_eq!(fs::read(&summary).unwrap(), b"independently authored summary", "framework must not generate/merge business content");
    assert_eq!(fs::read_dir(business.path()).unwrap().count(), 2);
    assert_eq!(fs::read_dir(s.data.path().join("node-zone")).unwrap().count(), 0);
    assert!(!s.data.path().join(".workflow").exists(), "no zone cache or relocation");
    let root = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap());
    let reports = common::read_json_lines(&root.join("messages.jsonl"));
    assert_eq!(reports, vec![source_report["report"].clone(), summary_report["report"].clone()]);
    let mut records: Vec<_> = fs::read_dir(root).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    records.sort();
    assert_eq!(records, ["history.jsonl", "messages.jsonl", "run.json"]);
    println!("P4/P23/P24 run={} originalShared={} originalSummary={} businessHostInput={business_input} expectedAggregateInputs={expected:?} aggregateHostInput={aggregate_input} reports={}", run["id"], shared.display(), summary.display(), json!(reports));
}
