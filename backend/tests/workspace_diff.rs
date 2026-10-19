//! workspace 变更审查 diff 的公开 HTTP interface（S2）。
//!
//! 每个用例都经真实 HTTP 请求观察行为，并使用隔离的真实 Git 仓库：旧侧期望来自独立写定的
//! HEAD 内容，新侧期望来自独立写定的最终本地文件；同时独立核对索引语义与文件内容没有因为审查
//! 而改变。边界反例使用真实的根外目录、Windows 目录联接与（可用时）符号链接。

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

fn config_with_registry_and_limit(registry: &Path, preview_limit: u64) -> omp_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.workspaces.registry_file = registry.to_path_buf();
    config.workspaces.preview_limit_bytes =
        omp_bot::config::PreviewLimitBytes::new(preview_limit).expect("测试上限必须合法");
    config
}

fn config_with_registry(registry: &Path) -> omp_bot::Config {
    config_with_registry_and_limit(registry, 1024 * 1024)
}

/// 把值编码进查询串（Windows 路径包含空格、反斜杠与盘符）。
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

fn diff_url(workspace: &Path, path: &str, old_path: Option<&str>) -> String {
    let mut url = format!(
        "/api/workspaces/diff?workspace={}&path={}",
        encode(&workspace.to_string_lossy()),
        encode(path)
    );
    if let Some(old_path) = old_path {
        url.push_str(&format!("&oldPath={}", encode(old_path)));
    }
    url
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body).expect("响应必须是 JSON")
}

async fn register(server: &TestServer, workspace: &Path) {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/workspaces",
        &json!({ "path": workspace }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "登记 workspace 应成功：{}",
        response.body
    );
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

fn git_stdout(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn init_repository(root: &Path) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["config", "user.name", "workspace-diff-test"]);
    git(root, &["config", "user.email", "diff@example.test"]);
}

/// 独立记录索引语义：路径、暂存内容与未暂存内容都必须保持不变。
fn index_semantics(root: &Path) -> (String, String, String) {
    (
        git_stdout(root, &["ls-files", "--stage"]),
        git_stdout(root, &["diff", "--cached", "--binary"]),
        git_stdout(root, &["diff", "--binary"]),
    )
}

fn file_snapshot(root: &Path, names: &[&str]) -> Vec<(String, Vec<u8>)> {
    let mut snapshot = Vec::new();
    for name in names {
        snapshot.push((
            (*name).to_string(),
            fs::read(root.join(name)).expect("读取文件快照"),
        ));
    }
    snapshot
}

fn side<'a>(payload: &'a Value, name: &str) -> &'a Value {
    &payload[name]
}

fn assert_text_side(payload: &Value, name: &str, content: &str, size: usize) {
    let side = side(payload, name);
    assert_eq!(side["kind"], json!("text"), "{name} 必须是文本：{payload}");
    assert_eq!(
        side["content"],
        json!(content),
        "{name} 必须返回独立写定的完整内容"
    );
    assert_eq!(side["size"], json!(size), "{name} 的大小必须准确");
}

fn assert_absent_side(payload: &Value, name: &str) {
    assert_eq!(
        side(payload, name),
        &json!({ "kind": "absent" }),
        "{name} 必须明确表示该侧不存在"
    );
}

fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(
        response.status, status,
        "请求应被拒绝：期望 {status} {code}，实际 {} {}",
        response.status, response.body
    );
    assert_eq!(body(response)["code"], json!(code), "{}", response.body);
}

/// 比较两次请求的响应体，避免把真实文件内容误发到断言消息里；只在失败时展示结构。
fn assert_no_secret(response: &HttpResponse) {
    assert!(
        !response.body.contains("top-secret"),
        "越界请求不能泄露根外内容：{}",
        response.body
    );
}

#[tokio::test]
async fn diff_reads_head_to_final_content_without_touching_index_or_files() {
    let dir = TempDir::new("workspace diff review");
    let root = dir.path().join("repository");
    init_repository(&root);

    // HEAD 的旧文本完全由测试独立写定。
    let head_review = "alpha\nbeta\n";
    let head_deleted = "delete me\n";
    let head_rename = "rename line one\nrename line two\n";
    fs::write(root.join("review.txt"), head_review).unwrap();
    fs::write(root.join("deleted.txt"), head_deleted).unwrap();
    fs::write(root.join("old-name.txt"), head_rename).unwrap();
    fs::write(root.join("untouched.txt"), "untouched\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    // 另造一个与 HEAD 不同的远端跟踪引用：若实现错误地比较远端→本地，旧侧会变成 REMOTE。
    git(&root, &["switch", "--quiet", "-c", "remote-side"]);
    fs::write(root.join("review.txt"), "alpha\nREMOTE\n").unwrap();
    git(&root, &["commit", "--quiet", "-am", "remote side"]);
    let remote_commit = git_stdout(&root, &["rev-parse", "HEAD"]).trim().to_string();
    git(
        &root,
        &["update-ref", "refs/remotes/origin/main", &remote_commit],
    );
    git(&root, &["switch", "--quiet", "main"]);
    let remote_review = git_stdout(&root, &["show", "refs/remotes/origin/main:review.txt"]);
    assert_ne!(
        remote_review, head_review,
        "远端跟踪引用的内容必须与 HEAD 不同，才能区分两种比较基准"
    );

    // 真实部分暂存：索引里是 STAGED 内容，最终本地文件还有一段未暂存内容。
    // 若实现错误地比较 index→工作区，旧侧会变成 STAGED 而不是 HEAD 的 beta。
    let staged_review = "alpha\nSTAGED\n";
    let final_review = "alpha\nSTAGED\nFINAL\n";
    fs::write(root.join("review.txt"), staged_review).unwrap();
    git(&root, &["add", "review.txt"]);
    fs::write(root.join("review.txt"), final_review).unwrap();

    // 新增、删除与 Git 已识别的暂存改名；改名的新路径再有未暂存修改。
    let final_added = "brand new content\n";
    fs::write(root.join("added.txt"), final_added).unwrap();
    fs::remove_file(root.join("deleted.txt")).unwrap();
    git(&root, &["mv", "old-name.txt", "new-name.txt"]);
    let final_rename = format!("{head_rename}unstaged tail\n");
    fs::write(root.join("new-name.txt"), &final_rename).unwrap();

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let snapshots = ["review.txt", "new-name.txt", "untouched.txt", "added.txt"];
    let before_files = file_snapshot(&root, &snapshots);
    let before_index = index_semantics(&root);

    // 修改：旧侧是 HEAD，新侧是最终本地内容（合并已暂存与未暂存）。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "review.txt", None)).await;
    assert_eq!(response.status, 200, "diff 应成功：{}", response.body);
    let payload = body(&response);
    assert_eq!(payload["path"], json!("review.txt"));
    assert_eq!(payload["complete"], json!(true));
    assert_text_side(&payload, "old", head_review, head_review.len());
    assert_text_side(&payload, "new", final_review, final_review.len());
    assert_ne!(
        payload["old"]["content"],
        json!(staged_review),
        "diff 旧侧必须是 HEAD，而不是索引内容"
    );
    assert_ne!(
        payload["old"]["content"],
        json!(remote_review),
        "diff 旧侧必须是 HEAD，而不是远端跟踪引用"
    );

    // 新增：HEAD 没有该路径，旧侧明确不存在；新侧是完整新增文本。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "added.txt", None)).await;
    assert_eq!(response.status, 200, "新增 diff 应成功：{}", response.body);
    let payload = body(&response);
    assert_absent_side(&payload, "old");
    assert_text_side(&payload, "new", final_added, final_added.len());
    assert_eq!(payload["complete"], json!(true));

    // 删除：旧侧仍是 HEAD 内容，新侧明确不存在。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "deleted.txt", None)).await;
    assert_eq!(response.status, 200, "删除 diff 应成功：{}", response.body);
    let payload = body(&response);
    assert_text_side(&payload, "old", head_deleted, head_deleted.len());
    assert_absent_side(&payload, "new");
    assert_eq!(payload["complete"], json!(true));

    // 已识别改名：旧路径来自 Git 的旧路径字段，旧侧必须读取 HEAD 的旧路径。
    let response = get_over_tcp(
        server.addr(),
        &diff_url(&root, "new-name.txt", Some("old-name.txt")),
    )
    .await;
    assert_eq!(response.status, 200, "改名 diff 应成功：{}", response.body);
    let payload = body(&response);
    assert_eq!(payload["path"], json!("new-name.txt"));
    assert_eq!(payload["oldPath"], json!("old-name.txt"));
    assert_text_side(&payload, "old", head_rename, head_rename.len());
    assert_text_side(&payload, "new", &final_rename, final_rename.len());

    assert_eq!(
        file_snapshot(&root, &snapshots),
        before_files,
        "审查 diff 不能改变任何文件内容"
    );
    assert_eq!(
        index_semantics(&root),
        before_index,
        "审查 diff 不能暂存、刷新或改写真实索引语义"
    );
}

#[tokio::test]
async fn empty_repository_compares_against_an_empty_tree() {
    let dir = TempDir::new("workspace diff no head");
    let root = dir.path().join("repository");
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    git(&root, &["config", "user.name", "workspace-diff-test"]);
    git(&root, &["config", "user.email", "diff@example.test"]);

    // 已初始化但没有首次提交：一个已暂存文件与一个未忽略未跟踪文件都属于待提交新增。
    let staged = "staged first content\n";
    let untracked = "untracked first content\n";
    fs::write(root.join("staged.txt"), staged).unwrap();
    git(&root, &["add", "staged.txt"]);
    fs::write(root.join("untracked.txt"), untracked).unwrap();

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    // 待提交集合本身仍然成立，且两项都是新增类别。
    let status = get_over_tcp(
        server.addr(),
        &format!(
            "/api/workspaces/git-status?workspace={}",
            encode(&root.to_string_lossy())
        ),
    )
    .await;
    assert_eq!(status.status, 200, "{}", status.body);
    let status = body(&status);
    assert_eq!(status["repository"], json!(true));
    assert_eq!(status["pendingCount"], json!(2));
    for change in status["changes"].as_array().expect("changes 数组") {
        assert_eq!(change["kind"], json!("added"));
    }

    for (path, content) in [("staged.txt", staged), ("untracked.txt", untracked)] {
        let response = get_over_tcp(server.addr(), &diff_url(&root, path, None)).await;
        assert_eq!(
            response.status, 200,
            "无 HEAD 仓库的 diff 应成功：{}",
            response.body
        );
        let payload = body(&response);
        assert_absent_side(&payload, "old");
        assert_text_side(&payload, "new", content, content.len());
        assert_eq!(
            payload["complete"],
            json!(true),
            "空树比较下新增文件应可完整审查"
        );
    }
}

#[tokio::test]
async fn diff_sides_apply_the_preview_limit_independently() {
    let dir = TempDir::new("workspace diff limits");
    let root = dir.path().join("repository");
    init_repository(&root);

    let exact = "0123456789abcdef0123456789abcdef"; // 32 字节
    let too_large = "0123456789abcdef0123456789abcdefg"; // 33 字节
    assert_eq!(exact.len(), 32);
    assert_eq!(too_large.len(), 33);

    fs::write(root.join("exact.txt"), exact).unwrap();
    fs::write(root.join("old-over.txt"), too_large).unwrap();
    fs::write(root.join("new-over.txt"), "tiny\n").unwrap();
    fs::write(root.join("binary-new.txt"), "plain old\n").unwrap();
    fs::write(root.join("binary-old.txt"), b"\0binary old\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    fs::write(root.join("old-over.txt"), "tiny\n").unwrap();
    fs::write(root.join("new-over.txt"), too_large).unwrap();
    fs::write(root.join("binary-new.txt"), b"\0binary new\n").unwrap();
    fs::write(root.join("binary-old.txt"), "plain new\n").unwrap();

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry_and_limit(&registry, 32)).await;
    register(&server, &root).await;

    // 精确边界：两侧都恰好 32 字节，必须作为完整文本返回。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "exact.txt", None)).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let payload = body(&response);
    assert_text_side(&payload, "old", exact, 32);
    assert_text_side(&payload, "new", exact, 32);
    assert_eq!(payload["complete"], json!(true));

    // 旧侧超限、新侧很小：不返回旧侧内容，也不假装可以渲染完整 diff。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "old-over.txt", None)).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let payload = body(&response);
    assert_eq!(payload["old"]["kind"], json!("too-large"));
    assert_eq!(payload["old"]["size"], json!(33));
    assert_eq!(payload["old"]["limit"], json!(32));
    assert!(
        payload["old"].get("content").is_none(),
        "超限不加载完整内容"
    );
    assert_text_side(&payload, "new", "tiny\n", 5);
    assert_eq!(payload["complete"], json!(false));

    // 旧侧很小、新侧超限：同样不渲染完整 diff。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "new-over.txt", None)).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let payload = body(&response);
    assert_text_side(&payload, "old", "tiny\n", 5);
    assert_eq!(payload["new"]["kind"], json!("too-large"));
    assert_eq!(payload["new"]["size"], json!(33));
    assert_eq!(payload["new"]["limit"], json!(32));
    assert!(payload["new"].get("content").is_none());
    assert_eq!(payload["complete"], json!(false));

    // 任一侧是二进制：给出不可文本 diff 的原因，而不是空内容或伪文本。
    let response = get_over_tcp(server.addr(), &diff_url(&root, "binary-new.txt", None)).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let payload = body(&response);
    assert_text_side(&payload, "old", "plain old\n", 10);
    assert_eq!(payload["new"]["kind"], json!("binary"));
    assert!(
        payload["new"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("NUL")),
        "二进制必须给出明确原因：{payload}"
    );
    assert_eq!(payload["complete"], json!(false));

    let response = get_over_tcp(server.addr(), &diff_url(&root, "binary-old.txt", None)).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let payload = body(&response);
    assert_eq!(payload["old"]["kind"], json!("binary"));
    assert!(
        payload["old"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("NUL")),
        "二进制旧侧必须给出明确原因：{payload}"
    );
    assert_text_side(&payload, "new", "plain new\n", 10);
    assert_eq!(payload["complete"], json!(false));

    // 预览限制不改变待提交集合：发生变化的文件仍然全部可见、计入项数。
    let status = get_over_tcp(
        server.addr(),
        &format!(
            "/api/workspaces/git-status?workspace={}",
            encode(&root.to_string_lossy())
        ),
    )
    .await;
    let status = body(&status);
    let paths: Vec<&str> = status["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .filter_map(|change| change["path"].as_str())
        .collect();
    for path in [
        "old-over.txt",
        "new-over.txt",
        "binary-new.txt",
        "binary-old.txt",
    ] {
        assert!(paths.contains(&path), "{path} 不能因预览不可用而消失");
    }
}

#[tokio::test]
async fn diff_entry_enforces_the_workspace_root_boundary() {
    let dir = TempDir::new("workspace diff boundary");
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "top-secret").unwrap();
    init_repository(&root);
    fs::write(root.join("review.txt"), "inside\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    let junction = root.join("junction-out");
    assert!(create_junction(&junction, &outside), "目录联接必须真实创建");
    let file_link = root.join("filelink-out");
    let file_link_created = try_native_file_symlink(&file_link, &outside.join("secret.txt"))
        || try_wsl_symlink(&file_link, &outside.join("secret.txt"));
    if !file_link_created {
        eprintln!("跳过文件符号链接反例：当前环境无法创建真实符号链接");
    }

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let outside_absolute = outside.join("secret.txt").to_string_lossy().into_owned();

    // 路径遍历与绝对路径：新路径入口拒绝，并且不读取根外内容。
    for path in [
        "../outside/secret.txt",
        "..\\outside\\secret.txt",
        "sub/../../outside/secret.txt",
        outside_absolute.as_str(),
        "C:\\Windows\\win.ini",
        "\\\\server\\share\\file.txt",
    ] {
        let response = get_over_tcp(server.addr(), &diff_url(&root, path, None)).await;
        assert_rejected(&response, 403, "path-outside-workspace");
        assert_no_secret(&response);
    }

    // 旧路径同样属于公开入口：不能用历史内容选择绕过根边界。
    let response = get_over_tcp(
        server.addr(),
        &diff_url(&root, "review.txt", Some("../outside/secret.txt")),
    )
    .await;
    assert_rejected(&response, 403, "path-outside-workspace");
    assert_no_secret(&response);

    // 目录联接内部：任何一级都不跟随，目标内容不被读取。
    let response = get_over_tcp(
        server.addr(),
        &diff_url(&root, "junction-out/secret.txt", None),
    )
    .await;
    assert_rejected(&response, 403, "path-is-link");
    assert_no_secret(&response);

    if file_link_created {
        let response = get_over_tcp(server.addr(), &diff_url(&root, "filelink-out", None)).await;
        assert_rejected(&response, 403, "path-is-link");
        assert_no_secret(&response);
    }

    // 未登记 workspace 与封闭请求面。
    let response = get_over_tcp(server.addr(), &diff_url(&outside, "secret.txt", None)).await;
    assert_rejected(&response, 404, "workspace-not-registered");
    assert_no_secret(&response);

    let response = get_over_tcp(server.addr(), "/api/workspaces/diff?workspace=&path=").await;
    assert_rejected(&response, 400, "workspace-required");

    let response = get_over_tcp(
        server.addr(),
        &format!("{}&extra=1", diff_url(&root, "review.txt", None)),
    )
    .await;
    assert_rejected(&response, 400, "invalid-query");

    // 路径替换：登记根被外部替换成指向根外的目录联接后，读取必须明确失败。
    let moved = dir.path().join("root-moved");
    fs::rename(&root, &moved).unwrap();
    assert!(
        create_junction(&root, &outside),
        "替换登记根的目录联接必须真实创建"
    );
    let response = get_over_tcp(server.addr(), &diff_url(&root, "review.txt", None)).await;
    assert_rejected(&response, 404, "workspace-unavailable");
    assert_no_secret(&response);
}

fn create_junction(link: &Path, target: &Path) -> bool {
    let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
    let command = format!(
        "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
        quote(link),
        quote(target)
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &command])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && fs::symlink_metadata(link).is_ok()
}

#[cfg(windows)]
fn try_native_file_symlink(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(not(windows))]
fn try_native_file_symlink(_link: &Path, _target: &Path) -> bool {
    false
}

fn wsl_path(path: &Path) -> Option<String> {
    let text = path.to_string_lossy();
    let (drive, rest) = text.split_once(':')?;
    if drive.len() != 1 {
        return None;
    }
    Some(format!(
        "/mnt/{}{}",
        drive.to_ascii_lowercase(),
        rest.replace('\\', "/")
    ))
}

fn try_wsl_symlink(link: &Path, target: &Path) -> bool {
    let (Some(link_wsl), Some(target_wsl)) = (wsl_path(link), wsl_path(target)) else {
        return false;
    };
    let escape = |value: String| value.replace('\'', "'\\''");
    let command = format!("ln -s '{}' '{}'", escape(target_wsl), escape(link_wsl));
    Command::new("wsl.exe")
        .args(["--", "bash", "-lc", &command])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && fs::symlink_metadata(link).is_ok()
}
