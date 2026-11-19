//! Public App/core CLI stand-in for the launch matrix. The test owns both
//! surfaces; only the actual Rust backend interprets their output.
use super::{data_dir, output, record, read_records, usage};
use serde_json::{Value, json};

pub(super) fn dispatch(args: &[String]) {
    const CURRENT: &str = "space-current";
    const DEFAULT: &str = "space-default";
    let tabs = || read_records("app-tabs.jsonl");
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["ls"] => {
            let tabs = tabs();
            let space = |id: &str, name: &str| {
                let selected = tabs.iter().filter(|t| t["space"] == id)
                    .map(|t| json!({"id":t["tab"],"terminals":[{"pane":t["pane"],"terminal":t["terminal"]}]}))
                    .collect::<Vec<_>>();
                json!({"id":id,"name":name,"tabs":selected})
            };
            output(json!({"spaces":[space(CURRENT,"alpha"),space(DEFAULT,"bot-playground")]}));
        }
        ["server", "status", "--json"] => output(json!({"core":{"id":"fixture-core"}})),
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
            let tab = tabs().into_iter().find(|t| t["terminal"] == *terminal).unwrap();
            output(json!({"address":terminal,"labels":{"herdr.space":tab["space"],"herdr.tab":tab["tab"]},
                "argv":tab["options"]["argv"],"cwd":tab["options"]["folder"],"state":"live","pid":43127}));
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
            if super::flag(&super::behavior(), "fail_app_send") {
                super::fail("blocked", "controlled App refused the input before sending");
            }
            output(json!({"turn":0,"started":true,"status":"idle"}));
        }
        _ => usage(args),
    }
}

pub(super) fn selected() -> bool { data_dir().join("chat-app-mode").is_file() }
