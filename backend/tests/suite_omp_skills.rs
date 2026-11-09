//! Approved seams: S2 public launch, S3 deployment configuration, S4 real Herdr
//! (the latter is exercised by scripts/test-real-herdr-suite-skills.py).
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use serde_json::{Value, json};
use std::fs;

async fn post(server: &TestServer, path: &str, value: Value) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), "POST", path, Some(&value.to_string())).await
}

// P46: after preview, an existing SKILL.md becomes unreadable under a real
// Windows sharing lock. Independent source: fail-closed launch, not a missing
// path approximation. A stale availability check would launch an empty env.
#[cfg(windows)]
#[tokio::test]
async fn unreadable_selected_skill_rejects_public_launch_before_herdr() {
    let root = TempDir::new("unreadable selected skill");
    let herdr = common::HerdrFixture::new("skill failure Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let skill = root.path().join("selected parents/unreadable skill");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "---\nname: selected\ndescription: Selected skill\n---\nSelected body").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "skills":{"omp":["selected parents"]}
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected skill", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    use std::os::windows::fs::OpenOptionsExt;
    let _locked = fs::OpenOptions::new().read(true).share_mode(0).open(skill.join("SKILL.md")).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("unreadable skill"), "{}", launch.body);
    assert!(herdr.calls().is_empty(), "no partial/empty startup is permitted");
}

// P46/S2+S3: an output location blocked by a regular file must fail closed,
// even with readable selected input. Checking resources alone would pass.
#[tokio::test]
async fn blocked_assembly_directory_rejects_public_launch_before_herdr() {
    let root = TempDir::new("blocked skill assembly");
    let herdr = common::HerdrFixture::new("blocked assembly Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("blocked");
    config.herdr.executable = herdr.executable().to_path_buf();
    fs::create_dir_all(root.path().join("selected parents/readable skill")).unwrap();
    fs::write(root.path().join("selected parents/readable skill/SKILL.md"), "---\nname: selected\ndescription: Selected skill\n---\nSelected body").unwrap();
    fs::write(&config.suites.generated_directory, "not a directory").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "skills":{"omp":["selected parents"]}
    }]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Selected skill", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("selected"), "{}", launch.body);
    assert!(herdr.calls().is_empty());
    assert_eq!(fs::read_to_string(&config.suites.generated_directory).unwrap(), "not a directory");
}

// S2/S3: inspect native launch inputs and consume their real source paths.
// Invalid grandchildren/root SKILL.md must not be scanned recursively; duplicate
// lexical parents must not become duplicate native sources or name collisions.
#[tokio::test]
async fn omp_direct_children_canonical_parents_empty_no_fallback_and_name_collisions() {
    let root = TempDir::new("native skill parents");
    let herdr = common::HerdrFixture::new("native skill parents Herdr");
    let mut config = common::config(std::path::Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_owned();
    let parent = root.path().join("selected parents");
    for relative in ["selected", "nested/deeper", ".hidden", "disabled", "undescribed", "repaired"] {
        fs::create_dir_all(parent.join(relative)).unwrap();
    }
    fs::write(parent.join("SKILL.md"), "---\n: [invalid yaml\n---").unwrap();
    for relative in ["nested/deeper", ".hidden"] {
        fs::write(parent.join(relative).join("SKILL.md"), "---\n: [invalid yaml\n---").unwrap();
    }
    fs::write(parent.join("disabled/SKILL.md"), "<!-- native metadata comment -->\r\n---\r\nname: selected\r\ndescription: Disabled: resource\r\nenabled: false\r\n---\r\nDisabled").unwrap();
    // OMP repairs ambiguous unquoted scalars rather than rejecting metadata.
    fs::write(parent.join("repaired/SKILL.md"), "<!-- native metadata comment -->\r\n---\r\nname: repaired\r\ndescription: Native: repaired scalar\r\n---\r\nREPAIRED-SOURCE-651").unwrap();
    fs::write(parent.join("undescribed/SKILL.md"), "---\nname: selected\n---\nNo description").unwrap();
    fs::write(parent.join("selected/SKILL.md"), "---\nname: selected\ndescription: Direct child\n---\nDIRECT-SOURCE-418").unwrap();
    fs::write(parent.join("selected/asset.txt"), "SOURCE-ASSET-925").unwrap();
    let other = root.path().join("other parents");
    fs::create_dir_all(other.join("literal")).unwrap();
    fs::write(other.join("literal/SKILL.md"), "---\nname: '!literal*?[ab]{x,y}(z)'\ndescription: Literal native name\n---\nLITERAL-SOURCE-731").unwrap();
    let inactive = root.path().join("inactive parents");
    fs::create_dir_all(inactive.join("disabled")).unwrap();
    fs::write(inactive.join("disabled/SKILL.md"), "---\nname: selected\ndescription: Disabled\nenabled: false\n---\nDisabled").unwrap();
    let empty = root.path().join("empty parents");
    fs::create_dir(&empty).unwrap();
    // Explicitly empty selection must not use the other host's roots.
    fs::write(&config.suites.definition_file, json!({"suites":[{"id":"selected","name":"Selected","agents":["omp"],
        "skills":{"omp":["selected parents", "selected parents/../selected parents"],"pi":["missing Pi root"]}}]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Native parents", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    for (paths, expected_names, expected_parents) in [
        (json!(["selected parents", "selected parents/../selected parents", "other parents"]),
            json!(["\\!literal\\*\\?\\[ab\\]\\{x,y\\}\\(z\\)", "added", "repaired", "selected"]),
            json!([fs::canonicalize(&parent).unwrap(), fs::canonicalize(&other).unwrap()])),
        (json!(["selected parents"]), json!(["added", "repaired", "selected"]), json!([fs::canonicalize(&parent).unwrap()])),
        (json!(["inactive parents"]), json!([]), json!([fs::canonicalize(&inactive).unwrap()])),
        (json!(["empty parents"]), json!([]), json!([fs::canonicalize(&empty).unwrap()])),
        (json!([]), json!([]), json!([])),
    ] {
        let expected_enabled = !expected_names.as_array().unwrap().is_empty();
        fs::write(&config.suites.definition_file, json!({"suites":[{"id":"selected","name":"Selected","agents":["omp"],
            "skills":{"omp":paths,"pi":["missing Pi root"]}}]}).to_string()).unwrap();
        let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
        assert_eq!(preview.status, 200, "{}", preview.body);
        let preview: Value = serde_json::from_str(&preview.body).unwrap();
        // A launch must rescan even after preview has read the collection.
        if !parent.join("added").exists() {
            fs::create_dir(parent.join("added")).unwrap();
            fs::write(parent.join("added/SKILL.md"), "---\nname: added\ndescription: Added after preview\n---\nDYNAMIC-SOURCE-842").unwrap();
        }
        let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id, "chatId":preview["chatId"], "input":"consume source"})).await;
        assert_eq!(launch.status, 202, "{}", launch.body);
        let accepted: Value = serde_json::from_str(&launch.body).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let reply = common::get_over_tcp(server.addr(), &format!("/api/chats/{}", accepted["id"].as_str().unwrap())).await;
                assert_eq!(reply.status, 200, "{}", reply.body);
                let state: Value = serde_json::from_str(&reply.body).unwrap();
                assert_ne!(state["stage"], "failed", "{state}");
                if state["stage"] == "succeeded" { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("owned launch finishes");
        let calls = herdr.calls();
        let call = calls.iter().rev().find(|call| call["kind"] == "agent_start").unwrap();
        let args = call["argv"].as_array().unwrap();
        let at = args.iter().position(|arg| arg == "--config").unwrap();
        let manifest = std::path::Path::new(args[at + 1].as_str().unwrap());
        let native: Value = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
        assert_eq!(native["skills"], json!({"enabled":expected_enabled,
            "enableCodexUser":false,"enableClaudeUser":false,"enableClaudeProject":false,
            "enablePiUser":false,"enablePiProject":false,"enableAgentsUser":false,"enableAgentsProject":false,
            "customDirectories":expected_parents,"includeSkills":expected_names,"ignoredSkills":[]}));
        assert!(!manifest.parent().unwrap().join("skills").exists());
        if expected_enabled {
            let source = std::path::Path::new(native["skills"]["customDirectories"][0].as_str().unwrap()).join("selected");
            assert!(fs::read_to_string(source.join("SKILL.md")).unwrap().contains("DIRECT-SOURCE-418"));
            assert_eq!(fs::read_to_string(source.join("asset.txt")).unwrap(), "SOURCE-ASSET-925");
        }
    }
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    // Earlier successful Chats may still query native session identity.
    // A rejected launch must add no effects, not stop those read-only queries.
    let effects = || herdr.calls().into_iter().filter(|call| {
        !matches!(call["kind"].as_str(), Some("workspace_list" | "agent_list" | "agent_get"))
    }).collect::<Vec<_>>();
    let before = effects();
    fs::create_dir(other.join("collision")).unwrap();
    fs::write(other.join("collision/SKILL.md"), "---\nname: repaired\ndescription: Different source\n---\nCOLLISION").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{"id":"selected","name":"Selected","agents":["omp"],
        "skills":{"omp":["selected parents", "other parents"]}}]}).to_string()).unwrap();
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id, "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("repaired"), "{}", launch.body);
    assert_eq!(effects(), before);
}
