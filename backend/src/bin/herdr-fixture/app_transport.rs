//! Scripted public App/core CLI replies for Resume. Separate copies of the
//! executable represent the App layout, its core, and an unselected TUI.

use serde_json::{Value, json};

pub(super) fn intercept(args: &[String]) -> bool {
    let behavior = super::behavior();
    let Some(replies) = behavior.get("app_transport") else { return false; };
    let words: Vec<_> = args.iter().map(String::as_str).collect();
    let (command, data) = match words.as_slice() {
        ["ls"] => ("ls", Value::Null),
        ["status"] => ("status", Value::Null),
        ["do", command, data] => (*command, serde_json::from_str(data).expect("App JSON argument")),
        ["get", terminal, "--json"] => ("get", json!(terminal)),
        ["server", "status", "--json"] => ("server status", Value::Null),
        _ => super::usage(args),
    };
    super::record("calls.jsonl", json!({"kind": command, "argv": args, "data": data}));
    let reply = replies.get(command).unwrap_or_else(|| super::usage(args));
    if let Some(message) = reply.get("fixture_error").and_then(Value::as_str) {
        super::fail("controlled-failure", message);
    }
    let mut reply = reply.clone();
    let created_path = super::data_dir().join("app-created.json");
    if command == "tab.new" && reply.get("terminal").is_some() {
        let pane = reply.as_object_mut().and_then(|record| record.remove("fixture_pane"))
            .expect("fixture pane for a created terminal");
        let created = json!({"space": reply["space"], "tab": {"id": reply["tab"],
            "terminals":[{"pane":pane, "terminal":reply["terminal"]}]}});
        std::fs::write(created_path, created.to_string()).unwrap();
    } else if command == "ls" && let Ok(created) = std::fs::read_to_string(created_path) {
        let created: Value = serde_json::from_str(&created).unwrap();
        if let Some(space) = reply["spaces"].as_array_mut().and_then(|spaces| spaces.iter_mut().find(|s| s["id"] == created["space"])) {
            space["tabs"].as_array_mut().unwrap().push(created["tab"].clone());
        }
    }
    super::output(reply);
    true
}
