//! Real HTTP and owned storage/host fixture; policy is prompt-only, not approval state.
use super::*;

fn set_policy(s: &Scenario, id: &str, policy: Value) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"][id]["report_policy"] = policy;
    fs::write(path, map.to_string()).unwrap();
}

async fn action(s: &Scenario, name: &str, body: Value) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

#[tokio::test]
async fn map_report_policy_defaults_round_trips_and_rejects_invalid_saves_without_writing() {
    let s = Scenario::new().await;
    let map_response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(map_response.status, 200);
    let map: Value = serde_json::from_str(&map_response.body).unwrap();
    assert_eq!(map["nodes"]["A"]["report_policy"], "auto");
    let template = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/new").await;
    assert_eq!(template.status, 200);
    assert_eq!(serde_json::from_str::<Value>(&template.body).unwrap()["content"]["report_policy"], "auto");
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    assert_eq!(edit.status, 200);
    let draft: Value = serde_json::from_str(&edit.body).unwrap();
    let path = s.config.scheduler.map_directory.join("work.json");
    let original = fs::read(&path).unwrap();
    for policy in [json!("AUTO"), json!("approved"), Value::Null, json!(false), json!({}), json!({"auto":null}), json!({"manual":null}), json!([])] {
        let mut content = draft["content"].clone();
        content["report_policy"] = policy;
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        let error: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(error["code"], "node-invalid");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    let mut content = draft["content"].clone();
    content["report_policy"] = json!("manual");
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(serde_json::from_str::<Value>(&saved.body).unwrap()["nodes"]["A"]["report_policy"], "manual");
    let readback = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(serde_json::from_str::<Value>(&readback.body).unwrap()["nodes"]["A"]["report_policy"], "manual");
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["nodes"]["A"]["report_policy"], "manual");
    set_policy(&s, "A", json!("invalid"));
    let rejected = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(rejected.status, 500, "{}", rejected.body);
    assert_eq!(serde_json::from_str::<Value>(&rejected.body).unwrap()["code"], "map-read-failed");
}

#[tokio::test]
async fn report_policy_is_fixed_per_execution_recovery_keeps_it_and_retry_reads_current_map() {
    let s = Scenario::new().await;
    // Run creation is not a policy snapshot; dispatch reads the current Map.
    let started = s.start("policy-manager").await;
    set_policy(&s, "A", json!("manual"));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-old"},"native_cwd":s.data.path().join("node-zone")}));
    let first = s.dispatch("policy-manager", &["A"]).await;
    let old = first["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(old["reportPolicy"], "manual");
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap()["nodes"]["A"]["execution"], old);
    set_policy(&s, "A", json!("auto"));
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Repair original work"})).await;
    let calls = s.herdr.calls();
    let recovered = action(&s, "recover", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"Original Chat repaired"})).await;
    assert_eq!(recovered["snapshot"]["nodes"]["A"]["execution"], old);
    assert_eq!(s.herdr.calls(), calls);
    // Manual is not a backend approval gate: report requires no approval token/state.
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"policy-old","body":"result.md; usable with documented limitations"}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let status = action(&s, "status", json!({"sessionId":"policy-manager"})).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["status"], "running");
    action(&s, "fail", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Choose independent execution"})).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = action(&s, "retry", json!({"sessionId":"policy-manager","nodeId":"A","confirmed":true,"reason":"User checked old effects"})).await;
    let node = &retried["snapshot"]["nodes"]["A"];
    assert_eq!(node["execution"]["reportPolicy"], "auto");
    assert_ne!(node["execution"]["id"], old["id"]);
    assert_eq!(node["previousExecutions"][0]["execution"], old);
    // complete remains the manager's existing responsibility, without a policy gate.
    let completed = action(&s, "complete", json!({"sessionId":"policy-manager","nodeId":"A","reason":"Verified handoff"})).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    set_policy(&s, "B", json!("manual"));
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-manual-complete"},"native_cwd":s.data.path().join("node-zone")}));
    let manual = s.dispatch("policy-manager", &["B"]).await;
    assert_eq!(manual["snapshot"]["nodes"]["B"]["execution"]["reportPolicy"], "manual");
    let completed = action(&s, "complete", json!({"sessionId":"policy-manager","nodeId":"B","reason":"Manager verified handoff without approval state"})).await;
    assert_eq!(completed["snapshot"]["nodes"]["B"]["status"], "completed");
}

#[tokio::test]
async fn old_execution_records_default_to_auto_without_rewriting_or_reading_map_policy() {
    let s = Scenario::new().await;
    let started = s.start("policy-legacy-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"policy-legacy-node"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("policy-legacy-manager", &["A"]).await;
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["execution"]["reportPolicy"], "auto");
    let path = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
    let mut legacy: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    legacy["nodes"]["A"]["execution"].as_object_mut().unwrap().remove("reportPolicy");
    fs::write(&path, legacy.to_string()).unwrap();
    let original = fs::read(&path).unwrap();
    set_policy(&s, "A", json!("manual"));
    let status = action(&s, "status", json!({"sessionId":"policy-legacy-manager"})).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["execution"]["reportPolicy"], "auto");
    assert_eq!(fs::read(&path).unwrap(), original);
    legacy["nodes"]["A"]["execution"]["reportPolicy"] = json!("invalid");
    fs::write(&path, legacy.to_string()).unwrap();
    let invalid = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(invalid.status, 500, "{}", invalid.body);
    let detail: Value = serde_json::from_str(&invalid.body).unwrap();
    assert_eq!(detail["code"], "run-read-failed");
}
