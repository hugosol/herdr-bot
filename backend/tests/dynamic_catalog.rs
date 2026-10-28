//! Approved seams S2/S3: real HTTP and real metadata files, not private parsing APIs.
mod common;
use common::{TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn configuration(root: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir().join("operations");
    herdr_bot::Config::from_toml(&format!(
        r#"
listen_addr = "127.0.0.1:0"
[operations.start]
script = '{}'
working_directory = '{}'
[operations.stop]
script = '{}'
working_directory = '{}'
[herdr]
workspace_name = 'catalog-test'
[tasks]
storage_file = '{}'
metadata_file = '{}'
"#,
        fixtures.join("success.bat").display(),
        fixtures.display(),
        fixtures.join("success.bat").display(),
        fixtures.display(),
        root.join("tasks.json").display(),
        root.join("options.toml").display()
    ))
    .unwrap()
}

// P31: real executable must fail before serving; no private parser-only assertion.
// A plausible violation is startup ignoring malformed metadata while HTTP loads it later.
#[test]
fn startup_rejects_missing_unreadable_syntax_duplicate_and_contradictory_metadata() {
    let dir = TempDir::new("catalog startup rejection");
    let metadata = dir.path().join("options.toml");
    let config = dir.path().join("herdr-bot.toml");
    let fixtures = common::repo_fixtures_dir().join("operations");
    fs::write(
        &config,
        format!(
            r#"
listen_addr = '127.0.0.1:0'
[mdns]
enabled = false
[operations.start]
script = '{}'
working_directory = '{}'
[operations.stop]
script = '{}'
working_directory = '{}'
[herdr]
workspace_name = 'startup-catalog'
[tasks]
metadata_file = '{}'
"#,
            fixtures.join("success.bat").display(),
            fixtures.display(),
            fixtures.join("success.bat").display(),
            fixtures.display(),
            metadata.display()
        ),
    )
    .unwrap();
    for (name, content, reason) in [
        ("missing", None, "读取失败"),
        ("syntax", Some("[[models".to_owned()), "TOML"),
        (
            "duplicate",
            Some(format!(
                "{CATALOG}\n[[models]]\nid='lab-model'\nlabel='重复'\nprovider_model='other/model'\nthinking={{omp=[]}}\n"
            )),
            "重复标识",
        ),
        (
            "contradiction",
            Some(CATALOG.replace("omp = ['low', 'high']", "ghost = ['high']")),
            "未知 agent",
        ),
    ] {
        if let Some(content) = content {
            fs::write(&metadata, content).unwrap();
        }
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            panic!("{name}: invalid metadata unexpectedly served");
        }
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "{name}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(&metadata.display().to_string()) && error.contains(reason),
            "{name}: {error}"
        );
    }
    // Windows sharing denial is a real unreadable file, even for an administrator.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        fs::write(&metadata, CATALOG).unwrap();
        let _exclusive = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&metadata)
            .unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .arg("--config")
            .arg(&config)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("读取失败") && error.contains(&metadata.display().to_string()),
            "{error}"
        );
    }
    assert!(metadata.exists());
    assert!(
        !dir.path().join("meta-data").exists(),
        "no replacement catalog generated"
    );
}

// P40 (metadata layout): use the actual delivered configuration, changing only network settings.
// A private path helper or a catalog in the process root cannot satisfy this observation.
#[tokio::test]
async fn delivered_configuration_reads_the_independent_metadata_directory_in_a_real_process() {
    use std::io::{BufRead, BufReader};
    let dir = TempDir::new("delivered metadata layout");
    fs::create_dir(dir.path().join("meta-data")).unwrap();
    fs::write(dir.path().join("meta-data/task-options.toml"), CATALOG).unwrap();
    // A decoy beside the config must not be used.
    fs::write(dir.path().join("chat-options.toml"), "not TOML").unwrap();
    let root = common::repo_fixtures_dir().parent().unwrap().to_path_buf();
    let delivered = fs::read_to_string(root.join("herdr-bot.toml")).unwrap();
    let config_path = dir.path().join("herdr-bot.toml");
    let defaulted = delivered.replace("metadata_file = 'meta-data/task-options.toml'", "");
    for shape in [&delivered, &defaulted] {
        fs::write(
            &config_path,
            format!(
                "{}\n[mdns]\nenabled = false\n",
                shape.replace("0.0.0.0:18787", "127.0.0.1:0")
            ),
        )
        .unwrap();
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .arg("--config")
            .arg(&config_path)
            .current_dir(dir.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(port) = line
                    .split("http://127.0.0.1:")
                    .nth(1)
                    .and_then(|v| v.trim_end_matches('/').parse::<u16>().ok())
                {
                    let _ = tx.send(port);
                    // Keep the pipe open and drain remaining startup diagnostics:
                    // the server still writes to stdout after announcing its port.
                }
            }
        });
        let received = rx.recv_timeout(std::time::Duration::from_secs(10));
        if received.is_err() {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "real program failed to serve: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let port = received.unwrap();
        let response = get_over_tcp(([127, 0, 0, 1], port).into(), "/api/tasks").await;
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(response.status, 200, "{}", response.body);
        let body: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(body["options"]["models"], json!(["default", "lab-model"]));
        assert_eq!(body["options"]["labels"]["models"]["lab-model"], "实验模型");
    }
}

const CATALOG: &str = r#"
[[agents]]
id = 'omp'
label = 'OMP'
[[agents]]
id = 'pi'
label = 'Pi'
[[models]]
id = 'default'
label = '沿用 Agent 默认值'
[[models]]
id = 'lab-model'
label = '实验模型'
provider_model = 'lab-provider/first-model'
thinking = { omp = ['low', 'high'], pi = ['off', 'high'] }
[[skills]]
id = 'lab-skill'
label = '实验 Skill'
native_name = 'lab-native'
[[extensions]]
id = 'omp-cost'
label = 'model-cost'
agent = 'omp'
kind = 'fixed'
entrypoint = 'C:/catalog/omp-cost.ts'
[[extensions]]
id = 'pi-cost'
label = 'model-cost'
agent = 'pi'
kind = 'fixed'
entrypoint = 'C:/catalog/pi-cost.ts'
"#;

fn payload(thinking: &str) -> String {
    json!({"name":"动态样例", "chatName":"{zone.name}-{chat.id}", "agent":"omp", "model":"lab-model", "thinking":thinking, "skill":"lab-skill", "prompt":"第一行\n第二行"}).to_string()
}

// P25/P26: independent literals distinguish current metadata from compiled/cached choices.
// Plausible violation: options reload but saving still uses the old compiled model/ability list.
#[tokio::test]
async fn current_metadata_drives_public_options_save_validation_and_stable_storage() {
    let dir = TempDir::new("dynamic catalog");
    let metadata = dir.path().join("options.toml");
    fs::write(&metadata, CATALOG).unwrap();
    let server = TestServer::start(&configuration(dir.path())).await;
    let response = get_over_tcp(server.addr(), "/api/tasks").await;
    assert_eq!(response.status, 200, "{}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["options"]["models"], json!(["default", "lab-model"]));
    assert_eq!(
        body["options"]["thinking"]["omp"]["lab-model"],
        json!(["low", "high"])
    );
    let saved = post_json_over_tcp(server.addr(), "/api/tasks", &payload("high")).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let stored: Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("tasks.json")).unwrap())
            .unwrap();
    let task = &stored["tasks"][0];
    assert_eq!(task["model"], "lab-model");
    assert_eq!(task["skill"], "lab-skill");
    assert_eq!(task["prompt"], "第一行\n第二行");
    assert_eq!(task["extensions"], json!([]));
    assert_eq!(task["taskType"], "normal");
    assert_eq!(
        task.as_object().unwrap().len(),
        10,
        "persist stable fields only"
    );
    fs::write(
        &metadata,
        CATALOG
            .replace("['low', 'high']", "['low']")
            .replace("实验模型", "新版模型"),
    )
    .unwrap();
    let rejected = post_json_over_tcp(server.addr(), "/api/tasks", &payload("high")).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    let accepted = post_json_over_tcp(server.addr(), "/api/tasks", &payload("low")).await;
    assert_eq!(accepted.status, 200, "{}", accepted.body);
    let body: Value = serde_json::from_str(&accepted.body).unwrap();
    assert_eq!(
        body["options"]["thinking"]["omp"]["lab-model"],
        json!(["low"])
    );
}
