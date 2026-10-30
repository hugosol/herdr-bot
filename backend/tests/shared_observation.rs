//! 共享观察流：首帧全量、命名增量、慢观察者隔离与多连接收敛（契约 P21、P22、P23、P48）。
//!
//! 被验收的是 S1（后端实例运行 interface）：**真实** TCP 上的会话事件与关机命令、**真实** SSE 长连接
//! （与交付运行时同一份装配、同一份路由），以及受控时刻驱动的维护。用例看到的就是页面会收到的 HTTP
//! 响应与 SSE 帧——不读任何进程内状态，也不使用内存发布或转发断言。
//!
//! **绝不真的关机**：全部用例注入记录型关机动作（既有接缝的替身，只数调用次数）。本票只观察新连接的
//! 首帧、打开期间的增量送达与慢观察者的隔离，不驱动到期执行；因此所有用例里的记录型替身调用数都必须
//! 是 0。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use herdr_bot::clock::ManualClock;
use herdr_bot::shutdown::ShutdownState;
use herdr_bot::sessions::{SessionPolicy, SessionsState};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

/// 会话事件写入路径。
const EVENTS: &str = "/api/sessions/events";
/// 监听命令路径。
const MONITOR: &str = "/api/shutdown/monitor";
/// 手动开始意图路径。
const START: &str = "/api/shutdown/start";

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

/// 一个用例独占的实例：真实监听端口、与交付运行时同一份装配、受控时刻、记录型关机动作。
struct ObservationServer {
    server: TestServer,
    double: Arc<RecordingPowerOff>,
    shutdown: Arc<ShutdownState>,
    sessions: Arc<SessionsState>,
    clock: Arc<ManualClock>,
}

impl ObservationServer {
    /// 用指定的倒计时秒数装配应用；会话窗口取缺省策略（在线 10 秒、离线保留 30 秒）。
    async fn start(countdown_seconds: u32) -> Self {
        let config = common::config_with_countdown(
            Path::new("unused.bat"),
            Path::new("."),
            countdown_seconds,
        );
        let clock = Arc::new(ManualClock::new());
        let sessions = Arc::new(SessionsState::with_policy_and_clock(
            SessionPolicy::default(),
            clock.clone(),
        ));
        let double = RecordingPowerOff::new();
        let (app, state) =
            herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            double,
            shutdown: state.shutdown,
            sessions,
            clock,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    async fn post_event(&self, body: String) -> HttpResponse {
        post_json_over_tcp(self.addr(), EVENTS, &body).await
    }

    /// 注册（或重注册）一个会话。
    async fn register(&self, session_id: &str) -> HttpResponse {
        self.register_in(session_id, r"C:\work").await
    }

    /// 用指定工作目录注册：目录会出现在会话视图里，因此也决定每帧的大小。
    async fn register_in(&self, session_id: &str, cwd: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "register", "data": {"cwd": cwd}}).to_string(),
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

    /// 一次心跳（刷新最后写入时间，不带状态）。
    async fn heartbeat(&self, session_id: &str) -> HttpResponse {
        self.post_event(
            json!({"sessionId": session_id, "kind": "heartbeat", "data": {}}).to_string(),
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

    /// 跑一遍维护（与交付运行时的 `spawn_expiry_scan` 每 5 秒那一遍是同一对调用）。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    /// 推进受控时刻：窗口边界因此是精确时刻，不需要真的等。
    fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// 建一条观察连接并读掉首帧。
    async fn observe(&self) -> Observer {
        Observer::open(self.addr(), &[]).await
    }

    /// 建一条**尚未**读首帧的观察连接：用于「读者故意不读」的场景。
    async fn connect(&self) -> SseConnection {
        SseConnection::open(self.addr(), EVENTS, &[]).await
    }
}

/// 一条已经读过首帧 `snapshot` 的观察连接。
struct Observer {
    connection: SseConnection,
    first: SseFrame,
}

impl Observer {
    async fn open(addr: SocketAddr, extra_headers: &[(&str, &str)]) -> Self {
        let mut connection = SseConnection::open(addr, EVENTS, extra_headers).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        Self { connection, first }
    }

    /// 首帧里的关机状态。
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

    /// 下一帧会话增量（`upsert`），返回变化后的会话视图。
    async fn next_upsert(&mut self) -> Value {
        let frame = self.connection.next_named_frame().await;
        frame.named("upsert");
        frame.json()["session"].clone()
    }

    /// 下一帧会话移除（`remove`），返回被移除的会话身份。
    async fn next_remove(&mut self) -> String {
        let frame = self.connection.next_named_frame().await;
        frame.named("remove");
        frame.json()["sessionId"]
            .as_str()
            .expect("remove 帧必须带 sessionId")
            .to_string()
    }

    /// 下一帧关机状态增量（`shutdown`），返回完整关机状态。
    async fn next_shutdown(&mut self) -> Value {
        let frame = self.connection.next_named_frame().await;
        frame.named("shutdown");
        shutdown_payload(&frame)
    }

    /// 一直读到阶段的取值等于 `phase`，返回那一帧的完整关机状态。
    async fn shutdown_until(&mut self, phase: &str) -> Value {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut seen: Vec<String> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待阶段 {phase:?} 超时；已观察：{seen:#?}"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待阶段 {phase:?} 超时；已观察：{seen:#?}");
            };
            if frame.event.as_deref() != Some("shutdown") {
                seen.push(format!("{frame:?}"));
                continue;
            }
            let view = shutdown_payload(&frame);
            if view["phase"].as_str() == Some(phase) {
                return view;
            }
            seen.push(format!("{frame:?}"));
        }
    }
}

/// 一次调用必须完成（HTTP 200）。
fn assert_ok(response: HttpResponse) {
    assert_eq!(
        response.status, 200,
        "调用必须完成：{} {}",
        response.status, response.body
    );
}

/// 响应体的 JSON。
fn response_body(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（{error}）：{}", response.body))
}

/// 一次成功的监听命令：类别是 `selected`。
fn assert_selected(response: &HttpResponse, session_id: &str) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("selected"),
        "响应体：{}",
        response.body
    );
    assert!(
        monitored_ids(&body["shutdown"]).contains(&session_id.to_string()),
        "回执必须给出命令之后的权威集合：{}",
        response.body
    );
}

/// 一次成功的撤销：类别是 `unselected`。
fn assert_unselected(response: &HttpResponse, session_id: &str) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    let body = response_body(response);
    assert_eq!(
        body["outcome"],
        json!("unselected"),
        "响应体：{}",
        response.body
    );
    assert!(
        !monitored_ids(&body["shutdown"]).contains(&session_id.to_string()),
        "回执必须给出命令之后的权威集合：{}",
        response.body
    );
}

/// 一帧里的会话列表（首帧 `snapshot` 与重发的快照用同一形状）。
fn snapshot_sessions(frame: &SseFrame) -> Vec<Value> {
    frame.json()["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("快照帧必须带 sessions 数组：{frame:?}"))
}

/// 一帧里的关机状态：首帧 `snapshot` 与增量 `shutdown` 用的是同一个字段名。
fn shutdown_payload(frame: &SseFrame) -> Value {
    let data = frame.json();
    data.get("shutdown")
        .cloned()
        .unwrap_or_else(|| panic!("帧必须带 shutdown 字段：{data}"))
}

/// 关机状态里的权威监听集合。
fn monitored_ids(view: &Value) -> Vec<String> {
    view["monitoredSessionIds"]
        .as_array()
        .unwrap_or_else(|| panic!("监听集合必须可读：{view}"))
        .iter()
        .map(|id| id.as_str().expect("集合成员是会话身份").to_string())
        .collect()
}

/// 会话列表里的身份，按帧里的顺序。
fn session_ids(sessions: &[Value]) -> Vec<String> {
    sessions
        .iter()
        .map(|view| {
            view["sessionId"]
                .as_str()
                .expect("视图必须带 sessionId")
                .to_string()
        })
        .collect()
}

/// 首帧会话列表里某个会话的视图。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("首帧里应有会话 {session_id}：{sessions:?}"))
}

/// 一直读到一帧重发的快照（落后太多时服务端丢弃增量、重发当下全量）。
///
/// 上界取洪泛的帧数：如果服务端**没有**裁决落后而是逐条重放，读者读完洪泛也等不到快照，
/// 这里会先失败——因此这条上界同时也是「有界缓冲确实丢增量」的判别依据。
async fn read_until_snapshot(connection: &mut SseConnection, flooded_frames: usize) -> SseFrame {
    for _ in 0..flooded_frames {
        let frame = connection
            .next_frame_within(Duration::from_secs(10))
            .await
            .unwrap_or_else(|| panic!("落后之后应重发快照，却一直没有下一帧"));
        if frame.event.as_deref() == Some("snapshot") {
            return frame;
        }
    }
    panic!("落后之后应重发快照，却没有等到");
}

/// 验收 P21：新观察连接的首个有效快照恰含当前登记，并带当前关机事实；重连同样以完整快照收敛。
///
/// 三类身份混合：在线登记、离线暂留登记、已删除身份。首帧必须恰含前两者、不含已删除身份，
/// 且带当前 `shutdown`；带 `Last-Event-ID` 的重连不依赖本地历史或事件重放。
#[tokio::test]
async fn the_first_valid_snapshot_carries_exactly_the_current_registry_and_shutdown() {
    let server = ObservationServer::start(3600).await;

    // 三类身份：在线登记、离线暂留登记、已删除身份。
    assert_ok(server.register("online").await);
    assert_ok(server.state("online", "working").await);
    assert_ok(server.register("retained").await);
    assert_ok(server.state("retained", "awaiting-input").await);
    assert_ok(server.register("deleted").await);
    assert_ok(server.state("deleted", "working").await);

    // t=11：超过缺省在线有效期 10 秒，三者都翻为离线暂留（登记都还在）。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    // 已删除身份：明确注销，登记不再存在。
    assert_ok(server.unregister("deleted").await);
    // 让 online 在保留期内恢复在线（t=11 仍远小于两个窗口之和 40 秒）。
    assert_ok(server.heartbeat("online").await);
    // 当前关机事实：online 是唯一被监听的会话（在线且工作中，因此不触发倒计时）。
    assert_selected(&server.monitor("online", true).await, "online");

    // 新观察连接的首个有效快照。
    let fresh = server.observe().await;
    let sessions = fresh.first_sessions();
    assert_eq!(
        session_ids(&sessions),
        vec!["online".to_string(), "retained".to_string()],
        "首帧恰含在线登记与离线暂留登记，不含已删除身份：{sessions:?}"
    );
    let online = session_of(&sessions, "online");
    assert_eq!(online["online"], json!(true), "{online}");
    assert_eq!(online["state"], json!("working"), "{online}");
    let retained = session_of(&sessions, "retained");
    assert_eq!(
        retained["online"],
        json!(false),
        "暂留登记仍被观察：{retained}"
    );
    assert_eq!(
        retained["state"],
        json!("awaiting-input"),
        "离线不改写最后上报状态：{retained}"
    );
    let shutdown = fresh.first_shutdown();
    assert_eq!(
        monitored_ids(&shutdown),
        vec!["online".to_string()],
        "{shutdown}"
    );
    assert_eq!(shutdown["phase"], json!("none"), "{shutdown}");
    assert!(
        shutdown["arrangementId"].is_null(),
        "没有安排时不带标识：{shutdown}"
    );

    // 断线期间事实继续变化：注销暂留成员、注册新会话、撤销监听。
    assert_ok(server.unregister("retained").await);
    assert_ok(server.register("fresh").await);
    assert_ok(server.state("fresh", "working").await);
    assert_unselected(&server.monitor("online", false).await, "online");

    // 带游标的重连：读路径不认游标，首帧仍是当下全量，不重放、不依赖本地历史。
    let reconnected = Observer::open(server.addr(), &[("Last-Event-ID", "4711")]).await;
    let sessions = reconnected.first_sessions();
    assert_eq!(
        session_ids(&sessions),
        vec!["online".to_string(), "fresh".to_string()],
        "重连首帧恰含断线后的当前登记：{sessions:?}"
    );
    assert_eq!(
        session_of(&sessions, "fresh")["online"],
        json!(true),
        "{sessions:?}"
    );
    let shutdown = reconnected.first_shutdown();
    assert_eq!(
        monitored_ids(&shutdown),
        Vec::<String>::new(),
        "重连首帧带当前（已清空）的监听集合：{shutdown}"
    );
    assert_eq!(shutdown["phase"], json!("none"), "{shutdown}");
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}

/// 验收 P21（活动安排）：存在活动关机安排时，新连接的首帧仍同时给出会话与当前安排。
#[tokio::test]
async fn the_first_frame_of_a_new_connection_carries_the_active_arrangement() {
    let server = ObservationServer::start(3600).await;
    let mut early = server.observe().await;

    assert_ok(server.register("m").await);
    assert_ok(server.state("m", "working").await);
    assert_selected(&server.monitor("m", true).await, "m");
    // 状态转为等待用户输入：非空全在线等待集合 → 形成唯一安排并消耗整组监听。
    assert_ok(server.state("m", "awaiting-input").await);
    let arranged = early.shutdown_until("counting-down").await;
    let arrangement_id = arranged["arrangementId"].clone();
    let deadline = arranged["deadline"].clone();
    assert!(arrangement_id.is_string(), "{arranged}");
    assert!(deadline.is_u64(), "{arranged}");
    assert_eq!(monitored_ids(&arranged), Vec::<String>::new(), "{arranged}");

    // 新连接的首帧：会话那一半照旧，关机那一半是**当前活动安排**（同一标识与截止时间）。
    let fresh = server.observe().await;
    let shutdown = fresh.first_shutdown();
    assert_eq!(shutdown["phase"], json!("counting-down"), "{shutdown}");
    assert_eq!(shutdown["arrangementId"], arrangement_id, "{shutdown}");
    assert_eq!(shutdown["deadline"], deadline, "{shutdown}");
    assert!(shutdown["serverTime"].is_u64(), "{shutdown}");
    assert_eq!(monitored_ids(&shutdown), Vec::<String>::new(), "{shutdown}");
    let session = session_of(&fresh.first_sessions(), "m");
    assert_eq!(session["state"], json!("awaiting-input"), "{session}");
    assert_eq!(session["online"], json!(true), "{session}");
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}

/// 验收 P22：打开期间，state 写入、离线维护、登记清除与监听变化都经共享观察流送达，不需刷新。
///
/// 全程沿用既有共享首帧与命名增量（`upsert` / `remove` / `shutdown`），不强制每次全量，
/// 也不出现第二次全量快照。
#[tokio::test]
async fn the_shared_stream_delivers_session_and_shutdown_changes_without_a_reload() {
    let server = ObservationServer::start(3600).await;
    let mut observer = server.observe().await;
    assert_eq!(observer.first_sessions(), Vec::<Value>::new());
    assert_eq!(observer.first_shutdown()["phase"], json!("none"));

    // state 写入：新登记与状态变化各一帧 `upsert`（增量，不是全量）。
    assert_ok(server.register("watch").await);
    let view = observer.next_upsert().await;
    assert_eq!(view["sessionId"], json!("watch"));
    assert_eq!(view["online"], json!(true));
    assert!(view.get("state").is_none(), "还没有状态：{view}");
    assert_ok(server.state("watch", "working").await);
    assert_eq!(observer.next_upsert().await["state"], json!("working"));

    // 监听变化：一帧 `shutdown` 携带完整关机状态（当前集合）。
    assert_selected(&server.monitor("watch", true).await, "watch");
    let set = observer.next_shutdown().await;
    assert_eq!(monitored_ids(&set), vec!["watch".to_string()], "{set}");
    assert_eq!(set["phase"], json!("none"), "{set}");

    // 离线维护：t=11 的维护把登记翻为离线，仍以增量 `upsert` 送达。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    let offline = observer.next_upsert().await;
    assert_eq!(offline["sessionId"], json!("watch"));
    assert_eq!(
        offline["online"],
        json!(false),
        "维护把行翻为离线：{offline}"
    );
    assert_eq!(offline["state"], json!("working"), "{offline}");

    // 监听变化：撤销离线成员 → 集合清空（仍是一帧完整关机状态）。
    assert_unselected(&server.monitor("watch", false).await, "watch");
    let cleared = observer.next_shutdown().await;
    assert_eq!(monitored_ids(&cleared), Vec::<String>::new(), "{cleared}");

    // 监听裁定：新的在线工作中登记被监听，随后进入等待用户输入 → 形成安排并消耗整组监听。
    assert_ok(server.register("other").await);
    assert_eq!(observer.next_upsert().await["sessionId"], json!("other"));
    assert_ok(server.state("other", "working").await);
    assert_eq!(observer.next_upsert().await["state"], json!("working"));
    assert_selected(&server.monitor("other", true).await, "other");
    assert_eq!(
        monitored_ids(&observer.next_shutdown().await),
        vec!["other".to_string()]
    );
    assert_ok(server.state("other", "awaiting-input").await);
    assert_eq!(
        observer.next_upsert().await["state"],
        json!("awaiting-input")
    );
    let arranged = observer.next_shutdown().await;
    assert_eq!(arranged["phase"], json!("counting-down"), "{arranged}");
    assert_eq!(
        monitored_ids(&arranged),
        Vec::<String>::new(),
        "形成安排即消耗整组监听"
    );

    // 登记清除：t=41 的维护清除 `watch`（离线已过两个窗口之和 40 秒）并把 `other` 翻为离线；
    // 活动安排不受会话变化影响（既有规则），因此这里只有会话增量。
    server.advance(Duration::from_secs(30));
    server.maintain().await;
    let flipped = observer.next_upsert().await;
    assert_eq!(flipped["sessionId"], json!("other"));
    assert_eq!(
        flipped["online"],
        json!(false),
        "维护把行翻为离线：{flipped}"
    );
    assert_eq!(
        observer.next_remove().await,
        "watch",
        "超过两个窗口之和的登记被清除"
    );

    // 全程只走首帧 + 命名增量：落后之外的路径没有第二次全量快照。
    observer
        .connection
        .assert_no_named_frame_within(Duration::from_millis(300))
        .await;
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}

/// 验收 P23：没有观察者时，写入、过期、清理与监听裁定都照常完成，新观察者取得当前结果。
#[tokio::test]
async fn writes_expiry_cleanup_and_adjudication_complete_without_any_reader() {
    let server = ObservationServer::start(3600).await;

    // 没有任何观察连接：正常调用仍要逐次完成。
    assert_ok(server.register("a").await);
    assert_ok(server.state("a", "working").await);
    assert_selected(&server.monitor("a", true).await, "a");

    // 过期与清理（无观察者）。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    server.advance(Duration::from_secs(30));
    server.maintain().await;

    // 监听裁定（无观察者）：新登记 → 监听 → 等待用户输入 → 形成安排。
    assert_ok(server.register("b").await);
    assert_ok(server.state("b", "working").await);
    assert_selected(&server.monitor("b", true).await, "b");
    assert_ok(server.state("b", "awaiting-input").await);

    // 新观察者取得当前结果：首帧同时给出会话与关机事实（已清除的 `a` 不在其中）。
    let fresh = server.observe().await;
    let sessions = fresh.first_sessions();
    assert_eq!(
        session_ids(&sessions),
        vec!["b".to_string()],
        "{sessions:?}"
    );
    assert_eq!(session_of(&sessions, "b")["state"], json!("awaiting-input"));
    let shutdown = fresh.first_shutdown();
    assert_eq!(shutdown["phase"], json!("counting-down"), "{shutdown}");
    assert_eq!(monitored_ids(&shutdown), Vec::<String>::new(), "{shutdown}");
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}

/// 验收 P23：受控阻塞读者不会阻塞维护与裁定；原落后观察者按有界缓冲 + 快照恢复当前事实。
#[tokio::test]
async fn a_blocked_reader_never_blocks_writes_and_converges_via_the_bounded_buffer() {
    let server = ObservationServer::start(3600).await;

    // 每帧都很大：读者不读时服务端会堵在 socket 缓冲上，订阅者的有界缓冲（64 帧）必然溢出。
    let long_cwd = format!(r"C:\work\{}", "x".repeat(4000));
    assert_ok(server.register_in("big", &long_cwd).await);
    assert_ok(server.state("big", "working").await);

    let mut blocked = server.connect().await;
    let first = blocked.next_frame().await;
    first.named("snapshot");
    assert_eq!(snapshot_sessions(&first).len(), 1);

    // 从这里起不再读这条连接：驱动写入（300 次真实可见的状态变化）仍必须逐次完成。
    const FLOOD: usize = 300;
    for round in 0..FLOOD {
        let value = if round % 2 == 0 {
            "awaiting-input"
        } else {
            "working"
        };
        assert_ok(server.state("big", value).await);
    }

    // 监听裁定、过期与清理也在读者阻塞时完成。
    assert_selected(&server.monitor("big", true).await, "big");
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    server.advance(Duration::from_secs(30));
    server.maintain().await;
    assert_ok(server.register("z").await);
    assert_ok(server.state("z", "working").await);

    // 原落后观察者恢复：读到的快照就是当下（`big` 已被清除、`z` 存在、集合已清空）。
    let recovered = read_until_snapshot(&mut blocked, FLOOD).await;
    let sessions = snapshot_sessions(&recovered);
    assert_eq!(
        session_ids(&sessions),
        vec!["z".to_string()],
        "落后的读者必须收敛到当下，而不是追赶历史：{sessions:?}"
    );
    let shutdown = shutdown_payload(&recovered);
    assert_eq!(monitored_ids(&shutdown), Vec::<String>::new(), "{shutdown}");
    assert_eq!(shutdown["phase"], json!("none"), "{shutdown}");

    // 新观察者取得同一份当前事实。
    let fresh = server.observe().await;
    assert_eq!(session_ids(&fresh.first_sessions()), vec!["z".to_string()]);
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");

    // 手动开始意图同样不依赖任何读者（本票不驱动到期执行）。
    let started = server.start_shutdown().await;
    assert_eq!(started.status, 200, "{} {}", started.status, started.body);
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}

/// 验收 P48（S1 支撑证据）：两个页面各自的观察连接收敛到同一实例的会话与关机事实。
///
/// 命令回执只回到发起命令的那一次调用；另一条连接没有收到任何回执，只靠自己那条观察流收敛。
#[tokio::test]
async fn independent_readers_converge_on_the_same_instance_facts() {
    let server = ObservationServer::start(3600).await;
    let mut page_one = server.observe().await;
    let mut page_two = server.observe().await;

    assert_ok(server.register("a").await);
    assert_ok(server.state("a", "working").await);
    for observer in [&mut page_one, &mut page_two] {
        assert_eq!(observer.next_upsert().await["sessionId"], json!("a"));
        assert_eq!(observer.next_upsert().await["state"], json!("working"));
    }

    // 一页发出监听命令：回执只回到这一页；另一页没有命令回执，只靠自己的观察连接收敛。
    assert_selected(&server.monitor("a", true).await, "a");
    for observer in [&mut page_one, &mut page_two] {
        assert_eq!(
            monitored_ids(&observer.next_shutdown().await),
            vec!["a".to_string()]
        );
    }

    // 离线维护反映于两页。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    for observer in [&mut page_one, &mut page_two] {
        let view = observer.next_upsert().await;
        assert_eq!(view["sessionId"], json!("a"));
        assert_eq!(view["online"], json!(false), "{view}");
    }

    // 登记清除反映于两页：移除登记，并把不再存在的成员移出监听集合。
    server.advance(Duration::from_secs(30));
    server.maintain().await;
    for observer in [&mut page_one, &mut page_two] {
        assert_eq!(observer.next_remove().await, "a");
        assert_eq!(
            monitored_ids(&observer.next_shutdown().await),
            Vec::<String>::new(),
            "清除登记后集合同步移出"
        );
    }
    assert_eq!(server.double.calls(), 0, "安全 adapter 不得被调用");
}
