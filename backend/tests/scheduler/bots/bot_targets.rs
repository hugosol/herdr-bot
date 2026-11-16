use super::*;

// S2 P37: reference locations and unchanged bytes are independent observations.
// A scanner treating malformed/unreadable Bot files as an empty catalog would
// wrongly delete the Map (and, with deleteHistory, its run records).
#[tokio::test]
async fn map_delete_protects_bot_references_and_fails_closed_on_unverifiable_catalog() {
    let s = Scenario::new().await;
    let map = s.config.scheduler.map_directory.join("work.json");
    let before = fs::read(&map).unwrap();
    let bot = s.config.scheduler.bot_directory.join("nested.json");
    let reference = capability("nested", "work.json").to_string();
    fs::write(&bot, &reference).unwrap();
    let delete = || post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/delete", r#"{"deleteHistory":true}"#);
    let rejected = delete().await;
    assert!(rejected.status >= 400, "{}", rejected.body);
    let body: Value = serde_json::from_str(&rejected.body).unwrap();
    assert_eq!(body["code"], "map-referenced");
    for location in ["nested", "nested.json", "work.json"] { assert!(body["message"].as_str().unwrap().contains(location)); }
    assert_eq!(fs::read(&map).unwrap(), before);
    assert_eq!(fs::read_to_string(&bot).unwrap(), reference);
    fs::write(&bot, "{ broken").unwrap();
    let broken = delete().await;
    assert!(broken.status >= 400, "{}", broken.body);
    let body: Value = serde_json::from_str(&broken.body).unwrap();
    assert_eq!(body["code"], "map-reference-check-failed");
    assert!(body["message"].as_str().unwrap().contains("nested.json"));
    assert_eq!(fs::read(&map).unwrap(), before);
    fs::write(&bot, &reference).unwrap();
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        let locked = fs::OpenOptions::new().read(true).share_mode(0).open(&bot).unwrap();
        let unreadable = delete().await;
        assert!(unreadable.status >= 400, "{}", unreadable.body);
        assert_eq!(serde_json::from_str::<Value>(&unreadable.body).unwrap()["code"], "map-reference-check-failed");
        assert_eq!(fs::read(&map).unwrap(), before);
        drop(locked);
    }
    fs::remove_file(&bot).unwrap();
    #[cfg(windows)] {
        let outside = s.data.path().join("outside-bots");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("canary.txt"), "OUTSIDE-UNCHANGED").unwrap();
        let link = s.config.scheduler.bot_directory.join("linked.json");
        let output = std::process::Command::new("cmd.exe").args(["/C", "mklink", "/J"]).arg(&link).arg(&outside).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let linked = delete().await;
        assert!(linked.status >= 400, "{}", linked.body);
        assert_eq!(serde_json::from_str::<Value>(&linked.body).unwrap()["code"], "map-reference-check-failed");
        assert_eq!(fs::read(&map).unwrap(), before);
        assert_eq!(fs::read_to_string(outside.join("canary.txt")).unwrap(), "OUTSIDE-UNCHANGED");
        fs::remove_dir(link).unwrap();
    }
    let removed = delete().await;
    assert_eq!(removed.status, 200, "{}", removed.body);
    assert!(!map.exists());
    assert!(s.herdr.calls().is_empty());
}

fn capability(id: &str, map: &str) -> Value {
    json!({"id":id,"name":id,"when":"","plan_guide":"","node_guide":"","map_file":map})
}
fn call_node(bot: &str, manager: &str) -> Value {
    json!({"title":"Call","bot_id":bot,"input":"business","zone":"node","blocked_by":[],"manager_task_id":manager})
}
async fn check(s: &Scenario, map: &str, start: bool) -> common::HttpResponse {
    post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{map}/{}", if start {"runs"} else {"preflight"}),
        &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"literal request"}).to_string()).await
}

// S2 P07: A calls B twice and B calls C. A global visited set would reject
// the second sibling; a shallow traversal would accept the introduced B→A cycle.
// Calls use real files and the real public launch path, never a mocked validator.
#[tokio::test]
async fn recursive_preflight_accepts_repeated_calls_and_rejects_path_cycles_without_launch() {
    let s = Scenario::new().await;
    let maps = &s.config.scheduler.map_directory;
    let bots = &s.config.scheduler.bot_directory;
    fs::write(bots.join("call-b.json"), capability("call-b", "b.json").to_string()).unwrap();
    fs::write(bots.join("call-c.json"), capability("call-c", "work.json").to_string()).unwrap();
    let a = json!({"title":"A","description":"","nodes":{"first":call_node("call-b", &s.manager_task),"second":call_node("call-b", &s.manager_task)}});
    let b = json!({"title":"B","description":"","nodes":{"inner":call_node("call-c", &s.manager_task)}});
    fs::write(maps.join("a.json"), a.to_string()).unwrap();
    fs::write(maps.join("b.json"), b.to_string()).unwrap();
    let valid = check(&s, "a.json", false).await;
    assert_eq!(valid.status, 200, "{}", valid.body);
    for (file, target, chain) in [("call-b.json", "a.json", vec!["a.json", "a.json"]), ("call-c.json", "a.json", vec!["a.json", "b.json", "a.json"])] {
        let id = if file == "call-b.json" {"call-b"} else {"call-c"};
        fs::write(bots.join(file), capability(id, target).to_string()).unwrap();
        for start in [false, true] {
            let rejected = check(&s, "a.json", start).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            let body: Value = serde_json::from_str(&rejected.body).unwrap();
            let issue = body["issues"].as_array().unwrap().iter().find(|i| i["code"] == "map-call-cycle").expect("cycle issue");
            assert_eq!(issue["callChain"], json!(chain));
        }
        fs::write(bots.join(file), capability(id, if file == "call-b.json" {"b.json"} else {"work.json"}).to_string()).unwrap();
    }
    assert!(s.herdr.calls().is_empty());
    assert!(!s.config.scheduler.run_directory.exists() || fs::read_dir(&s.config.scheduler.run_directory).unwrap().next().is_none());
}


// P02 plus manager configuration: real public launches must never reach Herdr
// for drafts, missing targets, wrong roles or a node lacking a manager Task.
#[tokio::test]
async fn invalid_targets_and_manager_configuration_never_start_a_host() {
    let s = Scenario::new().await;
    let original: Value = serde_json::from_str(&fs::read_to_string(s.config.scheduler.bot_directory.join("worker.json")).unwrap()).unwrap();
    let normal = original["task_id"].as_str().unwrap();
    for (target, manager, expected) in [
        (None, None, "bot-task-required"),
        (Some(("task_id", "missing-task")), None, "task-missing"),
        (Some(("task_id", s.manager_task.as_str())), None, "task-role-invalid"),
        (Some(("map_file", "missing-map.json")), Some(s.manager_task.as_str()), "map-read-failed"),
        (Some(("map_file", "work.json")), None, "node-manager-required"),
        (Some(("map_file", "work.json")), Some(normal), "task-role-invalid"),
        (Some(("task_id", normal)), Some(s.manager_task.as_str()), "node-manager-unexpected"),
    ] {
        let mut bot = json!({"name":"Draft","when":"","plan_guide":"","node_guide":""});
        if let Some((field, value)) = target { bot[field] = json!(value); }
        let id = format!("candidate-{expected}-{}", manager.unwrap_or("none"));
        let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":id,"content":bot.to_string(),"revision":null}).to_string()).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let mut full = bot; full["id"] = json!("candidate");
        fs::write(s.config.scheduler.bot_directory.join("candidate.json"), full.to_string()).unwrap();
        let mut node = call_node("candidate", manager.unwrap_or(""));
        if manager.is_none() { node.as_object_mut().unwrap().remove("manager_task_id"); }
        fs::write(s.config.scheduler.map_directory.join("candidate.json"), json!({"title":"Candidate","description":"","nodes":{"call":node}}).to_string()).unwrap();
        for start in [false, true] {
            let rejected = check(&s, "candidate.json", start).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            let body: Value = serde_json::from_str(&rejected.body).unwrap();
            assert!(body["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == expected), "{body}");
        }
    }
    assert!(s.herdr.calls().is_empty());
}

// Approved S2: public definition HTTP plus persistent edit readback and host effects.
// P01/P02 independent outcome: a Map target survives save/reload; accepting both
// targets would silently choose a capability and must leave the original unchanged.
#[tokio::test]
async fn map_target_persists_and_dual_binding_never_replaces_definition() {
    let s = Scenario::new().await;
    let content = json!({"name":"Nested capability","when":"","plan_guide":"","node_guide":"","map_file":"work.json"});
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"nested","content":content.to_string(),"revision":null}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let edit = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"nested"}"#).await;
    assert_eq!(edit.status, 200, "{}", edit.body);
    let edit: Value = serde_json::from_str(&edit.body).unwrap();
    assert_eq!(edit["content"], content);
    let mut invalid = content;
    invalid["task_id"] = json!(s.manager_task);
    let rejected = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"nested","content":invalid.to_string(),"revision":edit["revision"]}).to_string()).await;
    assert!(rejected.status >= 400, "{}", rejected.body);
    let again = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/edit", r#"{"id":"nested"}"#).await;
    assert_eq!(serde_json::from_str::<Value>(&again.body).unwrap(), edit);
    assert!(s.herdr.calls().is_empty());
}
