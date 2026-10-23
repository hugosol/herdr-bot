//! 新建 zone 的公开 HTTP interface。
//!
//! 期望值来自真实文件系统与真实 Git 命令：创建目录、初始化仓库与登记列表都必须能在测试进程之外
//! 直接观察到，不能只相信响应文字。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

fn config_with_creation(registry: &Path, parent: Option<&Path>) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config.zones.create_parent_directory = parent.map(Path::to_path_buf);
    config
}

fn zones_of(response: &HttpResponse) -> Vec<PathBuf> {
    assert_eq!(response.status, 200, "列表读取应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("列表响应必须是 JSON");
    body["zones"]
        .as_array()
        .expect("zones 必须是数组")
        .iter()
        .map(|entry| PathBuf::from(entry["path"].as_str().expect("登记项必须有 path")))
        .collect()
}

fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(
        response.status, status,
        "拒绝状态码不符合预期：{}",
        response.body
    );
    let body: Value = serde_json::from_str(&response.body).expect("拒绝响应必须是 JSON");
    assert_eq!(body["code"], json!(code), "拒绝类别不符合预期：{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "拒绝必须给出可读说明：{body}"
    );
}

#[tokio::test]
async fn listing_reports_creation_availability_and_explicit_parent() {
    let dir = TempDir::new("zone create availability");
    let parent = dir.path().join("bot-playground");
    fs::create_dir_all(&parent).expect("创建配置的父目录");
    let registry = dir.path().join("configured.json");
    let server = TestServer::start(&config_with_creation(&registry, Some(&parent))).await;

    let response = get_over_tcp(server.addr(), "/api/zones").await;
    assert_eq!(response.status, 200, "列表读取应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("列表响应必须是 JSON");
    assert_eq!(body["creation"]["available"], json!(true));
    assert_eq!(
        body["creation"]["parentDirectory"],
        json!(parent.to_string_lossy()),
        "页面应能显示实际配置的创建父目录"
    );

    let unconfigured = TempDir::new("zone create unconfigured");
    let registry = unconfigured.path().join("unconfigured.json");
    let server = TestServer::start(&config_with_creation(&registry, None)).await;
    let response = get_over_tcp(server.addr(), "/api/zones").await;
    assert_eq!(response.status, 200, "列表读取应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("列表响应必须是 JSON");
    assert_eq!(
        body["creation"]["available"],
        json!(false),
        "未显式配置父目录时新增必须不可用"
    );
}

#[tokio::test]
async fn creation_refuses_missing_configuration_invalid_names_existing_target_and_missing_parent() {
    let unconfigured = TempDir::new("zone create no config");
    let parent = unconfigured.path().join("parent");
    fs::create_dir_all(&parent).expect("创建父目录");
    let registry = unconfigured.path().join("zones.json");
    let server = TestServer::start(&config_with_creation(&registry, None)).await;
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones/create",
            &json!({ "name": "anything" }).to_string(),
        )
        .await,
        400,
        "create-parent-not-configured",
    );
    assert!(
        fs::read_dir(&parent).expect("读取父目录").next().is_none(),
        "未配置时不能创建任何目录"
    );
    drop(server);

    let dir = TempDir::new("zone create boundaries");
    let parent = dir.path().join("bot-playground");
    fs::create_dir_all(&parent).expect("创建配置的父目录");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_creation(&registry, Some(&parent))).await;

    for name in ["", "a/b", "a\\b", "..", "CON", "trailing."] {
        let response = post_json_over_tcp(
            server.addr(),
            "/api/zones/create",
            &json!({ "name": name }).to_string(),
        )
        .await;
        let expected_code = if name.is_empty() {
            "name-required"
        } else {
            "invalid-name"
        };
        assert_rejected(&response, 400, expected_code);
    }
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones/create",
            &json!({ "name": "sneaky", "path": "C:\\outside" }).to_string(),
        )
        .await,
        400,
        "invalid-request",
    );
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones/create?path=C%3A%5Coutside",
            &json!({ "name": "sneaky" }).to_string(),
        )
        .await,
        400,
        "unexpected-parameters",
    );
    assert!(
        fs::read_dir(&parent).expect("读取父目录").next().is_none(),
        "非法名称或请求面不能留下任何目录"
    );

    let first = post_json_over_tcp(
        server.addr(),
        "/api/zones/create",
        &json!({ "name": "taken" }).to_string(),
    )
    .await;
    assert_eq!(first.status, 200, "首次创建应成功：{}", first.body);
    let duplicate = post_json_over_tcp(
        server.addr(),
        "/api/zones/create",
        &json!({ "name": "taken" }).to_string(),
    )
    .await;
    assert_rejected(&duplicate, 409, "name-already-exists");
    assert_eq!(
        fs::read_dir(&parent)
            .expect("读取父目录")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == "taken")
            .count(),
        1,
        "同名目录已存在时不能创建第二份或覆盖"
    );
    drop(server);

    let missing = TempDir::new("zone create missing parent");
    let missing_parent = missing.path().join("does-not-exist");
    let registry = missing.path().join("zones.json");
    let server = TestServer::start(&config_with_creation(&registry, Some(&missing_parent))).await;
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones/create",
            &json!({ "name": "project" }).to_string(),
        )
        .await,
        400,
        "create-parent-not-found",
    );
    assert!(!missing_parent.exists(), "创建父目录缺失时不能自动创建它");
}

#[tokio::test]
async fn registration_failure_keeps_created_directory_and_git_initialization() {
    let dir = TempDir::new("zone create registration failure");
    let parent = dir.path().join("bot-playground");
    fs::create_dir_all(&parent).expect("创建配置的父目录");
    let registry = dir.path().join("registry-is-a-directory");
    fs::create_dir_all(&registry).expect("创建无法作为登记文件的目录");
    let server = TestServer::start(&config_with_creation(&registry, Some(&parent))).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/create",
        &json!({ "name": "kept-project" }).to_string(),
    )
    .await;

    assert_rejected(&response, 500, "zone-registration-failed");
    let body: Value = serde_json::from_str(&response.body).expect("失败响应必须是 JSON");
    let message = body["message"].as_str().expect("失败必须有说明");
    let created = parent.join("kept-project");
    assert!(
        message.contains(&created.to_string_lossy().to_string()),
        "失败说明必须给出完整保留路径：{message}"
    );
    assert!(created.is_dir(), "登记失败不能撤销已经创建的目录");
    assert!(
        created.join(".git").is_dir(),
        "登记失败不能撤销已经完成的 git init"
    );
}

#[tokio::test]
async fn creates_zone_directory_initializes_git_and_registers_it() {
    let dir = TempDir::new("zone create success");
    let parent = dir.path().join("bot-playground");
    fs::create_dir_all(&parent).expect("创建配置的父目录");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_creation(&registry, Some(&parent))).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/create",
        &json!({ "name": "new-project" }).to_string(),
    )
    .await;

    assert_eq!(response.status, 200, "新建 zone 应成功：{}", response.body);
    let created = parent.join("new-project");
    assert!(created.is_dir(), "必须在配置父目录下创建直接子目录");
    assert!(created.join(".git").is_dir(), "创建后必须执行 git init");
    let listed = zones_of(&response);
    assert_eq!(listed.len(), 1, "成功响应必须包含当前完整登记列表");
    assert_eq!(
        fs::canonicalize(&listed[0]).expect("登记路径必须真实存在"),
        fs::canonicalize(&created).expect("新目录必须存在"),
        "成功响应必须包含新登记目录"
    );

    let persisted: Value =
        serde_json::from_str(&fs::read_to_string(&registry).expect("登记文件必须存在"))
            .expect("登记文件必须是 JSON");
    assert_eq!(
        persisted["zones"][0]["path"],
        json!(created.to_string_lossy()),
        "登记必须在返回成功前持久化"
    );

    let git = Command::new("git")
        .args(["-C"])
        .arg(&created)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .expect("必须能运行 git");
    assert!(git.status.success(), "git rev-parse 失败：{git:?}");
    assert_eq!(String::from_utf8_lossy(&git.stdout).trim(), "true");
}
