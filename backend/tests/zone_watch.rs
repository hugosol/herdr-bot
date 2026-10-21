//! zone 自动刷新的公开接口（S2）：真实文件系统监听、按 zone 节流与监听健康。
//!
//! 这些用例经真实 HTTP/SSE 请求和隔离的真实目录/仓库观察行为：刷新时序由配置的窗口决定，
//! 外部写入由 `std::fs` 与真实 `git` 命令产生，不使用模拟通知。

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp, repo_fixtures_dir};
use serde_json::{Value, json};

/// 与交付配置同形，但把登记文件放进隔离目录，并把自动刷新窗口压到 1 秒。
fn config_with_watch(registry: &Path) -> omp_bot::Config {
    let fixtures = repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config.zones.refresh_interval_seconds =
        omp_bot::config::RefreshIntervalSeconds::new(1).expect("测试窗口必须合法");
    config
}

fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn entries_url(zone: &Path) -> String {
    format!(
        "/api/zones/entries?zone={}&path=",
        encode(&zone.to_string_lossy())
    )
}

fn status_url(zone: &Path) -> String {
    format!(
        "/api/zones/git-status?zone={}",
        encode(&zone.to_string_lossy())
    )
}

fn file_url(zone: &Path, relative: &str) -> String {
    format!(
        "/api/zones/file?zone={}&path={}",
        encode(&zone.to_string_lossy()),
        encode(relative)
    )
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body).expect("响应必须是 JSON")
}

async fn register(server: &TestServer, zone: &Path) {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "登记 zone 应成功：{}",
        response.body
    );
}

async fn snapshot_zone(connection: &mut SseConnection, zone: &Path) -> Value {
    let snapshot = connection.next_event("snapshot").await;
    let data = snapshot.json();
    assert!(
        data["intervalSeconds"].as_u64().is_some(),
        "首帧必须给出实际生效的最小刷新间隔：{data}"
    );
    let needle = zone.to_string_lossy().to_string();
    data["zones"]
        .as_array()
        .expect("zones 必须是数组")
        .iter()
        .find(|entry| entry["zone"] == json!(needle))
        .cloned()
        .unwrap_or_else(|| panic!("首帧必须包含已登记 zone：{data}"))
}

/// 在超时内取下一帧指定命名事件；keep-alive 与其它命名事件都跳过，避免把健康事件误当成刷新。
async fn next_event_within(
    connection: &mut SseConnection,
    expected: &str,
    timeout: Duration,
) -> Option<SseFrame> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let frame = connection.next_frame_within(remaining).await?;
        if frame.event.as_deref() == Some(expected) {
            return Some(frame);
        }
    }
}

async fn next_refresh(connection: &mut SseConnection, zone: &Path) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "等待自动刷新事件超时");
        let Some(frame) = connection.next_frame_within(remaining).await else {
            continue;
        };
        if frame.event.as_deref() != Some("refresh") {
            continue;
        }
        let data = frame.json();
        if data["zone"] == json!(zone.to_string_lossy()) {
            return data;
        }
    }
}

async fn next_health_error(connection: &mut SseConnection, zone: &Path) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "等待监听错误事件超时");
        let Some(frame) = connection.next_frame_within(remaining).await else {
            continue;
        };
        if frame.event.as_deref() != Some("health") {
            continue;
        }
        let data = frame.json();
        if data["zone"] == json!(zone.to_string_lossy())
            && data["health"] == json!("error")
        {
            return data;
        }
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("运行 git");
    assert!(
        output.status.success(),
        "git {args:?} 必须成功：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repository(root: &Path) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["config", "user.name", "zone-watch-test"]);
    git(root, &["config", "user.email", "zone-watch@example.test"]);
    fs::write(root.join("tracked.txt"), "first\n").expect("写入已跟踪文件");
    git(root, &["add", "tracked.txt"]);
    git(root, &["commit", "--quiet", "-m", "initial"]);
}

fn entry_names(response: &HttpResponse) -> Vec<String> {
    assert_eq!(response.status, 200, "目录读取应成功：{}", response.body);
    body(response)["entries"]
        .as_array()
        .expect("entries 必须是数组")
        .iter()
        .map(|entry| entry["name"].as_str().expect("目录项必须有名字").to_string())
        .collect()
}

#[tokio::test]
async fn default_config_reports_ten_second_interval_at_runtime() {
    let zone = TempDir::new("watch default interval");
    let registry = TempDir::new("watch default interval registry");
    let fixtures = repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations").join("success.bat"),
        &fixtures.join("operations"),
    );
    config.zones.registry_file = registry.path().join("zones.json");
    let server = TestServer::start(&config).await;
    register(&server, zone.path()).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let snapshot = connection.next_event("snapshot").await.json();
    assert_eq!(
        snapshot["intervalSeconds"],
        json!(10),
        "缺省配置必须在交付进程里保持 10 秒：{snapshot}"
    );
}

#[tokio::test]
async fn external_plain_directory_create_modify_delete_refreshes_browsing() {
    let zone = TempDir::new("watch plain");
    let registry = TempDir::new("watch plain registry");
    let server = TestServer::start(&config_with_watch(&registry.path().join("zones.json"))).await;
    register(&server, zone.path()).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let view = snapshot_zone(&mut connection, zone.path()).await;
    assert_eq!(view["health"], json!("live"), "监听建立后健康状态：{view}");

    // 外部新增：真实文件系统事件到达后，目录读取能看到新文件。
    fs::write(zone.path().join("appeared.txt"), "first\n").expect("写入新文件");
    let refresh = next_refresh(&mut connection, zone.path()).await;
    assert!(refresh["generation"].as_u64().unwrap_or(0) >= 1);
    assert!(
        entry_names(&get_over_tcp(server.addr(), &entries_url(zone.path())).await)
            .contains(&"appeared.txt".to_string()),
        "外部新增必须反映到目录读取"
    );

    // 外部修改：内容读取看到新内容。修改事件可能落在同一窗口内，下一次刷新是其补刷新。
    fs::write(zone.path().join("appeared.txt"), "second version\n").expect("修改文件");
    next_refresh(&mut connection, zone.path()).await;
    let file = get_over_tcp(
        server.addr(),
        &file_url(zone.path(), "appeared.txt"),
    )
    .await;
    assert_eq!(file.status, 200, "文件读取应成功：{}", file.body);
    assert_eq!(body(&file)["preview"]["content"], json!("second version\n"));

    // 外部删除：目录读取不再把它当成仍存在的文件。
    fs::remove_file(zone.path().join("appeared.txt")).expect("删除文件");
    next_refresh(&mut connection, zone.path()).await;
    assert!(
        !entry_names(&get_over_tcp(server.addr(), &entries_url(zone.path())).await)
            .contains(&"appeared.txt".to_string()),
        "外部删除必须反映到目录读取"
    );
}

#[tokio::test]
async fn external_git_branch_and_worktree_writes_refresh_branch_and_pending_status() {
    let zone = TempDir::new("watch git");
    init_repository(zone.path());
    let registry = TempDir::new("watch git registry");
    let server = TestServer::start(&config_with_watch(&registry.path().join("zones.json"))).await;
    register(&server, zone.path()).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    snapshot_zone(&mut connection, zone.path()).await;

    // 外部切换分支只写 Git 元数据，不碰工作树文件；监听必须覆盖相关 Git 元数据。
    git(zone.path(), &["switch", "--quiet", "-c", "feature-watch"]);
    next_refresh(&mut connection, zone.path()).await;
    let status = get_over_tcp(server.addr(), &status_url(zone.path())).await;
    assert_eq!(status.status, 200, "Git 状态读取应成功：{}", status.body);
    assert_eq!(
        body(&status)["branch"],
        json!("feature-watch"),
        "外部切换分支后刷新必须显示新分支"
    );

    // 外部新增未跟踪文件：待提交状态必须更新，而不是继续显示干净工作树。
    fs::write(zone.path().join("pending.txt"), "pending\n").expect("写入待提交文件");
    next_refresh(&mut connection, zone.path()).await;
    let status = get_over_tcp(server.addr(), &status_url(zone.path())).await;
    let status = body(&status);
    assert_eq!(status["pendingCount"], json!(1), "待提交数量：{status}");
    let changes = status["changes"].as_array().expect("changes 必须是数组");
    assert!(
        changes
            .iter()
            .any(|change| change["path"] == json!("pending.txt")),
        "外部新增必须出现在待提交变更里：{status}"
    );
}

#[tokio::test]
async fn linked_worktree_external_git_metadata_updates_branch() {
    let root = TempDir::new("watch linked worktree");
    let main = root.path().join("main");
    let linked = root.path().join("linked");
    init_repository(&main);
    // 链接工作树的 `.git` 是文件，真正的元数据目录在主仓库内、位于 zone 根之外。
    git(
        &main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "linked-branch",
            &linked.to_string_lossy(),
        ],
    );
    assert!(linked.join(".git").is_file(), "链接工作树的 .git 必须是文件");

    let registry = TempDir::new("watch linked registry");
    let server = TestServer::start(&config_with_watch(&registry.path().join("zones.json"))).await;
    register(&server, &linked).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    snapshot_zone(&mut connection, &linked).await;

    // 切换分支写的是 zone 之外的 Git 元数据目录；只监听工作树文件不足以发现它。
    git(&linked, &["switch", "--quiet", "-c", "linked-next"]);
    next_refresh(&mut connection, &linked).await;
    let status = get_over_tcp(server.addr(), &status_url(&linked)).await;
    assert_eq!(status.status, 200, "Git 状态读取应成功：{}", status.body);
    assert_eq!(body(&status)["branch"], json!("linked-next"));
}

#[tokio::test]
async fn manual_read_inside_refresh_window_returns_current_facts_immediately() {
    let zone = TempDir::new("watch manual read");
    let registry = TempDir::new("watch manual read registry");
    let fixtures = repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations").join("success.bat"),
        &fixtures.join("operations"),
    );
    config.zones.registry_file = registry.path().join("zones.json");
    // 使用缺省 10 秒窗口：窗口内的第二个事件只记待刷新，手动读取仍应立即看到当前事实。
    let server = TestServer::start(&config).await;
    register(&server, zone.path()).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    snapshot_zone(&mut connection, zone.path()).await;

    fs::write(zone.path().join("first.txt"), "first\n").expect("写入第一次变化");
    next_refresh(&mut connection, zone.path()).await;
    fs::write(zone.path().join("second.txt"), "second\n").expect("写入窗口内的变化");
    assert!(
        next_event_within(&mut connection, "refresh", Duration::from_millis(300))
            .await
            .is_none(),
        "窗口内的第二个事件必须等待窗口结束，不应立即刷新"
    );

    // 主动读取不经过自动刷新节流：即使窗口尚未结束，也必须取得当前目录事实。
    let entries = entry_names(&get_over_tcp(server.addr(), &entries_url(zone.path())).await);
    assert!(
        entries.contains(&"first.txt".to_string()) && entries.contains(&"second.txt".to_string()),
        "手动读取必须立即返回当前事实：{entries:?}"
    );
}

#[tokio::test]
async fn two_observers_share_one_throttled_refresh_sequence() {
    let zone = TempDir::new("watch observers");
    let registry = TempDir::new("watch observers registry");
    let server = TestServer::start(&config_with_watch(&registry.path().join("zones.json"))).await;
    register(&server, zone.path()).await;

    let mut first = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let mut second = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let first_view = snapshot_zone(&mut first, zone.path()).await;
    let second_view = snapshot_zone(&mut second, zone.path()).await;
    assert_eq!(first_view["health"], json!("live"));
    assert_eq!(second_view["health"], json!("live"));

    // 第一次写入没有等待窗口：观察者应尽快收到同一代刷新。
    fs::write(zone.path().join("one.txt"), "one\n").expect("写入第一次变化");
    let first_generation = next_refresh(&mut first, zone.path())
        .await["generation"]
        .as_u64()
        .expect("刷新必须携带代数");
    let second_generation = next_refresh(&mut second, zone.path())
        .await["generation"]
        .as_u64()
        .expect("刷新必须携带代数");
    assert_eq!(
        first_generation, second_generation,
        "两个观察者必须共享同一次 zone 刷新，而不是各自生成"
    );

    // 窗口内第二次写入：不立即刷新；窗口结束补一次，两个观察者看到同一代。
    fs::write(zone.path().join("two.txt"), "two\n").expect("写入第二次变化");
    for connection in [&mut first, &mut second] {
        let early = next_event_within(connection, "refresh", Duration::from_millis(350)).await;
        assert!(
            early.is_none(),
            "窗口内写入不应立即产生第二个刷新：{early:?}"
        );
    }
    let first_trailing = next_refresh(&mut first, zone.path())
        .await["generation"]
        .as_u64()
        .expect("补刷新必须携带代数");
    let second_trailing = next_refresh(&mut second, zone.path())
        .await["generation"]
        .as_u64()
        .expect("补刷新必须携带代数");
    assert_eq!(first_trailing, second_trailing, "补刷新也由两个观察者共享");
    assert_eq!(
        first_trailing,
        first_generation + 1,
        "一个窗口只应补一次刷新"
    );

    // 补刷新之后没有新的写入：不应再出现刷新事件。
    for connection in [&mut first, &mut second] {
        assert!(
            next_event_within(connection, "refresh", Duration::from_millis(350))
                .await
                .is_none(),
            "没有新写入时不应继续刷新"
        );
    }
}

#[tokio::test]
async fn listener_failure_is_reported_and_manual_queries_still_read_current_facts() {
    let zone = TempDir::new("watch failure");
    let registry = TempDir::new("watch failure registry");
    let server = TestServer::start(&config_with_watch(&registry.path().join("zones.json"))).await;
    register(&server, zone.path()).await;

    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    snapshot_zone(&mut connection, zone.path()).await;

    // 真实监听失败条件：监听建立后删除整个 zone 目录；重建同名路径不会恢复旧监听。
    fs::remove_dir_all(zone.path()).expect("删除 zone 目录");
    let failure = next_health_error(&mut connection, zone.path()).await;
    assert!(
        failure["message"]
            .as_str()
            .is_some_and(|message| !message.trim().is_empty()),
        "监听错误必须带明确原因：{failure}"
    );

    // 目录确实不可用时，手动查询显示真实错误，而不是空目录。
    let missing = get_over_tcp(server.addr(), &entries_url(zone.path())).await;
    assert_eq!(
        missing.status, 404,
        "目录不存在时读取必须明确失败：{}",
        missing.body
    );
    assert!(
        missing.body.contains("不存在") || missing.body.contains("无法访问"),
        "错误必须说明目录不可用：{}",
        missing.body
    );

    // 重建同名目录（新身份）：手动查询能取得当前事实，但旧监听不能被宣称为已恢复。
    fs::create_dir_all(zone.path()).expect("重建 zone 目录");
    fs::write(zone.path().join("current.txt"), "current\n").expect("写入当前文件");
    let current = get_over_tcp(server.addr(), &entries_url(zone.path())).await;
    assert_eq!(current.status, 200, "手动读取应成功：{}", current.body);
    assert!(
        entry_names(&current).contains(&"current.txt".to_string()),
        "手动读取必须取得重建目录的当前事实"
    );
    assert!(
        next_event_within(&mut connection, "refresh", Duration::from_millis(2500))
            .await
            .is_none(),
        "监听失效后不得继续宣称自动更新正常"
    );
    assert!(
        next_event_within(&mut connection, "health", Duration::from_millis(2500))
            .await
            .is_none(),
        "不得自动宣称监听恢复成功"
    );
}
