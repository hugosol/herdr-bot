//! S2（HTTP 操作 interface）接缝上的验收测试：经 HTTP interface 请求“启动”操作，
//! 观察服务端配置的启动批处理是否以配置工作目录被真实执行，以及调用结束事实是否回传。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

/// 隔离的临时目录；路径中带空格，用于覆盖 Windows 命令行引号处理的真实情况。
struct TempDir(PathBuf);

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl TempDir {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join("omp bot test fixtures")
            .join(format!("{label} {}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 写一个真实批处理文件（CRLF 行尾），内容由测试完全控制。
fn write_batch(path: &Path, lines: &[String]) {
    let mut content = String::from("@echo off\r\n");
    for line in lines {
        content.push_str(line);
        content.push_str("\r\n");
    }
    fs::write(path, content).expect("write batch file");
}

fn config(start_script: &Path, working_directory: &Path) -> omp_bot::Config {
    let binding = || omp_bot::OperationBinding {
        script: start_script.to_path_buf(),
        working_directory: working_directory.to_path_buf(),
    };
    omp_bot::Config {
        listen_addr: "0.0.0.0:0".to_string(),
        operations: omp_bot::Operations {
            start: binding(),
            stop: binding(),
        },
        mdns: omp_bot::MdnsConfig::default(),
    }
}

async fn post_start(app: axum::Router) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/api/operations/start")
        .body(Body::empty())
        .expect("build request");
    let response = app.oneshot(request).await.expect("call start operation");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let json = serde_json::from_slice(&bytes).expect("response body is JSON");
    (status, json)
}

#[tokio::test]
async fn start_operation_runs_configured_batch_with_configured_working_directory() {
    let dir = TempDir::new("start-cwd");
    let working_directory = dir.path().join("script working dir");
    fs::create_dir_all(&working_directory).expect("create working directory");
    let marker = dir.path().join("observed cwd.txt");
    let script = dir.path().join("record cwd.bat");
    write_batch(
        &script,
        &[
            format!("echo %CD%>\"{}\"", marker.display()),
            "exit /b 0".to_string(),
        ],
    );

    let app = omp_bot::app(&config(&script, &working_directory));
    let (status, body) = post_start(app).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["operation"], "start");
    assert_eq!(body["ended"], true);
    assert_eq!(body["started"], true);
    assert_eq!(body["exitCode"], 0);

    let observed =
        fs::read_to_string(&marker).expect("批处理必须被真实执行并按配置的工作目录写出标记文件");
    assert_eq!(
        observed.trim(),
        working_directory.display().to_string(),
        "批处理必须以配置的工作目录作为当前目录运行"
    );
}

#[tokio::test]
async fn start_response_arrives_only_after_script_process_ends() {
    let dir = TempDir::new("start-wait");
    let marker = dir.path().join("script finished.txt");
    let script = dir.path().join("slow.bat");
    // 脚本先等待约 2 秒再写出结束标记，并以退出码 7 结束。
    write_batch(
        &script,
        &[
            "ping -n 3 127.0.0.1 >nul".to_string(),
            format!("echo finished>\"{}\"", marker.display()),
            "exit /b 7".to_string(),
        ],
    );

    let app = omp_bot::app(&config(&script, dir.path()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(
        fs::read_to_string(&marker).ok().as_deref().map(str::trim),
        Some("finished"),
        "响应返回时脚本进程必须已经结束"
    );
    assert_eq!(body["exitCode"], 7, "退出码必须来自脚本进程的结束结果");
}

#[tokio::test]
async fn start_operation_target_comes_from_server_config() {
    let first = TempDir::new("config-first");
    let second = TempDir::new("config-second");
    let first_marker = first.path().join("first.txt");
    let second_marker = second.path().join("second.txt");

    let first_script = first.path().join("first.bat");
    write_batch(
        &first_script,
        &[
            format!("echo first>\"{}\"", first_marker.display()),
            "exit /b 0".to_string(),
        ],
    );
    let second_script = second.path().join("second.bat");
    write_batch(
        &second_script,
        &[
            format!("echo second>\"{}\"", second_marker.display()),
            "exit /b 0".to_string(),
        ],
    );

    let (status, _) = post_start(omp_bot::app(&config(&first_script, first.path()))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first_marker.is_file(), "配置的第一个脚本必须被执行");
    assert!(
        !second_marker.exists(),
        "未被配置的脚本不得被执行（目标来自服务端配置）"
    );

    let (status, _) = post_start(omp_bot::app(&config(&second_script, second.path()))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(second_marker.is_file(), "配置的第二个脚本必须被执行");
}

#[tokio::test]
async fn start_call_invalid_working_directory_reports_finished_call_without_starting() {
    let dir = TempDir::new("missing-workdir");
    let script = dir.path().join("exists.bat");
    write_batch(&script, &["exit /b 0".to_string()]);
    let missing_directory = dir.path().join("no such directory");

    let app = omp_bot::app(&config(&script, &missing_directory));
    let (status, body) = post_start(app).await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(body["started"], false, "工作目录不存在时脚本进程无法创建");
    assert_eq!(body["exitCode"], serde_json::Value::Null);
}
