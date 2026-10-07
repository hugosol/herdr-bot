//! 跨前端恢复与观察者资源边界（04 号票的 S2 覆盖：P29、P46、P49、P51）。
//!
//! 被验收的是**后端应用 interface**：共享观察流（`GET /api/sessions/events`）的（重）连首帧、关机状态
//! 事件的完整字段与顺序，以及一条被阻塞或已断开的观察连接**不拥有**安排生命周期——命令照常被接受、
//! 取消与到期裁定不被慢读者拖住、到期仍恰好调用一次实际关机。
//!
//! **绝不真的关机**：全部用例注入记录型关机动作（既有接缝的替身），它只数调用次数，不接触任何系统调用。
//! 命令与读取都经真实 TCP：用例看到的就是页面会收到的 HTTP 响应与 SSE 帧，不读任何进程内状态。
//!
//! 时间期望只用**独立字面量**（配置里的秒数），不从实现公式推导；也不为此修改任何生产代码。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use omp_bot::PowerOff;
use omp_bot::sessions::SessionsState;
use serde_json::{Value, json};

/// 记录型关机动作：只记录调用次数，**绝不**接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
}

impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), omp_bot::PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// 一个用例独占的服务：真实监听端口、与交付运行时同一份装配、记录型关机动作。
struct Panel {
    server: TestServer,
    double: Arc<RecordingPowerOff>,
}

impl Panel {
    /// 用指定的倒计时秒数装配应用（其余取测试缺省配置）。
    async fn start(countdown_seconds: u32) -> Self {
        let config = common::config_with_countdown(
            Path::new("unused.bat"),
            Path::new("."),
            countdown_seconds,
        );
        let sessions = Arc::new(SessionsState::new());
        let double = Arc::new(RecordingPowerOff {
            calls: AtomicUsize::new(0),
        });
        let (app, _state) = omp_bot::app_with_state(&config, double.clone(), sessions);
        Self {
            server: TestServer::start_app(app).await,
            double,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// 到期实际关机到目前为止被调用了几次。
    fn calls(&self) -> usize {
        self.double.calls.load(Ordering::SeqCst)
    }

    async fn post(&self, path: &str) -> HttpResponse {
        post_over_tcp(self.addr(), path).await
    }

    async fn post_json(&self, path: &str, body: Value) -> HttpResponse {
        post_json_over_tcp(self.addr(), path, &body.to_string()).await
    }

    /// 注册一个会话（可选带上一个很长的 cwd，用来把帧撑大）。
    async fn register(&self, session_id: &str, cwd: &str) -> HttpResponse {
        self.post_json(
            "/api/sessions/events",
            json!({ "sessionId": session_id, "kind": "register", "data": { "cwd": cwd } }),
        )
        .await
    }

    /// 上报一次 agent 状态。
    async fn state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post_json(
            "/api/sessions/events",
            json!({ "sessionId": session_id, "kind": "state", "data": { "value": value } }),
        )
        .await
    }

    /// 提交一次监听命令：明确的目标选中值。
    async fn monitor(&self, session_id: &str, selected: bool) -> HttpResponse {
        self.post_json(
            "/api/shutdown/monitor",
            json!({ "sessionId": session_id, "selected": selected }),
        )
        .await
    }

    async fn start_shutdown(&self) -> HttpResponse {
        self.post("/api/shutdown/start").await
    }

    async fn cancel_shutdown(&self) -> HttpResponse {
        self.post("/api/shutdown/cancel").await
    }

    /// 建立一条观察连接，返回它与已经读到的首帧 `snapshot`。
    async fn observe(&self) -> (SseConnection, SseFrame) {
        let mut connection = SseConnection::open(self.addr(), "/api/sessions/events", &[]).await;
        assert_eq!(connection.status, 200, "共享观察流必须可用");
        let first = connection.next_frame().await;
        first.named("snapshot");
        (connection, first)
    }

    /// 从一条**新**连接的（重）连首帧读取权威关机状态（页面的读取路径）。
    async fn authoritative(&self) -> Value {
        let (_, first) = self.observe().await;
        shutdown_payload(&first)
    }
}

/// 一帧里的关机状态：首帧 `snapshot` 与增量 `shutdown` 用的是同一个字段名。
fn shutdown_payload(frame: &SseFrame) -> Value {
    frame
        .json()
        .get("shutdown")
        .cloned()
        .unwrap_or_else(|| panic!("帧必须带 shutdown 字段：{frame:?}"))
}

/// 响应体里的权威关机状态（开始／取消的回执形状）。
fn shutdown_of(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "命令必须被接受：{}", response.body);
    serde_json::from_str::<Value>(&response.body).expect("回执必须是 JSON")["shutdown"].clone()
}

/// 权威监听集合（顺序稳定：升序）。
fn monitored_ids(view: &Value) -> Vec<String> {
    view["monitoredSessionIds"]
        .as_array()
        .unwrap_or_else(|| panic!("监听集合必须可读：{view}"))
        .iter()
        .map(|id| id.as_str().expect("集合成员是会话身份").to_string())
        .collect()
}

/// 会话视图里的某个会话。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("首帧里应有会话 {session_id}：{sessions:?}"))
}

/// 验收 P29、P46：断线期间另一个调用者改变了监听与安排，重连的**首帧**同时给出会话与关机的当下
/// 完整状态；之后关机变化仍是携带完整状态的独立事件，字段与顺序稳定。
#[tokio::test]
async fn a_reconnecting_reader_recovers_both_domains_in_the_first_frame_and_then_complete_events() {
    let panel = Panel::start(30).await;
    assert_eq!(panel.register("s-1", "C:\\work").await.status, 200);
    assert_eq!(panel.state("s-1", "working").await.status, 200);

    // 第一个读者：首帧同时带会话快照与关机状态。
    let (mut first_reader, first_frame) = panel.observe().await;
    let first_data = first_frame.json();
    assert_eq!(
        first_data["sessions"].as_array().map(Vec::len),
        Some(1),
        "首帧必须带会话快照：{first_data}"
    );
    let initial = shutdown_payload(&first_frame);
    assert_eq!(initial["phase"], json!("none"));
    assert!(monitored_ids(&initial).is_empty());

    // 监听变化由独立命名事件 `shutdown` 承载，且携带**完整**关机状态。
    assert_eq!(panel.monitor("s-1", true).await.status, 200);
    let monitored = first_reader.next_event("shutdown").await;
    let view = shutdown_payload(&monitored);
    assert_eq!(monitored_ids(&view), vec!["s-1".to_string()]);
    // 没有安排时状态就是「没有安排」：可空字段（安排标识、截止时间）按契约缺省即省略，
    // 而不是留着上一个安排的值——其余字段必须齐全，顺序稳定。
    for field in ["monitoredSessionIds", "phase", "serverTime"] {
        assert!(
            monitored.raw.contains(field),
            "关机变化必须是完整状态，缺字段 {field}：{}",
            monitored.raw
        );
    }
    assert!(
        !monitored.raw.contains("arrangementId") && !monitored.raw.contains("deadline"),
        "没有安排时不得带安排标识或截止时间：{}",
        monitored.raw
    );
    let position = |field: &str| monitored.raw.find(field).expect("字段在帧里");
    assert!(
        position("monitoredSessionIds") < position("phase")
            && position("phase") < position("serverTime"),
        "字段顺序必须稳定：{}",
        monitored.raw
    );
    assert!(
        monitored.json().get("sessions").is_none(),
        "关机事件只带关机那一半：{}",
        monitored.raw
    );

    // 这个读者断线，于是漏掉接下来发生的变化：会话进入等待输入 → 形成安排；随后被全局取消。
    drop(first_reader);
    assert_eq!(panel.state("s-1", "awaiting-input").await.status, 200);
    let arranged = panel.authoritative().await;
    assert_eq!(arranged["phase"], json!("counting-down"), "{arranged}");
    assert!(arranged["deadline"].is_u64(), "截止时间可观察：{arranged}");
    assert_eq!(panel.cancel_shutdown().await.status, 200);

    // 重连的首帧是**当下**的完整快照：漏掉的变化由它覆盖，过期视图不会留在读者手里。
    let (mut resumed, reconnected) = panel.observe().await;
    let recovered = shutdown_payload(&reconnected);
    assert_eq!(recovered["phase"], json!("cancelled"), "{recovered}");
    assert_eq!(
        recovered["arrangementId"], arranged["arrangementId"],
        "首帧必须给出同一安排的当下终局：{recovered}"
    );
    assert!(monitored_ids(&recovered).is_empty());
    let sessions = reconnected.json()["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        session_of(&sessions, "s-1")["state"],
        json!("awaiting-input"),
        "会话那一半也收敛到当下：{sessions:?}"
    );

    // 重连之后仍然只收增量：再次开始 → 一帧**带安排的完整**关机状态（五个字段齐全、顺序稳定）。
    assert_eq!(panel.start_shutdown().await.status, 200);
    let restarted = resumed.next_event("shutdown").await;
    let restarted_view = shutdown_payload(&restarted);
    assert_eq!(restarted_view["phase"], json!("counting-down"), "{restarted_view}");
    assert_ne!(restarted_view["arrangementId"], recovered["arrangementId"]);
    assert!(restarted_view["deadline"].is_u64());
    for field in ["monitoredSessionIds", "arrangementId", "phase", "deadline", "serverTime"] {
        assert!(
            restarted.raw.contains(field),
            "带安排的状态必须携带完整字段 {field}：{}",
            restarted.raw
        );
    }
    let restart_position = |field: &str| restarted.raw.find(field).expect("字段在帧里");
    assert!(
        restart_position("monitoredSessionIds") < restart_position("arrangementId")
            && restart_position("arrangementId") < restart_position("phase")
            && restart_position("phase") < restart_position("deadline")
            && restart_position("deadline") < restart_position("serverTime"),
        "字段顺序必须稳定：{}",
        restarted.raw
    );
}

/// 验收 P49（本票覆盖项）：两个**独立**读者看到的是同一个安排（标识与截止时间一致、与服务端时钟对齐），
/// 到期由后端按同一安排恰好执行一次——前端设备时钟不参与，本用例也不从实现公式推期望。
#[tokio::test]
async fn two_readers_see_the_same_arrangement_which_expires_exactly_once() {
    let panel = Panel::start(3).await;
    let started = Instant::now();

    // 第一个读者：命令回执里的权威状态。
    let accepted = panel.start_shutdown().await;
    let from_command = shutdown_of(&accepted);
    assert_eq!(from_command["phase"], json!("counting-down"), "{from_command}");

    // 独立字面量：配置了 3 秒，截止时间就比服务端当前时间晚 3000 毫秒。
    let deadline = from_command["deadline"].as_u64().expect("截止时间是数字");
    let server_time = from_command["serverTime"].as_u64().expect("服务端时间是数字");
    assert_eq!(
        deadline - server_time,
        3_000,
        "截止时间必须与配置的秒数对齐：{from_command}"
    );

    // 第二个读者（另一条连接）看到同一个安排：标识与截止时间一模一样。
    let other = panel.authoritative().await;
    assert_eq!(other["arrangementId"], from_command["arrangementId"]);
    assert_eq!(other["deadline"], from_command["deadline"]);
    assert_eq!(other["phase"], json!("counting-down"));

    // 截止时间之前没有任何实际关机调用。
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(panel.calls(), 0, "到期之前不得调用实际关机");

    // 到期由后端自己裁定并执行一次：两个读者都不拥有到期权。
    assert!(
        common::wait_until(Duration::from_secs(10), || panel.calls() >= 1).await,
        "过期后必须由后端调用一次实际关机（已等待 {:?}）",
        started.elapsed()
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(2_800),
        "实际关机不得早于配置的截止时间（实际 {elapsed:?}）"
    );

    // 只调用一次：越过截止时间再等一会儿也不会出现第二次。
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(panel.calls(), 1, "同一个安排只发出一次实际关机调用");

    // 终局在共享观察流里可读，且仍是同一个安排标识。
    let finished = panel.authoritative().await;
    assert_eq!(finished["phase"], json!("issued"), "{finished}");
    assert_eq!(finished["arrangementId"], from_command["arrangementId"]);
    assert!(monitored_ids(&finished).is_empty());
}

/// 验收 P51：一条被阻塞（连接着但完全不读）的观察连接不阻塞命令提交，也不阻塞取消与到期裁定；
/// 到期仍然恰好调用一次实际关机；恢复读取之后，读者能收敛到当下的终局状态。
#[tokio::test]
async fn a_blocked_reader_does_not_block_commands_or_the_arrangement() {
    let panel = Panel::start(3).await;

    // 帧很大：读者不读时服务端写这条连接会很快堵在 socket 缓冲上，订阅者的有界缓冲随之溢出。
    let long_cwd = format!("C:\\work\\{}", "x".repeat(4_000));
    assert_eq!(panel.register("s-big", &long_cwd).await.status, 200);
    assert_eq!(panel.register("s-cmd", "C:\\work").await.status, 200);
    assert_eq!(panel.state("s-cmd", "working").await.status, 200);

    // 阻塞的读者：建立连接、读掉首帧，之后**完全不读**。
    let (mut blocked, first) = panel.observe().await;
    assert_eq!(
        first.json()["sessions"].as_array().map(Vec::len),
        Some(2),
        "首帧就是当下的会话快照"
    );

    // 灌入远多于订阅者缓冲（64 帧）的可见变化，把这条连接彻底堵住。
    let flood = 300;
    let mut last = "working";
    for index in 0..flood {
        last = if index % 2 == 0 { "working" } else { "awaiting-input" };
        assert_eq!(panel.state("s-big", last).await.status, 200);
    }

    // 被阻塞的读者在场时，命令照常被接受，而且都不慢（没有等它读完）。
    let budget = Duration::from_secs(5);
    let monitor = tokio::time::timeout(budget, panel.monitor("s-cmd", true))
        .await
        .expect("监听命令不得被慢读者拖住");
    assert_eq!(monitor.status, 200, "{}", monitor.body);

    let start = tokio::time::timeout(budget, panel.start_shutdown())
        .await
        .expect("开始意图不得被慢读者拖住");
    let arranged = shutdown_of(&start);
    assert_eq!(arranged["phase"], json!("counting-down"));

    // 取消裁定也不被慢读者阻塞：这次安排不会发出任何关机调用。
    let cancel = tokio::time::timeout(budget, panel.cancel_shutdown())
        .await
        .expect("取消裁定不得被慢读者拖住");
    assert_eq!(cancel.status, 200, "{}", cancel.body);
    assert_eq!(panel.calls(), 0, "取消先于执行生效：这次安排零调用");

    // 再来一次安排并让它到期：阻塞的读者仍然不是安排生命周期的持有者——后端照常执行一次。
    let restart = tokio::time::timeout(budget, panel.start_shutdown())
        .await
        .expect("开始意图不得被慢读者拖住");
    assert_eq!(shutdown_of(&restart)["phase"], json!("counting-down"));
    assert!(
        common::wait_until(Duration::from_secs(10), || panel.calls() >= 1).await,
        "到期必须由后端执行，即使有读者完全不读"
    );
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(panel.calls(), 1, "同一个安排只调用一次实际关机");

    // 恢复全量读取之后读者能够收敛：不断读到一份反映当下终局的关机状态为止。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let converged = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "恢复读取后必须收敛到当下状态");
        let Some(frame) = blocked.next_frame_within(remaining).await else {
            panic!("恢复读取后必须继续收到帧");
        };
        match frame.event.as_deref() {
            Some("snapshot") | Some("shutdown") => {
                let view = shutdown_payload(&frame);
                if view["phase"] == json!("issued") {
                    break view;
                }
            }
            _ => {}
        }
    };
    assert!(converged["arrangementId"].is_string(), "{converged}");

    // 新读者（＝重连）的首帧给出同一份终局与会话的当下状态：视图完整收敛。
    let (_, fresh) = panel.observe().await;
    assert_eq!(shutdown_payload(&fresh)["phase"], json!("issued"));
    let sessions = fresh.json()["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        session_of(&sessions, "s-big")["state"],
        json!(last),
        "会话那一半也收敛到最后一帧：{sessions:?}"
    );
    assert!(
        common::wait_until(Duration::from_secs(2), || panel.calls() == 1).await,
        "收敛之后不再有新的实际关机调用"
    );
}

/// 验收 P51（观察者不持有安排生命周期）：读者在倒计时中途断开，安排照常在到期执行一次。
#[tokio::test]
async fn a_reader_that_disconnects_mid_countdown_does_not_hold_the_arrangement() {
    let panel = Panel::start(3).await;
    let (mut reader, _) = panel.observe().await;

    let start = panel.start_shutdown().await;
    let arranged = shutdown_of(&start);
    assert_eq!(arranged["phase"], json!("counting-down"));
    // 读者确实看到了这次安排：倒计时阶段的关机事件。
    let observed = loop {
        let frame = reader.next_frame().await;
        if frame.event.as_deref() != Some("shutdown") {
            continue;
        }
        let view = shutdown_payload(&frame);
        if view["phase"] == json!("counting-down") {
            break view;
        }
    };
    assert_eq!(observed["arrangementId"], arranged["arrangementId"]);

    // 读取者断开：没有浏览器、没有观察连接，后端仍然在到期执行一次。
    drop(reader);
    assert!(
        common::wait_until(Duration::from_secs(10), || panel.calls() >= 1).await,
        "观察者断开不得阻止到期执行"
    );
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(panel.calls(), 1);
    assert_eq!(panel.authoritative().await["phase"], json!("issued"));
}
