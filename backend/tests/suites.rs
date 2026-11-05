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

fn shared_metadata(root: &TempDir, config: &mut herdr_bot::Config) -> String {
    let directory = root.path().join("metadata elsewhere");
    fs::create_dir_all(&directory).unwrap();
    config.tasks.metadata_file = directory.join("options.toml");
    let roots_file = root.path().join("isolated-roots.toml");
    fs::write(&roots_file, "[roots]\npi_extensions = 'unavailable dependencies'\n").unwrap();
    config.roots = herdr_bot::paths::Roots::load(&roots_file).unwrap();
    let mut metadata: toml::Value = toml::from_str(&fs::read_to_string(common::test_metadata_file()).unwrap()).unwrap();
    let extensions = metadata["extensions"].as_array_mut().unwrap();
    extensions.retain(|extension| extension["kind"].as_str() == Some("fixed"));
    let connect = extensions.iter_mut().find(|extension| extension["id"].as_str() == Some("pi-connect-bot")).unwrap();
    connect["entrypoint"] = toml::Value::String("${roots.pi_extensions}/connect-bot.ts".into());
    let raw = toml::to_string(&metadata).unwrap();
    fs::write(&config.tasks.metadata_file, &raw).unwrap();
    raw
}

#[tokio::test]
async fn suite_paths_save_without_resources_and_fixed_catalog_preserves_roles() {
    let root = TempDir::new("suite shared extension declarations");
    let mut config = config(&root);
    shared_metadata(&root, &mut config);
    let server = TestServer::start(&config).await;
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let listing: Value = serde_json::from_str(&response.body).unwrap();
    assert!(listing.get("extensions").is_none());
    let fixed = listing["fixedExtensions"].as_array().unwrap();
    for host in ["omp", "pi"] {
        for (suffix, roles) in [("connect-bot", json!(["standalone","manager","node"])), ("manager-chat", json!(["manager"])), ("node-chat", json!(["node"]))] {
            let id = format!("{host}-{suffix}");
            let option = fixed.iter().find(|option| option["id"] == id).unwrap();
            assert_eq!(option["agent"], host);
            assert_eq!(option["kind"], "fixed");
            assert_eq!(option["roles"], roles);
        }
    }
    let connect = fixed.iter().find(|option| option["id"] == "pi-connect-bot").unwrap();
    assert_eq!(std::path::Path::new(connect["entrypoint"].as_str().unwrap()), root.path().join("unavailable dependencies/connect-bot.ts"));
    // No source dependency exists; listing and saving do not inspect resources.
    assert!(!root.path().join("unavailable dependencies").exists());
    let mut suite = json!({"id":"shared","name":"Shared choices","agents":["pi","omp"],"pi":{"codemode":"on"},
        "extensions":{"pi":["${roots.pi_extensions}/pi-web-access/index.ts","packages with spaces/entry.ts"],"omp":["custom-omp.ts"]}
    });
    for (method, url) in [("POST", "/api/suites"), ("PUT", "/api/suites/shared")] {
        let response = request_json_over_tcp(server.addr(), method, url, Some(&suite.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["suites"][0]["extensions"], suite["extensions"]);
        assert_eq!(body["fixedExtensions"], listing["fixedExtensions"]);
        suite["name"] = json!("Edited choices");
    }
    let stored: Value = serde_json::from_slice(&fs::read(&config.suites.definition_file).unwrap()).unwrap();
    assert_eq!(stored["suites"][0]["extensions"], suite["extensions"]);
    // A catalog edit never rewrites an existing Suite declaration into its new path.
    let raw = fs::read_to_string(&config.tasks.metadata_file).unwrap().replace("connect-bot.ts", "replacement.ts");
    fs::write(&config.tasks.metadata_file, raw).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert!(body.get("extensions").is_none());
    assert_eq!(body["suites"][0]["extensions"], suite["extensions"]);
    let response = get_over_tcp(server.addr(), "/api/suites/shared").await;
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"suite":stored["suites"][0]}));
    let response = get_over_tcp(server.addr(), "/api/tasks").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let tasks: Value = serde_json::from_str(&response.body).unwrap();
    let fixed = tasks["options"]["extensions"].as_array().unwrap();
    assert!(!fixed.is_empty());
    assert!(fixed.iter().all(|extension| extension["kind"] == "fixed"));
    let response = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/shared", None).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["suites"], json!([]));
    assert!(body["fixedExtensions"].as_array().unwrap().iter().any(|entry| entry["id"] == "pi-connect-bot" && std::path::Path::new(entry["entrypoint"].as_str().unwrap()) == root.path().join("unavailable dependencies/replacement.ts")));
    assert!(body.get("extensions").is_none());
}

#[tokio::test]
async fn invalid_response_catalogs_reject_all_suite_writes_before_persistence() {
    let root = TempDir::new("suite catalogs fail before writes");
    let mut config = config(&root);
    let metadata = shared_metadata(&root, &mut config);
    let server = TestServer::start(&config).await;
    let existing = json!({"id":"existing","name":"Preserved","agents":["omp"]});
    let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&existing.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let persisted = fs::read(&config.suites.definition_file).unwrap();
    let new = json!({"id":"new","name":"Never saved","agents":["omp"]});
    let mut edited = existing.clone();
    edited["name"] = json!("Never updated");
    for damage in [
        "invalid TOML [".to_owned(),
        metadata.replace("${roots.pi_extensions}", "${roots.unknown}"),
        metadata.replace("${roots.pi_extensions}/connect-bot.ts", ""),
    ] {
        fs::write(&config.tasks.metadata_file, damage).unwrap();
        let response = get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert!(response.body.contains("metadata-invalid"), "{}", response.body);
        for (method, url, body) in [
            ("POST", "/api/suites", Some(new.to_string())),
            ("PUT", "/api/suites/existing", Some(edited.to_string())),
            ("DELETE", "/api/suites/existing", None),
        ] {
            let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
            assert_eq!(response.status, 500, "{}", response.body);
            assert!(response.body.contains("metadata-invalid"), "{}", response.body);
            assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
        }
        // Single-definition reads remain independent of fixed extension metadata.
        assert_eq!(get_over_tcp(server.addr(), "/api/suites/existing").await.status, 200);
    }
    fs::write(&config.tasks.metadata_file, metadata).unwrap();
    fs::write(&config.suites.mcp_file, "{broken central catalog").unwrap();
    for (method, url, body) in [
        ("POST", "/api/suites", Some(new.to_string())),
        ("PUT", "/api/suites/existing", Some(edited.to_string())),
        ("DELETE", "/api/suites/existing", None),
    ] {
        let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    }
}

// P25: mode is a closed explicit choice, not a nullable/inherited global value.
// Saving invalid modes through HTTP must not replace the last valid authority.
#[tokio::test]
async fn pi_mode_is_explicit_and_applicable_through_public_http() {
    let root = TempDir::new("suite explicit pi mode");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let literal = json!({"id":"pi-tools","name":"Pi tools","agents":["pi"],"skills":{"omp":[],"pi":[]},"extensions":{},"prompts":[],"lsp":{},"mcp":[],"pi":{"codemode":"only"}});
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

#[tokio::test]
async fn skills_accept_only_host_paths_and_reject_invalid_writes_without_persistence() {
    let root = TempDir::new("host keyed skill paths");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let valid = json!({"id":"selected","name":"Selected","agents":["omp"],
        "skills":{"omp":["parents with spaces"],"pi":["individual skill"]}});
    let saved = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&valid.to_string())).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let persisted = fs::read(&config.suites.definition_file).unwrap();
    for skills in [json!([]), json!(["legacy root"]), json!({"other":[]}),
        json!({"omp":[""]}), json!({"pi":[" \t "]})] {
        let mut invalid = valid.clone();
        invalid["skills"] = skills;
        for (method, path) in [("POST", "/api/suites"), ("PUT", "/api/suites/selected")] {
            let reply = request_json_over_tcp(server.addr(), method, path, Some(&invalid.to_string())).await;
            assert_eq!(reply.status, 400, "{}", reply.body);
            assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
        }
    }
    for skills in [json!({}), json!({"omp":["parent"]}), json!({"pi":["root"]})] {
        let mut input = valid.clone();
        input["skills"] = skills.clone();
        let reply = request_json_over_tcp(server.addr(), "PUT", "/api/suites/selected", Some(&input.to_string())).await;
        assert_eq!(reply.status, 200, "{}", reply.body);
        let reply = get_over_tcp(server.addr(), "/api/suites/selected").await;
        let actual: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(actual["suite"]["skills"]["omp"], skills.get("omp").cloned().unwrap_or(json!([])));
        assert_eq!(actual["suite"]["skills"]["pi"], skills.get("pi").cloned().unwrap_or(json!([])));
    }
}

// P4: an external invalid authority is a visible error, not a reason to return
// cached definitions or overwrite the user's bytes from an independent copy.
#[tokio::test]
async fn external_authority_damage_is_not_hidden_or_overwritten() {
    let root = TempDir::new("suite external authority damage");
    let config = config(&root);
    fs::create_dir_all(config.suites.definition_file.parent().unwrap()).unwrap();
    let literal = "{\"suites\":[{\"id\":\"external\",\"name\":\"Independent definition\",\"agents\":[\"omp\"],\"skills\":{\"omp\":[],\"pi\":[]},\"extensions\":{},\"prompts\":[],\"lsp\":{},\"mcp\":[]}]}";
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
