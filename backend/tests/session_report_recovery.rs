//! 有效上报刷新与删除后的既有恢复协议（契约 P12–P17、P50）。
//!
//! 走交付运行时同一份装配：会话事件经**真实 TCP** 写入、登记与关机状态经**真实 SSE** 读取（页面收到的
//! 就是这些帧），时刻来自受控时钟（[`ManualClock`]），因此窗口边界是精确的 t=20／30／31／40／41／60／61，
//! 期望值全部取自缺省策略的独立字面值（在线有效期 10 秒、离线保留期 30 秒），不由实现推算。
//!
//! 覆盖：
//! - P12：`register`／`heartbeat`／`state` **分别**重新开始同一条身份登记的有效期（t=30 在线、t=31 离线、
//!   t=60 维护后仍保留、t=61 维护后清除），且刷新的是同一行、不新起登记。
//! - P13：t=20 提交非法输入与来源不被接受的写入，被拒绝的写入不延长有效期、不改变 agent 状态与监听集合；
//!   原登记仍按 t=40／41 边界由**维护**清除（后来的正常移除与拒绝副作用分开断言）。
//! - P14：t=41、清理尚未执行时先接受迟到 `heartbeat`，随后维护保留登记并按新起点重新计时；
//!   另有一例真并发交错的不变量用例，断言「被接受的刷新不会因陈旧的清理判断被误删」。
//! - P15：维护先删除后，`heartbeat` 与 `state` 都得到精确 `404 unknown-session`，且不隐式建行；
//!   从未登记身份同一规则。
//! - P16：在线阶段与离线暂留阶段分别 `unregister`，回执完成后读取无登记、观察流给出移除，不等待窗口结束。
//! - P17：已监听身份被维护清除后，以**同一** `sessionId` `register` 重建登记；登记存在，监听集合不含它。
//! - P50：整套流程沿用扩展上报的同一外部事件形状（`{sessionId, kind, data}`），只经 `register` 建行。
//!
//! 来源拒绝那一条对端地址本机不可能真实拥有（回环与网卡地址都算本机来源），因此它与既有
//! `session_write_boundary.rs` 用同一条受控对端地址接缝（`MockConnectInfo`，正是 `is_local_source` 读的那个
//! 输入）；其余每条断言（接受、刷新、404、移除）都走真实 TCP。所有用例都注入记录型关机动作，**绝不**接触
//! 任何系统关机路径。

mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp};
use herdr_bot::clock::ManualClock;
use herdr_bot::shutdown::ShutdownState;
use herdr_bot::sessions::{SessionPolicy, SessionsState};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use tower::ServiceExt;

/// 会话事件的唯一写入路径。
const EVENTS: &str = "/api/sessions/events";
/// 监听命令的路径。
const MONITOR: &str = "/api/shutdown/monitor";
/// 代表局域网设备的来源地址：RFC 5737 的 TEST-NET-3 文档地址，本机不可能拥有它。
const LAN_SOURCE: &str = "203.0.113.7:40000";

/// 记录型关机动作：只数调用次数，**绝不**接触系统关机。
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

/// 受控时刻下的服务：会话表与关机状态由用例持有，因此「那一遍维护」由用例自己驱动；
/// 写入与观察分别走真实 TCP 与真实 SSE。
struct ReportServer {
    server: TestServer,
    /// 未经真 socket 的应用副本：只用于受控对端地址这一条拒绝证据。
    app: Router,
    sessions: Arc<SessionsState>,
    shutdown: Arc<ShutdownState>,
    clock: Arc<ManualClock>,
    double: Arc<RecordingPowerOff>,
}

impl ReportServer {
    /// 缺省策略（10／30 秒）＋受控时刻的实例。
    async fn start() -> Self {
        let clock = Arc::new(ManualClock::new());
        let config = common::config(Path::new("unused.bat"), Path::new("."));
        let sessions = Arc::new(SessionsState::with_policy_and_clock(
            SessionPolicy::default(),
            clock.clone(),
        ));
        let double = RecordingPowerOff::new();
        let (app, state) =
            herdr_bot::app_with_state(&config, double.clone(), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app.clone()).await,
            app,
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

    fn addr(&self) -> SocketAddr {
        self.server.addr()
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&self, body: &str) -> HttpResponse {
        post_json_over_tcp(self.addr(), EVENTS, body).await
    }

    /// 一条 `register`（扩展会上报的字段形状）。
    async fn register(&self, session_id: &str) -> HttpResponse {
        self.post(&register_envelope(session_id)).await
    }

    /// 一条不带状态的 `heartbeat`。
    async fn heartbeat(&self, session_id: &str) -> HttpResponse {
        self.post(&heartbeat_envelope(session_id)).await
    }

    /// 一条 `state`。
    async fn state(&self, session_id: &str, value: &str) -> HttpResponse {
        self.post(&state_envelope(session_id, value)).await
    }

    /// 一条 `unregister`。
    async fn unregister(&self, session_id: &str) -> HttpResponse {
        self.post(&unregister_envelope(session_id)).await
    }

    /// 提交一次监听命令：明确的目标选中值。
    async fn monitor(&self, session_id: &str, selected: bool) -> HttpResponse {
        post_json_over_tcp(
            self.addr(),
            MONITOR,
            &json!({ "sessionId": session_id, "selected": selected }).to_string(),
        )
        .await
    }

    /// 跑一遍维护：过期行翻为离线、超过两个窗口的行清除，随后重判监听集合。
    ///
    /// 与 `spawn_expiry_scan` 每 5 秒跑的那一对调用完全相同。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    /// 经**受控对端地址**提交一条会话事件：来源不是本机，代表局域网设备。
    ///
    /// 用 axum 自带的 `MockConnectInfo`（它只往请求里放 `ConnectInfo`），校验读到的来源与真 TCP 情形
    /// 是同一个东西。
    async fn post_from_lan(&self, body: &str) -> (StatusCode, Value) {
        let app = self.app.clone().layer(MockConnectInfo(
            LAN_SOURCE.parse::<SocketAddr>().expect("受控来源地址"),
        ));
        let request = Request::builder()
            .method("POST")
            .uri(EVENTS)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("构造请求");
        let response = app.oneshot(request).await.expect("请求应用");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("读取响应体");
        let body: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            panic!(
                "拒绝响应必须是 JSON（{e}）：{}",
                String::from_utf8_lossy(&bytes)
            )
        });
        (status, body)
    }

    /// 建一条观察连接并读掉首帧。
    async fn observe(&self) -> Observer {
        Observer::connect(self.addr()).await
    }

    /// 从一条新的观察连接首帧读取当前会话列表（页面同一条读取路径）。
    async fn read_sessions(&self) -> Vec<Value> {
        self.observe().await.first_sessions()
    }

    /// 从一条新的观察连接首帧读取权威监听集合（页面同一条读取路径）。
    async fn monitored_set(&self) -> Vec<String> {
        monitored_ids(&self.observe().await.first_shutdown())
    }
}

/// 一条已经读过首帧 `snapshot` 的观察连接。
struct Observer {
    connection: SseConnection,
    first: SseFrame,
}

impl Observer {
    async fn connect(addr: SocketAddr) -> Self {
        let mut connection = SseConnection::open(addr, EVENTS, &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        Self { connection, first }
    }

    fn first_sessions(&self) -> Vec<Value> {
        self.first.json()["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn first_shutdown(&self) -> Value {
        shutdown_payload(&self.first)
    }

    /// 一直读到某会话被移除的那一帧；超时即用例失败。
    async fn session_removed_within(&mut self, session_id: &str, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(!remaining.is_zero(), "等待 {session_id} 的移除帧超时");
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待 {session_id} 的移除帧超时");
            };
            if frame.event.as_deref() == Some("remove")
                && frame.json()["sessionId"] == json!(session_id)
            {
                return;
            }
        }
    }

    /// 一直读到某会话的 `upsert` 帧且 `online` 等于 `expected`；超时即用例失败。
    async fn session_upsert_within(
        &mut self,
        session_id: &str,
        expected: bool,
        timeout: Duration,
    ) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut seen: Vec<String> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "等待 {session_id} 的 upsert(online={expected}) 超时；已收到：{seen:#?}"
            );
            let Some(frame) = self.connection.next_frame_within(remaining).await else {
                panic!("等待 {session_id} 的 upsert(online={expected}) 超时；已收到：{seen:#?}");
            };
            if frame.event.as_deref() == Some("upsert") {
                let view = frame.json()["session"].clone();
                if view["sessionId"] == json!(session_id) && view["online"] == json!(expected) {
                    return view;
                }
            }
            seen.push(format!("{frame:?}"));
        }
    }

    /// 在给定时间内断言没有命名事件到达。
    async fn assert_no_named_event_within(&mut self, timeout: Duration) {
        self.connection.assert_no_named_frame_within(timeout).await;
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

/// 首帧会话列表里的某个会话；缺了就是用例失败（并说明「登记本该存在」）。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("会话列表里应有 {session_id}：{sessions:?}"))
}

/// 扩展上报的同一外部形状：`{sessionId, kind, data}`，`register` 的 `data` 只含扩展真正会上报的字段。
fn register_envelope(session_id: &str) -> String {
    json!({
        "sessionId": session_id,
        "kind": "register",
        "data": { "cwd": "C:\\work\\agent", "sessionName": "agent", "model": "demo" },
    })
    .to_string()
}

/// 心跳：扩展不带状态时 `data` 为 `{}`。
fn heartbeat_envelope(session_id: &str) -> String {
    json!({ "sessionId": session_id, "kind": "heartbeat", "data": {} }).to_string()
}

/// 状态事件：扩展会多带一个 `reason` 字段，服务端按「未知字段忽略」处理。
fn state_envelope(session_id: &str, value: &str) -> String {
    json!({ "sessionId": session_id, "kind": "state", "data": { "value": value, "reason": "edge" } })
        .to_string()
}

fn unregister_envelope(session_id: &str) -> String {
    json!({ "sessionId": session_id, "kind": "unregister" }).to_string()
}

/// 断言一次成功回执（HTTP 200）。
fn assert_accepted(response: &HttpResponse) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
}

/// 断言一次精确拒绝：HTTP 状态码 + `{code, message}` 形状。
fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body: Value = serde_json::from_str(&response.body)
        .unwrap_or_else(|e| panic!("拒绝响应必须是 JSON（{e}）：{}", response.body));
    assert_eq!(body["code"], code, "响应体：{}", response.body);
}

/// 验收 P12：`register`、`heartbeat`、`state` 三类被接受的写入**各自**重新开始同一条登记的有效期。
///
/// 期望值取自缺省策略的独立字面值：t=20 写入被接受后，t=30（年龄 10）在线、t=31（年龄 11）离线、
/// t=60（年龄 40）维护后仍保留、t=61（年龄 41）维护后清除。
async fn assert_kind_refreshes(kind: &str) {
    let session_id = format!("sess-{kind}");
    let server = ReportServer::start().await;
    assert_accepted(&server.register(&session_id).await);
    let mut observer = server.observe().await;

    // 写入前：只有这一条登记。
    assert_eq!(server.read_sessions().await.len(), 1);

    // t=20：提交一条被接受的写入，重新开始有效期。
    server.advance(Duration::from_secs(20));
    let write = match kind {
        "register" => server.register(&session_id).await,
        "heartbeat" => server.heartbeat(&session_id).await,
        "state" => server.state(&session_id, "working").await,
        other => panic!("未知的 kind：{other}"),
    };
    assert_accepted(&write);
    // 刷新的是同一身份登记：仍然只有一行，且就是同一个 sessionId。
    let refreshed = server.read_sessions().await;
    assert_eq!(refreshed.len(), 1, "{kind} 刷新不得新起一行：{refreshed:?}");
    assert_eq!(
        session_of(&refreshed, &session_id)["sessionId"],
        json!(session_id)
    );

    // t=30：年龄恰好等于在线有效期，仍在线。
    server.advance(Duration::from_secs(10));
    assert_eq!(
        session_of(&server.read_sessions().await, &session_id)["online"],
        json!(true),
        "{kind} 之后 t=30 必须在线"
    );

    // t=31：超过在线有效期一秒，读取为离线（登记仍在）。
    server.advance(Duration::from_secs(1));
    let offline = server.read_sessions().await;
    assert_eq!(
        session_of(&offline, &session_id)["online"],
        json!(false),
        "{kind} 之后 t=31 必须离线"
    );

    // t=60：年龄恰好等于两个窗口之和，维护后登记仍保留（离线）。
    server.advance(Duration::from_secs(29));
    server.maintain().await;
    let retained = server.read_sessions().await;
    assert_eq!(
        session_of(&retained, &session_id)["online"],
        json!(false),
        "{kind} 之后 t=60 维护后应仍保留且离线：{retained:?}"
    );

    // t=61：超过两个窗口之和一秒，维护清除并向观察者发布移除。
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    observer
        .session_removed_within(&session_id, Duration::from_secs(5))
        .await;
    assert!(
        server.read_sessions().await.is_empty(),
        "{kind} 之后 t=61 维护后必须清除"
    );
}

/// 验收 P12：三类事件分别重新开始同一身份登记的有效期（循环体见 [`assert_kind_refreshes`]）。
#[tokio::test]
async fn every_accepted_report_kind_restarts_the_same_registration() {
    for kind in ["register", "heartbeat", "state"] {
        assert_kind_refreshes(kind).await;
    }
}

/// 验收 P13：被拒绝的写入不延长既有登记的有效期，也不改变它的业务状态或监听集合。
///
/// t=20 提交（1）非法输入（真实 TCP）与（2）来源不被接受的写入（受控对端地址）；两者都不产生副作用，
/// 原登记仍按 t=40／41 边界由**维护**清除。后来的正常移除与「拒绝副作用」分开断言：拒绝之后立刻读取，
/// 状态与集合原样；t=41 的移出是维护这一遍的结果。
#[tokio::test]
async fn a_rejected_report_never_extends_a_registration() {
    let server = ReportServer::start().await;
    assert_accepted(&server.register("sess-rejected").await);
    assert_accepted(&server.state("sess-rejected", "working").await);
    assert_accepted(&server.monitor("sess-rejected", true).await);
    let mut observer = server.observe().await;
    assert_eq!(
        server.monitored_set().await,
        vec!["sess-rejected".to_string()],
        "监听集合在拒绝之前应含该会话"
    );

    // t=20：先记下原登记视图。此刻已超过在线有效期，所以它本来就离线、最后上报状态未变。
    server.advance(Duration::from_secs(20));
    let before = server.read_sessions().await;
    let original = session_of(&before, "sess-rejected");
    assert_eq!(
        original["online"],
        json!(false),
        "t=20 已经超过在线有效期，本来就离线：{original}"
    );
    assert_eq!(original["state"], json!("working"), "{original}");

    // 非法输入（状态值不在三态里）经真实 TCP 被拒。
    let illegal = server
        .post(&state_envelope("sess-rejected", "bogus-state"))
        .await;
    assert_rejected(&illegal, 400, "invalid-event");

    // 同一时刻：来源不被接受的写入（局域网对端地址）被拒。
    let (status, body) = server
        .post_from_lan(&heartbeat_envelope("sess-rejected"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "拒绝响应：{body}");
    assert_eq!(body["code"], json!("forbidden"));

    // 拒绝没有副作用：登记视图与拒绝之前逐字段一致（状态与存活性都没有被推动）、
    // 监听集合不变、没有关机调用，观察流也没有帧。
    let after = server.read_sessions().await;
    assert_eq!(after, before, "被拒的写入不得改动登记视图");
    assert_eq!(
        server.monitored_set().await,
        vec!["sess-rejected".to_string()],
        "被拒的写入不得改动监听集合"
    );
    assert_eq!(server.double.calls(), 0, "被拒的写入不得触发关机调用");
    observer
        .assert_no_named_event_within(Duration::from_millis(300))
        .await;

    // 原登记仍按 t=40／41 边界清除：被拒的写入没有把它推到窗口之外。
    server.advance(Duration::from_secs(20)); // t=40
    server.maintain().await;
    let offline = observer
        .session_upsert_within("sess-rejected", false, Duration::from_secs(5))
        .await;
    assert_eq!(offline["online"], json!(false), "t=40 应为离线：{offline}");
    assert_eq!(
        server.read_sessions().await.len(),
        1,
        "恰好等于两个窗口之和仍保留登记"
    );

    server.advance(Duration::from_secs(1)); // t=41
    server.maintain().await;
    observer
        .session_removed_within("sess-rejected", Duration::from_secs(5))
        .await;
    assert!(
        server.read_sessions().await.is_empty(),
        "超过两个窗口之和一秒后由维护清除"
    );
    assert_eq!(server.double.calls(), 0, "全程没有形成安排");
}

/// 验收 P14：清理阈值已过但登记实际尚未删除时，先被接受的普通上报仍能刷新该登记。
///
/// t=41（年龄 41 > 40）先提交 `heartbeat`：**不能**按绝对年龄造一个硬期限来拒绝它。随后维护这一遍必须
/// 保留登记并按新起点重新计时，而不是拿旧截止时间把它删掉；新窗口同样按独立字面值验证（刷新后
/// 年龄 40 仍保留、年龄 41 才清除）。
#[tokio::test]
async fn a_late_report_before_cleanup_refreshes_and_the_scan_keeps_it() {
    let server = ReportServer::start().await;
    assert_accepted(&server.register("sess-late").await);
    assert_accepted(&server.state("sess-late", "working").await);
    let mut observer = server.observe().await;

    // t=41：超过两个窗口之和，但维护还没有执行过——登记仍然存在。
    server.advance(Duration::from_secs(41));
    let late = server.heartbeat("sess-late").await;
    assert_accepted(&late);

    // 迟到的普通上报刷新了这条登记：读取为在线。
    let refreshed = server.read_sessions().await;
    assert_eq!(
        session_of(&refreshed, "sess-late")["online"],
        json!(true),
        "迟到的被接受上报必须重新开始有效期：{refreshed:?}"
    );
    let upsert = observer
        .session_upsert_within("sess-late", true, Duration::from_secs(5))
        .await;
    assert_eq!(upsert["sessionId"], json!("sess-late"));

    // 刷新之后立刻维护：不能用旧截止时间误删。
    server.maintain().await;
    observer
        .assert_no_named_event_within(Duration::from_millis(300))
        .await;
    let kept = server.read_sessions().await;
    assert_eq!(kept.len(), 1, "刷新之后这一遍维护必须保留登记：{kept:?}");
    assert_eq!(session_of(&kept, "sess-late")["online"], json!(true));

    // 再扫一遍仍然保留：旧的过期判断不能留到下一遍。
    server.maintain().await;
    assert_eq!(server.read_sessions().await.len(), 1);

    // 重新计时：从 t=41 的这次上报起算，年龄 40（t=81）仍保留，年龄 41（t=82）才清除。
    server.advance(Duration::from_secs(40));
    server.maintain().await;
    let at_expiry = server.read_sessions().await;
    assert_eq!(
        at_expiry.len(),
        1,
        "刷新后年龄恰好等于两个窗口之和仍保留：{at_expiry:?}"
    );
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    observer
        .session_removed_within("sess-late", Duration::from_secs(5))
        .await;
    assert!(
        server.read_sessions().await.is_empty(),
        "刷新后按新起点清除"
    );
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P14（真并发交错）：一遍维护与一条迟到上报同时发生，两种次序都必须自洽。
///
/// **两种次序各自的确定证据**是相邻的两个用例——上一条（先接受刷新，再维护）与 P15（维护先删除，再上报）。
/// 本用例补的是真并发下必须成立的不变量：维护先拿到决定权 → 登记被删除，随后那条上报得到 `404`；
/// 上报先被接受 → 登记必须还在且重新在线，维护不得再用它开始前读到的过期判断把它删掉。
/// 「先据旧数据决定要删、再去删除」的实现会在第二种次序下产生「上报被接受（200）而登记却不见了」，
/// 因此这条不变量正是这类陈旧清理判断的检出条件。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_report_racing_the_scan_never_loses_an_accepted_refresh() {
    for _ in 0..20 {
        let server = ReportServer::start().await;
        assert_accepted(&server.register("sess-race").await);

        // t=41：维护尚未执行，登记已超过两个窗口之和。
        server.advance(Duration::from_secs(41));

        // 同一瞬间：生产同一对维护调用，与一条迟到的普通上报。
        let scan = {
            let sessions = Arc::clone(&server.sessions);
            let shutdown = Arc::clone(&server.shutdown);
            tokio::spawn(async move {
                sessions.scan_expired().await;
                shutdown.sessions_changed().await;
            })
        };
        let report = server.heartbeat("sess-race");
        let (scanned, report) = tokio::join!(scan, report);
        scanned.expect("维护任务不应 panic");

        let sessions = server.read_sessions().await;
        if report.status == 200 {
            assert_eq!(
                sessions.len(),
                1,
                "上报被接受（200）时维护不得把它删掉：{}",
                report.body
            );
            assert_eq!(
                session_of(&sessions, "sess-race")["online"],
                json!(true),
                "被接受的迟到上报必须重新在线：{sessions:?}"
            );
        } else {
            assert_rejected(&report, 404, "unknown-session");
            assert!(
                sessions.is_empty(),
                "维护先删除时不得留下登记：{sessions:?}"
            );
        }
    }
}

/// 验收 P15：登记被维护实际删除后，`heartbeat` 与 `state` 都返回精确 `404 unknown-session`，随后读取仍
/// 不存在登记；从未登记身份保持同一拒绝规则，不按普通上报隐式建行。
#[tokio::test]
async fn a_report_after_deletion_is_rejected_and_never_rebuilds() {
    let server = ReportServer::start().await;
    assert_accepted(&server.register("sess-gone").await);
    assert_accepted(&server.state("sess-gone", "working").await);
    let mut observer = server.observe().await;

    // 维护先完成删除。
    server.advance(Duration::from_secs(41));
    server.maintain().await;
    observer
        .session_removed_within("sess-gone", Duration::from_secs(5))
        .await;
    assert!(server.read_sessions().await.is_empty(), "维护已删除登记");

    // 删除之后：两类普通上报都被精确拒绝，且不隐式建行。
    assert_rejected(&server.heartbeat("sess-gone").await, 404, "unknown-session");
    assert_rejected(
        &server.state("sess-gone", "working").await,
        404,
        "unknown-session",
    );
    assert!(
        server.read_sessions().await.is_empty(),
        "被拒的普通上报不得重建登记"
    );

    // 从未登记的身份：同一拒绝规则。
    assert_rejected(
        &server.heartbeat("sess-never").await,
        404,
        "unknown-session",
    );
    assert_rejected(
        &server.state("sess-never", "idle").await,
        404,
        "unknown-session",
    );
    assert!(server.read_sessions().await.is_empty());
}

/// 验收 P16：明确 `unregister` 立即删除登记，在线阶段与离线暂留阶段都不等窗口结束。
///
/// 两个阶段各自：回执为 200 `accepted`，随后读取无该登记，观察流给出该身份的移除帧——期间时刻**没有**
/// 推进，因此删除不是窗口到期的结果。
#[tokio::test]
async fn unregister_removes_immediately_online_and_while_retained() {
    let server = ReportServer::start().await;
    let mut observer = server.observe().await;

    // 在线阶段：注册后立刻注销。
    assert_accepted(&server.register("sess-online").await);
    let receipt = server.unregister("sess-online").await;
    assert_accepted(&receipt);
    assert_eq!(
        serde_json::from_str::<Value>(&receipt.body).expect("回执是 JSON")["outcome"],
        json!("accepted")
    );
    observer
        .session_removed_within("sess-online", Duration::from_secs(5))
        .await;
    assert!(
        server.read_sessions().await.is_empty(),
        "在线阶段的注销立即删除登记，不等窗口"
    );

    // 离线暂留阶段：先让它过期（维护发布离线），再注销。
    assert_accepted(&server.register("sess-retained").await);
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    observer
        .session_upsert_within("sess-retained", false, Duration::from_secs(5))
        .await;
    let retained = server.read_sessions().await;
    assert_eq!(
        session_of(&retained, "sess-retained")["online"],
        json!(false),
        "此时是离线暂留：{retained:?}"
    );

    assert_accepted(&server.unregister("sess-retained").await);
    observer
        .session_removed_within("sess-retained", Duration::from_secs(5))
        .await;
    assert!(
        server.read_sessions().await.is_empty(),
        "离线暂留阶段的注销同样立即删除，不等保留期结束"
    );
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P17：已监听身份被维护清除后，以**同一** `sessionId` 重新 `register` 能重建登记，但监听集合
/// 不含该身份——清除就移出，重建不复活旧监听，也不需要更换身份。
#[tokio::test]
async fn re_registering_a_cleared_monitored_identity_restores_only_the_registration() {
    let server = ReportServer::start().await;
    assert_accepted(&server.register("sess-mon").await);
    assert_accepted(&server.state("sess-mon", "working").await);
    assert_accepted(&server.monitor("sess-mon", true).await);
    assert_eq!(
        server.monitored_set().await,
        vec!["sess-mon".to_string()],
        "清除之前它是被监听成员"
    );

    // 维护清除该身份：登记消失，成员同步移出，没有形成任何安排。
    server.advance(Duration::from_secs(41));
    server.maintain().await;
    assert!(server.read_sessions().await.is_empty(), "登记已被清除");
    assert!(
        server.monitored_set().await.is_empty(),
        "被清除的成员必须同步移出监听集合"
    );
    assert_eq!(server.double.calls(), 0);

    // 同一 sessionId 重新注册：登记重建（在线），监听集合仍然不含它。
    assert_accepted(&server.register("sess-mon").await);
    let rebuilt = server.read_sessions().await;
    assert_eq!(rebuilt.len(), 1, "register 重建登记：{rebuilt:?}");
    let row = session_of(&rebuilt, "sess-mon");
    assert_eq!(row["sessionId"], json!("sess-mon"), "用的是同一身份");
    assert_eq!(row["online"], json!(true), "重建的登记在线：{row}");
    assert!(
        server.monitored_set().await.is_empty(),
        "重建登记不自动恢复被移出的监听"
    );

    // 之后的状态变化也不把它加回监听集合（没有隐式重建监听意图的路径）。
    assert_accepted(&server.state("sess-mon", "working").await);
    assert!(server.monitored_set().await.is_empty());
    assert_eq!(server.double.calls(), 0);
}

/// 验收 P50：整套「暂留刷新 → 未知登记拒绝 → 重新 register」沿用**同一**外部事件形状。
///
/// 请求体就是扩展上报的 `{sessionId, kind, data}`（`state` 还多带扩展会发的 `reason` 字段）；
/// 建行的唯一入口是 `register`，服务端不代扩展重试、不为普通上报自动建行。
#[tokio::test]
async fn the_extension_event_shape_refreshes_rejects_and_reregisters() {
    let server = ReportServer::start().await;

    // 同一形状的 register 建立登记。
    assert_accepted(&server.register("ext-1").await);

    // 暂留刷新：进入离线后，同一形状的 heartbeat 仍刷新并恢复在线。
    server.advance(Duration::from_secs(11));
    server.maintain().await;
    assert_eq!(
        session_of(&server.read_sessions().await, "ext-1")["online"],
        json!(false),
        "超过在线有效期后是离线暂留"
    );
    assert_accepted(&server.heartbeat("ext-1").await);
    let refreshed = server.read_sessions().await;
    let row = session_of(&refreshed, "ext-1");
    assert_eq!(row["online"], json!(true), "暂留期内的上报刷新登记：{row}");

    // 维护清除之后，同一形状的 heartbeat 与 state 都是精确 404，且不自动建行。
    server.advance(Duration::from_secs(41));
    server.maintain().await;
    assert!(server.read_sessions().await.is_empty(), "维护已清除登记");
    assert_rejected(&server.heartbeat("ext-1").await, 404, "unknown-session");
    assert_rejected(&server.state("ext-1", "idle").await, 404, "unknown-session");
    assert!(
        server.read_sessions().await.is_empty(),
        "普通上报不得隐式建行（服务端不代扩展重试）"
    );

    // 同一形状的 register 重新建立登记。
    assert_accepted(&server.register("ext-1").await);
    let reregistered = server.read_sessions().await;
    assert_eq!(
        reregistered.len(),
        1,
        "重新 register 建立登记：{reregistered:?}"
    );
    assert_eq!(session_of(&reregistered, "ext-1")["online"], json!(true));
}
