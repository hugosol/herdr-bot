//! workspace Git 状态与待提交变更的公开 HTTP interface（S2）。
//!
//! 这些用例经真实 HTTP 请求观察行为，并使用隔离的真实 Git 仓库：期望值来自 Git 原生语义与
//! 独立写定的文件集合，不调用实现内部的计算路径。

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

/// 与交付配置同形，但把 workspace 登记文件放到隔离临时目录。
fn config_with_registry(registry: &Path) -> omp_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.workspaces.registry_file = registry.to_path_buf();
    config
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

fn status_url(workspace: &Path) -> String {
    format!(
        "/api/workspaces/git-status?workspace={}",
        encode(&workspace.to_string_lossy())
    )
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

/// 建立带一次真实提交的仓库；测试显式指定分支名，避免依赖机器的默认分支配置。
fn init_repository(root: &Path) {
    fs::create_dir_all(root).expect("创建仓库目录");
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["config", "user.name", "workspace-git-test"]);
    git(root, &["config", "user.email", "git-status@example.test"]);
}

/// 变更的 (路径, 类别) 集合；忽略 Git 返回的顺序。
fn change_set(response: &HttpResponse) -> Vec<(String, String)> {
    assert_eq!(
        response.status, 200,
        "Git 状态读取应成功：{}",
        response.body
    );
    let mut changes: Vec<(String, String)> = body(response)["changes"]
        .as_array()
        .expect("changes 必须是数组")
        .iter()
        .map(|change| {
            (
                change["path"]
                    .as_str()
                    .expect("变更必须有 path")
                    .to_string(),
                change["kind"]
                    .as_str()
                    .expect("变更必须有 kind")
                    .to_string(),
            )
        })
        .collect();
    changes.sort();
    changes
}

/// 独立记录索引语义：路径、暂存内容与未暂存内容都必须保持不变。
fn index_semantics(root: &Path) -> (String, String, String) {
    (
        git_stdout(root, &["ls-files", "--stage"]),
        git_stdout(root, &["diff", "--cached", "--binary"]),
        git_stdout(root, &["diff", "--binary"]),
    )
}

#[tokio::test]
async fn filesystem_renames_follow_git_and_never_change_the_index() {
    let dir = TempDir::new("workspace git renames");
    let root = dir.path().join("repository");
    init_repository(&root);

    fs::write(
        root.join("old.txt"),
        "content that is long enough to be a rename\n",
    )
    .unwrap();
    fs::write(root.join("untouched.txt"), "other\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    // 真实文件系统改名，尚未暂存：Git 当前结果是删除加未跟踪新增，而不是重命名。
    fs::rename(root.join("old.txt"), root.join("new.txt")).unwrap();
    let before = index_semantics(&root);
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        change_set(&response),
        vec![
            ("new.txt".to_string(), "added".to_string()),
            ("old.txt".to_string(), "deleted".to_string()),
        ],
        "未暂存的文件系统改名必须保持 Git 的删除加新增结果"
    );
    let payload = body(&response);
    assert_eq!(payload["pendingCount"], json!(2));
    assert!(
        payload["changes"]
            .as_array()
            .expect("changes 数组")
            .iter()
            .all(|change| change.get("oldPath").is_none()),
        "未识别重命名不能自创旧路径配对：{payload}"
    );
    assert_eq!(
        index_semantics(&root),
        before,
        "读取状态不能为了识别重命名而修改真实索引或内容"
    );

    // 外部把改名暂存：Git 的当前结果变成一项重命名；查询前后索引语义仍不变。
    git(&root, &["add", "--all"]);
    let before = index_semantics(&root);
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    let changes = body(&response)["changes"].clone();
    assert_eq!(
        changes,
        json!([{
            "path": "new.txt",
            "oldPath": "old.txt",
            "kind": "renamed",
        }]),
        "Git 已识别的重命名必须显示为旧路径到新路径的一项"
    );
    assert_eq!(body(&response)["pendingCount"], json!(1));
    assert_eq!(
        index_semantics(&root),
        before,
        "读取状态不能修改已暂存的改名或索引"
    );
}

#[tokio::test]
async fn staged_and_unstaged_changes_count_once_and_conflicts_are_distinct() {
    let dir = TempDir::new("workspace git conflicts");
    let root = dir.path().join("repository");
    init_repository(&root);

    fs::write(root.join("same.txt"), "base\n").unwrap();
    fs::write(root.join("conflict.txt"), "base\n").unwrap();
    fs::write(root.join("clean.txt"), "clean\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    // 同一文件既已暂存又未暂存：一项变更，计一次。
    fs::write(root.join("same.txt"), "staged\n").unwrap();
    git(&root, &["add", "same.txt"]);
    fs::write(root.join("same.txt"), "unstaged\n").unwrap();

    // 真实 merge 冲突：分支双方修改同一文件。
    git(&root, &["switch", "--quiet", "-c", "side"]);
    fs::write(root.join("conflict.txt"), "side\n").unwrap();
    git(&root, &["commit", "--quiet", "-am", "side"]);
    git(&root, &["switch", "--quiet", "main"]);
    fs::write(root.join("conflict.txt"), "main\n").unwrap();
    git(&root, &["commit", "--quiet", "-am", "main"]);
    let merge = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["merge", "--no-edit", "side"])
        .output()
        .expect("运行 git merge");
    assert!(
        !merge.status.success(),
        "真实 merge 必须产生冲突才能验证警告"
    );

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        change_set(&response),
        vec![
            ("conflict.txt".to_string(), "conflicted".to_string()),
            ("same.txt".to_string(), "modified".to_string()),
        ],
        "同一文件的暂存与未暂存修改只计一项；冲突必须是独立类别"
    );
    let payload = body(&response);
    assert_eq!(payload["pendingCount"], json!(2));
    assert_eq!(payload["conflict"], json!(true));
    assert!(
        payload["changes"]
            .as_array()
            .expect("changes 数组")
            .iter()
            .all(|change| !change["path"].as_str().unwrap_or("").starts_with(".git")),
        "Git 元数据不能进入普通待提交变更：{payload}"
    );

    // 外部解决冲突并暂存后，未合并条目消失：冲突标记不能残留为普通修改或持续警告。
    fs::write(root.join("conflict.txt"), "resolved\n").unwrap();
    git(&root, &["add", "conflict.txt"]);
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    let payload = body(&response);
    assert_eq!(payload["conflict"], json!(false), "已解决冲突不再报告冲突");
    assert_eq!(
        change_set(&response),
        vec![
            ("conflict.txt".to_string(), "modified".to_string()),
            ("same.txt".to_string(), "modified".to_string()),
        ]
    );
}

/// 概览里的一个 workspace；用于按路径找到固定期望。
fn overview_entry<'a>(payload: &'a Value, path: &Path) -> &'a Value {
    payload["workspaces"]
        .as_array()
        .expect("workspaces 必须是数组")
        .iter()
        .find(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|entry_path| common::same_path(Path::new(entry_path), path))
        })
        .unwrap_or_else(|| panic!("概览必须包含 {}：{payload}", path.display()))
}

#[tokio::test]
async fn workspace_badges_count_change_items_not_directories_or_diff_lines() {
    let dir = TempDir::new("workspace git badges");
    let root = dir.path().join("repository");
    let clean = dir.path().join("clean-repository");
    let plain = dir.path().join("plain");
    let missing = dir.path().join("missing");

    init_repository(&root);
    fs::write(root.join("same.txt"), "base\n").unwrap();
    fs::write(
        root.join("rename-me.txt"),
        "rename source that is long enough\n",
    )
    .unwrap();
    fs::write(root.join("keep.txt"), "keep source that is long enough\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored.log\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    // 同文件已暂存 + 未暂存：一项。
    let mut staged = String::new();
    for index in 0..100 {
        staged.push_str(&format!("staged line {index}\n"));
    }
    fs::write(root.join("same.txt"), &staged).unwrap();
    git(&root, &["add", "same.txt"]);
    fs::write(root.join("same.txt"), format!("{staged}unstaged tail\n")).unwrap();

    // Git 已识别的暂存改名：一项。
    git(&root, &["mv", "rename-me.txt", "renamed.txt"]);

    // 真实文件系统改名且未暂存：删除加新增，两项。
    fs::rename(root.join("keep.txt"), root.join("moved.txt")).unwrap();

    // 新目录里的一个未跟踪文件：只有文件计一项，目录本身不计。
    fs::create_dir_all(root.join("newdir")).unwrap();
    fs::write(root.join("newdir").join("file.txt"), "new\n").unwrap();

    fs::write(root.join("ignored.log"), "ignored\n").unwrap();
    // Git 元数据变化：不影响普通待提交项数。
    let config_path = root.join(".git").join("config");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n# metadata-only edit\n");
    fs::write(&config_path, config).unwrap();

    init_repository(&clean);
    fs::write(clean.join("tracked.txt"), "one\n").unwrap();
    git(&clean, &["add", "--all"]);
    git(&clean, &["commit", "--quiet", "-m", "initial"]);
    let clean_config = clean.join(".git").join("config");
    let mut clean_config_text = fs::read_to_string(&clean_config).unwrap();
    clean_config_text.push_str("\n# metadata-only edit\n");
    fs::write(&clean_config, clean_config_text).unwrap();

    fs::create_dir_all(&plain).unwrap();
    fs::create_dir_all(&missing).unwrap();

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    for workspace in [&root, &clean, &plain, &missing] {
        register(&server, workspace).await;
    }
    fs::remove_dir_all(&missing).unwrap();

    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        change_set(&response),
        vec![
            ("keep.txt".to_string(), "deleted".to_string()),
            ("moved.txt".to_string(), "added".to_string()),
            ("newdir/file.txt".to_string(), "added".to_string()),
            ("renamed.txt".to_string(), "renamed".to_string()),
            ("same.txt".to_string(), "modified".to_string()),
        ],
        "固定期望：暂存+未暂存一次、识别重命名一次、未识别删加两次、新目录内文件一次"
    );
    assert_eq!(body(&response)["pendingCount"], json!(5));

    let response = get_over_tcp(server.addr(), "/api/workspaces/overview").await;
    assert_eq!(
        response.status, 200,
        "workspace 概览应成功：{}",
        response.body
    );
    let payload = body(&response);

    let root_entry = overview_entry(&payload, &root);
    assert_eq!(root_entry["repository"], json!(true));
    assert_eq!(root_entry["pendingCount"], json!(5));
    assert_eq!(root_entry["conflict"], json!(false));

    let clean_entry = overview_entry(&payload, &clean);
    assert_eq!(clean_entry["repository"], json!(true));
    assert_eq!(
        clean_entry["pendingCount"],
        json!(0),
        "仅 Git 元数据变化不能增加角标"
    );

    let plain_entry = overview_entry(&payload, &plain);
    assert_eq!(plain_entry["repository"], json!(false));
    assert!(
        plain_entry.get("pendingCount").is_none() || plain_entry["pendingCount"] == json!(0),
        "普通目录没有 Git 待提交角标：{plain_entry}"
    );

    let missing_entry = overview_entry(&payload, &missing);
    assert!(
        missing_entry["error"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "不可访问的 workspace 不能伪装成干净或零变更：{missing_entry}"
    );
}

#[tokio::test]
async fn branch_refreshes_after_external_switches_without_marking_divergence_as_conflict() {
    let dir = TempDir::new("workspace git branch");
    let root = dir.path().join("repository");
    init_repository(&root);
    fs::write(root.join("tracked.txt"), "one\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    // 隔离 bare 远端：制造本地与远端各自前进的普通分叉，但没有未合并文件。
    let remote = dir.path().join("remote.git");
    let output = Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch=main"])
        .arg(&remote)
        .output()
        .expect("初始化 bare 远端");
    assert!(output.status.success(), "bare 远端必须创建成功");
    git(
        &root,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    git(
        &root,
        &["push", "--quiet", "--set-upstream", "origin", "main"],
    );

    let other = dir.path().join("other-clone");
    let clone = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(&remote)
        .arg(&other)
        .output()
        .expect("克隆隔离远端");
    assert!(clone.status.success(), "隔离克隆必须成功");
    git(&other, &["config", "user.name", "remote-writer"]);
    git(&other, &["config", "user.email", "remote@example.test"]);
    fs::write(other.join("remote.txt"), "remote\n").unwrap();
    git(&other, &["add", "--all"]);
    git(&other, &["commit", "--quiet", "-m", "remote ahead"]);
    git(&other, &["push", "--quiet", "origin", "HEAD:main"]);

    fs::write(root.join("local.txt"), "local\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "local ahead"]);

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    let payload = body(&response);
    assert_eq!(payload["repository"], json!(true));
    assert_eq!(payload["branch"], json!("main"));
    assert_eq!(payload["detached"], json!(false));
    assert_eq!(payload["changes"], json!([]));
    assert_eq!(payload["pendingCount"], json!(0));
    assert_eq!(
        payload["conflict"],
        json!(false),
        "普通 ahead/behind 分叉不是文件冲突"
    );

    // 外部切分支后，主动再次查询必须得到新分支，而不是缓存旧值。
    git(&root, &["switch", "--quiet", "-c", "feature"]);
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    let payload = body(&response);
    assert_eq!(payload["branch"], json!("feature"));
    assert_eq!(payload["detached"], json!(false));

    // detached HEAD 不编造分支名。
    git(&root, &["switch", "--quiet", "--detach"]);
    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    let payload = body(&response);
    assert_eq!(payload["repository"], json!(true));
    assert_eq!(payload["branch"], json!(null));
    assert_eq!(payload["detached"], json!(true));
}

#[tokio::test]
async fn ordinary_missing_and_unregistered_workspaces_are_not_clean_git_workspaces() {
    let dir = TempDir::new("workspace git boundaries");
    let plain = dir.path().join("plain");
    let repository = dir.path().join("repository");
    let missing = dir.path().join("missing");
    fs::create_dir_all(&plain).expect("创建普通目录");
    fs::write(plain.join("file.txt"), "plain\n").unwrap();
    init_repository(&repository);
    fs::write(repository.join("tracked.txt"), "one\n").unwrap();
    git(&repository, &["add", "--all"]);
    git(&repository, &["commit", "--quiet", "-m", "initial"]);
    let nested = repository.join("nested");
    fs::create_dir_all(&nested).expect("创建仓库子目录");
    fs::write(nested.join("inside.txt"), "inside\n").unwrap();
    fs::create_dir_all(&missing).expect("创建待删除目录");

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &plain).await;
    register(&server, &repository).await;
    register(&server, &nested).await;
    register(&server, &missing).await;
    fs::remove_dir_all(&missing).expect("外部删除登记目录");

    // 普通目录只有浏览：状态接口明确说它不是 Git 仓库，而不是「干净」。
    let response = get_over_tcp(server.addr(), &status_url(&plain)).await;
    assert_eq!(
        response.status, 200,
        "普通目录状态读取应成功：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["repository"], json!(false));
    assert_eq!(payload["branch"], json!(null));
    assert_eq!(payload["pendingCount"], json!(0));

    // 仓库内部子目录不聚合父仓库变更：它不是工作树根。
    let response = get_over_tcp(server.addr(), &status_url(&nested)).await;
    assert_eq!(response.status, 200);
    assert_eq!(
        body(&response)["repository"],
        json!(false),
        "父仓库的变更不能越界成为子目录 workspace 的待提交集合"
    );

    // 登记目录消失：明确错误，不伪装成干净仓库。
    let response = get_over_tcp(server.addr(), &status_url(&missing)).await;
    assert_eq!(response.status, 404);
    let payload = body(&response);
    assert_eq!(payload["code"], json!("workspace-unavailable"));
    assert!(
        payload["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );

    // 未登记路径同样是明确拒绝。
    let unregistered = dir.path().join("unregistered");
    fs::create_dir_all(&unregistered).expect("创建未登记目录");
    let response = get_over_tcp(server.addr(), &status_url(&unregistered)).await;
    assert_eq!(response.status, 404);
    assert_eq!(body(&response)["code"], json!("workspace-not-registered"));

    // 请求面封闭：未知查询参数不能被静默忽略。
    let response = get_over_tcp(server.addr(), &format!("{}&extra=1", status_url(&plain))).await;
    assert_eq!(response.status, 400);
    assert_eq!(body(&response)["code"], json!("invalid-query"));
}

#[tokio::test]
async fn broken_git_metadata_is_an_explicit_error_not_a_clean_repository() {
    let dir = TempDir::new("workspace git broken metadata");
    let root = dir.path().join("repository");
    init_repository(&root);
    fs::write(root.join("tracked.txt"), "one\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    // `.git` 仍然存在，但 HEAD 已损坏：Git 无法给出工作树事实。
    fs::write(root.join(".git").join("HEAD"), "not a ref\n").unwrap();

    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        response.status, 500,
        "损坏的 Git 元数据必须明确失败：{}",
        response.body
    );
    let payload = body(&response);
    assert_eq!(payload["code"], json!("git-command-failed"));
    assert!(
        payload["message"]
            .as_str()
            .is_some_and(|message| message.contains("rev-parse")),
        "失败说明应保留 Git 返回的诊断类别：{payload}"
    );
    assert!(
        payload.get("changes").is_none() && payload.get("pendingCount").is_none(),
        "Git 错误不能被伪装成空变更或干净仓库：{payload}"
    );

    let response = get_over_tcp(server.addr(), "/api/workspaces/overview").await;
    assert_eq!(
        response.status, 200,
        "概览本身应成功返回：{}",
        response.body
    );
    let payload = body(&response);
    let entry = overview_entry(&payload, &root);
    assert!(
        entry["error"]
            .as_str()
            .is_some_and(|message| message.contains("rev-parse")),
        "概览必须显示该 workspace 的 Git 错误：{entry}"
    );
    assert!(
        entry.get("pendingCount").is_none(),
        "无法读取时不能给出 0 项或其它假计数：{entry}"
    );
}

#[tokio::test]
async fn pending_set_follows_git_tracking_and_ignore_rules() {
    let dir = TempDir::new("workspace git pending set");
    let root = dir.path().join("repository");
    init_repository(&root);

    // 先提交一个稍后才被忽略规则匹配的已跟踪文件。
    fs::write(root.join("tracked.txt"), "one\n").unwrap();
    fs::write(root.join("gone.txt"), "gone\n").unwrap();
    fs::write(root.join("later.log"), "tracked log\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored.log\n").unwrap();
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "initial"]);

    // 真实 Git 事实：later.log 已被跟踪；ignored.log 尚未被跟踪且被忽略。
    fs::write(root.join(".gitignore"), "ignored.log\nlater.log\n").unwrap();
    fs::write(root.join("tracked.txt"), "two\n").unwrap();
    fs::remove_file(root.join("gone.txt")).unwrap();
    fs::write(root.join("new.txt"), "new\n").unwrap();
    fs::write(root.join("ignored.log"), "ignored\n").unwrap();
    fs::write(root.join("later.log"), "modified despite ignore\n").unwrap();
    // 已暂存的新文件同样属于待提交集合，并显示为新增。
    fs::write(root.join("staged-new.txt"), "staged\n").unwrap();
    git(&root, &["add", "staged-new.txt"]);

    assert_eq!(
        git_stdout(&root, &["check-ignore", "ignored.log"]).trim(),
        "ignored.log",
        "ignored.log 必须处于被忽略状态"
    );
    assert_eq!(
        git_stdout(&root, &["ls-files", "--error-unmatch", "later.log"]).trim(),
        "later.log",
        "later.log 必须是已跟踪文件"
    );

    let registry = dir.path().join("workspaces.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &status_url(&root)).await;
    assert_eq!(
        response.status, 200,
        "Git workspace 状态应成功：{}",
        response.body
    );
    let body = body(&response);
    assert_eq!(body["repository"], json!(true));
    assert_eq!(body["branch"], json!("main"));
    assert_eq!(body["detached"], json!(false));
    assert_eq!(body["conflict"], json!(false));

    assert_eq!(
        change_set(&response),
        vec![
            (".gitignore".to_string(), "modified".to_string()),
            ("gone.txt".to_string(), "deleted".to_string()),
            ("later.log".to_string(), "modified".to_string()),
            ("new.txt".to_string(), "added".to_string()),
            ("staged-new.txt".to_string(), "added".to_string()),
            ("tracked.txt".to_string(), "modified".to_string()),
        ],
        "待提交集合必须是已跟踪变更加未忽略的未跟踪文件，且排除被忽略的未跟踪文件"
    );
    assert_eq!(body["pendingCount"], json!(6));
}
