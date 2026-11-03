//! Suite S2 contracts. Expected values are independent declarations, never
//! computed through the product catalog. S1/S3 restart evidence lives in suites.spec.ts.
mod common;

use common::{TempDir, TestServer, get_over_tcp, request_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

fn config(root: &TempDir) -> herdr_bot::Config {
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.suites.definition_file = root.path().join("definitions with spaces").join("suites.json");
    config.suites.mcp_file = root.path().join("central-mcp.json");
    config
}

// P25: mode is a closed explicit choice, not a nullable/inherited global value.
// Saving invalid modes through HTTP must not replace the last valid authority.
#[tokio::test]
async fn pi_mode_is_explicit_and_applicable_through_public_http() {
    let root = TempDir::new("suite explicit pi mode");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let literal = json!({"id":"pi-tools","name":"Pi tools","agents":["pi"],"skills":[],"extensions":[],"prompts":[],"lsp":{},"mcp":[],"pi":{"codemode":"only"}});
    let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&literal.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let persisted = fs::read(&config.suites.definition_file).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&persisted).unwrap(), json!({"suites":[literal]}));
    let invalid = [
        json!(null), json!({}), json!({"codemode":"off"}),
        json!({"codemode":"inherit"}), json!({"codemode":false}),
        json!({"codemode":"on","only":false}),
    ];
    for pi in invalid {
        let mut input = literal.clone();
        input["pi"] = pi;
        let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/pi-tools", Some(&input.to_string())).await;
        assert_eq!(response.status, 400, "{}", response.body);
        assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    }
    let mut missing = literal.clone();
    missing.as_object_mut().unwrap().remove("pi");
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/pi-tools", Some(&missing.to_string())).await;
    assert_eq!(response.status, 400, "{}", response.body);
    let mut wrong_host = literal.clone();
    wrong_host["agents"] = json!(["omp"]);
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/pi-tools", Some(&wrong_host.to_string())).await;
    assert_eq!(response.status, 400, "{}", response.body);
    assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    let mut on = literal.clone();
    on["pi"] = json!({"codemode":"on"});
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/pi-tools", Some(&on.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let response = get_over_tcp(server.addr(), "/api/suites/pi-tools").await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"suite":on}));
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&config.suites.definition_file).unwrap()).unwrap(), json!({"suites":[on]}));
}

// P4: an external invalid authority is a visible error, not a reason to return
// cached definitions or overwrite the user's bytes from an independent copy.
#[tokio::test]
async fn external_authority_damage_is_not_hidden_or_overwritten() {
    let root = TempDir::new("suite external authority damage");
    let config = config(&root);
    fs::create_dir_all(config.suites.definition_file.parent().unwrap()).unwrap();
    let literal = "{\"suites\":[{\"id\":\"external\",\"name\":\"Independent definition\",\"agents\":[\"omp\"],\"skills\":[],\"extensions\":[],\"prompts\":[],\"lsp\":{},\"mcp\":[]}]}";
    fs::write(&config.suites.definition_file, literal).unwrap();
    let server = TestServer::start(&config).await;
    assert_eq!(get_over_tcp(server.addr(), "/api/suites/external").await.status, 200);
    let damaged = "{\"suites\":[BROKEN external edit";
    fs::write(&config.suites.definition_file, damaged).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(response.body.contains("suite-storage-failed"));
    let valid = json!({"id":"external","name":"Must not overwrite","agents":["omp"]});
    for (method, url, body) in [("PUT", "/api/suites/external", Some(valid.to_string())), ("DELETE", "/api/suites/external", None)] {
        let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), damaged);
    }
    fs::write(&config.suites.definition_file, literal).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites/external").await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["suite"]["name"], "Independent definition");
    assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), literal);
}
