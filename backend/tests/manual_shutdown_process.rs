//! 真实独立进程的关机安排生命周期（契约 P19；ADR 0005）。
//!
//! 这里启动的是**安全壳**（`examples/safe_shutdown_shell.rs`）：与交付运行时同一份路由与装配，但关机
//! 动作是记录型替身，**不接触任何系统调用**。本票的全部真实进程验证都复用这条既有 adapter；交付二进制
//! 只接受真实 Win32 关机动作，因此不由它驱动关机安排（它的启动与配置路径由 `session_window_config.rs`
//! 与 `mdns_*.rs` 覆盖）。
//!
//! 只有真正的独立进程启动才能证明「登记、监听意图与安排都不持久化、重启即清空」与「倒计时时长只在
//! 启动期读取一次，运行中改写文件不生效」：这些都是进程内状态，跨进程不可恢复。到期输出本身由
//! `tests/shutdown.rs`、`tests/monitoring.rs` 与 `tests/countdown_independence.rs` 用记录型替身覆盖，
//! 这里用长倒计时让进程总是在到期之前结束。

mod common;

use std::ffi::OsStr;
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use common::sse::SseConnection;
use common::{HttpResponse, TempDir, post_json_over_tcp, post_over_tcp};
use serde_json::{Value, json};

/// 安全壳在它的 stdout 上记录每一次关机动作调用；本票的用例都断言这个标记**从未**出现。
const SHUTDOWN_CALL_MARKER: &str = "记录到关机动作调用";

/// 用例的倒计时：远大于用例本身的时长，因此进程总是在到期之前被结束。
const RESTART_COUNTDOWN_SECONDS: u32 = 3600;

/// 「不热加载」用例的倒计时：启动时写 600，随后把文件改成 1200。
const STARTUP_COUNTDOWN_SECONDS: u32 = 600;
const REWRITTEN_COUNTDOWN_SECONDS: u32 = 1200;

/// 受测的真实独立进程（安全壳：记录型关机替身）。
struct Panel {
    child: Child,
    log: Arc<Mutex<Vec<u8>>>,
    port: u16,
    _config_dir: TempDir,
}

impl Panel {
    /// 用给定配置文件启动安全壳；返回时已经拿到它实际监听的端口。
    fn start(config_path: &Path, name: &str) -> Self {
        let mut child = Command::new(SAFE_SHELL_BINARY.as_path())
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动安全壳进程");

        let log = Arc::new(Mutex::new(Vec::new()));
        for source in [
            Box::new(child.stdout.take().expect("stdout")) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().expect("stderr")) as Box<dyn Read + Send>,
        ] {
            capture(source, Arc::clone(&log));
        }

        let mut panel = Panel {
            child,
            log,
            port: 0,
            _config_dir: TempDir::new(name),
        };
        panel.port = panel.wait_for_listen_port();
        panel
    }

    fn addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    fn log_text(&self) -> String {
        String::from_utf8_lossy(&self.log.lock().expect("读取日志")).into_owned()
    }

    /// 从启动输出里取得实际监听端口（配置用 0 端口，端口由系统分配）。
    fn wait_for_listen_port(&mut self) -> u16 {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let text = self.log_text();
            if let Some(line) = text.lines().find(|line| line.contains("[omp-bot] 监听：http://")) {
                let host_port = line
                    .split("//")
                    .nth(1)
                    .and_then(|after| after.split('/').next())
                    .expect("监听行含主机与端口");
                return host_port
                    .rsplit(':')
                    .next()
                    .expect("监听行含端口")
                    .parse()
                    .expect("监听端口是数字");
            }
            assert!(
                Instant::now() < deadline,
                "等待启动输出超时；已收到的输出：\n{text}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// 提交一次手动开始意图。
    async fn start_request(&self) -> HttpResponse {
        post_over_tcp(self.addr(), "/api/shutdown/start").await
    }

    /// 经真 TCP 提交一条带 JSON 请求体的命令（会话事件或监听命令）。
    async fn post_json(&self, path: &str, body: Value) -> HttpResponse {
        post_json_over_tcp(self.addr(), path, &body.to_string()).await
    }

    /// 注册一个在线会话并上报“工作中”。
    async fn register_working(&self, session_id: &str) {
        let registered = self
            .post_json(
                "/api/sessions/events",
                json!({"sessionId": session_id, "kind": "register", "data": {"cwd": "C:\\work"}}),
            )
            .await;
        assert_eq!(registered.status, 200, "注册应被接受：{}", registered.body);
        let working = self
            .post_json(
                "/api/sessions/events",
                json!({"sessionId": session_id, "kind": "state", "data": {"value": "working"}}),
            )
            .await;
        assert_eq!(working.status, 200, "状态上报应被接受：{}", working.body);
    }

    /// 提交一次监听命令，返回关机状态。
    async fn monitor(&self, session_id: &str, selected: bool) -> Value {
        let response = self
            .post_json(
                "/api/shutdown/monitor",
                json!({"sessionId": session_id, "selected": selected}),
            )
            .await;
        assert_eq!(response.status, 200, "监听命令应被接受：{}", response.body);
        serde_json::from_str::<Value>(&response.body).expect("回执必须是 JSON")["shutdown"].clone()
    }

    /// 建立一条观察连接并读掉首帧，返回首帧的快照（含会话列表与关机状态）。
    async fn observe(&self) -> Value {
        let mut connection = SseConnection::open(self.addr(), "/api/sessions/events", &[]).await;
        assert_eq!(connection.status, 200, "读路径必须可用");
        let first = connection.next_frame().await;
        first.named("snapshot");
        first.json()
    }

    /// 进程自启动以来是否记录过任何一次关机动作调用（安全 adapter 的调用证据）。
    fn shutdown_calls(&self) -> bool {
        self.log_text().contains(SHUTDOWN_CALL_MARKER)
    }

    fn is_running(&mut self) -> bool {
        self.child.try_wait().expect("查询子进程状态失败").is_none()
    }

    /// 结束进程：安排只属于当前进程，进程结束后不应留下任何可恢复的意图。
    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 把一路输出持续读进共享缓冲区。
fn capture(mut source: Box<dyn Read + Send>, log: Arc<Mutex<Vec<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        while let Ok(read) = source.read(&mut buffer) {
            if read == 0 {
                return;
            }
            log.lock().expect("写入日志").extend_from_slice(&buffer[..read]);
        }
    });
}

/// 安全壳可执行文件的位置：与本次测试可执行文件同一次构建的 examples 产物。
///
/// 每次都就地构建一次（已经是最新时只是一次约 50ms 的空转）：`cargo test --test <目标>` 并**不会**编译
/// examples，只按存在性判断就会驱动上一次构建留下的旧二进制——那样本目标看到的就不是当前源码。
static SAFE_SHELL_BINARY: LazyLock<PathBuf> = LazyLock::new(|| {
    let test_exe = std::env::current_exe().expect("当前测试可执行文件路径");
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("测试可执行文件位于 <target>/<profile>/deps 下")
        .to_path_buf();
    let binary = profile_dir
        .join("examples")
        .join(format!("safe_shutdown_shell{}", std::env::consts::EXE_SUFFIX));
    build_safe_shell(&profile_dir);
    assert!(
        binary.is_file(),
        "构建安全壳之后仍找不到产物：{}",
        binary.display()
    );
    binary
});

/// 就地构建安全壳：目标目录与本次测试可执行文件所在的 profile 保持一致（含 `--release` 与自定义
/// `--target-dir` 的情形），因此构建出的产物就在 [`SAFE_SHELL_BINARY`] 会找的位置。
fn build_safe_shell(profile_dir: &Path) {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    command
        .arg("build")
        .arg("--example")
        .arg("safe_shutdown_shell")
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    if profile_dir.file_name() == Some(OsStr::new("release")) {
        command.arg("--release");
    }
    if let Some(target_root) = profile_dir.parent() {
        command.arg("--target-dir").arg(target_root);
    }
    let output = command
        .output()
        .expect("运行 cargo build --example safe_shutdown_shell");
    assert!(
        output.status.success(),
        "构建安全壳失败：{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 首帧快照里的关机状态（与增量 `shutdown` 事件同一个字段名）。
fn shutdown_of(frame: &Value) -> Value {
    frame["shutdown"].clone()
}

/// 首帧快照里的会话列表。
fn sessions_of(frame: &Value) -> Vec<Value> {
    frame["sessions"].as_array().cloned().unwrap_or_default()
}

/// 写一份只发布本地回环、关闭 mDNS、两个固定操作都指向安全夹具的配置。
fn write_config(dir: &TempDir, countdown_seconds: u32) -> PathBuf {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let config_path = dir.path().join("omp-bot.toml");
    std::fs::write(
        &config_path,
        format!(
            "listen_addr = \"127.0.0.1:0\"\n\n\
             [operations.start]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [operations.stop]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [shutdown]\ncountdown_seconds = {countdown_seconds}\n\n\
             [mdns]\nenabled = false\n",
            script = script.display(),
            operations = operations.display(),
        ),
    )
    .expect("写临时配置");
    config_path
}

/// 一次开始意图的权威状态（安排标识与截止时间）。
fn arrangement_of(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "开始意图应被接受：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("回执必须是 JSON");
    body["shutdown"].clone()
}

/// 安排的时长（服务端时钟毫秒）：期望值来自配置里的独立字面量，不由实现推算。
fn arranged_millis(view: &Value) -> u64 {
    view["deadline"].as_u64().expect("倒计时中必须有截止时间")
        - view["serverTime"].as_u64().expect("服务端时间必须可读")
}

/// 验收 P19：旧实例有登记与活动倒计时时，新进程的首帧没有任何登记、没有任何安排，也不恢复任何
/// 过期意图。开始倒计时会消耗整组监听，因此监听意图由下一个独立场景覆盖，不构造两者同时存在的
/// 不可达状态。
#[tokio::test]
async fn restarting_the_process_clears_registrations_and_the_active_arrangement() {
    let dir = TempDir::new("shutdown restart");
    let config_path = write_config(&dir, RESTART_COUNTDOWN_SECONDS);

    // 旧实例：一条在线、工作中的登记，以及一次已经开始的倒计时（组织内状态只属于这个进程）。
    let mut first = Panel::start(&config_path, "shutdown restart a");
    first.register_working("sess-a").await;
    let arranged = arrangement_of(&first.start_request().await);
    assert_eq!(arranged["phase"], json!("counting-down"));
    assert_eq!(
        arranged_millis(&arranged),
        u64::from(RESTART_COUNTDOWN_SECONDS) * 1000,
        "启动期配置的时长必须如实生效：{arranged}"
    );
    let snapshot = first.observe().await;
    assert_eq!(
        shutdown_of(&snapshot)["phase"],
        json!("counting-down"),
        "同一进程的观察者能看到这个安排：{snapshot}"
    );
    assert_eq!(
        sessions_of(&snapshot).len(),
        1,
        "旧实例里有这条登记：{snapshot}"
    );
    assert!(!first.shutdown_calls(), "倒计时远未到期：安全 adapter 不得被调用");

    // 结束进程，再用**同一份配置**启动一个新进程。
    first.stop();
    assert!(!first.is_running(), "进程必须已经结束");

    let mut restarted = Panel::start(&config_path, "shutdown restart b");
    let frame = restarted.observe().await;
    let shutdown = shutdown_of(&frame);
    assert_eq!(shutdown["phase"], json!("none"), "重启后的首帧不得恢复任何旧安排：{frame}");
    assert!(
        shutdown.get("arrangementId").is_none() && shutdown.get("deadline").is_none(),
        "首帧的可观察字段只有「没有安排」：{frame}"
    );
    assert!(sessions_of(&frame).is_empty(), "重启后不得恢复旧登记：{frame}");
    assert_eq!(
        shutdown["monitoredSessionIds"],
        json!([]),
        "重启后不得恢复旧监听意图：{frame}"
    );

    // 旧登记不存在：普通上报与监听命令都按未知身份拒绝，因此旧意图不会被隐式复活。
    assert_rejected(
        &restarted
            .post_json(
                "/api/sessions/events",
                json!({"sessionId": "sess-a", "kind": "heartbeat", "data": {}}),
            )
            .await,
        404,
        "unknown-session",
    );
    assert_rejected(
        &restarted
            .post_json(
                "/api/shutdown/monitor",
                json!({"sessionId": "sess-a", "selected": true}),
            )
            .await,
        404,
        "unknown-session",
    );
    assert!(
        !restarted.shutdown_calls(),
        "重启后的进程同样不得调用安全 adapter"
    );
    assert!(restarted.is_running(), "重启后的进程必须正常服务");
    restarted.stop();
}

/// 验收 P19（监听意图与自动安排）：真实进程里分别建立监听集合与自动倒计时，重启后首帧都为空。
///
/// 监听集合与自动倒计时都只属于当前进程：重启不恢复旧监听，也不恢复旧安排，而且**不会**把旧安排
/// 补执行一次（倒计时很长，每个进程都在到期之前结束）。
#[tokio::test]
async fn restarting_the_process_clears_monitoring_and_the_automatic_arrangement() {
    let dir = TempDir::new("monitoring restart");
    let config_path = write_config(&dir, RESTART_COUNTDOWN_SECONDS);

    // 第一个进程：只建立监听集合（成员还在工作，因此还没有倒计时）。
    let mut first = Panel::start(&config_path, "monitoring restart a");
    first.register_working("sess-a").await;
    let monitored = first.monitor("sess-a", true).await;
    assert_eq!(
        monitored["monitoredSessionIds"],
        json!(["sess-a"]),
        "监听命令之后权威集合里应有这个会话：{monitored}"
    );
    let snapshot = first.observe().await;
    assert_eq!(shutdown_of(&snapshot)["monitoredSessionIds"], json!(["sess-a"]));
    assert_eq!(sessions_of(&snapshot).len(), 1, "旧实例里有这条登记：{snapshot}");
    assert!(!first.shutdown_calls(), "没有安排：安全 adapter 不得被调用");

    first.stop();
    assert!(!first.is_running(), "进程必须已经结束");

    // 重启后：首帧没有旧监听，也没有旧登记。
    let mut restarted = Panel::start(&config_path, "monitoring restart b");
    let empty = restarted.observe().await;
    let shutdown = shutdown_of(&empty);
    assert_eq!(shutdown["phase"], json!("none"), "重启后不得恢复旧安排：{empty}");
    assert_eq!(
        shutdown["monitoredSessionIds"],
        json!([]),
        "重启后不得恢复旧监听：{empty}"
    );
    assert!(sessions_of(&empty).is_empty(), "重启后不得恢复旧登记：{empty}");

    // 同一个新进程里再建立一次**自动倒计时**（成员进入等待用户输入即触发）。
    restarted.register_working("sess-a").await;
    let monitored = restarted.monitor("sess-a", true).await;
    assert_eq!(monitored["monitoredSessionIds"], json!(["sess-a"]));
    let awaiting = restarted
        .post_json(
            "/api/sessions/events",
            json!({"sessionId": "sess-a", "kind": "state", "data": {"value": "awaiting-input"}}),
        )
        .await;
    assert_eq!(awaiting.status, 200, "状态上报应被接受：{}", awaiting.body);
    let arranged = shutdown_of(&restarted.observe().await);
    assert_eq!(
        arranged["phase"],
        json!("counting-down"),
        "成员进入等待用户输入即自动形成安排：{arranged}"
    );
    assert_eq!(
        arranged["monitoredSessionIds"],
        json!([]),
        "形成安排即消耗整组监听：{arranged}"
    );
    assert!(!restarted.shutdown_calls(), "倒计时远未到期：安全 adapter 不得被调用");

    // 再次重启：旧安排也不恢复，而且不会被补执行一次。
    restarted.stop();
    let mut final_process = Panel::start(&config_path, "monitoring restart c");
    let first_frame = final_process.observe().await;
    let shutdown = shutdown_of(&first_frame);
    assert_eq!(
        shutdown["phase"],
        json!("none"),
        "重启后不得恢复旧倒计时：{first_frame}"
    );
    assert_eq!(shutdown["monitoredSessionIds"], json!([]));
    assert!(sessions_of(&first_frame).is_empty());
    assert!(
        shutdown.get("arrangementId").is_none() && shutdown.get("deadline").is_none(),
        "首帧的可观察字段只有「没有安排」：{first_frame}"
    );
    assert!(
        !final_process.shutdown_calls(),
        "跨进程不得补执行任何旧安排：安全 adapter 必须零调用"
    );
    assert!(final_process.is_running(), "重启后的进程必须正常服务");
    final_process.stop();
}

/// 一次命令必须被拒绝为给定状态码与类别。
fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("拒绝体必须是 JSON");
    assert_eq!(body["code"], json!(code), "响应体：{}", response.body);
}

/// 既有行为：倒计时时长只在启动期读取一次——运行中改写配置文件不生效（不热加载）。
#[tokio::test]
async fn a_running_process_keeps_the_countdown_it_started_with() {
    let dir = TempDir::new("shutdown no hot reload");
    let config_path = write_config(&dir, STARTUP_COUNTDOWN_SECONDS);

    let mut panel = Panel::start(&config_path, "shutdown no hot reload");
    // 运行中把文件改成另一个时长：它只影响**下一次**启动。
    write_config(&dir, REWRITTEN_COUNTDOWN_SECONDS);

    let first = arrangement_of(&panel.start_request().await);
    assert_eq!(
        arranged_millis(&first),
        u64::from(STARTUP_COUNTDOWN_SECONDS) * 1000,
        "运行中的实例继续用启动时读到的时长（不热加载）：{first}"
    );

    // 重复发起沿用同一个截止时间：既没有重置，也没有按新文件延长。
    let again = arrangement_of(&panel.start_request().await);
    assert_eq!(
        again["arrangementId"], first["arrangementId"],
        "重复发起沿用同一安排：{again}"
    );
    assert_eq!(again["deadline"], first["deadline"], "截止时间不得变化");

    panel.stop();
}
