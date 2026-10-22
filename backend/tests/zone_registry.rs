//! zone 登记列表的公开 HTTP interface（S2）。
//!
//! 这些用例只经真实 HTTP 请求观察行为：登记、路径校验与列出。登记列表由后端以文件持久保存，
//! 因此期望值来自独立创建的真实目录与直接读取的登记文件，不调用实现的计算路径生成。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

/// 把 zone 登记文件放到隔离的临时目录里；除此之外与交付配置同一形状。
fn config_with_registry(registry: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config
}

/// 响应里的登记路径（按出现顺序）。
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

/// 期望值是独立创建的真实目录：路径比较先各自 canonicalize，避免大小写与分隔符差异。
fn assert_same_directories(actual: &[PathBuf], expected: &[PathBuf]) {
    let canonical = |paths: &[PathBuf]| {
        paths
            .iter()
            .map(|path| fs::canonicalize(path).expect("被比较的目录必须真实存在"))
            .collect::<Vec<_>>()
    };
    assert_eq!(canonical(actual), canonical(expected));
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
async fn registers_an_existing_directory_and_rejects_ineligible_paths() {
    let dir = TempDir::new("zone register");
    let zone = dir.path().join("project");
    fs::create_dir_all(&zone).expect("创建待登记目录");
    fs::write(zone.join("note.txt"), "hello").expect("写入目录内容");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;

    // 首次启动没有任何登记：登记文件尚不存在不是读取错误。
    assert_eq!(
        zones_of(&get_over_tcp(server.addr(), "/api/zones").await),
        Vec::<PathBuf>::new()
    );

    // 登记一个真实存在的普通目录。
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记应成功：{}", response.body);
    assert_same_directories(&zones_of(&response), &[zone.clone()]);

    // 登记成功意味着持久化已经完成：直接读取登记文件，而不是相信响应。
    let persisted: Value =
        serde_json::from_str(&fs::read_to_string(&registry).expect("登记文件必须存在"))
            .expect("登记文件必须是 JSON");
    let persisted_paths: Vec<PathBuf> = persisted["zones"]
        .as_array()
        .expect("登记文件必须有 zones")
        .iter()
        .map(|entry| PathBuf::from(entry["path"].as_str().expect("登记项必须有 path")))
        .collect();
    assert_same_directories(&persisted_paths, &[zone.clone()]);

    // 第二个独立调用者（另一条 TCP 连接）读取同一份列表。
    assert_same_directories(
        &zones_of(&get_over_tcp(server.addr(), "/api/zones").await),
        &[zone.clone()],
    );

    // 同一个目录再次登记是明确拒绝，不产生第二项。
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones",
            &json!({ "path": zone }).to_string(),
        )
        .await,
        409,
        "already-registered",
    );

    // 非法输入逐类明确失败。
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones",
            &json!({ "path": dir.path().join("missing") }).to_string(),
        )
        .await,
        400,
        "path-not-found",
    );

    let file = dir.path().join("plain.txt");
    fs::write(&file, "not a directory").expect("创建普通文件");
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones",
            &json!({ "path": file }).to_string(),
        )
        .await,
        400,
        "not-a-directory",
    );

    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones",
            &json!({ "path": "relative/project" }).to_string(),
        )
        .await,
        400,
        "path-not-absolute",
    );

    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones",
            &json!({ "path": "   " }).to_string(),
        )
        .await,
        400,
        "path-required",
    );

    // 所有拒绝都没有改变已持久化的列表。
    assert_same_directories(
        &zones_of(&get_over_tcp(server.addr(), "/api/zones").await),
        &[zone],
    );
}

/// 一个真实初始化的 Git 工作树根目录可以登记；普通目录登记不初始化 Git，也不扫描、聚合嵌套仓库。
#[tokio::test]
async fn registers_a_real_git_worktree_root_without_initializing_or_aggregating_repositories() {
    let dir = TempDir::new("zone git root");

    // 普通目录：登记之后不得凭空出现 .git。
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).expect("创建普通目录");

    // 真实 Git 工作树根目录。
    let repository = dir.path().join("repository");
    fs::create_dir_all(&repository).expect("创建仓库目录");
    let initialized = Command::new("git")
        .args(["init", "--quiet"])
        .arg(&repository)
        .status()
        .expect("运行 git init");
    assert!(initialized.success(), "git init 必须成功");

    // 一个包含嵌套仓库的父目录：登记父目录只登记它自己，不聚合嵌套仓库。
    let parent = dir.path().join("parent");
    let nested = parent.join("nested");
    fs::create_dir_all(&nested).expect("创建嵌套仓库目录");
    let initialized = Command::new("git")
        .args(["init", "--quiet"])
        .arg(&nested)
        .status()
        .expect("运行 git init");
    assert!(initialized.success(), "嵌套 git init 必须成功");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": plain }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "普通目录应可登记：{}", response.body);
    assert!(!plain.join(".git").exists(), "登记普通目录不得初始化 Git");

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": repository }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "真实 Git 工作树根目录应可登记：{}",
        response.body
    );

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": parent }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "父目录应可登记：{}", response.body);
    assert_same_directories(
        &zones_of(&response),
        &[plain, repository, parent.clone()],
    );

    // 嵌套仓库没有被单独登记，也没有被当作第二个 zone 聚合进来。
    let listed = zones_of(&get_over_tcp(server.addr(), "/api/zones").await);
    assert_eq!(listed.len(), 3, "登记列表只包含被明确登记的三个目录");
    let nested_canonical = fs::canonicalize(&nested).expect("嵌套仓库存在");
    assert!(
        !listed.iter().any(|path| fs::canonicalize(path)
            .map(|path| path == nested_canonical)
            .unwrap_or(false)),
        "嵌套仓库不得被自动登记"
    );
}

/// 取消登记只移除登记项，不删除目录或文件；目录已被外部删除时也仍可取消登记。
#[tokio::test]
async fn unregistering_keeps_every_directory_and_file_intact() {
    let dir = TempDir::new("zone unregister");
    let zone = dir.path().join("keep");
    let nested = zone.join("nested");
    fs::create_dir_all(&nested).expect("创建待登记目录");
    fs::write(zone.join("note.txt"), "keep this content").expect("写入文件");
    fs::write(nested.join("inner.txt"), "nested content").expect("写入嵌套文件");
    let before_note = fs::read(zone.join("note.txt")).expect("取消登记前读取文件");
    let before_inner = fs::read(nested.join("inner.txt")).expect("取消登记前读取嵌套文件");

    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记应成功：{}", response.body);

    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/remove",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "取消登记应成功：{}", response.body);
    assert_eq!(zones_of(&response), Vec::<PathBuf>::new());

    // 独立读取原来的目录与文件：取消登记没有删除、也没有改动任何内容。
    assert!(zone.is_dir(), "取消登记不得删除目录");
    assert_eq!(
        fs::read(zone.join("note.txt")).expect("原文件仍可读取"),
        before_note,
        "取消登记前后的文件内容必须逐字节相同"
    );
    assert_eq!(
        fs::read(nested.join("inner.txt")).expect("嵌套文件仍可读取"),
        before_inner,
        "取消登记前后的嵌套文件内容必须逐字节相同"
    );

    // 重复取消登记是明确拒绝，不是静默成功。
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/zones/remove",
            &json!({ "path": zone }).to_string(),
        )
        .await,
        404,
        "not-registered",
    );

    // 目录在登记后被外部删除：取消登记仍然只处理登记项，不要求目录还存在。
    let vanished = dir.path().join("vanished");
    fs::create_dir_all(&vanished).expect("创建第二个目录");
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": vanished }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "第二个目录应可登记：{}",
        response.body
    );
    fs::remove_dir(&vanished).expect("外部删除目录");
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones/remove",
        &json!({ "path": vanished }).to_string(),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "目录已不存在时仍应能取消登记：{}",
        response.body
    );
    assert_eq!(zones_of(&response), Vec::<PathBuf>::new());
}

/// 受测的真实交付进程：启动后从 stdout 取得它实际监听的端口。
struct Panel {
    child: Child,
    port: u16,
}

impl Panel {
    fn start(config_path: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .current_dir(common::repo_fixtures_dir().parent().unwrap())
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动 herdr-bot 进程");

        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        for source in [
            Box::new(child.stdout.take().expect("stdout")) as Box<dyn std::io::Read + Send>,
            Box::new(child.stderr.take().expect("stderr")) as Box<dyn std::io::Read + Send>,
        ] {
            let log = std::sync::Arc::clone(&log);
            std::thread::spawn(move || {
                let mut source = source;
                let mut buffer = [0_u8; 4096];
                loop {
                    match source.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => log
                            .lock()
                            .expect("读取日志")
                            .extend_from_slice(&buffer[..read]),
                    }
                }
            });
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let port = loop {
            let text = String::from_utf8_lossy(&log.lock().expect("读取日志")).into_owned();
            if let Some(port) = text.lines().find_map(|line| {
                let rest = line.split("http://").nth(1)?;
                rest.trim_end_matches('/')
                    .rsplit(':')
                    .next()?
                    .parse::<u16>()
                    .ok()
            }) {
                break port;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "herdr-bot 没有在 30 秒内打印监听端口：{text}"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        };

        Self { child, port }
    }

    fn addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], self.port))
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 写一份隔离真实进程的配置：回环临时端口、关闭 mDNS、两个固定操作指向安全夹具。
fn write_process_config(dir: &TempDir, registry: &Path) -> PathBuf {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let config_path = dir.path().join("herdr-bot.toml");
    fs::write(
        &config_path,
        format!(
            "listen_addr = \"127.0.0.1:0\"\n\n\
             [operations.start]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [operations.stop]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [mdns]\nenabled = false\n\n\
             [herdr]\nworkspace_name = 'bot-playground'\n\n\
             [zones]\nregistry_file = '{registry}'\n",
            script = script.display(),
            operations = operations.display(),
            registry = registry.display(),
        ),
    )
    .expect("写临时配置");
    config_path
}

/// 两个独立调用者读取同一列表；真实进程重启后列表仍在（不以同页缓存或模拟重启作为证据）。
#[tokio::test]
async fn two_callers_share_the_registry_and_it_survives_a_real_process_restart() {
    let dir = TempDir::new("zone process restart");
    let zone = dir.path().join("shared-project");
    fs::create_dir_all(&zone).expect("创建待登记目录");
    fs::write(zone.join("note.txt"), "shared content").expect("写入目录内容");
    let registry = dir.path().join("zones.json");
    let config_path = write_process_config(&dir, &registry);

    // 第一个真实进程：登记后由两条独立 TCP 连接读取同一份列表。
    let first = Panel::start(&config_path);
    let response = post_json_over_tcp(
        first.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记应成功：{}", response.body);
    let caller_a = zones_of(&get_over_tcp(first.addr(), "/api/zones").await);
    let caller_b = zones_of(&get_over_tcp(first.addr(), "/api/zones").await);
    assert_same_directories(&caller_a, &[zone.clone()]);
    assert_same_directories(&caller_b, &[zone.clone()]);
    drop(first);

    // 同一个配置文件启动的第二个真实进程：登记列表来自磁盘，而不是上一个进程的内存。
    let restarted = Panel::start(&config_path);
    assert_same_directories(
        &zones_of(&get_over_tcp(restarted.addr(), "/api/zones").await),
        &[zone.clone()],
    );

    // 重启后的进程仍可取消登记，且取消后原目录与文件未受影响。
    let response = post_json_over_tcp(
        restarted.addr(),
        "/api/zones/remove",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "取消登记应成功：{}", response.body);
    assert_eq!(
        fs::read_to_string(zone.join("note.txt")).expect("原文件仍可读"),
        "shared content"
    );
}

/// 改一个文件的只读属性；用来构造「读取仍成功、写入失败」的真实存储条件。
fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = fs::metadata(path).expect("读取文件权限").permissions();
    permissions.set_readonly(readonly);
    fs::set_permissions(path, permissions).expect("设置文件权限");
}

/// 损坏的登记文件是明确错误：不伪装成空列表，也不被登记或重启静默覆盖。
#[tokio::test]
async fn corrupt_registry_is_reported_and_never_overwritten() {
    let dir = TempDir::new("zone corrupt registry");
    let zone = dir.path().join("candidate");
    fs::create_dir_all(&zone).expect("创建待登记目录");
    let registry = dir.path().join("zones.json");
    let corrupt = b"{ this is not a zone registry";
    fs::write(&registry, corrupt).expect("写入损坏内容");
    let before = fs::read(&registry).expect("读取损坏内容");

    let config_path = write_process_config(&dir, &registry);
    let panel = Panel::start(&config_path);

    let list = get_over_tcp(panel.addr(), "/api/zones").await;
    assert_rejected(&list, 500, "registry-corrupt");
    let list_body: Value = serde_json::from_str(&list.body).expect("错误响应必须是 JSON");
    assert!(
        list_body.get("zones").is_none(),
        "读取失败不得伪装成空列表：{list_body}"
    );

    let response = post_json_over_tcp(
        panel.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_rejected(&response, 500, "registry-corrupt");
    assert_eq!(
        fs::read(&registry).expect("登记文件仍应存在"),
        before,
        "登记失败不得覆盖损坏的原文件"
    );
    drop(panel);

    // 真实进程重启后仍然是同一个明确错误，原文件逐字节不变。
    let restarted = Panel::start(&config_path);
    assert_rejected(
        &get_over_tcp(restarted.addr(), "/api/zones").await,
        500,
        "registry-corrupt",
    );
    assert_eq!(
        fs::read(&registry).expect("登记文件仍应存在"),
        before,
        "重启不得修复或覆盖损坏的原文件"
    );
}

/// 读取失败（登记路径是目录）是明确错误：不伪装成空列表，也不覆盖原有内容。
#[tokio::test]
async fn unreadable_registry_is_reported_and_never_overwritten() {
    let dir = TempDir::new("zone unreadable registry");
    let zone = dir.path().join("candidate");
    fs::create_dir_all(&zone).expect("创建待登记目录");

    // 用一个同名的真实目录占住登记文件位置：读取它必然失败。
    let registry = dir.path().join("zones.json");
    fs::create_dir(&registry).expect("创建占位目录");
    let sentinel = registry.join("sentinel.txt");
    fs::write(&sentinel, "sentinel content").expect("写占位内容");

    let config_path = write_process_config(&dir, &registry);
    let panel = Panel::start(&config_path);

    let list = get_over_tcp(panel.addr(), "/api/zones").await;
    assert_rejected(&list, 500, "registry-unreadable");
    let list_body: Value = serde_json::from_str(&list.body).expect("错误响应必须是 JSON");
    assert!(
        list_body.get("zones").is_none(),
        "读取失败不得伪装成空列表：{list_body}"
    );

    let response = post_json_over_tcp(
        panel.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_rejected(&response, 500, "registry-unreadable");
    assert!(registry.is_dir(), "登记失败不得用文件替换原目录");
    assert_eq!(
        fs::read_to_string(&sentinel).expect("占位内容仍可读取"),
        "sentinel content",
        "登记失败不得改动原内容"
    );
    drop(panel);

    let restarted = Panel::start(&config_path);
    assert_rejected(
        &get_over_tcp(restarted.addr(), "/api/zones").await,
        500,
        "registry-unreadable",
    );
    assert_eq!(
        fs::read_to_string(&sentinel).expect("占位内容仍可读取"),
        "sentinel content",
        "重启不得改动原内容"
    );
}

/// 写入失败是明确错误：不报告成功，也不改动已持久化的原列表。
#[tokio::test]
async fn write_failure_is_reported_and_keeps_the_previous_registry() {
    let dir = TempDir::new("zone write failure");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    fs::create_dir_all(&first).expect("创建第一个目录");
    fs::create_dir_all(&second).expect("创建第二个目录");

    let registry = dir.path().join("zones.json");
    let config_path = write_process_config(&dir, &registry);
    let panel = Panel::start(&config_path);

    let response = post_json_over_tcp(
        panel.addr(),
        "/api/zones",
        &json!({ "path": first }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "第一次登记应成功：{}", response.body);
    let before = fs::read(&registry).expect("登记文件必须存在");

    // 让登记文件只读：读取仍然成功，但原子替换必然失败。
    set_readonly(&registry, true);
    let response = post_json_over_tcp(
        panel.addr(),
        "/api/zones",
        &json!({ "path": second }).to_string(),
    )
    .await;
    assert_rejected(&response, 500, "registry-write-failed");
    let after = fs::read(&registry).expect("登记文件仍应存在");
    let listed = zones_of(&get_over_tcp(panel.addr(), "/api/zones").await);
    set_readonly(&registry, false);

    assert_eq!(before, after, "写入失败不得改动原登记文件");
    assert_same_directories(&listed, &[first.clone()]);
    assert!(
        !dir.path().join("zones.json.tmp").exists(),
        "失败的临时写入必须被清理"
    );
    drop(panel);

    // 重启后读取到的仍是写入失败之前的列表。
    let restarted = Panel::start(&config_path);
    assert_same_directories(
        &zones_of(&get_over_tcp(restarted.addr(), "/api/zones").await),
        &[first],
    );
}
