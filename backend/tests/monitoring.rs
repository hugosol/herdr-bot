//! 完成后关机监听：集合裁定、自动触发与全局清空（契约 P7–P18、P20–P23、P25–P27、P30、P31、P35、
//! P39、P40、P44、P46–P48、P50）。
//!
//! 被验收的是 S2（后端应用 interface）：监听命令（`POST /api/shutdown/monitor`）的接受与拒绝类别、
//! 自动倒计时的实际到期输出、离线／注销成员的移出，以及共享观察流里的权威监听集合与安排状态。
//!
//! **绝不真的关机**：全部用例都注入记录型关机动作（既有接缝的替身），它只数调用次数，不接触任何
//! 系统调用。命令与观察都走真 TCP：用例不读任何进程内状态，看到的就是页面会收到的 HTTP 响应与 SSE 帧。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::connect_info::MockConnectInfo;
use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use herdr_bot::clock::ManualClock;
use herdr_bot::sessions::{SessionPolicy, SessionsState};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

/// 监听命令的请求路径。
const MONITOR: &str = "/api/shutdown/monitor";
/// 手动开始意图的请求路径。
const START: &str = "/api/shutdown/start";
/// 全局取消的请求路径。
const CANCEL: &str = "/api/shutdown/cancel";

/// 记录型关机动作：只记录调用次数，**绝不**接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
}

impl RecordingPowerOff {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    /// 到目前为止被调用了几次。
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// 一个用例独占的服务：真实监听端口、与交付运行时同一份装配、记录型关机动作。
struct MonitoringServer {
    server: TestServer,
    double: Arc<RecordingPowerOff>,
    shutdown: Arc<herdr_bot::countdown::ShutdownState>,
    sessions: Arc<SessionsState>,
    /// 受控时刻；`None` 表示这个装配走真实挂钟（用于「必须真的等窗口过去」的用例）。
    clock: Option<Arc<ManualClock>>,
}

impl MonitoringServer {
    /// 用指定的倒计时秒数装配应用（真实挂钟，其余取测试缺省配置）。
    async fn start(countdown_seconds: u32) -> Self {
        Self::start_with(countdown_seconds, None).await
    }

    /// 受控时刻的装配：窗口边界由用例推进，不需要真的等。
    async fn start_on_controlled_clock(countdown_seconds: u32) -> Self {
        Self::start_with(countdown_seconds, Some(Arc::new(ManualClock::new()))).await
    }

    /// 用指定的倒计时秒数与（可选的）受控时刻装配应用。
    async fn start_with(countdown_seconds: u32, clock: Option<Arc<ManualClock>>) -> Self {
        let config = common::config_with_countdown(
            Path::new("unused.bat"),
            Path::new("."),
            countdown_seconds,
        );
        let sessions = match &clock {
            Some(clock) => Arc::new(SessionsState::with_policy_and_clock(
                SessionPolicy::default(),
                clock.clone(),
            )),
            None => Arc::new(SessionsState::new()),
        };
        let double = RecordingPowerOff::new();
        let (app, state) = herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            double,
            shutdown: state.shutdown,
            sessions,
            clock,
        }
    }

    /// 启动交付运行时那份时钟驱动的维护循环（一遍维护 + 监听重判）。
    fn spawn_maintenance(&self) {
        herdr_bot::sessions::spawn_expiry_scan(
            Arc::clone(&self.sessions),
            Arc::clone(&self.shutdown),
        );
    }

    /// 跑一遍维护（与 `spawn_expiry_scan` 每 5 秒那一遍是同一对调用）。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    /// 推进受控时刻；只有 [`MonitoringServer::start_on_controlled_clock`] 的装配可以调用。
    fn advance(&self, by: Duration) {
        self.clock
            .as_ref()
            .expect("本用例不是受控时刻的装配")
            .advance(by);
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post_event(&self, body: String) -> HttpResponse {
        post_json_over_tcp(self.addr(), "/api/sessions/events", &body).await
    }

    /// 注册（或重注册）一个会话。
    async fn register(&self, session_id: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "register", "data": {"cwd": "C:\\work"}})
                .to_string(),
        )
        .await
    }

    /// 上报一次 agent 状态。
    async fn state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "state", "data": {"value": value}}).to_string(),
        )
        .await
    }

    /// 一次心跳（不带状态）。
    async fn heartbeat(&self, session_id: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "heartbeat", "data": {}}).to_string(),
        )
        .await
    }

    /// 一次**携带状态**的心跳：与显式状态上报同一处理，也能更新当前判定。
    async fn heartbeat_with_state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "heartbeat", "data": {"value": value}})
                .to_string(),
        )
        .await
    }

    /// 注销一个会话。
    async fn unregister(&self, session_id: &str) -> HttpResponse {
        self.post_event(json!({"sessionId": session_id, "kind": "unregister"}).to_string())
            .await
    }

    /// 提交一次监听命令：明确的目标选中值。
    async fn monitor(&self, session_id: &str, selected: bool) -> HttpResponse {
        post_json_over_tcp(
            self.addr(),
            MONITOR,
            &json!({"sessionId": session_id, "selected": selected}).to_string(),
        )
        .await
    }

    /// 提交一次手动开始意图。
    async fn start_shutdown(&self) -> HttpResponse {
        post_over_tcp(self.addr(), START).await
    }

    /// 提交一次全局清空（取消）。
    async fn clear_all(&self) -> HttpResponse {
        post_over_tcp(self.addr(), CANCEL).await
    }

    /// 建一条观察连接并读掉首帧。
    async fn observe(&self) -> Observer {
        Observer::connect(self.addr()).await
    }

    /// 从观察流的首帧读取权威关机状态（与页面同一条读取路径）。
    async fn authoritative(&self) -> Value {
        self.observe().await.first_shutdown()
    }

    /// 从观察流的首帧读取权威监听集合。
    async fn authoritative_set(&self) -> Vec<String> {
        monitored_ids(&self.authoritative().await)
    }
}

/// 一条已经读过首帧 `snapshot` 的观察连接。
struct Observer {
    connection: SseConnection,
    first: SseFrame,
}

impl Observer {
    async fn connect(addr: SocketAddr) -> Self {
        let mut connection = SseConnection::open(addr, "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        Self { connection, first }
    }

    fn first_shutdown(&self) -> Value {
        shutdown_payload(&self.first)
    }

    /// 首帧里的会话列表。
    fn first_sessions(&self) -> Vec<Value> {
        self.first.json()["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// 一直读到阶段的取值等于 `phase`，返回那一帧的完整关机状态。
    async fn shutdown_until(&mut self, phase: &str) -> Value {
        self.shutdown_until_within(phase, Duration::from_secs(20)).await
    }

    /// 同 [`Observer::shutdown_until`]，但等待上限由调用方给出（慢用例要把 keepAlive 真的等过去）。
    async fn shutdown_until_within(&mut self, phase: &str, timeout: Duration) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut seen: Vec<String> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待阶段 {phase:?} 超时；已观察到的帧：{seen:#?}"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待阶段 {phase:?} 超时；已观察到的帧：{seen:#?}");
            };
            if frame.event.as_deref() != Some("shutdown") {
                seen.push(format!("{frame:?}"));
                continue;
            }
            let view = shutdown_payload(&frame);
            let observed = view["phase"].as_str().unwrap_or_default().to_string();
            if observed == phase {
                return view;
            }
            seen.push(format!("〔{observed}〕{frame:?}"));
        }
    }

    /// 一直读到某个会话被翻为离线的那一帧，返回该会话的视图。
    async fn sessions_until_offline(&mut self, session_id: &str, timeout: Duration) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待会话 {session_id} 离线超时"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待会话 {session_id} 离线超时");
            };
            if frame.event.as_deref() != Some("upsert") {
                continue;
            }
            let session = frame.json()["session"].clone();
            if session["sessionId"] == json!(session_id) && session["online"] == json!(false) {
                return session;
            }
        }
    }

    /// 一直读到监听集合等于 `expected`，返回那一帧的完整关机状态。
    async fn set_until(&mut self, expected: &[&str]) -> Value {
        self.set_until_within(expected, Duration::from_secs(20)).await
    }

    /// 同 [`Observer::set_until`]，但等待上限由调用方给出。
    async fn set_until_within(&mut self, expected: &[&str], timeout: Duration) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut seen: Vec<String> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待监听集合 {expected:?} 超时；已观察到的帧：{seen:#?}"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待监听集合 {expected:?} 超时；已观察到的帧：{seen:#?}");
            };
            if frame.event.as_deref() != Some("shutdown") {
                seen.push(format!("{frame:?}"));
                continue;
            }
            let view = shutdown_payload(&frame);
            let observed = monitored_ids(&view);
            if observed == expected {
                return view;
            }
            seen.push(format!("〔{observed:?}〕{frame:?}"));
        }
    }
}

/// 一帧里的关机状态：首帧 `snapshot` 与增量 `shutdown` 用的是同一个字段名。
fn shutdown_payload(frame: &SseFrame) -> Value {
    let data = frame.json();
    data.get("shutdown")
        .cloned()
        .unwrap_or_else(|| panic!("帧必须带 shutdown 字段：{data}"))
}

/// 关机状态里的权威监听集合（顺序稳定：升序）。
fn monitored_ids(view: &Value) -> Vec<String> {
    view["monitoredSessionIds"]
        .as_array()
        .unwrap_or_else(|| panic!("监听集合必须可读：{view}"))
        .iter()
        .map(|id| id.as_str().expect("集合成员是会话身份").to_string())
        .collect()
}

/// 响应体的 JSON。
fn response_body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{}", response.body))
}

/// 一次成功的监听命令：类别是 `selected`，回执里的权威集合包含该会话。
fn assert_selected(response: &HttpResponse, session_id: &str) -> Value {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("selected"),
        "目标选中值必须如实回报：{}",
        response.body
    );
    let view = body["shutdown"].clone();
    assert!(
        monitored_ids(&view).contains(&session_id.to_string()),
        "回执必须给出命令之后的权威集合：{}",
        response.body
    );
    view
}

/// 一次成功的撤销：类别是 `unselected`，回执里的权威集合不含该会话。
fn assert_unselected(response: &HttpResponse, session_id: &str) -> Value {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("unselected"),
        "目标选中值必须如实回报：{}",
        response.body
    );
    let view = body["shutdown"].clone();
    assert!(
        !monitored_ids(&view).contains(&session_id.to_string()),
        "回执必须给出命令之后的权威集合：{}",
        response.body
    );
    view
}

/// 首帧会话列表里某个会话的视图。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("首帧里应有会话 {session_id}：{sessions:?}"))
}

/// 验收 P13、P12、P46：只有全部被监听成员**当前**都等待用户输入才形成安排；设置监听不改会话状态。
#[tokio::test]
async fn the_arrangement_waits_for_every_monitored_session_to_await_input() {
    let server = MonitoringServer::start(10).await;
    let mut observer = server.observe().await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    // 一个不属于监听集合、且一直在工作的会话：它不该阻塞安排。
    assert_accepted(&server.register("bystander").await);
    assert_accepted(&server.state("bystander", "working").await);

    assert_selected(&server.monitor("a", true).await, "a");
    let view = assert_selected(&server.monitor("b", true).await, "b");
    assert_eq!(
        monitored_ids(&view),
        vec!["a".to_string(), "b".to_string()],
        "权威集合按稳定顺序给出两个成员：{view}"
    );

    // 设置监听不改会话状态：a 的等待输入与在线信息照旧可查看（P12）。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let sessions = server.observe().await.first_sessions();
    let a = session_of(&sessions, "a");
    assert_eq!(a["state"], json!("awaiting-input"), "监听不替换 agent 状态：{a}");
    assert_eq!(a["online"], json!(true), "监听不改变 keepAlive：{a}");
    assert_eq!(a["cwd"], json!("C:\\work"), "会话身份信息仍可查看：{a}");

    // a 已经等待而 b 仍在工作：不启动；权威状态如实给出「没有安排」与当前集合。
    let pending = server.authoritative().await;
    assert_eq!(pending["phase"], json!("none"), "成员没全部等待就不启动：{pending}");
    assert_eq!(monitored_ids(&pending), vec!["a".to_string(), "b".to_string()]);
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");

    // b 也进入等待用户输入：全部成员当前都等待，形成唯一安排（未监听的 bystander 不阻塞）。
    // 这一次用**携带状态的心跳**上报，与显式状态上报同一处理。
    assert_accepted(&server.heartbeat_with_state("b", "awaiting-input").await);
    let arranged = observer.shutdown_until("counting-down").await;
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "开始倒计时即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
}

/// 验收 P16：A 等待、B 工作时以**注销**触发移出，剩余非空集合全等待即立即形成唯一安排。
#[tokio::test]
async fn unregistering_the_working_member_arranges_shutdown() {
    let server = MonitoringServer::start(10).await;
    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_eq!(server.authoritative().await["phase"], json!("none"));

    // 注销仍在工作的 b：它被移出，仅剩的 a 已经等待用户输入，于是立即形成安排。
    assert_accepted(&server.unregister("b").await);
    let arranged = server.authoritative().await;
    assert_eq!(
        arranged["phase"],
        json!("counting-down"),
        "剩余非空集合全等待即形成安排：{arranged}"
    );
    assert!(
        monitored_ids(&arranged).is_empty(),
        "形成安排即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
}

/// 验收 P14：成员恢复工作后必须**再次**进入等待用户输入才算满足条件，不保存「曾经结束过」的资格。
#[tokio::test]
async fn a_member_that_resumes_work_must_await_input_again() {
    let server = MonitoringServer::start(10).await;
    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");

    // a 曾经等待过，但随即恢复工作：这次「曾经结束」不构成资格。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.state("b", "awaiting-input").await);
    let still_pending = server.authoritative().await;
    assert_eq!(
        still_pending["phase"],
        json!("none"),
        "a 恢复工作后 b 单独等待不启动：{still_pending}"
    );

    // a 再次进入等待用户输入：这才满足条件。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let arranged = server.authoritative().await;
    assert_eq!(
        arranged["phase"],
        json!("counting-down"),
        "a 再次等待才形成安排：{arranged}"
    );
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P15、P16：注销与撤销立即移出成员并重判；重新注册不自动重新加入监听；剩余非空且全等待时立即形成安排。
#[tokio::test]
async fn removing_a_member_rejudges_the_set_immediately() {
    let server = MonitoringServer::start(10).await;
    let mut observer = server.observe().await;

    for session in ["a", "b", "c"] {
        assert_accepted(&server.register(session).await);
        assert_accepted(&server.state(session, "working").await);
        assert_selected(&server.monitor(session, true).await, session);
    }
    assert_accepted(&server.state("a", "awaiting-input").await);
    let pending = server.authoritative().await;
    assert_eq!(
        monitored_ids(&pending),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
    assert_eq!(pending["phase"], json!("none"), "b、c 仍在工作就不启动：{pending}");

    // 注销 c：c 立即移出，不留下一个已经不存在的成员。
    assert_accepted(&server.unregister("c").await);
    let without_c = observer.set_until(&["a", "b"]).await;
    assert_eq!(without_c["phase"], json!("none"), "剩下的 b 仍在工作：{without_c}");

    // 重新注册 c：它**不**自动重新加入监听（移出就是移出）。
    assert_accepted(&server.register("c").await);
    assert_accepted(&server.state("c", "working").await);
    assert_eq!(
        server.authoritative_set().await,
        vec!["a".to_string(), "b".to_string()],
        "重新注册的会话不回到监听集合"
    );

    // 撤销 b：剩余 {a} 全部等待用户输入，立即形成唯一安排。
    assert_unselected(&server.monitor("b", false).await, "b");
    let arranged = observer.shutdown_until("counting-down").await;
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "开始倒计时即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");

    // 同一成员的重复等待通知不重启安排、也不改变截止时间。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let again = server.authoritative().await;
    assert_eq!(again["arrangementId"], arranged["arrangementId"], "安排标识不变");
    assert_eq!(again["deadline"], arranged["deadline"], "截止时间不变");
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P17：集合为空（撤销最后一个成员、注销最后一个成员）都不形成安排；无成员时重复等待通知也没有安排。
#[tokio::test]
async fn an_empty_set_never_forms_an_arrangement() {
    let server = MonitoringServer::start(10).await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");

    // 撤销最后一个成员：只清空集合。
    assert_unselected(&server.monitor("a", false).await, "a");
    assert!(server.authoritative_set().await.is_empty());

    // 无成员时重复等待通知：仍然没有安排，记录型关机动作零调用。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("a", "awaiting-input").await);
    let empty = server.authoritative().await;
    assert_eq!(empty["phase"], json!("none"), "空集合不触发关机：{empty}");
    assert!(monitored_ids(&empty).is_empty());

    // 注销最后一个成员：同样只清空集合。
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("b", true).await, "b");
    assert_accepted(&server.unregister("b").await);
    assert!(server.authoritative_set().await.is_empty());
    let gone = server.authoritative().await;
    assert_eq!(gone["phase"], json!("none"), "移出最后一个成员不触发关机：{gone}");

    assert_eq!(server.double.calls(), 0, "零调用：没有任何安排形成");
}

/// 验收 P25：不建立任何浏览器或观察订阅，自动安排到期仍由后端调用一次实际关机。
#[tokio::test]
async fn the_backend_executes_the_automatic_arrangement_without_any_observer() {
    let server = MonitoringServer::start(3).await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_accepted(&server.state("a", "awaiting-input").await);

    // 到此为止没有建立任何观察连接：到期执行不依赖任何订阅者。
    assert_eq!(server.double.calls(), 0, "截止时间之前不得调用关机");
    assert!(
        common::wait_until(Duration::from_secs(15), || server.double.calls() == 1).await,
        "后端必须在没有订阅者时到期执行一次（当前 {} 次）",
        server.double.calls()
    );
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(server.double.calls(), 1, "到期执行恰好一次，不重复");
}

/// 验收 P23、P25、P31、P46：显式配置的 3 秒驱动**自动入口**的实际到期输出，且只调用一次。
///
/// 期望值来自启动期配置里的独立字面量（3 秒），与 02 手动入口的用例同一个来源：不由实现推算。
#[tokio::test]
async fn the_configured_three_seconds_drives_the_automatic_expiry_once() {
    let server = MonitoringServer::start(3).await;
    let mut observer = server.observe().await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");

    // 成员进入等待用户输入：自动形成安排（没有浏览器、没有手动确认）。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let arranged = observer.shutdown_until("counting-down").await;
    assert_eq!(
        arranged["deadline"].as_u64().expect("倒计时中必须有截止时间")
            - arranged["serverTime"].as_u64().expect("服务端时间必须可读"),
        3000,
        "自动入口必须用启动期配置的时长（3 秒）：{arranged}"
    );

    // 截止时间之前零调用。
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(server.double.calls(), 0, "截止时间之前不得调用关机：{arranged}");

    // 交错的手动开始与重复的等待通知都沿用同一安排：标识与截止时间都不变（P21、P31）。
    let repeated = assert_selected_after_start(&server.start_shutdown().await);
    assert_eq!(repeated["arrangementId"], arranged["arrangementId"], "沿用同一安排");
    assert_eq!(repeated["deadline"], arranged["deadline"], "截止时间不得改变");
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.heartbeat("a").await);
    let unchanged = server.authoritative().await;
    assert_eq!(unchanged["arrangementId"], arranged["arrangementId"]);
    assert_eq!(unchanged["deadline"], arranged["deadline"]);

    // 到期由后端自己执行一次；观察流给出实现已开始与指令已发出的完整状态。
    let issued = observer.shutdown_until("issued").await;
    assert_eq!(server.double.calls(), 1, "到期恰好调用一次实际关机：{issued}");
    assert_eq!(issued["arrangementId"], arranged["arrangementId"]);

    // 再等一段时间仍然只有这一次：不重试、不重复执行。
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(server.double.calls(), 1, "到期后的重复观察不增加调用：{issued}");
}

/// 一次手动开始意图的结果类别与权威状态。
fn assert_selected_after_start(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "开始意图应被接受：{}", response.body);
    let body = response_body(response);
    assert!(
        matches!(
            body["outcome"].as_str(),
            Some("counting-down" | "executing")
        ),
        "结果类别必须是安排阶段：{}",
        response.body
    );
    body["shutdown"].clone()
}

/// 验收 P20、P18、P22：手动确认立即形成安排并消耗整组监听；倒计时期间不接受新增监听；取消后不恢复。
#[tokio::test]
async fn a_manual_start_consumes_the_whole_set_and_refuses_new_monitoring() {
    let server = MonitoringServer::start(10).await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");
    // a 已经等待，而 b 仍在工作：自动条件不成立。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_eq!(server.authoritative().await["phase"], json!("none"));

    // 手动确认：立即形成安排，不等待仍在工作的监听成员，并消耗整组监听。
    let started = assert_selected_after_start(&server.start_shutdown().await);
    assert_eq!(started["phase"], json!("counting-down"), "{started}");
    assert!(
        monitored_ids(&started).is_empty(),
        "手动开始同样消耗整组监听：{started}"
    );

    // 倒计时期间新增监听被拒绝，权威集合保持空，原安排不受影响。
    assert_accepted(&server.register("c").await);
    assert_accepted(&server.state("c", "working").await);
    assert_refused(&server.monitor("c", true).await, 409, "shutdown-in-progress");
    assert_refused(&server.monitor("a", true).await, 409, "shutdown-in-progress");
    let after_refusals = server.authoritative().await;
    assert!(
        monitored_ids(&after_refusals).is_empty(),
        "被拒绝的新增不得改变权威集合：{after_refusals}"
    );
    assert_eq!(after_refusals["arrangementId"], started["arrangementId"]);
    assert_eq!(after_refusals["deadline"], started["deadline"]);

    // 重申未选中值不重置安排（重复命令不反向翻转、也不重启）。
    assert_unselected(&server.monitor("a", false).await, "a");
    let after_repeat = server.authoritative().await;
    assert_eq!(after_repeat["arrangementId"], started["arrangementId"]);
    assert_eq!(after_repeat["deadline"], started["deadline"]);

    // 取消后不恢复监听：重复的等待通知不再重启安排。
    let cancelled = server.clear_all().await;
    assert_eq!(cancelled.status, 200, "全局取消必须生效：{}", cancelled.body);
    assert_accepted(&server.state("c", "awaiting-input").await);
    let after_cancel = server.authoritative().await;
    assert_eq!(
        after_cancel["phase"],
        json!("cancelled"),
        "取消是这次安排的终点：{after_cancel}"
    );
    assert!(
        monitored_ids(&after_cancel).is_empty(),
        "取消后监听集合保持空，需要用户重新选择：{after_cancel}"
    );
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P30：倒计时已经启动后，会话重新工作、注销与前端关闭都不撤销同一活动安排。
#[tokio::test]
async fn a_running_arrangement_survives_session_changes_and_frontend_closure() {
    let server = MonitoringServer::start(3).await;
    let mut observer = server.observe().await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("b", "awaiting-input").await);
    let arranged = observer.shutdown_until("counting-down").await;
    let arrangement_id = arranged["arrangementId"].clone();
    let deadline = arranged["deadline"].clone();

    // 成员重新工作、另一个成员注销、观察连接关闭：安排都不该被自动取消。
    assert_accepted(&server.state("b", "working").await);
    assert_accepted(&server.unregister("b").await);
    drop(observer);

    let after_changes = server.authoritative().await;
    assert_eq!(
        after_changes["phase"],
        json!("counting-down"),
        "会话变化不取消已启动的安排：{after_changes}"
    );
    assert_eq!(after_changes["arrangementId"], arrangement_id);
    assert_eq!(after_changes["deadline"], deadline);

    // 同一安排仍然到期执行一次。
    assert!(
        common::wait_until(Duration::from_secs(15), || server.double.calls() == 1).await,
        "活动安排必须在到期时执行一次（当前 {} 次）",
        server.double.calls()
    );
    let settled = server.authoritative().await;
    assert_eq!(settled["phase"], json!("issued"), "结果仍属于同一安排：{settled}");
    assert_eq!(settled["arrangementId"], arrangement_id);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.double.calls(), 1, "只执行一次");
}

/// 验收 P27、P40：无倒计时时的全局清空入口一次命令原子清空集合，且不中途触发安排。
#[tokio::test]
async fn the_global_clear_empties_the_whole_set_atomically() {
    let server = MonitoringServer::start(10).await;
    let mut observer = server.observe().await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_accepted(&server.register("b").await);
    assert_accepted(&server.state("b", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");
    // a 已经等待、b 仍在工作：自动条件不成立，所以这是一次「只有监听、没有倒计时」的清空。
    assert_accepted(&server.state("a", "awaiting-input").await);

    let cleared = server.clear_all().await;
    assert_eq!(cleared.status, 200, "全局清空必须被接受：{}", cleared.body);
    let body = response_body(&cleared);
    assert_eq!(body["outcome"], json!("cancelled"), "{}", cleared.body);
    assert!(
        monitored_ids(&body["shutdown"]).is_empty(),
        "清空必须一次清掉整组监听：{}",
        cleared.body
    );
    assert_eq!(
        body["shutdown"]["phase"],
        json!("none"),
        "没有倒计时时不虚构一个安排：{}",
        cleared.body
    );

    // 观察流也给出这次清空：整组一起消失，且没有形成任何安排。
    let observed = observer.set_until(&[]).await;
    assert_eq!(observed["phase"], json!("none"), "{observed}");
    assert_eq!(server.double.calls(), 0, "清空不等于关机，也不触发安排");
}

/// 验收 P25：已监听成员进入离线后仍在监听集合里，并阻塞自动触发——在线窗口结束不移出监听。
///
/// 跑的是交付运行时同一份后台维护循环（每 5 秒一遍、真实挂钟），因此「离线」这一可见变化来自维护，
/// 而集合仍含该身份；离线成员的最后上报状态不作当前完成证据。
#[tokio::test]
async fn an_offline_member_stays_in_the_set_and_blocks_automatic_shutdown() {
    let server = MonitoringServer::start(10).await;
    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");

    // 观察连接在集合已经是 {a} 之后建立，因此后面的每一帧都是它之后发生的可见变化。
    let mut observer = server.observe().await;
    assert_eq!(
        monitored_ids(&observer.first_shutdown()),
        vec!["a".to_string()]
    );
    server.spawn_maintenance();

    let started = std::time::Instant::now();
    let stale = observer
        .sessions_until_offline("a", Duration::from_secs(10 + 10))
        .await;
    assert!(
        started.elapsed() > Duration::from_secs(10),
        "不得在 10 秒在线有效期内提前翻为离线：实际 {:?}",
        started.elapsed()
    );
    assert_eq!(
        stale["state"],
        json!("working"),
        "过期只改 keepAlive：{stale}"
    );

    // 在线窗口结束不移出监听：身份仍在集合里，且它阻塞自动触发。
    let after = server.authoritative().await;
    assert_eq!(
        monitored_ids(&after),
        vec!["a".to_string()],
        "离线成员必须留在监听集合里：{after}"
    );
    assert_eq!(after["phase"], json!("none"), "离线成员阻塞自动触发：{after}");
    assert_eq!(server.double.calls(), 0, "没有任何关机调用");

    // 离线成员仍是同一个监听成员：重申选中值是确认，不是新增，也不需要重新选择。
    assert_selected(&server.monitor("a", true).await, "a");
    // 保留期内恢复在线：原监听意图继续有效（不需要再次 selected=true）。
    assert_accepted(&server.heartbeat("a").await);
    let restored = server.observe().await.first_sessions();
    assert_eq!(
        session_of(&restored, "a")["online"],
        json!(true),
        "心跳把会话恢复在线：{restored:?}"
    );
    assert_eq!(
        monitored_ids(&server.authoritative().await),
        vec!["a".to_string()],
        "恢复在线后原监听仍在"
    );
    assert_eq!(server.double.calls(), 0, "恢复在线本身不触发关机");
}

/// 验收 P26：任一离线监听成员都阻塞自动触发，即使它最后上报的是等待用户输入。
///
/// 参数化被阻塞成员的最后上报状态；算例里保持 A 在线且等待用户输入——唯一不满足条件的原因就是 B 离线。
#[tokio::test]
async fn an_offline_member_blocks_the_automatic_shutdown_even_when_it_awaits_input() {
    for blocked_state in ["awaiting-input", "working"] {
        let server = MonitoringServer::start_on_controlled_clock(10).await;
        assert_accepted(&server.register("a").await);
        assert_accepted(&server.state("a", "working").await);
        assert_accepted(&server.register("b").await);
        assert_accepted(&server.state("b", "working").await);
        assert_selected(&server.monitor("a", true).await, "a");
        assert_selected(&server.monitor("b", true).await, "b");
        // B 的最后上报状态就是它停机前的状态：此后不再上报。
        assert_accepted(&server.state("b", blocked_state).await);

        // 先让两个会话都过期，再把 A 心跳回在线并让它进入「等待用户输入」：
        // 从这一刻起，唯一不满足自动条件的只能是 B 离线。
        server.advance(Duration::from_secs(11));
        assert_accepted(&server.heartbeat("a").await);
        assert_accepted(&server.state("a", "awaiting-input").await);
        server.maintain().await;

        let observed = server.authoritative().await;
        assert_eq!(
            observed["phase"],
            json!("none"),
            "B（最后状态 {blocked_state}）离线必须阻塞自动触发：{observed}"
        );
        assert_eq!(server.double.calls(), 0, "没有关机调用");
        assert_eq!(
            monitored_ids(&observed),
            vec!["a".to_string(), "b".to_string()],
            "离线成员保留在集合里：{observed}"
        );

        let sessions = server.observe().await.first_sessions();
        assert_eq!(
            session_of(&sessions, "a")["online"],
            json!(true),
            "算例必须保持 A 在线：{sessions:?}"
        );
        assert_eq!(
            session_of(&sessions, "a")["state"],
            json!("awaiting-input"),
            "算例里 A 必须是等待用户输入：{sessions:?}"
        );
        let b = session_of(&sessions, "b");
        assert_eq!(b["online"], json!(false), "{b}");
        assert_eq!(
            b["state"],
            json!(blocked_state),
            "最后一次上报的状态原样保留：{b}"
        );

        // 反过来证明阻塞确实来自 B：把 B 的登记注销掉，剩下的 A 立刻满足条件。
        assert_accepted(&server.unregister("b").await);
        assert_eq!(
            server.authoritative().await["phase"],
            json!("counting-down"),
            "移出离线成员后，只剩 A 就应当立即开始倒计时"
        );
    }
}

/// 验收 P30：登记清除、注销、撤销分别移出成员后，剩余集合按**同一条件**重判。
///
/// 参数化三种移出原因与剩余成员 A 的三种情形：A 在线且等待用户输入时触发倒计时（并消耗整组监听）；
/// A 工作中或离线时不触发，且 A 仍留在集合里——不采用「移出一个成员就取消整组」的规则。
#[tokio::test]
async fn removing_a_member_rejudges_the_remaining_set_for_every_reason() {
    for reason in ["cleared", "unregistered", "unselected"] {
        for remaining in ["awaiting-input", "working", "offline"] {
            let server = MonitoringServer::start_on_controlled_clock(10).await;
            // B 先注册并被监听；20 秒后 A 注册并被监听（于是「清除 B」时 A 还在保留期内）。
            assert_accepted(&server.register("b").await);
            assert_accepted(&server.state("b", "working").await);
            assert_selected(&server.monitor("b", true).await, "b");
            server.advance(Duration::from_secs(20));
            assert_accepted(&server.register("a").await);
            assert_accepted(&server.state("a", "working").await);
            assert_selected(&server.monitor("a", true).await, "a");
            assert_accepted(
                &server
                    .state("a", if remaining == "working" { "working" } else { "awaiting-input" })
                    .await,
            );

            // 让「离线」这个情形真的离线（A 的在线窗口 10 秒）。
            if remaining == "offline" {
                server.advance(Duration::from_secs(11));
            }

            match reason {
                "cleared" => {
                    // 推进到 B 超过两个窗口之和（41 秒），A 仍在保留期内。
                    server.advance(Duration::from_secs(if remaining == "offline" { 10 } else { 21 }));
                    if remaining != "offline" {
                        // 保持 A 在线（也保持它最后一次上报的等待／工作状态）。
                        assert_accepted(&server.heartbeat("a").await);
                    }
                    server.maintain().await;
                    assert!(
                        server
                            .observe()
                            .await
                            .first_sessions()
                            .iter()
                            .all(|session| session["sessionId"] != json!("b")),
                        "B 的登记必须已被清除"
                    );
                }
                "unregistered" => {
                    assert_accepted(&server.unregister("b").await);
                }
                "unselected" => {
                    assert_unselected(&server.monitor("b", false).await, "b");
                }
                other => panic!("未知的移出原因：{other}"),
            }

            let observed = server.authoritative().await;
            match remaining {
                "awaiting-input" => {
                    assert_eq!(
                        observed["phase"],
                        json!("counting-down"),
                        "移出 B（原因 {reason}）后，剩下的 A 在线等待用户输入应立即开始倒计时：{observed}"
                    );
                    assert!(
                        monitored_ids(&observed).is_empty(),
                        "开始倒计时即消耗整组监听：{observed}"
                    );
                }
                _ => {
                    assert_eq!(
                        observed["phase"],
                        json!("none"),
                        "剩余的 A 为 {remaining} 时（原因 {reason}）不触发：{observed}"
                    );
                    assert_eq!(
                        monitored_ids(&observed),
                        vec!["a".to_string()],
                        "移出 B 不改动其余成员的监听意图（原因 {reason}）：{observed}"
                    );
                }
            }
            assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
        }
    }
}

/// 验收 P31：清除、注销、撤销分别移出**最后一个**监听成员时，空集合不触发安排，安全 adapter 无调用。
#[tokio::test]
async fn removing_the_last_member_never_starts_a_countdown() {
    for reason in ["cleared", "unregistered", "unselected"] {
        let server = MonitoringServer::start_on_controlled_clock(10).await;
        assert_accepted(&server.register("only").await);
        assert_accepted(&server.state("only", "working").await);
        assert_selected(&server.monitor("only", true).await, "only");

        match reason {
            "cleared" => {
                server.advance(Duration::from_secs(41));
                server.maintain().await;
            }
            "unregistered" => {
                assert_accepted(&server.unregister("only").await);
            }
            "unselected" => {
                assert_unselected(&server.monitor("only", false).await, "only");
            }
            other => panic!("未知的移出原因：{other}"),
        }

        let observed = server.authoritative().await;
        assert!(
            monitored_ids(&observed).is_empty(),
            "最后一个成员被移出（原因 {reason}）后集合为空：{observed}"
        );
        assert_eq!(
            observed["phase"],
            json!("none"),
            "空集合不触发安排（原因 {reason}）：{observed}"
        );
        // 给可能的异步裁定留一点时间：空集合之后也不该冒出安排。
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            server.double.calls(),
            0,
            "空集合在任何原因下都不得调用关机（原因 {reason}）"
        );
        assert_eq!(server.authoritative().await["phase"], json!("none"));
    }
}

/// 验收 P40（受控交错）：登记删除与新增／撤销监听**并发**发出时，集合里不留下指向不存在登记的身份，
/// 也不依赖再过一遍维护修复。
///
/// 每一轮都把「注销」与「监听命令」同时发出（两种次序都可能发生）：命令必须有明确归宿
/// （登记在命令处理期间消失时，监听命令如实回 `404 unknown-session` / `409 session-not-eligible`，
/// 而不是先收下再留成幽灵成员），且两条命令都完成之后集合里绝不能出现这个已经不存在的身份。
///
/// 说明：命令内部「读取资格快照」与「提交新增」之间只有微秒级窗口，外部 seam 无法为它精确排序；
/// 每一种**可确定**的次序由下一条用例逐条覆盖，而本用例断言的是并发之下必须成立的不变量。
#[tokio::test]
async fn interleaved_removals_and_monitoring_commands_leave_no_ghost_member() {
    let server = MonitoringServer::start(10).await;
    for selected in [true, false] {
        for round in 0..8 {
            let id = format!("race-{selected}-{round}");
            assert_accepted(&server.register(&id).await);
            assert_accepted(&server.state(&id, "working").await);
            if !selected {
                // 撤销路径的前提：这一身份先被监听。
                assert_selected(&server.monitor(&id, true).await, &id);
            }

            let (unregistered, monitored) =
                tokio::join!(server.unregister(&id), server.monitor(&id, selected));
            assert_accepted(&unregistered);
            assert!(
                matches!(monitored.status, 200 | 404 | 409),
                "监听命令必须有明确归宿：{} {}",
                monitored.status,
                monitored.body
            );

            // 不等下一周期：这一刻的权威集合就不该含这个已经不存在的身份。
            let observed = server.authoritative_set().await;
            assert!(
                !observed.contains(&id),
                "第 {round} 轮（selected={selected}）留下了幽灵成员：{observed:?}"
            );

            // 那一遍维护之后仍然干净。
            server.maintain().await;
            assert!(
                !server.authoritative_set().await.contains(&id),
                "维护之后仍留下幽灵成员（selected={selected}）"
            );
        }
    }
    assert!(
        server.authoritative_set().await.is_empty(),
        "所有登记都已删除，集合里不该剩下任何身份"
    );
}

/// 验收 P40（可确定的每一种次序）：登记删除、写入与监听命令交错之后，集合里都不留下该身份。
///
/// 逐条走完：注销后立刻读取、注销后再新增监听、注销后写入、重新注册后清除那一遍维护、
/// 清除后重新注册。每一步都在**外部 seam** 上观察，且不等待下一周期修复。
#[tokio::test]
async fn a_removed_registration_never_leaves_a_monitored_identity() {
    let server = MonitoringServer::start_on_controlled_clock(10).await;

    // 1) 先监听再注销：注销在同一条命令内就把成员移出（不等待维护）。
    assert_accepted(&server.register("gone").await);
    assert_accepted(&server.state("gone", "working").await);
    assert_selected(&server.monitor("gone", true).await, "gone");
    assert_accepted(&server.unregister("gone").await);
    assert!(
        server.authoritative_set().await.is_empty(),
        "注销必须立即移出成员"
    );

    // 2) 注销之后再新增监听：登记不存在 → 明确拒绝，且不产生成员。
    assert_refused(&server.monitor("gone", true).await, 404, "unknown-session");
    assert!(
        server.authoritative_set().await.is_empty(),
        "被拒绝的监听不得留下身份"
    );

    // 3) 注销之后再写入：心跳与状态都是未知会话，集合不受影响。
    assert_eq!(server.heartbeat("gone").await.status, 404);
    assert_eq!(server.state("gone", "awaiting-input").await.status, 404);
    assert!(server.authoritative_set().await.is_empty());

    // 4) 重新注册同一身份并监听，随后由维护清除：那一次维护完成后集合里不再有该身份。
    assert_accepted(&server.register("gone").await);
    assert_accepted(&server.state("gone", "working").await);
    assert_selected(&server.monitor("gone", true).await, "gone");
    server.advance(Duration::from_secs(41));
    server.maintain().await;
    assert!(
        server.authoritative_set().await.is_empty(),
        "清除登记的那一遍维护同步移出成员"
    );
    let sessions = server.observe().await.first_sessions();
    assert!(sessions.is_empty(), "登记已被清除：{sessions:?}");

    // 5) 清除之后再用同一身份注册：登记回来了，但不会自动恢复已经移出的监听。
    assert_accepted(&server.register("gone").await);
    assert_accepted(&server.state("gone", "working").await);
    assert!(
        server.authoritative_set().await.is_empty(),
        "重新注册不恢复已移出的监听"
    );
}

/// 代表**不是这台机器**的局域网设备的来源地址：RFC 5737 的 TEST-NET-3 文档地址，本机不可能拥有它。
const LAN_SOURCE: &str = "203.0.113.7:40000";

/// 经验证的应用提交一次请求，来源由受控的 `ConnectInfo` 决定，返回状态码与响应体。
async fn send(app: axum::Router, method: &str, uri: &str, body: String) -> (u16, String) {
    use tower::ServiceExt;

    let mut builder = axum::http::Request::builder().method(method).uri(uri);
    if !body.is_empty() {
        builder = builder.header("content-type", "application/json");
    }
    let response = app
        .oneshot(builder.body(axum::body::Body::from(body)).expect("构造请求"))
        .await
        .expect("请求应用");
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// 验收 P26、P39：关机操作（设置／撤销监听、全局清空）与扩展会话写入是**两条不同的边界**。
///
/// 两侧各用一半接缝：
/// - **真 TCP**：服务监听在本机网卡地址上，连它时的 TCP 源地址就是那个网卡地址（不是回环），代表局域网
///   浏览器——它能设置／撤销监听并执行全局清空。
/// - **受控来源**：`MockConnectInfo` 把对端地址设成 RFC 5737 的文档地址（本机不可能拥有），代表**不是
///   这台机器**的局域网设备——它的会话写入被拒且不建行，但关机操作照常可接受。
#[tokio::test]
async fn a_non_loopback_source_may_monitor_and_clear_but_not_write_session_events() {
    let Some(address) = local_nic_ipv4() else {
        panic!("本机没有可发布的局域网 IPv4，无法构造非回环来源的验证");
    };

    let config = common::config(Path::new("unused.bat"), Path::new("."));
    let sessions = Arc::new(SessionsState::new());
    let double = RecordingPowerOff::new();
    let (app, _state) = herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
    let server = TestServer::start_app_on(SocketAddr::from((address, 0)), app.clone()).await;
    let addr = server.addr();
    assert!(
        !addr.ip().is_loopback(),
        "这一次的连接源必须是回环之外的地址：{addr}"
    );

    // 非回环来源（同机网卡地址，正是扩展经 `herdr-bot.local` 接入时服务端看到的形态）：会话写入可用。
    assert_accepted(
        &post_json_over_tcp(
            addr,
            "/api/sessions/events",
            &json!({"sessionId": "sess-1", "kind": "register", "data": {"cwd": "C:\\work"}})
                .to_string(),
        )
        .await,
    );
    assert_accepted(
        &post_json_over_tcp(
            addr,
            "/api/sessions/events",
            &json!({"sessionId": "sess-1", "kind": "state", "data": {"value": "working"}})
                .to_string(),
        )
        .await,
    );

    // 设置、撤销监听与全局清空都接受这个来源。
    assert_selected(
        &post_json_over_tcp(
            addr,
            MONITOR,
            &json!({"sessionId": "sess-1", "selected": true}).to_string(),
        )
        .await,
        "sess-1",
    );
    assert_selected(
        &post_json_over_tcp(
            addr,
            MONITOR,
            &json!({"sessionId": "sess-1", "selected": true}).to_string(),
        )
        .await,
        "sess-1",
    );
    let cleared = post_over_tcp(addr, CANCEL).await;
    assert_eq!(
        cleared.status, 200,
        "非回环来源的全局清空必须被接受：{} {}",
        cleared.status, cleared.body
    );
    assert!(monitored_ids(&response_body(&cleared)["shutdown"]).is_empty());

    // 受控来源代表「不是这台机器」的局域网设备：写入会话事件被拒。
    let lan = app.clone().layer(MockConnectInfo(
        LAN_SOURCE.parse::<std::net::SocketAddr>().expect("受控来源地址"),
    ));
    let (status, body) = send(
        lan.clone(),
        "POST",
        "/api/sessions/events",
        json!({"sessionId": "lan-1", "kind": "state", "data": {"value": "working"}}).to_string(),
    )
    .await;
    assert_eq!(
        status, 403,
        "非同机来源不得写入扩展 agent 状态：{status} {body}"
    );
    assert_eq!(
        response_body_str(&body)["code"],
        json!("forbidden"),
        "{body}"
    );

    // 被拒绝的写入没有改写任何会话数据：这一行仍然不存在，既有会话保持原样。
    let first = SseConnection::open(addr, "/api/sessions/events", &[])
        .await
        .next_frame()
        .await;
    let sessions_now = first.json()["sessions"].clone();
    assert_eq!(
        sessions_now.as_array().map(|rows| rows.len()),
        Some(1),
        "被拒的写入不得建行：{sessions_now}"
    );
    assert_eq!(
        session_of(sessions_now.as_array().expect("会话列表"), "sess-1")["state"],
        json!("working"),
        "被拒的写入不得改写既有会话：{sessions_now}"
    );

    // 同一个「不是这台机器」的来源仍然可以设置监听与全局清空：两条边界互不影响。
    let (status, body) = send(
        lan,
        "POST",
        MONITOR,
        json!({"sessionId": "sess-1", "selected": true}).to_string(),
    )
    .await;
    assert_eq!(
        status, 200,
        "局域网来源的监听命令必须被接受：{status} {body}"
    );
    assert_eq!(
        response_body_str(&body)["outcome"],
        json!("selected"),
        "{body}"
    );
    assert_eq!(double.calls(), 0);
}

/// 一段响应体文本里的 JSON 对象。
fn response_body_str(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{body}"))
}

/// 本机某个可发布的 IPv4（与 mDNS 发布同一枚举口径）。
fn local_nic_ipv4() -> Option<std::net::Ipv4Addr> {
    if_addrs::get_if_addrs()
        .ok()?
        .into_iter()
        .find_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) if herdr_bot::mdns::is_publishable_ipv4(address.ip) => {
                Some(address.ip)
            }
            _ => None,
        })
}

/// 验收 P46：监听变化与自动触发都经共享观察流下发**完整**关机状态；新读取者读到当前集合与安排。
#[tokio::test]
async fn the_shared_observation_stream_carries_monitoring_and_the_automatic_arrangement() {
    let server = MonitoringServer::start(30).await;
    let mut observer = server.observe().await;

    assert_accepted(&server.register("a").await);
    assert_accepted(&server.state("a", "working").await);

    // 监听集合变化：一帧 `shutdown` 携带完整状态（字段集合与顺序固定）。
    assert_selected(&server.monitor("a", true).await, "a");
    let monitored = observer.set_until(&["a"]).await;
    assert_eq!(monitored["phase"], json!("none"), "{monitored}");

    // 自动触发沿同一条流：先是让条件成立的会话增量，随后是关机状态事件。
    assert_eq!(server.state("a", "awaiting-input").await.status, 200);
    let mut upsert_seen = false;
    let arranged = loop {
        let frame = observer.connection.next_frame().await;
        match frame.event.as_deref() {
            Some("upsert") => {
                assert_eq!(frame.json()["session"]["sessionId"], json!("a"));
                assert_eq!(frame.json()["session"]["state"], json!("awaiting-input"));
                upsert_seen = true;
            }
            Some("shutdown") => {
                let view = shutdown_payload(&frame);
                if view["phase"] == json!("counting-down") {
                    assert!(upsert_seen, "会话增量必须先于它引起的关机状态变化");
                    break (frame, view);
                }
            }
            other => panic!("未预期的帧：{other:?}"),
        }
    };
    let (raw, view) = arranged;
    for field in ["monitoredSessionIds", "arrangementId", "phase", "deadline", "serverTime"] {
        assert!(
            raw.raw.contains(field),
            "自动触发的关机状态必须携带完整字段 {field}：{}",
            raw.raw
        );
    }
    let position = |field: &str| raw.raw.find(field).expect("字段在帧里");
    assert!(
        position("monitoredSessionIds") < position("arrangementId")
            && position("arrangementId") < position("phase")
            && position("phase") < position("deadline")
            && position("deadline") < position("serverTime"),
        "字段顺序必须稳定：{}",
        raw.raw
    );
    assert_eq!(monitored_ids(&view), Vec::<String>::new(), "形成安排即消耗整组监听");
    assert!(view["arrangementId"].is_string(), "安排标识可观察：{view}");
    assert!(view["deadline"].is_u64(), "截止时间可观察：{view}");
    assert!(view["serverTime"].is_u64(), "服务端时间可观察：{view}");

    // 新读取者：首帧同时给出会话快照与当前关机状态（集合、安排标识、阶段、截止时间、服务端时间）。
    let fresh = server.observe().await;
    let first = fresh.first_shutdown();
    assert_eq!(first["phase"], json!("counting-down"), "{first}");
    assert_eq!(first["arrangementId"], view["arrangementId"]);
    assert_eq!(first["deadline"], view["deadline"]);
    assert!(first["serverTime"].is_u64());
    assert!(monitored_ids(&first).is_empty());
    let session = session_of(&fresh.first_sessions(), "a");
    assert_eq!(session["state"], json!("awaiting-input"), "会话那一半照旧：{session}");
    assert_eq!(session["online"], json!(true));
}

/// 验收 P47：资格拒绝与「没有回应」可区分——拒绝是可机器读取的 4xx 类别，且不混入内部详情。
#[tokio::test]
async fn monitoring_refusals_are_machine_readable_and_kept_separate_from_the_response_shape() {
    let server = MonitoringServer::start(10).await;
    assert_accepted(&server.register("awaiting-1").await);
    assert_accepted(&server.state("awaiting-1", "awaiting-input").await);

    let refused = server.monitor("awaiting-1", true).await;
    assert_refused(&refused, 409, "session-not-eligible");
    let body = response_body(&refused);
    let keys: Vec<&String> = body.as_object().expect("拒绝体是对象").keys().collect();
    assert_eq!(
        keys,
        vec!["code", "message"],
        "公开拒绝只带类别与说明，不带任何内部细节：{}",
        refused.body
    );

    // 未注册的会话是另一个可区分的类别。
    assert_refused(&server.monitor("ghost", true).await, 404, "unknown-session");
}

/// 一次明确拒绝：HTTP 状态码 + 复用既有形状的 `{code, message}`。
fn assert_refused(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(body["code"], code, "响应体：{}", response.body);
    assert!(
        body["message"].as_str().is_some_and(|message| !message.is_empty()),
        "拒绝必须带中文说明：{}",
        response.body
    );
}

/// 一条会话事件必须被接受。
fn assert_accepted(response: &HttpResponse) {
    assert_eq!(
        response.status, 200,
        "会话事件必须被接受：{} {}",
        response.status, response.body
    );
}

/// 验收 P7、P44：只有**当前**在线且工作中的未监听会话可新增；页面曾经看到什么不参与判定。
#[tokio::test]
async fn only_a_currently_online_and_working_session_can_be_monitored() {
    let server = MonitoringServer::start(10).await;

    // 在线工作中：可新增。
    assert_accepted(&server.register("working-1").await);
    assert_accepted(&server.state("working-1", "working").await);
    assert_selected(&server.monitor("working-1", true).await, "working-1");
    assert_eq!(server.authoritative_set().await, vec!["working-1".to_string()]);

    // 页面曾见工作中，但命令到达前已经等待用户输入：拒绝，且不改变权威集合。
    assert_accepted(&server.register("late-1").await);
    assert_accepted(&server.state("late-1", "working").await);
    assert_accepted(&server.state("late-1", "awaiting-input").await);
    assert_refused(&server.monitor("late-1", true).await, 409, "session-not-eligible");

    // 等待用户输入：不可新增。
    assert_accepted(&server.register("awaiting-1").await);
    assert_accepted(&server.state("awaiting-1", "awaiting-input").await);
    assert_refused(&server.monitor("awaiting-1", true).await, 409, "session-not-eligible");

    // 空闲：不可新增。
    assert_accepted(&server.register("idle-1").await);
    assert_accepted(&server.state("idle-1", "idle").await);
    assert_refused(&server.monitor("idle-1", true).await, 409, "session-not-eligible");

    // 在线但还没收到过状态：不可新增。
    assert_accepted(&server.register("stateless-1").await);
    assert_refused(&server.monitor("stateless-1", true).await, 409, "session-not-eligible");

    // 未注册的会话：明确拒绝。
    assert_refused(&server.monitor("ghost-1", true).await, 404, "unknown-session");

    // 被拒绝的命令都不改变权威集合：仍然只有最初那一个成员。
    assert_eq!(server.authoritative_set().await, vec!["working-1".to_string()]);
}

/// 验收 P48：监听命令提交的是**明确目标值**——两次选中仍选中，两次未选中仍未选中，不反向翻转。
#[tokio::test]
async fn monitoring_commands_carry_an_explicit_target_value() {
    let server = MonitoringServer::start(10).await;
    assert_accepted(&server.register("working-1").await);
    assert_accepted(&server.state("working-1", "working").await);

    // 两次选中仍是选中。
    assert_selected(&server.monitor("working-1", true).await, "working-1");
    assert_selected(&server.monitor("working-1", true).await, "working-1");
    assert_eq!(server.authoritative_set().await, vec!["working-1".to_string()]);

    // 两次未选中仍未选中。
    assert_unselected(&server.monitor("working-1", false).await, "working-1");
    assert_unselected(&server.monitor("working-1", false).await, "working-1");
    assert!(server.authoritative_set().await.is_empty());

    // 撤销一个当前不满足新增条件的会话：这是一个明确的「未选中」目标，不是反向翻转。
    assert_accepted(&server.register("awaiting-1").await);
    assert_accepted(&server.state("awaiting-1", "awaiting-input").await);
    assert_unselected(&server.monitor("awaiting-1", false).await, "awaiting-1");
    assert!(server.authoritative_set().await.is_empty());
}
