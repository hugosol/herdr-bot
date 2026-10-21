//! 监听资格、恢复后的当前状态裁定与离线撤销（契约 P24、P27、P28、P29、P32、P33、P34）。
//!
//! 被验收的是 S1（后端实例运行 interface）：监听命令（`POST /api/shutdown/monitor`）的接受与拒绝类别、
//! 暂留成员恢复上报后监听意图的保有、自动触发条件只按**当前**在线与 agent 状态裁定、以及离线成员仍可
//! 明确撤销。命令与观察都走真 TCP（`POST /api/sessions/events`、`POST /api/shutdown/monitor` 与
//! `GET /api/sessions/events`），时刻来自受控时钟（[`ManualClock`]），因此「过期但仍在保留期」是精确的
//! t=11 秒，期望值全部取自缺省策略的独立字面值（在线有效期 10 秒、离线保留期 30 秒），不由实现推算。
//!
//! **绝不真的关机**：全部用例都注入记录型关机动作（既有接缝的替身），它只数调用次数，不接触任何系统
//! 调用。P29 需要观察「到期执行」，它用的正是这个替身，因此只证明「后端到点调用了关机动作一次」，
//! 不推断电脑已经断电。
//!
//! 逐项：
//! - P24：在线 working 接受；离线、idle、awaiting-input、状态缺省拒绝为 `409 session-not-eligible`；
//!   无登记拒绝为 `404 unknown-session`；重复声明既有选中值不算新增（不因当前资格被拒）。
//! - P27：离线保留期内接受恢复上报，读取在线且原监听仍在（不需要再次 `selected=true`），
//!   以「尚未满足自动触发」的工作中状态证明监听保有。
//! - P28：A 在线 awaiting-input、B 恢复上报 working：集合继续等待，无倒计时且安全 adapter 零调用；
//!   A 保持在线，隔离「阻塞来自 B 当前仍在工作」这一原因。
//! - P29：A 在线 awaiting-input、B 恢复上报并携带 awaiting-input：非空集合进入倒计时；到期观察到
//!   安全 adapter 被调用一次（不推断真实断电）。其他成员保持在线。
//! - P32：已监听成员分别以 working、idle 阻塞；状态缺省先验证不可新增，再验证不存在把缺省当完成的
//!   可达路径；以「非空且全部在线 awaiting-input」为唯一触发条件。
//! - P33：A awaiting-input 后转 working、B 再进入 awaiting-input 仍不触发，不缓存历史完成资格。
//! - P34：仍有登记的离线 selected 成员提交 `selected=false` 得到接受并移出；随后按统一剩余集合判定
//!   （含移出最后一个成员只清空集合），不因离线拒绝，也不新增恢复专用规则。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp};
use herdr_bot::clock::ManualClock;
use herdr_bot::countdown::ShutdownState;
use herdr_bot::sessions::{SessionPolicy, SessionsState};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

/// 会话事件的唯一写入路径。
const EVENTS: &str = "/api/sessions/events";
/// 监听命令的路径。
const MONITOR: &str = "/api/shutdown/monitor";

/// 记录型关机动作：只数调用次数并记录标识，**绝不**接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
}

impl RecordingPowerOff {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

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

/// 受控时刻下的服务：会话表与关机状态由用例持有，写入／命令／观察分别走真实 TCP 与真实 SSE。
///
/// 会话窗口用[`ManualClock`]把边界变成精确的 t=11 秒；倒计时仍按进程单调时钟真实到期，因此 P29 的
/// 「到点执行」只需要等一个很短的合法倒计时，不需要伪造时间。
struct RecoveryServer {
    server: TestServer,
    sessions: Arc<SessionsState>,
    shutdown: Arc<ShutdownState>,
    clock: Arc<ManualClock>,
    double: Arc<RecordingPowerOff>,
}

impl RecoveryServer {
    /// 缺省会话策略（10／30 秒）＋受控时刻＋指定倒计时秒数。
    async fn start(countdown_seconds: u32) -> Self {
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
        let double = RecordingPowerOff::new();
        let (app, state) = herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            sessions,
            shutdown: state.shutdown,
            clock,
            double,
        }
    }

    /// 把受控时刻向前推进：窗口边界因此可以精确取到。
    fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// 跑一遍维护（与交付运行时 `spawn_expiry_scan` 每 5 秒那一遍是同一对调用）：
    /// 把超过在线有效期的登记翻为离线并发布，随后按当前登记重判监听集合。
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

    /// 提交一次监听命令：明确的目标选中值。
    async fn monitor(&self, session_id: &str, selected: bool) -> HttpResponse {
        post_json_over_tcp(
            self.addr(),
            MONITOR,
            &json!({"sessionId": session_id, "selected": selected}).to_string(),
        )
        .await
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

/// 一次明确拒绝：HTTP 状态码 + 既有 `{code, message}` 形状。
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

/// 登记一个在线、工作中的会话（多步 setup 的公共前缀）。
async fn register_working(server: &RecoveryServer, session_id: &str) {
    assert_accepted(&server.register(session_id).await);
    assert_accepted(&server.state(session_id, "working").await);
}

/// 验收 P24：只有**当前**已登记、在线且工作中的会话可以新增监听；重复声明既有选中值不算新增。
///
/// 期望值取自契约 P24 与用户说明的资格规则（在线 && 工作中）：在线 working → 接受；离线、idle、
/// awaiting-input 与状态缺省 → `409 session-not-eligible`；无登记 → `404 unknown-session`。
/// 能抓住的违规：资格丢掉 `online`（离线也被接受）、丢掉 `state==Working`（idle／等待输入也被接受）、
/// 或把「已在集合里的重申」误当作新增而按当前资格拒绝。
#[tokio::test]
async fn only_a_currently_online_and_working_registration_can_be_added() {
    let server = RecoveryServer::start(10).await;

    // 在线、工作中：接受，权威集合含它。
    register_working(&server, "a").await;
    let added = assert_selected(&server.monitor("a", true).await, "a");
    assert_eq!(monitored_ids(&added), vec!["a".to_string()]);

    // 一个一直在工作的同伴：它让集合不会在 A 进入等待时立刻触发，从而隔离本用例关注的资格判定。
    register_working(&server, "block").await;
    assert_selected(&server.monitor("block", true).await, "block");

    // 重复声明既有选中值：A 已进入等待用户输入、已不满足新增资格，但这是对**同一个目标值**的重申，
    // 不算新增，因此仍被接受，集合不变。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_selected(&server.monitor("a", true).await, "a");
    assert_eq!(
        server.authoritative_set().await,
        vec!["a".to_string(), "block".to_string()],
        "重申既有选中值不改变集合"
    );

    // 另一个在线工作中的会话，用来验证新增接受；`e` 上线但不监听，用来在时间推进后验证「离线」拒绝。
    register_working(&server, "d").await;
    assert_selected(&server.monitor("d", true).await, "d");
    register_working(&server, "e").await;

    // 推进 11 秒：四个登记都超过在线有效期（10 秒）但仍在保留期内 → 离线。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_eq!(
        server.authoritative_set().await,
        vec!["a".to_string(), "block".to_string(), "d".to_string()],
        "离线不移出集合"
    );

    // 已选中成员即使在离线且不满足新增资格时重申，仍是对既有意图的重申：接受、不反向翻转。
    assert_selected(&server.monitor("a", true).await, "a");

    // 离线且**未**选中：拒绝为 409 session-not-eligible。
    assert_refused(&server.monitor("e", true).await, 409, "session-not-eligible");

    // 空闲：拒绝。
    assert_accepted(&server.register("idle-1").await);
    assert_accepted(&server.state("idle-1", "idle").await);
    assert_refused(&server.monitor("idle-1", true).await, 409, "session-not-eligible");

    // 等待用户输入：拒绝。
    assert_accepted(&server.register("awaiting-1").await);
    assert_accepted(&server.state("awaiting-1", "awaiting-input").await);
    assert_refused(
        &server.monitor("awaiting-1", true).await,
        409,
        "session-not-eligible",
    );

    // 在线但从未同步状态：拒绝（缺省状态不被当作工作中）。
    assert_accepted(&server.register("stateless-1").await);
    assert_refused(
        &server.monitor("stateless-1", true).await,
        409,
        "session-not-eligible",
    );

    // 无登记：拒绝为 404 unknown-session。
    assert_refused(&server.monitor("ghost-1", true).await, 404, "unknown-session");

    // 被拒绝的命令都不改变权威集合。
    assert_eq!(
        server.authoritative_set().await,
        vec!["a".to_string(), "block".to_string(), "d".to_string()],
        "拒绝不改变权威集合"
    );
}

/// 验收 P27：离线保留期内接受恢复上报，外部读取在线且**原监听仍在**，不需要再次 `selected=true`。
///
/// 期望值：t=0 上线并监听 b，推进 11 秒后 b 离线但仍在集合里；在保留期内上报 working 后，外部读取
/// `online=true` 且 `monitoredSessionIds` 仍含 b。故意用「工作中」这个**尚未满足自动触发**的状态来证明
/// 监听保有——不靠倒计时是否发生来反推。
/// 能抓住的违规：恢复上报把成员移出集合、或要求用户重新勾选才恢复监听。
#[tokio::test]
async fn a_retained_member_keeps_its_monitoring_intention_after_recovering() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // 超过在线有效期：b 离线，但登记与监听都暂留。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    let offline = server.authoritative().await;
    assert_eq!(monitored_ids(&offline), vec!["b".to_string()], "离线不移出集合");

    // 保留期内的恢复上报：重新计时并恢复在线。
    assert_accepted(&server.state("b", "working").await);

    let observer = server.observe().await;
    let b = session_of(&observer.first_sessions(), "b");
    assert_eq!(b["online"], json!(true), "恢复上报后外部读取在线：{b}");
    assert_eq!(b["state"], json!("working"), "当前 agent 状态按最新上报：{b}");
    assert_eq!(
        monitored_ids(&observer.first_shutdown()),
        vec!["b".to_string()],
        "原监听意图仍保有，无需再次 selected=true"
    );

    // 尚未满足自动触发（b 工作中）：没有倒计时，安全 adapter 零调用。
    let view = observer.first_shutdown();
    assert_eq!(view["phase"], json!("none"), "工作中不触发：{view}");
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");
}

/// 验收 P28：A 在线 awaiting-input、B 恢复上报 working → 集合继续等待，无倒计时且 adapter 零调用。
///
/// A 全程在线（受控时刻不推进），因此「不触发」只能来自 B 当前仍在工作。
/// 能抓住的违规：把恢复在线的成员按「最后上报是等待输入」判定为完成（缓存恢复前的状态）。
#[tokio::test]
async fn a_recovered_member_that_is_still_working_keeps_the_group_waiting() {
    let server = RecoveryServer::start(10).await;

    // A 与 B 都在工作中被监听（两个成员让集合不会在 A 进入等待时立刻触发）。
    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // A 进入等待用户输入：B 仍在工作，整组不满足。
    assert_accepted(&server.state("a", "awaiting-input").await);

    // B 超过在线有效期：离线但仍在集合里；A 也随受控时刻离线。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_eq!(
        monitored_ids(&server.authoritative().await),
        vec!["a".to_string(), "b".to_string()],
        "B 离线后仍在集合里并阻塞"
    );

    // A 的恢复上报：A 重新在线并仍在等待用户输入；B 仍离线 → 不触发。
    assert_accepted(&server.state("a", "awaiting-input").await);

    // B 恢复上报 working：A 保持在线，B 当前仍在工作 → 继续等待。
    assert_accepted(&server.state("b", "working").await);

    let observer = server.observe().await;
    let a = session_of(&observer.first_sessions(), "a");
    assert_eq!(a["online"], json!(true), "A 保持在线以隔离原因：{a}");
    assert_eq!(a["state"], json!("awaiting-input"), "A 仍在等待：{a}");
    let b = session_of(&observer.first_sessions(), "b");
    assert_eq!(b["online"], json!(true), "B 恢复在线的证据：{b}");
    assert_eq!(b["state"], json!("working"), "B 恢复后仍在工作：{b}");
    let view = observer.first_shutdown();
    assert_eq!(
        monitored_ids(&view),
        vec!["a".to_string(), "b".to_string()],
        "集合继续等待：{view}"
    );
    assert_eq!(view["phase"], json!("none"), "B 仍在工作就不触发：{view}");
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");
}

/// 验收 P29：A 在线 awaiting-input、B 恢复上报并携带 awaiting-input → 非空集合进入倒计时；
/// 到期观察到安全 adapter 被调用**一次**（不推断真实电脑已断电）。其他成员保持在线。
///
/// 倒计时取合法最小值 1 秒，用进程单调时钟真实到期；记录型替身只数调用次数。
/// 能抓住的违规：恢复上报不参与整组判定（永远不触发）、或到期不执行。
#[tokio::test]
async fn a_recovered_member_awaiting_input_makes_the_group_trigger_and_execute_once() {
    let server = RecoveryServer::start(3).await;
    let mut observer = server.observe().await;

    // A 与 B 都在工作中被监听（两个成员让集合不会在 A 进入等待时立刻触发）。
    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // A 进入等待用户输入；B 仍在工作 → 整组不满足。
    assert_accepted(&server.state("a", "awaiting-input").await);

    // B 超过在线有效期：离线但仍在集合里（A 也随受控时刻离线）。
    server.advance(Duration::from_secs(11));
    server.maintain().await;

    // A 的恢复上报：A 重新在线并仍在等待；B 离线 → 仍不触发。
    assert_accepted(&server.state("a", "awaiting-input").await);

    // B 在保留期内携带 awaiting-input 的恢复上报：集合如今全部在线并等待用户输入。
    assert_accepted(&server.state("b", "awaiting-input").await);

    // 非空集合现在全部在线并等待用户输入：形成唯一安排并消耗整组监听。
    let arranged = observer.shutdown_until("counting-down", Duration::from_secs(10)).await;
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "开始倒计时即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");

    // A 保持在线：到期前再确认一次当前事实。
    let a = session_of(&server.observe().await.first_sessions(), "a");
    assert_eq!(a["online"], json!(true), "其他成员保持在线：{a}");

    // 到期：后端调用既有安全 adapter 恰一次，阶段推进到「指令已发出」。
    let issued = observer.shutdown_until("issued", Duration::from_secs(10)).await;
    assert_eq!(issued["arrangementId"], arranged["arrangementId"], "同一安排到期执行");
    assert_eq!(
        server.double.calls(),
        1,
        "到期观察到一次安全 adapter 调用（只证明调用，不推断电脑已断电）"
    );
}

/// 验收 P32：已监听成员以 working、idle 阻塞；状态缺省不能新增，也不存在被当作完成的可达路径。
///
/// 唯一触发条件是非空集合全部在线 awaiting-input：working、idle、离线、缺省都不满足；缺省连新增资格
/// 都不具备，因此集合里不可能出现「缺省状态」的成员，也就没有把它当完成的路径。
/// 能抓住的违规：idle 或缺省被当作轮次结束（倒计时）、或任意成员满足即触发（不看整组）。
#[tokio::test]
async fn only_every_online_member_awaiting_input_triggers_the_group() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // A 进入等待、B 仍在工作：整组不满足 → 不触发。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let working_blocks = server.authoritative().await;
    assert_eq!(working_blocks["phase"], json!("none"), "working 阻塞：{working_blocks}");
    assert_eq!(
        monitored_ids(&working_blocks),
        vec!["a".to_string(), "b".to_string()]
    );

    // B 进入空闲（在线、agent 没在跑、本轮尚未观察到结束）：不满足 → 不触发。
    assert_accepted(&server.state("b", "idle").await);
    let idle_blocks = server.authoritative().await;
    assert_eq!(idle_blocks["phase"], json!("none"), "idle 阻塞：{idle_blocks}");
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");

    // 状态缺省不能新增：集合里因此不可能出现缺省状态的成员。
    assert_accepted(&server.register("c").await);
    assert_refused(&server.monitor("c", true).await, 409, "session-not-eligible");
    assert!(
        !server.authoritative_set().await.contains(&"c".to_string()),
        "缺省状态的会话进不了集合"
    );

    // B 也进入等待用户输入：非空集合全部在线 awaiting-input，唯一触发条件成立，
    // 并作为关机状态变化经观察流下发。
    let mut observer = server.observe().await;
    assert_accepted(&server.state("b", "awaiting-input").await);
    let arranged = observer.shutdown_until("counting-down", Duration::from_secs(10)).await;
    assert_eq!(arranged["phase"], json!("counting-down"), "全组等待才触发：{arranged}");
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "触发即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
}

/// 验收 P32（在线这一半）：**离线**成员即使最后上报的是等待用户输入也不满足触发条件。
///
/// 触发条件里的「在线」与「等待用户输入」必须同时成立；仅凭过期的旧状态不构成完成证据。
/// 能抓住的违规：触发条件丢掉 `online`（把离线成员的最后上报状态当作当前完成）。
#[tokio::test]
async fn an_offline_member_awaiting_input_never_satisfies_the_trigger() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");
    // 一个一直在工作的同伴：它单独保证集合在 A、B 都进入等待时不会立刻触发。
    register_working(&server, "blocker").await;
    assert_selected(&server.monitor("blocker", true).await, "blocker");

    // A、B 依次进入等待用户输入：blocker 仍在工作 → 整组不满足。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("b", "awaiting-input").await);
    assert_eq!(server.authoritative().await["phase"], json!("none"));

    // 推进 11 秒并跑维护：三个登记都离线，最后上报的状态各自保留。
    server.advance(Duration::from_secs(11));
    server.maintain().await;

    // 撤销仍被监听的离线 blocker：集合只剩 A、B；两者最后上报都是等待用户输入，但当前都离线。
    assert_unselected(&server.monitor("blocker", false).await, "blocker");
    let offline_pair = server.authoritative().await;
    assert_eq!(
        monitored_ids(&offline_pair),
        vec!["a".to_string(), "b".to_string()],
        "剩下的两个离线成员仍在集合里：{offline_pair}"
    );
    assert_eq!(
        offline_pair["phase"],
        json!("none"),
        "两个都离线时，旧的「等待输入」不满足触发条件：{offline_pair}"
    );

    // A 的恢复上报：A 重新在线且等待用户输入；B 仍离线、最后上报仍是 awaiting-input。
    assert_accepted(&server.state("a", "awaiting-input").await);

    let observer = server.observe().await;
    let a = session_of(&observer.first_sessions(), "a");
    assert_eq!(a["online"], json!(true), "A 在线等待：{a}");
    assert_eq!(a["state"], json!("awaiting-input"), "A 当前等待：{a}");
    let b = session_of(&observer.first_sessions(), "b");
    assert_eq!(b["online"], json!(false), "B 离线：{b}");
    assert_eq!(b["state"], json!("awaiting-input"), "B 的最后上报状态原样：{b}");

    let view = observer.first_shutdown();
    assert_eq!(
        monitored_ids(&view),
        vec!["a".to_string(), "b".to_string()],
        "离线成员仍留在集合里：{view}"
    );
    assert_eq!(
        view["phase"],
        json!("none"),
        "离线的旧「等待输入」不满足触发条件：{view}"
    );
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");
}

/// 验收 P33：A awaiting-input 后转 working、B 再进入 awaiting-input 仍不触发；不缓存历史完成资格。
///
/// 能抓住的违规：把「曾经等待过一次」记为永久资格（B 进入等待时立刻触发）。
#[tokio::test]
async fn a_historical_turn_end_never_counts_as_completion_eligibility() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // A 曾经等待过，随后恢复工作。
    assert_accepted(&server.state("a", "awaiting-input").await);
    assert_accepted(&server.state("a", "working").await);

    // B 现在进入等待用户输入：A 的历史不构成完成资格，整组仍在等待。
    assert_accepted(&server.state("b", "awaiting-input").await);
    let still_pending = server.authoritative().await;
    assert_eq!(
        still_pending["phase"],
        json!("none"),
        "不缓存 A 的历史完成资格：{still_pending}"
    );
    assert_eq!(
        monitored_ids(&still_pending),
        vec!["a".to_string(), "b".to_string()],
        "集合不变：{still_pending}"
    );
    assert_eq!(server.double.calls(), 0);

    // A 再次进入等待用户输入：这才满足条件。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let arranged = server.authoritative().await;
    assert_eq!(arranged["phase"], json!("counting-down"), "A 再次等待才触发：{arranged}");
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
}

/// 验收 P34：仍有登记的离线 selected 成员提交 `selected=false` 得到接受并移出，不因离线拒绝；
/// 随后按 02 的统一剩余集合判定（非空全等待即触发；移出最后一个成员只清空集合）。
///
/// 能抓住的违规：撤销路径复用新增资格（离线 → `409 session-not-eligible`）、或为恢复／离线另加一套规则。
#[tokio::test]
async fn an_offline_monitored_member_can_still_be_withdrawn() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "a").await;
    assert_selected(&server.monitor("a", true).await, "a");
    register_working(&server, "b").await;
    assert_selected(&server.monitor("b", true).await, "b");

    // 推进 11 秒：两个成员都离线但仍保留在集合里。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_eq!(
        server.authoritative_set().await,
        vec!["a".to_string(), "b".to_string()],
        "离线不移出集合"
    );

    // 离线 B 提交明确的未选中值：接受（不是 409），B 被移出。
    let withdrawn = assert_unselected(&server.monitor("b", false).await, "b");
    assert_eq!(
        monitored_ids(&withdrawn),
        vec!["a".to_string()],
        "离线成员的撤销立即生效，集合只剩 A：{withdrawn}"
    );
    // 剩下的 A 目前离线（超过在线有效期）：不触发，使用与新增/撤销同一套整组判定。
    assert_eq!(withdrawn["phase"], json!("none"), "A 离线就不触发：{withdrawn}");
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");

    // A 的恢复上报（等待用户输入）：剩余非空集合全部在线等待 → 按统一规则立即触发。
    assert_accepted(&server.state("a", "awaiting-input").await);
    let arranged = server.authoritative().await;
    assert_eq!(
        arranged["phase"],
        json!("counting-down"),
        "剩余集合按同一条件重新判定：{arranged}"
    );
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "触发即消耗整组监听：{arranged}"
    );
    assert_eq!(server.double.calls(), 0, "倒计时中还没有关机调用");
}

/// 验收 P34（末位分支）：撤销仍有登记的**最后一个**离线监听成员得到接受，只清空集合、不触发安排。
///
/// P30／P31 的通用覆盖归 02；这里只证明离线撤销走的是同一套剩余集合判定。
#[tokio::test]
async fn withdrawing_the_last_offline_member_never_starts_a_countdown() {
    let server = RecoveryServer::start(10).await;

    register_working(&server, "only").await;
    assert_selected(&server.monitor("only", true).await, "only");

    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_eq!(
        server.authoritative_set().await,
        vec!["only".to_string()],
        "离线成员仍在集合里"
    );

    let withdrawn = assert_unselected(&server.monitor("only", false).await, "only");
    assert!(
        monitored_ids(&withdrawn).is_empty(),
        "撤销最后一个成员只清空集合：{withdrawn}"
    );
    assert_eq!(withdrawn["phase"], json!("none"), "空集合不触发：{withdrawn}");

    let after = server.authoritative().await;
    assert!(monitored_ids(&after).is_empty(), "外部读取确认为空集合：{after}");
    assert_eq!(after["phase"], json!("none"), "外部读取确认没有安排：{after}");
    assert_eq!(server.double.calls(), 0, "没有安排就没有任何关机调用");
}
