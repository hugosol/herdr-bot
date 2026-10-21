//! 全局关机倒计时的独立性与两个入口的一致性（契约 P35、P36、P37、P38、P39；ADR 0005）。
//!
//! 被验收的是 S1（后端实例运行 interface）：手动开始意图（`POST /api/shutdown/start`）、全局取消
//! （`POST /api/shutdown/cancel`）与完成后关机监听（`POST /api/shutdown/monitor`）的接受／拒绝类别，
//! 以及共享观察流（`GET /api/sessions/events`）里被消耗的监听集合、安排标识、截止时间与终局。
//! 命令与观察都走真 TCP：用例不读任何进程内状态，看到的就是页面会收到的 HTTP 响应与 SSE 帧。
//! 会话窗口用受控时刻（[`ManualClock`]）把「上报过期但登记仍在保留期内」变成精确的 t=11／t=41，
//! 倒计时用启动期配置里的独立字面量（3／5／10 秒）真实到期。
//!
//! P19（进程重启不恢复登记、监听意图与活动安排）由 `tests/manual_shutdown_process.rs` 在**安全壳
//! 进程**上覆盖：那里用同一份装配、真实 HTTP／SSE 与独立的进程启动。
//!
//! **绝不真的关机**：全部用例都注入记录型关机动作（既有接缝的替身），它只数调用次数（需要观察
//! 「执行已开始」时停在返回之前等用例放行），不接触任何系统调用；到期执行只证明「后端调用了关机
//! 动作一次」，不推断电脑已经断电。
//!
//! 逐项：
//! - P35：监听组含一条离线成员与一条在线工作中的成员（自动触发条件不成立，且不需要任何 agent 状态
//!   变化）时，合法手动开始仍立即建立安排。
//! - P36：同一活动安排分别经历成员离线、登记被维护清除、明确注销与重新工作，安排标识与截止时间都
//!   不变（不取消、不延长、不重算）；到期只经安全 adapter 执行一次。
//! - P37：手动与自动两个入口参数化：任一入口开始倒计时都立即消耗整组监听，取消之后被消耗的成员
//!   不回来，重复的等待上报也不重启安排。
//! - P38：活动倒计时期间对新的在线 working 登记新增监听得到精确 `409 shutdown-in-progress`，
//!   权威集合不新增；拒绝体只有类别与说明。
//! - P39：仅监听（没有倒计时）与尚可取消的倒计时分别覆盖全局取消：回执与观察都体现清空／撤销，
//!   原截止点安全 adapter 不执行；执行已开始时得到 `409 shutdown-not-cancellable`，不宣称撤回。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use herdr_bot::clock::ManualClock;
use herdr_bot::countdown::ShutdownState;
use herdr_bot::sessions::{SessionPolicy, SessionsState};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// 会话事件的唯一写入路径。
const EVENTS: &str = "/api/sessions/events";
/// 监听命令的请求路径。
const MONITOR: &str = "/api/shutdown/monitor";
/// 手动开始意图的请求路径。
const START: &str = "/api/shutdown/start";
/// 全局取消的请求路径。
const CANCEL: &str = "/api/shutdown/cancel";

/// 「执行已开始」的闸门：让用例稳定地停在执行已经开始、尚未返回的那个阶段上。
///
/// 它不模拟关机，只是把记录型替身的返回时机交给用例；替身的等待是有界的（见 [`Gate::WAIT_LIMIT`]），
/// 因此用例即使失败也不会把测试进程永久挂住。
struct Gate {
    /// 执行开始时通知用例：此刻阶段已经是「执行已开始」。
    entered: mpsc::UnboundedSender<()>,
    /// 用例放行后才返回。
    released: AtomicBool,
}

impl Gate {
    /// 替身最多等多久：用例失败时也不会把工作线程永久占住。
    const WAIT_LIMIT: Duration = Duration::from_secs(60);
}

/// 记录型关机动作：只数调用次数，**绝不**接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
    gate: Option<Gate>,
}

impl RecordingPowerOff {
    /// 立即返回成功的替身。
    fn new() -> Arc<Self> {
        Self::build(None)
    }

    /// 成功返回、但在「执行已开始」处停下等用例放行的替身；返回的接收端在替身被调用时收到通知。
    fn gated() -> (Arc<Self>, mpsc::UnboundedReceiver<()>) {
        let (entered, receiver) = mpsc::unbounded_channel();
        let double = Self::build(Some(Gate {
            entered,
            released: AtomicBool::new(false),
        }));
        (double, receiver)
    }

    fn build(gate: Option<Gate>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            gate,
        })
    }

    /// 到目前为止被调用了几次。
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// 放行停在「执行已开始」处的调用。
    fn release(&self) {
        if let Some(gate) = &self.gate {
            gate.released.store(true, Ordering::SeqCst);
        }
    }
}

impl PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            let _ = gate.entered.send(());
            let deadline = Instant::now() + Gate::WAIT_LIMIT;
            while !gate.released.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        Ok(())
    }
}

/// 一个用例独占的服务：真实监听端口、与交付运行时同一份装配、记录型关机动作与受控会话时刻。
struct CountdownServer {
    server: TestServer,
    sessions: Arc<SessionsState>,
    shutdown: Arc<ShutdownState>,
    clock: Arc<ManualClock>,
    double: Arc<RecordingPowerOff>,
}

impl CountdownServer {
    /// 缺省会话策略（10／30 秒）＋受控时刻＋指定倒计时秒数的装配。
    async fn start(countdown_seconds: u32) -> Self {
        Self::start_with(countdown_seconds, RecordingPowerOff::new()).await
    }

    /// 同 [`CountdownServer::start`]，但替身停在「执行已开始」处；返回的接收端用来确认已进入该阶段。
    async fn start_gated(countdown_seconds: u32) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (double, entered) = RecordingPowerOff::gated();
        (Self::start_with(countdown_seconds, double).await, entered)
    }

    async fn start_with(countdown_seconds: u32, double: Arc<RecordingPowerOff>) -> Self {
        let clock = Arc::new(ManualClock::new());
        let config = common::config_with_countdown(
            Path::new("unused.bat"),
            Path::new("."),
            countdown_seconds,
        );
        let sessions = Arc::new(SessionsState::with_policy_and_clock(
            SessionPolicy::default(),
            clock.clone(),
        ));
        let (app, state) = herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            sessions,
            shutdown: state.shutdown,
            clock,
            double,
        }
    }

    /// 推进受控时刻：窗口边界因此可以精确取到。
    fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// 跑一遍维护（与交付运行时 `spawn_expiry_scan` 每 5 秒那一遍是同一对调用）。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&self, body: String) -> HttpResponse {
        post_json_over_tcp(self.addr(), EVENTS, &body).await
    }

    /// 注册（或重注册）一个会话。
    async fn register(&self, session_id: &str) -> HttpResponse {
        self.post(
            json!({"sessionId": session_id, "kind": "register", "data": {"cwd": "C:\\work"}})
                .to_string(),
        )
        .await
    }

    /// 上报一次 agent 状态。
    async fn state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post(
            json!({"sessionId": session_id, "kind": "state", "data": {"value": value}}).to_string(),
        )
        .await
    }

    /// 一次心跳（不带状态）：只刷新上报年龄。
    async fn heartbeat(&self, session_id: &str) -> HttpResponse {
        self.post(json!({"sessionId": session_id, "kind": "heartbeat", "data": {}}).to_string())
            .await
    }

    /// 注销一个会话。
    async fn unregister(&self, session_id: &str) -> HttpResponse {
        self.post(json!({"sessionId": session_id, "kind": "unregister"}).to_string())
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

    /// 提交一次全局取消。
    async fn cancel_shutdown(&self) -> HttpResponse {
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
    async fn shutdown_until(&mut self, phase: &str, timeout: Duration) -> Value {
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

    /// 一直读到监听集合等于 `expected`，返回那一帧的完整关机状态。
    async fn set_until(&mut self, expected: &[&str], timeout: Duration) -> Value {
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

/// 首帧会话列表里某个会话的视图。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("首帧里应有会话 {session_id}：{sessions:?}"))
}

/// 一条会话事件必须被接受。
fn assert_accepted(response: &HttpResponse) {
    assert_eq!(
        response.status, 200,
        "会话事件必须被接受：{} {}",
        response.status, response.body
    );
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

/// 一次被接受的手动开始意图：结果类别是安排阶段，返回命令之后的权威关机状态。
fn assert_started(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "开始意图应被接受：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("counting-down"),
        "结果类别必须是安排阶段：{}",
        response.body
    );
    body["shutdown"].clone()
}

/// 一次成功的全局取消：类别是 `cancelled`，返回命令之后的权威关机状态。
fn assert_cancelled(response: &HttpResponse) -> Value {
    assert_eq!(response.status, 200, "取消必须生效：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("cancelled"),
        "结果类别必须如实回报：{}",
        response.body
    );
    body["shutdown"].clone()
}

/// 一次明确拒绝：HTTP 状态码 + 既有 `{code, message}` 形状，且拒绝体不夹带任何业务状态。
fn assert_refused(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(body["code"], code, "响应体：{}", response.body);
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "拒绝必须带中文说明：{}",
        response.body
    );
    let keys: Vec<&String> = body.as_object().expect("拒绝体是对象").keys().collect();
    assert_eq!(
        keys,
        vec!["code", "message"],
        "拒绝体只能是类别与说明，不能夹带权威状态：{}",
        response.body
    );
}

/// 权威监听集合必须为空：开始倒计时即消耗整组监听，取消即清空。
fn assert_no_monitoring(view: &Value, what: &str) {
    assert!(monitored_ids(view).is_empty(), "{what}：{view}");
}

/// 安排的时长（服务端时钟毫秒）：期望值来自启动期配置里的独立字面量，不由实现推算。
fn arranged_millis(view: &Value) -> u64 {
    view["deadline"].as_u64().expect("倒计时中必须有截止时间")
        - view["serverTime"].as_u64().expect("服务端时间必须可读")
}

/// 登记一个在线、工作中的会话（多步 setup 的公共前缀）。
async fn register_working(server: &CountdownServer, session_id: &str) {
    assert_accepted(&server.register(session_id).await);
    assert_accepted(&server.state(session_id, "working").await);
}

/// 活动安排没有因为一次会话变化被取消或延长：阶段、标识与截止时间都原样，安全 adapter 仍未被调用。
async fn assert_arrangement_unchanged(
    server: &CountdownServer,
    arrangement_id: &Value,
    deadline: &Value,
    change: &str,
) {
    let view = server.authoritative().await;
    assert_eq!(
        view["phase"],
        json!("counting-down"),
        "{change} 之后安排仍必须在倒计时中：{view}"
    );
    assert_eq!(view["arrangementId"], *arrangement_id, "{change} 不得换安排：{view}");
    assert_eq!(view["deadline"], *deadline, "{change} 不得延长或重算截止时间：{view}");
    assert_no_monitoring(&view, &format!("{change} 不得改变被消耗的监听集合"));
    assert_eq!(server.double.calls(), 0, "{change} 之后仍未到截止时间：{view}");
}

/// 两个开始入口：手动开始意图与自动触发（监听组成员全部进入等待用户输入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Manual,
    Automatic,
}

/// 验收 P35：监听组含离线与在线工作中的成员、自动触发条件不成立时，合法手动开始仍立即建立安排。
///
/// 期望值取自启动期配置里的独立字面量（10 秒）与 P35 的规则：手动入口不看会话状态，也不等 agent
/// 结束当前轮次。能抓住的违规：手动开始被改成要求「监听集合已满足自动触发条件」（例如复用自动
/// 判定的 `all_ready`），于是集合里还有 working／离线成员时它仍然等待、不建立安排。
#[tokio::test]
async fn a_manual_start_arranges_while_the_monitored_group_is_not_ready() {
    let server = CountdownServer::start(10).await;
    let mut observer = server.observe().await;

    // 两个成员都在工作：自动条件不成立（还有成员在跑）。
    register_working(&server, "a").await;
    register_working(&server, "b").await;
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");

    // a 的上报过期（登记仍在保留期内，因此仍在监听集合里），b 在同一时刻重新上报——它保持在线而且
    // 仍然工作中。集合因此同时含一条离线成员与一条在线工作中成员。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_accepted(&server.heartbeat("b").await);
    let sessions = server.observe().await.first_sessions();
    assert_eq!(
        session_of(&sessions, "a")["online"],
        json!(false),
        "a 的上报已过期：{sessions:?}"
    );
    assert_eq!(
        session_of(&sessions, "b")["online"],
        json!(true),
        "b 重新上报后仍在线：{sessions:?}"
    );
    assert_eq!(
        session_of(&sessions, "b")["state"],
        json!("working"),
        "b 仍在工作，没有任何轮次结束：{sessions:?}"
    );

    // 自动入口此刻不成立：没有安排，安全 adapter 零调用，集合保持两名成员。
    let pending = server.authoritative().await;
    assert_eq!(pending["phase"], json!("none"), "自动条件不成立：{pending}");
    assert_eq!(server.authoritative_set().await, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(server.double.calls(), 0);

    // 合法手动开始：立即建立唯一安排——不等 a 重新上报，也不等 b 结束当前轮次。
    let started = assert_started(&server.start_shutdown().await);
    assert_eq!(started["phase"], json!("counting-down"), "{started}");
    assert_eq!(
        arranged_millis(&started),
        10_000,
        "手动入口用启动期配置的时长，与自动入口同一来源：{started}"
    );
    assert_no_monitoring(&started, "开始倒计时即消耗整组监听");
    assert_eq!(server.double.calls(), 0, "倒计时中还没有执行");

    // 不依赖回执：共享观察流给出同一安排与同一个截止时间。
    let observed = observer.shutdown_until("counting-down", Duration::from_secs(10)).await;
    assert_eq!(observed["arrangementId"], started["arrangementId"]);
    assert_eq!(observed["deadline"], started["deadline"]);
    assert_no_monitoring(&observed, "观察到的集合同样为空");
}

/// 验收 P36：同一活动安排经历成员离线、登记清除、注销与重新工作都不改变标识与截止时间。
///
/// 期望值取自启动期配置里的独立字面量（5 秒）与创建时读到的安排标识／截止时间。能抓住的违规：
/// 会话变化（写入、维护清除、注销）触发对活动安排的重判，于是倒计时被取消、被延长或被重算
/// ——例如 `judge_locked` 不再在活动安排存在时短路，或维护按新起点重建安排。
#[tokio::test]
async fn an_active_countdown_survives_member_offline_deletion_unregister_and_resume() {
    let server = CountdownServer::start(5).await;
    let mut observer = server.observe().await;

    // 自动入口建立安排：三名监听成员全部在线等待用户输入。
    for id in ["a", "b", "c"] {
        register_working(&server, id).await;
        assert_selected(&server.monitor(id, true).await, id);
    }
    for id in ["a", "b", "c"] {
        assert_accepted(&server.state(id, "awaiting-input").await);
    }
    // 时刻与时长都取自安排**创建那一刻**下发的帧：截止时间在创建时定死，之后的快照只能如实复述它。
    let arranged = observer.shutdown_until("counting-down", Duration::from_secs(10)).await;
    assert_eq!(arranged["phase"], json!("counting-down"), "{arranged}");
    assert_eq!(arranged_millis(&arranged), 5_000, "自动入口的时长：{arranged}");
    assert_no_monitoring(&arranged, "开始倒计时即消耗整组监听");
    let arrangement_id = arranged["arrangementId"].clone();
    let deadline = arranged["deadline"].clone();

    // ① 成员离线：上报过期，但登记仍在离线保留期内。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_arrangement_unchanged(&server, &arrangement_id, &deadline, "成员离线").await;

    // ② 登记被维护清除：越过在线有效期与离线保留期之和。
    server.advance(Duration::from_secs(30));
    server.maintain().await;
    assert_arrangement_unchanged(&server, &arrangement_id, &deadline, "登记被清除").await;

    // ③ 明确注销：重新登记一个身份再注销它。
    register_working(&server, "a").await;
    assert_accepted(&server.unregister("a").await);
    assert_arrangement_unchanged(&server, &arrangement_id, &deadline, "明确注销").await;

    // ④ 成员重新工作：回到在线工作中也不改变已经开始的安排。
    register_working(&server, "c").await;
    assert_arrangement_unchanged(&server, &arrangement_id, &deadline, "成员重新工作").await;

    // 到期只经安全 adapter 观察执行：恰好一次，且结果仍属于同一安排。
    assert!(
        common::wait_until(Duration::from_secs(15), || server.double.calls() == 1).await,
        "活动安排必须在到期时执行一次（当前 {} 次）",
        server.double.calls()
    );
    let settled = server.authoritative().await;
    assert_eq!(settled["phase"], json!("issued"), "结果仍属于同一安排：{settled}");
    assert_eq!(settled["arrangementId"], arrangement_id);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.double.calls(), 1, "已经开始的安排只执行一次，不重试");
}

/// 验收 P37：手动与自动入口都立即消耗整组监听，取消之后被消耗的成员不回来。
///
/// 两个入口参数化在同一用例里：`Entry::Manual` 提交开始意图，`Entry::Automatic` 让全部成员进入
/// 等待用户输入。能抓住的违规：某个入口不消耗集合（例如只清空一半、或取消时把成员恢复），
/// 或重复的等待上报让被消耗的成员重新凑成集合并重启安排。
#[tokio::test]
async fn both_entries_consume_the_whole_group_and_a_cancel_never_restores_members() {
    for entry in [Entry::Manual, Entry::Automatic] {
        let server = CountdownServer::start(10).await;
        let mut observer = server.observe().await;
        register_working(&server, "a").await;
        register_working(&server, "b").await;
        assert_selected(&server.monitor("a", true).await, "a");
        let before = assert_selected(&server.monitor("b", true).await, "b");
        assert_eq!(monitored_ids(&before), vec!["a".to_string(), "b".to_string()]);

        let started = match entry {
            Entry::Manual => assert_started(&server.start_shutdown().await),
            Entry::Automatic => {
                assert_accepted(&server.state("a", "awaiting-input").await);
                assert_accepted(&server.state("b", "awaiting-input").await);
                let view = observer.shutdown_until("counting-down", Duration::from_secs(10)).await;
                assert_eq!(arranged_millis(&view), 10_000, "入口 {entry:?}：{view}");
                view
            }
        };
        assert_eq!(started["phase"], json!("counting-down"), "入口 {entry:?}：{started}");
        assert_no_monitoring(&started, &format!("入口 {entry:?} 的开始回执必须给出被消耗后的集合"));
        // 独立观察者：集合的确为空，不只看回执。
        assert_eq!(server.authoritative_set().await, Vec::<String>::new());
        assert_eq!(server.double.calls(), 0, "入口 {entry:?}：倒计时中还没有执行");

        // 被消耗的成员再次报告「等待用户输入」：不得塞回集合，也不得重启安排。
        assert_accepted(&server.state("a", "awaiting-input").await);
        assert_accepted(&server.state("b", "awaiting-input").await);
        let repeated = server.authoritative().await;
        assert_no_monitoring(&repeated, "重复的等待上报不得恢复被消耗的成员");
        assert_eq!(
            repeated["arrangementId"], started["arrangementId"],
            "入口 {entry:?}：重复上报不得重启安排：{repeated}"
        );
        assert_eq!(repeated["deadline"], started["deadline"]);

        // 全局取消：清空并撤销这次安排；被消耗的成员也不会因为取消而回来。
        let cancelled = assert_cancelled(&server.cancel_shutdown().await);
        assert_eq!(cancelled["phase"], json!("cancelled"), "入口 {entry:?}：{cancelled}");
        assert_no_monitoring(&cancelled, "取消即清空监听集合");
        assert_accepted(&server.state("a", "awaiting-input").await);
        assert_accepted(&server.state("b", "awaiting-input").await);
        let after_cancel = observer.shutdown_until("cancelled", Duration::from_secs(10)).await;
        assert_no_monitoring(&after_cancel, "取消之后集合保持空，需要用户重新选择");
        assert_eq!(server.authoritative_set().await, Vec::<String>::new());
        assert_eq!(server.double.calls(), 0, "入口 {entry:?}：取消后原截止点不得执行");
    }
}

/// 验收 P38：活动倒计时期间不接受新增监听，得到精确 409 `shutdown-in-progress`，集合不新增。
///
/// 期望值取自契约 P38 的类别码与「活动倒计时期间集合不新增」。能抓住的违规：把这条拒绝并进
/// `session-not-eligible`、在活动安排期间照旧接受新增、或让被拒绝的命令改变权威集合。
#[tokio::test]
async fn adding_monitoring_during_an_active_countdown_is_refused_precisely() {
    let server = CountdownServer::start(10).await;
    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    let started = assert_started(&server.start_shutdown().await);
    assert_eq!(started["phase"], json!("counting-down"), "{started}");

    // 新的在线 working 登记：资格本身满足，但活动倒计时期间仍然被拒绝。
    register_working(&server, "b").await;
    let refusal = server.monitor("b", true).await;
    assert_refused(&refusal, 409, "shutdown-in-progress");
    // 刚刚被消耗掉的成员同样不能再被新增。
    assert_refused(&server.monitor("a", true).await, 409, "shutdown-in-progress");

    let after = server.authoritative().await;
    assert_no_monitoring(&after, "被拒绝的新增不得进入集合");
    assert_eq!(after["phase"], json!("counting-down"));
    assert_eq!(after["arrangementId"], started["arrangementId"]);
    assert_eq!(after["deadline"], started["deadline"], "被拒绝的命令不得重置安排");
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P39（仅监听）：没有倒计时时全局取消仍然清空整组监听，回执与观察都体现清空。
///
/// 能抓住的违规：取消只在有活动安排时生效、或只在回执里清空而观察流没有发布新的集合。
#[tokio::test]
async fn a_global_cancel_without_a_countdown_clears_the_whole_monitored_set() {
    let server = CountdownServer::start(10).await;
    let mut observer = server.observe().await;
    register_working(&server, "a").await;
    register_working(&server, "b").await;
    assert_selected(&server.monitor("a", true).await, "a");
    assert_selected(&server.monitor("b", true).await, "b");
    assert_eq!(server.authoritative_set().await, vec!["a".to_string(), "b".to_string()]);

    let receipt = assert_cancelled(&server.cancel_shutdown().await);
    assert_no_monitoring(&receipt, "取消回执必须给出清空后的集合");
    assert_eq!(receipt["phase"], json!("none"), "本来就没有倒计时：{receipt}");
    assert!(
        receipt.get("arrangementId").is_none(),
        "没有安排时回执不得编造安排标识：{receipt}"
    );

    // 观察体现清空：集合从两名成员变为空。
    let observed = observer.set_until(&[], Duration::from_secs(10)).await;
    assert_eq!(observed["phase"], json!("none"));
    assert_eq!(server.double.calls(), 0);

    // 取消后不恢复：成员再次进入等待用户输入也不会凑成集合或开始倒计时。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("b", "awaiting-input").await);
    let after = server.authoritative().await;
    assert_no_monitoring(&after, "取消后集合保持空");
    assert_eq!(after["phase"], json!("none"), "空集合不形成安排：{after}");
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P39（尚可取消的倒计时）：全局取消撤销该安排，越过原截止时间安全 adapter 不执行。
///
/// 期望值取自启动期配置里的独立字面量（3 秒）与创建时读到的安排标识。能抓住的违规：取消没有真正
/// 阻止到期执行（越过原截止时间仍调用一次关机），或回执／观察谎称撤销了别的安排。
#[tokio::test]
async fn a_cancellable_countdown_is_revoked_and_its_original_deadline_stays_silent() {
    let server = CountdownServer::start(3).await;
    let mut observer = server.observe().await;
    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    let started = assert_started(&server.start_shutdown().await);
    assert_eq!(arranged_millis(&started), 3_000, "{started}");

    let receipt = assert_cancelled(&server.cancel_shutdown().await);
    assert_eq!(receipt["phase"], json!("cancelled"), "{receipt}");
    assert_eq!(
        receipt["arrangementId"], started["arrangementId"],
        "回执必须如实给出被撤销的安排：{receipt}"
    );
    assert_no_monitoring(&receipt, "取消即清空监听集合");

    let observed = observer.shutdown_until("cancelled", Duration::from_secs(10)).await;
    assert_eq!(observed["arrangementId"], started["arrangementId"]);
    assert_no_monitoring(&observed, "观察到的集合同样为空");

    // 越过原截止时间：这次安排不得发出任何关机调用。
    tokio::time::sleep(Duration::from_millis(3_600)).await;
    assert_eq!(server.double.calls(), 0, "取消生效后原截止点不得执行");
    let settled = server.authoritative().await;
    assert_eq!(settled["phase"], json!("cancelled"), "越过截止时间仍是取消的终点：{settled}");
    assert_eq!(settled["arrangementId"], started["arrangementId"]);
    assert!(settled.get("deadline").is_none(), "终局没有截止时间：{settled}");
}

/// 验收 P39（执行已开始）：执行已经开始时取消得到 409 `shutdown-not-cancellable`，不宣称撤回。
///
/// 期望值取自契约 P39 保留的既有类别码。能抓住的违规：执行开始后仍回报取消成功、把这次取消
/// 记成终局、或让同一安排既取消成功又发出调用。
#[tokio::test]
async fn a_cancel_once_the_execution_started_is_refused_and_never_withdrawn() {
    let (server, mut entered) = CountdownServer::start_gated(1).await;
    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    let started = assert_started(&server.start_shutdown().await);

    // 到期执行已经开始：安全 adapter 停在返回之前，阶段因此稳定地停在「执行已开始」。
    entered.recv().await.expect("到期必须开始执行一次");
    assert_eq!(server.double.calls(), 1, "这次执行已经发生");
    let during = server.authoritative().await;
    assert_eq!(during["phase"], json!("executing"), "{during}");
    assert_eq!(during["arrangementId"], started["arrangementId"]);

    let mut observer = server.observe().await;
    let refusal = server.cancel_shutdown().await;
    assert_refused(&refusal, 409, "shutdown-not-cancellable");
    let after_refusal = server.authoritative().await;
    assert_eq!(
        after_refusal["phase"],
        json!("executing"),
        "被拒绝的取消失效，本次执行不受影响：{after_refusal}"
    );
    assert_eq!(after_refusal["arrangementId"], started["arrangementId"]);

    // 放行执行并让它走到终局：结果属于同一安排，取消从未生效，也不重试。
    server.double.release();
    let issued = observer.shutdown_until("issued", Duration::from_secs(15)).await;
    assert_eq!(issued["arrangementId"], started["arrangementId"]);
    assert_eq!(server.double.calls(), 1, "只执行一次，不重试");
}
