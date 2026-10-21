//! 经审查复核的整仓提交（S2）：真实 HTTP 请求 + 隔离的真实 Git 仓库、bare 远端与 hooks。
//!
//! 期望值来自本契约与独立写定的文件内容、Git 引用和提交树，不调用实现内部的审查/提交计算。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::{
    HttpResponse, OpenJsonRequest, TempDir, TestServer, get_over_tcp, post_json_over_tcp, wait_until,
};
use serde_json::{Value, json};

/// 与交付配置同形，但把 zone 登记文件放到隔离临时目录。
fn config_with_registry(registry: &Path) -> omp_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config
}

/// 编码查询串（Windows 路径包含空格、反斜杠与盘符）。
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
        "/api/zones/git-status?zone={}",
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

fn init_repository(root: &Path) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git_ok(root, &["init", "--quiet", "--initial-branch=main"]);
    git_ok(root, &["config", "user.name", "zone-commit-test"]);
    git_ok(root, &["config", "user.email", "commit@example.test"]);
}

fn write(root: &Path, relative: &str, content: impl AsRef<[u8]>) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录");
    }
    fs::write(path, content).expect("写入测试文件");
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

async fn git_status(server: &TestServer, zone: &Path) -> Value {
    let response = get_over_tcp(server.addr(), &status_url(zone)).await;
    assert_eq!(
        response.status, 200,
        "读取 Git 状态应成功：{}",
        response.body
    );
    body(&response)
}

fn review_token(status: &Value) -> String {
    status["reviewToken"]
        .as_str()
        .expect("Git 仓库的审查结果必须带 reviewToken")
        .to_string()
}

async fn submit_commit(
    server: &TestServer,
    zone: &Path,
    message: &str,
    token: &str,
) -> HttpResponse {
    post_json_over_tcp(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": zone,
            "message": message,
            "reviewToken": token,
        })
        .to_string(),
    )
    .await
}

fn head_oid(root: &Path) -> String {
    git_stdout(root, &["rev-parse", "HEAD"]).trim().to_string()
}

fn tree_paths(root: &Path) -> Vec<String> {
    git_stdout(root, &["ls-tree", "-r", "--name-only", "HEAD"])
        .lines()
        .map(|line| line.to_string())
        .collect()
}

fn index_snapshot(root: &Path) -> String {
    git_stdout(root, &["diff", "--cached", "--binary"])
}

fn write_hook(root: &Path, name: &str, script: &str) {
    let hooks = root.join(".git").join("hooks");
    fs::create_dir_all(&hooks).expect("创建 hooks 目录");
    let path = hooks.join(name);
    fs::write(&path, script.replace("\r\n", "\n")).expect("写入 hook");
}

/// 建立一个已推送一次提交的本地仓库与 bare 远端。
fn init_with_remote(root: &Path) -> PathBuf {
    init_repository(root);
    write(root, "tracked.txt", "base\n");
    git_ok(root, &["add", "--all"]);
    git_ok(root, &["commit", "--quiet", "-m", "initial"]);

    let remote = root.with_extension("remote.git");
    let output = Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch=main"])
        .arg(&remote)
        .output()
        .expect("创建 bare 远端");
    assert!(output.status.success(), "创建 bare 远端必须成功");
    git_ok(
        root,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("远端路径必须是 UTF-8"),
        ],
    );
    git_ok(root, &["push", "--quiet", "-u", "origin", "main"]);
    remote
}

fn remote_head(remote: &Path) -> String {
    Command::new("git")
        .arg("--git-dir")
        .arg(remote)
        .args(["rev-parse", "main"])
        .output()
        .map(|output| {
            assert!(output.status.success(), "读取远端 main 必须成功");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        })
        .expect("运行 git")
}

#[tokio::test]
async fn commit_stages_all_eligible_final_changes_and_never_pushes() {
    let dir = TempDir::new("zone commit all changes");
    let root = dir.path().join("repository");
    let remote = init_with_remote(&root);
    let remote_before = remote_head(&remote);

    write(&root, "tracked-ignored.log", "tracked before ignore\n");
    write(&root, "tracked-kept.log", "tracked kept before ignore\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "track ignore files"]);

    // 已跟踪修改、已跟踪删除、后来匹配忽略规则的已跟踪文件、忽略规则自身修改、
    // 被忽略的未跟踪、未忽略的未跟踪。
    write(&root, "tracked.txt", "final tracked\n");
    write(&root, "tracked-kept.log", "tracked kept final\n");
    fs::remove_file(root.join("tracked-ignored.log")).expect("删除已跟踪文件");
    write(&root, ".gitignore", "*.log\n");
    write(&root, "ignored.log", "never commit me\n");
    write(&root, "added.txt", "brand new\n");
    // 外部已有的部分暂存不得缩小整仓范围。
    git_ok(&root, &["add", "added.txt"]);
    let index_before = git_stdout(&root, &["diff", "--cached", "--name-only"]);
    assert!(index_before.contains("added.txt"));

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let changes = status["changes"].as_array().expect("changes 数组");
    let mut paths: Vec<&str> = changes
        .iter()
        .map(|change| change["path"].as_str().expect("变更 path"))
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            ".gitignore",
            "added.txt",
            "tracked-ignored.log",
            "tracked-kept.log",
            "tracked.txt",
        ],
        "待提交集合必须包含已跟踪变更与未忽略的未跟踪文件，不包含 ignored 文件：{status}"
    );
    assert_eq!(
        status["blockedReason"], Value::Null,
        "普通待提交状态不应被标记为阻塞：{status}"
    );

    let response = submit_commit(&server, &root, "reviewed whole repository", &token).await;
    assert_eq!(
        response.status, 200,
        "复核通过后应创建提交：{}",
        response.body
    );
    let accepted = body(&response);
    assert_eq!(accepted["outcome"], json!("committed"));
    assert_eq!(accepted["message"], json!("reviewed whole repository"));
    assert_eq!(accepted["commit"], json!(head_oid(&root)));

    assert_eq!(
        git_stdout(&root, &["log", "-1", "--format=%s"]).trim(),
        "reviewed whole repository"
    );
    assert_eq!(
        fs::read_to_string(root.join("tracked.txt")).expect("读取最终内容"),
        "final tracked\n",
        "提交应包含最终本地内容"
    );

    let tree = tree_paths(&root);
    assert!(tree.contains(&"added.txt".to_string()));
    assert!(tree.contains(&".gitignore".to_string()));
    assert!(
        !tree.contains(&"tracked-ignored.log".to_string()),
        "已跟踪删除必须进入提交树"
    );
    assert_eq!(
        git_stdout(&root, &["show", "HEAD:tracked-kept.log"]),
        "tracked kept final\n",
        "已经跟踪、后来匹配忽略规则的文件仍然进入提交树"
    );
    assert!(
        !tree.contains(&"ignored.log".to_string()),
        "被忽略的未跟踪文件不能被强加进提交树：{tree:?}"
    );

    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        "",
        "提交后工作树应干净"
    );
    assert_eq!(
        remote_head(&remote),
        remote_before,
        "Commit 不能自动 push，远端引用必须保持不变"
    );
    assert_eq!(
        index_snapshot(&root),
        "",
        "提交后索引不应残留未提交的暂存内容"
    );
}

#[tokio::test]
async fn review_invalidates_same_path_same_count_content_change_without_writing() {
    let dir = TempDir::new("zone commit content invalidation");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "same.txt", "one\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    write(&root, "same.txt", "reviewed\n");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let head_before = head_oid(&root);
    let index_before = index_snapshot(&root);

    // 路径与数量完全相同，只有内容变了。
    write(&root, "same.txt", "changed after review\n");
    let response = submit_commit(&server, &root, "must not commit stale review", &token).await;
    assert_eq!(response.status, 409, "失效审查必须被拒绝：{}", response.body);
    assert_eq!(body(&response)["code"], json!("review-invalid"));
    assert_eq!(head_oid(&root), head_before, "失效审查不能创建提交");
    assert_eq!(index_snapshot(&root), index_before, "失效审查不能暂存内容");

    // 重新读取当前事实后可以提交被重新审查的内容。
    let refreshed = git_status(&server, &root).await;
    let head_before_retry = head_oid(&root);
    let response = submit_commit(
        &server,
        &root,
        "must not commit stale review",
        &review_token(&refreshed),
    )
    .await;
    assert_eq!(response.status, 200, "重新审查后应可提交：{}", response.body);
    assert_ne!(
        head_oid(&root),
        head_before_retry,
        "重新审查后的提交必须创建新的 HEAD"
    );
    assert_eq!(
        fs::read_to_string(root.join("same.txt")).expect("读取文件"),
        "changed after review\n"
    );
}

#[tokio::test]
async fn external_staging_head_and_branch_changes_invalidate_the_review() {
    let dir = TempDir::new("zone commit staging head branch");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    // 外部暂存：最终内容不变，但索引状态变了。
    write(&root, "file.txt", "modified\n");
    let reviewed = git_status(&server, &root).await;
    let token = review_token(&reviewed);
    let head_before = head_oid(&root);
    git_ok(&root, &["add", "file.txt"]);
    let response = submit_commit(&server, &root, "external staging", &token).await;
    assert_eq!(response.status, 409, "外部暂存必须使审查失效：{}", response.body);
    assert_eq!(body(&response)["code"], json!("review-invalid"));
    assert_eq!(head_oid(&root), head_before);

    // HEAD 变化。
    let reviewed = git_status(&server, &root).await;
    let token = review_token(&reviewed);
    write(&root, "other.txt", "external commit\n");
    git_ok(&root, &["add", "other.txt"]);
    git_ok(&root, &["commit", "--quiet", "-m", "external head move"]);
    let head_after_external = head_oid(&root);
    let response = submit_commit(&server, &root, "head moved", &token).await;
    assert_eq!(response.status, 409, "HEAD 变化必须使审查失效：{}", response.body);
    assert_eq!(body(&response)["code"], json!("review-invalid"));
    assert_eq!(head_oid(&root), head_after_external);

    // 切分支：旧审查不能向另一分支提交。
    write(&root, "file.txt", "on main\n");
    let reviewed = git_status(&server, &root).await;
    let token = review_token(&reviewed);
    git_ok(&root, &["checkout", "--quiet", "-b", "other"]);
    let other_before = head_oid(&root);
    let response = submit_commit(&server, &root, "wrong branch", &token).await;
    assert_eq!(response.status, 409, "切分支后旧审查必须失效：{}", response.body);
    assert_eq!(head_oid(&root), other_before, "旧审查不能向另一分支提交");
    assert_eq!(
        git_stdout(&root, &["symbolic-ref", "--short", "HEAD"]).trim(),
        "other"
    );
}

#[tokio::test]
async fn review_hashes_full_content_beyond_preview_limits_and_binary() {
    let dir = TempDir::new("zone commit binary invalidation");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "binary.bin", [0u8, 1, 2, 3, 4, 5, 6, 7]);
    write(&root, "large.txt", b"0123456789abcdef".to_vec());
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    write(&root, "binary.bin", [0u8, 9, 8, 7, 6, 5, 4, 3]);
    write(&root, "large.txt", b"0123456789ABCDEF".to_vec());

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.zones.preview_limit_bytes =
        omp_bot::config::PreviewLimitBytes::new(4).expect("测试上限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    let token = review_token(&status);

    // 预览上限内看不到完整内容，但复核必须覆盖完整字节。
    write(&root, "binary.bin", [0u8, 9, 8, 7, 6, 5, 4, 2]);
    write(&root, "large.txt", b"0123456789ABCDEf".to_vec());
    let response = submit_commit(&server, &root, "stale large content", &token).await;
    assert_eq!(
        response.status, 409,
        "超限/二进制内容变化必须使审查失效：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("review-invalid"));
}

#[tokio::test]
async fn empty_message_and_open_request_surface_are_rejected_without_writes() {
    let dir = TempDir::new("zone commit empty message");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending\n");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let head_before = head_oid(&root);

    for message in ["", "   ", "\n\t "] {
        let response = submit_commit(&server, &root, message, &token).await;
        assert_eq!(
            response.status, 400,
            "空提交信息必须被拒绝：{}",
            response.body
        );
        assert_eq!(body(&response)["code"], json!("empty-message"));
    }

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": root,
            "message": "valid",
            "reviewToken": token,
            "extra": true,
        })
        .to_string(),
    )
    .await;
    assert_eq!(response.status, 400, "未知字段必须被拒绝");

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/commit?zone=x",
        &json!({
            "zone": root,
            "message": "valid",
            "reviewToken": token,
        })
        .to_string(),
    )
    .await;
    assert_eq!(response.status, 400, "查询参数必须被拒绝");
    assert_eq!(body(&response)["code"], json!("unexpected-parameters"));

    assert_eq!(head_oid(&root), head_before, "被拒绝的请求不能产生提交");
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        " M file.txt\n",
        "被拒绝的请求不能改变索引"
    );
}

#[tokio::test]
async fn hook_rejection_keeps_the_staged_index_and_reports_git_output() {
    let dir = TempDir::new("zone commit hook rejection");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending\n");
    write(&root, "added.txt", "new\n");

    write_hook(
        &root,
        "pre-commit",
        "#!/bin/sh\necho 'hook rejected <script>window.__pwned = 1</script>' >&2\nexit 1\n",
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let head_before = head_oid(&root);

    let response = submit_commit(&server, &root, "must be rejected by hook", &token).await;
    assert_eq!(response.status, 500, "hook 拒绝必须明确失败：{}", response.body);
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-command-failed"));
    assert!(
        payload["message"]
            .as_str()
            .expect("message 字符串")
            .contains("window.__pwned = 1"),
        "页面必须拿到 Git 返回的纯文本诊断：{payload}"
    );
    assert_eq!(head_oid(&root), head_before, "hook 拒绝不能创建提交");

    let staged = git_stdout(&root, &["diff", "--cached", "--name-only"]);
    assert!(
        staged.contains("file.txt") && staged.contains("added.txt"),
        "暂存已经成功，不能在 commit 失败时自动回滚：{staged}"
    );
    assert!(
        fs::read_to_string(root.join("file.txt"))
            .expect("读取文件")
            .contains("pending"),
        "工作树内容不能被失败流程改写"
    );
    let after = git_status(&server, &root).await;
    assert_eq!(
        after["conflict"],
        json!(false),
        "普通 hook 失败不能被误标为文件冲突：{after}"
    );
    assert_eq!(
        after["blockedReason"],
        Value::Null,
        "普通失败不产生阻塞原因：{after}"
    );
}

#[tokio::test]
async fn first_commit_uses_empty_tree_and_commits_staged_and_untracked_files() {
    let dir = TempDir::new("zone commit unborn head");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, ".gitignore", "*.log\n");
    write(&root, "staged.txt", "staged before first commit\n");
    git_ok(&root, &["add", "staged.txt"]);
    write(&root, "untracked.txt", "untracked before first commit\n");
    write(&root, "ignored.log", "must stay ignored\n");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    assert_eq!(status["repository"], json!(true));
    assert_eq!(status["blockedReason"], Value::Null);
    let changes = status["changes"].as_array().expect("changes 数组");
    assert_eq!(changes.len(), 3, "忽略文件不进入待提交集合：{status}");
    assert!(
        changes.iter().all(|change| change["kind"] == json!("added")),
        "没有 HEAD 时已暂存与未忽略未跟踪文件都按新增展示：{status}"
    );

    let response = submit_commit(
        &server,
        &root,
        "first commit from empty tree",
        &review_token(&status),
    )
    .await;
    assert_eq!(response.status, 200, "首提交应成功：{}", response.body);
    assert_eq!(
        git_stdout(&root, &["log", "-1", "--format=%s"]).trim(),
        "first commit from empty tree"
    );
    let tree = tree_paths(&root);
    assert!(tree.contains(&".gitignore".to_string()));
    assert!(tree.contains(&"staged.txt".to_string()));
    assert!(tree.contains(&"untracked.txt".to_string()));
    assert!(!tree.contains(&"ignored.log".to_string()));
    assert_eq!(
        git_stdout(&root, &["rev-list", "--count", "HEAD"]).trim(),
        "1",
        "首提交必须恰好创建一个提交"
    );
}

#[tokio::test]
async fn plain_directory_detached_head_conflicts_and_unfinished_flows_are_blocked() {
    let dir = TempDir::new("zone commit blocked states");
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).expect("创建普通目录");
    write(&plain, "file.txt", "plain content\n");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &plain).await;

    let status = git_status(&server, &plain).await;
    assert_eq!(status["repository"], json!(false));
    assert_eq!(status["changes"], json!([]));
    let response = submit_commit(&server, &plain, "not a repo", "anything").await;
    assert_eq!(
        response.status, 409,
        "普通目录的直接 Commit 请求必须被拒绝：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("not-a-repository"));
    assert!(!plain.join(".git").exists(), "不能隐式初始化 Git 仓库");

    // detached HEAD：页面与直接请求都阻止。
    let detached = dir.path().join("detached");
    init_repository(&detached);
    write(&detached, "file.txt", "base\n");
    git_ok(&detached, &["add", "--all"]);
    git_ok(&detached, &["commit", "--quiet", "-m", "initial"]);
    git_ok(&detached, &["checkout", "--quiet", "--detach"]);
    write(&detached, "file.txt", "pending\n");
    register(&server, &detached).await;
    let status = git_status(&server, &detached).await;
    assert_eq!(status["detached"], json!(true));
    assert!(
        status["blockedReason"]
            .as_str()
            .expect("阻塞原因")
            .contains("detached HEAD"),
        "detached HEAD 必须明确说明原因：{status}"
    );
    let head_before = head_oid(&detached);
    let response =
        submit_commit(&server, &detached, "detached", &review_token(&status)).await;
    assert_eq!(response.status, 409);
    assert_eq!(body(&response)["code"], json!("commit-blocked"));
    assert_eq!(head_oid(&detached), head_before);

    // 未解决冲突，以及冲突已解决但 merge 流程尚未结束。
    let conflicted = dir.path().join("conflicted");
    init_repository(&conflicted);
    write(&conflicted, "file.txt", "base\n");
    git_ok(&conflicted, &["add", "--all"]);
    git_ok(&conflicted, &["commit", "--quiet", "-m", "base"]);
    git_ok(&conflicted, &["checkout", "--quiet", "-b", "feature"]);
    write(&conflicted, "file.txt", "feature\n");
    git_ok(&conflicted, &["commit", "--quiet", "-am", "feature"]);
    git_ok(&conflicted, &["checkout", "--quiet", "main"]);
    write(&conflicted, "file.txt", "main\n");
    git_ok(&conflicted, &["commit", "--quiet", "-am", "main"]);
    let merge = git(&conflicted, &["merge", "feature"]);
    assert!(!merge.status.success(), "合并必须产生冲突");

    register(&server, &conflicted).await;
    let status = git_status(&server, &conflicted).await;
    assert_eq!(status["conflict"], json!(true));
    assert!(
        status["blockedReason"]
            .as_str()
            .expect("阻塞原因")
            .contains("冲突"),
        "冲突必须明确阻止普通 Commit：{status}"
    );
    let response = submit_commit(
        &server,
        &conflicted,
        "conflicted commit",
        &review_token(&status),
    )
    .await;
    assert_eq!(response.status, 409);
    assert_eq!(body(&response)["code"], json!("commit-blocked"));

    // 解决文件冲突，但 merge 还没有结束：仍然阻止。
    write(&conflicted, "file.txt", "resolved\n");
    git_ok(&conflicted, &["add", "file.txt"]);
    let status = git_status(&server, &conflicted).await;
    assert_eq!(status["conflict"], json!(false), "冲突文件已解决：{status}");
    assert!(
        status["blockedReason"]
            .as_str()
            .expect("阻塞原因")
            .contains("merge"),
        "流程尚未结束仍必须阻止：{status}"
    );
    let response = submit_commit(
        &server,
        &conflicted,
        "mid merge commit",
        &review_token(&status),
    )
    .await;
    assert_eq!(response.status, 409);
    assert_eq!(body(&response)["code"], json!("commit-blocked"));

    // 外部撤销流程后，刷新恢复可提交状态。
    git_ok(&conflicted, &["merge", "--abort"]);
    write(&conflicted, "ordinary.txt", "ordinary change\n");
    let status = git_status(&server, &conflicted).await;
    assert_eq!(status["blockedReason"], Value::Null);
    let response = submit_commit(
        &server,
        &conflicted,
        "after external abort",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "外部完成/撤销流程后应恢复：{}",
        response.body
    );
}

#[tokio::test]
async fn nested_repositories_and_submodules_do_not_leak_internal_files() {
    let dir = TempDir::new("zone commit nested repositories");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "top.txt", "top\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    // 一个真实子模块：父仓库只跟踪 gitlink。
    let sub_source = dir.path().join("sub-source");
    init_repository(&sub_source);
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
    git_ok(&root, &["commit", "--quiet", "-m", "add submodule"]);

    let sub = root.join("sub");
    write(&sub, "moved.txt", "sub next\n");
    git_ok(&sub, &["add", "--all"]);
    git_ok(&sub, &["commit", "--quiet", "-m", "sub next"]);
    let sub_head = git_stdout(&sub, &["rev-parse", "HEAD"]).trim().to_string();
    write(&sub, "dirty-only.txt", "uncommitted inside submodule\n");

    // 一个未跟踪的嵌套仓库：父仓库只能记录其 gitlink，不能把内部文件当普通内容。
    let embedded = root.join("embedded");
    init_repository(&embedded);
    write(&embedded, "inner.txt", "must not enter parent\n");
    git_ok(&embedded, &["add", "--all"]);
    git_ok(&embedded, &["commit", "--quiet", "-m", "embedded"]);
    let embedded_head = git_stdout(&embedded, &["rev-parse", "HEAD"])
        .trim()
        .to_string();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let mut paths: Vec<&str> = status["changes"]
        .as_array()
        .expect("changes 数组")
        .iter()
        .map(|change| change["path"].as_str().expect("path"))
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec!["embedded/", "sub"],
        "子模块内部脏文件不产生变更；父仓库只看到 gitlink 与嵌入仓库：{status}"
    );

    let response = submit_commit(
        &server,
        &root,
        "nested scope",
        &review_token(&status),
    )
    .await;
    assert_eq!(response.status, 200, "提交应成功：{}", response.body);

    let sub_entry = git_stdout(&root, &["ls-tree", "HEAD", "sub"]);
    assert!(
        sub_entry.starts_with("160000 commit "),
        "父仓库必须记录子模块的 gitlink：{sub_entry}"
    );
    assert!(
        sub_entry.contains(&sub_head),
        "更新后的 gitlink 必须指向子模块当前 HEAD：{sub_entry}"
    );
    let embedded_entry = git_stdout(&root, &["ls-tree", "HEAD", "embedded"]);
    assert!(
        embedded_entry.starts_with("160000 commit "),
        "嵌套仓库必须作为 gitlink 记录：{embedded_entry}"
    );
    assert!(embedded_entry.contains(&embedded_head));

    let tree = tree_paths(&root);
    assert!(
        !tree.iter().any(|path| path.starts_with("sub/")),
        "子模块内部文件不能作为父仓库普通内容提交：{tree:?}"
    );
    assert!(
        !tree.iter().any(|path| path.starts_with("embedded/")),
        "嵌套仓库内部文件不能作为父仓库普通内容提交：{tree:?}"
    );
    assert!(
        fs::read_to_string(sub.join("dirty-only.txt"))
            .expect("子模块内部文件仍在")
            .contains("uncommitted"),
        "提交父仓库不能改写子模块内部工作树"
    );
    assert_eq!(
        git_stdout(&sub, &["status", "--porcelain=v1"]),
        "?? dirty-only.txt\n",
        "子模块内部未提交变化必须保持原样"
    );
    assert_eq!(
        git_stdout(&embedded, &["rev-parse", "HEAD"]).trim(),
        embedded_head,
        "嵌套仓库自身不能被提交"
    );
}

#[tokio::test]
async fn concurrent_commits_do_not_overlap_and_disconnect_does_not_cancel() {
    let dir = TempDir::new("zone commit concurrency");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending once\n");
    let hook_marker = dir.path().join("hook-running.txt");
    let hook_marker_for_shell = hook_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-commit",
        &format!("#!/bin/sh\necho started > '{hook_marker_for_shell}'\nsleep 2\n"),
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let request = json!({
        "zone": root,
        "message": "one accepted commit",
        "reviewToken": token,
    })
    .to_string();

    let first = OpenJsonRequest::submit(server.addr(), "/api/zones/commit", &request).await;
    assert!(
        wait_until(Duration::from_secs(10), || hook_marker.exists()).await,
        "第一条提交必须真的进入执行"
    );

    // 第一条仍在执行：同一仓库的第二条请求立即被拒绝，不等待、不排队。
    let second = submit_commit(
        &server,
        &root,
        "overlapping commit",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        second.status, 409,
        "同一仓库的写操作不能重叠：{}",
        second.body
    );
    assert_eq!(body(&second)["code"], json!("git-write-in-progress"));

    // 客户端断开观察连接：已接受的提交继续完成，不在别处重放。
    first.abort();
    assert!(
        wait_until(Duration::from_secs(15), || {
            git_stdout(&root, &["rev-list", "--count", "HEAD"]).trim() == "2"
        })
        .await,
        "断线不能取消已接受的提交"
    );
    assert_eq!(
        git_stdout(&root, &["rev-list", "--count", "HEAD"]).trim(),
        "2",
        "断线后的提交不能被自动重放"
    );
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain=v1"]),
        "",
        "提交必须真实完成"
    );

    // 资格在执行结束后释放：下一条正常提交可以执行。
    write(&root, "file.txt", "pending twice\n");
    let status = git_status(&server, &root).await;
    let response = submit_commit(&server, &root, "second commit", &review_token(&status)).await;
    assert_eq!(
        response.status, 200,
        "执行结束后应恢复仓库资格：{}",
        response.body
    );
    assert_eq!(
        git_stdout(&root, &["rev-list", "--count", "HEAD"]).trim(),
        "3"
    );
}

#[tokio::test]
async fn path_aliases_of_the_same_repository_share_one_write_qualification() {
    let dir = TempDir::new("zone commit path aliases");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending once\n");

    // 同一个实际仓库的另一种写法（结尾分隔符）：登记仍只有一项，请求可以经别名到达。
    let alias = format!("{}\\", root.display());
    let registry = dir.path().join("zones.json");
    fs::write(
        &registry,
        serde_json::to_string(&json!({
            "zones": [{ "path": root.to_string_lossy() }],
        }))
        .expect("序列化登记文件"),
    )
    .expect("写入登记文件");

    let hook_marker = dir.path().join("alias-hook-running.txt");
    let hook_marker_for_shell = hook_marker.to_string_lossy().replace('\\', "/");
    write_hook(
        &root,
        "pre-commit",
        &format!("#!/bin/sh\necho started > '{hook_marker_for_shell}'\nsleep 2\n"),
    );
    let server = TestServer::start(&config_with_registry(&registry)).await;
    let status = git_status(&server, &root).await;
    let token = review_token(&status);

    let alias_path = PathBuf::from(&alias);
    let first = OpenJsonRequest::submit(
        server.addr(),
        "/api/zones/commit",
        &json!({
            "zone": root,
            "message": "through canonical path",
            "reviewToken": token,
        })
        .to_string(),
    )
    .await;
    assert!(
        wait_until(Duration::from_secs(10), || hook_marker.exists()).await,
        "第一条提交必须真的进入执行"
    );

    let second = submit_commit(&server, &alias_path, "through alias", &token).await;
    assert_eq!(
        second.status, 409,
        "路径别名不能绕过同一实际仓库的串行资格：{}",
        second.body
    );
    assert_eq!(body(&second)["code"], json!("git-write-in-progress"));

    first.abort();
    assert!(
        wait_until(Duration::from_secs(15), || {
            git_stdout(&root, &["rev-list", "--count", "HEAD"]).trim() == "2"
        })
        .await,
        "别名的重叠请求被拒绝后，第一条提交仍应完成"
    );
}

#[tokio::test]
async fn configured_timeout_stops_git_and_its_children_then_releases_the_repository() {
    let dir = TempDir::new("zone commit timeout");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending after timeout\n");
    write_hook(
        &root,
        "pre-commit",
        "#!/bin/sh\necho 'hook started' >&2\n( sleep 8; echo child-alive > child-marker.txt ) &\nsleep 8\n",
    );

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.git.command_timeout_seconds =
        omp_bot::CommandTimeoutSeconds::new(1).expect("测试时限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let token = review_token(&status);
    let head_before = head_oid(&root);

    let request = json!({
        "zone": root,
        "message": "blocked hook",
        "reviewToken": token,
    })
    .to_string();
    let pending = tokio::spawn({
        let addr = server.addr();
        let request = request.clone();
        async move { post_json_over_tcp(addr, "/api/zones/commit", &request).await }
    });
    // 等待 commit 真正开始；此时仓库资格被持有。
    assert!(
        wait_until(Duration::from_secs(10), || {
            root.join(".git").join("index.lock").exists()
                || git_stdout(&root, &["diff", "--cached", "--name-only"]).contains("file.txt")
        })
        .await
    );
    let overlapping = submit_commit(&server, &root, "overlap during timeout", &token).await;
    assert_eq!(
        overlapping.status, 409,
        "执行尚未确认停止时不能释放资格：{}",
        overlapping.body
    );

    let response = pending.await.expect("等待超时结果");
    assert_eq!(
        response.status, 504,
        "超时必须明确是本功能时限：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-timeout"));
    let message = payload["message"].as_str().expect("message 字符串");
    assert!(
        message.contains("1 秒执行上限") && message.contains("本功能配置"),
        "页面必须标明本功能超时，而不是伪造 Git 退出消息：{message}"
    );

    assert_eq!(head_oid(&root), head_before, "超时不能创建提交");
    assert!(
        git_stdout(&root, &["diff", "--cached", "--name-only"]).contains("file.txt"),
        "暂存成功后发生超时，不能自动回滚已成功的暂存"
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        !root.join("child-marker.txt").exists(),
        "超时必须终止 Git 的派生执行，背景子进程不应继续写出标记"
    );

    // 终止确认后，移除阻塞 hook 即可恢复正常提交。
    fs::remove_file(root.join(".git").join("hooks").join("pre-commit")).expect("移除 hook");
    let status = git_status(&server, &root).await;
    let response = submit_commit(
        &server,
        &root,
        "after timeout recovery",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "确认终止后必须恢复仓库资格：{}",
        response.body
    );
}

#[tokio::test]
async fn zone_git_qualification_is_independent_of_fixed_operation_execution() {
    let fixtures = common::FixtureTree::new("zone commit independent qualification");
    let script = fixtures.script("long-running.bat");
    let mut config = common::config(&script, &fixtures.operations());
    let registry = fixtures.root().join("zones.json");
    config.zones.registry_file = registry.clone();
    let root = fixtures.root().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "pending while script runs\n");

    let server = TestServer::start(&config).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;

    // 固定操作正在执行：它持有自己的执行资格。
    let fixed = tokio::spawn({
        let addr = server.addr();
        async move { common::post_over_tcp(addr, "/api/operations/start").await }
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

    let response = submit_commit(
        &server,
        &root,
        "commit while fixed operation runs",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "zone Git 资格不能与固定操作的执行资格混同：{}",
        response.body
    );

    fixtures.stop_fixtures();
    let _ = fixed.await;
}

#[tokio::test]
async fn inaccessible_zone_rejects_commit_and_reports_read_failure() {
    let dir = TempDir::new("zone commit missing directory");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    fs::remove_dir_all(&root).expect("外部删除登记目录");
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        response.status, 404,
        "目录消失必须明确报错而不是空状态：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("zone-unavailable"));

    let response = submit_commit(&server, &root, "deleted zone", "old-token").await;
    assert_eq!(response.status, 404);
    assert_eq!(body(&response)["code"], json!("zone-unavailable"));

    // 路径被替换成普通文件同样不是可提交的干净事实。
    fs::write(&root, "not a directory").expect("用文件替换目录");
    let response = submit_commit(&server, &root, "replaced zone", "old-token").await;
    assert_eq!(response.status, 404);
    assert_eq!(body(&response)["code"], json!("zone-unavailable"));
}

#[tokio::test]
async fn missing_upstream_does_not_block_local_commit_and_configuration_is_unchanged() {
    let dir = TempDir::new("zone commit no upstream");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "local only\n");

    let config_before = git_stdout(&root, &["config", "--local", "--list"]);
    assert!(
        git(&root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
            .status
            .success()
            == false,
        "测试仓库不应配置 upstream"
    );

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let response = submit_commit(
        &server,
        &root,
        "local commit without upstream",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "没有 upstream 不妨碍本地 Commit：{}",
        response.body
    );
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        config_before,
        "Commit 不能隐式修改 Git 配置或设置 upstream"
    );
    assert_eq!(git_stdout(&root, &["remote"]), "");
}

#[tokio::test]
async fn editor_and_signing_conditions_fail_or_are_not_invoked_without_overriding_policy() {
    let dir = TempDir::new("zone commit noninteractive");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    // 编辑器不应被打开：`-m` 已经提供信息，页面也不提供终端输入。
    let editor_marker = root.join("editor-marker.txt");
    let editor = dir.path().join("editor.bat");
    fs::write(&editor, "@echo off\r\necho editor-opened > editor-marker.txt\r\n")
        .expect("写入假编辑器");
    git_ok(
        &root,
        &[
            "config",
            "core.editor",
            editor.to_str().expect("UTF-8 路径"),
        ],
    );
    write(&root, "file.txt", "editor must not run\n");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let response = submit_commit(
        &server,
        &root,
        "message supplied up front",
        &review_token(&status),
    )
    .await;
    assert_eq!(response.status, 200, "编辑器不应参与提交：{}", response.body);
    assert!(
        !editor_marker.exists(),
        "提交不能打开交互式编辑器并要求用户输入"
    );

    // 签名条件无法满足：明确失败，不覆盖 Git 配置伪造成功。
    write(&root, "file.txt", "signing must fail\n");
    git_ok(&root, &["config", "commit.gpgsign", "true"]);
    git_ok(
        &root,
        &[
            "config",
            "gpg.program",
            r"C:\definitely-does-not-exist\gpg.exe",
        ],
    );
    let readme = git_stdout(&root, &["config", "--local", "--list"]);
    let head_before = head_oid(&root);
    let status = git_status(&server, &root).await;
    let response = submit_commit(
        &server,
        &root,
        "signed but impossible",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 500,
        "签名条件不满足必须明确失败：{}",
        response.body
    );
    assert_eq!(body(&response)["code"], json!("git-command-failed"));
    assert_eq!(head_oid(&root), head_before, "失败不能创建伪造的提交");
    assert_eq!(
        git_stdout(&root, &["config", "--local", "--list"]),
        readme,
        "不能为了成功而覆盖 Git 签名策略"
    );
}

#[tokio::test]
async fn preview_limitations_do_not_change_commit_eligibility_or_the_real_tree() {
    let dir = TempDir::new("zone commit preview limits");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "base.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    let large = vec![b'x'; 64];
    let binary = [0u8, 159, 146, 150, 0, 1, 2, 3, 4, 5];
    write(&root, "large.txt", &large);
    write(&root, "binary.bin", binary);

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.zones.preview_limit_bytes =
        omp_bot::config::PreviewLimitBytes::new(8).expect("测试上限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;

    let status = git_status(&server, &root).await;
    let mut paths: Vec<&str> = status["changes"]
        .as_array()
        .expect("changes 数组")
        .iter()
        .map(|change| change["path"].as_str().expect("path"))
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec!["binary.bin", "large.txt"],
        "超限与二进制文件仍必须进入待提交集合：{status}"
    );

    let response = submit_commit(
        &server,
        &root,
        "commit limited previews",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "预览限制不能改变提交资格：{}",
        response.body
    );

    let committed_large = git(&root, &["cat-file", "blob", "HEAD:large.txt"]);
    assert!(committed_large.status.success());
    assert_eq!(committed_large.stdout, large, "超限文件必须完整进入提交树");
    let committed_binary = git(&root, &["cat-file", "blob", "HEAD:binary.bin"]);
    assert!(committed_binary.status.success());
    assert_eq!(
        committed_binary.stdout, binary,
        "二进制文件必须按原始字节进入提交树"
    );
}

#[tokio::test]
async fn credential_conditions_do_not_open_an_interactive_prompt() {
    let dir = TempDir::new("zone commit credentials");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);
    write(&root, "file.txt", "credential test\n");

    // 关闭本地凭据 helper；hook 尝试从没有终端输入的凭据接口取用户名。
    // 如果服务端允许交互提示，hook 会挂起并最终按超时失败；非交互环境下它应快速失败。
    git_ok(&root, &["config", "credential.helper", ""]);
    write_hook(
        &root,
        "pre-commit",
        "#!/bin/sh\nif printf 'protocol=https\\nhost=example.invalid\\n\\n' | git credential fill >/dev/null 2>&1; then\n  echo 'unexpected credential' >&2\n  exit 1\nfi\nexit 0\n",
    );

    let registry = dir.path().join("zones.json");
    let mut config = config_with_registry(&registry);
    config.git.command_timeout_seconds =
        omp_bot::CommandTimeoutSeconds::new(20).expect("测试时限必须合法");
    let server = TestServer::start(&config).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;

    let started = std::time::Instant::now();
    let response = submit_commit(
        &server,
        &root,
        "no credential prompt",
        &review_token(&status),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "凭据不可用时不应阻塞或交互提示；hook 已确认非交互失败：{}",
        response.body
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "凭据条件必须立即失败或按超时结束，不能打开交互流程"
    );
}

#[tokio::test]
async fn clean_repository_rejects_direct_commit_without_creating_an_empty_commit() {
    let dir = TempDir::new("zone commit clean");
    let root = dir.path().join("repository");
    init_repository(&root);
    write(&root, "file.txt", "base\n");
    git_ok(&root, &["add", "--all"]);
    git_ok(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    let status = git_status(&server, &root).await;
    let head_before = head_oid(&root);

    let response = submit_commit(&server, &root, "empty", &review_token(&status)).await;
    assert_eq!(response.status, 409, "干净仓库不能创建空提交：{}", response.body);
    assert_eq!(body(&response)["code"], json!("nothing-to-commit"));
    assert_eq!(head_oid(&root), head_before);
}


