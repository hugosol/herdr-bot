use super::*;

async fn manager_action(s: &Scenario, action: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{action}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn start_unprepared(s: &Scenario, session: &str) -> Value {
    native(s, session);
    let response = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/runs", &json!({
        "managerTaskId":s.manager_task, "zone":s.zone, "userInput":"Freeze the graph for this Run"
    }).to_string()).await;
    assert_eq!(response.status, 201, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn native(s: &Scenario, identity: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path()}));
}

// P12 / S1,S3,S4: distinct legal graphs and rules make substituting the
// latest definition observable in preparation, each dispatch, retry and
// finish. Keep these same old/new controls when adding the public finish rule.
#[tokio::test]
async fn edited_source_map_cannot_change_prepare_dispatch_retry_or_finish() {
    let s = Scenario::new().await;
    let source = s.config.scheduler.map_directory.join("work.json");
    let old = json!({"title":"Original route","description":"Original graph",
        "map_input_rule":{"files":[{"id":"old-brief","required":true}]},
        "map_output_rule":{"description":"Hand the old Chat to the caller","files":[{"id":"old-result"}]},
        "nodes":{
            "A":{"title":"Original worker","bot_id":"worker","input":"ORIGINAL-WORK","zone":"manager","blocked_by":[],
                "input_rule":[{"id":"source","type":"run","run_file_ref":"old-brief"}]},
            "B":{"title":"Original handoff","bot_id":"worker","input":"ORIGINAL-HANDOFF","zone":"manager","blocked_by":["A"],"end_node":true}
        }});
    let new = json!({"title":"Replacement route","description":"Replacement graph",
        "map_input_rule":{"files":[{"id":"new-brief","required":true}]},
        "map_output_rule":{"files":[{"id":"new-result"}]},
        "nodes":{
            "A":{"title":"Replacement worker","bot_id":"worker","input":"REPLACEMENT-WORK","zone":"manager","blocked_by":["C"],
                "input_rule":[{"id":"source","type":"run","run_file_ref":"new-brief"}]},
            "C":{"title":"New prerequisite","bot_id":"worker","input":"NEW-PREREQUISITE","zone":"manager","blocked_by":[]}
        }});
    let old_bytes = format!("{old:#}\n");
    fs::write(&source, &old_bytes).unwrap();
    let started = start_unprepared(&s, "frozen-original").await;
    let id = started["id"].as_str().unwrap();
    let frozen = s.config.scheduler.run_directory.join(id).join("map.json");
    fs::write(&source, format!("{new:#}\n")).unwrap();

    let brief = s.data.path().join("brief.txt");
    fs::write(&brief, "independent named input").unwrap();
    let prepare = |session: &str, input: &str| json!({"sessionId":session,"fileInputs":[{"id":input,"path":brief}],"confirmed":true,"reason":"Reviewed the supplied brief"});
    manager_action(&s, "prepare_inputs", prepare("frozen-original", "new-brief"), 409).await;
    let ready = manager_action(&s, "prepare_inputs", prepare("frozen-original", "old-brief"), 200).await;
    assert_eq!(ready["ready"], json!(["A"]));
    assert_eq!(ready["snapshot"]["mapInput"], json!({"runInput":"Freeze the graph for this Run","files":[{"id":"old-brief","path":brief}]}));
    native(&s, "frozen-first-worker");
    let first = manager_action(&s, "dispatch", json!({"sessionId":"frozen-original","nodes":["A"]}), 200).await;
    assert_eq!(first["results"][0]["status"], "running");
    assert_eq!(first["snapshot"]["nodes"]["A"]["execution"]["inputs"], json!([{"id":"source","path":brief}]));
    manager_action(&s, "fail", json!({"sessionId":"frozen-original","nodeId":"A","reason":"First worker failed after starting"}), 200).await;
    let calls = s.herdr.calls();
    fs::remove_file(&brief).unwrap();
    let rejected = manager_action(&s, "retry", json!({"sessionId":"frozen-original","nodeId":"A","confirmed":true,"reason":"Original worker effects checked"}), 409).await;
    assert_eq!(rejected["code"], "run-inputs-invalid");
    assert!(rejected["message"].as_str().unwrap().contains("old-brief"));
    assert_eq!(s.herdr.calls(), calls);
    fs::write(&brief, "independent named input").unwrap();
    native(&s, "frozen-retry-worker");
    let retry = manager_action(&s, "retry", json!({"sessionId":"frozen-original","nodeId":"A","confirmed":true,"reason":"Original worker effects checked"}), 200).await;
    assert_eq!(retry["result"]["status"], "running");
    assert_eq!(retry["snapshot"]["nodes"]["A"]["execution"]["inputs"], json!([{"id":"source","path":brief}]));
    assert_ne!(retry["snapshot"]["nodes"]["A"]["execution"]["id"], first["snapshot"]["nodes"]["A"]["execution"]["id"]);
    let calls = s.herdr.calls();
    let inputs: Vec<_> = calls.iter().filter(|call| call["kind"] == "agent_prompt").map(common::prompt_body).filter(|text| text.contains("ORIGINAL-WORK")).collect();
    assert_eq!(inputs.len(), 2, "both actual worker prompts use the original definition");
    assert!(!calls.iter().any(|call| call["kind"] == "agent_prompt" && common::prompt_body(call).contains("REPLACEMENT-WORK")));
    approve_execution(&s, "frozen-retry-worker").await;
    submit_report(&s, "frozen-retry-worker", "Original work accepted", true).await;
    manager_action(&s, "complete", json!({"sessionId":"frozen-original","nodeId":"A","reason":"Reviewed original worker report"}), 200).await;
    native(&s, "frozen-handoff");
    let handoff = manager_action(&s, "dispatch", json!({"sessionId":"frozen-original","nodes":["B"]}), 200).await;
    assert_eq!(handoff["snapshot"]["nodes"]["B"]["execution"]["endNode"], true);
    let old_result = s.data.path().join("original-delivery.txt");
    let new_result = s.data.path().join("replacement-delivery.txt");
    fs::write(&old_result, b"ORIGINAL-RESULT").unwrap();
    fs::write(&new_result, b"REPLACEMENT-RESULT").unwrap();
    let old_output = json!({"files":[{"id":"old-result","path":old_result}]});
    let new_output = json!({"files":[{"id":"new-result","path":new_result}]});
    let wrong = manager_action(&s, "finish", json!({"sessionId":"frozen-original","mapOutput":new_output}), 409).await;
    assert_eq!(wrong["code"], "undeclared-output");
    assert!(wrong["snapshot"]["finished"].is_null());
    let finish = manager_action(&s, "finish", json!({"sessionId":"frozen-original","mapOutput":old_output}), 200).await;
    assert_eq!(finish["snapshot"]["mapOutput"], old_output);
    assert_eq!(finish["snapshot"]["status"], "completed");
    assert_eq!(finish["snapshot"]["finished"]["reason"], "handed-to-user");
    assert_eq!(fs::read(&frozen).unwrap(), old_bytes.as_bytes());

    let replacement = start_unprepared(&s, "frozen-replacement").await;
    assert_eq!(replacement["run"]["name"], "[M]Replacement route");
    manager_action(&s, "prepare_inputs", prepare("frozen-replacement", "old-brief"), 409).await;
    let ready = manager_action(&s, "prepare_inputs", prepare("frozen-replacement", "new-brief"), 200).await;
    assert_eq!(ready["ready"], json!(["C"]));
    assert_eq!(ready["snapshot"]["mapInput"]["files"], json!([{"id":"new-brief","path":brief}]));
    assert!(ready["snapshot"]["nodes"].get("B").is_none());
    let replacement_copy = s.config.scheduler.run_directory.join(replacement["id"].as_str().unwrap()).join("map.json");
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(replacement_copy).unwrap()).unwrap(), new);
    for (node, session) in [("C", "frozen-new-c"), ("A", "frozen-new-a")] {
        native(&s, session);
        let dispatched = manager_action(&s, "dispatch", json!({"sessionId":"frozen-replacement","nodes":[node]}), 200).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        approve_execution(&s, session).await;
        submit_report(&s, session, "Replacement work accepted", true).await;
        manager_action(&s, "complete", json!({"sessionId":"frozen-replacement","nodeId":node,"reason":"Reviewed replacement work"}), 200).await;
    }
    let wrong = manager_action(&s, "finish", json!({"sessionId":"frozen-replacement","mapOutput":old_output}), 409).await;
    assert_eq!(wrong["code"], "undeclared-output");
    assert!(wrong["snapshot"]["finished"].is_null());
    let newer = manager_action(&s, "finish", json!({"sessionId":"frozen-replacement","mapOutput":new_output}), 200).await;
    assert_eq!(newer["snapshot"]["mapOutput"], new_output);
    assert_eq!(newer["snapshot"]["status"], "completed");
    assert_eq!(fs::read(old_result).unwrap(), b"ORIGINAL-RESULT");
    assert_eq!(fs::read(new_result).unwrap(), b"REPLACEMENT-RESULT");
}

// P13 / S1,S4: a still-legitimate source would mask either failure if any
// runtime entry fell back to it. Failed actions must not mutate the Run.
#[tokio::test]
async fn missing_or_damaged_frozen_map_fails_without_using_valid_source() {
    let s = Scenario::new().await;
    let started = s.start("frozen-corruption").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let map = root.join("map.json");
    let files = ["run.json", "history.jsonl", "messages.jsonl"];
    let before = files.map(|file| fs::read(root.join(file)).unwrap());
    let calls = s.herdr.calls();
    assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.status, 200);
    for damage in [None, Some("{invalid snapshot"), Some(r#"{"title":"different graph","description":"","nodes":{}}"#)] {
        if let Some(damage) = damage { fs::write(&map, damage).unwrap(); }
        else { fs::remove_file(&map).unwrap(); }
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}/map")).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert!(response.body.contains("map.json"), "{}", response.body);
        for (action, body) in [
            ("status", json!({})),
            ("dispatch", json!({"nodes":["A"]})),
            ("retry", json!({"nodeId":"A","confirmed":true,"reason":"This Run cannot be retried"})),
            ("prepare_inputs", json!({"fileInputs":[],"confirmed":true,"reason":"The original Map is still readable"})),
            ("finish", json!({})),
        ] {
            let mut body = body;
            body["sessionId"] = json!("frozen-corruption");
            let failure = manager_action(&s, action, body, 500).await;
            assert!(failure["message"].as_str().unwrap().contains("map.json"), "{failure}");
        }
        assert_eq!(files.map(|file| fs::read(root.join(file)).unwrap()), before);
        assert_eq!(s.herdr.calls(), calls);
    }
}

// P55 / S1,S4: use the deployer's real directory boundary (a move before
// observation), not a product migration hook, and compare the entire tree.
#[tokio::test]
async fn offline_archive_keeps_every_original_byte_without_online_roles_or_history() {
    use std::{collections::BTreeMap, path::Path};

    fn tree(root: &Path) -> BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, directory: &Path, entries: &mut BTreeMap<std::path::PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                let relative = path.strip_prefix(root).unwrap().to_owned();
                if path.is_dir() {
                    entries.insert(relative, None);
                    visit(root, &path, entries);
                } else {
                    entries.insert(relative, Some(fs::read(path).unwrap()));
                }
            }
        }
        let mut entries = BTreeMap::new();
        visit(root, root, &mut entries);
        entries
    }

    let s = Scenario::new().await;
    let online = &s.config.scheduler.run_directory;
    let original = online.join("old-native-run");
    fs::create_dir_all(original.join("assets/empty")).unwrap();
    fs::write(original.join("run.json"), br#"{"id":"old-native-run","map":"work.json","createdAt":17,"managerTaskId":"legacy-manager-task","zone":"old-zone","workspaceName":"legacy","userInput":"Original legacy input","manager":{"sessionId":"legacy-manager","workspaceId":"w","tabId":"t","paneId":"p","agentName":"omp"},"status":"running","finished":null,"nodes":{}}
"#).unwrap();
    fs::write(original.join("history.jsonl"), b"{\"legacy\":\"unchanged\"}\r\n").unwrap();
    fs::write(original.join("messages.jsonl"), b"{\"body\":\"original report\"}\n").unwrap();
    fs::write(original.join("assets/raw.bin"), [0, 255, 13, 10, 128]).unwrap();
    let before = tree(&original);
    let archive = s.data.path().join("offline").join("old-native-run");
    fs::create_dir(archive.parent().unwrap()).unwrap();
    fs::rename(&original, &archive).unwrap();

    let fresh = s.start("archive-new-manager").await;
    let listed = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(listed.status, 200, "{}", listed.body);
    let list: Value = serde_json::from_str(&listed.body).unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["id"], fresh["id"]);
    for suffix in ["", "/map"] {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/old-native-run{suffix}")).await;
        assert!(!matches!(response.status, 200..=299), "{}", response.body);
    }
    let denied = manager_action(&s, "status", json!({"sessionId":"legacy-manager"}), 403).await;
    assert_eq!(denied["code"], "manager-unassociated");
    assert!(!original.exists());
    assert_eq!(tree(&archive), before);
}
