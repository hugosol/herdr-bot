//! 启动期配置决定会话登记的两个独立窗口（契约 P1–P7）。
//!
//! 走交付运行时同一份装配：启动输入经 [`herdr_bot::Config::from_toml`] 解析（配置进入实例的唯一入口），
//! 窗口策略只由 `config.sessions.policy()` 注入会话表；会话事件经真实 TCP 写入，登记经真实 SSE 读取
//! （页面收到的就是这些帧），时刻由受控时钟精确驱动。
//!
//! 期望值全部是独立字面值：就是配置里写的秒数，以及由它算出的两个窗口之和。受控时刻只用来把边界
//! 变成精确时刻，不改变阈值本身。
//!
//! 本票只用真实 TCP／SSE 与真实进程观察行为：不读私有字段、不断言源文本，也不新增测试端点。

mod common;

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::sse::SseConnection;
use common::{HttpResponse, TempDir, TestServer, post_json_over_tcp};
use herdr_bot::PowerOff;
use herdr_bot::PowerOffFailure;
use herdr_bot::clock::ManualClock;
use herdr_bot::countdown::ShutdownState;
use herdr_bot::sessions::SessionsState;
use serde_json::{Value, json};

/// 会话窗口用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        panic!("会话窗口用例不应触发关机");
    }
}

/// 一份合法的启动输入：除 `[sessions]` 段外都固定，段本身由调用方给出（空串表示整段缺省）。
///
/// 关机倒计时取一个很长的值：这些用例只在会话窗口上做判定，万一形成了安排也在很久之后到期。
fn config_text(sessions: &str) -> String {
    format!(
        "listen_addr = \"127.0.0.1:0\"\n\n\
         [operations.start]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
         [operations.stop]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
         [shutdown]\ncountdown_seconds = 3600\n\n\
         [mdns]\nenabled = false\n\n\
         {sessions}"
    )
}

/// 受控时刻下的实例：启动输入决定窗口策略，观察走真 TCP／SSE。
struct WindowServer {
    server: TestServer,
    sessions: Arc<SessionsState>,
    shutdown: Arc<ShutdownState>,
    clock: Arc<ManualClock>,
}

impl WindowServer {
    /// 从启动输入文本装配实例：策略只来自这次解析出来的配置。
    async fn start(sessions: &str) -> Self {
        let config =
            herdr_bot::Config::from_toml(&config_text(sessions)).expect("启动输入必须可解析");
        let clock = Arc::new(ManualClock::new());
        let sessions = Arc::new(SessionsState::with_policy_and_clock(
            config.sessions.policy(),
            clock.clone(),
        ));
        let (app, state) =
            herdr_bot::app_with_state(&config, Arc::new(NeverPowerOff), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            sessions,
            shutdown: state.shutdown,
            clock,
        }
    }

    /// 把受控时刻向前推进：窗口边界因此可以精确取到。
    fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&self, body: &str) -> HttpResponse {
        post_json_over_tcp(self.server.addr(), "/api/sessions/events", body).await
    }

    /// 跑一遍维护：过期行翻为离线、超过两个窗口的行清除，随后重判监听集合。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    /// 经真 TCP 提交一次完成后关机监听命令。
    async fn post_monitor(&self, session_id: &str, selected: bool) -> HttpResponse {
        post_json_over_tcp(
            self.server.addr(),
            "/api/shutdown/monitor",
            &json!({ "sessionId": session_id, "selected": selected }).to_string(),
        )
        .await
    }

    /// 经真 TCP 上报一条 agent 状态。
    async fn post_state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post(&json!({
            "sessionId": session_id,
            "kind": "state",
            "data": { "value": value },
        })
        .to_string())
        .await
    }

    /// 从一条新的观察连接首帧读取权威关机状态（页面同一条读取路径）。
    async fn shutdown(&self) -> Value {
        let mut connection =
            SseConnection::open(self.server.addr(), "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        first.json()["shutdown"].clone()
    }

    /// 从一条新的观察连接首帧读取当前会话列表（页面同一条读取路径）。
    async fn sessions(&self) -> Vec<Value> {
        let mut connection =
            SseConnection::open(self.server.addr(), "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        first.json()["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

/// 注册一个会话（唯一建行入口）。
fn register(session_id: &str) -> String {
    json!({
        "sessionId": session_id,
        "kind": "register",
        "data": { "cwd": "C:/work/omp" },
    })
    .to_string()
}

/// 首帧会话列表里的某个会话；缺了就是用例失败。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("会话列表里应有 {session_id}：{sessions:?}"))
}

/// 首帧会话列表里是否还有该会话。
fn has_session(sessions: &[Value], session_id: &str) -> bool {
    sessions
        .iter()
        .any(|session| session["sessionId"] == json!(session_id))
}

/// 从运行 interface 观察一条登记的四个阈值边界。
///
/// `online` 与 `retention` 是配置里写的独立字面值：上报年龄恰好等于 `online` 时仍在线，多一秒离线；
/// 恰好等于两个窗口之和 `online + retention` 时维护后仍保留，多一秒即被维护清除。
async fn assert_window_boundaries(sessions: &str, online: u64, retention: u64) {
    let server = WindowServer::start(sessions).await;
    let accepted = server.post(&register("sess-window")).await;
    assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);

    // t = online：恰好等于在线有效期，仍在在线一侧。
    server.advance(Duration::from_secs(online));
    let list = server.sessions().await;
    assert_eq!(
        session_of(&list, "sess-window")["online"],
        json!(true),
        "恰好等于在线有效期仍必须在线（t={online}）：{list:?}"
    );

    // t = online + 1：超过在线有效期一秒，离线但登记仍保留。
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    let list = server.sessions().await;
    assert!(
        has_session(&list, "sess-window"),
        "超过在线有效期只是离线，登记仍保留（t={}）：{list:?}",
        online + 1
    );
    assert_eq!(
        session_of(&list, "sess-window")["online"],
        json!(false),
        "超过在线有效期一秒即离线（t={}）：{list:?}",
        online + 1
    );

    // t = online + retention：恰好等于两个窗口之和，维护后仍保留。
    server.advance(Duration::from_secs(retention - 1));
    server.maintain().await;
    let list = server.sessions().await;
    assert!(
        has_session(&list, "sess-window"),
        "恰好等于两个窗口之和仍必须保留（t={}）：{list:?}",
        online + retention
    );

    // t = online + retention + 1：超过两个窗口之和，维护清除登记。
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    let list = server.sessions().await;
    assert!(
        !has_session(&list, "sess-window"),
        "超过两个窗口之和必须被维护清除（t={}）：{list:?}",
        online + retention + 1
    );
}

/// 验收 P1：合法启动输入缺少 `sessions` 段时，采用在线 10 秒、离线保留 30 秒。
///
/// 两个阈值（10／40 秒）从运行 interface 观察，期望值是独立字面值。
#[tokio::test]
async fn the_default_windows_apply_when_the_sessions_section_is_absent() {
    // 先确认这一份启动输入确实没有给出任何会话窗口。
    let config = herdr_bot::Config::from_toml(&config_text("")).expect("缺 [sessions] 段仍必须可解析");
    assert_eq!(config.sessions.online_timeout_seconds.get(), 10);
    assert_eq!(config.sessions.offline_retention_seconds.get(), 30);

    // 10／40 秒边界：在线有效期 10 秒、两个窗口之和 40 秒。
    assert_window_boundaries("", 10, 30).await;
}

/// 验收 P2：只缺一个会话配置字段时，仅该字段采用对应默认值。
///
/// 只配置 online=7 → 7／37 秒边界（保留期仍是缺省的 30 秒）。
#[tokio::test]
async fn configuring_only_the_online_window_keeps_the_default_retention() {
    assert_window_boundaries("[sessions]\nonline_timeout_seconds = 7\n", 7, 30).await;
}

/// 验收 P2（另一半）：只配置 retention=13 → 10／23 秒边界（在线有效期仍是缺省的 10 秒）。
#[tokio::test]
async fn configuring_only_the_retention_window_keeps_the_default_online_window() {
    assert_window_boundaries("[sessions]\noffline_retention_seconds = 13\n", 10, 13).await;
}

/// 验收 P5：显式 online=7、retention=13 时，两个阈值都按配置移动。
///
/// 期望值使用独立字面量：age=7 在线、age=8 离线、age=20 维护后仍保留、age=21 维护后清除。
#[tokio::test]
async fn explicitly_configured_windows_move_both_registration_thresholds() {
    assert_window_boundaries(
        "[sessions]\nonline_timeout_seconds = 7\noffline_retention_seconds = 13\n",
        7,
        13,
    )
    .await;
}

/// 验收 P3（下界）：两个窗口分别取 1 时都合法启动，并按所选值运行。
///
/// 期望值是配置里写的独立字面值：online=1 → 1／31 秒边界（缺省保留期 30 秒）；
/// retention=1 → 10／11 秒边界（缺省在线有效期 10 秒）。缺省值会给出明显不同的边界，
/// 因此这两组观察同时证明合法极小值没有被替换成缺省。
#[tokio::test]
async fn the_smallest_legal_windows_take_effect_as_configured() {
    assert_window_boundaries("[sessions]\nonline_timeout_seconds = 1\n", 1, 30).await;
    assert_window_boundaries("[sessions]\noffline_retention_seconds = 1\n", 10, 1).await;
}

/// 验收 P3（上界）：两个窗口分别取 4294967295 时都合法启动，并按所选值运行。
///
/// 观察点选在**缺省值**会给出不同结果的地方：缺省在线有效期 10 秒时 t=11 已离线，缺省两个窗口之和
/// 40 秒时 t=41 已清除。若合法上界被替换成缺省，这两处观察都会翻转。
#[tokio::test]
async fn the_largest_legal_windows_are_kept_instead_of_falling_back_to_defaults() {
    // 解析层：字段被如实保留。
    let config = herdr_bot::Config::from_toml(&config_text(
        "[sessions]\nonline_timeout_seconds = 4294967295\noffline_retention_seconds = 4294967295\n",
    ))
    .expect("两个合法上界都必须可解析");
    assert_eq!(config.sessions.online_timeout_seconds.get(), 4294967295);
    assert_eq!(config.sessions.offline_retention_seconds.get(), 4294967295);

    // 运行 interface：online=4294967295（保留期取缺省 30 秒）时，t=11 仍在线。
    let large_online = WindowServer::start("[sessions]\nonline_timeout_seconds = 4294967295\n").await;
    let accepted = large_online.post(&register("sess-large-online")).await;
    assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);
    large_online.advance(Duration::from_secs(11));
    let list = large_online.sessions().await;
    assert_eq!(
        session_of(&list, "sess-large-online")["online"],
        json!(true),
        "缺省在线有效期 10 秒时 t=11 早已离线；上界必须被如实采用：{list:?}"
    );

    // 运行 interface：retention=4294967295（在线有效期取缺省 10 秒）时，t=41 仍保留。
    let large_retention =
        WindowServer::start("[sessions]\noffline_retention_seconds = 4294967295\n").await;
    let accepted = large_retention.post(&register("sess-large-retention")).await;
    assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);
    large_retention.advance(Duration::from_secs(41));
    large_retention.maintain().await;
    let list = large_retention.sessions().await;
    assert_eq!(
        session_of(&list, "sess-large-retention")["online"],
        json!(false),
        "t=41 只是离线，登记仍在：{list:?}"
    );
}

/// 验收 P6：两个窗口都取合法上界时启动成功，总阈值按独立字面值 8589934590 秒判定。
///
/// 8589934590 = 4294967295 + 4294967295。受控时刻把总阈值变成精确时刻，因此不需要真的等：
/// 恰好等于总阈值仍保留，多一秒由维护清除；整个过程不溢出、不提前过期、不崩溃。
#[tokio::test]
async fn the_two_largest_windows_sum_without_overflow_or_early_expiry() {
    const TOTAL_WINDOW_SECONDS: u64 = 8589934590;

    // 「启动成功」：两个上界都能被这份启动输入接受。
    let config = herdr_bot::Config::from_toml(&config_text(
        "[sessions]\nonline_timeout_seconds = 4294967295\noffline_retention_seconds = 4294967295\n",
    ))
    .expect("两个合法上界都必须可启动");
    assert_eq!(config.sessions.online_timeout_seconds.get(), 4294967295);
    assert_eq!(config.sessions.offline_retention_seconds.get(), 4294967295);

    let server = WindowServer::start(
        "[sessions]\nonline_timeout_seconds = 4294967295\noffline_retention_seconds = 4294967295\n",
    )
    .await;
    let accepted = server.post(&register("sess-total")).await;
    assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);

    // t = 8589934590：恰好等于两个窗口之和，维护后仍保留（早已离线）。
    server.advance(Duration::from_secs(TOTAL_WINDOW_SECONDS));
    server.maintain().await;
    let list = server.sessions().await;
    assert_eq!(
        session_of(&list, "sess-total")["online"],
        json!(false),
        "超过单个窗口很久之后是离线，但仍在两个窗口之和以内：{list:?}"
    );

    // t = 8589934591：超过两个窗口之和一秒，由维护清除。
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    let list = server.sessions().await;
    assert!(
        !has_session(&list, "sess-total"),
        "超过两个合法窗口之和一秒必须被维护清除，而不是提前清除或永不过期：{list:?}"
    );
}

/// 验收（策略贯通监听新增资格）：新增监听的资格用的是配置里的在线有效期。
///
/// online=7：注册 8 秒后该会话已离线（缺省 10 秒窗口下此刻仍会算在线），提交监听必须被明确拒绝；
/// 同一条会话在保留期内刷新上报后即可新增——因此这次拒绝来自「上报过期」，而不是请求形状。
#[tokio::test]
async fn the_configured_online_window_decides_monitoring_eligibility() {
    let server = WindowServer::start(
        "[sessions]\nonline_timeout_seconds = 7\noffline_retention_seconds = 13\n",
    )
    .await;
    let accepted = server.post(&register("sess-late")).await;
    assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);
    let working = server.post_state("sess-late", "working").await;
    assert_eq!(working.status, 200, "状态上报应被接受：{}", working.body);

    server.advance(Duration::from_secs(8));
    let refusal = server.post_monitor("sess-late", true).await;
    assert_eq!(
        refusal.status, 409,
        "离线会话不能新增监听（配置的在线有效期是 7 秒）：{}",
        refusal.body
    );
    assert!(
        refusal.body.contains("session-not-eligible"),
        "拒绝必须给出明确类别：{}",
        refusal.body
    );

    // 保留期内的一次普通上报（心跳不改变 agent 状态）之后即可新增。
    let heartbeat = server
        .post(
            &json!({ "sessionId": "sess-late", "kind": "heartbeat", "data": {} }).to_string(),
        )
        .await;
    assert_eq!(heartbeat.status, 200, "心跳应被接受：{}", heartbeat.body);
    let accepted = server.post_monitor("sess-late", true).await;
    assert_eq!(accepted.status, 200, "刷新后即可新增：{}", accepted.body);
    let receipt: Value = serde_json::from_str(&accepted.body).expect("回执必须是 JSON");
    let shutdown = receipt["shutdown"].clone();
    assert_eq!(
        shutdown["monitoredSessionIds"],
        json!(["sess-late"]),
        "刷新后的新增必须生效：{shutdown}"
    );
}

/// 验收（策略贯通自动触发）：配置的在线有效期同样决定整组是否被离线成员阻塞。
///
/// 同一段时间线分别跑在两个实例上：缺省窗口（在线 10 秒）与配置窗口（在线 7 秒）。
/// 两者只差一条配置——8 秒前最后一次上报的成员在配置窗口下已离线，整组因此继续等待；
/// 在缺省窗口下它仍在线，整组当场满足条件。配置窗口实例里该成员刷新上报后同样立即满足条件。
#[tokio::test]
async fn the_configured_online_window_blocks_and_then_allows_the_automatic_arrangement() {
    /// 跑同一段时间线：两个成员都新增监听，A 先进入等待用户输入，B 在 t=9 才进入。
    async fn run(sessions: &str) -> WindowServer {
        let server = WindowServer::start(sessions).await;
        for session in ["sess-a", "sess-b"] {
            let accepted = server.post(&register(session)).await;
            assert_eq!(accepted.status, 200, "注册应被接受：{}", accepted.body);
            let working = server.post_state(session, "working").await;
            assert_eq!(working.status, 200, "状态上报应被接受：{}", working.body);
            let monitored = server.post_monitor(session, true).await;
            assert_eq!(monitored.status, 200, "在线工作中即可新增：{}", monitored.body);
        }

        // t=1：A 进入等待用户输入；B 仍工作中，整组继续等待。
        server.advance(Duration::from_secs(1));
        let awaiting = server.post_state("sess-a", "awaiting-input").await;
        assert_eq!(awaiting.status, 200, "状态上报应被接受：{}", awaiting.body);
        assert_eq!(
            server.shutdown().await["phase"],
            json!("none"),
            "B 仍工作中时整组不满足条件"
        );

        // t=9：A 的最后一次被接受写入已过去 8 秒；B 此刻进入等待用户输入。
        server.advance(Duration::from_secs(8));
        let awaiting = server.post_state("sess-b", "awaiting-input").await;
        assert_eq!(awaiting.status, 200, "状态上报应被接受：{}", awaiting.body);
        server
    }

    // 配置窗口（在线 7 秒）：A 在 t=9 时已离线，因此整组被阻塞，两个成员都还在集合里。
    let configured = run("[sessions]\nonline_timeout_seconds = 7\noffline_retention_seconds = 13\n")
        .await;
    let shutdown = configured.shutdown().await;
    assert_eq!(
        shutdown["phase"],
        json!("none"),
        "配置窗口下的离线成员必须阻塞自动触发：{shutdown}"
    );
    assert_eq!(
        shutdown["monitoredSessionIds"],
        json!(["sess-a", "sess-b"]),
        "被阻塞只是不触发，成员不移出：{shutdown}"
    );

    // 同一瞬间的缺省窗口（在线 10 秒）：A 仍在线，整组当场形成安排——差别只来自配置。
    let default_windows = run("").await;
    let shutdown = default_windows.shutdown().await;
    assert_eq!(
        shutdown["phase"],
        json!("counting-down"),
        "缺省 10 秒窗口下同一时间线应已满足条件：{shutdown}"
    );
    assert_eq!(
        shutdown["monitoredSessionIds"],
        json!([]),
        "形成安排即消耗整组监听：{shutdown}"
    );

    // 配置窗口的实例：A 刷新上报（保留期内仍被接受）后，同一组立即满足条件。
    let heartbeat = configured
        .post(&json!({ "sessionId": "sess-a", "kind": "heartbeat", "data": {} }).to_string())
        .await;
    assert_eq!(heartbeat.status, 200, "心跳应被接受：{}", heartbeat.body);
    let shutdown = configured.shutdown().await;
    assert_eq!(
        shutdown["phase"],
        json!("counting-down"),
        "刷新后同一组立即满足条件：{shutdown}"
    );
}

/// 一份真实进程可用的启动输入：回环随机端口、关闭 mDNS、关机动作用长倒计时。
///
/// 本票的进程用例**从不**提交任何关机意图（不手动开始、不设置监听），因此关机动作用永不可达；
/// 长倒计时只是把「万一」也压到一个小时之后。
fn process_config_text(sessions: &str) -> String {
    format!(
        "listen_addr = \"127.0.0.1:0\"\n\n\
         [operations.start]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
         [operations.stop]\nscript = 'unused.bat'\nworking_directory = '.'\n\n\
         [shutdown]\ncountdown_seconds = 3600\n\n\
         [mdns]\nenabled = false\n\n\
         {sessions}"
    )
}

/// 把一份启动输入写进临时目录，返回配置文件路径（真实进程只认文件）。
fn write_process_config(dir: &TempDir, sessions: &str) -> PathBuf {
    let config_path = dir.path().join("herdr-bot.toml");
    std::fs::write(&config_path, process_config_text(sessions)).expect("写临时配置");
    config_path
}

/// 受测的真实 herdr-bot 进程：交付二进制、真实端口、只经真 TCP／SSE 观察。
struct Panel {
    child: Child,
    log: Arc<Mutex<Vec<u8>>>,
    port: u16,
}

impl Panel {
    /// 用给定配置文件启动真实二进制；返回时已经拿到它实际监听的端口。
    fn start(config_path: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动 herdr-bot 进程");

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
        };
        panel.port = panel.wait_for_listen_port();
        panel
    }

    fn addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    /// 启动输出（交付运行时自己报告的加载结果）。
    fn log_text(&self) -> String {
        String::from_utf8_lossy(&self.log.lock().expect("读取日志")).into_owned()
    }

    /// 从启动输出里取得实际监听端口（配置用 0 端口，端口由系统分配）。
    fn wait_for_listen_port(&mut self) -> u16 {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let text = self.log_text();
            if let Some(line) = text
                .lines()
                .find(|line| line.contains("[herdr-bot] 监听：http://"))
            {
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

    /// 经真 TCP 提交一条带 JSON 请求体的会话事件。
    async fn post_json(&self, body: Value) -> HttpResponse {
        post_json_over_tcp(self.addr(), "/api/sessions/events", &body.to_string()).await
    }

    /// 注册一个会话（唯一建行入口）。
    async fn register(&self, session_id: &str) {
        let response = self
            .post_json(json!({
                "sessionId": session_id,
                "kind": "register",
                "data": { "cwd": "C:/work/omp" },
            }))
            .await;
        assert_eq!(response.status, 200, "注册应被接受：{}", response.body);
    }

    /// 从新的观察连接首帧读取当前会话列表（页面同一条读取路径）。
    async fn sessions(&self) -> Vec<Value> {
        let mut connection = SseConnection::open(self.addr(), "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        first.json()["sessions"].as_array().cloned().unwrap_or_default()
    }

    fn is_running(&mut self) -> bool {
        self.child.try_wait().expect("查询子进程状态失败").is_none()
    }

    /// 结束进程：窗口策略只属于当前进程，进程结束后不留后台行为。
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

/// 启动一个真实进程并等它自己退出，返回（退出状态, 输出）。
///
/// 非法配置下它必须在服务可用之前就结束：超过 20 秒仍在运行即判失败并结束它，用例不会挂住。
fn run_expecting_immediate_exit(config_path: &Path) -> (std::process::ExitStatus, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
        .arg("--config")
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动 herdr-bot 进程");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("查询子进程状态失败") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("非法配置必须让进程在启动阶段明确失败；它却仍在运行");
        }
        std::thread::sleep(Duration::from_millis(25));
    };

    // 进程已退出，两条管道的写端都关闭了，因此这里读到的就是它启动阶段打印的全部内容。
    let mut text = String::new();
    stdout.read_to_string(&mut text).expect("读取启动输出");
    stderr.read_to_string(&mut text).expect("读取启动错误输出");
    (status, text)
}

/// 验收 P4（实际启动）：非法会话配置让真实进程明确启动失败、非零退出、没有可用实例。
///
/// 错误必须能定位到配置（出错的字段与配置文件路径），并且不会静默回退到缺省值。
#[test]
fn a_real_process_refuses_to_start_on_an_illegal_session_window() {
    for (field, literal, why) in [
        ("online_timeout_seconds", "0", "零不是合法的窗口时长"),
        ("offline_retention_seconds", "\"13\"", "字符串不是整数秒"),
        ("offline_retention_seconds", "4294967296", "超过 u32 上界"),
    ] {
        let dir = TempDir::new("session window illegal config");
        let config_path = write_process_config(&dir, &format!("[sessions]\n{field} = {literal}\n"));

        let (status, output) = run_expecting_immediate_exit(&config_path);

        assert!(
            !status.success(),
            "{field} = {literal}（{why}）必须非零退出，实际 {status}：\n{output}"
        );
        assert!(
            output.contains(field),
            "启动错误必须定位到出错的字段（{field} = {literal}）：\n{output}"
        );
        assert!(
            output.contains(&config_path.display().to_string()),
            "启动错误必须指出配置文件：\n{output}"
        );
        assert!(
            !output.contains("[herdr-bot] 主界面已启动"),
            "非法配置下不得出现可用实例：\n{output}"
        );
    }
}

/// 验收 P7：改写配置文件不会改变已运行实例的窗口策略，重启后的新进程才使用新值。
///
/// 原实例启动为 7／13（在线 7 秒、两个窗口之和 20 秒），随后文件被改写为 10／30；
/// 新进程用改写后的文件启动（在线 10 秒、两个窗口之和 40 秒）。四个观察点都在真实进程上：
/// 原实例在 t≈8 秒仍按 7 秒在线有效期判离线，新进程在 t≈8 秒按 10 秒仍在线；
/// t≈28 秒时原实例的登记已被维护清除，新进程的登记仍在保留期内。
#[tokio::test]
async fn a_running_process_keeps_the_windows_it_started_with() {
    let dir = TempDir::new("session window no hot reload");
    let config_path = write_process_config(
        &dir,
        "[sessions]\nonline_timeout_seconds = 7\noffline_retention_seconds = 13\n",
    );

    // 原实例：启动时读到 7／13。
    let mut original = Panel::start(&config_path);
    assert!(
        original.log_text().contains("会话窗口：在线 7 秒、离线保留 13 秒"),
        "原实例必须报告它启动时读到的窗口：\n{}",
        original.log_text()
    );
    original.register("sess-original").await;

    // 运行中把文件改写为 10／30：它只影响**下一次**启动。
    write_process_config(
        &dir,
        "[sessions]\nonline_timeout_seconds = 10\noffline_retention_seconds = 30\n",
    );

    // 新进程用改写后的文件启动，与原实例同时运行。
    let mut restarted = Panel::start(&config_path);
    assert!(
        restarted.log_text().contains("会话窗口：在线 10 秒、离线保留 30 秒"),
        "新进程必须报告改写后的窗口：\n{}",
        restarted.log_text()
    );
    restarted.register("sess-restarted").await;

    // t≈8 秒：原实例仍按 7 秒在线有效期判离线（若热加载了 10 秒此刻仍会是在线）；新进程是 10 秒，仍在线。
    tokio::time::sleep(Duration::from_secs(8)).await;
    let list = original.sessions().await;
    assert_eq!(
        session_of(&list, "sess-original")["online"],
        json!(false),
        "原实例必须继续用启动时的 7 秒在线有效期（不热加载）：{list:?}"
    );
    let list = restarted.sessions().await;
    assert_eq!(
        session_of(&list, "sess-restarted")["online"],
        json!(true),
        "新进程必须使用改写后的 10 秒在线有效期：{list:?}"
    );

    // t≈28 秒：原实例的两个窗口之和是 20 秒，登记已被维护清除；新进程是 40 秒，仍在保留期内。
    tokio::time::sleep(Duration::from_secs(20)).await;
    let list = original.sessions().await;
    assert!(
        !has_session(&list, "sess-original"),
        "原实例必须继续按 7／20 秒边界清除登记：{list:?}"
    );
    let list = restarted.sessions().await;
    assert!(
        has_session(&list, "sess-restarted"),
        "新进程按 10／40 秒边界保留登记：{list:?}"
    );
    assert_eq!(
        session_of(&list, "sess-restarted")["online"],
        json!(false),
        "新进程的登记此刻已离线但仍在保留期内：{list:?}"
    );

    assert!(
        original.is_running() && restarted.is_running(),
        "两个实例都必须仍在服务"
    );
    original.stop();
    restarted.stop();
}
