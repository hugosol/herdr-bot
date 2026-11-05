//! Enabling S2 checks only. Native business/paired Task evidence belongs to
//! 21/22, not to this controlled host. Expected values are literal materials.
use super::*;

async fn edit_bot(s: &Scenario, id: &str) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", &json!({"id":id}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn save_bot(s: &Scenario, id: &str, content: &Value, revision: &Value) -> common::HttpResponse {
    post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"content":content.to_string(),"revision":revision}).to_string()).await
}

// The material contract requires one complete candidate, real revision, full
// readback and non-target preservation for either binding. A last-write-wins
// save would accept the stale baseline and overwrite the independent edit.
#[tokio::test]
async fn shared_business_batch_save_preserves_target_and_rejects_stale_baseline() {
    let s = Scenario::new().await;
    let worker = edit_bot(&s, "worker").await;
    let task_id = worker["content"]["task_id"].as_str().unwrap();
    let task_before = get_over_tcp(s.server.addr(), &format!("/api/tasks/{task_id}")).await;
    assert_eq!(task_before.status, 200, "{}", task_before.body);
    let map_before = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(map_before.status, 200, "{}", map_before.body);
    for (id, binding) in [("shared-task", json!({"task_id":task_id})), ("shared-map", json!({"map_file":"work.json"}))] {
        let mut blank = json!({"name":"Unchanged business identity","when":"","plan_guide":"","node_guide":""});
        blank.as_object_mut().unwrap().extend(binding.as_object().unwrap().clone());
        let created = save_bot(&s, id, &blank, &Value::Null).await;
        assert_eq!(created.status, 200, "{}", created.body);
        let baseline = edit_bot(&s, id).await;
        assert_eq!(baseline["content"], blank);
        assert!(baseline["revision"]["original"].is_string());
        let mut candidate = blank.clone();
        candidate["when"] = json!("查找发票差异时使用。");
        candidate["plan_guide"] = json!("先取得已授权的发票清单，再安排差异核对。\n只交付差异报告。");
        candidate["node_guide"] = json!("提供本次清单、核对范围及报告保存授权。");
        let saved = save_bot(&s, id, &candidate, &baseline["revision"]).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let final_state = edit_bot(&s, id).await;
        assert_eq!(final_state["content"], candidate);
        assert_ne!(final_state["revision"], baseline["revision"]);
        let mut stale = candidate.clone();
        stale["when"] = json!("STALE MUST NOT REPLACE");
        let rejected = save_bot(&s, id, &stale, &baseline["revision"]).await;
        assert!(rejected.status >= 400, "{}", rejected.body);
        assert_eq!(edit_bot(&s, id).await, final_state);
    }
    assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/tasks/{task_id}")).await.body, task_before.body);
    assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body, map_before.body);
    assert!(s.herdr.calls().is_empty(), "configuration delivery cannot run business work");
}

// The approved materials are a baseline, one text and small handoff, all direct
// inputs. A framework that routes via handoff, copies bodies, or treats bound
// addresses as content snapshots disagrees at the actual host boundary.
#[tokio::test]
async fn shared_business_direct_materials_survive_save_preflight_and_dispatch() {
    let s = Scenario::new().await;
    let zone = s.data.path().join("node-zone");
    for (name, body) in [("baseline.md", "BASELINE-PRIVATE-573\nTarget and authority"), ("text.txt", "TEXT-PRIVATE-914\n唯一正文"), ("handoff.md", "CONFIRMATION-PRIVATE-628\nConfirmed range; no body or route")] {
        fs::write(zone.join(name), body).unwrap();
    }
    let draft = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"A"}"#).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let mut node = draft["content"].clone();
    node["input_rule"] = json!([
        {"id":"baseline","type":"file","path":"baseline.md","zone_id":"node"},
        {"id":"text","type":"file","path":"text.txt","zone_id":"node"},
        {"id":"handoff","type":"file","path":"handoff.md","zone_id":"node"}
    ]);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":node.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["nodes"]["A"]["input_rule"], node["input_rule"]);
    let preflight = || async { post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/preflight", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"Literal business scope"}).to_string()).await };
    let valid = preflight().await;
    assert_eq!(valid.status, 200, "{}", valid.body);
    assert!(s.herdr.calls().is_empty());
    // Preflight validates definitions, not future node-file availability.
    // A separate actual dispatch below owns the missing-material observation.
    s.start("shared-material-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"shared-material-node"},"native_cwd":zone}));
    let dispatched = s.dispatch("shared-material-manager", &["A"]).await;
    assert_eq!(dispatched["results"][0]["status"], "running");
    let inputs = &dispatched["snapshot"]["nodes"]["A"]["execution"]["inputs"];
    assert_eq!(inputs.as_array().unwrap().len(), 3);
    let calls = s.herdr.calls();
    let prompt = calls.iter().rev().find(|call| call["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap();
    for (id, name) in [("baseline", "baseline.md"), ("text", "text.txt"), ("handoff", "handoff.md")] {
        assert!(prompt.contains(&format!("{id}: {}", zone.join(name).display())), "{prompt}");
    }
    for excluded in ["BASELINE-PRIVATE-573", "TEXT-PRIVATE-914", "CONFIRMATION-PRIVATE-628", "BOT-WHEN-SECRET", "BOT-PLAN-SECRET", "BOT-NODE-SECRET"] {
        assert!(!prompt.contains(excluded), "body/guide was injected: {excluded}");
    }
    fs::write(zone.join("text.txt"), "Changed in place; bound address is not a snapshot").unwrap();
    assert_eq!(fs::read_to_string(zone.join("text.txt")).unwrap(), "Changed in place; bound address is not a snapshot");
    let run = dispatched["snapshot"]["id"].as_str().unwrap();
    let current = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{run}")).await;
    assert_eq!(current.status, 200, "{}", current.body);
    assert_eq!(&serde_json::from_str::<Value>(&current.body).unwrap()["run"]["nodes"]["A"]["execution"]["inputs"], inputs);
}

// A valid declaration is not evidence that its body can be consumed. The file
// really does not exist; no mock read error or successful fixture is supplied.
#[tokio::test]
async fn shared_business_missing_text_refuses_dispatch_without_starting_consumer() {
    let s = Scenario::new().await;
    let draft = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", r#"{"id":"A"}"#).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let mut node = draft["content"].clone();
    node["input_rule"] = json!([{"id":"text","type":"file","path":"not-produced.txt","zone_id":"node"}]);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":node.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    s.start("shared-missing-manager").await;
    let before = s.herdr.calls();
    let rejected = s.dispatch("shared-missing-manager", &["A"]).await;
    assert_eq!(rejected["results"][0]["status"], "failed");
    assert_eq!(s.herdr.calls(), before, "no consumer Tab, process or prompt may start");
    assert!(!s.data.path().join("node-zone/not-produced.txt").exists());
}
