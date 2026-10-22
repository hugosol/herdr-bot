//! S2（HTTP 操作 interface）接缝上的验收测试（票 03）：两个固定操作的服务端绑定与封闭请求面。
//!
//! 这些检查证明：客户端只能提交“启动”“停止”两个操作名；究竟执行哪个脚本、在哪个工作目录
//! 执行，只由服务端配置决定；其他操作名，以及携带命令、脚本路径或额外参数的请求都被拒绝，
//! 且拒绝不产生任何脚本执行副作用（用夹具自己写下的调用记录证明）。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{FixtureTree, same_path};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 把两个操作分别绑定到指定脚本与工作目录（两个操作在测试里刻意使用不同目标）。
fn config_with_operations(
    start_script: &Path,
    start_working_directory: &Path,
    stop_script: &Path,
    stop_working_directory: &Path,
) -> herdr_bot::Config {
    herdr_bot::Config {
        listen_addr: "0.0.0.0:0".to_string(),
        operations: herdr_bot::Operations {
            start: herdr_bot::OperationBinding {
                script: start_script.to_path_buf(),
                working_directory: start_working_directory.to_path_buf(),
            },
            stop: herdr_bot::OperationBinding {
                script: stop_script.to_path_buf(),
                working_directory: stop_working_directory.to_path_buf(),
            },
        },
        mdns: herdr_bot::MdnsConfig::default(),
        shutdown: herdr_bot::ShutdownConfig::default(),
        sessions: herdr_bot::SessionsConfig::default(),
        zones: herdr_bot::ZonesConfig::default(),
        bot_templates: herdr_bot::BotTemplatesConfig::default(),
        herdr: herdr_bot::HerdrConfig::new("bot-playground"),
        git: herdr_bot::GitConfig::default(),
    }
}

/// 发一个任意请求，返回状态码与原始响应体。
async fn send(app: axum::Router, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = app.oneshot(request).await.expect("调用操作接口");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, bytes.to_vec())
}

/// 发一个请求，并要求响应体是 JSON。
async fn send_json(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let (status, bytes) = send(app, request).await;
    let body = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        panic!(
            "拒绝必须以 JSON 给出可理解的结果类别（HTTP {status}）：{error}；原始响应体：{}",
            String::from_utf8_lossy(&bytes)
        )
    });
    (status, body)
}

/// 裸 POST：没有请求体，也没有查询串。
fn bare_post(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .expect("构造请求")
}

/// 取夹具目录里两个不同批处理的绝对路径。
fn scripts(tree: &FixtureTree) -> (PathBuf, PathBuf) {
    (tree.script("success.bat"), tree.script("failure.bat"))
}

/// 两个固定操作各自调用自己绑定的脚本，并使用自己配置的工作目录。
#[tokio::test]
async fn two_operations_invoke_their_own_bound_scripts_with_their_configured_working_directories() {
    let tree = FixtureTree::new("two-operations");
    let start_directory = tree.root().join("working directories").join("start");
    let stop_directory = tree.root().join("working directories").join("stop");
    fs::create_dir_all(&start_directory).expect("创建启动工作目录");
    fs::create_dir_all(&stop_directory).expect("创建停止工作目录");
    let (start_script, stop_script) = scripts(&tree);

    let app = herdr_bot::app(&config_with_operations(
        &start_script,
        &start_directory,
        &stop_script,
        &stop_directory,
    ));

    let (status, body) = common::post_operation(app.clone(), "start").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["operation"], "start");
    assert_eq!(body["started"], true);
    assert_eq!(
        body["exitCode"], 0,
        "“启动”调用的是 success.bat，独立给定的退出码是 0：{body}"
    );

    let records = tree.records();
    assert_eq!(records.len(), 1, "“启动”只调用它自己绑定的脚本：{records:#?}");
    assert_eq!(records[0].fixture, "success", "“启动”必须调用配置绑定的启动脚本");
    assert_eq!(records[0].stage, "invoke");
    assert!(
        same_path(&records[0].cwd, &start_directory),
        "启动脚本必须以配置的工作目录运行：实际 {}，配置 {}",
        records[0].cwd.display(),
        start_directory.display()
    );

    let (status, body) = common::post_operation(app, "stop").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["operation"], "stop");
    assert_eq!(body["started"], true);
    assert_eq!(
        body["exitCode"], 3,
        "“停止”调用的是 failure.bat，独立给定的退出码是 3：{body}"
    );

    let records = tree.records();
    assert_eq!(
        records.len(),
        2,
        "“停止”只新增它自己绑定的脚本这一次调用：{records:#?}"
    );
    assert_eq!(records[1].fixture, "failure", "“停止”必须调用配置绑定的关闭脚本");
    assert_eq!(records[1].stage, "invoke");
    assert!(
        same_path(&records[1].cwd, &stop_directory),
        "关闭脚本必须以配置的工作目录运行：实际 {}，配置 {}",
        records[1].cwd.display(),
        stop_directory.display()
    );

    // 两个操作互不影响：启动那一次调用的记录保持原样。
    assert_eq!(records[0].fixture, "success");
    assert_eq!(
        (records[0].call, records[1].call),
        (1, 2),
        "两次操作是两条独立的调用记录：{records:#?}"
    );
}

/// 交付默认配置把“启动”“停止”绑定到规格 Further Notes 的现有批处理，工作目录为它们所在目录。
#[tokio::test]
async fn default_config_binds_the_specified_start_and_shutdown_batches_in_their_directory() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("herdr-bot.toml");
    let config = herdr_bot::Config::load(&config_path).expect("读取交付默认配置");

    let streaming = Path::new(r"E:\Developer\myhub\my-streaming");
    let start_script = streaming.join("start-all.bat");
    let stop_script = streaming.join("shutdown-all.bat");

    assert!(
        same_path(&config.operations.start.script, &start_script),
        "默认配置的启动操作必须是规格指定的启动批处理：{}",
        config.operations.start.script.display()
    );
    assert!(
        same_path(&config.operations.stop.script, &stop_script),
        "默认配置的停止操作必须是规格指定的关闭批处理：{}",
        config.operations.stop.script.display()
    );
    assert!(
        !same_path(&config.operations.start.script, &config.operations.stop.script),
        "两个按钮必须对应两个不同的固定脚本"
    );
    assert!(
        same_path(&config.operations.start.working_directory, streaming),
        "启动操作的执行工作目录必须是脚本所在目录：{}",
        config.operations.start.working_directory.display()
    );
    assert!(
        same_path(&config.operations.stop.working_directory, streaming),
        "停止操作的执行工作目录必须是脚本所在目录：{}",
        config.operations.stop.working_directory.display()
    );
    assert!(
        same_path(
            &config.operations.start.working_directory,
            config.operations.start.script.parent().expect("启动脚本有父目录")
        ),
        "工作目录应等于启动脚本所在目录"
    );
    assert!(
        same_path(
            &config.operations.stop.working_directory,
            config.operations.stop.script.parent().expect("关闭脚本有父目录")
        ),
        "工作目录应等于关闭脚本所在目录"
    );
}

/// 未知操作名与怪路径被拒绝，返回可理解的类别，且不执行任何脚本。
#[tokio::test]
async fn unknown_operation_names_are_rejected_with_a_category_and_without_running_any_script() {
    let tree = FixtureTree::new("unknown-operation");
    let (start_script, stop_script) = scripts(&tree);
    let app = herdr_bot::app(&config_with_operations(
        &start_script,
        &tree.operations(),
        &stop_script,
        &tree.operations(),
    ));

    for path in [
        "/api/operations/restart",
        "/api/operations/status",
        "/api/operations/start/extra",
        "/api/operations/start/",
        "/api/operations/START",
        "/api/operations/%73tart",
        "/api/operations/../start",
        "/api/operations/start/../stop/extra",
        "/api/operations",
    ] {
        let (status, body) = send_json(app.clone(), bare_post(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path} 必须被明确拒绝：{body}");
        assert_eq!(body["code"], "unknown-operation", "{path}：{body}");
        assert!(body["message"].is_string(), "{path} 应给出可读说明：{body}");
    }

    for path in ["/api/command", "/api/shell"] {
        let (status, body) = send_json(app.clone(), bare_post(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path} 必须被明确拒绝：{body}");
        assert_eq!(body["code"], "unknown-endpoint", "{path}：{body}");
    }

    assert!(
        tree.records().is_empty(),
        "被拒绝的请求不得执行任何脚本：{:#?}",
        tree.records()
    );

    // 正对照：同一个测试里，合法的裸请求确实产生一条调用记录。
    let (status, body) = common::post_operation(app, "start").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let records = tree.records();
    assert_eq!(records.len(), 1, "合法请求必须真的执行一次：{records:#?}");
    assert_eq!(records[0].fixture, "success");
}

/// 携带命令、脚本路径或额外参数的请求被拒绝，且不产生任何脚本调用；两种操作都如此。
#[tokio::test]
async fn parameter_carrying_requests_are_rejected_without_running_any_script() {
    let tree = FixtureTree::new("override-attempts");
    let (start_script, stop_script) = scripts(&tree);
    let app = herdr_bot::app(&config_with_operations(
        &start_script,
        &tree.operations(),
        &stop_script,
        &tree.operations(),
    ));

    let other_script = stop_script.display().to_string();
    let mut attempts = Vec::new();
    for operation in ["start", "stop"] {
        let uri = format!("/api/operations/{operation}");
        // 请求体里点名别的脚本 / 命令 / 路径 / 参数 / 工作目录
        for body in [
            json!({ "script": other_script }),
            json!({ "command": "cmd /c echo evil" }),
            json!({ "path": other_script }),
            json!({ "args": ["-x"] }),
            json!({ "working_directory": "C:\\Windows" }),
        ] {
            attempts.push(
                Request::builder()
                    .method("POST")
                    .uri(&uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("构造请求"),
            );
        }
        // 非 JSON 的裸命令体
        attempts.push(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .body(Body::from("cmd /c echo evil"))
                .expect("构造请求"),
        );
        // 大于读取上限的请求体：同样是越界请求
        attempts.push(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .body(Body::from(vec![b'x'; 64 * 1024]))
                .expect("构造请求"),
        );
        // 查询串里点名同样的东西
        for query in [
            "script=other.bat",
            "command=cmd",
            "path=other.bat",
            "args=-x",
            "working_directory=C%3A%2FWindows",
        ] {
            attempts.push(bare_post(&format!("{uri}?{query}")));
        }
    }

    for request in attempts {
        let uri = request.uri().to_string();
        let (status, body) = send_json(app.clone(), request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} 必须被明确拒绝：{body}");
        assert_eq!(body["code"], "unexpected-parameters", "{uri}：{body}");
    }
    assert!(
        tree.records().is_empty(),
        "被拒绝的请求不得执行任何脚本，尤其不得执行请求里点名的脚本：{:#?}",
        tree.records()
    );

    // 正对照：同一个测试里，两个操作各自仍然只执行服务端配置的脚本。
    let (status, body) = common::post_operation(app.clone(), "start").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let (status, body) = common::post_operation(app, "stop").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let records = tree.records();
    let fixtures: Vec<&str> = records.iter().map(|record| record.fixture.as_str()).collect();
    assert_eq!(
        fixtures,
        vec!["success", "failure"],
        "执行目标只能来自服务端配置：{records:#?}"
    );
}

/// 非 POST 的方法不执行任何脚本：只有 POST 才是提交固定操作的方式。
#[tokio::test]
async fn non_post_methods_never_run_the_bound_scripts() {
    let tree = FixtureTree::new("non-post");
    let (start_script, stop_script) = scripts(&tree);
    let app = herdr_bot::app(&config_with_operations(
        &start_script,
        &tree.operations(),
        &stop_script,
        &tree.operations(),
    ));

    for (method, path) in [
        ("GET", "/api/operations/start"),
        ("PUT", "/api/operations/start"),
        ("DELETE", "/api/operations/stop"),
        ("GET", "/api/operations/stop"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .expect("构造请求");
        let (status, _) = send(app.clone(), request).await;
        assert!(
            status.is_client_error(),
            "{method} {path} 必须以 4xx 拒绝，实际 {status}"
        );
    }

    assert!(
        tree.records().is_empty(),
        "非 POST 请求不得执行任何脚本：{:#?}",
        tree.records()
    );

    // 正对照：POST 仍然是可用的提交方式。
    let (status, body) = common::post_operation(app, "start").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(tree.records().len(), 1);
}
