//! S2/S3/S4 contract checks, not evidence of native role-extension loading.
use super::*;
use axum::{body::Body, extract::connect_info::MockConnectInfo, http::Request};
use tower::ServiceExt;

async fn from_source(s: &Scenario, source: &str, route: &str, body: Value) -> (u16, Value) {
    // Existing network-guardrail technique: inject only the transport peer at
    // the HTTP boundary. No LAN listener, mDNS, token or alternate auth path.
    let app = s.app.clone().layer(MockConnectInfo(source.parse::<std::net::SocketAddr>().unwrap()));
    let request = Request::builder().method("POST").uri(route)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

// P23.01: topology is not authentication. Loopback with a real association is
// allowed; the same source without association is denied; even a known real
// identity from a nonlocal source is denied before effects. Plausible violation:
// a role endpoint omits the source guard or treats local IP as authorization.
#[tokio::test]
async fn source_matrix_enforces_local_transport_and_session_association_separately() {
    let s = Scenario::new().await;
    let run = s.start("source-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"source-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("source-manager", &["A"]).await;
    let before = detail(&s, &run["id"]).await;
    let calls = s.herdr.calls();
    for source in ["127.0.0.1:40000", "[::1]:40000"] {
        let (status, value) = from_source(&s, source, "/api/workflow/manager/status", json!({"sessionId":"source-manager"})).await;
        assert_eq!(status, 200, "{source}: {value}");
        assert_eq!(value["snapshot"]["id"], run["id"]);
        for (name, body) in manager_requests("source-unassociated") {
            let (status, value) = from_source(&s, source, &format!("/api/workflow/manager/{name}"), body).await;
            assert_eq!(status, 403, "{source} {name}: {value}");
            assert_eq!(value["code"], "manager-unassociated");
        }
        let (status, value) = from_source(&s, source, "/api/workflow/node/report", json!({"sessionId":"source-unassociated","body":"not authorized"})).await;
        assert_eq!(status, 403, "{source}: {value}");
        assert_eq!(value["code"], "node-unassociated");
    }
    for source in ["203.0.113.7:40000", "192.0.2.7:40000", "[2001:db8::7]:40000"] {
        for (name, body) in manager_requests("source-manager") {
            let (status, value) = from_source(&s, source, &format!("/api/workflow/manager/{name}"), body).await;
            assert_eq!(status, 403, "{source} {name}: {value}");
            assert_eq!(value["code"], "forbidden");
        }
        let (status, value) = from_source(&s, source, "/api/workflow/node/report", json!({"sessionId":"source-node","body":"remote cannot report"})).await;
        assert_eq!(status, 403, "{source}: {value}");
        assert_eq!(value["code"], "forbidden");
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(detail(&s, &run["id"]).await, before);
    assert_eq!(s.herdr.calls(), calls);
    println!("P23.01 controlled-peer allowed=2 denied=60 no-effectsWindowMs=150 externalCalls={}", json!(calls));
}

// P22.01: association must be unique across roles and Runs, including retained
// executions. A host returning a session already used by a node must not let
// the manager row alone authorize that session. Independent expected outcome:
// every manager route denies, neither Run changes, no external call follows.
#[tokio::test]
async fn manager_authorization_rejects_cross_run_current_and_old_node_identity_collisions() {
    for retained in [false, true] {
        let s = Scenario::new().await;
        let original = s.start("collision-original-manager").await;
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"collision-node"},"native_cwd":s.data.path().join("node-zone")}));
        s.dispatch("collision-original-manager", &["A"]).await;
        if retained {
            manager(&s, "fail", json!({"sessionId":"collision-original-manager","nodeId":"A","reason":"Human verified failed original"}), 200).await;
            s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"collision-replacement"},"native_cwd":s.data.path().join("node-zone")}));
            manager(&s, "retry", json!({"sessionId":"collision-original-manager","nodeId":"A","confirmed":true,"reason":"Human approved independent attempt"}), 200).await;
        }
        let conflicting = s.start("collision-node").await;
        let before = [detail(&s, &original["id"]).await, detail(&s, &conflicting["id"]).await];
        let calls = s.herdr.calls();
        for (name, body) in manager_requests("collision-node") {
            let denied = manager(&s, name, body, 403).await;
            assert_eq!(denied["code"], "manager-ambiguous");
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(s.herdr.calls(), calls);
        assert_eq!(detail(&s, &original["id"]).await, before[0]);
        assert_eq!(detail(&s, &conflicting["id"]).await, before[1]);
        let unaffected = manager(&s, "status", json!({"sessionId":"collision-original-manager"}), 200).await;
        assert_eq!(unaffected["snapshot"]["id"], original["id"]);
        println!("P22.01 cross-role-collision retained={retained} denied=11 no-effectsWindowMs=150 externalCalls={}", json!(calls));
    }
}

async fn manager(s: &Scenario, name: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{name}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn manager_requests(identity: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("status", json!({"sessionId":identity})),
        ("collect", json!({"sessionId":identity})),
        ("dispatch", json!({"sessionId":identity,"nodes":["B"]})),
        ("complete", json!({"sessionId":identity,"nodeId":"A","reason":"Unauthorized handoff"})),
        ("fail", json!({"sessionId":identity,"nodeId":"A","reason":"Unauthorized failure"})),
        ("recover", json!({"sessionId":identity,"nodeId":"A","confirmed":true,"reason":"Unauthorized recovery"})),
        ("retry", json!({"sessionId":identity,"nodeId":"A","confirmed":true,"reason":"Unauthorized retry"})),
        ("reconcile_dispatch", json!({"sessionId":identity,"nodeId":"A","finding":"uncertain","reason":"Unauthorized reconciliation"})),
        ("finish", json!({"sessionId":identity})),
        ("pause", json!({"sessionId":identity})),
        ("resume", json!({"sessionId":identity})),
    ]
}

async fn detail(s: &Scenario, id: &Value) -> Value {
    let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", id.as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// P22.01: two Runs share the same Map/Task/node names, never ownership. A
// plausible violation authorizes node/old/unassociated callers or accepts a
// caller-supplied Run. Literal 403/400, unchanged public details + record bytes,
// and a 150ms external-call observation distinguish it. The real HTTP backend,
// storage and host adapter stay in path; only the external host is controlled.
#[tokio::test]
async fn two_run_role_and_retained_execution_matrix_rejects_scope_forgery() {
    let s = Scenario::new().await;
    let first = s.start("security-manager-one").await;
    let second = s.start("security-manager-two").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"security-node-old"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("security-manager-one", &["A"]).await;
    manager(&s, "fail", json!({"sessionId":"security-manager-one","nodeId":"A","reason":"Human checked original execution"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"security-node-current"},"native_cwd":s.data.path().join("node-zone")}));
    manager(&s, "retry", json!({"sessionId":"security-manager-one","nodeId":"A","confirmed":true,"reason":"Human confirmed independent attempt"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"security-node-other-run"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("security-manager-two", &["A"]).await;

    let mut reports = Vec::new();
    for (identity, body, run, target) in [
        ("security-node-current", "CURRENT: eligible", &first, "security-manager-one"),
        ("security-node-old", "OLD: late history only", &first, "security-manager-one"),
        ("security-node-other-run", "OTHER RUN: isolated", &second, "security-manager-two"),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":identity,"body":body}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["report"]["runId"], run["id"]);
        assert_eq!(value["report"]["nodeId"], "A");
        assert_eq!(value["report"]["source"]["sessionId"], identity);
        assert_eq!(value["report"]["target"]["sessionId"], target);
        assert_eq!(value["report"]["body"], body);
        reports.push(value["report"].clone());
    }
    for (identity, run, expected) in [
        ("security-manager-one", &first, &reports[0]),
        ("security-manager-two", &second, &reports[2]),
    ] {
        let status = manager(&s, "status", json!({"sessionId":identity}), 200).await;
        assert_eq!(status["snapshot"]["id"], run["id"]);
        assert_eq!(status["snapshot"]["nodes"]["A"]["status"], "running");
        assert_eq!(status["pendingReports"]["A"], expected["id"]);
        let collected = manager(&s, "collect", json!({"sessionId":identity}), 200).await;
        assert_eq!(collected["reason"], "notification");
        assert_eq!(collected["pendingReports"]["A"], expected["id"]);
    }
    let before = [detail(&s, &first["id"]).await, detail(&s, &second["id"]).await];
    let paths: Vec<_> = [&first, &second].into_iter().flat_map(|run| {
        let root = s.config.scheduler.run_directory.join(run["id"].as_str().unwrap());
        [root.join("run.json"), root.join("history.jsonl"), root.join("messages.jsonl")]
    }).collect();
    let bytes: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    let calls = s.herdr.calls();
    let mut denied = 0;
    for identity in ["security-node-old", "security-node-current", "security-node-other-run", "security-unassociated"] {
        for (name, body) in manager_requests(identity) {
            let result = manager(&s, name, body, 403).await;
            assert_eq!(result["code"], "manager-unassociated");
            denied += 1;
        }
    }
    for (identity, code) in [("security-manager-one", "node-role-invalid"), ("security-manager-two", "node-role-invalid"), ("security-unassociated", "node-unassociated")] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":identity,"body":"not authorized"}).to_string()).await;
        assert_eq!(response.status, 403, "{}", response.body);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["code"], code);
        denied += 1;
    }
    // nodeId on a manager action selects work inside the caller's Run; it is
    // not a self-reported node role. Run/role/execution/source remain forbidden.
    for (identity, other) in [("security-manager-one", &second), ("security-manager-two", &first)] {
        for (name, body) in manager_requests(identity) {
            for (key, value) in [("runId", other["id"].clone()), ("role", json!("manager")), ("executionId", json!("forged-execution")), ("source", json!({"sessionId":"security-node-current"}))] {
                let mut forged = body.clone();
                forged[key] = value;
                manager(&s, name, forged, 400).await;
                denied += 1;
            }
        }
    }
    for identity in ["security-node-old", "security-node-current", "security-node-other-run"] {
        for (key, value) in [("runId", second["id"].clone()), ("role", json!("node")), ("nodeId", json!("B")), ("executionId", json!("forged-current")), ("target", json!({"sessionId":"security-manager-two"}))] {
            let mut body = json!({"sessionId":identity,"body":"forged report"});
            body[key] = value;
            let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
            assert_eq!(response.status, 400, "{}", response.body);
            denied += 1;
        }
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls, "rejections must not cause autonomous external actions within 150ms");
    assert_eq!(detail(&s, &first["id"]).await, before[0]);
    assert_eq!(detail(&s, &second["id"]).await, before[1]);
    for (path, original) in paths.iter().zip(bytes) { assert_eq!(fs::read(path).unwrap(), original); }
    println!("P22.01 backend-only denied={denied} no-effectsWindowMs=150 first={} second={} externalCalls={}", first["id"], second["id"], json!(calls));
}
