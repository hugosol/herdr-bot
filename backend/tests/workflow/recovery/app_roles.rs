//! P42 S1: public Resume, workflow/history HTTP, and the selected App/core argv.
use super::*;
use std::{path::Path, time::Duration};

// The same normal Task really launches a worker, an end Chat, and an
// independent Chat. A Task-type-based recovery would give them the same role;
// the nested manager would also lose its parent node duties.
#[tokio::test]
async fn p42_app_resume_preserves_original_run_roles_and_ownership() {
    for agent in ["omp", "pi"] {
        let (data, herdr, config, server, zone, ids) = fixture_with_host(agent, true).await;
        let cwd = data.path().join("alpha");
        let addr = server.addr();
        fs::write(config.scheduler.bot_directory.join("nested.json"), json!({
            "id":"nested","name":"Nested","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"
        }).to_string()).unwrap();
        fs::write(config.scheduler.map_directory.join("work.json"), json!({"title":"Root","description":"","nodes":{
            "A":{"title":"Work","bot_id":"worker","input":"Ordinary node business input","zone":zone,"blocked_by":[]},
            "N":{"title":"Nested","bot_id":"nested","manager_task_id":ids[0],"input":"Nested manager business input","zone":zone,"blocked_by":[]}
        }}).to_string()).unwrap();
        fs::write(config.scheduler.map_directory.join("inner.json"), json!({"title":"Inner","description":"","nodes":{
            "E":{"title":"Handoff","bot_id":"worker","input":"User-only end Chat input","zone":zone,"blocked_by":[],"end_node":true}
        }}).to_string()).unwrap();

        let start = start_fixture_run(&server, ids[0].clone(), zone.clone());
        let manager = app_launch(&server, &herdr, 1, agent, "original-manager", &cwd).await;
        let run_id = run_reply(start.await.unwrap(), 201)["id"].as_str().unwrap().to_owned();
        prepare_run(&server, "original-manager").await;
        let dispatch = dispatch_app_node(&server, "original-manager", "A");
        let worker = app_launch(&server, &herdr, 2, agent, "original-node", &cwd).await;
        assert_eq!(run_reply(dispatch.await.unwrap(), 200)["results"][0]["status"], "running");
        let dispatch = dispatch_app_node(&server, "original-manager", "N");
        let nested = app_launch(&server, &herdr, 3, agent, "original-nested", &cwd).await;
        assert_eq!(run_reply(dispatch.await.unwrap(), 200)["results"][0]["status"], "running");
        prepare_run(&server, "original-nested").await;
        let dispatch = dispatch_app_node(&server, "original-nested", "E");
        let end = app_launch(&server, &herdr, 4, agent, "original-end", &cwd).await;
        assert_eq!(run_reply(dispatch.await.unwrap(), 200)["results"][0]["status"], "running");
        let finished = run_reply(post_json_over_tcp(addr, "/api/workflow/manager/finish",
            r#"{"sessionId":"original-nested"}"#).await, 200);
        assert_eq!(finished["snapshot"]["finished"]["reason"], "handed-to-user");

        let preview = run_reply(post_json_over_tcp(addr, "/api/chats/preview",
            &json!({"zone":zone,"taskId":ids[1]}).to_string()).await, 200);
        let started = run_reply(post_json_over_tcp(addr, "/api/chats",
            &json!({"zone":zone,"taskId":ids[1],"chatId":preview["chatId"],"input":"Unrelated independent input"}).to_string()).await, 202);
        let standalone = app_launch(&server, &herdr, 5, agent, "original-standalone", &cwd).await;
        let status_path = format!("/api/chats/{}", started["id"].as_str().unwrap());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = run_reply(get_over_tcp(addr, &status_path).await, 200);
                if status["stage"] == "succeeded" { break; }
                assert_ne!(status["stage"], "failed", "{status}");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }).await.unwrap();

        let list = || async { run_reply(get_over_tcp(addr, "/api/workflow/runs").await, 200) };
        let before = list().await;
        let runs = before["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 2);
        let root = &runs.iter().find(|row| row["id"] == run_id).unwrap()["run"];
        assert_eq!(root["manager"]["sessionId"], "original-manager");
        let execution = &root["nodes"]["A"]["execution"];
        assert_eq!(execution["chat"]["sessionId"], "original-node");
        assert_eq!(execution["taskId"], ids[1]);
        let nested_execution = &root["nodes"]["N"]["execution"];
        let child = &runs.iter().find(|row| row["id"] == nested_execution["childRunId"]).unwrap()["run"];
        assert_eq!(child["parent"], json!({"runId":run_id,"nodeId":"N","executionId":nested_execution["id"]}));
        assert_eq!(child["manager"]["sessionId"], "original-nested");
        assert_eq!(child["nodes"]["E"]["execution"]["chat"]["sessionId"], "original-end");
        assert_eq!(child["nodes"]["E"]["execution"]["taskId"], ids[1]);
        assert_eq!(child["nodes"]["E"]["execution"]["endNode"], true);
        let submissions = || common::read_json_lines(&herdr.path().join("submitted.jsonl"));
        let submitted = submissions();
        assert_eq!(submitted.len(), 5);

        let mut resumed_nested = None;
        for (index, (session, origin, expected)) in [
            ("original-manager", manager, &["manager-chat"][..]),
            ("original-node", worker, &["node-chat"][..]),
            ("original-nested", nested, &["manager-chat", "node-chat"][..]),
            ("original-end", end, &[][..]),
            ("original-standalone", standalone, &[][..]),
        ].into_iter().enumerate() {
            let native = cwd.join(format!("{session}.jsonl"));
            let bytes = fs::read(&native).unwrap();
            app_role_event(&server, &origin, session, &native, "launch_ended").await;
            let tab = restore_app_role(&server, &herdr, index + 6, session, &native, expected).await;
            assert_eq!(tab["space"], origin["space"], "{agent}/{session}");
            assert_ne!(tab["pane"], origin["pane"]);
            assert_eq!(fs::read(&native).unwrap(), bytes);
            if session == "original-nested" { resumed_nested = Some(tab); }
        }

        // Invalid catalog must not fall back to a manager-only guess or claim
        // either of the original dual-role resources was explicitly loaded.
        let native = cwd.join("original-nested.jsonl");
        app_role_event(&server, &resumed_nested.unwrap(), "original-nested", &native, "launch_ended").await;
        fs::write(&config.tasks.metadata_file, "this is not a valid catalog").unwrap();
        let result = run_reply(post_json_over_tcp(addr, "/api/chats/resume",
            r#"{"sessionId":"original-nested"}"#).await, 200);
        assert_eq!(result["outcome"], "unconfirmed", "{result}");
        assert_eq!(result["progress"]["conversationConfirmed"], false);
        let tab = app_tab(&herdr, 11).await;
        assert_eq!(tab["options"]["argv"], json!([agent, if agent == "omp" {"--resume"} else {"--session"}, native]));
        assert_eq!(result["target"]["paneId"], tab["pane"]);
        let warnings = result["degradations"].as_array().unwrap();
        for role in ["manager-chat", "node-chat"] {
            assert!(warnings.iter().any(|item| {
                let reason = item["reason"].as_str().unwrap();
                item["element"] == "extension" && reason.contains(&format!("{agent}-{role}")) && reason.contains("未显式装配")
            }), "{result}");
        }

        let context = |session: &'static str| async move {
            post_json_over_tcp(addr, "/api/workflow/node/context", &json!({"sessionId":session}).to_string()).await
        };
        let worker_context = run_reply(context("original-node").await, 200);
        assert_eq!(worker_context["runId"], run_id);
        assert_eq!(worker_context["executionId"], execution["id"]);
        let nested_context = run_reply(context("original-nested").await, 200);
        assert_eq!(nested_context["runId"], run_id);
        assert_eq!(nested_context["executionId"], nested_execution["id"]);
        for (session, expected) in [("original-manager", &root["id"]), ("original-nested", &child["id"])] {
            let status = run_reply(post_json_over_tcp(addr, "/api/workflow/manager/status",
                &json!({"sessionId":session}).to_string()).await, 200);
            assert_eq!(&status["snapshot"]["id"], expected);
        }
        for session in ["original-end", "original-standalone"] {
            assert_eq!(context(session).await.status, 403, "{agent}/{session}");
        }
        assert_eq!(list().await, before, "{agent}: Resume cannot create or rewrite a Run or execution");
        assert_eq!(submissions(), submitted, "{agent}: input cannot be replayed");
    }
}

pub(super) async fn restore_app_role(server: &TestServer, herdr: &HerdrFixture, n: usize, session: &str, file: &Path, expected: &[&str]) -> Value {
    let addr = server.addr();
    let request = json!({"sessionId":session}).to_string();
    let pending = tokio::spawn(async move { post_json_over_tcp(addr, "/api/chats/resume", &request).await });
    let tab = app_tab(herdr, n).await;
    let argv = tab["options"]["argv"].as_array().unwrap();
    let agent = argv[6].as_str().unwrap();
    assert_eq!(argv[argv.len() - 2..], [json!(if agent == "omp" {"--resume"} else {"--session"}), json!(file)]);
    assert!(argv.contains(&json!("--no-extensions")));
    assert!(!argv.contains(&json!("--model")) && !argv.contains(&json!("--thinking")));
    let roles = argv.windows(2).filter(|pair| pair[0] == "--extension")
        .filter_map(|pair| Path::new(pair[1].as_str()?).file_name()?.to_str())
        .filter(|name| ["manager-chat.ts", "node-chat.ts", "chief-report.ts"].iter().any(|suffix| name.ends_with(suffix)))
        .collect::<Vec<_>>();
    assert_eq!(roles, expected.iter().map(|role| format!("{agent}-{role}.ts")).collect::<Vec<_>>(), "{session}");
    app_role_event(server, &tab, session, file, "launch_identity").await;
    let result = run_reply(pending.await.unwrap(), 200);
    assert_eq!(result["outcome"], "restored", "{result}");
    assert_eq!(result["target"], json!({"workspaceId":tab["space"],"tabId":tab["tab"],"paneId":tab["pane"]}));
    tab
}

pub(super) async fn app_role_event(server: &TestServer, tab: &Value, session: &str, file: &Path, kind: &str) {
    let argv = &tab["options"]["argv"];
    run_reply(post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({
        "kind":kind,"sessionId":session,"data":{
            "launchId":argv[4],"targetId":argv[5],"agentType":argv[6],"cwd":tab["options"]["folder"],"pid":43127,"sessionFile":file,
        }
    }).to_string()).await, 200);
}
