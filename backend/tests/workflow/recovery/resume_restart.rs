//! P32/P33 seams: public HTTP, persisted facts, controlled external Herdr.
//! Native host loading and post-resume tool wiring belong to ticket 15.
use super::*;

async fn request(s: &Scenario, path: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), path, &body.to_string()).await;
    assert_eq!(response.status, status, "{path}: {}", response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn action(s: &Scenario, name: &str, body: Value) -> Value {
    request(s, &format!("/api/workflow/manager/{name}"), body, 200).await
}
fn native(s: &Scenario, session: &str) {
    input_resolution::identity(s, session);
}
fn facts(s: &Scenario) -> std::collections::BTreeMap<String, [Vec<u8>; 3]> {
    fs::read_dir(&s.config.scheduler.run_directory).unwrap().map(|entry| {
        let entry = entry.unwrap();
        (entry.file_name().to_string_lossy().into_owned(), ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(entry.path().join(name)).unwrap()))
    }).collect()
}
async fn nested(s: &Scenario) {
    request(s, "/api/scheduler/bots/save", json!({"id":"nested","revision":null,"content":json!({"name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string()}), 200).await;
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{"internal":{"title":"Internal work","bot_id":"worker","input":"INNER","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Outer","description":"","nodes":{"A":{"title":"Call","bot_id":"nested","manager_task_id":s.manager_task,"input":"TRIGGER-ONLY-731","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
}

// Independent expectation from P32: both retained and current Chats load the
// same two roles and latest source environment, without new Run/input. A
// latest-attempt-only lookup or trigger replay would violate these observations.
#[tokio::test]
async fn dual_role_current_and_retired_chat_resume_preserves_all_facts_and_uses_current_environment() {
    let s = Scenario::new().await;
    nested(&s).await;
    let root = s.start("resume-root").await;
    native(&s, "resume-old");
    let first = s.dispatch("resume-root", &["A"]).await;
    let old = first["snapshot"]["nodes"]["A"]["execution"].clone();
    action(&s, "fail", json!({"sessionId":"resume-root","nodeId":"A","reason":"Confirmed original failure"})).await;
    native(&s, "resume-current");
    let retried = action(&s, "retry", json!({"sessionId":"resume-root","nodeId":"A","confirmed":true,"reason":"Authorized independent retry"})).await;
    let current = &retried["snapshot"]["nodes"]["A"]["execution"];
    assert_ne!(current["childRunId"], old["childRunId"]);
    // Change the live source mapping, not the execution or historical source.
    let mut catalog: toml::Value = toml::from_str(&fs::read_to_string(&s.config.tasks.metadata_file).unwrap()).unwrap();
    for role in ["manager", "node"] {
        let path = s.data.path().join(format!("latest-{role}.ts"));
        fs::write(&path, "export default function () {}\n").unwrap();
        let entry = catalog["extensions"].as_array_mut().unwrap().iter_mut().find(|entry| entry["id"].as_str() == Some(&format!("omp-{role}-chat"))).unwrap();
        entry["entrypoint"] = toml::Value::String(path.to_string_lossy().into_owned());
    }
    fs::write(&s.config.tasks.metadata_file, toml::to_string(&catalog).unwrap()).unwrap();
    let before = facts(&s);
    for session in ["resume-current", "resume-old"] {
        native(&s, session);
        let offset = s.herdr.calls().len();
        let result = request(&s, "/api/chats/resume", json!({"sessionId":session}), 200).await;
        assert_eq!(result["outcome"], "restored");
        let calls = s.herdr.calls();
        let starts: Vec<_> = calls[offset..].iter().filter(|call| call["kind"] == "agent_start").collect();
        assert_eq!(starts.len(), 1);
        let args = starts[0]["args"].as_array().unwrap();
        for role in ["manager", "node"] {
            assert!(args.contains(&json!(s.data.path().join(format!("latest-{role}.ts")))));
            assert!(!args.contains(&json!(s.data.path().join(format!("omp-{role}-chat.ts")))));
        }
        assert!(args.windows(2).any(|pair| pair == [json!("--resume"), json!(session)]));
        assert!(!args.contains(&json!("--model")) && !args.contains(&json!("--thinking")));
        assert!(!calls[offset..].iter().any(|call| call["kind"] == "agent_prompt" || call["kind"] == "agent_input"));
        assert_eq!(facts(&s), before, "Chat resume must not create Runs or modify either execution");
        let status = action(&s, "status", json!({"sessionId":session})).await;
        let expected = if session == "resume-old" { &old } else { current };
        assert_eq!(status["snapshot"]["id"], expected["childRunId"]);
        assert_eq!(status["snapshot"]["parent"], json!({"runId":root["id"],"nodeId":"A","executionId":expected["id"]}));
    }
}

// P32: matching source Task alone must not authorize a dual role. A loader
// falling back to manager on invalid links would launch a partial-role Chat.
// Keep real persisted conflicts in path and observe rejection before startup.
#[tokio::test]
async fn dual_role_resume_rejects_broken_parent_duplicate_identity_and_wrong_source_without_starting() {
    for fault in ["wrong-parent", "duplicate", "wrong-source"] {
        let s = Scenario::new().await;
        nested(&s).await;
        let root = s.start("invalid-root").await;
        native(&s, "invalid-child");
        let dispatched = s.dispatch("invalid-root", &["A"]).await;
        let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
        let child_root = s.config.scheduler.run_directory.join(execution["childRunId"].as_str().unwrap());
        let child_path = child_root.join("run.json");
        let mut child: Value = serde_json::from_slice(&fs::read(&child_path).unwrap()).unwrap();
        match fault {
            "wrong-parent" => {
                child["parent"]["executionId"] = json!("not-the-parent-attempt");
                fs::write(&child_path, child.to_string()).unwrap();
            }
            "duplicate" => {
                let duplicate = s.config.scheduler.run_directory.join("duplicate-child");
                fs::create_dir(&duplicate).unwrap();
                child["id"] = json!("duplicate-child");
                fs::write(duplicate.join("run.json"), child.to_string()).unwrap();
                for name in ["history.jsonl", "messages.jsonl"] { fs::write(duplicate.join(name), b"").unwrap(); }
            }
            "wrong-source" => {
                let path = s.config.scheduler.run_directory.join(root["id"].as_str().unwrap()).join("run.json");
                let mut parent: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                parent["nodes"]["A"]["execution"]["taskId"] = json!("not-the-source-task");
                fs::write(path, parent.to_string()).unwrap();
            }
            _ => unreachable!(),
        }
        let before = facts(&s);
        let offset = s.herdr.calls().len();
        let rejected = request(&s, "/api/chats/resume", json!({"sessionId":"invalid-child"}), 502).await;
        assert_eq!(rejected["code"], "execution-association-invalid", "{fault}: {rejected}");
        assert_eq!(rejected["stage"], "settings");
        assert_eq!(facts(&s), before);
        assert!(!s.herdr.calls()[offset..].iter().any(|call| matches!(call["kind"].as_str(), Some("tab_create" | "agent_start" | "agent_prompt" | "agent_input"))), "{fault}: invalid identity started a Chat");
    }
}
