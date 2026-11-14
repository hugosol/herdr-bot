//! S2 supporting P7 assembly, not S3/native/model acceptance.
//! Independent oracle: shipped chief fixed entries apply only to chief purpose,
//! and both public launch/resume must actually send those entries to the external
//! CLI without a Suite. Omitting an entry or treating it as generic fixed makes
//! the literal public roles/argv checks fail. No private resolver is called.
#[path = "../common/mod.rs"]
mod common;
use common::{TempDir, TestServer, HerdrFixture, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};

fn body(reply: common::HttpResponse, status: u16) -> Value {
    assert_eq!(reply.status, status, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}
async fn post(server: &TestServer, path: &str, value: Value, status: u16) -> Value {
    body(post_json_over_tcp(server.addr(), path, &value.to_string()).await, status)
}
async fn get(server: &TestServer, path: &str) -> Value {
    body(get_over_tcp(server.addr(), path).await, 200)
}

#[tokio::test]
async fn shipped_chief_fixed_entries_are_chief_only_in_public_catalog_launch_and_resume_without_suite() {
    let root = TempDir::new("chief entry assembly");
    let herdr = HerdrFixture::new("chief entry external CLI");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.sessions.history_file = root.path().join("history.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.herdr.executable = herdr.executable().to_owned();
    let server = TestServer::start(&config).await;
    let fixed = get(&server, "/api/suites").await["fixedExtensions"].as_array().unwrap().clone();
    for host in ["omp", "pi"] {
        let id = format!("{host}-chief-chat");
        let declaration = fixed.iter().find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("shipped metadata must declare {id}: {fixed:?}"));
        assert_eq!(declaration["roles"], json!(["chief"]), "chief is not a generic fixed role");
        assert_eq!(declaration["agent"], host);
        let extension = declaration["entrypoint"].as_str().unwrap();
        let zone = format!("entry/{host}");
        post(&server, "/api/zones", json!({"id":zone,"path":root.path()}), 200).await;
        for kind in ["normal", "manager", "chief"] {
            let session = format!("entry-{host}-{kind}");
            let native = root.path().join(format!("{session}.jsonl"));
            fs::write(&native, format!("{}\n", json!({"type":"session","version":3,"id":session,"cwd":root.path()}))).unwrap();
            herdr.set_behavior(json!({"agents":[],"native_session":{"source":format!("herdr:{host}"),"agent":host,"kind":"path","value":native}}));
            let name = format!("{host}-{kind}");
            let saved = post(&server, "/api/tasks", json!({"name":name,"chatName":"{chat.id}","taskType":kind,"agent":host,"model":"default","thinking":"default","skill":null,"suite":null,"prompt":""}), 200).await;
            let task = saved["tasks"].as_array().unwrap().iter().find(|task| task["name"] == name).unwrap();
            assert!(task["suite"].is_null());
            let preview = post(&server, "/api/chats/preview", json!({"taskId":task["id"],"zone":zone}), 200).await;
            let offset = herdr.calls().len();
            let launch = post(&server, "/api/chats", json!({"taskId":task["id"],"zone":zone,"chatId":preview["chatId"],"input":"read-only assembly check"}), 202).await;
            tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    let state = get(&server, &format!("/api/chats/{}", launch["id"].as_str().unwrap())).await;
                    assert_ne!(state["stage"], "failed", "{state}");
                    let history = get(&server, "/api/sessions/history").await;
                    if state["stage"] == "succeeded" && history["sessions"].as_array().unwrap().iter().any(|row| row["sessionId"] == session) { break; }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }).await.unwrap();
            let calls = herdr.calls();
            let args = calls[offset..].iter().find(|call| call["kind"] == "agent_start").unwrap()["args"].as_array().unwrap();
            assert_eq!(args.contains(&json!(extension)), kind == "chief", "launch {host} {kind}: {args:?}");
            if kind == "chief" {
                post(&server, "/api/sessions/events", json!({"sessionId":session,"kind":"register","data":{"cwd":root.path(),"agentType":host,"sessionFile":native}}), 200).await;
                post(&server, "/api/sessions/events", json!({"sessionId":session,"kind":"unregister"}), 200).await;
            }
            let offset = herdr.calls().len();
            let resumed = post(&server, "/api/chats/resume", json!({"sessionId":session}), 200).await;
            assert_eq!(resumed["outcome"], "restored", "{resumed}");
            let calls = herdr.calls();
            let args = calls[offset..].iter().find(|call| call["kind"] == "agent_start").unwrap()["args"].as_array().unwrap();
            assert_eq!(args.contains(&json!(extension)), kind == "chief", "resume {host} {kind}: {args:?}");
            assert!(args.contains(&json!(if host == "omp" { "--resume" } else { "--session" })));
            if kind == "chief" {
                post(&server, "/api/sessions/events", json!({"sessionId":session,"kind":"register","data":{"cwd":root.path(),"agentType":host,"sessionFile":native}}), 200).await;
                post(&server, "/api/sessions/events", json!({"sessionId":session,"kind":"unregister"}), 200).await;
            }
        }
    }
}
