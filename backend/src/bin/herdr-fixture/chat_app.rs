//! Public App/core CLI stand-in for the launch matrix. The test owns both
//! surfaces; only the actual Rust backend interprets their output.
use super::{behavior, data_dir, fail, flag, output, read_records, record, usage, wait_at_gate};
use serde_json::{Value, json};

pub(super) fn dispatch(args: &[String]) {
    const CURRENT: &str = "space-current";
    const DEFAULT: &str = "space-default";
    let tabs = || read_records("app-tabs.jsonl");
    let removed = || read_records("app-removed-tabs.jsonl");
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["ls"] => {
            let tabs = tabs();
            let removed = removed();
            let spaces = [(CURRENT, "alpha"), (DEFAULT, "bot-playground")].into_iter().filter_map(|(id, name)| {
                let selected = tabs.iter().filter(|t| t["space"] == id && !removed.iter().any(|closed| closed["tab"] == t["tab"]))
                    .map(|t| json!({"id":t["tab"],"terminals":[{"pane":t["pane"],"terminal":t["terminal"]}]}))
                    .collect::<Vec<_>>();
                (!selected.is_empty() || !removed.iter().any(|row| row["space"] == id && row["spaceRemoved"] == true))
                    .then(|| json!({"id":id,"name":name,"tabs":selected}))
            }).collect::<Vec<_>>();
            output(json!({"spaces":spaces}));
        }
        ["do", "tab.close", raw] => {
            let request: Value = serde_json::from_str(raw).unwrap();
            let removed = removed();
            let mut tabs = tabs();
            tabs.retain(|tab| !removed.iter().any(|row| row["tab"] == tab["tab"]));
            let Some(tab) = tabs.iter().find(|t| t["tab"] == request["id"]) else { fail("missing_tab", "no such App Tab") };
            record("calls.jsonl", json!({"kind":"app_tab_close","request":request,"terminal":tab["terminal"]}));
            if flag(&behavior(), "fail_app_tab_close") {
                output(json!({"error":"controlled App close refused"}));
                return;
            }
            let last = tabs.iter().filter(|t| t["space"] == tab["space"]).count() == 1;
            if last && request["confirmed"] != true {
                fail("confirmation_required", "space.close ends its terminals: pass confirmed: true");
            }
            record("app-removed-tabs.jsonl", json!({"tab":tab["tab"],"space":tab["space"],"spaceRemoved":last}));
            record("app-close-receipts.jsonl", json!({"tab":tab["tab"],"terminal":tab["terminal"]}));
            output(json!({"ok":true}));
        }
        ["server", "status", "--json"] => {
            let id = if flag(&behavior(), "change_app_core") { "other-core" } else { "fixture-core" };
            output(json!({"core":{"id":id}}));
        }
        ["do", "pane.focus", raw] => {
            let request: Value = serde_json::from_str(raw).unwrap();
            let tab = tabs().into_iter().find(|t| t["pane"] == request["pane"]).unwrap();
            std::fs::write(data_dir().join("app-focused.json"), tab.to_string()).unwrap();
            output(json!({"ok":true}));
        }
        ["status"] => {
            let tab: Value = serde_json::from_slice(&std::fs::read(data_dir().join("app-focused.json")).unwrap()).unwrap();
            let pane = if super::flag(&super::behavior(), "unconfirmed_focus") { json!("wrong-pane") } else { tab["pane"].clone() };
            output(json!({"space":tab["space"],"tab":tab["tab"],"focused_pane":pane}));
        }
        ["do", "tab.new", raw] => {
            let launch: Value = serde_json::from_str(raw).unwrap();
            let n = tabs().len() + 1;
            let tab = json!({"space":launch["space"],"tab":format!("tab-{n}"),"pane":format!("pane-{n}"),
                "terminal":format!("t{n}"),"options":launch});
            record("app-tabs.jsonl", tab.clone());
            output(tab);
        }
        ["get", terminal, "--json"] => {
            if *terminal == "t-unrelated" && data_dir().join("app-unrelated-terminal").is_file() {
                output(json!({"address":terminal,"state":"live","pid":43128}));
                return;
            }
            let tab = tabs().into_iter().find(|t| t["terminal"] == *terminal).unwrap();
            let exiting = data_dir().join("app-release-terminal-exit").is_file()
                && read_records("app-close-receipts.jsonl").iter().any(|receipt| receipt["terminal"] == *terminal);
            if exiting {
                output(json!({"address":terminal,"state":"dead","reason":"killed"}));
            } else {
                output(json!({"address":terminal,"labels":{"herdr.space":tab["space"],"herdr.tab":tab["tab"]},
                    "argv":tab["options"]["argv"],"cwd":tab["options"]["folder"],"state":"live","pid":behavior()["launch_pid"].as_u64().unwrap_or(43127)}));
            }
        }
        ["send", "--json", terminal, "--", text] => {
            assert!(tabs().iter().any(|t| t["terminal"] == *terminal));
            let mut call = json!({"kind":"core_send","terminal":terminal,"text":text});
            if let Ok(raw) = std::fs::read(data_dir().join("observe-app-send.json")) {
                let files: Vec<std::path::PathBuf> = serde_json::from_slice(&raw).unwrap();
                call["observedFiles"] = json!(files.iter().map(|file| {
                    std::fs::read_to_string(file).ok()
                }).collect::<Vec<_>>());
            }
            record("calls.jsonl", call);
            let behavior = behavior();
            wait_at_gate(&behavior, "prompt_gate", text);
            if flag(&behavior, "block_prompt") {
                eprintln!("{}", json!({"error":{"kind":"blocked","message":"controlled core modal"}}));
                std::process::exit(9);
            }
            if flag(&behavior, "fail_app_send") {
                fail("blocked", "controlled App refused the input before sending");
            }
            if flag(&behavior, "fail_prompt") {
                fail("core_send_rejected", "controlled core rejected the input");
            }
            record("submitted.jsonl", json!({"terminal":terminal,"text":text}));
            output(json!({"turn":0,"started":behavior["started"].as_bool().unwrap_or(true),"status":"idle"}));
        }
        _ => usage(args),
    }
}

pub(super) fn selected() -> bool { data_dir().join("chat-app-mode").is_file() }
