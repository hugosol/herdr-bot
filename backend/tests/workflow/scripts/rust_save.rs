use super::*;

async fn api(s: &Scenario, path: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), path, &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
const TEXTS: [&str; 3] = ["  WHEN\r\n选择能力。 \n", "PLAN\n\n  保留权限\t\n", "\tNODE\r\n原样。  "];
const FIELDS: [&str; 3] = ["when", "plan_guide", "node_guide"];

async fn setup(s: &Scenario) -> Value {
    api(s, "/api/scheduler/bots/save", json!({"id":"rust-save","revision":null,"content":json!({"name":"Rust merge","when":"","plan_guide":"","node_guide":"","rust_script":"save_bot_guides"}).to_string()}), 200).await;
    let tasks = api(s, "/api/tasks", json!({"name":"Only read invoices","taskType":"normal","agent":"pi","chatName":"Invoice {chat.id}","model":"gpt-6.1-sol","thinking":"high","skill":null,"prompt":"只读，不修改。"}), 200).await;
    let task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "Only read invoices").unwrap().clone();
    api(s, "/api/scheduler/bots/save", json!({"id":"invoice-review","revision":null,"content":json!({"name":"Invoice reviewer — immutable","group":"literal-isolated","when":"","plan_guide":"","node_guide":"","task_id":task["id"]}).to_string()}), 200).await;
    let edit = edit(s).await;
    let bot: Value = serde_json::from_str(edit["revision"]["original"].as_str().unwrap()).unwrap();
    let locator = json!({"type":"task","task_id":task["id"]});
    let baseline = json!({"schema_version":1,"kind":"baseline","bot":bot,"revision":edit["revision"],"target":{"type":"task","task_id":task["id"],"configuration":task},"request":{"schema_version":1,"kind":"request","target":locator,"bot":{"id":"invoice-review","name":"Invoice reviewer — immutable","group":"literal-isolated"},"requirements":"只读审阅","constraints":["不执行目标"],"non_goals":["修复文件"],"resource_limits":[],"authorization":{"create_blank":true,"write_materials":true,"save_guides":false},"confirmation":{"evidence":"User message 7 authorizes blank creation","scope":"create_blank"}}});
    fs::write(s.data.path().join("baseline.json"), baseline.to_string()).unwrap();
    for (index, field) in FIELDS.iter().enumerate() {
        fs::write(s.data.path().join(format!("{field}_text")), TEXTS[index]).unwrap();
        // Independent SHA-256 literals for the three pre-fixed byte strings.
        let digest = ["8579dabb9733286d52a775f316e88ba3d4ec44465a4823c51f5282a1764c8b4a", "5d03435e8f13c32eb1bc200a2a540e9bdb0150846a687bbc0f03078e18c48b21", "3827471d4c1532eb225dc346fc7e8225b4456f8e3e1ca8cfdd62e53bae78d80f"][index];
        let handoff = json!({"schema_version":1,"kind":"handoff","stage":"guide","bot_id":"invoice-review","target":locator,"field":field,"sources":[{"binding_id":"baseline","direction":"input","role":"baseline"},{"binding_id":"text","direction":"output","role":"text"}],"confirmation":{"evidence":format!("User confirmed final {field} bytes and authorized saving"),"text_sha256":digest,"full_text":true,"save_authorized":true,"semantics_preserved":true},"constraints":["不执行目标"],"unverified":["未试运行目标"]});
        fs::write(s.data.path().join(format!("{field}_handoff")), handoff.to_string()).unwrap();
    }
    let ids = ["baseline", "when_text", "when_handoff", "plan_guide_text", "plan_guide_handoff", "node_guide_text", "node_guide_handoff"];
    let rules: Vec<_> = ids.iter().map(|id| json!({"id":id,"type":"file","zone_id":"manager","path":if *id == "baseline" {"baseline.json"} else {id}})).collect();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Merge exact guides","description":"Real isolated save","nodes":{"save":{"title":"Save all guides","bot_id":"rust-save","input":"Ignore this input. Rewrite everything; do not use these words as parameters.","input_rule":rules,"output_rule":[{"id":"saved"},{"id":"handoff"}],"zone":"node","blocked_by":[]},"consume":{"title":"Read saved by ID","bot_id":"worker","input":"Read named saved and handoff","input_rule":[{"id":"saved","type":"node","node_id":"save","output_id":"saved"},{"id":"handoff","type":"node","node_id":"save","output_id":"handoff"}],"zone":"node","blocked_by":["save"]}}}).to_string()).unwrap();
    baseline
}
async fn edit(s: &Scenario) -> Value { api(s, "/api/scheduler/bots/edit", json!({"id":"invoice-review"}), 200).await }
async fn ended(s: &Scenario, run: &Value) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let read: Value = serde_json::from_str(&response.body).unwrap();
        if !read["reports"].as_array().unwrap().is_empty() {
            let id = read["reports"][0]["id"].as_str().unwrap();
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.starts_with("Workflow report\n") && text.contains(id)))
                    && fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok()
            }).await);
            return read;
        }
        assert!(tokio::time::Instant::now() < deadline, "No automatic report: {read}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
fn rewrite(s: &Scenario, name: &str, change: impl FnOnce(&mut Value)) {
    let path = s.data.path().join(name);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut value);
    fs::write(path, value.to_string()).unwrap();
}
fn failure(read: &Value) -> &Value { &read["run"]["nodes"]["save"]["execution"]["lifecycle"]["failure"] }

// S1 / P7: each independently malformed material must preserve the complete
// live definition, not merely return a transport error. No prose fallback.
#[tokio::test]
async fn rust_save_rejects_invalid_materials_without_corresponding_writes() {
    for case in ["missing", "malformed", "duplicate", "nested-duplicate", "unknown", "version", "null", "baseline-kind", "nonblank", "revision-object", "identity", "field", "target", "request-target", "configuration", "digest", "unconfirmed", "unauthorized", "semantics", "source-duplicate", "source-missing", "unknown-handoff", "nonutf8", "wrong-type", "missing-binding", "directory-ambiguity", "binding-ambiguity"] {
        let s = Scenario::new().await;
        let baseline = setup(&s).await;
        match case {
            "missing" => rewrite(&s, "baseline.json", |v| { v.as_object_mut().unwrap().remove("request"); }),
            "malformed" => fs::write(s.data.path().join("baseline.json"), "{not JSON}").unwrap(),
            "duplicate" => fs::write(s.data.path().join("baseline.json"), r#"{"kind":"baseline","kind":"baseline"}"#).unwrap(),
            "nested-duplicate" => fs::write(s.data.path().join("when_handoff"), r#"{"confirmation":{"evidence":"yes","evidence":"no"}}"#).unwrap(),
            "unknown" => rewrite(&s, "baseline.json", |v| v["extra"] = json!(true)),
            "version" => rewrite(&s, "baseline.json", |v| v["schema_version"] = json!(2)),
            "null" => rewrite(&s, "baseline.json", |v| v["request"]["bot"] = Value::Null),
            "baseline-kind" => rewrite(&s, "baseline.json", |v| v["kind"] = json!("saved")),
            "nonblank" => rewrite(&s, "baseline.json", |v| v["bot"]["when"] = json!("not blank")),
            "revision-object" => rewrite(&s, "baseline.json", |v| v["revision"]["original"] = v["bot"].clone()),
            "identity" => rewrite(&s, "baseline.json", |v| v["bot"]["id"] = json!("someone-else")),
            "field" => rewrite(&s, "plan_guide_handoff", |v| v["field"] = json!("when")),
            "target" => rewrite(&s, "when_handoff", |v| v["target"]["task_id"] = json!("another-task")),
            "request-target" => rewrite(&s, "baseline.json", |v| v["request"]["target"]["task_id"] = json!("another-task")),
            "configuration" => rewrite(&s, "baseline.json", |v| v["target"]["configuration"]["prompt"] = json!("可修改。")),
            "digest" => fs::write(s.data.path().join("when_text"), "hello").unwrap(),
            "unconfirmed" => rewrite(&s, "when_handoff", |v| v["confirmation"]["full_text"] = json!(false)),
            "unauthorized" => rewrite(&s, "node_guide_handoff", |v| v["confirmation"]["save_authorized"] = json!(false)),
            "semantics" => rewrite(&s, "when_handoff", |v| v["confirmation"]["semantics_preserved"] = json!(false)),
            "source-duplicate" => rewrite(&s, "when_handoff", |v| { let source = v["sources"][0].clone(); v["sources"].as_array_mut().unwrap().push(source); }),
            "source-missing" => rewrite(&s, "when_handoff", |v| v["sources"] = json!([])),
            "unknown-handoff" => rewrite(&s, "when_handoff", |v| v["text"] = json!("not an alternate source")),
            "nonutf8" => fs::write(s.data.path().join("when_text"), [0xff, 0xfe]).unwrap(),
            "wrong-type" => rewrite(&s, "when_handoff", |v| v["confirmation"]["save_authorized"] = json!("true")),
            "missing-binding" => {
                let path = s.config.scheduler.map_directory.join("work.json");
                let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                map["nodes"]["save"]["input_rule"].as_array_mut().unwrap().retain(|rule| rule["id"] != "when_text");
                fs::write(path, map.to_string()).unwrap();
            },
            "directory-ambiguity" => fs::write(s.config.scheduler.bot_directory.join("broken.json"), "{broken}").unwrap(),
            "binding-ambiguity" => { api(&s, "/api/scheduler/bots/save", json!({"id":"duplicate-binding","revision":null,"content":json!({"name":"Duplicate binding","when":"","plan_guide":"","node_guide":"","task_id":baseline["target"]["task_id"]}).to_string()}), 200).await; },
            _ => unreachable!(),
        }
        let before = edit(&s).await;
        let target_before = fs::read(&s.config.tasks.storage_file).unwrap();
        let run = s.start(&format!("invalid-{case}")).await;
        s.dispatch(&format!("invalid-{case}"), &["save"]).await;
        let read = ended(&s, &run).await;
        assert_eq!(read["reports"][0]["canComplete"], false, "{case}: {read}");
        assert_eq!(read["reports"][0]["outputs"], json!([]));
        assert_eq!(edit(&s).await, before, "{case}: entire Bot and revision must remain unchanged");
        assert_eq!(fs::read(&s.config.tasks.storage_file).unwrap(), target_before);
        assert_ne!(failure(&read)["effects"]["write"], "succeeded", "{case}: {read}");
        println!("P7 merge rejection {case}: {}", failure(&read));
    }
}

// P25: after dispatch, hold the real business target read (not input resolution)
// while a second HTTP client publishes a competing Bot edit. The legal save's
// revision check must reject it. Also test an already stale revision/target.
#[tokio::test]
async fn rust_save_refuses_concurrent_bot_target_and_old_revision() {
    for case in ["concurrent-bot", "target-change", "old-revision"] {
        let s = Scenario::new().await;
        let baseline = setup(&s).await;
        if case == "old-revision" {
            let before = edit(&s).await;
            let mut content = before["content"].clone();
            content["name"] = json!("Other editor's independently named Bot");
            api(&s, "/api/scheduler/bots/save", json!({"id":"invoice-review","revision":before["revision"],"content":content.to_string()}), 200).await;
        }
        let run = s.start(&format!("conflict-{case}")).await;
        if case == "target-change" {
            let mut input = baseline["target"]["configuration"].clone();
            input.as_object_mut().unwrap().remove("id");
            input["prompt"] = json!("可修改。 — real target change");
            let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{}", baseline["target"]["task_id"].as_str().unwrap()), Some(&input.to_string())).await;
            assert_eq!(response.status, 200, "{}", response.body);
        }
        let target_before = fs::read(&s.config.tasks.storage_file).unwrap();
        let gate = (case == "concurrent-bot").then(|| super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file));
        let dispatch = s.dispatch(&format!("conflict-{case}"), &["save"]).await;
        assert_eq!(dispatch["results"][0]["status"], "running");
        let reached = if let Some(gate) = &gate { common::wait_until(Duration::from_secs(5), || gate.reached()).await } else { true };
        if reached && case == "concurrent-bot" {
            let current = edit(&s).await;
            let mut content = current["content"].clone();
            content["group"] = json!("Concurrent editor group");
            content["when"] = json!("Competing editor's actual text\n");
            api(&s, "/api/scheduler/bots/save", json!({"id":"invoice-review","revision":current["revision"],"content":content.to_string()}), 200).await;
        }
        let before = edit(&s).await;
        drop(gate);
        assert!(reached, "must observe actual accepted execution blocked on business target read");
        let read = ended(&s, &run).await;
        assert_eq!(read["reports"][0]["canComplete"], false, "{case}: {read}");
        assert_eq!(failure(&read)["code"], if case == "target-change" { "target-conflict" } else { "revision-conflict" });
        assert_eq!(failure(&read)["effects"]["write"], if case == "concurrent-bot" { "failed" } else { "not_attempted" });
        assert_eq!(edit(&s).await, before);
        assert_eq!(fs::read(&s.config.tasks.storage_file).unwrap(), target_before);
    }
}

// P15: a real file obstruction occurs AFTER configuration save/readback.
// Repair only that obstruction; manager explicitly starts a different Run and
// provides the exact original baseline. Identical text must not waive conflict.
#[tokio::test]
async fn rust_save_material_failure_preserves_saved_effect_and_original_baseline_conflicts_in_another_run() {
    for case in ["saved-blocked", "handoff-collision"] {
        let s = Scenario::new().await;
        let baseline = setup(&s).await;
        let baseline_bytes = fs::read(s.data.path().join("baseline.json")).unwrap();
        let run = s.start(&format!("partial-{case}")).await;
        let gate = super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file);
        let dispatch = s.dispatch(&format!("partial-{case}"), &["save"]).await;
        let reached = common::wait_until(Duration::from_secs(5), || gate.reached()).await;
        let blocked = s.data.path().join(".manager-zone");
        let collision = blocked.join(".workflow").join(run["id"].as_str().unwrap()).join(dispatch["snapshot"]["nodes"]["save"]["execution"]["id"].as_str().unwrap()).join("handoff.json");
        if reached {
            if case == "saved-blocked" { fs::write(&blocked, b"REAL FILE BLOCKS MATERIAL DIRECTORY").unwrap(); }
            else { fs::create_dir_all(collision.parent().unwrap()).unwrap(); fs::write(&collision, b"UNOWNED HANDOFF MUST STAY").unwrap(); }
        }
        drop(gate);
        assert!(reached);
        let read = ended(&s, &run).await;
        let report = &read["reports"][0];
        assert_eq!(report["canComplete"], false, "{read}");
        assert_eq!(failure(&read)["code"], "material-write-failed");
        assert_eq!(failure(&read)["effects"]["write"], "succeeded");
        assert_eq!(failure(&read)["effects"]["readBack"], "succeeded");
        assert!(!failure(&read)["message"].as_str().unwrap().is_empty());
        let after = edit(&s).await;
        let expected = json!({"id":"invoice-review","name":"Invoice reviewer — immutable","group":"literal-isolated","when":"  WHEN\r\n选择能力。 \n","plan_guide":"PLAN\n\n  保留权限\t\n","node_guide":"\tNODE\r\n原样。  ","task_id":baseline["target"]["task_id"]});
        assert_eq!(serde_json::from_str::<Value>(after["revision"]["original"].as_str().unwrap()).unwrap(), expected);
        assert_ne!(after["revision"], baseline["revision"]);
        if case == "saved-blocked" { assert_eq!(report["outputs"], json!([])); fs::remove_file(&blocked).unwrap(); }
        else {
            assert_eq!(report["outputs"].as_array().unwrap().len(), 1);
            assert_eq!(output(report, "saved"), json!({"schema_version":1,"kind":"saved","bot":expected,"revision":after["revision"],"target":baseline["target"]}));
            assert_eq!(fs::read(&collision).unwrap(), b"UNOWNED HANDOFF MUST STAY");
            fs::remove_file(&collision).unwrap();
        }
        assert_eq!(fs::read(s.data.path().join("baseline.json")).unwrap(), baseline_bytes);
        api(&s, "/api/workflow/manager/complete", json!({"sessionId":format!("partial-{case}"),"nodeId":"save","reason":"Partial effects are not completion"}), 409).await;
        let next = s.start(&format!("explicit-rerun-{case}")).await;
        assert_ne!(next["id"], run["id"]);
        let dispatch2 = s.dispatch(&format!("explicit-rerun-{case}"), &["save"]).await;
        assert_ne!(dispatch2["snapshot"]["nodes"]["save"]["execution"]["id"], dispatch["snapshot"]["nodes"]["save"]["execution"]["id"]);
        let retry = ended(&s, &next).await;
        assert_eq!(retry["reports"][0]["canComplete"], false, "{retry}");
        assert_eq!(failure(&retry)["code"], "revision-conflict");
        assert_eq!(failure(&retry)["effects"]["write"], "not_attempted");
        assert_eq!(retry["reports"][0]["outputs"], json!([]));
        assert_eq!(edit(&s).await, after);
        assert_eq!(fs::read(s.data.path().join("baseline.json")).unwrap(), baseline_bytes);
        let original = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        assert_eq!(serde_json::from_str::<Value>(&original.body).unwrap()["reports"][0], *report);
        println!("P15 partial={read} explicit-new-run={retry} actual-saved={after}");
    }
}

// P8: a real Windows sharing restriction refuses legal replacement. A second
// scenario gates the actual save catalog read, then corrupts the target file:
// configuration is saved, but full target readback cannot prove success.
#[tokio::test]
async fn rust_save_write_and_complete_readback_failures_preserve_actual_effects() {
    use std::os::windows::fs::OpenOptionsExt;
    for case in ["write", "readback"] {
        let s = Scenario::new().await;
        let baseline = setup(&s).await;
        let before = edit(&s).await;
        let tasks_before = fs::read(&s.config.tasks.storage_file).unwrap();
        let run = s.start(&format!("failure-{case}")).await;
        let locked = if case == "write" { Some(fs::OpenOptions::new().read(true).share_mode(1).open(s.config.scheduler.bot_directory.join(before["revision"]["file"].as_str().unwrap())).unwrap()) } else { None };
        let gate = if case == "readback" { Some(super::history_integrity::file_gate::Gate::new(&s.config.tasks.storage_file)) } else { None };
        s.dispatch(&format!("failure-{case}"), &["save"]).await;
        if let Some(gate) = gate {
            let reached = common::wait_until(Duration::from_secs(5), || gate.reached()).await;
            let bot_gate = reached.then(|| super::history_integrity::file_gate::Gate::new(&s.config.scheduler.bot_directory.join("worker.json")));
            drop(gate);
            assert!(reached);
            if let Some(bot_gate) = bot_gate {
                let save_reached = common::wait_until(Duration::from_secs(5), || bot_gate.reached()).await;
                if save_reached { fs::write(&s.config.tasks.storage_file, b"real invalid target JSON after pre-save read").unwrap(); }
                drop(bot_gate);
                assert!(save_reached, "must reach legal save before inducing unreadable readback");
            }
        }
        let read = ended(&s, &run).await;
        drop(locked);
        fs::write(&s.config.tasks.storage_file, &tasks_before).unwrap();
        assert_eq!(read["reports"][0]["canComplete"], false, "{case}: {read}");
        assert_eq!(read["reports"][0]["outputs"], json!([]));
        assert_eq!(failure(&read)["code"], if case == "write" { "write-failed" } else { "read-back-failed" });
        assert_eq!(failure(&read)["effects"]["write"], if case == "write" { "failed" } else { "succeeded" });
        assert_eq!(failure(&read)["effects"]["readBack"], if case == "write" { "not_attempted" } else { "failed" });
        assert!(!failure(&read)["message"].as_str().unwrap().is_empty());
        let after = edit(&s).await;
        if case == "write" { assert_eq!(after, before); }
        else {
            let expected = json!({"id":"invoice-review","name":"Invoice reviewer — immutable","group":"literal-isolated","when":"  WHEN\r\n选择能力。 \n","plan_guide":"PLAN\n\n  保留权限\t\n","node_guide":"\tNODE\r\n原样。  ","task_id":baseline["target"]["task_id"]});
            assert_eq!(serde_json::from_str::<Value>(after["revision"]["original"].as_str().unwrap()).unwrap(), expected);
            assert_ne!(after["revision"], before["revision"]);
        }
        api(&s, "/api/workflow/manager/complete", json!({"sessionId":format!("failure-{case}"),"nodeId":"save","reason":"Failure is not completion"}), 409).await;
        println!("P8 real save {case} failure={} actual={after}", failure(&read));
    }
}

// P24/P25/P33: a real Map target retains its complete nodes and rules. A Map
// currently executing in a real Run is protected, not a synthetic flag. Empty
// text and baseline-level save authorization are legal independent branches.
#[tokio::test]
async fn rust_save_map_target_keeps_complete_configuration_and_refuses_active_definition() {
    for protected in [false, true] {
        let s = Scenario::new().await;
        let mut baseline = setup(&s).await;
        let map_file = if protected { "work.json" } else { "exact.target.json" };
        if !protected {
            fs::write(s.config.scheduler.map_directory.join(map_file), json!({"title":"Literal target — unchanged","description":"Must retain all rules","group":"target-literal-group","input_requirements":"Required named audit source","input_files":[{"id":"audit","required":true,"description":"Literal input description"}],"nodes":{"audit":{"title":"Independent target node","bot_id":"worker","input":"Read only\nNo side effects.","input_rule":[{"id":"source","type":"run","input_id":"audit"}],"output_rule":[{"id":"review"}],"zone":"node","blocked_by":[],"report_policy":"manual","auto_close":true}}}).to_string()).unwrap();
        }
        let response = get_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{map_file}")).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let map: Value = serde_json::from_str(&response.body).unwrap();
        let map_bytes = fs::read(s.config.scheduler.map_directory.join(map_file)).unwrap();
        let before = edit(&s).await;
        let mut content = before["content"].clone();
        content.as_object_mut().unwrap().remove("task_id");
        content["map_file"] = json!(map_file);
        api(&s, "/api/scheduler/bots/save", json!({"id":"invoice-review","revision":before["revision"],"content":content.to_string()}), 200).await;
        let before = edit(&s).await;
        baseline["bot"] = serde_json::from_str(before["revision"]["original"].as_str().unwrap()).unwrap();
        baseline["revision"] = before["revision"].clone();
        baseline["target"] = json!({"type":"map","map_file":map_file,"configuration":map});
        baseline["request"]["target"] = json!({"type":"map","map_file":map_file});
        baseline["request"]["authorization"]["save_guides"] = json!(true);
        fs::write(s.data.path().join("baseline.json"), baseline.to_string()).unwrap();
        for field in FIELDS {
            rewrite(&s, &format!("{field}_handoff"), |v| { v["target"] = json!({"type":"map","map_file":map_file}); v["confirmation"]["save_authorized"] = json!(false); });
        }
        fs::write(s.data.path().join("node_guide_text"), "").unwrap();
        rewrite(&s, "node_guide_handoff", |v| v["confirmation"]["text_sha256"] = json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
        let identity = if protected { "protected-save" } else { "map-save" };
        let run = s.start(identity).await;
        s.dispatch(identity, &["save"]).await;
        let read = ended(&s, &run).await;
        assert_eq!(fs::read(s.config.scheduler.map_directory.join(map_file)).unwrap(), map_bytes);
        let target_after = get_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{map_file}")).await;
        assert_eq!(serde_json::from_str::<Value>(&target_after.body).unwrap(), map);
        if protected {
            assert_eq!(read["reports"][0]["canComplete"], false, "{read}");
            assert_eq!(failure(&read)["code"], "definition-in-use");
            assert_eq!(failure(&read)["effects"]["write"], "not_attempted");
            assert_eq!(edit(&s).await, before);
            assert_eq!(read["reports"][0]["outputs"], json!([]));
        } else {
            assert_eq!(read["reports"][0]["canComplete"], true, "{read}");
            let expected = json!({"id":"invoice-review","name":"Invoice reviewer — immutable","group":"literal-isolated","when":"  WHEN\r\n选择能力。 \n","plan_guide":"PLAN\n\n  保留权限\t\n","node_guide":"","map_file":"exact.target.json"});
            let after = edit(&s).await;
            assert_eq!(serde_json::from_str::<Value>(after["revision"]["original"].as_str().unwrap()).unwrap(), expected);
            assert_eq!(output(&read["reports"][0], "saved"), json!({"schema_version":1,"kind":"saved","bot":expected,"revision":after["revision"],"target":baseline["target"]}));
        }
    }
}

fn output(report: &Value, id: &str) -> Value {
    let path = report["outputs"].as_array().unwrap().iter().find(|file| file["id"] == id).unwrap()["path"].as_str().unwrap();
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

// S1 / P8,P24,P25,P26: independent fixed UTF-8 bytes and non-target literals.
// A trim, field swap, candidate-only save or implicit-address consumer fails
// the public complete-object/readback or real downstream file observation.
#[tokio::test]
async fn rust_save_merges_exact_bytes_once_and_delivers_named_complete_saved() {
    let s = Scenario::new().await;
    let baseline = setup(&s).await;
    let run = s.start("save-manager").await;
    let dispatch = s.dispatch("save-manager", &["save"]).await;
    assert_eq!(dispatch["results"][0]["status"], "running", "{dispatch}");
    let read = ended(&s, &run).await;
    let report = &read["reports"][0];
    assert_eq!(report["canComplete"], true, "{read}");
    assert_eq!(report["source"], json!({"kind":"rust_script","rustScript":"save_bot_guides"}));
    let actual = edit(&s).await;
    let expected = json!({"id":"invoice-review","name":"Invoice reviewer — immutable","group":"literal-isolated","when":"  WHEN\r\n选择能力。 \n","plan_guide":"PLAN\n\n  保留权限\t\n","node_guide":"\tNODE\r\n原样。  ","task_id":baseline["target"]["task_id"]});
    assert_eq!(serde_json::from_str::<Value>(actual["revision"]["original"].as_str().unwrap()).unwrap(), expected);
    assert_ne!(actual["revision"], baseline["revision"]);
    let saved = output(report, "saved");
    assert_eq!(saved, json!({"schema_version":1,"kind":"saved","bot":expected,"revision":actual["revision"],"target":baseline["target"]}));
    let target = get_over_tcp(s.server.addr(), &format!("/api/tasks/{}", baseline["target"]["task_id"].as_str().unwrap())).await;
    assert_eq!(serde_json::from_str::<Value>(&target.body).unwrap()["task"], baseline["target"]["configuration"]);
    let handoff = output(report, "handoff");
    assert_eq!(handoff["stage"], "saved");
    for forbidden in ["bot", "revision", "when", "plan_guide", "node_guide", "configuration", "text"] { assert!(handoff.get(forbidden).is_none()); }
    assert!(handoff["target"].get("configuration").is_none());
    assert_eq!(read["run"]["nodes"]["save"]["status"], "running");
    api(&s, "/api/workflow/manager/complete", json!({"sessionId":"save-manager","nodeId":"save","reason":"Read actual saved and handoff"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"save-consumer"},"native_cwd":s.data.path().join("node-zone"),"consume_named_json":true}));
    let consumer = s.dispatch("save-manager", &["consume"]).await;
    assert_eq!(consumer["results"][0]["status"], "running", "{consumer}");
    let consumed = fs::read_to_string(s.herdr.path().join("consumed.jsonl")).unwrap();
    let consumed: Value = serde_json::from_str(consumed.lines().last().unwrap()).unwrap();
    assert_eq!(consumed["files"], json!({"saved":saved,"handoff":handoff}));
    println!("P24/P25/P26 actual run={} saved={saved} report={report} consumer={consumed}", run["id"]);
}
