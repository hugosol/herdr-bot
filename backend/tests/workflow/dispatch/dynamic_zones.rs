//! Public definition/workflow HTTP, real isolated roots and controlled external host.
use super::*;

fn dynamic(s: &Scenario) {
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    map["nodes"]["A"]["zone"] = json!({"type":"dynamic"});
    map["nodes"]["A"]["input_rule"] = json!([{"type":"file","id":"source","path":"manager-source.txt"}]);
    fs::write(s.data.path().join("manager-source.txt"), "Manager-scoped input, not dynamic cwd").unwrap();
    fs::write(path, serde_json::to_vec_pretty(&map).unwrap()).unwrap();
}
async fn action(s: &Scenario, name: &str, body: Value) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn register(s: &Scenario, id: &str, path: &std::path::Path) {
    let response = post_json_over_tcp(s.server.addr(), "/api/zones", &json!({"id":id,"path":path}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
}

#[tokio::test]
async fn dynamic_zone_starts_unbound_and_rejects_entire_bad_batch_before_any_session() {
    let s = Scenario::new().await;
    dynamic(&s);
    let started = s.start("dynamic-manager").await;
    assert!(started["run"]["zoneBindings"].get("late").is_none());
    let before = s.herdr.calls();
    for body in [
        json!({"sessionId":"dynamic-manager","nodes":["B","A"]}),
        json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":""}}),
        json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"  "}}),
        json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"late"}}),
        json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"node","missing":"node"}}),
        json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"node","B":"node"}}),
        json!({"sessionId":"dynamic-manager","nodes":["B"],"zoneIds":{"B":"node"}}),
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/dispatch", &body.to_string()).await;
        assert!(response.status >= 400, "{}", response.body);
        assert_eq!(s.herdr.calls(), before, "rejected selections must not launch any selected node");
    }
    let late = s.data.path().join("late-root");
    fs::create_dir(&late).unwrap();
    register(&s, "late", &late).await;
    fs::remove_dir(&late).unwrap();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"late"}}).to_string()).await;
    assert!(response.status >= 400, "{}", response.body);
    assert_eq!(s.herdr.calls(), before);
    let status = action(&s, "status", json!({"sessionId":"dynamic-manager"})).await;
    assert_eq!(status["snapshot"]["nodes"]["A"]["status"], "pending");
    assert!(status["snapshot"]["nodes"]["A"]["execution"].is_null());
    fs::create_dir(&late).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"dynamic-node"},"native_cwd":late}));
    let dispatched = action(&s, "dispatch", json!({"sessionId":"dynamic-manager","nodes":["A"],"zoneIds":{"A":"late"}})).await;
    assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["execution"]["zoneId"], "late");
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["execution"]["zone"], json!(late));
    assert_eq!(dispatched["snapshot"]["nodes"]["A"]["execution"]["inputs"], json!([{"id":"source","path":s.data.path().join("manager-source.txt")} ]));
    assert!(dispatched["snapshot"]["zoneBindings"].get("late").is_none());
    assert!(s.herdr.calls().iter().any(|call| call["kind"] == "tab_create" && call["cwd"] == json!(late)));
    let map: Value = serde_json::from_str(&fs::read_to_string(s.config.scheduler.map_directory.join("work.json")).unwrap()).unwrap();
    assert_eq!(map["nodes"]["A"]["zone"], json!({"type":"dynamic"}));
}

#[tokio::test]
async fn dynamic_zone_retry_requires_new_selection_and_keeps_previous_execution_frozen() {
    let s = Scenario::new().await;
    dynamic(&s);
    let started = s.start("dynamic-retry-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"dynamic-old"},"native_cwd":s.data.path().join("node-zone")}));
    let first = action(&s, "dispatch", json!({"sessionId":"dynamic-retry-manager","nodes":["A"],"zoneIds":{"A":"node"}})).await;
    let old = first["snapshot"]["nodes"]["A"]["execution"].clone();
    action(&s, "fail", json!({"sessionId":"dynamic-retry-manager","nodeId":"A","reason":"Original left partial effects"})).await;
    let before = s.herdr.calls();
    for zone in [None, Some(""), Some("unregistered")] {
        let mut body = json!({"sessionId":"dynamic-retry-manager","nodeId":"A","confirmed":true,"reason":"Approved separate attempt"});
        if let Some(zone) = zone { body["zoneId"] = json!(zone); }
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/retry", &body.to_string()).await;
        assert_eq!(response.status, 409, "{}", response.body);
        let rejected: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(rejected["code"], "zone-selection-invalid");
        assert_eq!(s.herdr.calls(), before);
    }
    let next = s.data.path().join("retry-root");
    fs::create_dir(&next).unwrap();
    register(&s, "next", &next).await;
    let rebound = common::request_json_over_tcp(s.server.addr(), "PUT", "/api/zones", Some(&json!({"id":"node","path":next}).to_string())).await;
    assert_eq!(rebound.status, 200, "{}", rebound.body);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"dynamic-new"},"native_cwd":next}));
    let retried = action(&s, "retry", json!({"sessionId":"dynamic-retry-manager","nodeId":"A","zoneId":"next","confirmed":true,"reason":"Approved separate attempt in next root"})).await;
    let node = &retried["snapshot"]["nodes"]["A"];
    assert_eq!(retried["result"]["status"], "running", "{retried}");
    assert_eq!(node["previousExecutions"][0]["execution"], old);
    assert_eq!(node["execution"]["zoneId"], "next");
    assert_eq!(node["execution"]["zone"], json!(next));
    assert_ne!(node["execution"]["id"], old["id"]);
    assert_eq!(retried["snapshot"]["zoneBindings"], started["run"]["zoneBindings"]);
    let persisted: Value = serde_json::from_str(&fs::read_to_string(s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(persisted["nodes"]["A"], *node);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"fixed-after-dynamic-retry"},"native_cwd":s.data.path().join("node-zone")}));
    let fixed = s.dispatch("dynamic-retry-manager", &["B"]).await;
    assert_eq!(fixed["results"][0]["status"], "running", "{fixed}");
    assert_eq!(fixed["snapshot"]["nodes"]["B"]["execution"]["zoneId"], "node");
    assert_eq!(fixed["snapshot"]["nodes"]["B"]["execution"]["zone"], json!(s.data.path().join("node-zone")));
}

#[tokio::test]
async fn dynamic_zone_definition_save_round_trips_and_rejects_noncanonical_objects() {
    let s = Scenario::new().await;
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"A"}"#).await;
    assert_eq!(edit.status, 200, "{}", edit.body);
    let draft: Value = serde_json::from_str(&edit.body).unwrap();
    let original = fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap();
    let mut node = draft["content"].clone();
    node["input_rule"] = json!([{"type":"file","id":"placement","path":"registered.json"}]);
    for zone in [json!({"type":"dynamic","default":"node"}), json!({"type":"dynamic","value":"node"}), json!({"type":"fixed","value":"node"}), json!({}), json!(null), json!(true), json!([]),
        json!({"type":"dynamic","input_id":null}), json!({"type":"dynamic","input_id":42}), json!({"type":"dynamic","input_id":""}), json!({"type":"dynamic","input_id":" \t"}), json!({"type":"dynamic","input_id":"unknown"}), json!({"type":"dynamic","input_id":"placement","extra":true})] {
        node["zone"] = zone;
        let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":node.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read(s.config.scheduler.map_directory.join("work.json")).unwrap(), original);
    }
    node["zone"] = json!({"type":"dynamic"});
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":node.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let saved: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(saved["nodes"]["A"]["zone"], json!({"type":"dynamic"}));
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"A"}"#).await;
    assert_eq!(edit.status, 200, "{}", edit.body);
    let draft: Value = serde_json::from_str(&edit.body).unwrap();
    node["zone"] = json!({"type":"dynamic","input_id":"placement"});
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":node.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let saved: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(saved["nodes"]["A"]["zone"], node["zone"]);
    // Dynamic execution locations do not relax explicitly fixed file input zones.
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    map["nodes"]["A"]["input_rule"] = json!([{"type":"file","id":"source","path":"input.txt","zone_id":"not-registered"}]);
    fs::write(path, map.to_string()).unwrap();
    let preflight = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/preflight", &json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":""}).to_string()).await;
    assert_eq!(preflight.status, 400, "{}", preflight.body);
    assert!(s.herdr.calls().is_empty());
}
