//! Real backend HTTP and controlled external CLI; no workflow business mocks.
use super::*;

async fn set_role(s: &Scenario, role: &str, host: &str, task_type: &str) -> String {
    let response = get_over_tcp(s.server.addr(), "/api/tasks").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let tasks: Value = serde_json::from_str(&response.body).unwrap();
    let id = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == role).unwrap()["id"].as_str().unwrap().to_owned();
    let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&json!({
        "name":role,"taskType":task_type,"agent":host,"chatName":"{chat.id}",
        "model":"default","thinking":"default","skill":null,"prompt":"FIXED ROLE","extensions":[]
    }).to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    id
}

fn identity(s: &Scenario, host: &str, session: &str, node: bool) {
    s.herdr.set_behavior(json!({"native_session":{"source":format!("herdr:{host}"),"agent":host,"kind":"id","value":session},"native_cwd":if node {s.data.path().join("node-zone")} else {s.data.path().to_owned()}}));
}

async fn start(s: &Scenario, host: &str) -> Value {
    identity(s, host, "mixed-manager", false);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"MIXED USER INPUT"}).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn action(s: &Scenario, name: &str, extra: Value, status: u16) -> Value {
    let mut body = extra;
    body["sessionId"] = json!("mixed-manager");
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{name}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

#[tokio::test]
async fn role_hosts_launch_independently_and_history_preserves_each_actual_agent() {
    for (manager, node) in [("pi", "omp"), ("omp", "pi"), ("pi", "pi")] {
        let s = Scenario::new().await;
        set_role(&s, "manager", manager, "manager").await;
        let node_task = set_role(&s, "node", node, "node").await;
        let started = start(&s, manager).await;
        assert_eq!(started["run"]["manager"]["sessionId"], "mixed-manager");
        identity(&s, node, "mixed-node", true);
        let dispatched = s.dispatch("mixed-manager", &["A"]).await;
        assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running");
        let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
        assert_eq!(execution["agent"], node);
        assert_eq!(execution["taskId"], node_task);
        let calls = s.herdr.calls();
        let starts: Vec<_> = calls.iter().filter(|call| call["kind"] == "agent_start").collect();
        assert_eq!(starts.len(), 2);
        assert_eq!(starts[0]["agent_kind"], manager);
        assert_eq!(starts[1]["agent_kind"], node);
        for (call, host, role, other_role) in [(starts[0], manager, "manager", "node"), (starts[1], node, "node", "manager")] {
            let args = call["args"].as_array().unwrap();
            assert!(args.contains(&json!(s.data.path().join(format!("{host}-{role}-chat.ts")))));
            assert!(!args.contains(&json!(s.data.path().join(format!("{host}-{other_role}-chat.ts")))));
        }
        let history = get_over_tcp(s.server.addr(), "/api/sessions/history").await;
        assert_eq!(history.status, 200, "{}", history.body);
        let history: Value = serde_json::from_str(&history.body).unwrap();
        let rows = history["sessions"].as_array().unwrap();
        for (id, host) in [("mixed-manager", manager), ("mixed-node", node)] {
            assert_eq!(rows.iter().find(|row| row["sessionId"] == id).unwrap()["agentType"], host);
        }
        let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"mixed-node","body":"results/A.md; checked"}"#).await;
        assert_eq!(report.status, 200, "{}", report.body);
        let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"mixed-manager","body":"cannot impersonate node"}"#).await;
        assert_eq!(denied.status, 403);
        assert_eq!(action(&s, "status", json!({}), 200).await["snapshot"]["nodes"]["A"]["execution"], *execution);
    }
}

#[tokio::test]
async fn pi_reconciliation_reads_original_execution_after_task_host_changes_and_retry_uses_new_host() {
    let s = Scenario::new().await;
    set_role(&s, "node", "pi", "node").await;
    start(&s, "omp").await;
    s.herdr.set_behavior(json!({"fail_agent_get":true}));
    let failed = s.dispatch("mixed-manager", &["A"]).await;
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["stage"], "identity");
    let original = &failed["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(original["agent"], "pi");
    set_role(&s, "node", "omp", "node").await;
    let before = s.herdr.calls();
    // Current Task is OMP; this already-started execution is still Pi.
    identity(&s, "pi", "original-pi", true);
    let reconciled = action(&s, "reconcile_dispatch", json!({"nodeId":"A","finding":"created","reason":"Inspected original Pi process"}), 200).await;
    let execution = &reconciled["snapshot"]["nodes"]["A"]["execution"];
    assert_eq!(execution["id"], original["id"]);
    assert_eq!(execution["agent"], "pi");
    assert_eq!(execution["chat"]["sessionId"], "original-pi");
    let after = s.herdr.calls();
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().unwrap()["kind"], "agent_get");
    assert_eq!(after.last().unwrap()["target"], failed["snapshot"]["nodes"]["A"]["failure"]["effects"]["agentName"]);
    action(&s, "fail", json!({"nodeId":"A","reason":"Original execution unusable"}), 200).await;
    identity(&s, "omp", "retried-omp", true);
    let retried = action(&s, "retry", json!({"nodeId":"A","confirmed":true,"reason":"New host Task deliberately selected"}), 200).await;
    assert_eq!(retried["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(retried["snapshot"]["nodes"]["A"]["execution"]["agent"], "omp");
    assert_eq!(retried["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], *execution);
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_start").last().unwrap()["agent_kind"], "omp");
}

#[tokio::test]
async fn pi_roles_reject_wrong_native_host_and_whole_graph_rejects_wrong_task_role() {
    let s = Scenario::new().await;
    set_role(&s, "manager", "pi", "manager").await;
    set_role(&s, "node", "pi", "normal").await;
    let before = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}).to_string()).await;
    assert_eq!(response.status, 400, "{}", response.body);
    assert!(response.body.contains("task-role-invalid"));
    assert_eq!(s.herdr.calls(), before, "preflight must not start any host");
    set_role(&s, "node", "pi", "node").await;
    identity(&s, "omp", "wrong-manager-host", false);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}).to_string()).await;
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt"));
    start(&s, "pi").await;
    identity(&s, "omp", "wrong-node-host", true);
    let failed = s.dispatch("mixed-manager", &["A"]).await;
    assert_eq!(failed["snapshot"]["nodes"]["A"]["failure"]["stage"], "identity");
    assert!(failed["snapshot"]["nodes"]["A"]["execution"]["chat"].is_null());
    let calls = s.herdr.calls();
    let rejected = action(&s, "reconcile_dispatch", json!({"nodeId":"A","finding":"created","reason":"Must not adopt wrong host"}), 409).await;
    assert_eq!(rejected["code"], "dispatch-chat-unverified");
    assert_eq!(s.herdr.calls().len(), calls.len() + 1);
}
