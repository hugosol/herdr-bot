//! S2/S3: real process starts, public HTTP operations and independently prepared disk evidence.
mod common;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{HerdrFixture, HttpResponse, TempDir, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

struct Panel {
    child: Child,
    addr: SocketAddr,
}

fn command(root: &Path) -> Command {
    let metadata_dir = root.join("meta-data");
    fs::create_dir_all(&metadata_dir).unwrap();
    fs::write(
        metadata_dir.join("bot-options.toml"),
        include_str!("../../meta-data/bot-options.toml"),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-bot"));
    command
        .current_dir(root)
        .arg("--config")
        .arg("herdr-bot.toml")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

impl Panel {
    fn start(root: &Path) -> Self {
        let mut child = command(root).spawn().expect("start real application");
        let log = Arc::new(Mutex::new(Vec::new()));
        for source in [
            Box::new(child.stdout.take().unwrap()) as Box<dyn std::io::Read + Send>,
            Box::new(child.stderr.take().unwrap()) as Box<dyn std::io::Read + Send>,
        ] {
            let log = Arc::clone(&log);
            std::thread::spawn(move || {
                let mut source = source;
                let mut buffer = [0; 4096];
                while let Ok(n) = source.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    log.lock().unwrap().extend_from_slice(&buffer[..n]);
                }
            });
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let text = String::from_utf8_lossy(&log.lock().unwrap()).into_owned();
            if let Some(addr) = text.lines().find_map(|line| {
                line.split("http://")
                    .nth(1)?
                    .trim_end_matches('/')
                    .parse()
                    .ok()
            }) {
                return Self { child, addr };
            }
            if let Some(status) = child.try_wait().unwrap() {
                panic!("application exited {status}: {text}");
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("application did not start: {text}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn configure(root: &Path, herdr: &HerdrFixture, storage: Option<&str>) {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations/success.bat");
    let operations = fixtures.join("operations");
    let mut config = format!(
        "listen_addr = '127.0.0.1:0'\n\n\
         [operations.start]\nscript = '{}'\nworking_directory = '{}'\n\n\
         [operations.stop]\nscript = '{}'\nworking_directory = '{}'\n\n\
         [mdns]\nenabled = false\n\n\
         [herdr]\nworkspace_name = 'bot-playground'\nexecutable = '{}'\n",
        script.display(),
        operations.display(),
        script.display(),
        operations.display(),
        herdr.executable().display(),
    );
    if let Some(storage) = storage {
        config.push_str(&format!("\n[bot_templates]\nstorage_file = '{storage}'\n"));
    }
    fs::write(root.join("herdr-bot.toml"), config).unwrap();
}

fn configure_delivery(root: &Path, herdr: &HerdrFixture) {
    let mut config: toml::Value = toml::from_str(include_str!("../../herdr-bot.toml")).unwrap();
    config["listen_addr"] = toml::Value::String("127.0.0.1:0".into());
    let mut mdns = toml::Table::new();
    mdns.insert("enabled".into(), toml::Value::Boolean(false));
    config
        .as_table_mut()
        .unwrap()
        .insert("mdns".into(), toml::Value::Table(mdns));
    config["herdr"].as_table_mut().unwrap().insert(
        "executable".into(),
        toml::Value::String(herdr.executable().to_string_lossy().into_owned()),
    );
    let operations = common::repo_fixtures_dir().join("operations");
    for name in ["start", "stop"] {
        config["operations"][name]["script"] = toml::Value::String(
            operations
                .join("success.bat")
                .to_string_lossy()
                .into_owned(),
        );
        config["operations"][name]["working_directory"] =
            toml::Value::String(operations.to_string_lossy().into_owned());
    }
    fs::write(
        root.join("herdr-bot.toml"),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
}

fn body(response: HttpResponse) -> Value {
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn legacy_data(root: &Path, zone: &Path) -> Vec<u8> {
    let registry = serde_json::to_vec_pretty(&json!({"zones": [{"path": zone}]})).unwrap();
    fs::write(root.join("zones.json"), &registry).unwrap();
    fs::write(root.join("zones.json.tmp"), "old zone temporary").unwrap();
    fs::write(
        root.join("bot-templates.json"),
        r#"{"templates":[{"id":"legacy","name":"discard me"}]}"#,
    )
    .unwrap();
    fs::write(
        root.join("bot-templates.json.tmp"),
        "old template temporary",
    )
    .unwrap();
    fs::create_dir(root.join("bot-sessions")).unwrap();
    fs::write(root.join("bot-sessions/old.jsonl"), "OLD CONTEXT").unwrap();
    fs::write(root.join("unrelated.txt"), "keep unrelated bytes\0\n").unwrap();
    registry
}

async fn save(panel: &Panel) -> Value {
    let input = json!({
        "name": "Fresh template", "botName": "{zone.name}-{bot.id}",
        "agent": "omp", "model": "default", "thinking": "default",
        "skill": null, "prompt": "fresh prompt\nsecond line"
    });
    body(post_json_over_tcp(panel.addr, "/api/bot-templates", &input.to_string()).await)["templates"][0].clone()
}

async fn create(panel: &Panel, zone: &Path, template: &Value) -> Value {
    let preview = body(
        post_json_over_tcp(
            panel.addr,
            "/api/bots/preview",
            &json!({"zone": zone, "templateId": template["id"]}).to_string(),
        )
        .await,
    );
    let response = post_json_over_tcp(panel.addr, "/api/bots", &json!({
        "zone": zone, "templateId": template["id"], "botId": preview["botId"], "input": "new task"
    }).to_string()).await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let view = body(
            get_over_tcp(
                panel.addr,
                &format!("/api/bots/{}", accepted["id"].as_str().unwrap()),
            )
            .await,
        );
        if view["stage"] == "succeeded" {
            return view;
        }
        assert_ne!(view["stage"], "failed", "{view}");
        assert!(Instant::now() < deadline, "{view}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn native_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, dir: &Path, entries: &mut std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                entries.insert(relative, None);
                visit(root, &path, entries);
            } else {
                entries.insert(relative, Some(fs::read(path).unwrap()));
            }
        }
    }
    let mut entries = std::collections::BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

// P23/P25, S4: public creations leave the complete old native tree unchanged;
// the actual delivered executable is stopped/restarted, not an in-process store double.
#[tokio::test]
async fn default_and_delivered_creations_leave_old_native_tree_unchanged() {
    for delivered in [false, true] {
        let root = TempDir::new("runtime fresh creations");
        let herdr = HerdrFixture::new("runtime fresh creation calls");
        if delivered {
            configure_delivery(root.path(), &herdr);
        } else {
            configure(root.path(), &herdr, None);
        }
        let zone = root.path().join("zone");
        fs::create_dir(&zone).unwrap();
        assert!(!root.path().join(".data").exists());
        let panel = Panel::start(root.path());
        body(
            post_json_over_tcp(panel.addr, "/api/zones", &json!({"path": zone}).to_string()).await,
        );
        let template = save(&panel).await;
        assert!(root.path().join(".data/bot-templates.json").is_file());
        assert!(root.path().join(".data/zones.json").is_file());
        // P25/S4: legacy main + nested sidecar bytes and directory inventory are
        // independent literals, not files produced by the implementation.
        let native = root.path().join(".data/bot-sessions");
        fs::create_dir_all(native.join("old-session/records/empty")).unwrap();
        fs::write(native.join("old-session/context.jsonl"), b"{\"type\":\"session\",\"id\":\"old-native\"}\n").unwrap();
        fs::write(native.join("old-session/records/tool.bin"), b"\0old-sidecar\xff\r\n").unwrap();
        let before = native_snapshot(&native);
        let first = create(&panel, &zone, &template).await;
        let second = create(&panel, &zone, &template).await;
        assert_ne!(first["id"], second["id"]);
        let starts: Vec<Value> = herdr
            .calls()
            .into_iter()
            .filter(|call| call["kind"] == "agent_start")
            .collect();
        assert_eq!(starts.len(), 2);
        for start in starts {
            assert_eq!(start["args"][0], "--no-extensions");
        }
        assert!(!root.path().join("bot-sessions").exists());
        assert_eq!(native_snapshot(&native), before, "P25: old main and sidecars unchanged");
        drop(panel);
        for _ in 0..2 {
            let restarted = Panel::start(root.path());
            assert_eq!(
                body(get_over_tcp(restarted.addr, "/api/bot-templates").await)["templates"],
                json!([template.clone()])
            );
            assert_eq!(native_snapshot(&native), before, "P25: restart must not touch old native files");
        }
    }
}

// P41/P46. Save through HTTP only after the legacy cutover, then actually stop and
// start the executable twice. A repeated reset or old-location fallback loses the saved literal.
#[tokio::test]
async fn default_and_explicit_template_locations_survive_two_restarts_without_legacy_fallback() {
    for explicit in [None, Some("private/templates.json")] {
        let root = TempDir::new("runtime persistence restart");
        let herdr = HerdrFixture::new("runtime restart calls");
        configure(root.path(), &herdr, explicit);
        let zone = root.path().join("zone");
        fs::create_dir(&zone).unwrap();
        legacy_data(root.path(), &zone);
        let storage = root
            .path()
            .join(explicit.unwrap_or(".data/bot-templates.json"));
        let session = storage
            .parent()
            .unwrap()
            .join("bot-sessions/new-session/context.jsonl");
        let saved = {
            let panel = Panel::start(root.path());
            assert_eq!(
                body(get_over_tcp(panel.addr, "/api/bot-templates").await)["templates"],
                json!([])
            );
            let saved = save(&panel).await;
            assert_eq!(saved["prompt"], "fresh prompt\nsecond line");
            assert!(storage.is_file());
            fs::create_dir_all(session.parent().unwrap()).unwrap();
            fs::write(&session, b"NEW SESSION\0\n").unwrap();
            saved
        };
        // Reappearing legacy files must never become a fallback or trigger a second cleanup.
        fs::write(
            root.path().join("bot-templates.json"),
            "invalid old template source",
        )
        .unwrap();
        fs::write(root.path().join("zones.json"), "invalid old zone source").unwrap();
        for _ in 0..2 {
            let restarted = Panel::start(root.path());
            assert_eq!(
                body(get_over_tcp(restarted.addr, "/api/bot-templates").await)["templates"],
                json!([saved.clone()])
            );
            assert_eq!(
                body(
                    get_over_tcp(
                        restarted.addr,
                        &format!("/api/bot-templates/{}", saved["id"].as_str().unwrap())
                    )
                    .await
                )["template"],
                saved
            );
            assert_eq!(
                body(get_over_tcp(restarted.addr, "/api/zones").await)["zones"],
                json!([{"path": zone}])
            );
            assert_eq!(fs::read(&session).unwrap(), b"NEW SESSION\0\n");
        }
        fs::remove_file(&storage).unwrap();
        fs::remove_file(root.path().join(".data/zones.json")).unwrap();
        let restarted = Panel::start(root.path());
        assert_eq!(
            body(get_over_tcp(restarted.addr, "/api/bot-templates").await)["templates"],
            json!([])
        );
        assert_eq!(
            body(get_over_tcp(restarted.addr, "/api/zones").await)["zones"],
            json!([])
        );
        assert_eq!(
            fs::read(root.path().join("bot-templates.json")).unwrap(),
            b"invalid old template source"
        );
        assert_eq!(
            fs::read(root.path().join("zones.json")).unwrap(),
            b"invalid old zone source"
        );
        if explicit.is_some() {
            assert!(!root.path().join(".data/bot-templates.json").exists());
        }
    }
}

// P47. Real files occupy directory/write targets. These errors must be observable at
// startup/save/create, and no controlled Herdr invocation may hide a failed session allocation.
#[tokio::test]
async fn runtime_directory_and_write_failures_are_not_reported_as_success() {
    let blocked = TempDir::new("runtime blocked directory");
    let herdr = HerdrFixture::new("runtime failure calls");
    configure(blocked.path(), &herdr, None);
    fs::write(blocked.path().join(".data"), "not a directory").unwrap();
    let output = command(blocked.path()).output().unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains(".data"), "{diagnostic}");
    assert_eq!(
        fs::read(blocked.path().join(".data")).unwrap(),
        b"not a directory"
    );

    let root = TempDir::new("runtime blocked writes");
    configure(root.path(), &herdr, None);
    let panel = Panel::start(root.path());
    let zone = root.path().join("zone");
    fs::create_dir(&zone).unwrap();
    body(post_json_over_tcp(panel.addr, "/api/zones", &json!({"path": zone}).to_string()).await);
    let saved = save(&panel).await;
    fs::write(
        root.path().join(".data/bot-sessions"),
        "not a session directory",
    )
    .unwrap();
    let preview = body(
        post_json_over_tcp(
            panel.addr,
            "/api/bots/preview",
            &json!({"zone": zone, "templateId": saved["id"]}).to_string(),
        )
        .await,
    );
    let response = post_json_over_tcp(
        panel.addr,
        "/api/bots",
        &json!({
            "zone": zone, "templateId": saved["id"], "botId": preview["botId"], "input": "task"
        })
        .to_string(),
    )
    .await;
    assert_eq!(response.status, 202, "legacy native path is not a prerequisite: {}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let view = body(get_over_tcp(panel.addr, &format!("/api/bots/{}", accepted["id"].as_str().unwrap())).await);
        if view["stage"] == "succeeded" { break; }
        assert_ne!(view["stage"], "failed", "{view}");
        assert!(Instant::now() < deadline, "{view}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(fs::read(root.path().join(".data/bot-sessions")).unwrap(), b"not a session directory");

    let storage = root.path().join(".data/bot-templates.json");
    let original = fs::read(&storage).unwrap();
    let permissions = fs::metadata(&storage).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&storage, readonly).unwrap();
    let response = post_json_over_tcp(panel.addr, "/api/bot-templates", &json!({
        "name": "must fail", "botName": "new", "agent": "omp", "model": "default", "thinking": "default", "prompt": "new"
    }).to_string()).await;
    fs::set_permissions(&storage, permissions).unwrap();
    assert_eq!(response.status, 500, "{}", response.body);
    assert_eq!(fs::read(&storage).unwrap(), original);
    assert_eq!(
        body(get_over_tcp(panel.addr, "/api/bot-templates").await)["templates"],
        json!([saved])
    );
}

// P42. Git itself determines candidates using the delivered ignore file, with no add
// or commit of runtime data. A missing .data rule exposes every seeded runtime/temporary file.
#[test]
fn delivered_ignore_rules_exclude_runtime_and_temporary_files_but_not_metadata() {
    let root = TempDir::new("runtime git candidates");
    fs::write(
        root.path().join(".gitignore"),
        include_bytes!("../../.gitignore"),
    )
    .unwrap();
    let initialized = Command::new("git")
        .current_dir(root.path())
        .arg("init")
        .output()
        .unwrap();
    assert!(initialized.status.success());
    for file in [
        ".data/zones.json",
        ".data/zones.json.tmp",
        ".data/bot-templates.json",
        ".data/bot-templates.json.tmp",
        ".data/bot-sessions/new/context.jsonl",
        ".data/.runtime-layout-v1",
        "zones.json",
        "zones.json.tmp",
        "bot-templates.json",
        "bot-templates.json.tmp",
        "bot-sessions/old/context.jsonl",
        "meta-data/bot-options.toml",
    ] {
        let path = root.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "isolated literal").unwrap();
    }
    let candidates = Command::new("git")
        .current_dir(root.path())
        .args([
            "-c",
            "core.excludesFile=",
            "ls-files",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .unwrap();
    assert!(candidates.status.success());
    let text = String::from_utf8(candidates.stdout).unwrap();
    let paths: Vec<&str> = text.lines().collect();
    assert_eq!(paths, vec![".gitignore", "meta-data/bot-options.toml"]);
}

async fn snapshot(panel: &Panel) -> Value {
    let mut connection =
        common::sse::SseConnection::open(panel.addr, "/api/sessions/events", &[]).await;
    let frame = connection.next_frame().await;
    frame.named("snapshot");
    frame.json()
}

// P48. Establish real session/monitor/countdown intent and an externally blocked creation
// before stopping the process. Restart must not recover or replay any of these transient states.
#[tokio::test]
async fn real_restart_does_not_restore_registry_shutdown_intent_or_inflight_creation() {
    let root = TempDir::new("runtime transient lifecycle");
    let herdr = HerdrFixture::new("runtime transient calls");
    configure(root.path(), &herdr, None);
    let config_path = root.path().join("herdr-bot.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    // Never risk an actual machine shutdown during acceptance.
    config.push_str("\n[shutdown]\ncountdown_seconds = 4294967295\n");
    fs::write(&config_path, config).unwrap();
    let zone = root.path().join("zone");
    fs::create_dir(&zone).unwrap();
    let release = root.path().join("release-agent-start");
    herdr
        .set_behavior(json!({"agent_start_gate": {"wait_for_file": release, "timeout_ms": 30000}}));
    let panel = Panel::start(root.path());
    body(post_json_over_tcp(panel.addr, "/api/zones", &json!({"path": zone}).to_string()).await);
    let saved = save(&panel).await;
    body(
        post_json_over_tcp(
            panel.addr,
            "/api/sessions/events",
            &json!({
                "sessionId": "transient", "kind": "register", "data": {"cwd": zone}
            })
            .to_string(),
        )
        .await,
    );
    body(
        post_json_over_tcp(
            panel.addr,
            "/api/sessions/events",
            &json!({
                "sessionId": "transient", "kind": "state", "data": {"value": "working"}
            })
            .to_string(),
        )
        .await,
    );
    let monitored = body(
        post_json_over_tcp(
            panel.addr,
            "/api/shutdown/monitor",
            &json!({
                "sessionId": "transient", "selected": true
            })
            .to_string(),
        )
        .await,
    );
    assert_eq!(
        monitored["shutdown"]["monitoredSessionIds"],
        json!(["transient"])
    );
    let first = snapshot(&panel).await;
    assert_eq!(first["sessions"][0]["sessionId"], "transient");
    let started = common::post_over_tcp(panel.addr, "/api/shutdown/start").await;
    assert_eq!(started.status, 200, "{}", started.body);
    assert_eq!(snapshot(&panel).await["shutdown"]["phase"], "counting-down");
    let preview = body(
        post_json_over_tcp(
            panel.addr,
            "/api/bots/preview",
            &json!({"zone": zone, "templateId": saved["id"]}).to_string(),
        )
        .await,
    );
    let response = post_json_over_tcp(
        panel.addr,
        "/api/bots",
        &json!({
            "zone": zone, "templateId": saved["id"], "botId": preview["botId"], "input": "task"
        })
        .to_string(),
    )
    .await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).unwrap();
    assert!(
        common::wait_until(Duration::from_secs(10), || herdr
            .calls()
            .iter()
            .any(|call| call["kind"] == "agent_start"))
        .await
    );
    let creation_path = format!("/api/bots/{}", accepted["id"].as_str().unwrap());
    assert_eq!(
        body(get_over_tcp(panel.addr, &creation_path).await)["stage"],
        "starting"
    );
    drop(panel);
    fs::write(release, "release external fixture").unwrap();
    let calls = herdr.calls().len();
    let restarted = Panel::start(root.path());
    let current = snapshot(&restarted).await;
    assert_eq!(current["sessions"], json!([]));
    assert_eq!(current["shutdown"]["phase"], "none");
    assert_eq!(current["shutdown"]["monitoredSessionIds"], json!([]));
    assert_eq!(
        get_over_tcp(restarted.addr, &creation_path).await.status,
        404
    );
    assert_eq!(
        herdr.calls().len(),
        calls,
        "restart replayed external creation"
    );
    let mut files: Vec<_> = fs::read_dir(root.path().join(".data"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec![
            ".runtime-layout-v1",
            "bot-templates.json",
            "session-history.json",
            "zones.json"
        ]
    );
}

// P43/P47. A directory is not a new registry. A failed migration must not destroy
// the only registration, nor report a successfully started application.
#[test]
fn invalid_zone_migration_target_fails_without_removing_the_old_registration() {
    let root = TempDir::new("runtime invalid migration target");
    let herdr = HerdrFixture::new("runtime invalid target calls");
    configure(root.path(), &herdr, None);
    let zone = root.path().join("zone");
    fs::create_dir(&zone).unwrap();
    let registry = legacy_data(root.path(), &zone);
    fs::create_dir_all(root.path().join(".data/zones.json")).unwrap();
    let mut child = command(root.path()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "invalid target was accepted as successful startup: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("zones.json"));
    assert_eq!(fs::read(root.path().join("zones.json")).unwrap(), registry);
    assert!(!root.path().join(".data/.runtime-layout-v1").exists());
}

// P44. New and old registrations deliberately differ. A merge or overwrite must fail
// either the byte comparison or the public list, even if startup otherwise succeeds.
#[tokio::test]
async fn existing_zone_target_wins_without_merging_or_stopping_startup() {
    let root = TempDir::new("runtime existing zone target");
    let herdr = HerdrFixture::new("runtime existing target calls");
    configure(root.path(), &herdr, None);
    let old_zone = root.path().join("old zone");
    let new_zone = root.path().join("new zone");
    fs::create_dir(&old_zone).unwrap();
    fs::create_dir(&new_zone).unwrap();
    fs::write(old_zone.join("keep"), "old zone content").unwrap();
    fs::write(new_zone.join("keep"), "new zone content").unwrap();
    legacy_data(root.path(), &old_zone);
    fs::create_dir(root.path().join(".data")).unwrap();
    let target = format!(
        "{{\n  \"zones\": [{{\"path\": {}}}]\n}}\n",
        serde_json::to_string(&new_zone).unwrap()
    );
    fs::write(root.path().join(".data/zones.json"), &target).unwrap();
    let panel = Panel::start(root.path());
    assert_eq!(
        body(get_over_tcp(panel.addr, "/api/zones").await)["zones"],
        json!([{"path": new_zone}])
    );
    assert!(!root.path().join("zones.json").exists());
    assert_eq!(
        fs::read(root.path().join(".data/zones.json")).unwrap(),
        target.as_bytes()
    );
    assert_eq!(
        fs::read(old_zone.join("keep")).unwrap(),
        b"old zone content"
    );
    assert_eq!(
        fs::read(new_zone.join("keep")).unwrap(),
        b"new zone content"
    );
}

// P25 + existing zone cutover: literal registry, binary zone content, and old native
// main/sidecars are independent expectations. Violation: legacy cleanup deletes native
// files on first startup. Observe directory/byte inventory plus public zone registration.
#[tokio::test]
async fn cutover_preserves_old_native_main_and_sidecars_while_moving_zone_registration() {
    let root = TempDir::new("runtime cutover");
    let herdr = HerdrFixture::new("runtime cutover external calls");
    configure(root.path(), &herdr, None);
    let zone = root.path().join("actual zone");
    fs::create_dir(&zone).unwrap();
    let content = b"zone content\0\xff\n";
    fs::write(zone.join("payload.bin"), content).unwrap();
    let registry = legacy_data(root.path(), &zone);
    let old_native = root.path().join("bot-sessions");
    fs::create_dir_all(old_native.join("old.records/empty")).unwrap();
    fs::write(old_native.join("old.records/tool.bin"), b"\0old-record\xff").unwrap();
    let old_before = native_snapshot(&old_native);

    let panel = Panel::start(root.path());
    assert!(old_native.is_dir(), "P25: first startup must preserve old native files");
    assert_eq!(native_snapshot(&old_native), old_before);
    let zones = body(get_over_tcp(panel.addr, "/api/zones").await);
    assert_eq!(zones["zones"], json!([{"path": zone}]));
    assert_eq!(
        fs::read(root.path().join(".data/zones.json")).unwrap(),
        registry
    );
    for old in [
        "zones.json",
        "zones.json.tmp",
        "bot-templates.json",
        "bot-templates.json.tmp",
    ] {
        assert!(
            !root.path().join(old).exists(),
            "legacy source remains: {old}"
        );
    }
    assert_eq!(
        body(get_over_tcp(panel.addr, "/api/bot-templates").await)["templates"],
        json!([])
    );
    assert_eq!(fs::read(zone.join("payload.bin")).unwrap(), content);
    assert_eq!(
        fs::read(root.path().join("unrelated.txt")).unwrap(),
        b"keep unrelated bytes\0\n"
    );
    assert!(
        herdr.calls().is_empty(),
        "cutover must not control running agents"
    );
}
