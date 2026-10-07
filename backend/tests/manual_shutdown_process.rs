//! 真实 omp-bot 进程的关机安排生命周期（P35、P45）。
//!
//! 这里用**真实二进制**（`env!("CARGO_BIN_EXE_omp-bot")`）：只有真正的进程启动才能证明「安排不持久化、
//! 重启即清空」与「配置只在启动期读取一次，运行中改写文件不生效」。
//!
//! **绝不真的关机**：用例只用很长的倒计时（600 / 3600 秒），并在这些安排到期之前结束进程，因此到期
//! 路径不会被触发。到期路径本身由 `tests/shutdown.rs` 与 `tests/monitoring.rs` 用记录型替身覆盖。

mod common;

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::sse::SseConnection;
use common::{HttpResponse, TempDir, post_json_over_tcp, post_over_tcp};
use serde_json::{Value, json};

/// 用例的倒计时：远大于用例本身的时长，因此进程总是在到期之前被结束。
const RESTART_COUNTDOWN_SECONDS: u32 = 3600;

/// 「不热加载」用例的倒计时：启动时写 600，随后把文件改成 1200。
const STARTUP_COUNTDOWN_SECONDS: u32 = 600;
const REWRITTEN_COUNTDOWN_SECONDS: u32 = 1200;

/// 受测的真实 omp-bot 进程。
struct Panel {
    child: Child,
    log: Arc<Mutex<Vec<u8>>>,
    port: u16,
    _config_dir: TempDir,
}

impl Panel {
    /// 用给定配置文件启动真实二进制；返回时已经拿到它实际监听的端口。
    fn start(config_path: &Path, name: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omp-bot"))
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动 omp-bot 主界面进程");

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

    /// 建立一条观察连接并读掉首帧，返回首帧里的关机状态。
    async fn observe(&self) -> Value {
        let mut connection = SseConnection::open(self.addr(), "/api/sessions/events", &[]).await;
        assert_eq!(connection.status, 200, "读路径必须可用");
        let first = connection.next_frame().await;
        first.named("snapshot");
        first.json()["shutdown"].clone()
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

/// 验收 P35：后端进程重启后监听与倒计时均为空——安排只属于当前进程。
#[tokio::test]
async fn restarting_the_real_process_clears_the_arrangement() {
    let dir = TempDir::new("shutdown restart");
    let config_path = write_config(&dir, RESTART_COUNTDOWN_SECONDS);

    let mut first = Panel::start(&config_path, "shutdown restart a");
    let response = first.start_request().await;
    let arranged = arrangement_of(&response);
    assert_eq!(arranged["phase"], serde_json::json!("counting-down"));
    assert_eq!(
        arranged_millis(&arranged),
        u64::from(RESTART_COUNTDOWN_SECONDS) * 1000,
        "启动期配置的时长必须如实生效：{arranged}"
    );
    assert_eq!(
        first.observe().await["phase"],
        serde_json::json!("counting-down"),
        "同一进程的观察者能看到这个安排"
    );

    // 结束进程（安排随之消失），再用**同一份配置**启动一个新进程。
    first.stop();
    assert!(!first.is_running(), "进程必须已经结束");

    let mut restarted = Panel::start(&config_path, "shutdown restart b");
    let first_frame = restarted.observe().await;
    assert_eq!(
        first_frame["phase"],
        serde_json::json!("none"),
        "重启后的首帧不得恢复任何旧安排：{first_frame}"
    );
    assert!(
        first_frame.get("arrangementId").is_none() && first_frame.get("deadline").is_none(),
        "首帧的可观察字段只有「没有安排」：{first_frame}"
    );
    assert!(restarted.is_running(), "重启后的进程必须正常服务");
    restarted.stop();
}

/// 验收 P35（完成后关机监听）：真实进程里分别建立监听集合与自动倒计时，重启后首帧都为空。
///
/// 监听集合与自动倒计时都只属于当前进程：重启不恢复旧监听，也不恢复旧安排。
#[tokio::test]
async fn restarting_the_real_process_clears_monitoring_and_the_automatic_arrangement() {
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
    assert_eq!(first.observe().await["monitoredSessionIds"], json!(["sess-a"]));

    first.stop();
    assert!(!first.is_running(), "进程必须已经结束");

    // 重启后：首帧没有旧监听。
    let mut restarted = Panel::start(&config_path, "monitoring restart b");
    let empty = restarted.observe().await;
    assert_eq!(empty["phase"], json!("none"), "重启后不得恢复旧安排：{empty}");
    assert_eq!(
        empty["monitoredSessionIds"],
        json!([]),
        "重启后不得恢复旧监听：{empty}"
    );

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
    let arranged = restarted.observe().await;
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

    // 再次重启：旧安排也不恢复，而且**不会**被重新执行（倒计时很长，进程在到期前就结束）。
    restarted.stop();
    let mut final_process = Panel::start(&config_path, "monitoring restart c");
    let first_frame = final_process.observe().await;
    assert_eq!(
        first_frame["phase"],
        json!("none"),
        "重启后不得恢复旧倒计时：{first_frame}"
    );
    assert_eq!(first_frame["monitoredSessionIds"], json!([]));
    assert!(
        first_frame.get("arrangementId").is_none() && first_frame.get("deadline").is_none(),
        "首帧的可观察字段只有「没有安排」：{first_frame}"
    );
    assert!(final_process.is_running(), "重启后的进程必须正常服务");
    final_process.stop();
}

/// 验收 P45：倒计时时长只在启动期读取一次——运行中改写配置文件不生效。
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
