//! S2（HTTP 操作 interface）接缝上的验收测试（票 04）：执行结果分类。
//!
//! 三类结果必须能经 HTTP interface 直接观察并互相区分：“脚本无法启动”“脚本成功退出”
//! “脚本失败退出”。期望来自夹具独立给定的情形（`fixtures/README.md` 声明
//! `success.bat` 以 0 结束、`failure.bat` 以 3 结束、`operations/unstartable/` 的三个变体
//! 都不会运行起来），这里不复算后端的分类逻辑，只断言独立给定的类别与退出结果。
//!
//! 结果类别是响应里的可观察字段：`outcome`（`completed` / `failed`）与 `failure`
//! （`not-started` / `failed-exit`，仅失败时出现）。字段命名见根目录 `README.md`。

mod common;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{FixtureTree, config, post_operation, post_start};
use serde_json::Value;

/// 写一个真实批处理文件（CRLF 行尾），内容由测试完全控制。
fn write_batch(path: &Path, lines: &[String]) {
    let mut content = String::from("@echo off\r\n");
    for line in lines {
        content.push_str(line);
        content.push_str("\r\n");
    }
    fs::write(path, content).expect("写批处理文件");
}

/// 从响应体读出结果类别：`outcome` 与可选的失败子类别 `failure`。
fn class(body: &Value) -> (String, Option<String>) {
    (
        body["outcome"]
            .as_str()
            .unwrap_or_else(|| panic!("响应必须带结果类别字段 outcome：{body}"))
            .to_string(),
        body["failure"].as_str().map(str::to_string),
    )
}

/// 明确成功退出（夹具 `success.bat`，退出码 0）得到“执行完成”结果。
#[tokio::test]
async fn success_exit_is_reported_as_completed() {
    let tree = FixtureTree::new("result-success");
    let app = herdr_bot::app(&config(&tree.script("success.bat"), &tree.operations()));

    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["ended"], true);
    assert_eq!(
        body["exitCode"], 0,
        "夹具 success.bat 以退出码 0 结束（fixtures/README.md）"
    );
    assert_eq!(
        class(&body),
        ("completed".to_string(), None),
        "成功退出必须被报告为执行完成，且不带失败子类别：{body}"
    );
}

/// 明确失败退出（夹具 `failure.bat`，退出码 3）得到“执行失败”结果。
#[tokio::test]
async fn failure_exit_is_reported_as_failed() {
    let tree = FixtureTree::new("result-failure");
    let app = herdr_bot::app(&config(&tree.script("failure.bat"), &tree.operations()));

    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(
        body["exitCode"], 3,
        "夹具 failure.bat 以退出码 3 结束（fixtures/README.md）"
    );
    assert_eq!(
        class(&body),
        ("failed".to_string(), Some("failed-exit".to_string())),
        "失败退出必须被报告为执行失败，并与“无法启动”区分为不同子类别：{body}"
    );
}

/// “无法启动”之一：配置的目标脚本不存在。
#[tokio::test]
async fn missing_script_is_reported_as_failed_not_started() {
    let tree = FixtureTree::new("result-missing-script");
    let missing = tree.script("unstartable/missing-script.bat");
    assert!(!missing.exists(), "夹具前提：该目标确实不存在");

    let app = herdr_bot::app(&config(&missing, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(
        class(&body),
        ("failed".to_string(), Some("not-started".to_string())),
        "目标不存在时必须报告为执行失败（无法启动）：{body}"
    );
    assert_eq!(
        body["started"], false,
        "脚本进程没有创建，不得报告为已启动：{body}"
    );
    assert_eq!(
        tree.records().len(),
        0,
        "目标不存在时不得写入任何调用记录（夹具没有被执行）"
    );
}

/// “无法启动”之二：配置的目标存在但不是可执行文件。
#[tokio::test]
async fn non_executable_target_is_reported_as_failed_not_started() {
    let tree = FixtureTree::new("result-invalid-target");
    let target = tree.script("unstartable/invalid-target.exe");
    assert!(target.is_file(), "夹具前提：该目标文件存在");

    let app = herdr_bot::app(&config(&target, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(
        class(&body),
        ("failed".to_string(), Some("not-started".to_string())),
        "目标不可执行时必须报告为执行失败（无法启动）：{body}"
    );
    assert_eq!(
        body["started"], false,
        "目标不可执行时没有创建脚本进程，不得报告为已启动：{body}"
    );
    assert_eq!(
        tree.records().len(),
        0,
        "不可执行的目标不会运行，因此不得写入调用记录"
    );
}

/// “无法启动”之三：配置的工作目录不存在。
#[tokio::test]
async fn missing_working_directory_is_reported_as_failed_not_started() {
    let tree = FixtureTree::new("result-missing-workdir");
    let missing_directory = tree.root().join("unstartable/missing-working-directory");
    assert!(
        !missing_directory.exists(),
        "夹具前提：该工作目录确实不存在"
    );

    let app = herdr_bot::app(&config(&tree.script("success.bat"), &missing_directory));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(
        class(&body),
        ("failed".to_string(), Some("not-started".to_string())),
        "工作目录不存在时进程无法创建，必须报告为执行失败（无法启动）：{body}"
    );
    assert_eq!(tree.records().len(), 0, "脚本没有运行，不得写入调用记录");
}

/// 分离长期子进程的夹具：操作在**脚本退出时**即报告执行完成，不等该长期进程结束。
#[tokio::test(flavor = "multi_thread")]
async fn detached_child_does_not_delay_the_completed_result() {
    let tree = FixtureTree::new("result-detached");
    let app = herdr_bot::app(&config(&tree.script("spawn-detached.bat"), &tree.operations()));

    let started_at = Instant::now();
    let (status, body) = tokio::time::timeout(Duration::from_secs(30), post_start(app))
        .await
        .expect("分离子进程长期运行，父夹具的调用必须立即返回");
    let elapsed = started_at.elapsed();

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(
        class(&body),
        ("completed".to_string(), None),
        "父夹具以 0 结束后操作即报告执行完成，不受分离子进程影响：{body}"
    );
    assert_eq!(body["exitCode"], 0, "父夹具以 0 结束（fixtures/README.md）");
    assert!(
        elapsed < Duration::from_secs(15),
        "完成判定不得等待分离子进程（耗时 {elapsed:?}）"
    );

    // 父调用已经报告完成，而它另开的长期进程仍在运行。
    let registered = common::wait_until(Duration::from_secs(20), || {
        tree.records()
            .iter()
            .any(|record| record.fixture == "detached-child")
    })
    .await;
    assert!(registered, "分离子进程必须留下登记记录：{:#?}", tree.records());
    let child = tree
        .records()
        .into_iter()
        .find(|record| record.fixture == "detached-child")
        .expect("子进程登记记录");
    assert!(
        common::process_is_alive(child.pid),
        "报告执行完成时分离子进程必须仍在运行（PID {}）",
        child.pid
    );

    // 验证后按夹具的结束入口清理，不遗留进程。
    let stop_output = tree.stop_fixtures();
    assert!(
        stop_output.status.success(),
        "结束脚本必须成功退出：{stop_output:?}"
    );
    assert!(
        common::wait_until(Duration::from_secs(15), || !common::process_is_alive(
            child.pid
        ))
        .await,
        "分离子进程必须被结束（PID {}）",
        child.pid
    );
}

/// 三类结果经 HTTP interface 互相区分：两类失败必须与完成不同，且彼此可区分。
#[tokio::test]
async fn the_three_result_classes_are_mutually_distinguishable() {
    let tree = FixtureTree::new("result-classes");
    let operations = tree.operations();

    let (_, completed) = post_start(herdr_bot::app(&config(
        &tree.script("success.bat"),
        &operations,
    )))
    .await;
    let (_, failed) = post_start(herdr_bot::app(&config(
        &tree.script("failure.bat"),
        &operations,
    )))
    .await;
    let (_, not_started) = post_start(herdr_bot::app(&config(
        &tree.script("unstartable/missing-script.bat"),
        &operations,
    )))
    .await;

    let completed = class(&completed);
    let failed = class(&failed);
    let not_started = class(&not_started);

    assert_eq!(completed, ("completed".to_string(), None));
    assert_eq!(failed.0, "failed", "失败退出是执行失败：{failed:?}");
    assert_eq!(not_started.0, "failed", "无法启动是执行失败：{not_started:?}");
    assert_ne!(
        failed, not_started,
        "“脚本失败退出”与“脚本无法启动”必须能互相区分"
    );
    assert_ne!(completed, failed, "完成与失败必须能互相区分");
    assert_ne!(completed, not_started, "完成与无法启动必须能互相区分");
}

/// 脚本输出不参与判定：看起来像失败输出的脚本成功退出时仍报告完成。
#[tokio::test]
async fn failure_looking_output_does_not_change_a_success_exit() {
    let tree = FixtureTree::new("result-output-success");
    let script = tree.root().join("noisy success.bat");
    write_batch(
        &script,
        &[
            "echo ERROR: script failed, service is not running".to_string(),
            "echo 执行失败 1>&2".to_string(),
            "exit /b 0".to_string(),
        ],
    );

    let app = herdr_bot::app(&config(&script, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["exitCode"], 0);
    assert_eq!(
        class(&body),
        ("completed".to_string(), None),
        "判定只看脚本进程的启动与退出结果，不看它打印了什么：{body}"
    );
}

/// 脚本输出不参与判定：看起来像成功输出的脚本失败退出时仍报告失败。
#[tokio::test]
async fn success_looking_output_does_not_change_a_failure_exit() {
    let tree = FixtureTree::new("result-output-failure");
    let script = tree.root().join("noisy failure.bat");
    write_batch(
        &script,
        &[
            "echo SUCCESS: 脚本执行完成，后台服务已就绪".to_string(),
            "echo operation completed".to_string(),
            "exit /b 4".to_string(),
        ],
    );

    let app = herdr_bot::app(&config(&script, &tree.operations()));
    let (status, body) = post_start(app).await;

    assert_eq!(status, axum::http::StatusCode::OK, "body: {body}");
    assert_eq!(body["exitCode"], 4, "退出码来自脚本进程的结束结果");
    assert_eq!(
        class(&body),
        ("failed".to_string(), Some("failed-exit".to_string())),
        "打印得像成功不改变失败退出的判定：{body}"
    );
}

/// 两个固定操作共用同一条执行路径，因此都报告结果类别。
#[tokio::test]
async fn both_fixed_operations_report_the_result_class() {
    let tree = FixtureTree::new("result-both-operations");
    let mut configuration = config(&tree.script("success.bat"), &tree.operations());
    configuration.operations.stop.script = tree.script("failure.bat");

    let (_, start_body) = post_operation(herdr_bot::app(&configuration), "start").await;
    let (_, stop_body) = post_operation(herdr_bot::app(&configuration), "stop").await;

    assert_eq!(start_body["operation"], "start");
    assert_eq!(class(&start_body), ("completed".to_string(), None));
    assert_eq!(stop_body["operation"], "stop");
    assert_eq!(
        class(&stop_body),
        ("failed".to_string(), Some("failed-exit".to_string()))
    );
}
