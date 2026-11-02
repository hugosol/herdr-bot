//! Approved S1 (real HTTP) and S3 (real persisted records) seams.
use super::*;

async fn submit(s: &Scenario, body: Value, expected: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &body.to_string()).await;
    assert_eq!(response.status, expected, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn manager(s: &Scenario, action: &str, body: Value, expected: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body.to_string()).await;
    assert_eq!(response.status, expected, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// P2/P10/P11/P26: contract literals require explicit completion and absolute
// named paths. A legacy-only parser rejects this request; a file-checking or
// copying implementation fails on the deliberately nonexistent external paths.
#[tokio::test]
async fn report_registers_named_addresses_without_business_file_operations() {
    let s = Scenario::new().await;
    let started = s.start("outputs-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"outputs-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("outputs-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = fs::read(root.join("run.json")).unwrap();
    let background = fs::read(root.join("history.jsonl")).unwrap();
    let source = s.data.path().join("source.bin");
    fs::write(&source, b"independent-business-bytes\x00\xff").unwrap();
    let absent = s.data.path().join("outside-zone/not-created.txt");
    let outputs = json!([{"id":"A","path":source}, {"id":"B","path":absent}, {"id":"otherVolume","path":"Z:\\report-registration-owned-nonexistent\\result.bin"}]);
    let value = submit(&s, json!({"sessionId":"outputs-node","body":"Partial result; B has not been generated", "canComplete":false,"outputs":outputs}), 200).await;
    assert_eq!(value["saved"], true);
    assert_eq!(value["report"]["canComplete"], false);
    assert_eq!(value["report"]["outputs"], outputs);
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), vec![value["report"].clone()]);
    let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    let detail: Value = serde_json::from_str(&detail.body).unwrap();
    assert_eq!(detail["reports"], json!([value["report"]]));
    assert_eq!(fs::read(root.join("run.json")).unwrap(), before);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), background);
    assert_eq!(fs::read(source).unwrap(), b"independent-business-bytes\x00\xff");
    assert!(!absent.exists());
    assert!(!absent.parent().unwrap().exists());
    let mut files: Vec<_> = fs::read_dir(&root).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!(files, ["history.jsonl", "messages.jsonl", "run.json"]);
}

// P2/P26: accepting relative paths would turn an ambiguous address into a
// committed fact. Every invalid request must leave all three records and host
// calls unchanged; malformed legacy requests must not default to completion.
#[tokio::test]
async fn invalid_new_report_requests_never_append_or_notify() {
    let s = Scenario::new().await;
    let started = s.start("strict-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"strict-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("strict-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    let valid = json!({"sessionId":"strict-node","body":"Explicit declaration", "canComplete":true});
    for outputs in [json!([{"id":"A","path":"relative.txt"}]), json!([{"id":"A","path":"C:relative.txt"}]), json!([{"id":"A","path":"\\root-only.txt"}]), json!([{"id":"A","path":""}]), json!([{"id":"A","path":"C:\\nul\u{0000}file"}]), json!({"A":7}), json!(["C:\\absolute.txt"]), Value::Null] {
        let mut body = valid.clone();
        body["outputs"] = outputs;
        let rejected = submit(&s, body, 400).await;
        assert!(rejected["message"].is_string());
    }
    for outputs in [
        json!([{"id":"unknown","path":"C:/unknown.txt"}]),
        json!([{"id":"A","path":"C:/first.txt"},{"id":"A","path":"C:/second.txt"}]),
        json!([{"id":"A","path":"C:/valid.txt"},{"id":"unknown","path":"C:/unknown.txt"}]),
        json!([{"id":"","path":"C:/empty-id.txt"}]),
        json!([["A","C:/positional.txt"]]),
        json!([{"id":"A","path":7}]),
        json!([{"id":7,"path":"C:/typed.txt"}]),
        json!([{"id":"A","path":"C:/typed.txt","extra":true}]),
        json!({"A":"C:/legacy-map.txt"}),
    ] {
        let mut body = valid.clone();
        body["outputs"] = outputs;
        submit(&s, body, 400).await;
        assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
        assert_eq!(s.herdr.calls(), calls);
    }
    for field in ["canComplete", "body", "sessionId"] {
        let mut body = valid.clone();
        body.as_object_mut().unwrap().remove(field);
        submit(&s, body, 400).await;
    }
    for wrong in [Value::Null, json!(0), json!("true"), json!({}), json!([])] {
        let mut body = valid.clone();
        body["canComplete"] = wrong;
        submit(&s, body, 400).await;
    }
    for wrong in [Value::Null, json!(true), json!(7), json!([])] {
        let mut body = valid.clone();
        body["body"] = wrong;
        submit(&s, body, 400).await;
    }
    for field in ["runId", "nodeId", "executionId", "from", "to", "source", "target", "role"] {
        let mut body = valid.clone();
        body[field] = json!("forged");
        submit(&s, body, 400).await;
    }
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert_eq!(s.herdr.calls(), calls);
    let accepted = submit(&s, valid, 200).await;
    assert_eq!(accepted["report"]["canComplete"], true);
    assert_eq!(accepted["report"]["outputs"], json!([]));
}

// P8/P9/P11: the independent update sequence is A=a1/B=b1, then A=a2,
// then no outputs. Reports are deltas, not a second mutable registry. Retry
// must get a distinct execution; a late old B cannot become a new-run fact.
#[tokio::test]
async fn output_deltas_preserve_names_and_remain_scoped_to_the_real_execution() {
    let s = Scenario::new().await;
    let started = s.start("deltas-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"deltas-old"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("deltas-manager", &["A"]).await;
    let old = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    let mut reports = Vec::new();
    for outputs in [Some(json!([{"id":"A","path":"C:\\results\\a1"}, {"id":"B","path":"C:\\results\\b1"}])), Some(json!([{"id":"A","path":"C:\\results\\a2"}])), None] {
        let mut body = json!({"sessionId":"deltas-old","body":"Only the structured addresses register outputs","canComplete":false});
        if let Some(outputs) = outputs { body["outputs"] = outputs; }
        reports.push(submit(&s, body, 200).await["report"].clone());
    }
    manager(&s, "fail", json!({"sessionId":"deltas-manager","nodeId":"A","reason":"Independent new attempt required"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"deltas-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retried = manager(&s, "retry", json!({"sessionId":"deltas-manager","nodeId":"A","confirmed":true,"reason":"User approved independent execution"}), 200).await;
    let new = retried["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_ne!(old["id"], new["id"]);
    reports.push(submit(&s, json!({"sessionId":"deltas-new","body":"New execution has no outputs yet", "canComplete":true}), 200).await["report"].clone());
    reports.push(submit(&s, json!({"sessionId":"deltas-old","body":"Old cleanup only", "canComplete":true,"outputs":[{"id":"B","path":"C:\\results\\old-late"}]}), 200).await["report"].clone());
    assert_eq!(reports[0]["outputs"], json!([{"id":"A","path":"C:\\results\\a1"}, {"id":"B","path":"C:\\results\\b1"}]));
    assert_eq!(reports[1]["outputs"], json!([{"id":"A","path":"C:\\results\\a2"}]));
    assert_eq!(reports[2]["outputs"], json!([]));
    assert_eq!(reports[3]["outputs"], json!([]));
    for index in [0, 1, 2, 4] {
        assert_eq!(reports[index]["executionId"], old["id"]);
        assert_eq!(reports[index]["from"], old["chat"]);
    }
    assert_eq!(reports[3]["executionId"], new["id"]);
    assert_eq!(reports[3]["from"], new["chat"]);
    for report in &reports {
        assert_eq!(report["runId"], started["id"]);
        assert_eq!(report["nodeId"], "A");
        assert_eq!(report["to"]["sessionId"], "deltas-manager");
    }
    let status = manager(&s, "status", json!({"sessionId":"deltas-manager"}), 200).await;
    assert_eq!(status["reports"], json!(reports));
    assert_eq!(status["pendingReports"], json!({"A":reports[3]["id"]}));
    assert_eq!(status["snapshot"], retried["snapshot"]);
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")), reports);
}

#[tokio::test]
async fn execution_rules_are_frozen_until_an_explicit_retry() {
    let s = Scenario::new().await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let original_inputs = json!([{"id":"brief","type":"file","path":"C:/frozen/original.md"}]);
    let original_outputs = json!([{"id":"old"}]);
    map["nodes"]["A"]["input_rule"] = original_inputs.clone();
    map["nodes"]["A"]["output_rule"] = original_outputs.clone();
    fs::write(&path, map.to_string()).unwrap();
    s.start("frozen-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"frozen-old"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("frozen-manager", &["A"]).await;
    let old = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
    assert_eq!(old["inputRule"], original_inputs);
    assert_eq!(old["outputRule"], original_outputs);
    assert_eq!(old["inputs"], json!([{"id":"brief","path":"C:/frozen/original.md"}]));
    let draft = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let mut content = draft["content"].clone();
    content["input_rule"] = json!([{"id":"replacement","type":"file","path":"Z:/new/input.md"}]);
    content["output_rule"] = json!([{"id":"new"}]);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    submit(&s, json!({"sessionId":"frozen-old","body":"Original contract still applies","canComplete":true,"outputs":[{"id":"old","path":"C:/frozen/result.md"}]}), 200).await;
    submit(&s, json!({"sessionId":"frozen-old","body":"New map cannot widen old execution","canComplete":true,"outputs":[{"id":"new","path":"C:/forbidden.md"}]}), 400).await;
    let status = manager(&s, "status", json!({"sessionId":"frozen-manager"}), 200).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["execution"], old);
    manager(&s, "fail", json!({"sessionId":"frozen-manager","nodeId":"A","reason":"Explicit replacement"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"frozen-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retry = manager(&s, "retry", json!({"sessionId":"frozen-manager","nodeId":"A","confirmed":true,"reason":"New rule approved"}), 200).await;
    let current = &retry["snapshot"]["nodes"]["A"]["execution"];
    assert_ne!(current["id"], old["id"]);
    assert_eq!(current["inputRule"], content["input_rule"]);
    assert_eq!(current["outputRule"], content["output_rule"]);
    assert_eq!(current["inputs"], json!([{"id":"replacement","path":"Z:/new/input.md"}]));
    submit(&s, json!({"sessionId":"frozen-new","body":"No inherited declaration","canComplete":true,"outputs":[{"id":"old","path":"C:/old.md"}]}), 400).await;
    submit(&s, json!({"sessionId":"frozen-new","body":"Replacement output","canComplete":true,"outputs":[{"id":"new","path":"Z:/new/result.md"}]}), 200).await;
}

// P8: independent persisted associations can be ambiguous even if normal
// dispatch prevents identity reuse. Choosing the first match would commit to
// one of these two live Runs; HTTP rejection and unchanged records catch it.
#[tokio::test]
async fn ambiguous_node_associations_cannot_register_for_either_run() {
    let s = Scenario::new().await;
    let first = s.start("ambiguous-manager-one").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"ambiguous-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("ambiguous-manager-one", &["A"]).await;
    let second = s.start("ambiguous-manager-two").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"different-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("ambiguous-manager-two", &["A"]).await;
    let roots = [&first, &second].map(|run| s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()));
    let mut collision: Value = serde_json::from_slice(&fs::read(roots[1].join("run.json")).unwrap()).unwrap();
    collision["nodes"]["A"]["execution"]["chat"]["sessionId"] = json!("ambiguous-node");
    fs::write(roots[1].join("run.json"), collision.to_string()).unwrap();
    let before = roots.each_ref().map(|root| ["run.json","history.jsonl","messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()));
    let calls = s.herdr.calls();
    let denied = submit(&s, json!({"sessionId":"ambiguous-node","body":"Must not pick a Run", "canComplete":true,"outputs":[{"id":"A","path":"C:\\ambiguous.txt"}]}), 403).await;
    assert_eq!(denied["code"], "node-ambiguous");
    assert_eq!(roots.each_ref().map(|root| ["run.json","history.jsonl","messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap())), before);
    assert_eq!(s.herdr.calls(), calls);
}



