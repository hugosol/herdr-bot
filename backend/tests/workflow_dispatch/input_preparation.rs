//! Public manager HTTP and external host effects own the preparation gate.
use super::*;
use super::file_consumption::declare_inputs;
use super::input_resolution::save_node;

async fn action(s: &Scenario, route: &str, body: Value, expected: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{route}"), &body.to_string()).await;
    assert_eq!(response.status, expected, "{route}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn snapshot(s: &Scenario) -> Value {
    action(s, "status", json!({"sessionId":"prepare-manager"}), 200).await["snapshot"].clone()
}
async fn prepare(s: &Scenario, files: Value, confirmed: bool, reason: &str, expected: u16) -> Value {
    action(s, "prepare_inputs", json!({"sessionId":"prepare-manager","fileInputs":files,"confirmed":confirmed,"reason":reason}), expected).await
}

// Fileless Runs still need semantic scope review; an empty binding list must
// not silently attest readiness at startup.
#[tokio::test]
async fn fileless_runs_require_explicit_preparation_before_business_dispatch() {
    let s = Scenario::new().await;
    let started = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"approved business scope; no inferred bindings"}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    // The fixture's initial native identity is manager-05; use the associated
    // identity rather than selecting a Run in manager requests.
    let run: Value = serde_json::from_str(&started.body).unwrap();
    let manager = run["run"]["manager"]["sessionId"].as_str().unwrap();
    assert_eq!(manager, "manager-05");
    assert_eq!(run["run"]["inputsReady"], false);
    let calls = s.herdr.calls();
    let rejected = action(&s, "dispatch", json!({"sessionId":manager,"nodes":["A"]}), 200).await;
    assert_eq!(rejected["results"][0]["status"], "rejected");
    assert_eq!(s.herdr.calls(), calls);
    action(&s, "prepare_inputs", json!({"sessionId":manager,"fileInputs":[],"confirmed":true,"reason":"No files needed; scope reviewed"}), 200).await;
    let status = action(&s, "status", json!({"sessionId":manager}), 200).await;
    assert_eq!(status["snapshot"]["inputsReady"], true);
}

#[tokio::test]
async fn drafts_confirmation_authority_and_replacement_preserve_the_gate() {
    let s = Scenario::new().await;
    declare_inputs(&s, json!([{"id":"brief","required":true},{"id":"approval","required":true}])).await;
    save_node(&s, "B", json!([{"id":"local","type":"run","input_id":"brief"}]), json!([])).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"prepare-manager"},"native_cwd":s.data.path()}));
    let started = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Business text is not semantic approval"}).to_string()).await;
    assert_eq!(started.status, 201, "{}", started.body);
    assert_eq!(snapshot(&s).await["inputsReady"], false);
    let calls = s.herdr.calls();
    for nodes in [json!(["A"]), json!(["A","B"])] {
        let rejected = action(&s, "dispatch", json!({"sessionId":"prepare-manager","nodes":nodes}), 200).await;
        assert!(rejected["results"].as_array().unwrap().iter().all(|result| result["status"] == "rejected"));
    }
    action(&s, "retry", json!({"sessionId":"prepare-manager","nodeId":"A","confirmed":true,"reason":"Not a preparation"}), 409).await;
    action(&s, "prepare_inputs", json!({"sessionId":"stranger","fileInputs":[],"confirmed":false,"reason":""}), 403).await;
    for invalid in [json!({"sessionId":"prepare-manager","fileInputs":[],"reason":"Missing confirmation"}), json!({"sessionId":"prepare-manager","fileInputs":[],"confirmed":false,"reason":"","runId":"caller-selected"}), json!({"sessionId":"prepare-manager","fileInputs":[{"id":"brief","path":"relative.txt","required":false}],"confirmed":false,"reason":""})] {
        action(&s, "prepare_inputs", invalid, 400).await;
    }
    assert_eq!(snapshot(&s).await["fileInputs"], json!([]));
    let brief = s.data.path().join("brief.txt");
    let approval = s.data.path().join("approval.txt");
    fs::write(&brief, "BRIEF-731").unwrap();
    fs::write(&approval, "APPROVAL-492").unwrap();
    let partial = json!([{"id":"brief","path":brief}]);
    prepare(&s, partial.clone(), false, "", 200).await;
    let draft = snapshot(&s).await;
    assert_eq!(draft["fileInputs"], partial);
    assert_eq!(draft["inputsReady"], false);
    for invalid in [json!([{"id":"alien","path":brief}]), json!([{"id":"brief","path":brief},{"id":"brief","path":brief}]), json!([{"id":"brief","path":s.data.path().join("missing.txt")}])] {
        prepare(&s, invalid, false, "Draft cannot bypass file validation", 409).await;
        assert_eq!(snapshot(&s).await, draft);
    }
    prepare(&s, partial, true, "Semantic review cannot supply missing approval", 409).await;
    assert_eq!(snapshot(&s).await, draft);
    let rejected = action(&s, "dispatch", json!({"sessionId":"prepare-manager","nodes":["A"]}), 200).await;
    assert_eq!(rejected["results"][0]["status"], "rejected");
    let complete = json!([{"id":"brief","path":brief},{"id":"approval","path":approval}]);
    for reason in ["", " \n\t "] {
        prepare(&s, complete.clone(), true, reason, 409).await;
        assert_eq!(snapshot(&s).await, draft);
    }
    prepare(&s, complete.clone(), false, "All paths are present but no semantic attestation", 200).await;
    assert_eq!(snapshot(&s).await["inputsReady"], false);
    assert_eq!(s.herdr.calls(), calls, "preparation must never dispatch business work");
    prepare(&s, complete.clone(), true, "Reviewed business scope and both material contents", 200).await;
    assert_eq!(snapshot(&s).await["inputsReady"], true);
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", serde_json::from_str::<Value>(&started.body).unwrap()["id"].as_str().unwrap())).await;
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["run"]["fileInputs"], complete);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"prepared-consumer"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = action(&s, "dispatch", json!({"sessionId":"prepare-manager","nodes":["B"]}), 200).await;
    assert_eq!(dispatched["results"][0]["status"], "running");
    assert_eq!(dispatched["snapshot"]["nodes"]["B"]["execution"]["inputs"], json!([{"id":"local","path":brief}]));
    // Editing remains legal after execution; replacement must remove old bindings
    // and clear readiness, without mutating the already dispatched address.
    prepare(&s, json!([]), false, "Reconsider materials", 200).await;
    let replaced = snapshot(&s).await;
    assert_eq!(replaced["fileInputs"], json!([]));
    assert_eq!(replaced["inputsReady"], false);
    assert_eq!(replaced["nodes"]["B"]["execution"]["inputs"], json!([{"id":"local","path":brief}]));
    let calls = s.herdr.calls();
    let rejected = action(&s, "dispatch", json!({"sessionId":"prepare-manager","nodes":["A"]}), 200).await;
    assert_eq!(rejected["results"][0]["status"], "rejected");
    assert_eq!(s.herdr.calls(), calls);
}
