//! P47/P48 control-seam evidence only: launch over public Chat HTTP, observe
//! the selected native CLI and post the independent connect-bot state stream.
//! The fixture neither reports native state nor plays audio; real native state
//! and sound policy belong to the installed-host acceptance scenarios.

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn tui_loads_its_pane_state_extension_once_and_does_not_forward_panel_state() {
    let host = HerdrFixture::new("tui reminder boundary");
    let storage = TempDir::new("tui reminder storage");
    let server = TestServer::start(&config_with(&host, &storage)).await;
    let cwd = storage.path().join("reminder-zone");
    fs::create_dir(&cwd).unwrap();
    let zone = register(&server, &cwd).await;

    for (agent, own_extension) in [
        ("omp", "omp-herdr-agent-state.ts"),
        ("pi", "pi-herdr-agent-state.ts"),
    ] {
        let task = save_task(
            &server,
            json!({
                "name": format!("Reminder {agent}"),
                "chatName": "{task.name}",
                "agent": agent,
                "model": "default",
                "thinking": "default",
                "prompt": "",
            }),
        ).await;
        let task_id = task["id"].as_str().unwrap();
        let id = accept_creation(&server, &zone, task_id, "Start one owned turn").await;
        let view = wait_stage(&server, &id, "succeeded").await;
        let tabs = host.tabs();
        let tab = tabs.last().unwrap();
        let starts = calls_of_kind(&host, "agent_start");
        let start = starts.last().unwrap();
        assert_eq!(view["paneId"], tab["pane_id"]);
        assert_eq!(start["pane"], tab["pane_id"]);
        assert_eq!(start["agent_kind"], agent);
        let args = start["args"].as_array().unwrap();
        let reporters: Vec<_> = args.windows(2)
            .filter(|pair| pair[0] == "--extension")
            .filter_map(|pair| Path::new(pair[1].as_str()?).file_name()?.to_str())
            .filter(|name| name.ends_with("-herdr-agent-state.ts"))
            .collect();
        assert_eq!(reporters, [own_extension]);
        let session = format!("fixture-session-{}", tab["pane_id"].as_str().unwrap());
        assert_panel_state_has_no_host_action(&server, &host, &session, &cwd).await;
    }
}

// A panel transition (including a duplicate) really reaches the session feed;
// it must not cause any external Herdr CLI action, even an unsupported one.
pub(super) async fn assert_panel_state_has_no_host_action(
    server: &TestServer,
    host: &HerdrFixture,
    session: &str,
    cwd: &Path,
) {
    let log = host.path().join("invocations.jsonl");
    let before = fs::read(&log).unwrap();
    let post = |kind: &str, data: Value| json!({"kind":kind,"sessionId":session,"data":data}).to_string();
    let endpoint = "/api/sessions/events";
    let register = post("register", json!({"cwd":cwd}));
    assert_eq!(post_json_over_tcp(server.addr(), endpoint, &register).await.status, 200);
    for value in ["working", "awaiting-input", "awaiting-input"] {
        let state = post("state", json!({"value":value}));
        assert_eq!(post_json_over_tcp(server.addr(), endpoint, &state).await.status, 200);
        let mut feed = SseConnection::open(server.addr(), endpoint, &[]).await;
        let snapshot = feed.next_frame_within(Duration::from_secs(2)).await.unwrap().json();
        let row = snapshot["sessions"].as_array().unwrap().iter()
            .find(|row| row["sessionId"] == session).unwrap();
        assert_eq!(row["state"], value);
    }
    assert_eq!(fs::read(log).unwrap(), before, "panel state caused a second native path");
}
