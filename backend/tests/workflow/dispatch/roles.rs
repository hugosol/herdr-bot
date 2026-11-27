//! Public HTTP + persisted Run facts + production adapter argv, not native extension execution.
use super::*;

#[tokio::test]
async fn node_entrypoint_is_required_for_dispatch_but_resume_reports_local_degradation() {
    for host in ["omp", "pi"] {
        let s = Scenario::new().await;
        let response = get_over_tcp(s.server.addr(), "/api/tasks").await;
        let tasks: Value = serde_json::from_str(&response.body).unwrap();
        let mut task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "node").unwrap().clone();
        let id = task.as_object_mut().unwrap().remove("id").unwrap();
        task["agent"] = json!(host);
        let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{}", id.as_str().unwrap()), Some(&task.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        s.start("role-manager").await;
        identity(&s, host, "role-node-degraded");
        let dispatched = s.dispatch("role-manager", &["A"]).await;
        assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running");
        fs::remove_file(s.data.path().join(format!("{host}-node-chat.ts"))).unwrap();
        let offset = s.herdr.calls().len();
        let rejected = s.dispatch("role-manager", &["B"]).await;
        assert_eq!(rejected["snapshot"]["nodes"]["B"]["status"], "failed");
        assert!(rejected["snapshot"]["nodes"]["B"]["failure"]["message"].as_str().unwrap().contains("node-chat"));
        assert!(!s.herdr.calls()[offset..].iter().any(|call| call["kind"] == "agent_start"));
        let response = post_json_over_tcp(s.server.addr(), "/api/chats/resume", r#"{"sessionId":"role-node-degraded"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let result: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(result["outcome"], "restored");
        assert!(result["degradations"].as_array().unwrap().iter().any(|item| item["reason"].as_str().unwrap().contains("node-chat")));
        assert_role(&s, offset, host, false, true);
    }
}

// P42 S1: a real dispatched node keeps its execution when env config becomes
// unreadable or the history lacks a Task source. Default discovery alone cannot
// stand in for the node role: the public result must name the missing resource.
#[tokio::test]
async fn p42_default_environment_names_unassembled_node_role_and_keeps_run() {
    for (agent, missing_source) in [("omp", false), ("pi", true)] {
        let s = Scenario::new().await;
        if agent == "pi" {
            let tasks = get_over_tcp(s.server.addr(), "/api/tasks").await;
            let tasks: Value = serde_json::from_str(&tasks.body).unwrap();
            let mut task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "node").unwrap().clone();
            let id = task.as_object_mut().unwrap().remove("id").unwrap();
            task["agent"] = json!(agent);
            let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{}", id.as_str().unwrap()), Some(&task.to_string())).await;
            assert_eq!(response.status, 200, "{}", response.body);
        }
        s.start("role-manager").await;
        identity(&s, agent, "role-node-default");
        let dispatched = s.dispatch("role-manager", &["A"]).await;
        assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running");
        let runs = || async {
            let response = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
            assert_eq!(response.status, 200, "{}", response.body);
            serde_json::from_str::<Value>(&response.body).unwrap()
        };
        let before = runs().await;
        let execution = &before["runs"][0]["run"]["nodes"]["A"]["execution"];
        assert_eq!(execution["chat"]["sessionId"], "role-node-default");
        assert_eq!(before["runs"].as_array().unwrap().len(), 1);
        let history_file = &s.config.sessions.history_file;
        if missing_source {
            let mut history: Vec<Value> = serde_json::from_slice(&fs::read(history_file).unwrap()).unwrap();
            let node = history.iter_mut().find(|entry| entry["sessionId"] == "role-node-default").unwrap();
            assert_eq!(node.as_object_mut().unwrap().remove("sourceTaskId").unwrap(), execution["taskId"]);
            fs::write(history_file, serde_json::to_vec(&history).unwrap()).unwrap();
        } else {
            fs::write(&s.config.tasks.storage_file, "unreadable task environment").unwrap();
        }
        let history = fs::read(history_file).unwrap();
        let offset = s.herdr.calls().len();

        let response = post_json_over_tcp(s.server.addr(), "/api/chats/resume", r#"{"sessionId":"role-node-default"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let result: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(result["outcome"], "restored", "{result}");
        assert_eq!(runs().await, before);
        assert_eq!(fs::read(history_file).unwrap(), history);
        let calls = s.herdr.calls();
        let calls = &calls[offset..];
        let starts: Vec<_> = calls.iter().filter(|call| call["kind"] == "agent_start").collect();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0]["args"], json!([if agent == "omp" {"--resume"} else {"--session"}, "role-node-default"]));
        assert!(!calls.iter().any(|call| matches!(call["kind"].as_str(), Some("agent_prompt" | "agent_input"))));
        assert!(result["degradations"].as_array().unwrap().iter().any(|item| {
            item["element"] == "extension" && item["reason"].as_str().unwrap().contains(&format!("{agent}-node-chat"))
        }), "{result}");
        assert!(result["message"].as_str().unwrap().contains("未显式装配"), "{result}");
    }
}

fn identity(s: &Scenario, host: &str, session: &str) {
    s.herdr.set_behavior(json!({"native_session":{"source":format!("herdr:{host}"),"agent":host,"kind":"id","value":session},"native_cwd":s.data.path().join("node-zone")}));
}

async fn action(s: &Scenario, name: &str, extra: Value) -> Value {
    let mut body = extra;
    body["sessionId"] = json!("role-manager");
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn assert_role(s: &Scenario, offset: usize, host: &str, node: bool, resume: bool) {
    let calls = s.herdr.calls();
    let calls = &calls[offset..];
    let starts: Vec<_> = calls.iter().filter(|call| call["kind"] == "agent_start").collect();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["agent_kind"], host);
    let args = starts[0]["args"].as_array().unwrap();
    assert!(args.contains(&json!("--no-extensions")));
    assert_eq!(args.contains(&json!(s.data.path().join(format!("{host}-node-chat.ts")))), node);
    assert!(!args.contains(&json!(s.data.path().join(format!("{host}-manager-chat.ts")))));
    if resume {
        assert!(args.contains(&json!(if host == "omp" {"--resume"} else {"--session"})));
        assert!(!args.contains(&json!("--model")));
        assert!(!args.contains(&json!("--thinking")));
        assert!(!calls.iter().any(|call| call["kind"] == "agent_prompt" || call["kind"] == "agent_input"));
    }
}

#[tokio::test]
async fn same_normal_task_uses_standalone_node_and_persisted_resume_roles_for_both_hosts() {
    for host in ["omp", "pi"] {
        let s = Scenario::new().await;
        s.herdr.set_workspaces(json!([
            {"workspace_id":"w-manager","label":s.data.path().file_name().unwrap().to_str().unwrap()},
            {"workspace_id":"w-default","label":"bot-playground"}
        ]));
        let tasks = get_over_tcp(s.server.addr(), "/api/tasks").await;
        let tasks: Value = serde_json::from_str(&tasks.body).unwrap();
        let task = tasks["tasks"].as_array().unwrap().iter().find(|task| task["name"] == "node").unwrap();
        let id = task["id"].as_str().unwrap();
        let mut task = task.clone();
        task.as_object_mut().unwrap().remove("id");
        task["agent"] = json!(host);
        task["prompt"] = json!("NODE-FIXED: read ${implementation}");
        let implementation = s.data.path().join("implementation.md");
        fs::write(&implementation, "IMPLEMENTATION-BODY-9247").unwrap();
        fs::write(&s.config.prompts_file, format!("implementation = '{}'\n", implementation.display())).unwrap();
        let response = common::request_json_over_tcp(s.server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&task.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let zone = "node";
        identity(&s, host, "role-standalone");
        let offset = s.herdr.calls().len();
        let preview = post_json_over_tcp(s.server.addr(), "/api/chats/preview", &json!({"zone":zone,"taskId":id}).to_string()).await;
        assert_eq!(preview.status, 200, "{}", preview.body);
        let preview: Value = serde_json::from_str(&preview.body).unwrap();
        let response = post_json_over_tcp(s.server.addr(), "/api/chats", &json!({"zone":zone,"taskId":id,"chatId":preview["chatId"],"input":"STANDALONE ONLY"}).to_string()).await;
        assert_eq!(response.status, 202, "{}", response.body);
        let creation: Value = serde_json::from_str(&response.body).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let response = get_over_tcp(s.server.addr(), &format!("/api/chats/{}", creation["id"].as_str().unwrap())).await;
                let status: Value = serde_json::from_str(&response.body).unwrap();
                assert_ne!(status["stage"], "failed", "{status}");
                if status["stage"] == "succeeded" { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        assert_role(&s, offset, host, false, false);
        let submissions = || common::read_json_lines(&s.herdr.path().join("submitted.jsonl"));
        let standalone = submissions();
        assert_eq!(common::prompt_body(&standalone[0]), format!("/skill:teach NODE-FIXED: read {}\n\nSTANDALONE ONLY", implementation.display()));
        assert!(!standalone[0]["text"].as_str().unwrap().contains("IMPLEMENTATION-BODY-9247"));
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = get_over_tcp(s.server.addr(), "/api/sessions/history").await;
                let history: Value = serde_json::from_str(&response.body).unwrap();
                if history["sessions"].as_array().unwrap().iter().any(|row| row["sessionId"] == "role-standalone") { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();

        let started = s.start("role-manager").await;
        identity(&s, host, "role-node-old");
        let offset = s.herdr.calls().len();
        let dispatched = s.dispatch("role-manager", &["A"]).await;
        let original = dispatched["snapshot"]["nodes"]["A"]["execution"].clone();
        assert_eq!(original["taskId"], id);
        assert_eq!(original["agent"], host);
        assert_eq!(dispatched["snapshot"]["nodes"]["A"]["status"], "running");
        assert_role(&s, offset, host, true, false);
        let messages = submissions();
        let node = messages.last().unwrap()["text"].as_str().unwrap();
        assert!(node.starts_with(&format!("/skill:teach NODE-FIXED: read {}\n\n你负责本次派发的工作", implementation.display())));
        assert!(!node.contains("IMPLEMENTATION-BODY-9247"));
        assert!(node.contains("[Business input]\nNODE-INPUT\n第二行"));
        assert!(node.contains("[Run background / runInput]\nMANAGER-USER-SECRET"));
        assert!(!node.contains("STANDALONE ONLY") && !node.contains("BOT-PLAN-SECRET"));

        // A new application has no active-run memory: Resume must use persisted execution facts.
        let restored = TestServer::start(&s.config).await;
        let offset = s.herdr.calls().len();
        let response = post_json_over_tcp(restored.addr(), "/api/chats/resume", r#"{"sessionId":"role-node-old"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["outcome"], "restored");
        assert_role(&s, offset, host, true, true);

        action(&s, "fail", json!({"nodeId":"A","reason":"Original execution stopped"})).await;
        identity(&s, host, "role-node-new");
        let retried = action(&s, "retry", json!({"nodeId":"A","confirmed":true,"reason":"Explicit new execution"})).await;
        assert_eq!(retried["snapshot"]["nodes"]["A"]["previousExecutions"][0]["execution"], original);
        let run_file = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap()).join("run.json");
        let before = fs::read(&run_file).unwrap();
        identity(&s, host, "role-node-old");
        let offset = s.herdr.calls().len();
        let response = post_json_over_tcp(restored.addr(), "/api/chats/resume", r#"{"sessionId":"role-node-old"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["outcome"], "restored");
        assert_role(&s, offset, host, true, true);
        assert_eq!(fs::read(&run_file).unwrap(), before, "Resume must not replace or replay an execution");

        identity(&s, host, "role-standalone");
        let offset = s.herdr.calls().len();
        let response = post_json_over_tcp(restored.addr(), "/api/chats/resume", r#"{"sessionId":"role-standalone"}"#).await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert_role(&s, offset, host, false, true);
    }
}
