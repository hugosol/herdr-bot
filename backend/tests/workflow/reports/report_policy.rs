//! S1: frozen approval mode, recovery and independent retry identity.
use super::*;

fn set_mode(s: &Scenario, auto: bool) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["A"]["auto_approved"] = json!(auto);
    fs::write(path, map.to_string()).unwrap();
}
async fn action(s: &Scenario, name: &str, body: Value) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn context(s: &Scenario, identity: &str) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/context", &json!({"sessionId":identity}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
// Preserve the existing public definition selector, not its obsolete protocol.
include!("../../scheduler/maps/report_policy.rs");

// P3/P15/P38: dispatch freezes manual; changing the Map to auto cannot change
// recovery. A retry inheriting either the old approval or report must fail the
// literal new-context and pending-report checks below. All transitions are HTTP.
#[tokio::test]
async fn approval_mode_is_frozen_recovery_keeps_approval_and_retry_reads_current_map() {
    let s = Scenario::without_output_contract().await;
    s.start("policy-manager").await;
    set_mode(&s, false);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-old"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("policy-manager", &["A"]).await;
    let initial = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(initial["autoApproved"], false);
    assert_eq!(initial["approved"], false);
    set_mode(&s, true);
    assert_eq!(context(&s, "policy-old").await["autoApproved"], false);
    approve_execution(&s, "policy-old").await;
    let old = action(&s, "status", json!({"sessionId":"policy-manager"})).await["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(old["approved"], true);
    assert_eq!(old["approvalSource"], "manual");
    let old_report = submit_report(&s, "policy-old", "Original approved handoff", true).await;
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Repair original work"})).await;
    let calls = s.herdr.calls();
    let recovered = action(&s, "recover", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"Original Chat repaired"})).await;
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["execution"], old);
    assert_eq!(s.herdr.calls(), calls);
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Choose independent execution"})).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = action(&s, "retry", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"User checked old effects"})).await;
    let node = &retried["snapshot"]["nodes"]["A"];
    assert_ne!(node["execution"]["id"], old["id"]);
    assert_eq!(node["execution"]["autoApproved"], true);
    assert_eq!(node["execution"]["approved"], false);
    assert_eq!(node["execution"]["approvalSource"], Value::Null);
    assert_eq!(node["previousExecutions"][0]["execution"], old);
    assert_eq!(retried["reports"], json!([old_report]));
    assert!(retried["pendingReports"].get("A").is_none());
    for (identity, auto, approved, source) in [("policy-old", false, true, json!("manual")), ("policy-new", true, false, Value::Null)] {
        let c = context(&s, identity).await;
        assert_eq!(c["autoApproved"], auto);
        assert_eq!(c["approved"], approved);
        assert_eq!(c["approvalSource"], source);
    }
    let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"policy-manager","nodeId":"A","reason":"Old approval cannot authorize replacement"}).to_string()).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    assert_eq!(serde_json::from_str::<Value>(&denied.body).unwrap()["code"], "node-not-approved");
    approve_execution(&s, "policy-new").await;
    let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"policy-manager","nodeId":"A","reason":"Old report cannot authorize replacement"}).to_string()).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    assert_eq!(serde_json::from_str::<Value>(&denied.body).unwrap()["code"], "report-not-completable");
    submit_report(&s, "policy-new", "Independent replacement handoff", true).await;
    let completed = action(&s, "complete", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Accept replacement only"})).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    println!("P3/P15/P38 initial={initial} old={old} retry={retried} completed={completed}");
}
