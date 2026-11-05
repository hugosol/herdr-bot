//! P34/P35/P36 at approved S2/S3: real disk artifacts and public launch HTTP.
mod common;
use common::{TempDir, TestServer, request_json_over_tcp};
use serde_json::{Value, json};
use futures_util::FutureExt;
use std::{fs, path::Path};
#[cfg(windows)]
use std::path::PathBuf;

async fn post(server: &TestServer, path: &str, value: Value) -> common::HttpResponse {
    request_json_over_tcp(server.addr(), "POST", path, Some(&value.to_string())).await
}

// Source: P34/P36. A failed launch must not retain a published partial skill
// environment or delete an unowned sibling. Plausible violation: publish skills
// first, discover a missing extension second, and forget the first artifact.
#[tokio::test]
async fn failed_resource_check_leaves_no_partial_assembly_or_deleted_unowned_files() {
    resource_check_failure(false).await;
}

#[cfg(windows)]
#[tokio::test]
async fn controlled_read_permission_failure_preserves_source_and_unowned_output_without_starting_herdr() {
    resource_check_failure(true).await;
}

async fn resource_check_failure(locked: bool) {
    let root = TempDir::new("Suite assembly ownership");
    let herdr = common::HerdrFixture::new("assembly failure Herdr");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_path_buf();
    let entry = root.path().join("entry.ts");
    fs::write(&entry, "export default function(pi) {}\n").unwrap();
    fs::write(&config.suites.definition_file, json!({"suites":[{
        "id":"selected", "name":"Selected", "agents":["omp"], "extensions":{"omp":["entry.ts"]}
    }]}).to_string()).unwrap();
    let unowned = config.suites.generated_directory.join("selected").join("user-note.txt");
    fs::create_dir_all(unowned.parent().unwrap()).unwrap();
    fs::write(&unowned, "UNOWNED-KEEP-746").unwrap();
    let server = TestServer::start(&config).await;
    let saved = post(&server, "/api/tasks", json!({"name":"Assembly", "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":"selected"})).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let id = &saved["tasks"][0]["id"];
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned", "path":root.path()})).await.status, 200);
    let preview = post(&server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    #[cfg(windows)]
    let source_before = file_hash(&entry);
    #[cfg(windows)]
    let lock = if locked {
        use std::os::windows::fs::OpenOptionsExt;
        Some(fs::OpenOptions::new().read(true).share_mode(0).open(&entry).unwrap())
    } else { None };
    if !locked { fs::remove_file(&entry).unwrap(); }
    let launch = post(&server, "/api/chats", json!({"zone":"owned", "taskId":id,
        "chatId":preview["chatId"], "input":"must not start"})).await;
    assert_eq!(launch.status, 400, "{}", launch.body);
    assert!(launch.body.contains("entry.ts"), "{}", launch.body);
    assert!(herdr.calls().is_empty());
    #[cfg(windows)]
    {
        drop(lock);
        if locked {
            assert_eq!(file_hash(&entry), source_before);
            assert_eq!(fs::read_to_string(&entry).unwrap(), "export default function(pi) {}\n");
        }
    }
    assert_eq!(fs::read_to_string(&unowned).unwrap(), "UNOWNED-KEEP-746");
    let names: Vec<_> = fs::read_dir(unowned.parent().unwrap()).unwrap()
        .map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(names, ["user-note.txt"], "failed launch published partial resources");
}

fn declaration(id: &str, sentinel: &str) -> Value {
    json!({"id":id, "name":id, "agents":["omp"], "mcp":[sentinel], "lsp":{
        "publication-probe":{"command":sentinel, "fileTypes":[".probe"]}
    }})
}

async fn save_task(server: &TestServer, suite: &str, name: &str) -> Value {
    let reply = post(server, "/api/tasks", json!({"name":name, "chatName":"{task.name}",
        "agent":"omp", "taskType":"normal", "model":"default", "thinking":"default", "skill":null,
        "prompt":"unchanged", "suite":suite})).await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let reply: Value = serde_json::from_str(&reply.body).unwrap();
    reply["tasks"].as_array().unwrap().iter().find(|task| task["name"] == name).unwrap()["id"].clone()
}

async fn launch_body(server: &TestServer, id: &Value) -> Value {
    let preview = post(server, "/api/chats/preview", json!({"zone":"owned", "taskId":id})).await;
    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview: Value = serde_json::from_str(&preview.body).unwrap();
    json!({"zone":"owned", "taskId":id, "chatId":preview["chatId"], "input":"observe publication"})
}

async fn starts(herdr: &common::HerdrFixture, count: usize) -> Vec<Value> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let calls: Vec<_> = herdr.calls().into_iter().filter(|call| call["kind"] == "agent_start").collect();
            if calls.len() >= count { return calls; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("both launches must reach Herdr while its first effect remains held")
}

async fn wait_finished(server: &TestServer, id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let reply = common::get_over_tcp(server.addr(), &format!("/api/chats/{id}")).await;
            assert_eq!(reply.status, 200, "{}", reply.body);
            let state: Value = serde_json::from_str(&reply.body).unwrap();
            if state["stage"] == "succeeded" { return; }
            assert_ne!(state["stage"], "failed", "{state}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("owned Herdr commands must exit and public launch must finish");
}

fn native_argument(call: &Value, flag: &str) -> std::path::PathBuf {
    let args = call["argv"].as_array().unwrap();
    let index = args.iter().position(|arg| arg == flag).unwrap();
    args[index + 1].as_str().unwrap().into()
}

#[cfg(windows)]
fn file_hash(path: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    fs::read(path).unwrap().hash(&mut hash);
    hash.finish()
}

#[cfg(windows)]
struct OwnedVolumeDirectory(PathBuf);
#[cfg(windows)]
impl Drop for OwnedVolumeDirectory {
    fn drop(&mut self) { fs::remove_dir_all(&self.0).expect("remove only the second-volume directory created by this test"); }
}

// P34/P36 real evidence. Intentionally ignored in the ordinary suite, never
// treated as a pass: the explicit command must run it with both prerequisites.
// No privilege probes, junction, shell elevation, developer-mode edits or copy
// fallbacks. Sources live on a separately supplied existing volume; Suite
// consumption uses their canonical parent directly, never generated links.
#[cfg(windows)]
#[tokio::test]
#[ignore = "requires HERDR_BOT_MANUAL_SYMLINKS=1 and HERDR_BOT_SYMLINK_SECOND_DRIVE (e.g. E:/)"]
async fn windows_true_file_directory_and_cross_volume_links_preserve_targets_on_failed_rebuild() {
    use std::os::windows::fs::{FileTypeExt, symlink_file};
    assert_eq!(std::env::var("HERDR_BOT_MANUAL_SYMLINKS").as_deref(), Ok("1"), "explicit real symlink opt-in is required");
    let second = PathBuf::from(std::env::var_os("HERDR_BOT_SYMLINK_SECOND_DRIVE")
        .expect("BLOCKED: supply an existing second disk root; do not install, mount or elevate"));
    let root = TempDir::new("true Suite symlinks");
    assert!(second.is_absolute() && second.is_dir(), "second volume must already exist");
    let local = fs::canonicalize(root.path()).unwrap();
    let other = fs::canonicalize(&second).unwrap();
    assert_ne!(local.components().next(), other.components().next(), "BLOCKED: cross-volume evidence requires a second disk");
    let volume = second.join(format!("herdr-owned-suite-{}", root.path().file_name().unwrap().to_string_lossy()));
    fs::create_dir(&volume).expect("exclusively create test-owned directory on second volume");
    let volume = OwnedVolumeDirectory(volume);
    let source = volume.0.join("source skill");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: selected\ndescription: Real link sentinel\n---\nSKILL-CROSS-VOLUME-618\n").unwrap();
    let real_file = root.path().join("real target.txt");
    fs::write(&real_file, "REAL-FILE-SENTINEL-902").unwrap();
    let alias = source.join("asset.txt");
    let directory_probe = root.path().join("cross-volume-directory-probe");
    let mut blockers = Vec::new();
    if let Err(error) = symlink_file(&real_file, &alias) {
        blockers.push(format!("true file symlink {} -> {}: {error}", alias.display(), real_file.display()));
    }
    if let Err(error) = std::os::windows::fs::symlink_dir(&source, &directory_probe) {
        blockers.push(format!("true directory symlink {} -> {}: {error}", directory_probe.display(), source.display()));
    }
    assert!(blockers.is_empty(), "BLOCKED: Windows true file/directory and cross-volume evidence; no junction/copy/elevation fallback: {}", blockers.join("; "));
    assert!(fs::symlink_metadata(&directory_probe).unwrap().file_type().is_symlink_dir());
    assert!(fs::symlink_metadata(&alias).unwrap().file_type().is_symlink_file());
    assert_eq!(fs::read_to_string(&alias).unwrap(), "REAL-FILE-SENTINEL-902");
    let file_before = file_hash(&real_file);
    let skill_before = file_hash(&source.join("SKILL.md"));
    let herdr = common::HerdrFixture::new("true link Herdr");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.tasks.storage_file = root.path().join("tasks.json");
    config.zones.registry_file = root.path().join("zones.json");
    config.suites.definition_file = root.path().join("suites.json");
    config.suites.mcp_file = root.path().join("mcp.json");
    config.suites.generated_directory = root.path().join("generated");
    config.herdr.executable = herdr.executable().to_owned();
    let suite = json!({"id":"selected","name":"Selected","agents":["omp"],"skills":{"omp":[volume.0.clone()]}});
    fs::write(&config.suites.definition_file, json!({"suites":[suite]}).to_string()).unwrap();
    let server = TestServer::start(&config).await;
    assert_eq!(post(&server, "/api/zones", json!({"id":"owned","path":root.path()})).await.status, 200);
    let id = save_task(&server, "selected", "True links").await;
    let body = launch_body(&server, &id).await;
    let reply = post(&server, "/api/chats", body).await;
    assert_eq!(reply.status, 202, "{}", reply.body);
    let accepted: Value = serde_json::from_str(&reply.body).unwrap();
    wait_finished(&server, accepted["id"].as_str().unwrap()).await;
    let calls = starts(&herdr, 1).await;
    let manifest = native_argument(&calls[0], "--config");
    let assembly = manifest.parent().unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(manifest["skills"]["includeSkills"], json!(["selected"]));
    assert_eq!(manifest["skills"]["customDirectories"], json!([fs::canonicalize(&volume.0).unwrap()]));
    assert!(!assembly.join("skills").exists());
    assert_eq!(fs::read_to_string(source.join("asset.txt")).unwrap(), "REAL-FILE-SENTINEL-902");
    let unowned = config.suites.generated_directory.join("selected").join("keep.txt");
    fs::write(&unowned, "UNOWNED-KEEP-417").unwrap();
    let bad = volume.0.join("bad skill");
    fs::create_dir(&bad).unwrap();
    fs::write(bad.join("SKILL.md"), "---\nname: selected\ndescription: Duplicate selected name\n---\nDO-NOT-DELETE-579\n").unwrap();
    let bad_before = file_hash(&bad.join("SKILL.md"));
    let definition = json!({"suites":[{"id":"selected","name":"Selected","agents":["omp"],"skills":{"omp":[volume.0.clone()]}}]});
    fs::write(&config.suites.definition_file, definition.to_string()).unwrap();
    let body = launch_body(&server, &id).await;
    let failed = post(&server, "/api/chats", body).await;
    assert_eq!(failed.status, 400, "{}", failed.body);
    assert!(failed.body.contains("selected"), "{}", failed.body);
    assert_eq!(starts(&herdr, 1).await.len(), 1, "failed rebuild must not start another agent");
    let remaining: Vec<_> = fs::read_dir(unowned.parent().unwrap()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(remaining.len(), 2, "only previous complete assembly and unowned file may remain: {remaining:?}");
    assert!(remaining.contains(&assembly.to_path_buf()) && remaining.contains(&unowned));
    assert!(fs::symlink_metadata(&directory_probe).unwrap().file_type().is_symlink_dir());
    assert!(fs::symlink_metadata(&alias).unwrap().file_type().is_symlink_file());
    assert_eq!(file_hash(&real_file), file_before);
    assert_eq!(file_hash(&source.join("SKILL.md")), skill_before);
    assert_eq!(file_hash(&bad.join("SKILL.md")), bad_before);
    assert_eq!(fs::read_to_string(&unowned).unwrap(), "UNOWNED-KEEP-417");
    println!("P34/P36 VERIFIED: file symlink={} -> {}; directory symlink={} -> {}; cross-volume=true; independent file hashes={file_before:x}/{skill_before:x}/{bad_before:x}; failed rebuild cleaned only its private tree", alias.display(), real_file.display(), directory_probe.display(), source.display());
}

fn observe(call: &Value, sentinel: &str, resources: bool) -> std::path::PathBuf {
    let manifest = native_argument(call, "--config");
    let directory = native_argument(call, "--plugin-dir");
    assert_eq!(manifest.parent().unwrap(), directory);
    let native_mcp = directory.join("mcp");
    assert!(call["argv"].as_array().unwrap().windows(2).any(|pair|
        pair[0] == "--extension" && pair[1] == json!(native_mcp)));
    let mcp: Value = serde_json::from_slice(&fs::read(native_mcp.join(".mcp.json")).unwrap()).unwrap();
    let expected_mcp = if sentinel == "PUBLICATION-A-193" {
        json!({"mcpServers":{"PUBLICATION-A-193":{"command":"MCP-A-461"}}})
    } else { json!({"mcpServers":{"PUBLICATION-B-827":{"command":"MCP-B-902"}}}) };
    assert_eq!(mcp, expected_mcp);
    let skills: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(skills["skills"]["enabled"], resources);
    assert_eq!(skills["skills"]["includeSkills"], if resources { json!(["selected"]) } else { json!([]) });
    assert_eq!(skills["skills"]["ignoredSkills"], json!([]));
    if resources {
        let parents = skills["skills"]["customDirectories"].as_array().unwrap();
        assert_eq!(parents.len(), 1);
        let parent = Path::new(parents[0].as_str().unwrap());
        assert_eq!(fs::canonicalize(parent).unwrap(), parent);
        let skill = parent.join("selected");
        assert!(!fs::symlink_metadata(&skill).unwrap().file_type().is_symlink());
        let extension = native_argument(call, "--extension");
        assert!(!extension.starts_with(&directory), "native extension must remain at its source");
        let (body, asset) = if sentinel == "PUBLICATION-A-193" {
            ("SKILL-A-514", "EXTENSION-A-361")
        } else { ("SKILL-B-928", "EXTENSION-B-705") };
        assert!(fs::read_to_string(skill.join("SKILL.md")).unwrap().contains(body));
        assert_eq!(fs::read_to_string(extension.parent().unwrap().join("asset.txt")).unwrap(), asset);
        assert_eq!(fs::read_to_string(&extension).unwrap(), "export default function(pi) {}\n");
        assert!(!directory.join("extensions").exists(), "extensions need no generated package tree");
    }
    if !resources { assert_eq!(skills["skills"]["customDirectories"], json!([])); }
    assert!(!directory.join("skills").exists(), "skills are consumed directly from their sources");
    let lsp: Value = serde_json::from_slice(&fs::read(directory.join("lsp.json")).unwrap()).unwrap();
    assert_eq!(lsp, json!({"servers":{"publication-probe":{"command":sentinel,"fileTypes":[".probe"]}}}));
    directory
}

// P35: independent A/B literals, not expected manifests computed by production.
// Hold the first *external* effect while assembling the second, then reread both
// delivered artifacts. A shared live directory or a lease spanning Herdr would
// respectively mix the sets or prevent the second request reaching its gate.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_and_different_suites_publish_complete_independent_environments_without_holding_herdr() {
    publication_scenarios(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_and_different_suites_publish_complete_source_resource_sets() {
    publication_scenarios(true).await;
}

fn panic_message(error: &Box<dyn std::any::Any + Send>) -> &str {
    error.downcast_ref::<String>().map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied()).unwrap_or("non-string panic")
}

async fn publication_scenarios(resources: bool) {
    for same_suite in [true, false] {
        let root = TempDir::new("concurrent Suite publication");
        let herdr = common::HerdrFixture::new("publication Herdr gate");
        let mut config = common::config(Path::new("unused.bat"), root.path());
        config.tasks.storage_file = root.path().join("tasks.json");
        config.zones.registry_file = root.path().join("zones.json");
        config.suites.definition_file = root.path().join("suites.json");
        config.suites.mcp_file = root.path().join("mcp.json");
        config.suites.generated_directory = root.path().join("generated");
        config.herdr.executable = herdr.executable().to_owned();
        fs::write(&config.suites.mcp_file, json!({"mcpServers":{
            "PUBLICATION-A-193":{"command":"MCP-A-461"},
            "PUBLICATION-B-827":{"command":"MCP-B-902"}
        }}).to_string()).unwrap();
        let second_suite = if same_suite { "first" } else { "second" };
        let mut first = declaration("first", "PUBLICATION-A-193");
        let mut second = declaration(second_suite, "PUBLICATION-B-827");
        if resources {
            for (suite, name, body, asset) in [
                (&mut first, "source A", "SKILL-A-514", "EXTENSION-A-361"),
                (&mut second, "source B", "SKILL-B-928", "EXTENSION-B-705"),
            ] {
                let source = root.path().join(name);
                fs::create_dir_all(source.join("skill parents/selected")).unwrap();
                fs::create_dir(source.join("extension")).unwrap();
                fs::write(source.join("skill parents/selected").join("SKILL.md"), format!("---\nname: selected\ndescription: Selected resource\n---\n{body}\n")).unwrap();
                fs::write(source.join("extension").join("entry.ts"), "export default function(pi) {}\n").unwrap();
                fs::write(source.join("extension").join("asset.txt"), asset).unwrap();
                suite["skills"] = json!({"omp":[source.join("skill parents")]});
                suite["extensions"] = json!({"omp":[source.join("extension").join("entry.ts")]});
            }
        }
        #[cfg(windows)]
        let source_hashes: Vec<_> = if resources {
            ["source A", "source B"].into_iter().flat_map(|name| {
                ["skill parents/selected/SKILL.md", "extension/entry.ts", "extension/asset.txt"].map(|relative| root.path().join(name).join(relative))
            }).map(|path| { let hash = file_hash(&path); (path, hash) }).collect()
        } else { Vec::new() };
        let initial = if same_suite { vec![first] } else { vec![first, second.clone()] };
        fs::write(&config.suites.definition_file, json!({"suites":initial}).to_string()).unwrap();
        let release = root.path().join("release-herdr");
        herdr.set_behavior(json!({"agent_start_gate":{"wait_for_file":release,"timeout_ms":15000}}));
        let server = TestServer::start(&config).await;
        assert_eq!(post(&server, "/api/zones", json!({"id":"owned","path":root.path()})).await.status, 200);
        let task_a = save_task(&server, "first", "First observer").await;
        let task_b = save_task(&server, second_suite, "Second observer").await;
        let mut accepted = Vec::new();
        let observation = std::panic::AssertUnwindSafe(async {
            let body_a = launch_body(&server, &task_a).await;
            let a = post(&server, "/api/chats", body_a).await;
            assert_eq!(a.status, 202, "{}", a.body);
            accepted.push(serde_json::from_str::<Value>(&a.body).unwrap()["id"].as_str().unwrap().to_owned());
            let first_call = starts(&herdr, 1).await.remove(0);
            let first_directory = observe(&first_call, "PUBLICATION-A-193", resources);
            if same_suite {
                // External edits are not protected by the coordinator. Each
                // launch must still consume one complete read of authority.
                fs::write(&config.suites.definition_file, json!({"suites":[second]}).to_string()).unwrap();
            }
            let body_b = launch_body(&server, &task_b).await;
            let b = post(&server, "/api/chats", body_b).await;
            assert_eq!(b.status, 202, "{}", b.body);
            accepted.push(serde_json::from_str::<Value>(&b.body).unwrap()["id"].as_str().unwrap().to_owned());
            let calls = starts(&herdr, 2).await;
            let second_directory = observe(&calls[1], "PUBLICATION-B-827", resources);
            assert_ne!(first_directory, second_directory);
            assert_eq!(observe(&calls[0], "PUBLICATION-A-193", resources), first_directory);
            calls
        }).catch_unwind().await;
        let mut cleanup_errors = Vec::new();
        if let Err(error) = fs::write(&release, "release owned Herdr calls") {
            cleanup_errors.push(format!("release gate: {error}"));
        }
        // Always release and await even when a publication assertion failed.
        // Public completion follows reaping all owned adapter CLI commands.
        for id in accepted {
            if let Err(error) = std::panic::AssertUnwindSafe(wait_finished(&server, &id)).catch_unwind().await {
                cleanup_errors.push(format!("owned creation {id}: {}", panic_message(&error)));
            }
        }
        assert!(cleanup_errors.is_empty(), "original observation={:?}; separate cleanup failures={cleanup_errors:?}", observation.as_ref().err().map(panic_message));
        let calls = match observation { Ok(calls) => calls, Err(error) => std::panic::resume_unwind(error) };
        observe(&calls[0], "PUBLICATION-A-193", resources);
        observe(&calls[1], "PUBLICATION-B-827", resources);
        #[cfg(windows)]
        for (path, before) in &source_hashes { assert_eq!(file_hash(path), *before, "{}", path.display()); }
    }
}
