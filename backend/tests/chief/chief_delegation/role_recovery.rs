//! P42 S1: the worker's saved delegation, not its normal Task type, owns
//! chief-report. Observe both normal assembly and native default discovery.
use super::*;
use std::path::Path;

#[tokio::test]
async fn p42_resume_keeps_chief_delegation_with_or_without_its_role_resource() {
    for app in [false, true] {
        for agent in ["omp", "pi"] {
            let f = Fixture::with_host(app, agent).await;
            let addr = f.server.addr();
            let preview = f.preview().await;
            let accepted = body(post_json_over_tcp(addr, "/api/chief/delegations", &json!({
                "sessionId":"chief-a","previewId":preview["previewId"]
            }).to_string()).await, 202);
            let id = accepted["id"].as_str().unwrap();
            if app {
                let tab = f.app_tab(id).await;
                f.app_event(&tab, "worker-a", "launch_identity").await;
                f.app_event(&tab, "worker-a", "launch_ready").await;
            }
            let creation = terminal(&f.server, id).await;
            assert_eq!(creation["stage"], "succeeded", "{creation}");
            let delegations = || async {
                body(get_over_tcp(addr, "/api/chief/delegations?sessionId=worker-a").await, 200)
            };
            let record = delegations().await;
            assert_eq!(record["delegations"].as_array().unwrap().len(), 1);
            assert_eq!(record["delegations"][0]["id"], id);
            assert_eq!(record["delegations"][0]["sessionId"], "worker-a");
            assert!(record["delegations"][0].get("runId").is_none());
            let delegation_file = f.config.sessions.history_file.with_extension("delegations.json");
            let saved_delegations = fs::read(&delegation_file).unwrap();
            assert_eq!(serde_json::from_slice::<Vec<Value>>(&saved_delegations).unwrap().len(), 1);
            let submissions = || common::read_json_lines(&f.herdr.path().join("submitted.jsonl"));
            let before = submissions();
            assert_eq!(before.len(), 2);
            assert_eq!(before[1]["text"], preview["facts"]["prompt"]);
            let app_log = f.herdr.path().join("app-tabs.jsonl");
            let role_id = format!("{agent}-chief-report");

            for degraded in [false, true] {
                if degraded {
                    fs::write(&f.config.tasks.storage_file, "unreadable task environment").unwrap();
                }
                let offset = f.herdr.calls().len();
                let (argv, result) = if app {
                    let tabs = common::read_json_lines(&app_log);
                    f.app_event(tabs.last().unwrap(), "worker-a", "launch_ended").await;
                    let resume = tokio::spawn(async move {
                        post_json_over_tcp(addr, "/api/chats/resume", r#"{"sessionId":"worker-a"}"#).await
                    });
                    let mut tab = None;
                    assert!(common::wait_until(Duration::from_secs(5), || {
                        tab = common::read_json_lines(&app_log).into_iter().nth(tabs.len());
                        tab.is_some()
                    }).await, "App did not create the resumed Chat");
                    let tab = tab.unwrap();
                    if !degraded {
                        f.app_event(&tab, "worker-a", "launch_identity").await;
                    }
                    let result = body(resume.await.unwrap(), 200);
                    assert_eq!(result["target"]["paneId"], tab["pane"]);
                    (tab["options"]["argv"].clone(), result)
                } else {
                    let result = post(&f.server, "/api/chats/resume", json!({"sessionId":"worker-a"})).await;
                    let calls = f.herdr.calls();
                    let starts: Vec<_> = calls[offset..].iter().filter(|call| call["kind"] == "agent_start").collect();
                    assert_eq!(starts.len(), 1);
                    assert_eq!(starts[0]["agent_kind"], agent);
                    (starts[0]["args"].clone(), result)
                };
                assert_eq!(result["outcome"], if app && degraded {"unconfirmed"} else {"restored"}, "{result}");
                let args = argv.as_array().unwrap();
                let flag = if agent == "omp" { "--resume" } else { "--session" };
                let load = if app { json!("worker-a") } else { json!(f.root.path().join("worker-a.jsonl")) };
                assert_eq!(args[args.len() - 2..], [json!(flag), load]);
                let roles: Vec<_> = args.windows(2).filter(|pair| pair[0] == "--extension")
                    .filter_map(|pair| Path::new(pair[1].as_str()?).file_name()?.to_str())
                    .filter(|name| ["chief-report.ts", "manager-chat.ts", "node-chat.ts"].iter().any(|suffix| name.ends_with(suffix)))
                    .collect();
                let role_warnings = result["degradations"].as_array().unwrap();
                if degraded {
                    assert!(roles.is_empty());
                    assert!(!args.contains(&json!("--no-extensions")));
                    assert!(role_warnings.iter().any(|item| {
                        let reason = item["reason"].as_str().unwrap();
                        item["element"] == "extension" && reason.contains(&role_id) && reason.contains("未显式装配")
                    }), "{result}");
                } else {
                    assert_eq!(roles, [format!("{role_id}.ts")]);
                    assert!(!role_warnings.iter().any(|item| item["reason"].as_str().unwrap().contains(&role_id)), "{result}");
                }
                assert_eq!(delegations().await, record);
                assert_eq!(fs::read(&delegation_file).unwrap(), saved_delegations);
                assert_eq!(body(get_over_tcp(addr, "/api/workflow/runs").await, 200)["runs"], json!([]));
                assert_eq!(submissions(), before, "Resume cannot replay approved delegated input");
            }
        }
    }
}
