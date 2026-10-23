//! 跨流程验收（08）：文件功能与既有固定操作、会话和关机行为互不破坏；主动查询不受自动刷新节流
//! 阻碍；本地文件事件不会变成持续远端检查；独立调用者可浏览并执行约定的 Git 操作，而会话写入的
//! 同机来源边界保持不变。
//!
//! 所有观察都经真实 HTTP/SSE 与真实隔离目录/仓库完成；关机动作是记录型替身，**绝不接触系统关机**。

mod common;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use common::sse::SseConnection;
use common::{FixtureTree, HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use herdr_bot::sessions::SessionsState;
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 代表局域网设备的来源地址：RFC 5737 的 TEST-NET-3 文档地址，本机不可能拥有它。
const LAN_SOURCE: &str = "203.0.113.7:40000";

/// 记录型关机动作：只记录调用，绝不接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
}

impl RecordingPowerOff {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// 隔离登记文件，并把自动刷新窗口压到 1 秒；其余配置取交付默认。
fn config_with(registry: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let mut config = common::config(
        &fixtures.join("operations").join("success.bat"),
        &fixtures.join("operations"),
    );
    config.zones.registry_file = registry.to_path_buf();
    config.zones.refresh_interval_seconds =
        herdr_bot::config::RefreshIntervalSeconds::new(1).expect("测试窗口必须合法");
    config
}

/// 把应用装上受控的 TCP 对端地址：这批请求不来自真 socket，来源就是这里指定的地址。
fn from_source(app: &Router, source: &str) -> Router {
    app.clone().layer(MockConnectInfo(
        source.parse::<SocketAddr>().expect("受控来源地址"),
    ))
}

/// 不经真 socket 提交一次请求（来源由 [`from_source`] 决定），返回状态码与响应体。
async fn send(app: Router, method: &str, uri: &str, body: String) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if !body.is_empty() {
        builder = builder.header("content-type", "application/json");
    }
    let response = app
        .oneshot(builder.body(Body::from(body)).expect("构造请求"))
        .await
        .expect("请求应用");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// 建立只读长连接并读出首帧（到帧尾的空行为止），返回状态码与首帧原文。
async fn first_sse_frame(app: Router, uri: &str) -> (StatusCode, String) {
    use futures_util::StreamExt;

    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .expect("构造请求");
    let response = app.oneshot(request).await.expect("请求应用");
    let status = response.status();
    let mut stream = response.into_body().into_data_stream();
    let mut frame: Vec<u8> = Vec::new();
    while !frame.ends_with(b"\n\n") {
        let chunk = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("SSE 首帧应在超时前到达")
            .expect("SSE 流不应在首帧前结束")
            .expect("读取 SSE 首帧失败");
        frame.extend_from_slice(&chunk);
    }
    (status, String::from_utf8_lossy(&frame).into_owned())
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

fn status_url(zone: &Path) -> String {
    format!(
        "/api/zones/git-status?zone={}&refresh=true",
        encode(&zone.to_string_lossy())
    )
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body).expect("响应必须是 JSON")
}

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("运行 git")
}

fn git_ok(root: &Path, args: &[&str]) {
    let output = git(root, args);
    assert!(
        output.status.success(),
        "git {args:?} 必须成功：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(root: &Path, args: &[&str]) -> String {
    let output = git(root, args);
    assert!(
        output.status.success(),
        "git {args:?} 必须成功：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn write(root: &Path, relative: &str, content: impl AsRef<[u8]>) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录");
    }
    fs::write(path, content).expect("写入测试文件");
}

fn init_repository(root: &Path, branch: &str) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git_ok(
        root,
        &["init", "--quiet", &format!("--initial-branch={branch}")],
    );
    git_ok(root, &["config", "user.name", "zone-integration-test"]);
    git_ok(root, &["config", "user.email", "integration@example.test"]);
}

/// 建立一个已推送一次提交的本地仓库与 bare 远端；远端名与分支名都由调用者指定。
fn init_with_remote(root: &Path, remote_name: &str, branch: &str) -> PathBuf {
    init_repository(root, branch);
    write(root, "tracked.txt", "base\n");
    git_ok(root, &["add", "--all"]);
    git_ok(root, &["commit", "--quiet", "-m", "initial"]);

    let remote = root.with_extension("remote.git");
    let output = Command::new("git")
        .args([
            "init",
            "--bare",
            "--quiet",
            &format!("--initial-branch={branch}"),
        ])
        .arg(&remote)
        .output()
        .expect("创建 bare 远端");
    assert!(output.status.success(), "创建 bare 远端必须成功");
    git_ok(
        root,
        &[
            "remote",
            "add",
            remote_name,
            remote.to_str().expect("远端路径必须是 UTF-8"),
        ],
    );
    git_ok(root, &["push", "--quiet", "-u", remote_name, branch]);
    remote
}

fn remote_head(remote: &Path, branch: &str) -> String {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(remote)
        .args(["rev-parse", branch])
        .output()
        .expect("运行 git");
    assert!(output.status.success(), "读取远端分支必须成功");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn remote_tracking_head(root: &Path, remote_name: &str, branch: &str) -> String {
    git_stdout(
        root,
        &["rev-parse", &format!("refs/remotes/{remote_name}/{branch}")],
    )
    .trim()
    .to_string()
}

/// 在独立克隆中向 bare 远端推进一步；返回新的远端提交标识。
fn advance_remote(
    remote: &Path,
    clone: &Path,
    branch: &str,
    relative: &str,
    content: &str,
) -> String {
    let output = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(remote)
        .arg(clone)
        .output()
        .expect("克隆远端");
    assert!(
        output.status.success(),
        "克隆远端必须成功：{}",
        String::from_utf8_lossy(&output.stderr)
    );
    git_ok(clone, &["config", "user.name", "remote-advance"]);
    git_ok(clone, &["config", "user.email", "advance@example.test"]);
    write(clone, relative, content);
    git_ok(clone, &["add", "--all"]);
    git_ok(clone, &["commit", "--quiet", "-m", "remote advance"]);
    git_ok(clone, &["push", "--quiet", "origin", branch]);
    remote_head(remote, branch)
}

async fn register(server: &TestServer, zone: &Path) {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记 zone 应成功：{}", response.body);
}

async fn git_status(server: &TestServer, zone: &Path) -> Value {
    let response = get_over_tcp(server.addr(), &status_url(zone)).await;
    assert_eq!(
        response.status, 200,
        "读取 Git 状态应成功：{}",
        response.body
    );
    body(&response)
}

async fn next_refresh(connection: &mut SseConnection, zone: &Path) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
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

/// P42（08 唯一拥有的交叉用例）：真实监听期间本地文件与 Git 元数据事件只会触发本地状态刷新，
/// **不会**变成持续 fetch。独立推进隔离远端后，本地远端跟踪引用与最近检查时间必须保持不变；
/// 只有显式检查远端才更新它们。
#[tokio::test]
async fn local_file_and_git_metadata_events_never_fetch_the_remote() {
    let dir = TempDir::new("zone integration watch remote");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with(&registry)).await;
    register(&server, &root).await;

    // 页面先建立真实监听；首帧证明监听已经生效。
    let mut connection = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let snapshot = connection.next_event("snapshot").await.json();
    assert!(
        snapshot["zones"]
            .as_array()
            .expect("首帧必须包含 zone 视图")
            .iter()
            .any(|entry| entry["zone"] == json!(root.to_string_lossy())),
        "首帧必须包含已登记 zone：{snapshot}"
    );

    // 隔离远端独立前进；本地尚未检查，远端跟踪引用仍是旧值。
    let checked_before = remote_tracking_head(&root, "upstream", "feature");
    let advanced = advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote one\n",
    );
    assert_ne!(advanced, checked_before, "远端必须独立前进");
    let status = git_status(&server, &root).await;
    assert_eq!(status["sync"]["remoteCheckedAt"], Value::Null, "{status}");
    assert_eq!(status["sync"]["behind"], json!(0), "{status}");
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        checked_before,
        "只读取状态不能更新远端跟踪引用"
    );
    let fetch_head_before = fs::read(root.join(".git").join("FETCH_HEAD")).ok();

    // 本地文件事件：真实写入后监听必须下发刷新，但刷新只通知页面重新读取本地事实。
    write(&root, "local.txt", "local\n");
    next_refresh(&mut connection, &root).await;

    // Git 元数据事件：外部切分支只写 .git，同样必须经真实监听下发刷新。
    // 从远端跟踪分支创建，保留当前分支的既有 upstream，后续主动检查仍走同一目标。
    git_ok(
        &root,
        &[
            "switch",
            "--quiet",
            "-c",
            "browser-branch",
            "upstream/feature",
        ],
    );
    next_refresh(&mut connection, &root).await;

    // 再来一次文件事件，覆盖连续本地活动。
    write(&root, "local2.txt", "local two\n");
    next_refresh(&mut connection, &root).await;

    // 本地事件之后：检查时间仍为空，远端跟踪引用仍是旧值，behind 仍按旧引用计算。
    let status = git_status(&server, &root).await;
    assert_eq!(
        status["branch"],
        json!("browser-branch"),
        "本地事件必须让状态反映真实分支：{status}"
    );
    assert_eq!(
        status["sync"]["remoteCheckedAt"],
        Value::Null,
        "本地事件不能产生远端检查时间：{status}"
    );
    assert_eq!(
        status["sync"]["behind"],
        json!(0),
        "本地事件不能把远端前进算进来：{status}"
    );
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        checked_before,
        "本地事件不得触发 fetch 更新远端跟踪引用"
    );
    assert_eq!(
        fs::read(root.join(".git").join("FETCH_HEAD")).ok(),
        fetch_head_before,
        "本地事件期间不得产生任何 fetch"
    );

    // 主动检查仍按 06 的行为生效：只有它更新远端跟踪引用与检查时间。
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/check-remote",
        &json!({ "zone": &root }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "主动检查远端应成功：{}",
        response.body
    );
    let checked = body(&response);
    assert!(
        checked["sync"]["remoteCheckedAt"].as_u64().unwrap_or(0) > 0,
        "主动检查必须记录检查时间：{checked}"
    );
    assert_eq!(checked["sync"]["behind"], json!(1), "{checked}");
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        advanced,
        "主动检查必须更新本地远端跟踪引用"
    );
}

/// P46：独立调用者（这里以受控的局域网来源代表另一台设备）可以浏览登记 zone 并发起约定的
/// Git 写操作；同一来源的会话事件写入仍按 ADR-0004 被拒绝且不改动会话。公开操作范围里没有
/// 任意命令、脚本或终端入口。
#[tokio::test]
async fn lan_caller_can_browse_and_commit_but_cannot_write_sessions_or_run_commands() {
    let dir = TempDir::new("zone integration lan caller");
    let root = dir.path().join("repository");
    init_repository(&root, "main");
    write(&root, "tracked.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("zones.json");
    let config = config_with(&registry);
    let sessions = Arc::new(SessionsState::new());
    let power = RecordingPowerOff::new();
    let app = herdr_bot::app_with_sessions(
        &config,
        Arc::clone(&power) as Arc<dyn PowerOff>,
        Arc::clone(&sessions),
    );
    // 同一个应用既接受真实回环 TCP，也接受受控的局域网来源：路由与运行时完全一致。
    let server = TestServer::start_app(app.clone()).await;
    let lan = from_source(&app, LAN_SOURCE);

    // 同机来源先登记一条会话；受控局域网来源随后写入必须被拒绝。
    let local = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &json!({"sessionId": "local-1", "kind": "register", "data": {"cwd": "C:\\work"}})
            .to_string(),
    )
    .await;
    assert_eq!(local.status, 200, "同机会话写入应成功：{}", local.body);
    let rejected = send(
        lan.clone(),
        "POST",
        "/api/sessions/events",
        json!({"sessionId": "lan-1", "kind": "register", "data": {"cwd": "C:\\work"}}).to_string(),
    )
    .await;
    assert_eq!(
        rejected.0,
        StatusCode::FORBIDDEN,
        "非同机来源的会话写入必须被拒绝：{} {}",
        rejected.0,
        rejected.1
    );
    let (_, frame) = first_sse_frame(lan.clone(), "/api/sessions/events").await;
    assert!(
        frame.contains("local-1") && !frame.contains("lan-1"),
        "被拒的会话写入不得改动静止会话集合：{frame:?}"
    );

    // 同一局域网来源可以读取登记列表、浏览目录与文件、读取 Git 状态。
    let (status, listed) = send(lan.clone(), "GET", "/api/zones", String::new()).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let (status, registered) = send(
        lan.clone(),
        "POST",
        "/api/zones",
        json!({ "path": &root }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "局域网来源登记应成功：{registered}");

    let entries = format!(
        "/api/zones/entries?zone={}&path=",
        encode(&root.to_string_lossy())
    );
    let (status, entries) = send(lan.clone(), "GET", &entries, String::new()).await;
    assert_eq!(status, StatusCode::OK, "局域网来源浏览应成功：{entries}");
    assert!(
        entries.contains("tracked.txt"),
        "目录输出必须反映真实文件：{entries}"
    );
    let file = format!(
        "/api/zones/file?zone={}&path=tracked.txt",
        encode(&root.to_string_lossy())
    );
    let (status, file) = send(lan.clone(), "GET", &file, String::new()).await;
    assert_eq!(status, StatusCode::OK, "局域网来源读取文件应成功：{file}");
    assert!(file.contains("base"), "文件内容必须真实：{file}");

    // 产生待提交变更并由同一个局域网来源完成 Commit。
    write(&root, "tracked.txt", "changed\n");
    write(&root, "added.txt", "added\n");
    let (status, status_body) = send(lan.clone(), "GET", &status_url(&root), String::new()).await;
    assert_eq!(status, StatusCode::OK, "{status_body}");
    let status_json: Value = serde_json::from_str(&status_body).expect("Git 状态必须是 JSON");
    assert_eq!(status_json["repository"], json!(true), "{status_json}");
    let token = status_json["reviewToken"]
        .as_str()
        .expect("审查版本必须存在")
        .to_string();

    let (status, committed) = send(
        lan.clone(),
        "POST",
        "/api/zones/commit",
        json!({
            "zone": &root,
            "message": "from a lan caller",
            "reviewToken": token,
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "局域网来源 Commit 应成功：{committed}"
    );
    let committed: Value = serde_json::from_str(&committed).expect("提交响应必须是 JSON");
    assert_eq!(committed["outcome"], json!("committed"), "{committed}");
    let tree = git_stdout(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(tree.contains("added.txt"), "提交树必须包含新增文件：{tree}");
    assert!(
        tree.contains("tracked.txt"),
        "提交树必须包含被修改文件：{tree}"
    );

    // 公开操作范围检查：没有任意命令、脚本或终端入口；参数也不能换成别的执行目标。
    for (method, path, expected) in [
        ("GET", "/api/terminal", "unknown-endpoint"),
        ("POST", "/api/command", "unknown-endpoint"),
        ("POST", "/api/zones/exec", "unknown-endpoint"),
        ("POST", "/api/operations/run", "unknown-operation"),
        ("GET", "/api/zones/file/extra", "unknown-endpoint"),
    ] {
        let (status, body) = send(lan.clone(), method, path, String::new()).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{method} {path} 必须被封闭请求面拒绝：{status} {body}"
        );
        let rejection: Value = serde_json::from_str(&body).expect("拒绝响应必须是 JSON");
        assert_eq!(
            rejection["code"], expected,
            "{method} {path} 必须被封闭请求面拒绝：{body}"
        );
    }
    // 固定操作也不能携带命令/参数：带请求体或查询串一律拒绝，且不执行脚本。
    let (status, body) = send(
        lan.clone(),
        "POST",
        "/api/operations/start?command=calc",
        String::new(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "固定操作请求面必须封闭：{status} {body}"
    );
    assert_eq!(power.calls(), 0, "验收过程中不得触发真实关机");
}

/// P3：文件功能运行期间（真实监听已建立），固定操作、会话写入与全局关机命令仍按原有语义工作；
/// 关机只建立/取消后端安排，记录型动作零调用。
#[tokio::test]
async fn zone_activity_does_not_change_fixed_operations_sessions_or_shutdown() {
    let fixtures = FixtureTree::new("zone integration fixed operations");
    let zone = TempDir::new("zone integration fixed operations zone");
    let registry = TempDir::new("zone integration fixed operations registry");
    let mut config = common::config(&fixtures.script("success.bat"), &fixtures.operations());
    config.zones.registry_file = registry.path().join("zones.json");
    let sessions = Arc::new(SessionsState::new());
    let power = RecordingPowerOff::new();
    let server = TestServer::start_app(herdr_bot::app_with_sessions(
        &config,
        Arc::clone(&power) as Arc<dyn PowerOff>,
        Arc::clone(&sessions),
    ))
    .await;

    register(&server, zone.path()).await;
    let mut watch = SseConnection::open(server.addr(), "/api/zones/watch", &[]).await;
    let _ = watch.next_event("snapshot").await;

    // 固定操作仍按名称执行，并且实际调用了配置的脚本一次。
    let started = common::post_over_tcp(server.addr(), "/api/operations/start").await;
    assert_eq!(started.status, 200, "固定操作应成功：{}", started.body);
    assert_eq!(
        fixtures.records().len(),
        1,
        "固定操作必须真实调用一次夹具脚本"
    );

    // 同机会话写入仍被接受，并能从观察流读到。
    let registered = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &json!({"sessionId": "still-here", "kind": "register", "data": {"cwd": "C:\\work"}})
            .to_string(),
    )
    .await;
    assert_eq!(
        registered.status, 200,
        "文件功能不得放宽或收紧会话写入：{}",
        registered.body
    );

    // 全局关机命令仍只建立/取消安排；记录型动作不得被调用。
    let start = common::post_over_tcp(server.addr(), "/api/shutdown/start").await;
    assert_eq!(start.status, 200, "关机开始意图应成功：{}", start.body);
    let cancel = common::post_over_tcp(server.addr(), "/api/shutdown/cancel").await;
    assert_eq!(cancel.status, 200, "关机取消应成功：{}", cancel.body);
    assert_eq!(power.calls(), 0, "自动化验收不得触发真实关机");
}
