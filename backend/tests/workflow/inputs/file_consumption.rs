//! Approved seams: public definition/start/dispatch HTTP, controlled external
//! host effects, and independent persistent record readback. No private seam.
use super::*;
use super::input_resolution::{save_node, identity, report, initial_input, named_input};

pub(super) async fn declare_inputs(s: &Scenario, inputs: Value) {
    let route = "/api/scheduler/maps/work.json/inputs";
    let draft = get_over_tcp(s.server.addr(), route).await;
    assert_eq!(draft.status, 200, "{}", draft.body);
    let draft: Value = serde_json::from_str(&draft.body).unwrap();
    let mut edited = json!({"map_input_rule":{"files":inputs},"revision":draft["revision"]});
    if let Some(output) = draft.get("map_output_rule") { edited["map_output_rule"] = output.clone(); }
    let saved = post_json_over_tcp(s.server.addr(), route, &edited.to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
}

async fn start_inputs(s: &Scenario, manager: &str, files: Value) -> Value {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":manager},"native_cwd":s.data.path()}));
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"business only","fileInputs":files}).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    let started = serde_json::from_str(&response.body).unwrap();
    let _ = s.prepare_fixture(manager).await;
    started
}

// P10/P12: Map declarations are the independent source of scope; the local
// consumer name differs from the interface ID. A global binding table would
// give both Runs the same path; a fabricated optional path adds a second entry.
#[tokio::test]
async fn run_inputs_are_local_renamed_and_optional_absence_stays_absent() {
    let s = Scenario::new().await;
    declare_inputs(&s, json!([{"id":"brief","required":true},{"id":"extra","required":false}])).await;
    save_node(&s, "A", json!([{"id":"local","type":"run","input_id":"brief"},{"id":"optional","type":"run","input_id":"extra"}]), json!([])).await;
    let first = s.data.path().join("one.txt");
    let second = s.data.path().join("two.txt");
    fs::write(&first, b"ONE").unwrap();
    fs::write(&second, b"TWO").unwrap();
    let one = start_inputs(&s, "input-one", json!([{"id":"brief","path":first}])).await;
    let two = start_inputs(&s, "input-two", json!([{"id":"brief","path":second}])).await;
    for (manager, run, path, session) in [("input-one", one, first, "consumer-one"), ("input-two", two, second, "consumer-two")] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
        let result = s.dispatch(manager, &["A"]).await;
        assert_eq!(result["results"][0]["status"], "running", "{result}");
        let expected = json!([{"id":"local","path":path}]);
        assert_eq!(result["snapshot"]["nodes"]["A"]["execution"]["inputs"], expected);
        let prompt = s.herdr.calls().into_iter().rev().find(|c| c["kind"] == "agent_prompt").unwrap()["text"].as_str().unwrap().to_owned();
        assert!(prompt.contains(&format!("local: {}\n[/Named file inputs]", path.display())), "{prompt}");
        assert!(!prompt.contains("optional:"));
        let persisted: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
        assert_eq!(persisted["nodes"]["A"]["execution"]["inputs"], expected);
    }
}

// P11/P15: optional means omission only. A provided missing or exclusively
// locked address must fail both public start entries before allocating a Run.
// Metadata-only validation and swallowing optional errors are plausible faults.
#[cfg(windows)]
#[tokio::test]
async fn root_input_readability_rejects_missing_and_locked_optional_files() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    declare_inputs(&s, json!([{"id":"optional","required":false}])).await;
    let locked = s.data.path().join("root-locked.txt");
    fs::write(&locked, b"root material").unwrap();
    let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&locked).unwrap();
    assert!(fs::File::open(&locked).is_err());
    for path in [s.data.path().join("root-missing.txt"), locked, s.data.path().to_path_buf(), std::path::PathBuf::from("relative.txt")] {
        for entry in ["preflight", "runs"] {
            let response = post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/work.json/{entry}"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"no discovery","fileInputs":[{"id":"optional","path":path}]}).to_string()).await;
            assert_eq!(response.status, 400, "{entry} {path:?}: {}", response.body);
            let body: Value = serde_json::from_str(&response.body).unwrap();
            assert!(body["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "run-input-unreadable" && issue["inputId"] == "optional"), "{body}");
            assert!(s.herdr.calls().is_empty());
            assert!(!s.config.scheduler.run_directory.exists());
        }
    }
    drop(lock);
}

// P15: a real file with a Windows exclusive handle exists but cannot be opened.
// Metadata-only validation would launch it. Observe zero new host effects and a
// persisted resolve failure, not merely an error message from a mock.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_direct_file_fails_before_any_host_effect() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    let path = s.data.path().join("locked.txt");
    fs::write(&path, b"actual bytes").unwrap();
    save_node(&s, "A", json!([{"id":"local","type":"file","path":"locked.txt"}]), json!([])).await;
    let run = s.start("locked-direct-manager").await;
    let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
    assert!(fs::File::open(&path).is_err(), "platform must actually refuse access");
    let calls = s.herdr.calls();
    let result = s.dispatch("locked-direct-manager", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "failed", "{result}");
    assert_eq!(result["snapshot"]["nodes"]["A"]["failure"]["stage"], "resolve");
    assert_eq!(s.herdr.calls(), calls);
    let stored: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
    assert_eq!(stored["nodes"]["A"]["status"], "failed");
    assert_eq!(stored["nodes"]["A"]["execution"]["chat"], Value::Null);
    drop(lock);
}

// P11/P15: each source has success, missing and actual OS-denied read cases.
// Run input is valid at start, then disappears/locks before dispatch: a start-
// only check or treating provided optional as absent cannot satisfy this test.
#[cfg(windows)]
#[tokio::test]
async fn dispatch_rechecks_direct_report_and_run_file_availability() {
    use std::os::windows::fs::OpenOptionsExt;
    for source in ["file", "node", "run"] {
        for condition in ["readable", "missing", "locked"] {
            let s = Scenario::new().await;
            let business = TempDir::new("outside every zone");
            let path = if source == "file" { s.data.path().join("material.txt") } else { business.path().join("material.txt") };
            fs::write(&path, b"MATERIAL").unwrap();
            declare_inputs(&s, json!([{"id":"brief","required":false}])).await;
            let rule = match source {
                "file" => json!({"id":"local","type":"file","path":"material.txt"}),
                "node" => json!({"id":"local","type":"node","node_id":"A","output_id":"result"}),
                _ => json!({"id":"local","type":"run","input_id":"brief"}),
            };
            save_node(&s, "B", json!([rule]), json!([])).await;
            let run = start_inputs(&s, "availability-manager", json!([{"id":"brief","path":path}])).await;
            if source == "node" {
                identity(&s, "availability-producer");
                assert_eq!(s.dispatch("availability-manager", &["A"]).await["results"][0]["status"], "running");
                report(&s, "availability-producer", Some(json!([{"id":"result","path":path}]))).await;
            }
            let lock = if condition == "locked" { Some(fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap()) } else { None };
            if condition == "missing" { fs::remove_file(&path).unwrap(); }
            if condition != "readable" { assert!(fs::File::open(&path).is_err()); }
            identity(&s, "availability-consumer");
            let calls = s.herdr.calls();
            let result = s.dispatch("availability-manager", &["B"]).await;
            let expected = if condition == "readable" { "running" } else { "failed" };
            assert_eq!(result["results"][0]["status"], expected, "{source}/{condition}: {result}");
            if condition == "readable" {
                assert_eq!(result["snapshot"]["nodes"]["B"]["execution"]["inputs"], json!([{"id":"local","path":path}]));
                assert_eq!(named_input(&initial_input(&s)), format!("local: {}\n", path.display()));
            } else {
                assert_eq!(s.herdr.calls(), calls, "{source}/{condition} started work");
                assert_eq!(result["snapshot"]["nodes"]["B"]["failure"]["stage"], "resolve");
            }
            let persisted: Value = serde_json::from_slice(&fs::read(s.config.scheduler.run_directory.join(run["id"].as_str().unwrap()).join("run.json")).unwrap()).unwrap();
            assert_eq!(persisted["nodes"]["B"]["status"], expected);
            drop(lock);
        }
    }
}

#[tokio::test]
async fn input_driven_zone_requires_its_file_binding_before_launching_a_batch() {
    let s = Scenario::new().await;
    declare_inputs(&s, json!([{"id":"optional-placement","required":false}])).await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["A"]["zone"] = json!({"type":"dynamic","input_id":"local"});
    map["nodes"]["A"]["input_rule"] = json!([{"type":"run","id":"local","input_id":"optional-placement"}]);
    fs::write(path, map.to_string()).unwrap();
    let manager = "missing-placement-manager";
    s.start(manager).await;
    let before = s.herdr.calls();
    let result = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":manager,"nodes":["B","A"]}).to_string()).await;
    assert!(result.status >= 400 && result.body.contains("local"), "{}", result.body);
    assert_eq!(s.herdr.calls(), before);
    let status = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":manager}).to_string()).await;
    assert_eq!(status.status, 200, "{}", status.body);
    let nodes = &serde_json::from_str::<Value>(&status.body).unwrap()["snapshot"]["nodes"];
    for id in ["A", "B"] {
        assert_eq!(nodes[id]["status"], "pending");
        assert!(nodes[id]["execution"].is_null());
    }
}

#[tokio::test]
async fn input_driven_zone_uses_completed_output_and_rereads_it_on_retry() {
    let s = Scenario::new().await;
    let material = s.data.path().join("registered.json");
    let first = s.data.path().join("node-zone");
    let second = s.data.path().join("next-root");
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["A"]["output_rule"] = json!([{"id":"result"}]);
    map["nodes"]["B"]["zone"] = json!({"type":"dynamic","input_id":"placement"});
    map["nodes"]["B"]["blocked_by"] = json!(["A"]);
    map["nodes"]["B"]["input_rule"] = json!([{"id":"placement","type":"node","node_id":"A","output_id":"result"}]);
    fs::write(&map_path, map.to_string()).unwrap();
    let manager = "placement-manager";
    let run = s.start(manager).await;
    let before = s.herdr.calls();
    let blocked = s.dispatch(manager, &["B"]).await;
    assert_eq!(blocked["results"][0]["status"], "rejected", "{blocked}");
    assert_eq!(s.herdr.calls(), before);

    let attempt = |body: Value| {
        let addr = s.server.addr();
        async move { post_json_over_tcp(addr, "/api/workflow/manager/dispatch", &body.to_string()).await }
    };
    identity(&s, "placement-producer");
    assert_eq!(s.dispatch(manager, &["A"]).await["results"][0]["status"], "running");
    fs::write(&material, r#"{"zone_id":"node"}"#).unwrap();
    report(&s, "placement-producer", Some(json!([{"id":"result","path":material}]))).await;
    approve_execution(&s, "placement-producer").await;
    let completed = super::input_resolution::action(&s, "complete", manager, "A").await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");

    let before = s.herdr.calls();
    let body = json!({"sessionId":manager,"nodes":["B"]});
    fs::remove_file(&material).unwrap();
    let missing = attempt(body.clone()).await;
    assert!(missing.status >= 400 && missing.body.contains("placement"), "{}", missing.body);
    for contents in ["not JSON", r#"["node"]"#, "{}", r#"{"zone_id":null}"#, r#"{"zone_id":42}"#, r#"{"zone_id":"  "}"#,
        r#"{"other":{"zone_id":"node"}}"#, r#"{"zone_id":"node","zone_id":"node"}"#, r#"{"zone_id":"not-registered"}"#] {
        fs::write(&material, contents).unwrap();
        let rejected = attempt(body.clone()).await;
        assert!(rejected.status >= 400 && rejected.body.contains("placement"), "{contents}: {}", rejected.body);
    }
    fs::write(&material, r#"{"zone_id":"node","other":{"zone_id":"not-registered"}}"#).unwrap();
    let rejected = attempt(json!({"sessionId":manager,"nodes":["B"],"zoneIds":{"B":"node"}})).await;
    assert!(rejected.status >= 400, "{}", rejected.body);
    assert_eq!(s.herdr.calls(), before, "invalid zone material or a manual override must not launch");

    identity(&s, "placement-first");
    let dispatched = s.dispatch(manager, &["B"]).await;
    assert_eq!(dispatched["results"][0]["status"], "running", "{dispatched}");
    let previous = &dispatched["snapshot"]["nodes"]["B"]["execution"];
    let inputs = json!([{"id":"placement","path":material}]);
    assert_eq!(previous["zoneId"], "node");
    assert_eq!(previous["zone"], json!(first));
    assert_eq!(previous["inputs"], inputs);
    assert_eq!(named_input(&initial_input(&s)), format!("placement: {}\n", material.display()));
    let calls = s.herdr.calls();
    assert!(calls[before.len()..].iter().any(|call| call["kind"] == "tab_create" && call["cwd"] == json!(first)));
    let failed = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/fail", &json!({"sessionId":manager,"nodeId":"B","reason":"Original partial effects inspected"}).to_string()).await;
    assert_eq!(failed.status, 200, "{}", failed.body);
    fs::write(&material, r#"{"zone_id":"next","reused":true}"#).unwrap();
    let retry = |body: Value| {
        let addr = s.server.addr();
        async move { post_json_over_tcp(addr, "/api/workflow/manager/retry", &body.to_string()).await }
    };
    let retry_body = json!({"sessionId":manager,"nodeId":"B","confirmed":true,"reason":"Approved another execution"});
    let mut override_body = retry_body.clone();
    override_body["zoneId"] = json!("next");
    let before = s.herdr.calls();
    let rejected = retry(override_body).await;
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    fs::create_dir(&second).unwrap();
    let register = post_json_over_tcp(s.server.addr(), "/api/zones", &json!({"id":"next","path":second}).to_string()).await;
    assert_eq!(register.status, 200, "{}", register.body);
    fs::remove_dir(&second).unwrap();
    let unavailable = retry(retry_body.clone()).await;
    assert_eq!(unavailable.status, 409, "{}", unavailable.body);
    let unchanged: Value = serde_json::from_str(&unavailable.body).unwrap();
    assert_eq!(unchanged["snapshot"]["nodes"]["B"]["status"], "failed");
    assert_eq!(&unchanged["snapshot"]["nodes"]["B"]["execution"], previous);
    assert!(unchanged["snapshot"]["nodes"]["B"]["previousExecutions"].as_array().is_none_or(Vec::is_empty));
    assert_eq!(s.herdr.calls(), before);

    fs::create_dir(&second).unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"placement-retry"},"native_cwd":second}));
    let retried = retry(retry_body).await;
    assert_eq!(retried.status, 200, "{}", retried.body);
    let retried: Value = serde_json::from_str(&retried.body).unwrap();
    assert_eq!(retried["result"]["status"], "running", "{retried}");
    let current = &retried["snapshot"]["nodes"]["B"];
    assert_eq!(current["execution"]["zoneId"], "next");
    assert_eq!(current["execution"]["zone"], json!(second));
    assert_eq!(current["execution"]["inputs"], inputs);
    assert_eq!(&current["previousExecutions"][0]["execution"], previous);
    assert_ne!(current["execution"]["id"], previous["id"]);
    assert!(s.herdr.calls()[before.len()..].iter().any(|call| call["kind"] == "tab_create" && call["cwd"] == json!(second)));
    let detail = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
    assert_eq!(detail.status, 200, "{}", detail.body);
    assert_eq!(&serde_json::from_str::<Value>(&detail.body).unwrap()["run"]["nodes"]["B"], current);
}

// P15: use a real Windows junction (no symlink privilege required). Both the
// root public-file convention and zone-relative source must reject traversal,
// even though the target bytes are genuinely readable by this process.
#[cfg(windows)]
#[tokio::test]
async fn root_and_direct_sources_reject_real_junction_traversal() {
    let s = Scenario::new().await;
    let business = TempDir::new("junction target outside zone");
    fs::write(business.path().join("material.txt"), b"OUTSIDE").unwrap();
    let junction = s.data.path().join("escape");
    let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&junction).arg(business.path()).output().unwrap();
    assert!(made.status.success(), "{} {}", String::from_utf8_lossy(&made.stdout), String::from_utf8_lossy(&made.stderr));
    let linked = junction.join("material.txt");
    assert_eq!(fs::read(&linked).unwrap(), b"OUTSIDE");
    declare_inputs(&s, json!([{"id":"optional","required":false}])).await;
    for entry in ["preflight", "runs"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/work.json/{entry}"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":"","fileInputs":[{"id":"optional","path":linked}]}).to_string()).await;
        assert_eq!(response.status, 400, "{}", response.body);
        assert!(s.herdr.calls().is_empty());
    }
    save_node(&s, "A", json!([{"id":"local","type":"file","path":"escape/material.txt"}]), json!([])).await;
    s.start("junction-manager").await;
    let calls = s.herdr.calls();
    let result = s.dispatch("junction-manager", &["A"]).await;
    assert_eq!(result["results"][0]["status"], "failed");
    assert_eq!(result["snapshot"]["nodes"]["A"]["failure"]["stage"], "resolve");
    assert_eq!(s.herdr.calls(), calls);
    fs::remove_dir(junction).unwrap();
    assert_eq!(fs::read(business.path().join("material.txt")).unwrap(), b"OUTSIDE");
}

// P12: the source form cannot select another Run or an undeclared interface.
// Both rejection and original-definition readback matter; coercion to a local
// reference would otherwise silently change what the consumer receives.
#[tokio::test]
async fn run_sources_reject_undeclared_inputs_and_arbitrary_run_selection() {
    let s = Scenario::new().await;
    declare_inputs(&s, json!([{"id":"brief","required":false}])).await;
    let original = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body;
    for rule in [json!({"id":"local","type":"run","input_id":"unknown"}), json!({"id":"local","type":"run","input_id":"brief","run_id":"another-run"})] {
        let draft = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/edit", &json!({"id":"A"}).to_string()).await;
        let draft: Value = serde_json::from_str(&draft.body).unwrap();
        let mut content = draft["content"].clone();
        content["input_rule"] = json!([rule]);
        let rejected = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/nodes/save", &json!({"id":"A","content":content.to_string(),"revision":draft["revision"],"create":false}).to_string()).await;
        assert!(rejected.status >= 400, "{}", rejected.body);
        assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body, original);
        assert!(s.herdr.calls().is_empty());
    }
}

// P13/P14/P16: dispatch B at old address, update producer, then dispatch C.
// Repeat in an independent Run with another address. Read actual modified bytes
// and deleted-file errors, and all records/host effects over a bounded quiet
// window: copying/locking, rebinding B, global output state or repair/retry fails.
#[tokio::test]
async fn dispatched_addresses_stay_fixed_without_content_snapshot_or_repair() {
    let s = Scenario::new().await;
    let business = TempDir::new("mutable outside materials");
    let old = business.path().join("old.txt");
    let new = business.path().join("new.txt");
    let other = business.path().join("other.txt");
    fs::write(&old, b"OLD").unwrap();
    fs::write(&new, b"NEW").unwrap();
    fs::write(&other, b"OTHER").unwrap();
    let rule = json!([{"id":"renamed","type":"node","node_id":"A","output_id":"result"}]);
    save_node(&s, "B", rule.clone(), json!([])).await;
    save_node(&s, "C", rule, json!([])).await;
    let one = s.start("fixed-manager-one").await;
    identity(&s, "fixed-source-one");
    s.dispatch("fixed-manager-one", &["A"]).await;
    report(&s, "fixed-source-one", Some(json!([{"id":"result","path":old}]))).await;
    identity(&s, "fixed-old-consumer");
    let before = s.dispatch("fixed-manager-one", &["B"]).await;
    assert_eq!(before["results"][0]["status"], "running");
    assert_eq!(named_input(&initial_input(&s)), format!("renamed: {}\n", old.display()));
    let two = s.start("fixed-manager-two").await;
    identity(&s, "fixed-source-two");
    s.dispatch("fixed-manager-two", &["A"]).await;
    report(&s, "fixed-source-two", Some(json!([{"id":"result","path":other}]))).await;
    report(&s, "fixed-source-one", Some(json!([{"id":"result","path":new}]))).await;
    identity(&s, "fixed-new-consumer");
    let after = s.dispatch("fixed-manager-one", &["C"]).await;
    assert_eq!(after["results"][0]["status"], "running");
    assert_eq!(named_input(&initial_input(&s)), format!("renamed: {}\n", new.display()));
    assert_eq!(after["snapshot"]["nodes"]["B"]["execution"], before["snapshot"]["nodes"]["B"]["execution"]);
    identity(&s, "fixed-other-consumer");
    let separate = s.dispatch("fixed-manager-two", &["B"]).await;
    assert_eq!(separate["results"][0]["status"], "running");
    assert_eq!(named_input(&initial_input(&s)), format!("renamed: {}\n", other.display()));
    fs::write(&old, b"CHANGED IN PLACE").unwrap();
    let bound = &after["snapshot"]["nodes"]["B"]["execution"]["inputs"][0]["path"];
    assert_eq!(fs::read(bound.as_str().unwrap()).unwrap(), b"CHANGED IN PLACE");
    fs::remove_file(&old).unwrap();
    assert!(fs::read(bound.as_str().unwrap()).is_err());
    let paths: Vec<_> = [one, two].iter().map(|run| s.config.scheduler.run_directory.join(run["id"].as_str().unwrap())).collect();
    let records: Vec<_> = paths.iter().flat_map(|root| ["run.json", "history.jsonl", "messages.jsonl"].map(|name| (root.join(name), fs::read(root.join(name)).unwrap()))).collect();
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(300)).await;
    for (path, bytes) in records { assert_eq!(fs::read(path).unwrap(), bytes); }
    assert_eq!(s.herdr.calls(), calls, "no automatic work during 300ms observation");
    assert!(!old.exists());
    assert_eq!(fs::read_dir(business.path()).unwrap().count(), 2);
    assert!(!s.data.path().join(".manager-zone").join(".workflow").exists());
    assert_eq!(fs::read_dir(s.data.path().join("node-zone")).unwrap().count(), 0);
}
