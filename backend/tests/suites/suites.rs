//! Suite S2 contracts. Expected values are independent declarations, never
//! computed through the product catalog. S1/S3 restart evidence lives in suites.spec.ts.
#[path = "../common/mod.rs"]
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

// Chief P3: shared fixed entries apply to chief too, while manager/node entries
// remain exclusive. Omitting chief from the public catalog would hide actual
// applicability; listing never claims that a native host consumed the extension.
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
        for (suffix, roles) in [("connect-bot", json!(["standalone","manager","node","chief","chief-delegated"])), ("manager-chat", json!(["manager"])), ("node-chat", json!(["node"])), ("chief-report", json!(["chief-delegated"]))] {
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
    let mut choices = [
        json!({"id":"shared-pi","name":"Shared choices · Pi","harness":"pi","codemode":"on","skills":[],"mcp":[],"prompts":[],
            "extensions":["${roots.pi_extensions}/pi-web-access/index.ts","packages with spaces/entry.ts"]}),
        json!({"id":"shared-omp","name":"Shared choices · OMP","harness":"omp","skills":[],"mcp":[],"lsp":{},
            "extensions":["custom-omp.ts"]}),
    ];
    for (suite, label) in choices.iter_mut().zip(["Pi", "OMP"]) {
        let detail = format!("/api/suites/{}", suite["id"].as_str().unwrap());
        for (method, url) in [("POST", "/api/suites"), ("PUT", detail.as_str())] {
            if method == "PUT" { suite["name"] = json!(format!("Edited choices · {label}")); }
            let response = request_json_over_tcp(server.addr(), method, url, Some(&suite.to_string())).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let body: Value = serde_json::from_str(&response.body).unwrap();
            assert!(body["suites"].as_array().unwrap().contains(suite));
            assert_eq!(body["fixedExtensions"], listing["fixedExtensions"]);
        }
    }
    let stored: Value = serde_json::from_slice(&fs::read(&config.suites.definition_file).unwrap()).unwrap();
    assert_eq!(stored["suites"], json!(choices));
    // A catalog edit never rewrites an existing Suite declaration into its new path.
    let raw = fs::read_to_string(&config.tasks.metadata_file).unwrap().replace("connect-bot.ts", "replacement.ts");
    fs::write(&config.tasks.metadata_file, raw).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert!(body.get("extensions").is_none());
    assert_eq!(body["suites"], stored["suites"]);
    for suite in &choices {
        let detail = format!("/api/suites/{}", suite["id"].as_str().unwrap());
        let response = get_over_tcp(server.addr(), &detail).await;
        assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"suite":suite}));
    }
    let response = get_over_tcp(server.addr(), "/api/tasks").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let tasks: Value = serde_json::from_str(&response.body).unwrap();
    let fixed = tasks["options"]["extensions"].as_array().unwrap();
    assert!(!fixed.is_empty());
    assert!(fixed.iter().all(|extension| extension["kind"] == "fixed"));
    let response = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/shared-pi", None).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["suites"], json!([choices[1]]));
    let response = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/shared-omp", None).await;
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
    let existing = json!({"id":"existing-omp","name":"Preserved · OMP","harness":"omp"});
    let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&existing.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let persisted = fs::read(&config.suites.definition_file).unwrap();
    let new = json!({"id":"new-omp","name":"Never saved · OMP","harness":"omp"});
    let mut edited = existing.clone();
    edited["name"] = json!("Never updated · OMP");
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
            ("PUT", "/api/suites/existing-omp", Some(edited.to_string())),
            ("DELETE", "/api/suites/existing-omp", None),
        ] {
            let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
            assert_eq!(response.status, 500, "{}", response.body);
            assert!(response.body.contains("metadata-invalid"), "{}", response.body);
            assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
        }
        // Single-definition reads remain independent of fixed extension metadata.
        assert_eq!(get_over_tcp(server.addr(), "/api/suites/existing-omp").await.status, 200);
    }
    fs::write(&config.tasks.metadata_file, metadata).unwrap();
    fs::write(&config.suites.mcp_file, "{broken central catalog").unwrap();
    for (method, url, body) in [
        ("POST", "/api/suites", Some(new.to_string())),
        ("PUT", "/api/suites/existing-omp", Some(edited.to_string())),
        ("DELETE", "/api/suites/existing-omp", None),
    ] {
        let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    }
}

// P25: mode is a closed explicit choice, not a nullable/inherited global value.
// Saving invalid modes through HTTP must not replace the last valid authority.
#[tokio::test]
async fn pi_mode_is_explicit_through_public_http() {
    let root = TempDir::new("suite explicit pi mode");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let literal = json!({"id":"tools-pi","name":"Tools · Pi","harness":"pi","skills":[],"extensions":[],"prompts":[],"mcp":[],"codemode":"only"});
    let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&literal.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let persisted = fs::read(&config.suites.definition_file).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&persisted).unwrap(), json!({"suites":[literal]}));
    for codemode in [json!(null), json!({}), json!("off"), json!("inherit"), json!(false)] {
        let mut input = literal.clone();
        input["codemode"] = codemode;
        let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/tools-pi", Some(&input.to_string())).await;
        assert_eq!(response.status, 400, "{}", response.body);
        assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    }
    let mut missing = literal.clone();
    missing.as_object_mut().unwrap().remove("codemode");
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/tools-pi", Some(&missing.to_string())).await;
    assert_eq!(response.status, 400, "{}", response.body);
    assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
    let mut on = literal.clone();
    on["codemode"] = json!("on");
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/tools-pi", Some(&on.to_string())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let response = get_over_tcp(server.addr(), "/api/suites/tools-pi").await;
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap(), json!({"suite":on}));
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&config.suites.definition_file).unwrap()).unwrap(), json!({"suites":[on]}));
}

// The path retains the saved ID, so a changed harness must be rejected as
// immutable, not treated as a rename, conversion, or cleared environment.
#[tokio::test]
async fn suite_harness_cannot_change_after_save() {
    let root = TempDir::new("immutable suite harness");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let omp = json!({"id":"tools-omp","name":"Tools · OMP","harness":"omp","skills":[],"extensions":["keep.ts"],"mcp":[],"lsp":{}});
    let saved = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&omp.to_string())).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let before = fs::read(&config.suites.definition_file).unwrap();
    let reassigned = json!({"id":"tools-omp","name":"Tools · Pi","harness":"pi","codemode":"on"});
    let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/tools-omp", Some(&reassigned.to_string())).await;
    assert_eq!(response.status, 400, "{}", response.body);
    assert!(response.body.contains("不能更改 Suite 宿主"), "{}", response.body);
    assert_eq!(fs::read(&config.suites.definition_file).unwrap(), before);
    let read = get_over_tcp(server.addr(), "/api/suites/tools-omp").await;
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap(), json!({"suite":omp}));
}

// Wrong-harness fields (even empty) and old fields are errors, not resources
// to discard. Both public mutation routes must leave the stored file intact.
#[tokio::test]
async fn suite_rejects_wrong_harness_fields_and_legacy_wire_shape() {
    let root = TempDir::new("strict suite wire shape");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let omp = json!({"id":"strict-omp","name":"Strict · OMP","harness":"omp"});
    let pi = json!({"id":"strict-pi","name":"Strict · Pi","harness":"pi","codemode":"on"});
    for suite in [&omp, &pi] {
        let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&suite.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    let before = fs::read(&config.suites.definition_file).unwrap();
    for (suite, key, value) in [
        (&omp, "prompts", json!([])), (&omp, "codemode", json!("on")),
        (&pi, "lsp", json!({})), (&pi, "lsp", json!(null)),
        (&omp, "agents", json!(["omp"])), (&pi, "pi", json!({"codemode":"on"})),
        (&pi, "extensions", json!({"pi":[]})),
        (&omp, "harness", json!(["omp"])),
        (&omp, "id", json!("strict-pi")), (&pi, "name", json!("Strict · OMP")),
    ] {
        let original = suite["id"].as_str().unwrap();
        let detail = format!("/api/suites/{original}");
        let mut invalid = suite.clone();
        invalid[key] = value;
        for (method, path) in [("POST", "/api/suites"), ("PUT", detail.as_str())] {
            let response = request_json_over_tcp(server.addr(), method, path, Some(&invalid.to_string())).await;
            assert_eq!(response.status, 400, "{invalid}: {}", response.body);
            assert_eq!(fs::read(&config.suites.definition_file).unwrap(), before);
        }
    }
    let malformed_file = json!({"suites":[{"id":"strict-omp","name":"Strict · OMP","harness":"omp","prompts":[]}]}).to_string();
    fs::write(&config.suites.definition_file, &malformed_file).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(response.body.contains("suite-storage-failed"));
    assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), malformed_file);
}

#[tokio::test]
async fn skills_accept_single_harness_paths_and_reject_invalid_writes_without_persistence() {
    let root = TempDir::new("flat harness skill paths");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    for (valid, detail) in [
        (json!({"id":"selected-omp","name":"Selected · OMP","harness":"omp","skills":["parents with spaces"]}), "/api/suites/selected-omp"),
        (json!({"id":"selected-pi","name":"Selected · Pi","harness":"pi","skills":["individual skill"],"codemode":"on"}), "/api/suites/selected-pi"),
    ] {
        let saved = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&valid.to_string())).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let persisted = fs::read(&config.suites.definition_file).unwrap();
        for skills in [json!({}), json!({"omp":[]}), json!({"pi":["legacy root"]}), json!([""]), json!([" \t "])] {
            let mut invalid = valid.clone();
            invalid["skills"] = skills;
            for (method, path) in [("POST", "/api/suites"), ("PUT", detail)] {
                let reply = request_json_over_tcp(server.addr(), method, path, Some(&invalid.to_string())).await;
                assert_eq!(reply.status, 400, "{}", reply.body);
                assert_eq!(fs::read(&config.suites.definition_file).unwrap(), persisted);
            }
        }
        for skills in [valid["skills"].clone(), json!([])] {
            let mut input = valid.clone();
            input["skills"] = skills.clone();
            let reply = request_json_over_tcp(server.addr(), "PUT", detail, Some(&input.to_string())).await;
            assert_eq!(reply.status, 200, "{}", reply.body);
            let reply = get_over_tcp(server.addr(), detail).await;
            let actual: Value = serde_json::from_str(&reply.body).unwrap();
            assert_eq!(actual["suite"]["skills"], skills);
        }
    }
}

// P4: an external invalid authority is a visible error, not a reason to return
// cached definitions or overwrite the user's bytes from an independent copy.
#[tokio::test]
async fn external_authority_damage_is_not_hidden_or_overwritten() {
    let root = TempDir::new("suite external authority damage");
    let config = config(&root);
    fs::create_dir_all(config.suites.definition_file.parent().unwrap()).unwrap();
    let literal = r#"{"suites":[{"id":"external-omp","name":"Independent definition · OMP","harness":"omp","skills":[],"extensions":[],"lsp":{},"mcp":[]}]}"#;
    fs::write(&config.suites.definition_file, literal).unwrap();
    let server = TestServer::start(&config).await;
    assert_eq!(get_over_tcp(server.addr(), "/api/suites/external-omp").await.status, 200);
    let damaged = "{\"suites\":[BROKEN external edit";
    fs::write(&config.suites.definition_file, damaged).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(response.status, 500, "{}", response.body);
    assert!(response.body.contains("suite-storage-failed"));
    let valid = json!({"id":"external-omp","name":"Must not overwrite · OMP","harness":"omp"});
    for (method, url, body) in [("PUT", "/api/suites/external-omp", Some(valid.to_string())), ("DELETE", "/api/suites/external-omp", None)] {
        let response = request_json_over_tcp(server.addr(), method, url, body.as_deref()).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), damaged);
    }
    fs::write(&config.suites.definition_file, literal).unwrap();
    let response = get_over_tcp(server.addr(), "/api/suites/external-omp").await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["suite"]["name"], "Independent definition · OMP");
    assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), literal);
}
