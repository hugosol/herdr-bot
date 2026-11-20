//! Real HTTP: frozen approval facts, recovery and independent retry identity.
use super::*;

fn set_policy(s: &Scenario, id: &str, policy: Value) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"][id]["auto_approved"] = policy;
    fs::write(path, map.to_string()).unwrap();
}

async fn action(s: &Scenario, name: &str, body: Value) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// Preserve the report_policy libtest module while locating the Map save contract with definitions.
include!("../../scheduler/maps/report_policy.rs");

// P3/P15/P38: recovery preserves approval, while retry inherits neither approval nor report eligibility.
#[tokio::test]
async fn approval_mode_is_frozen_recovery_keeps_approval_and_retry_reads_current_map() {
    let s = Scenario::without_output_contract().await;
    // Run creation is not a policy snapshot; dispatch reads the current Map.
    let started = s.start("policy-manager").await;
    set_policy(&s, "A", json!(false));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-old"},"native_cwd":s.data.path().join("node-zone")}));
    let first = s.dispatch("policy-manager", &["A"]).await;
    let initial = first["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(initial["autoApproved"], false);
    assert_eq!(initial["approved"], false);
    assert!(initial["approvalSource"].is_null());
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap()["nodes"]["A"]["execution"], initial);
    set_policy(&s, "A", json!(true));
    // Reading the live Map instead would incorrectly replace the frozen mode.
    let calls = s.herdr.calls();
    let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &json!({"sessionId":"policy-old"}).to_string()).await;
    assert_eq!(context.status, 200, "{}", context.body);
    assert_eq!(serde_json::from_str::<Value>(&context.body).unwrap(), json!({"runId":started["id"],"nodeId":"A","executionId":initial["id"],"autoApproved":false,"approved":false,"approvalSource":null}));
    for input in [json!({"sessionId":"policy-manager"}), json!({"sessionId":"unknown"}), json!({"sessionId":""}), json!({}), json!({"sessionId":null}), json!({"sessionId":"policy-old","runId":started["id"]}), json!(["policy-old"])] {
        let refused = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &input.to_string()).await;
        assert!(matches!(refused.status, 400 | 403), "{} {}", refused.status, refused.body);
        assert!(serde_json::from_str::<Value>(&refused.body).unwrap().get("autoApproved").is_none());
    }
    for (route, body) in [("/api/workflow/node/context?sessionId=policy-old", r#"{"sessionId":"policy-old"}"#), ("/api/workflow/node/context", "{")] {
        let refused = post_json_over_tcp(s.server.addr(), route, body).await;
        assert_eq!(refused.status, 400, "{}", refused.body);
    }
    let untouched = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    assert_eq!(untouched["reports"], json!([]));
    assert_eq!(s.herdr.calls(), calls);
    // A report alone does not approve the execution, even in manual mode.
    let old_report = submit_report(&s, "policy-old", "Original approved handoff", true).await;
    let reported = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    assert_eq!(reported["snapshot"]["nodes"]["A"]["execution"], initial);
    assert_eq!(reported["snapshot"]["nodes"]["A"]["status"], "running");
    approve_execution(&s, "policy-old").await;
    let old = action(&s, "status", json!({"sessionId":"policy-manager"})).await["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(old["approved"], true);
    assert_eq!(old["approvalSource"], "manual");
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Repair original work"})).await;
    let calls = s.herdr.calls();
    let recovered = action(&s, "recover", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"Original Chat repaired"})).await;
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["execution"], old);
    assert_eq!(s.herdr.calls(), calls);
    let status = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["status"], "running");
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Choose independent execution"})).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = action(&s, "retry", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"User checked old effects"})).await;
    let node = &retried["snapshot"]["nodes"]["A"];
    assert_eq!(node["execution"]["autoApproved"], true);
    assert_eq!(node["execution"]["approved"], false);
    assert!(node["execution"]["approvalSource"].is_null());
    assert_ne!(node["execution"]["id"], old["id"]);
    assert_eq!(node["previousExecutions"][0]["execution"], old);
    let before_lookup = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    let calls = s.herdr.calls();
    for (session, execution, policy, approved, source) in [("policy-old", &old["id"], false, true, json!("manual")), ("policy-new", &node["execution"]["id"], true, false, Value::Null)] {
        let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &json!({"sessionId":session}).to_string()).await;
        assert_eq!(context.status, 200, "{}", context.body);
        assert_eq!(serde_json::from_str::<Value>(&context.body).unwrap(), json!({"runId":started["id"],"nodeId":"A","executionId":execution,"autoApproved":policy,"approved":approved,"approvalSource":source}));
    }
    let after_lookup = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    assert_eq!(after_lookup["reports"], before_lookup["reports"]);
    assert_eq!(after_lookup["snapshot"], before_lookup["snapshot"]);
    assert_eq!(s.herdr.calls(), calls);
    assert_eq!(retried["reports"], json!([old_report]));
    assert!(retried["pendingReports"].get("A").is_none());
    // The replacement needs its own approval and ready report.
    let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"policy-manager","nodeId":"A","reason":"Old approval cannot authorize replacement"}).to_string()).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    assert_eq!(serde_json::from_str::<Value>(&denied.body).unwrap()["code"], "node-not-approved");
    approve_execution(&s, "policy-new").await;
    let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"policy-manager","nodeId":"A","reason":"Old report cannot authorize replacement"}).to_string()).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    assert_eq!(serde_json::from_str::<Value>(&denied.body).unwrap()["code"], "report-not-completable");
    submit_report(&s, "policy-new", "Replacement execution ready", true).await;
    let completed = action(&s, "complete", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Verified handoff"})).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    set_policy(&s, "B", json!(false));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-manual-complete"},"native_cwd":s.data.path().join("node-zone")}));
    let manual = s.dispatch("policy-manager", &["B"]).await;
    assert_eq!(manual["snapshot"]["nodes"]["B"]["execution"]["autoApproved"], false);
    approve_execution(&s, "policy-manual-complete").await;
    submit_report(&s, "policy-manual-complete", "User permitted report; work ready", true).await;
    let completed = action(&s, "complete", json!({"sessionId":"policy-manager","nodeId":"B","reason":"Manager verified approved handoff"})).await;
    assert_eq!(completed["snapshot"]["nodes"]["B"]["status"], "completed");
    println!("P3/P15/P38 initial={initial} old={old} retry={retried} completed={completed}");
}

#[tokio::test]
async fn current_execution_approval_is_frozen_without_read_rewrite_after_map_mode_changes() {
    let s = Scenario::without_output_contract().await;
    let started = s.start("policy-legacy-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-legacy-node"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("policy-legacy-manager", &["A"]).await;
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["execution"]["autoApproved"], true);
    let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
    approve_execution(&s, "policy-legacy-node").await;
    let mut legacy: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let original = fs::read(&path).unwrap();
    set_policy(&s, "A", json!(false));
    let status = action(&s, "status", json!({"sessionId":"policy-legacy-manager"})).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["execution"]["autoApproved"], true);
    assert_eq!(status["snapshot"]["nodes"]["A"]["execution"]["approved"], true);
    assert_eq!(status["snapshot"]["nodes"]["A"]["execution"]["approvalSource"], "auto");
    let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &json!({"sessionId":"policy-legacy-node"}).to_string()).await;
    assert_eq!(context.status, 200, "{}", context.body);
    assert_eq!(serde_json::from_str::<Value>(&context.body).unwrap(), json!({"runId":started["id"],"nodeId":"A","executionId":dispatched["snapshot"]["nodes"]["A"]["execution"]["id"],"autoApproved":true,"approved":true,"approvalSource":"auto"}));
    assert_eq!(fs::read(&path).unwrap(), original);
    legacy["nodes"]["A"]["execution"]["autoApproved"] = json!("invalid");
    fs::write(&path, legacy.to_string()).unwrap();
    let invalid = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(invalid.status, 500, "{}", invalid.body);
    let detail: Value = serde_json::from_str(&invalid.body).unwrap();
    assert_eq!(detail["code"], "run-read-failed");
    let context = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &json!({"sessionId":"policy-legacy-node"}).to_string()).await;
    assert_eq!(context.status, 500, "{}", context.body);
    let error: Value = serde_json::from_str(&context.body).unwrap();
    assert_eq!(error["code"], "run-read-failed");
    assert!(error.get("autoApproved").is_none());
}
