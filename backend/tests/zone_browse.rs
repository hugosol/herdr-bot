//! zone 文件浏览的公开 HTTP interface（S2）。
//!
//! 这些用例只经真实 HTTP 请求观察行为：目录输出与独立创建的真实文件集合核对；边界反例通过公开
//! 请求实施，并使用真实的 Windows 目录联接与（可用时）符号链接；内容识别、大小边界与只读语义都由
//! 独立的真实文件或 Git 索引状态给出期望值，不调用实现内部的计算路径。

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

/// 与交付配置同形，但把登记文件放到隔离临时目录，并显式给出预览上限。
fn config_with_registry_and_limit(registry: &Path, preview_limit: u64) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config.zones.preview_limit_bytes =
        herdr_bot::config::PreviewLimitBytes::new(preview_limit).expect("测试上限必须合法");
    config
}

fn config_with_registry(registry: &Path) -> herdr_bot::Config {
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

fn entries_url(zone: &Path, path: &str) -> String {
    format!(
        "/api/zones/entries?zone={}&path={}",
        encode(&zone.to_string_lossy()),
        encode(path)
    )
}

fn file_url(zone: &Path, path: &str) -> String {
    format!(
        "/api/zones/file?zone={}&path={}",
        encode(&zone.to_string_lossy()),
        encode(path)
    )
}

fn tree_url(zone: &Path) -> String {
    format!("/api/zones/tree?zone={}", encode(&zone.to_string_lossy()))
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body).expect("响应必须是 JSON")
}

fn entry_names(response: &HttpResponse) -> Vec<String> {
    assert_eq!(response.status, 200, "目录读取应成功：{}", response.body);
    body(response)["entries"]
        .as_array()
        .expect("entries 必须是数组")
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("目录项必须有 name")
                .to_string()
        })
        .collect()
}

fn entry_of(response: &HttpResponse, name: &str) -> Value {
    body(response)["entries"]
        .as_array()
        .expect("entries 必须是数组")
        .iter()
        .find(|entry| entry["name"] == json!(name))
        .unwrap_or_else(|| panic!("目录输出必须包含 {name}：{}", response.body))
        .clone()
}

fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(
        response.status, status,
        "拒绝状态码不符合预期：{}",
        response.body
    );
    let body = body(response);
    assert_eq!(body["code"], json!(code), "拒绝类别不符合预期：{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "拒绝必须给出可读说明：{body}"
    );
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

/// 独立按照磁盘内容构造快照：相对路径 → 文件字节。用于证明读取没有创建、删除或修改文件。
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).expect("读取快照目录") {
            let entry = entry.expect("读取快照目录项");
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("读取快照元数据");
            if metadata.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("快照路径在根下")
                    .to_path_buf();
                files.insert(relative, fs::read(&path).expect("读取快照文件"));
            }
        }
    }

    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn tree_entry_names(tree: &Value, path: &str) -> Vec<String> {
    tree["directories"]
        .as_array()
        .expect("directories 必须是数组")
        .iter()
        .find(|directory| directory["path"] == json!(path))
        .unwrap_or_else(|| panic!("目录树必须包含 {path:?}：{tree}"))["entries"]
        .as_array()
        .expect("entries 必须是数组")
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("目录项必须有 name")
                .to_string()
        })
        .collect()
}

fn git(root: &Path, args: &[&str]) -> String {
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

#[tokio::test]
async fn whole_tree_uses_git_native_ignores_but_keeps_tracked_and_empty_directories() {
    let dir = TempDir::new("zone tree git ignores");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("ignored")).expect("创建 ignored 目录");
    fs::create_dir_all(root.join("kept")).expect("创建 kept 目录");
    fs::create_dir_all(root.join("empty")).expect("创建空目录");
    fs::write(root.join(".gitignore"), "ignored/\n*.log\n").unwrap();
    fs::write(root.join("ignored").join("dependency.txt"), "dependency").unwrap();
    fs::write(root.join("kept").join(".gitignore"), "local.tmp\n").unwrap();
    fs::write(root.join("kept").join("tracked.txt"), "tracked").unwrap();
    fs::write(root.join("kept").join("scratch.log"), "ignored sibling").unwrap();
    fs::write(root.join("kept").join("local.tmp"), "nested ignored").unwrap();
    fs::write(root.join("visible.txt"), "visible").unwrap();
    fs::write(root.join(".hidden.txt"), "hidden").unwrap();

    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "tree@example.test"]);
    git(&root, &["config", "user.name", "Tree Test"]);
    git(
        &root,
        &[
            "add",
            ".gitignore",
            "kept/.gitignore",
            "kept/tracked.txt",
            "visible.txt",
            ".hidden.txt",
        ],
    );
    git(&root, &["commit", "-qm", "fixture"]);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &tree_url(&root)).await;
    assert_eq!(response.status, 200, "整树读取应成功：{}", response.body);
    let tree = body(&response);
    assert_eq!(tree["ignoredHidden"], json!(true));

    let root_entries = tree_entry_names(&tree, "");
    for expected in [".gitignore", ".hidden.txt", "visible.txt", "kept", "empty"] {
        assert!(
            root_entries.iter().any(|name| name == expected),
            "根目录必须保留 {expected}：{root_entries:?}"
        );
    }
    assert!(
        !root_entries.iter().any(|name| name == "ignored"),
        "整棵 ignored 目录不应出现：{root_entries:?}"
    );
    assert!(
        !root_entries.iter().any(|name| name == ".git"),
        "Git 元数据不应出现在浏览树：{root_entries:?}"
    );
    assert!(
        tree["directories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|directory| !directory["path"].as_str().unwrap_or("").starts_with(".git")),
        "不应遍历 .git 元数据：{}",
        tree
    );

    let kept_entries = tree_entry_names(&tree, "kept");
    assert!(kept_entries.iter().any(|name| name == "tracked.txt"));
    assert!(kept_entries.iter().any(|name| name == ".gitignore"));
    assert!(!kept_entries.iter().any(|name| name == "scratch.log"));
    assert!(!kept_entries.iter().any(|name| name == "local.tmp"));
    assert_eq!(
        tree_entry_names(&tree, "empty"),
        Vec::<String>::new(),
        "未被忽略的空目录必须保留"
    );
    assert!(
        tree["directories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|directory| directory["path"] != json!("ignored")),
        "不应遍历 ignored 目录：{}",
        tree
    );
}

#[tokio::test]
async fn broken_git_metadata_fails_the_whole_tree_instead_of_showing_ignored_content() {
    let dir = TempDir::new("zone tree broken git");
    let root = dir.path().join("project");
    fs::create_dir_all(&root).expect("创建目录");
    fs::write(root.join(".git"), "gitdir: C:\\missing\\metadata\n").unwrap();
    fs::write(root.join("visible.txt"), "visible").unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &tree_url(&root)).await;
    assert_rejected(&response, 500, "git-ignore-failed");
}

#[tokio::test]
async fn plain_directory_tree_keeps_gitignore_like_files_unfiltered() {
    let dir = TempDir::new("zone tree plain");
    let root = dir.path().join("project");
    fs::create_dir_all(&root).expect("创建普通目录");
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("ignored.txt"), "plain file").unwrap();
    fs::write(root.join("visible.txt"), "visible").unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &tree_url(&root)).await;
    assert_eq!(
        response.status, 200,
        "普通目录整树读取应成功：{}",
        response.body
    );
    let tree = body(&response);
    assert_eq!(tree["ignoredHidden"], json!(false));
    let root_entries = tree_entry_names(&tree, "");
    assert!(root_entries.iter().any(|name| name == ".gitignore"));
    assert!(root_entries.iter().any(|name| name == "ignored.txt"));
    assert!(root_entries.iter().any(|name| name == "visible.txt"));
}

#[tokio::test]
async fn lists_real_contents_including_hidden_ignored_and_git_metadata() {
    let dir = TempDir::new("zone browse contents");
    let root = dir.path().join("project");
    fs::create_dir_all(root.join("src").join("nested")).expect("创建目录树");
    fs::create_dir_all(root.join(".git")).expect("创建 Git 元数据目录");
    fs::write(root.join("visible.txt"), "visible").unwrap();
    fs::write(root.join(".hidden.txt"), "hidden").unwrap();
    fs::write(root.join(".gitignore"), "ignored.log\n").unwrap();
    fs::write(root.join("ignored.log"), "ignored").unwrap();
    fs::write(root.join("README"), "extensionless text").unwrap();
    fs::write(root.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        root.join(".git").join("config"),
        "[core]\n\trepositoryformatversion = 0\n",
    )
    .unwrap();
    fs::write(root.join("src").join("main.ts"), "console.log(1)\n").unwrap();
    fs::write(root.join("src").join("nested").join("deep.txt"), "deep").unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let root_listing = get_over_tcp(server.addr(), &entries_url(&root, "")).await;
    let mut expected: Vec<String> = fs::read_dir(&root)
        .expect("独立读取真实目录")
        .map(|entry| {
            entry
                .expect("目录项")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    expected.sort();
    let mut actual = entry_names(&root_listing);
    actual.sort();
    assert_eq!(actual, expected, "目录输出必须与独立读取的真实文件集合一致");

    // 隐藏文件、忽略规则文件、被忽略文件与 Git 元数据都必须出现。
    assert_eq!(
        entry_of(&root_listing, ".hidden.txt")["kind"],
        json!("file")
    );
    assert_eq!(entry_of(&root_listing, ".gitignore")["kind"], json!("file"));
    assert_eq!(
        entry_of(&root_listing, "ignored.log")["kind"],
        json!("file")
    );
    assert_eq!(entry_of(&root_listing, ".git")["kind"], json!("directory"));

    // 登记身份按 Windows 规则比较：同一路径用 `/` 书写仍指向同一个 zone。
    let forward_slashes = root.to_string_lossy().replace('\\', "/");
    let response = get_over_tcp(
        server.addr(),
        &format!("/api/zones/entries?zone={}&path=", encode(&forward_slashes)),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "同一 zone 的 `/` 写法应被接受：{}",
        response.body
    );

    let src_listing = get_over_tcp(server.addr(), &entries_url(&root, "src")).await;
    let mut expected_src: Vec<String> = fs::read_dir(root.join("src"))
        .expect("独立读取 src")
        .map(|entry| {
            entry
                .expect("目录项")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    expected_src.sort();
    let mut actual_src = entry_names(&src_listing);
    actual_src.sort();
    assert_eq!(actual_src, expected_src);

    // 目录读取入口与文件读取入口对目标类型不匹配时明确拒绝。
    let response = get_over_tcp(server.addr(), &entries_url(&root, "visible.txt")).await;
    assert_rejected(&response, 400, "not-a-file");
    let response = get_over_tcp(server.addr(), &file_url(&root, "src")).await;
    assert_rejected(&response, 400, "not-a-file");

    for (relative, expected_content) in [
        (".hidden.txt", "hidden"),
        (".gitignore", "ignored.log\n"),
        ("README", "extensionless text"),
        ("src/main.ts", "console.log(1)\n"),
        (".git/HEAD", "ref: refs/heads/main\n"),
    ] {
        let response = get_over_tcp(server.addr(), &file_url(&root, relative)).await;
        assert_eq!(
            response.status, 200,
            "读取 {relative} 应成功：{}",
            response.body
        );
        let body = body(&response);
        assert_eq!(body["preview"]["kind"], json!("text"), "{relative}");
        assert_eq!(
            body["preview"]["content"],
            json!(expected_content),
            "{relative}"
        );
    }
}

#[tokio::test]
async fn rejects_traversal_absolute_paths_and_unregistered_zones() {
    let dir = TempDir::new("zone browse traversal");
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    fs::create_dir_all(root.join("sub")).expect("创建根目录");
    fs::create_dir_all(&outside).expect("创建根外目录");
    fs::write(outside.join("secret.txt"), "top-secret").unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let outside_absolute = outside.join("secret.txt").to_string_lossy().into_owned();
    for path in [
        "../outside/secret.txt",
        "..\\outside\\secret.txt",
        "sub/../../outside/secret.txt",
        // Windows 会去掉分量末尾的空格与点：`.. ` 与 `....` 不能被当成普通文件名。
        ".. /outside/secret.txt",
        "..../outside/secret.txt",
        outside_absolute.as_str(),
        "C:\\Windows\\win.ini",
        "\\\\server\\share\\file.txt",
    ] {
        let response = get_over_tcp(server.addr(), &entries_url(&root, path)).await;
        assert_rejected(&response, 403, "path-outside-zone");
        assert!(
            !response.body.contains("top-secret"),
            "越界请求不能泄露根外内容：{}",
            response.body
        );
    }

    for path in ["sub./file.txt", "sub /file.txt"] {
        let response = get_over_tcp(server.addr(), &entries_url(&root, path)).await;
        assert_rejected(&response, 400, "invalid-path");
    }

    // 直接文件读取入口同样执行边界检查。
    let response = get_over_tcp(server.addr(), &file_url(&root, "../outside/secret.txt")).await;
    assert_rejected(&response, 403, "path-outside-zone");
    assert!(!response.body.contains("top-secret"));

    // 未登记路径与空 zone 参数都是明确拒绝。
    let response = get_over_tcp(server.addr(), &entries_url(&outside, "")).await;
    assert_rejected(&response, 404, "zone-not-registered");

    let response = get_over_tcp(server.addr(), "/api/zones/entries?zone=&path=").await;
    assert_rejected(&response, 400, "zone-required");

    // 请求面封闭：未知查询参数不能被静默忽略。
    let response = get_over_tcp(
        server.addr(),
        &format!("{}&extra=1", entries_url(&root, "")),
    )
    .await;
    assert_rejected(&response, 400, "invalid-query");
}

/// 用 PowerShell 创建真实 Windows 目录联接（junction）；不需要管理员权限。
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

/// 尝试创建标准 Windows 目录符号链接；没有权限时返回 false。
#[cfg(windows)]
fn try_native_symlink(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}

#[cfg(not(windows))]
fn try_native_symlink(_link: &Path, _target: &Path) -> bool {
    false
}

/// 尝试创建标准 Windows 文件符号链接；没有权限时返回 false。
#[cfg(windows)]
fn try_native_file_symlink(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

#[cfg(not(windows))]
fn try_native_file_symlink(_link: &Path, _target: &Path) -> bool {
    false
}

/// 把 Windows 路径转成 WSL 的 `/mnt/<drive>/…` 形式。
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

/// 无管理员权限时，退而通过 WSL 在 NTFS 上创建同样属于 reparse point 的符号链接。
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

#[cfg(windows)]
fn is_reparse_point(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_attributes() & 0x400 != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_reparse_point(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[tokio::test]
async fn rejects_real_junctions_and_symlinks_without_reading_outside_content() {
    let dir = TempDir::new("zone browse links");
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    fs::create_dir_all(&root).expect("创建根目录");
    fs::create_dir_all(&outside).expect("创建根外目录");
    fs::write(outside.join("secret.txt"), "top-secret").unwrap();

    let junction = root.join("junction-out");
    assert!(create_junction(&junction, &outside), "目录联接必须真实创建");

    let symlink = root.join("symlink-out");
    let native_symlink = try_native_symlink(&symlink, &outside);
    let symlink_created = native_symlink || try_wsl_symlink(&symlink, &outside);
    if symlink_created {
        assert!(
            is_reparse_point(&symlink),
            "符号链接必须是真实的 reparse point"
        );
        eprintln!(
            "已创建真实符号链接（native={native_symlink}）：{}",
            symlink.display()
        );
    } else {
        eprintln!("跳过符号链接部分：当前环境无法创建标准或 WSL 符号链接");
    }

    let file_link = root.join("filelink-out");
    let file_link_created = try_native_file_symlink(&file_link, &outside.join("secret.txt"))
        || try_wsl_symlink(&file_link, &outside.join("secret.txt"));
    if file_link_created {
        assert!(
            is_reparse_point(&file_link),
            "文件符号链接必须是真实的 reparse point"
        );
        eprintln!("已创建真实文件符号链接：{}", file_link.display());
    }

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let listing = get_over_tcp(server.addr(), &entries_url(&root, "")).await;
    let junction_entry = entry_of(&listing, "junction-out");
    assert_eq!(junction_entry["kind"], json!("link"));
    assert!(
        junction_entry["linkTarget"]
            .as_str()
            .is_some_and(|target| target.contains("outside")),
        "目录联接必须显示真实目标：{junction_entry}"
    );
    assert!(
        junction_entry["linkReason"]
            .as_str()
            .is_some_and(|reason| reason.contains("zone 之外")),
        "根外链接必须说明不可进入原因：{junction_entry}"
    );
    if symlink_created {
        let symlink_entry = entry_of(&listing, "symlink-out");
        assert_eq!(symlink_entry["kind"], json!("link"));
        assert!(symlink_entry["linkReason"].as_str().is_some());
    }
    if file_link_created {
        let file_link_entry = entry_of(&listing, "filelink-out");
        assert_eq!(file_link_entry["kind"], json!("link"));
        assert!(file_link_entry["linkReason"].as_str().is_some());
    }
    assert!(
        !listing.body.contains("top-secret"),
        "目录列表不能读取链接目标内容"
    );

    for path in ["junction-out", "junction-out/secret.txt"] {
        let response = get_over_tcp(server.addr(), &entries_url(&root, path)).await;
        assert_rejected(&response, 403, "path-is-link");
        assert!(
            response.body.contains("目标"),
            "拒绝应显示链接目标：{}",
            response.body
        );
        assert!(!response.body.contains("top-secret"));
    }

    let response = get_over_tcp(server.addr(), &file_url(&root, "junction-out/secret.txt")).await;
    assert_rejected(&response, 403, "path-is-link");
    assert!(!response.body.contains("top-secret"));

    if symlink_created {
        let response =
            get_over_tcp(server.addr(), &file_url(&root, "symlink-out/secret.txt")).await;
        assert_rejected(&response, 403, "path-is-link");
        assert!(!response.body.contains("top-secret"));
    }

    if file_link_created {
        let response = get_over_tcp(server.addr(), &file_url(&root, "filelink-out")).await;
        assert_rejected(&response, 403, "path-is-link");
        assert!(
            !response.body.contains("top-secret"),
            "文件符号链接不能读取根外内容：{}",
            response.body
        );
    }
}

#[tokio::test]
async fn external_deletion_and_root_replacement_are_explicit_errors() {
    let dir = TempDir::new("zone browse deletion");
    let root = dir.path().join("root");
    let outside = dir.path().join("outside");
    fs::create_dir_all(root.join("sub")).expect("创建子目录");
    fs::create_dir_all(&outside).expect("创建根外目录");
    fs::write(root.join("sub").join("file.txt"), "content").unwrap();
    fs::write(outside.join("secret.txt"), "top-secret").unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;
    assert_eq!(
        get_over_tcp(server.addr(), &entries_url(&root, ""))
            .await
            .status,
        200
    );

    // 外部删除子目录：明确报不存在，不伪装成空目录。
    fs::remove_dir_all(root.join("sub")).expect("删除子目录");
    let response = get_over_tcp(server.addr(), &entries_url(&root, "sub")).await;
    assert_rejected(&response, 404, "path-not-found");
    assert!(
        !response.body.contains("\"entries\":[]"),
        "不能伪装成空目录"
    );

    let response = get_over_tcp(server.addr(), &file_url(&root, "sub/file.txt")).await;
    assert_rejected(&response, 404, "path-not-found");

    // 子目录被替换成普通文件：请求目录与请求其子路径都明确报错，不当成空目录。
    fs::write(root.join("sub"), "now a file").expect("用普通文件替换子目录");
    let response = get_over_tcp(server.addr(), &entries_url(&root, "sub")).await;
    assert_rejected(&response, 400, "not-a-file");
    let response = get_over_tcp(server.addr(), &file_url(&root, "sub/file.txt")).await;
    assert_rejected(&response, 400, "not-a-directory");
    fs::remove_file(root.join("sub")).expect("移除替换文件");

    // 用根外目录联接替换子目录：即使路径仍在根下，也不能跟随。
    assert!(
        create_junction(&root.join("sub"), &outside),
        "目录联接必须真实创建"
    );
    let response = get_over_tcp(server.addr(), &entries_url(&root, "sub")).await;
    assert_rejected(&response, 403, "path-is-link");
    assert!(!response.body.contains("top-secret"));

    // 根目录本身被替换为根外目录联接：整次读取明确失败，不列出根外内容。
    let replaced_root = dir.path().join("replaced-root");
    fs::create_dir_all(&replaced_root).expect("创建待替换根");
    fs::write(replaced_root.join("inside.txt"), "inside").unwrap();
    let replaced_registry = dir.path().join("replaced-zones.json");
    let replaced_server = TestServer::start(&config_with_registry(&replaced_registry)).await;
    register(&replaced_server, &replaced_root).await;

    fs::rename(&replaced_root, dir.path().join("moved-root")).expect("移走原根");
    assert!(
        create_junction(&replaced_root, &outside),
        "根目录联接必须真实创建"
    );

    let response = get_over_tcp(replaced_server.addr(), &entries_url(&replaced_root, "")).await;
    assert_rejected(&response, 404, "zone-unavailable");
    assert!(
        !response.body.contains("top-secret"),
        "根目录被替换后不能读取根外内容：{}",
        response.body
    );

    // 根目录被直接删除后，同一路径同样是明确错误。
    fs::remove_dir_all(&replaced_root).expect("删除被替换的根");
    let response = get_over_tcp(replaced_server.addr(), &entries_url(&replaced_root, "")).await;
    assert_rejected(&response, 404, "zone-unavailable");
}

#[tokio::test]
async fn reading_does_not_change_files_or_git_index_semantics() {
    let dir = TempDir::new("zone browse read only");
    let root = dir.path().join("repository");
    fs::create_dir_all(&root).expect("创建仓库目录");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );

    git(&root, &["config", "user.name", "zone-browse-test"]);
    git(&root, &["config", "user.email", "browse@example.test"]);
    fs::write(root.join("tracked.txt"), "one\n").unwrap();
    git(&root, &["add", "tracked.txt"]);
    git(&root, &["commit", "-m", "initial"]);

    // 制造真实的部分暂存：一个已暂存的新文件 + 一个未暂存修改。
    fs::write(root.join("staged.txt"), "staged\n").unwrap();
    git(&root, &["add", "staged.txt"]);
    fs::write(root.join("tracked.txt"), "two\n").unwrap();

    let before_ls_files = git(&root, &["ls-files", "--stage"]);
    let before_cached = git(&root, &["diff", "--cached", "--binary"]);
    let before_unstaged = git(&root, &["diff", "--binary"]);
    let before_status = git(&root, &["status", "--porcelain=v1"]);
    // Git 自己的只读查询可能刷新索引缓存，因此快照在这之后取，只观察本服务的读取。
    let before_snapshot = snapshot(&root);

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    for url in [
        entries_url(&root, ""),
        entries_url(&root, ".git"),
        file_url(&root, "tracked.txt"),
        file_url(&root, "staged.txt"),
        file_url(&root, ".git/HEAD"),
        file_url(&root, ".git/index"),
    ] {
        let response = get_over_tcp(server.addr(), &url).await;
        assert_eq!(
            response.status, 200,
            "只读请求应成功：{url}：{}",
            response.body
        );
    }

    let after_snapshot = snapshot(&root);
    assert_eq!(
        after_snapshot, before_snapshot,
        "浏览读取不能创建、删除或修改任何文件"
    );

    assert_eq!(
        git(&root, &["ls-files", "--stage"]),
        before_ls_files,
        "真实索引内容不能改变"
    );
    assert_eq!(
        git(&root, &["diff", "--cached", "--binary"]),
        before_cached,
        "已暂存语义不能改变"
    );
    assert_eq!(
        git(&root, &["diff", "--binary"]),
        before_unstaged,
        "未暂存内容不能改变"
    );
    assert_eq!(
        git(&root, &["status", "--porcelain=v1"]),
        before_status,
        "待提交状态不能改变"
    );
}

#[tokio::test]
async fn text_is_detected_from_content_and_preview_limit_applies_exactly() {
    let dir = TempDir::new("zone browse preview");
    let root = dir.path().join("root");
    fs::create_dir_all(&root).expect("创建根目录");
    let exact_text = "a".repeat(64);
    fs::write(root.join("plain"), &exact_text).unwrap();
    fs::write(root.join("over"), format!("{exact_text}b")).unwrap();
    fs::write(root.join("binary.txt"), [0x00, 0x01, 0x02, 0xFF]).unwrap();
    fs::write(root.join("invalid.txt"), [0xFF, 0xFE]).unwrap();
    fs::write(root.join("page.html"), "<script>window.__pwned=1</script>").unwrap();
    fs::write(root.join("utf8.txt"), "中文\n").unwrap();
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(b"bom text");
    fs::write(root.join("bom.txt"), bom).unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry_and_limit(&registry, 64)).await;
    register(&server, &root).await;

    let exact = get_over_tcp(server.addr(), &file_url(&root, "plain")).await;
    assert_eq!(exact.status, 200);
    let exact = body(&exact);
    assert_eq!(exact["preview"]["kind"], json!("text"));
    assert_eq!(exact["preview"]["content"], json!(exact_text));
    assert_eq!(exact["size"], json!(64));

    let over = get_over_tcp(server.addr(), &file_url(&root, "over")).await;
    assert_eq!(over.status, 200);
    let over = body(&over);
    assert_eq!(over["preview"]["kind"], json!("too-large"));
    assert_eq!(over["preview"]["size"], json!(65));
    assert_eq!(over["preview"]["limit"], json!(64));
    assert!(
        over["preview"].get("content").is_none(),
        "越界预览不能伪装成完整内容：{over}"
    );

    for path in ["binary.txt", "invalid.txt"] {
        let response = get_over_tcp(server.addr(), &file_url(&root, path)).await;
        assert_eq!(response.status, 200);
        let response = body(&response);
        assert_eq!(response["preview"]["kind"], json!("binary"), "{path}");
        assert!(
            response["preview"]["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty()),
            "{path} 必须给出不可预览原因：{response}"
        );
        assert!(response["preview"].get("content").is_none());
    }

    for (path, expected) in [
        ("page.html", "<script>window.__pwned=1</script>"),
        ("utf8.txt", "中文\n"),
        ("bom.txt", "\u{feff}bom text"),
    ] {
        let response = get_over_tcp(server.addr(), &file_url(&root, path)).await;
        let response = body(&response);
        assert_eq!(response["preview"]["kind"], json!("text"));
        assert_eq!(response["preview"]["content"], json!(expected));
    }
}

/// 超限文件在返回限制说明之前不应被打开读取：用独占共享模式锁住文件内容，仍能读取元数据。
#[cfg(windows)]
#[tokio::test]
async fn oversized_file_content_is_not_opened() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = TempDir::new("zone browse oversized no open");
    let root = dir.path().join("root");
    fs::create_dir_all(&root).expect("创建根目录");
    let path = root.join("locked.txt");
    fs::write(&path, "a".repeat(65)).unwrap();
    // 共享模式 0：其他句柄不能读取内容；GetFileAttributes 仍然可用。
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("以独占共享模式打开超限文件");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry_and_limit(&registry, 64)).await;
    register(&server, &root).await;

    let response = get_over_tcp(server.addr(), &file_url(&root, "locked.txt")).await;
    assert_eq!(
        response.status, 200,
        "超限文件只应读取元数据，不应打开内容：{}",
        response.body
    );
    let body = body(&response);
    assert_eq!(body["preview"]["kind"], json!("too-large"));
    assert_eq!(body["preview"]["size"], json!(65));
    assert!(body["preview"].get("content").is_none());

    drop(held);
}

#[tokio::test]
async fn default_preview_limit_is_one_mebibyte_at_the_exact_boundary() {
    let dir = TempDir::new("zone browse default limit");
    let root = dir.path().join("root");
    fs::create_dir_all(&root).expect("创建根目录");
    let limit = 1024 * 1024;
    fs::write(root.join("exact.txt"), "a".repeat(limit)).unwrap();
    fs::write(root.join("over.txt"), "a".repeat(limit + 1)).unwrap();

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &root).await;

    let exact = get_over_tcp(server.addr(), &file_url(&root, "exact.txt")).await;
    assert_eq!(exact.status, 200);
    let exact = body(&exact);
    assert_eq!(exact["preview"]["kind"], json!("text"));
    assert_eq!(
        exact["preview"]["content"].as_str().unwrap().len(),
        limit,
        "恰好 1 MiB 必须完整读取"
    );

    let over = get_over_tcp(server.addr(), &file_url(&root, "over.txt")).await;
    assert_eq!(over.status, 200);
    let over = body(&over);
    assert_eq!(over["preview"]["kind"], json!("too-large"));
    assert_eq!(over["preview"]["limit"], json!(limit));
    assert_eq!(over["preview"]["size"], json!(limit + 1));
    assert!(over["preview"].get("content").is_none());
}
