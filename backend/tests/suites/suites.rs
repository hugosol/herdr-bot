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
    config.suites.dap_file = root.path().join("central-dap.json");
    config
}

#[tokio::test]
async fn dap_catalog_is_read_only_and_only_omp_suites_can_select_ids() {
    let root = TempDir::new("Suite DAP catalog");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let listing = get_over_tcp(server.addr(), "/api/suites").await;
    assert_eq!(listing.status, 200, "{}", listing.body);
    assert_eq!(serde_json::from_str::<Value>(&listing.body).unwrap()["dap"], json!([]));

    // Declaration only: the root and adapter executable are intentionally absent.
    let native = json!({"command":"${roots.dap_fixture}/missing.exe", "languages":["rust"], "connectMode":"stdio"});
    let definition = json!({"name":"Fixture adapter", "description":"Windows x64 fixture", "config":native});
    let catalog = json!({"adapters":{"codelldb":definition}}).to_string();
    fs::write(&config.suites.dap_file, &catalog).unwrap();
    let expected = json!([{"id":"codelldb", "name":"Fixture adapter", "description":"Windows x64 fixture", "config":native}]);
    let mut suite = json!({"id":"debug-omp", "name":"Debug · OMP", "harness":"omp", "dap":["codelldb"]});
    for (method, url) in [("POST", "/api/suites"), ("PUT", "/api/suites/debug-omp")] {
        let response = request_json_over_tcp(server.addr(), method, url, Some(&suite.to_string())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["dap"], expected);
        assert_eq!(body["suites"][0]["dap"], json!(["codelldb"]));
        suite["name"] = json!("Debug edited · OMP");
    }
    let stored = fs::read_to_string(&config.suites.definition_file).unwrap();
    assert!(!stored.contains("missing.exe"), "Suite must persist only adapter IDs");
    for choice in [json!(["unknown"]), json!(["codelldb", "codelldb"]), json!([""])] {
        suite["dap"] = choice;
        let response = request_json_over_tcp(server.addr(), "PUT", "/api/suites/debug-omp", Some(&suite.to_string())).await;
        assert_eq!(response.status, 400, "{}", response.body);
    }
    let pi = json!({"id":"debug-pi", "name":"Debug · Pi", "harness":"pi", "codemode":"on", "dap":[]});
    let response = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&pi.to_string())).await;
    assert_eq!(response.status, 400, "{}", response.body);
    assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), stored);
    assert_eq!(fs::read_to_string(&config.suites.dap_file).unwrap(), catalog);

    for malformed in [
        r#"{"adapters":{"bad":{"name":"Bad","description":"Wrong type","config":{"command":"dap","languages":"rust"}}}}"#,
        r#"{"adapters":{"bad":{"name":"Bad","description":"Duplicate ID","config":{"command":"old"}},"bad":{"name":"Bad","description":"Duplicate ID","config":{"command":"new"}}}}"#,
    ] {
        fs::write(&config.suites.dap_file, malformed).unwrap();
        let response = get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert!(response.body.contains("DAP") && response.body.contains("bad"), "{}", response.body);
        let response = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/debug-omp", None).await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), stored);
    }
    fs::write(&config.suites.dap_file, &catalog).unwrap();
    let response = request_json_over_tcp(server.addr(), "DELETE", "/api/suites/debug-omp", None).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["dap"], expected);
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
        json!({"id":"shared-omp","name":"Shared choices · OMP","harness":"omp","skills":[],"mcp":[],"lsp":{},"dap":[],
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

#[tokio::test]
async fn suite_ids_are_harness_independent_but_validated_on_reads_and_writes() {
    let root = TempDir::new("suite IDs independent of harness");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    for (id, harness, name) in [
        ("plain_tools", "omp", "Plain · OMP"),
        ("plain-pi-tools", "pi", "Plain · Pi"),
        ("opposite-pi", "omp", "Opposite · OMP"),
        ("opposite-omp", "pi", "Opposite · Pi"),
    ] {
        let mut input = json!({"id":id,"harness":harness,"name":name});
        if harness == "pi" { input["codemode"] = json!("on"); }
        let saved = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&input.to_string())).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let duplicate = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&input.to_string())).await;
        assert_eq!(duplicate.status, 409, "{}", duplicate.body);
        input["name"] = json!(format!("Edited {name}"));
        let detail = format!("/api/suites/{id}");
        let edited = request_json_over_tcp(server.addr(), "PUT", &detail, Some(&input.to_string())).await;
        assert_eq!(edited.status, 200, "{}", edited.body);
        let read = get_over_tcp(server.addr(), &detail).await;
        assert_eq!(read.status, 200, "{}", read.body);
        let actual: Value = serde_json::from_str(&read.body).unwrap();
        assert_eq!(actual["suite"]["id"], id);
        assert_eq!(actual["suite"]["harness"], harness);
        assert_eq!(actual["suite"]["name"], input["name"]);
    }
    let before = fs::read(&config.suites.definition_file).unwrap();
    for id in ["", " ", "../escape", "folder/name", "folder\\name", "tools.name", "工具"] {
        let invalid = json!({"id":id,"name":"Invalid · OMP","harness":"omp"});
        let reply = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&invalid.to_string())).await;
        assert_eq!(reply.status, 400, "{id}: {}", reply.body);
        assert_eq!(fs::read(&config.suites.definition_file).unwrap(), before);
    }
    let mut external: Value = serde_json::from_slice(&before).unwrap();
    external["suites"][0]["id"] = json!("manual_id");
    fs::write(&config.suites.definition_file, external.to_string()).unwrap();
    let read = get_over_tcp(server.addr(), "/api/suites/manual_id").await;
    assert_eq!(read.status, 200, "{}", read.body);
    for invalid in [json!(""), json!("../escape"), json!("opposite-pi")] {
        external["suites"][0]["id"] = invalid;
        let bytes = external.to_string();
        fs::write(&config.suites.definition_file, &bytes).unwrap();
        let read = get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(read.status, 500, "{}", read.body);
        assert!(read.body.contains("suite-storage-failed"), "{}", read.body);
        assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), bytes);
    }
}

#[tokio::test]
async fn suite_harness_cannot_change_after_save() {
    let root = TempDir::new("immutable suite harness");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    let omp = json!({"id":"tools-omp","name":"Tools · OMP","harness":"omp","skills":[],"extensions":["keep.ts"],"mcp":[],"lsp":{},"dap":[]});
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
        (&omp, "id", json!("invalid/id")), (&pi, "name", json!("Strict · OMP")),
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
    for (key, value) in [("prompts", json!([])), ("skills", json!(["legacy collection"]))] {
        let mut invalid = omp.clone();
        invalid[key] = value;
        let malformed_file = json!({"suites":[invalid]}).to_string();
        fs::write(&config.suites.definition_file, &malformed_file).unwrap();
        let response = get_over_tcp(server.addr(), "/api/suites").await;
        assert_eq!(response.status, 500, "{}", response.body);
        assert!(response.body.contains("suite-storage-failed"));
        assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), malformed_file);
    }
}

#[tokio::test]
async fn skills_round_trip_harness_specific_types_and_reject_invalid_writes() {
    let root = TempDir::new("harness-specific skill sources");
    let config = config(&root);
    let server = TestServer::start(&config).await;
    for (valid, detail, invalid_lists) in [
        (json!({"id":"selected-omp","name":"Selected · OMP","harness":"omp","skills":[
            {"kind":"collection","path":"parents with spaces"}, {"kind":"skill","path":"individual OMP skill"}
        ]}), "/api/suites/selected-omp", vec![
            json!(["legacy collection"]), json!([["skill", "root"]]),
            json!([{"kind":0,"path":"root"}]), json!([{"kind":"unknown","path":"root"}]),
            json!([{"path":"root"}]), json!([{"kind":"skill"}]), json!([{"kind":"skill","path":42}]),
            json!([{"kind":"skill","path":"root","name":"extra"}]),
            json!([{"kind":"collection","path":""}]), json!([{"kind":"skill","path":" \t "}]),
        ]),
        (json!({"id":"selected-pi","name":"Selected · Pi","harness":"pi","skills":["individual skill"],"codemode":"on"}),
            "/api/suites/selected-pi", vec![json!([{"kind":"skill","path":"root"}]), json!([""]), json!([" \t "])]),
    ] {
        let saved = request_json_over_tcp(server.addr(), "POST", "/api/suites", Some(&valid.to_string())).await;
        assert_eq!(saved.status, 200, "{}", saved.body);
        let persisted = fs::read(&config.suites.definition_file).unwrap();
        for skills in [json!({}), json!({"omp":[]}), json!({"pi":["legacy root"]})].into_iter().chain(invalid_lists) {
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
    let response: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(response["suite"]["name"], "Independent definition · OMP");
    assert_eq!(response["suite"]["dap"], json!([]));
    assert_eq!(fs::read_to_string(&config.suites.definition_file).unwrap(), literal);
}
