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
    for (id, label, agent, entrypoint) in [
        ("shared-pi", "Shared Pi", "pi", "${roots.pi_extensions}/pi-web-access"),
        ("relative-pi", "Relative Pi", "pi", "packages with spaces/pi-package"),
        ("relative-omp", "Relative OMP", "omp", "packages with spaces/omp-package"),
    ] {
        extensions.push(toml::Value::try_from(json!({"id":id,"label":label,"agent":agent,"kind":"optional","entrypoint":entrypoint})).unwrap());
    }
    let raw = toml::to_string(&metadata).unwrap();
    fs::write(&config.tasks.metadata_file, &raw).unwrap();
    raw
}

#[tokio::test]
async fn shared_extensions_preserve_roots_and_rebase_from_metadata_not_suite_storage() {
    let root = TempDir::new("suite shared extension declarations");
    let mut config = config(&root);
    shared_metadata(&root, &mut config);
    let metadata_directory = config.tasks.metadata_file.parent().unwrap();
    let pi_package = metadata_directory.join("packages with spaces").join("pi-package");
    let omp_package = metadata_directory.join("packages with spaces").join("omp-package");
    let server = TestServer::start(&config).await;
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let listing: Value = serde_json::from_str(&response.body).unwrap();
    let options = listing["extensions"].as_array().unwrap();
    assert!(options.iter().all(|option| option["kind"] == "optional"));
    let shared = options.iter().find(|option| option["id"] == "shared-pi").unwrap();
    assert_eq!(shared["agent"], "pi");
    assert_eq!(shared["entrypoint"], "${roots.pi_extensions}/pi-web-access");
    for (id, agent, expected_path) in [("relative-pi", "pi", &pi_package), ("relative-omp", "omp", &omp_package)] {
        let option = options.iter().find(|option| option["id"] == id).unwrap();
        assert_eq!(option["agent"], agent);
        assert_eq!(std::path::Path::new(option["entrypoint"].as_str().unwrap()), expected_path.as_path());
    }
    // No optional dependency exists; listing and saving do not inspect resources.
    assert!(!root.path().join("unavailable dependencies").exists());
    let mut suite = json!({"id":"shared","name":"Shared choices","agents":["pi","omp"],"pi":{"codemode":"on"},
        "extensionOptions":{"pi":["shared-pi","relative-pi"],"omp":["relative-omp"]},
        "extensions":{"omp":["custom-omp.ts"]}
    });
    for (method, url) in [("POST", "/api/suites"), ("PUT", "/api/suites/shared")] {
        let response = request_json_over_tcp(server.addr(), method, url, Some(&suite.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["suites"][0]["extensions"], suite["extensions"]);
        suite["name"] = json!("Edited choices");
    }
    let stored: Value = serde_json::from_slice(&fs::read(&config.suites.definition_file).unwrap()).unwrap();
    assert_eq!(stored["suites"][0]["extensions"], suite["extensions"]);
    assert_eq!(stored["suites"][0]["extensionOptions"], suite["extensionOptions"]);
    // A catalog edit never rewrites an existing Suite declaration into its new path.
    let raw = fs::read_to_string(&config.tasks.metadata_file).unwrap().replace("pi-web-access", "replacement-package");
    fs::write(&config.tasks.metadata_file, raw).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["extensions"][0]["entrypoint"], "${roots.pi_extensions}/replacement-package");
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
    assert_eq!(body["extensions"][0]["entrypoint"], "${roots.pi_extensions}/replacement-package");
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
        metadata.replace("packages with spaces/pi-package", ""),
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
        // Single-definition reads remain independent of the optional catalog.
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
    let literal = json!({"id":"pi-tools","name":"Pi tools","agents":["pi"],"skills":[],"extensions":{},"prompts":[],"lsp":{},"mcp":[],"pi":{"codemode":"only"}});
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
    let literal = "{\"suites\":[{\"id\":\"external\",\"name\":\"Independent definition\",\"agents\":[\"omp\"],\"skills\":[],\"extensions\":{},\"prompts\":[],\"lsp\":{},\"mcp\":[]}]}";
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
