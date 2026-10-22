//! 按既有 upstream 与 Git 配置的安全 Sync（S2）：真实 HTTP 请求 + 隔离的真实 Git 仓库、bare 远端与 hooks。
//!
//! 期望值来自本契约与独立写定的文件内容、Git 引用和提交历史，不调用实现内部的同步计算。
//! 所有远端操作只针对隔离的 bare 远端，不向任何业务远端推送。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::{
    FixtureTree, HttpResponse, OpenJsonRequest, TempDir, TestServer, get_over_tcp,
    post_json_over_tcp, post_over_tcp, wait_until,
};
use serde_json::{Value, json};

/// 与交付配置同形，但把 zone 登记文件放到隔离临时目录。
fn config_with_registry(registry: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
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

fn status_url(zone: &Path) -> String {
    format!(
        "/api/zones/git-status?zone={}&refresh=true",
        encode(&zone.to_string_lossy())
    )
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|_| panic!("响应必须是 JSON：{}", response.body))
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

fn init_repository(root: &Path, branch: &str) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git_ok(
        root,
        &["init", "--quiet", &format!("--initial-branch={branch}")],
    );
    git_ok(root, &["config", "user.name", "zone-sync-test"]);
    git_ok(root, &["config", "user.email", "sync@example.test"]);
}

fn write(root: &Path, relative: &str, content: impl AsRef<[u8]>) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录");
    }
    fs::write(path, content).expect("写入测试文件");
}

fn write_hook(root: &Path, name: &str, script: &str) {
    let hooks = root.join(".git").join("hooks");
    fs::create_dir_all(&hooks).expect("创建 hooks 目录");
    let path = hooks.join(name);
    fs::write(&path, script.replace("\r\n", "\n")).expect("写入 hook");
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
    Command::new("git")
        .arg("--git-dir")
        .arg(remote)
        .args(["rev-parse", branch])
        .output()
        .map(|output| {
            assert!(output.status.success(), "读取远端分支必须成功");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        })
        .expect("运行 git")
}

fn remote_tracking_head(root: &Path, remote_name: &str, branch: &str) -> String {
    git_stdout(
        root,
        &["rev-parse", &format!("refs/remotes/{remote_name}/{branch}")],
    )
    .trim()
    .to_string()
}

fn head_oid(root: &Path) -> String {
    git_stdout(root, &["rev-parse", "HEAD"]).trim().to_string()
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

async fn check_remote(server: &TestServer, zone: &Path) -> HttpResponse {
    post_json_over_tcp(
        server.addr(),
        "/api/zones/check-remote",
        &json!({ "zone": zone }).to_string(),
    )
    .await
}

async fn submit_sync(server: &TestServer, zone: &Path) -> HttpResponse {
    post_json_over_tcp(
        server.addr(),
        "/api/zones/sync",
        &json!({ "zone": zone }).to_string(),
    )
    .await
}

#[tokio::test]
async fn check_remote_updates_real_references_while_status_reads_never_fetch() {
    let dir = TempDir::new("zone sync check remote");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let checked_at_before = remote_head(&remote, "feature");
    let clone = dir.path().join("advance");
    let remote_advanced = advance_remote(&remote, &clone, "feature", "remote.txt", "remote one\n");

    let status = git_status(&server, &root).await;
    assert_eq!(status["sync"]["upstream"]["remote"], json!("upstream"));
    assert_eq!(status["sync"]["upstream"]["branch"], json!("feature"));
    assert_eq!(
        status["sync"]["upstream"]["short"],
        json!("upstream/feature")
    );
    assert_eq!(
        status["sync"]["remoteCheckedAt"],
        Value::Null,
        "从未检查过的远端不能宣称已知：{status}"
    );
    assert_eq!(
        status["sync"]["behind"],
        json!(0),
        "读取状态只能使用本地引用，不能自动 fetch：{status}"
    );
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        checked_at_before,
        "只读状态读取不能更新远端跟踪引用"
    );
    assert_eq!(
        head_oid(&root),
        checked_at_before,
        "检查远端前本地 HEAD 不变"
    );

    // 主动检查：真实更新远端跟踪引用，并在响应中给出检查时间与真实 behind。
    let response = check_remote(&server, &root).await;
    assert_eq!(response.status, 200, "检查远端应成功：{}", response.body);
    let checked = body(&response);
    assert!(
        checked["sync"]["remoteCheckedAt"].as_u64().unwrap_or(0) > 0,
        "成功检查必须记录检查时间：{checked}"
    );
    assert_eq!(checked["sync"]["behind"], json!(1), "{checked}");
    assert_eq!(checked["sync"]["ahead"], json!(0), "{checked}");
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        remote_advanced,
        "检查必须更新实际的远端跟踪引用"
    );
    assert_eq!(head_oid(&root), checked_at_before, "检查不改变本地 HEAD");
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        "",
        "检查不改变工作树"
    );

    // 检查后远端再次独立前进：只读取状态不得 fetch；再次主动检查才更新引用与页面事实。
    let second_advance = advance_remote(
        &remote,
        &dir.path().join("advance-2"),
        "feature",
        "remote2.txt",
        "remote two\n",
    );
    let status = git_status(&server, &root).await;
    assert_eq!(
        status["sync"]["behind"],
        json!(1),
        "状态读取不能偷偷 fetch：{status}"
    );
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        remote_advanced,
        "远端独立前进不会自动改变本地引用"
    );
    let response = check_remote(&server, &root).await;
    assert_eq!(response.status, 200, "再次检查应成功：{}", response.body);
    let checked = body(&response);
    assert_eq!(checked["sync"]["behind"], json!(2), "{checked}");
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        second_advance
    );

    // 同一路径上的仓库被外部替换后，旧检查时间不能继续冒充当前仓库的远端状态。
    fs::remove_dir_all(&root).expect("外部删除仓库");
    init_repository(&root, "feature");
    write(&root, "tracked.txt", "replacement base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "replacement base"]);
    git_ok(
        &root,
        &[
            "remote",
            "add",
            "upstream",
            remote.to_str().expect("UTF-8 路径"),
        ],
    );
    let status = git_status(&server, &root).await;
    assert_eq!(
        status["sync"]["remoteCheckedAt"],
        Value::Null,
        "仓库替换后旧检查记录必须失效：{status}"
    );
    assert_eq!(
        status["sync"]["ahead"],
        Value::Null,
        "新仓库尚未取得远端跟踪引用：{status}"
    );
    assert_eq!(status["sync"]["behind"], Value::Null, "{status}");
}

#[tokio::test]
async fn sync_uses_non_default_upstream_and_checks_again_before_pulling_then_pushing() {
    let dir = TempDir::new("zone sync merge");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");

    // 远端独立前进；本地也有自己的提交，同步前必须先 fetch 才能看到两次远端变化。
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote one\n",
    );
    advance_remote(
        &remote,
        &dir.path().join("advance-2"),
        "feature",
        "remote2.txt",
        "remote two\n",
    );
    write(&root, "local.txt", "local work\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "local work"]);

    // 编辑器条件：Sync 不能在服务器上打开交互式编辑器；若被调用会留下标记。
    let editor_marker = dir.path().join("editor-marker.txt");
    let editor = dir.path().join("editor.bat");
    fs::write(
        &editor,
        format!(
            "@echo off\r\necho editor-opened > \"{}\"\r\n",
            editor_marker.display()
        ),
    )
    .expect("写入假编辑器");
    git_ok(
        &root,
        &[
            "config",
            "core.editor",
            editor.to_str().expect("UTF-8 路径"),
        ],
    );
    let config_before = git_stdout(&root, &["config", "--local", "--list"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(response.status, 200, "Sync 应成功：{}", response.body);
    assert!(
        !editor_marker.exists(),
        "Sync 不能在服务器上打开交互式编辑器"
    );
    let accepted = body(&response);
    assert_eq!(accepted["outcome"], json!("synced"));
    assert_eq!(accepted["branch"], json!("feature"));
    assert_eq!(accepted["upstream"]["remote"], json!("upstream"));
    assert_eq!(accepted["upstream"]["short"], json!("upstream/feature"));
    assert_eq!(
        accepted["pulled"],
        json!(2),
        "Sync 前必须重新检查远端：{accepted}"
    );
    assert!(accepted["pushed"].as_u64().unwrap_or(0) >= 1, "{accepted}");
    assert!(accepted["remoteCheckedAt"].as_u64().unwrap_or(0) > 0);
    assert_eq!(accepted["ahead"], json!(0), "同步后没有待推送：{accepted}");
    assert_eq!(accepted["behind"], json!(0), "同步后没有待拉取：{accepted}");

    // 本地合并了远端内容，又没有强推：远端 head 与本地 head 一致，且远端包含本地提交。
    assert_eq!(
        remote_head(&remote, "feature"),
        head_oid(&root),
        "按需 push 必须把同步后的本地提交推到既有 upstream"
    );
    assert_eq!(
        git_stdout(&root, &["show", "HEAD:remote.txt"]),
        "remote one\n"
    );
    assert_eq!(
        git_stdout(&root, &["show", "HEAD:remote2.txt"]),
        "remote two\n"
    );
    assert_eq!(
        git_stdout(&root, &["show", "HEAD:local.txt"]),
        "local work\n"
    );
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        remote_head(&remote, "feature"),
        "同步后远端跟踪引用必须与远端一致"
    );
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        "",
        "Sync 后工作树应干净"
    );
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        config_before,
        "Sync 不能修改 Git 配置"
    );
}

#[tokio::test]
async fn sync_follows_existing_rebase_configuration() {
    let dir = TempDir::new("zone sync rebase config");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    write(&root, "local.txt", "local\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "local on old base"]);
    git_ok(&root, &["config", "pull.rebase", "true"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(response.status, 200, "Sync 应成功：{}", response.body);
    assert_eq!(
        git_stdout(&root, &["rev-list", "--merges", "--count", "HEAD"]).trim(),
        "0",
        "pull.rebase=true 时必须按既有配置 rebase，而不是另造 merge 提交"
    );
    let subjects = git_stdout(&root, &["log", "--format=%s", "--reverse"]);
    let lines: Vec<&str> = subjects.lines().collect();
    assert_eq!(
        lines,
        vec!["initial", "remote advance", "local on old base"],
        "rebase 后本地提交应位于远端提交之后：{subjects}"
    );
    assert_eq!(remote_head(&remote, "feature"), head_oid(&root));
}

#[tokio::test]
async fn behind_only_sync_never_creates_a_meaningless_push() {
    let dir = TempDir::new("zone sync behind only");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    git_ok(&root, &["config", "pull.ff", "only"]);

    let hook_marker = dir.path().join("push-hook-ran.txt");
    let marker_for_shell = hook_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-push",
        &format!("#!/bin/sh\necho pushed > '{marker_for_shell}'\n"),
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(response.status, 200, "Sync 应成功：{}", response.body);
    let accepted = body(&response);
    assert_eq!(accepted["pulled"], json!(1));
    assert_eq!(
        accepted["pushed"],
        json!(0),
        "没有待推送时不能 push：{accepted}"
    );
    assert!(
        !hook_marker.exists(),
        "只有 behind 时不能产生无意义的 push 副作用"
    );
    assert_eq!(remote_head(&remote, "feature"), head_oid(&root));
}

#[tokio::test]
async fn push_rejection_after_a_successful_pull_keeps_effective_local_state_and_reports_git_output()
{
    let dir = TempDir::new("zone sync push rejected");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    write(&root, "local.txt", "local\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "local work"]);
    let remote_before = remote_head(&remote, "feature");

    let hooks = remote.join("hooks");
    fs::create_dir_all(&hooks).expect("创建远端 hooks 目录");
    fs::write(
        hooks.join("pre-receive"),
        "#!/bin/sh\necho 'push rejected <script>window.__pwned = 1</script>' >&2\nexit 1\n",
    )
    .expect("写入远端拒绝 hook");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 500,
        "push 拒绝必须明确失败：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-command-failed"));
    let message = payload["message"].as_str().expect("message 字符串");
    assert!(message.contains("push"), "失败必须指出步骤：{message}");
    assert!(
        message.contains("window.__pwned = 1"),
        "页面必须保留 Git 原始诊断：{message}"
    );
    assert!(
        message.contains("不会被自动撤销"),
        "不能宣称已完成的 pull 回滚：{message}"
    );

    // pull 已生效：本地有一个包含远端内容的提交；push 被拒绝，远端引用不变。
    assert_eq!(remote_head(&remote, "feature"), remote_before);
    assert!(
        git_stdout(&root, &["show", "HEAD:remote.txt"]).contains("remote"),
        "pull 成功后发生的 push 拒绝不能撤销已生效的本地内容"
    );
    assert_ne!(head_oid(&root), remote_before);
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        "",
        "pull 后工作树仍应干净，不伪造冲突"
    );

    let after = git_status(&server, &root).await;
    assert_eq!(after["conflict"], json!(false));
    assert_eq!(after["blockedReason"], Value::Null);
    assert!(
        after["sync"]["ahead"].as_u64().unwrap_or(0) >= 1,
        "实际状态应显示仍需推送：{after}"
    );

    // 移除远端拒绝后，按实际状态重试即可完成推送。
    fs::remove_file(hooks.join("pre-receive")).expect("移除远端 hook");
    let response = submit_sync(&server, &root).await;
    assert_eq!(response.status, 200, "恢复后应成功：{}", response.body);
    assert_eq!(remote_head(&remote, "feature"), head_oid(&root));
}

#[tokio::test]
async fn fetch_and_pull_failures_stop_later_steps_and_are_not_reported_as_conflicts() {
    let dir = TempDir::new("zone sync fetch failure");
    let root = dir.path().join("repository");
    init_with_remote(&root, "upstream", "feature");
    let head_before = head_oid(&root);
    git_ok(
        &root,
        &[
            "remote",
            "set-url",
            "upstream",
            r"C:\definitely-missing\remote.git",
        ],
    );
    let hook_marker = dir.path().join("fetch-failure-push-hook.txt");
    let marker_for_shell = hook_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-push",
        &format!("#!/bin/sh\necho pushed > '{marker_for_shell}'\n"),
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 500,
        "fetch 失败必须明确：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-command-failed"));
    let message = payload["message"].as_str().expect("message 字符串");
    assert!(
        message.contains("fetch"),
        "失败必须指出 fetch 步骤：{message}"
    );
    assert_eq!(head_oid(&root), head_before, "fetch 失败不能改变本地 HEAD");
    assert!(!hook_marker.exists(), "fetch 失败后不能继续 pull/push");
    let after = git_status(&server, &root).await;
    assert_eq!(
        after["conflict"],
        json!(false),
        "网络/路径失败不能误标为文件冲突：{after}"
    );
    assert_eq!(after["blockedReason"], Value::Null);

    // pull 冲突：本地与远端修改同一文件。冲突真实存在，但后续 push 仍必须停止。
    let conflicted_root = dir.path().join("pull-conflict");
    let conflicted_remote = init_with_remote(&conflicted_root, "origin", "main");
    let clone = dir.path().join("conflict-advance");
    {
        let output = Command::new("git")
            .args(["clone", "--quiet"])
            .arg(&conflicted_remote)
            .arg(&clone)
            .output()
            .expect("克隆远端");
        assert!(output.status.success());
        git_ok(&clone, &["config", "user.name", "conflict"]);
        git_ok(&clone, &["config", "user.email", "conflict@example.test"]);
        write(&clone, "tracked.txt", "remote version\n");
        git_ok(&clone, &["add", "--all"]);
        git_ok(&clone, &["commit", "--quiet", "-m", "remote conflict"]);
        git_ok(&clone, &["push", "--quiet", "origin", "main"]);
    }
    write(&conflicted_root, "tracked.txt", "local version\n");
    git_ok(&conflicted_root, &["add", "--all"]);
    git_ok(
        &conflicted_root,
        &["commit", "--quiet", "-m", "local conflict"],
    );
    let conflict_marker = dir.path().join("pull-conflict-push-hook.txt");
    let conflict_marker_for_shell = conflict_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &conflicted_root,
        "pre-push",
        &format!("#!/bin/sh\necho pushed > '{conflict_marker_for_shell}'\n"),
    );
    let remote_before_conflict = remote_head(&conflicted_remote, "main");
    register(&server, &conflicted_root).await;

    let response = submit_sync(&server, &conflicted_root).await;
    assert_eq!(response.status, 500, "pull 冲突必须明确：{}", response.body);
    let message = body(&response)["message"]
        .as_str()
        .expect("message 字符串")
        .to_string();
    assert!(
        message.contains("pull"),
        "失败必须指出 pull 步骤：{message}"
    );
    assert!(!conflict_marker.exists(), "pull 冲突后不能继续 push");
    assert_eq!(
        remote_head(&conflicted_remote, "main"),
        remote_before_conflict,
        "pull 冲突后不能改变远端"
    );
    let after = git_status(&server, &conflicted_root).await;
    assert_eq!(after["conflict"], json!(true), "真实冲突必须可见：{after}");
    assert!(
        after["blockedReason"]
            .as_str()
            .expect("阻塞原因")
            .contains("冲突"),
        "{after}"
    );
    let response = submit_sync(&server, &conflicted_root).await;
    assert_eq!(response.status, 409, "冲突时直接请求也必须拒绝");
    assert_eq!(body(&response)["code"], json!("sync-blocked"));

    // 外部撤销未完成流程后，刷新立即恢复；这不会自动完成冲突或改写本地提交。
    git_ok(&conflicted_root, &["merge", "--abort"]);
    let after_abort = git_status(&server, &conflicted_root).await;
    assert_eq!(after_abort["conflict"], json!(false), "{after_abort}");
    assert_eq!(after_abort["blockedReason"], Value::Null, "{after_abort}");
    assert_eq!(
        git_stdout(&conflicted_root, &["status", "--porcelain=v1"]),
        "",
        "撤销流程后工作树应恢复干净"
    );

    // 外部重新合并、解决并完成流程后，刷新恢复；同步随后按实际状态推送。
    let merge = git(&conflicted_root, &["merge", "origin/main"]);
    assert!(!merge.status.success(), "重新合入远端应产生冲突供外部解决");
    write(&conflicted_root, "tracked.txt", "resolved\n");
    git_ok(&conflicted_root, &["add", "tracked.txt"]);
    git_ok(
        &conflicted_root,
        &["commit", "--quiet", "-m", "resolve merge externally"],
    );
    let after = git_status(&server, &conflicted_root).await;
    assert_eq!(after["conflict"], json!(false));
    assert_eq!(after["blockedReason"], Value::Null);
    let response = submit_sync(&server, &conflicted_root).await;
    assert_eq!(
        response.status, 200,
        "外部完成流程后应恢复 Sync：{}",
        response.body
    );
    assert_eq!(
        remote_head(&conflicted_remote, "main"),
        head_oid(&conflicted_root)
    );
}

#[tokio::test]
async fn pull_policy_rejection_stops_before_push_and_does_not_override_configuration() {
    let dir = TempDir::new("zone sync pull policy");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    write(&root, "local.txt", "local\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "local work"]);
    git_ok(&root, &["config", "pull.ff", "only"]);
    let config_before = git_stdout(&root, &["config", "--local", "--list"]);
    let head_before = head_oid(&root);
    let remote_before = remote_head(&remote, "feature");

    let marker = dir.path().join("policy-push-hook.txt");
    let marker_for_shell = marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-push",
        &format!("#!/bin/sh\necho pushed > '{marker_for_shell}'\n"),
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 500,
        "pull 配置拒绝必须明确失败：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-command-failed"));
    let message = payload["message"].as_str().expect("message 字符串");
    assert!(
        message.contains("pull"),
        "失败必须指出 pull 步骤：{message}"
    );
    assert!(
        message.contains("fast-forward") || message.contains("not possible"),
        "必须保留 Git 的配置拒绝诊断：{message}"
    );
    assert!(!marker.exists(), "pull 失败后不能继续 push");
    assert_eq!(head_oid(&root), head_before, "失败的 pull 不能改动 HEAD");
    assert_eq!(remote_head(&remote, "feature"), remote_before);
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        config_before,
        "不能为了成功而覆盖 pull 策略"
    );
    let after = git_status(&server, &root).await;
    assert_eq!(after["conflict"], json!(false), "策略拒绝不是文件冲突");
    assert_eq!(after["blockedReason"], Value::Null);
    assert!(
        after["sync"]["ahead"].as_u64().unwrap_or(0) >= 1
            && after["sync"]["behind"].as_u64().unwrap_or(0) >= 1,
        "实际状态应保留分叉事实：{after}"
    );
}

#[tokio::test]
async fn plain_detached_dirty_and_no_upstream_are_rejected_without_writes() {
    let dir = TempDir::new("zone sync blocked states");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;

    // 普通目录：页面与直接请求都不产生 Sync。
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).expect("创建普通目录");
    write(&plain, "file.txt", "plain\n");
    register(&server, &plain).await;
    let response = submit_sync(&server, &plain).await;
    assert_eq!(response.status, 409, "普通目录不能 Sync：{}", response.body);
    assert_eq!(body(&response)["code"], json!("not-a-repository"));
    let response = check_remote(&server, &plain).await;
    assert_eq!(response.status, 409, "普通目录不能检查远端");
    assert!(!plain.join(".git").exists(), "不能隐式初始化 Git");

    // 脏工作树：拒绝，且不 stash、不 commit、不改引用。
    let dirty = dir.path().join("dirty");
    let dirty_remote = init_with_remote(&dirty, "upstream", "feature");
    let dirty_remote_before = remote_head(&dirty_remote, "feature");
    let dirty_head_before = head_oid(&dirty);
    register(&server, &dirty).await;
    assert_eq!(git_status(&server, &dirty).await["pendingCount"], json!(0));
    // A previously cached clean status is never authority for a write operation.
    write(&dirty, "tracked.txt", "uncommitted\n");
    let response = submit_sync(&server, &dirty).await;
    assert_eq!(response.status, 409, "脏工作树不能 Sync：{}", response.body);
    assert_eq!(body(&response)["code"], json!("sync-dirty-worktree"));
    assert_eq!(head_oid(&dirty), dirty_head_before);
    assert_eq!(remote_head(&dirty_remote, "feature"), dirty_remote_before);
    assert_eq!(
        git_stdout(&dirty, &["status", "--porcelain=v1"]),
        " M tracked.txt\n",
        "Sync 不能 stash 或隐式提交本地修改"
    );

    // detached HEAD：明确阻止。
    let detached = dir.path().join("detached");
    let detached_remote = init_with_remote(&detached, "origin", "main");
    git_ok(&detached, &["checkout", "--quiet", "--detach"]);
    let detached_head = head_oid(&detached);
    let detached_remote_before = remote_head(&detached_remote, "main");
    register(&server, &detached).await;
    let response = submit_sync(&server, &detached).await;
    assert_eq!(
        response.status, 409,
        "detached HEAD 不能 Sync：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("sync-blocked"));
    assert_eq!(head_oid(&detached), detached_head);
    assert_eq!(
        remote_head(&detached_remote, "main"),
        detached_remote_before
    );

    // 无 upstream：只显示限制，不配置 upstream / Publish Branch。
    let no_upstream = dir.path().join("no-upstream");
    init_repository(&no_upstream, "local-only");
    write(&no_upstream, "file.txt", "base\n");
    git_ok(&no_upstream, &["add", "--all"]);
    git_ok(&no_upstream, &["commit", "--quiet", "-m", "initial"]);
    let config_before = git_stdout(&no_upstream, &["config", "--local", "--list"]);
    register(&server, &no_upstream).await;
    let status = git_status(&server, &no_upstream).await;
    assert_eq!(status["sync"]["upstream"], Value::Null);
    assert_eq!(status["sync"]["ahead"], Value::Null);
    assert_eq!(status["sync"]["behind"], Value::Null);
    let response = submit_sync(&server, &no_upstream).await;
    assert_eq!(
        response.status, 409,
        "无 upstream 不能 Sync：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("no-upstream"));
    let response = check_remote(&server, &no_upstream).await;
    assert_eq!(response.status, 409, "无 upstream 不能检查远端");
    assert_eq!(body(&response)["code"], json!("no-upstream"));
    assert_eq!(
        git_stdout(&no_upstream, &["config", "--local", "--list"]),
        config_before,
        "不能隐式配置 upstream 或修改 Git 配置"
    );
    assert_eq!(git_stdout(&no_upstream, &["remote"]), "");
}

#[tokio::test]
async fn nested_repositories_and_submodules_keep_their_own_references_and_state() {
    let dir = TempDir::new("zone sync nested repositories");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "origin", "main");

    // 子模块：父仓库只记录 gitlink。
    let sub_source = dir.path().join("sub-source");
    init_repository(&sub_source, "main");
    write(&sub_source, "peer.txt", "sub base\n");
    git_ok(&sub_source, &["add", "--all"]);
    git_ok(&sub_source, &["commit", "--quiet", "-m", "sub base"]);
    let add_submodule = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
            sub_source.to_str().expect("UTF-8 路径"),
            "sub",
        ])
        .output()
        .expect("添加子模块");
    assert!(
        add_submodule.status.success(),
        "添加子模块必须成功：{}",
        String::from_utf8_lossy(&add_submodule.stderr)
    );

    // 嵌套仓库：父仓库记录其 gitlink。
    let embedded = root.join("embedded");
    init_repository(&embedded, "main");
    write(&embedded, "inner.txt", "embedded base\n");
    git_ok(&embedded, &["add", "--all"]);
    git_ok(&embedded, &["commit", "--quiet", "-m", "embedded base"]);
    git_ok(&root, &["add", "embedded"]);
    git_ok(
        &root,
        &["commit", "--quiet", "-m", "track embedded and submodule"],
    );
    git_ok(&root, &["push", "--quiet", "origin", "main"]);

    let sub = root.join("sub");
    let sub_head = git_stdout(&sub, &["rev-parse", "HEAD"]).trim().to_string();
    let embedded_head = git_stdout(&embedded, &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    write(&sub, "dirty-only.txt", "sub dirty\n");
    write(&embedded, "dirty-only.txt", "embedded dirty\n");

    // 父仓库自己前进一个提交，Sync 会 push；嵌套仓库内部状态不能被改写。
    write(&root, "parent.txt", "parent next\n");
    git_ok(&root, &["add", "parent.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "parent next"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 200,
        "父仓库 Sync 应成功：{}",
        response.body
    );
    assert_eq!(remote_head(&remote, "main"), head_oid(&root));

    assert_eq!(
        git_stdout(&sub, &["rev-parse", "HEAD"]).trim(),
        sub_head,
        "Sync 不能移动子模块内部 HEAD"
    );
    assert_eq!(
        git_stdout(&embedded, &["rev-parse", "HEAD"]).trim(),
        embedded_head,
        "Sync 不能移动嵌套仓库内部 HEAD"
    );
    assert_eq!(
        git_stdout(&sub, &["status", "--porcelain=v1"]),
        "?? dirty-only.txt\n",
        "Sync 不能改写子模块内部工作树"
    );
    assert_eq!(
        git_stdout(&embedded, &["status", "--porcelain=v1"]),
        "?? dirty-only.txt\n",
        "Sync 不能改写嵌套仓库内部工作树"
    );
    let parent_status = git_stdout(&root, &["status", "--porcelain=v1"]);
    assert!(
        !parent_status.contains("dirty-only.txt"),
        "嵌套仓库内部文件不能进入父仓库状态：{parent_status}"
    );
}

#[tokio::test]
async fn concurrent_syncs_and_mixed_commit_calls_never_overlap_and_disconnect_does_not_cancel() {
    let dir = TempDir::new("zone sync concurrency");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    write(&root, "local.txt", "one accepted sync\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "one accepted sync"]);

    let hook_marker = dir.path().join("sync-hook-running.txt");
    let marker_for_shell = hook_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-push",
        &format!("#!/bin/sh\necho started > '{marker_for_shell}'\nsleep 3\n"),
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;

    let first = OpenJsonRequest::submit(
        server.addr(),
        "/api/zones/sync",
        &json!({ "zone": root }).to_string(),
    )
    .await;
    assert!(
        wait_until(Duration::from_secs(10), || hook_marker.exists()).await,
        "第一条 Sync 必须真的进入 push 执行"
    );

    // 同一仓库的第二个 Sync（经路径别名）与混合 Commit 都必须立即得到忙碌结果。
    let alias = PathBuf::from(format!("{}\\", root.display()));
    let second = submit_sync(&server, &alias).await;
    assert_eq!(second.status, 409, "Sync/Sync 不能重叠：{}", second.body);
    assert_eq!(body(&second)["code"], json!("git-write-in-progress"));
    let commit = post_json_over_tcp(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": root,
            "message": "must not overlap",
            "reviewToken": status["reviewToken"],
        })
        .to_string(),
    )
    .await;
    assert_eq!(commit.status, 409, "Commit/Sync 不能重叠：{}", commit.body);
    assert_eq!(body(&commit)["code"], json!("git-write-in-progress"));

    // 客户端断开不取消已接受的 Sync，也不会重放。
    first.abort();
    let expected = head_oid(&root);
    assert!(
        wait_until(Duration::from_secs(15), || {
            remote_head(&remote, "feature") == expected
        })
        .await,
        "断线不能取消已接受的 Sync"
    );
    assert_eq!(
        remote_head(&remote, "feature"),
        expected,
        "断线后的 Sync 不能被自动重放"
    );
    assert_eq!(
        remote_head(&remote, "feature"),
        head_oid(&root),
        "同步只应发生一次"
    );

    // 执行结束后资格释放：新的 Commit 可以执行。
    write(&root, "tracked.txt", "after sync\n");
    let status = git_status(&server, &root).await;
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": root,
            "message": "commit after sync",
            "reviewToken": status["reviewToken"],
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "Sync 结束后应恢复仓库资格：{}",
        response.body
    );
}

#[tokio::test]
async fn configured_timeout_stops_the_running_command_and_keeps_effective_pull_state() {
    let dir = TempDir::new("zone sync timeout");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    advance_remote(
        &remote,
        &dir.path().join("advance"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    write(&root, "local.txt", "local\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "local work"]);
    let remote_before = remote_head(&remote, "feature");

    write_hook(
        &root,
        "pre-push",
        "#!/bin/sh\necho 'push hook started' >&2\n( sleep 8; echo child-alive > child-marker.txt ) &\nsleep 8\n",
    );

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.git.command_timeout_seconds =
        herdr_bot::CommandTimeoutSeconds::new(1).expect("测试时限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;

    let started = std::time::Instant::now();
    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 504,
        "超时必须明确是本功能时限：{}",
        response.body
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "超时必须按配置结束，而不是无限等待"
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-timeout"));
    let message = payload["message"].as_str().expect("message 字符串");
    assert!(
        message.contains("本功能配置的 1 秒执行上限"),
        "页面必须标明本功能超时：{message}"
    );
    assert!(
        message.contains("pull") || message.contains("不会被自动撤销"),
        "必须保留已完成步骤的事实：{message}"
    );

    // pull 已生效，push 超时未产生远端更新；已生效的本地状态不被回滚。
    assert_eq!(
        remote_head(&remote, "feature"),
        remote_before,
        "push 超时不能更新远端"
    );
    assert_ne!(head_oid(&root), remote_before);
    assert!(
        git_stdout(&root, &["show", "HEAD:remote.txt"]).contains("remote"),
        "pull 已生效的内容不能被超时回滚"
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        !root.join("child-marker.txt").exists(),
        "超时必须终止 Git 的派生执行"
    );

    // 终止确认后移除阻塞 hook，Sync 恢复。
    fs::remove_file(root.join(".git").join("hooks").join("pre-push")).expect("移除 hook");
    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 200,
        "确认终止后必须恢复仓库资格：{}",
        response.body
    );
    assert_eq!(remote_head(&remote, "feature"), head_oid(&root));
}

#[tokio::test]
async fn sync_runs_noninteractively_and_does_not_change_repository_configuration() {
    let dir = TempDir::new("zone sync noninteractive");
    let root = dir.path().join("repository");
    init_with_remote(&root, "upstream", "feature");
    // 一个必然连接失败的 HTTPS 远端：若允许交互提示，命令会等待输入；本功能必须快速失败。
    git_ok(
        &root,
        &[
            "remote",
            "set-url",
            "upstream",
            "https://127.0.0.1:9/unreachable.git",
        ],
    );
    git_ok(&root, &["config", "credential.helper", ""]);
    let config_before = git_stdout(&root, &["config", "--local", "--list"]);

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.git.command_timeout_seconds =
        herdr_bot::CommandTimeoutSeconds::new(20).expect("测试时限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;

    let started = std::time::Instant::now();
    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 500,
        "不可达远端必须明确失败：{}",
        response.body
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "凭据/网络条件必须立即失败或按超时结束，不能打开交互流程"
    );
    let message = body(&response)["message"]
        .as_str()
        .expect("message 字符串")
        .to_string();
    assert!(
        message.contains("fetch"),
        "失败必须指出 fetch 步骤：{message}"
    );
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        config_before,
        "不能为了成功而修改 Git 配置"
    );
    let after = git_status(&server, &root).await;
    assert_eq!(after["conflict"], json!(false), "网络失败不能误标冲突");
}

#[tokio::test]
async fn external_branch_switch_is_reflected_and_sync_targets_the_current_branch() {
    let dir = TempDir::new("zone sync branch switch");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "main");
    let main_head = head_oid(&root);
    let main_remote_head = remote_head(&remote, "main");

    git_ok(&root, &["checkout", "--quiet", "-b", "feature"]);
    write(&root, "feature.txt", "feature work\n");
    git_ok(&root, &["add", "feature.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "feature work"]);
    git_ok(&root, &["push", "--quiet", "-u", "upstream", "feature"]);
    let feature_head = head_oid(&root);
    git_ok(&root, &["checkout", "--quiet", "main"]);
    assert_eq!(head_oid(&root), main_head);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    assert_eq!(status["branch"], json!("main"));
    assert_eq!(status["sync"]["upstream"]["short"], json!("upstream/main"));

    // 真实外部切分支：刷新后页面事实跟随当前分支，旧分支的检查记录不再冒充新分支。
    git_ok(&root, &["checkout", "--quiet", "feature"]);
    let status = git_status(&server, &root).await;
    assert_eq!(status["branch"], json!("feature"), "{status}");
    assert_eq!(
        status["sync"]["upstream"]["short"],
        json!("upstream/feature"),
        "{status}"
    );
    assert_eq!(
        status["sync"]["remoteCheckedAt"],
        Value::Null,
        "另一个 upstream 的检查记录不能冒充当前分支已检查：{status}"
    );

    let response = submit_sync(&server, &root).await;
    assert_eq!(response.status, 200, "Sync 应成功：{}", response.body);
    let accepted = body(&response);
    assert_eq!(accepted["branch"], json!("feature"), "{accepted}");
    assert_eq!(accepted["upstream"]["short"], json!("upstream/feature"));

    assert_eq!(
        remote_head(&remote, "feature"),
        feature_head,
        "Sync 必须针对重新确认的当前分支"
    );
    assert_eq!(
        remote_head(&remote, "main"),
        main_remote_head,
        "Sync 不能改写另一分支的远端引用"
    );
    assert_eq!(head_oid(&root), feature_head);
    assert_eq!(
        git_stdout(&root, &["symbolic-ref", "--short", "HEAD"]).trim(),
        "feature"
    );
}

#[tokio::test]
async fn zone_git_qualification_does_not_mix_with_fixed_operation_execution() {
    let fixtures = FixtureTree::new("zone sync independent qualification");
    let script = fixtures.script("long-running.bat");
    let mut config = common::config(&script, &fixtures.operations());
    let registry = fixtures.root().join("zones.json");
    config.zones.registry_file = registry;
    let root = fixtures.root().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    write(&root, "local.txt", "pending while script runs\n");
    git_ok(&root, &["add", "local.txt"]);
    git_ok(
        &root,
        &["commit", "--quiet", "-m", "pending while script runs"],
    );

    let server = TestServer::start(&config).await;
    register(&server, &root).await;

    let fixed = tokio::spawn({
        let addr = server.addr();
        async move { post_over_tcp(addr, "/api/operations/start").await }
    });
    assert!(
        wait_until(Duration::from_secs(10), || {
            fixtures
                .records()
                .iter()
                .any(|record| record.stage == "invoke")
        })
        .await,
        "固定操作必须真正开始执行"
    );

    let response = submit_sync(&server, &root).await;
    assert_eq!(
        response.status, 200,
        "zone Git 资格不能与固定操作的执行资格混同：{}",
        response.body
    );
    assert_eq!(remote_head(&remote, "feature"), head_oid(&root));

    fixtures.stop_fixtures();
    let _ = fixed.await;
}

#[tokio::test]
async fn open_request_surface_and_missing_zone_are_rejected_without_writes() {
    let dir = TempDir::new("zone sync request surface");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "origin", "main");
    let remote_before = remote_head(&remote, "main");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/sync?zone=x",
        &json!({ "zone": root }).to_string(),
    )
    .await;
    assert_eq!(response.status, 400, "查询参数必须被拒绝");
    assert_eq!(body(&response)["code"], json!("unexpected-parameters"));

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/sync",
        &json!({ "zone": root, "extra": true }).to_string(),
    )
    .await;
    assert_eq!(response.status, 400, "未知字段必须被拒绝");

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/check-remote",
        &json!({ "zone": "   " }).to_string(),
    )
    .await;
    assert_eq!(response.status, 400, "空 zone 必须被拒绝");
    assert_eq!(body(&response)["code"], json!("zone-required"));

    let missing = dir.path().join("missing");
    let response = submit_sync(&server, &missing).await;
    assert_eq!(response.status, 404, "未登记目录必须被拒绝");
    assert_eq!(body(&response)["code"], json!("zone-not-registered"));

    assert_eq!(remote_head(&remote, "main"), remote_before);
}

async fn commit_reviewed(server: &TestServer, root: &Path) -> HttpResponse {
    let status = git_status(server, root).await;
    post_json_over_tcp(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": root, "message": "application commit", "reviewToken": status["reviewToken"],
        })
        .to_string(),
    )
    .await
}

async fn wait_for_post_commit_check(server: &TestServer, root: &Path) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        // Ordinary reads must learn completion through invalidation, not an explicit forced refresh.
        let url = format!(
            "/api/zones/git-status?zone={}",
            encode(&root.to_string_lossy())
        );
        let response = get_over_tcp(server.addr(), &url).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let status = body(&response);
        if status["sync"]["remoteChecking"] == json!(false) {
            return status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "remote check did not finish: {status}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

struct FetchRelease(PathBuf);
impl Drop for FetchRelease {
    fn drop(&mut self) {
        if !self.0.exists() {
            let _ = fs::write(&self.0, "release");
        }
    }
}

#[tokio::test]
async fn successful_app_commit_returns_before_fetch_and_publishes_shared_remote_facts() {
    let dir = TempDir::new("zone post commit background");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    let remote_advanced = advance_remote(
        &remote,
        &dir.path().join("remote writer"),
        "feature",
        "remote.txt",
        "remote\n",
    );
    let release = dir.path().join("release-fetch");
    let _release_on_drop = FetchRelease(release.clone());
    let helper = dir.path().join("upload-pack.sh");
    let release_shell = release
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    fs::write(&helper, format!("#!/bin/sh\nwhile [ ! -f '{release_shell}' ]; do sleep 0.05; done\nexec git-upload-pack \"$@\"\n")).unwrap();
    let helper_shell = helper
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    git_ok(
        &root,
        &[
            "config",
            "remote.upstream.uploadpack",
            &format!("sh '{helper_shell}'"),
        ],
    );
    write(&root, "tracked.txt", "local application change\n");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let result =
        tokio::time::timeout(Duration::from_secs(8), commit_reviewed(&server, &root)).await;
    if result.is_err() {
        fs::write(&release, "release").unwrap();
    }
    let response = result.expect("commit response must not await a blocked network check");
    assert_eq!(response.status, 200, "{}", response.body);

    let checking = git_status(&server, &root).await;
    assert_eq!(checking["pendingCount"], json!(0));
    assert_eq!(
        checking["sync"]["remoteChecking"],
        json!(true),
        "{checking}"
    );
    assert_eq!(checking["sync"]["ahead"], json!(1), "{checking}");
    let overview = body(&get_over_tcp(server.addr(), "/api/zones/overview").await);
    assert_eq!(
        overview["zones"][0]["sync"]["remoteChecking"],
        json!(true),
        "{overview}"
    );
    let busy = submit_sync(&server, &root).await;
    assert_eq!(busy.status, 409, "{}", busy.body);
    assert_eq!(body(&busy)["code"], json!("git-write-in-progress"));
    let busy_commit = commit_reviewed(&server, &root).await;
    assert_eq!(busy_commit.status, 409, "{}", busy_commit.body);
    assert_eq!(body(&busy_commit)["code"], json!("git-write-in-progress"));

    fs::write(&release, "release").unwrap();
    let checked = wait_for_post_commit_check(&server, &root).await;
    assert_eq!(
        checked["sync"]["remoteCheckError"],
        Value::Null,
        "{checked}"
    );
    assert!(
        checked["sync"]["remoteCheckedAt"].as_u64().unwrap_or(0) > 0,
        "{checked}"
    );
    assert_eq!(checked["sync"]["ahead"], json!(1), "{checked}");
    assert_eq!(checked["sync"]["behind"], json!(1), "{checked}");
    assert_eq!(
        remote_tracking_head(&root, "upstream", "feature"),
        remote_advanced
    );
    assert_eq!(
        remote_head(&remote, "feature"),
        remote_advanced,
        "background check never pushes"
    );
    let overview = body(&get_over_tcp(server.addr(), "/api/zones/overview").await);
    assert_eq!(overview["zones"][0]["sync"], checked["sync"], "{overview}");
}

#[tokio::test]
async fn post_commit_fetch_failure_preserves_commit_and_local_known_counts() {
    let dir = TempDir::new("zone post commit failed fetch");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root, "upstream", "feature");
    let remote_before = remote_head(&remote, "feature");
    git_ok(
        &root,
        &[
            "remote",
            "set-url",
            "upstream",
            dir.path().join("missing.git").to_str().unwrap(),
        ],
    );
    write(&root, "tracked.txt", "committed despite fetch failure\n");
    let server = TestServer::start(&config_with_registry(&dir.path().join("zones.json"))).await;
    register(&server, &root).await;
    let response = commit_reviewed(&server, &root).await;
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(body(&response)["outcome"], json!("committed"));
    let checked = wait_for_post_commit_check(&server, &root).await;
    assert_eq!(checked["pendingCount"], json!(0), "{checked}");
    assert_eq!(checked["sync"]["ahead"], json!(1), "{checked}");
    assert_eq!(checked["sync"]["behind"], json!(0), "{checked}");
    assert!(
        checked["sync"]["remoteCheckError"]
            .as_str()
            .is_some_and(|error| error.contains("fetch")),
        "{checked}"
    );
    assert_eq!(remote_head(&remote, "feature"), remote_before);
    assert_eq!(body(&response)["commit"], json!(head_oid(&root)));
}

#[tokio::test]
async fn post_commit_without_upstream_explains_unavailable_check_without_configuring_or_publishing()
{
    let dir = TempDir::new("zone post commit missing upstream");
    let root = dir.path().join("repository");
    init_repository(&root, "main");
    write(&root, "tracked.txt", "first application commit\n");
    let config_before = git_stdout(&root, &["config", "--local", "--list"]);
    let server = TestServer::start(&config_with_registry(&dir.path().join("zones.json"))).await;
    register(&server, &root).await;
    let response = commit_reviewed(&server, &root).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let checked = wait_for_post_commit_check(&server, &root).await;
    assert_eq!(checked["sync"]["upstream"], Value::Null, "{checked}");
    assert_eq!(checked["sync"]["ahead"], Value::Null, "{checked}");
    assert!(
        checked["sync"]["remoteCheckError"]
            .as_str()
            .is_some_and(|error| error.contains("upstream")),
        "{checked}"
    );
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        config_before
    );
}
