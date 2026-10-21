//! P41：目标 Herdr Workspace 的启动期配置。
//!
//! 这里用**真实 TOML 输入**启动交付二进制：缺失、空串与纯空白都以配置错误退出；合法显式值
//! 真正开始监听，并在启动日志中报告这次显式目标。没有代码缺省值可以掩盖缺失。

mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::TempDir;

const STARTUP_DEADLINE: Duration = Duration::from_secs(20);

/// 一份除 `[herdr]` 外可启动的配置；`herdr_section` 为空表示整段缺失。
fn write_config(dir: &TempDir, herdr_section: &str) -> PathBuf {
    let path = dir.path().join("herdr-bot.toml");
    std::fs::write(
        &path,
        format!(
            "listen_addr = \"127.0.0.1:0\"\n\n\
             [operations.start]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
             [operations.stop]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
             [mdns]\nenabled = false\n\n\
             {herdr_section}"
        ),
    )
    .expect("写临时配置");
    path
}

/// 启动失败时进程不进入监听；stderr 必须说明问题出在 Herdr Workspace 配置上。
fn assert_startup_rejected(label: &str, section: &str) {
    let dir = TempDir::new(&format!("herdr config {label}"));
    let config_path = write_config(&dir, section);
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
        .arg("--config")
        .arg(&config_path)
        .stdin(Stdio::null())
        .output()
        .expect("启动真实二进制");
    assert!(
        !output.status.success(),
        "{label}：目标 Workspace 配置不合法时必须启动失败"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("herdr") || stderr.contains("workspace_name"),
        "{label}：启动错误必须指出 Herdr Workspace 配置，实际 stderr：{stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("监听："),
        "{label}：配置失败时不得进入监听"
    );
}

#[test]
fn missing_empty_or_whitespace_workspace_name_prevents_startup() {
    assert_startup_rejected("missing", "");
    assert_startup_rejected("empty", "[herdr]\nworkspace_name = ''\n");
    assert_startup_rejected("blank", "[herdr]\nworkspace_name = '   '\n");
    assert_startup_rejected("tabs", "[herdr]\nworkspace_name = \"\\t\\n\"\n");
}

/// 合法显式值启动成功；日志说明这次目标不是代码缺省，而是 TOML 中的字面值。
#[test]
fn an_explicit_workspace_name_starts_the_real_process() {
    let static_index = Path::new(env!("CARGO_MANIFEST_DIR")).join("static/index.html");
    assert!(
        static_index.is_file(),
        "运行进程级启动检查前需先构建前端（缺少 {}）。",
        static_index.display()
    );

    let dir = TempDir::new("herdr config valid");
    let config_path = write_config(
        &dir,
        "[herdr]\nworkspace_name = 'explicit-target'\nexecutable = 'herdr'\n",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
        .arg("--config")
        .arg(&config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动真实二进制");

    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + STARTUP_DEADLINE;
    let mut log = Vec::new();
    let mut started = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                log.push(line.clone());
                if line.contains("监听：http://127.0.0.1:") {
                    started = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let _guard = KillOnDrop(&mut child);
    let joined = log.join("\n");
    assert!(started, "合法显式配置必须真正开始监听：\n{joined}");
    assert!(
        joined.contains("Herdr Workspace：explicit-target"),
        "启动日志必须报告 TOML 中显式配置的目标，而不是缺省值：\n{joined}"
    );
}

/// 测试失败或提前返回时也结束真实进程。
struct KillOnDrop<'a>(&'a mut Child);

impl Drop for KillOnDrop<'_> {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
