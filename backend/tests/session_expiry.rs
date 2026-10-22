//! 会话登记的两阶段生命周期：在线有效期 + 离线保留期（契约 P8–P11、P18、P20）。
//!
//! 走交付运行时同一份装配：会话事件经真实 TCP 写入，登记经真实 SSE 读取（页面收到的就是这些帧），
//! 时刻来自受控时钟（[`ManualClock`]）。因此窗口边界是精确的 t=10 / t=11 / t=40 / t=41，
//! 期望值全部是独立字面值（默认策略：在线 10 秒、离线保留 30 秒），不由实现推算。
//!
//! 覆盖：边界内含在在线一侧、超过在线有效期但仍在保留期内离线且保留最后上报的 agent 状态、
//! 恰好等于两个窗口之和仍保留、超过两者之和由维护清除并发布移除、重复维护不复活、
//! 任何被接受的写入都重新计时、`unregister` 立即删除且重启即清空、清除不碰隔离的对话文件、
//! 以及交付运行时那份真实每 5 秒维护循环在默认策略下的离线与清除发布上界。

mod common;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::sse::SseConnection;
use common::{HttpResponse, TempDir, TestServer, post_json_over_tcp};
use herdr_bot::PowerOff;
use herdr_bot::PowerOffFailure;
use herdr_bot::clock::ManualClock;
use herdr_bot::countdown::ShutdownState;
use herdr_bot::sessions::{EXPIRY_SCAN_INTERVAL, SessionPolicy, SessionsState, spawn_expiry_scan};
use serde_json::{Value, json};

/// 会话维护用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        panic!("会话生命周期用例不应触发关机");
    }
}

/// 受控时刻下的服务：会话表由用例持有，因此「那一遍维护」由用例自己驱动，观察走真 TCP／SSE。
struct LifecycleServer {
    server: TestServer,
    sessions: Arc<SessionsState>,
    shutdown: Arc<ShutdownState>,
    clock: Arc<ManualClock>,
}

impl LifecycleServer {
    /// 默认策略（10／30 秒）＋受控时刻的实例。
    async fn start() -> Self {
        let clock = Arc::new(ManualClock::new());
        let config = common::config(Path::new("unused.bat"), Path::new("."));
        let sessions = Arc::new(SessionsState::with_policy_and_clock(
            SessionPolicy::default(),
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
    ///
    /// 与 `spawn_expiry_scan` 每 [`EXPIRY_SCAN_INTERVAL`] 跑的那一遍是同一对调用。
    async fn maintain(&self) {
        self.sessions.scan_expired().await;
        self.shutdown.sessions_changed().await;
    }

    /// 建一条观察连接并读掉首帧。
    async fn observe(&self) -> SseConnection {
        let mut connection =
            SseConnection::open(self.server.addr(), "/api/sessions/events", &[]).await;
        let first = connection.next_frame().await;
        first.named("snapshot");
        connection
    }

    /// 从一条新的观察连接首帧读取当前会话列表（页面同一条读取路径）。
    async fn read_sessions(&self) -> Vec<Value> {
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

/// 断言一次成功回执（HTTP 200）。
fn assert_accepted(response: &HttpResponse) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
}

/// 首帧会话列表里的某个会话；缺了就是用例失败。
fn session_of(sessions: &[Value], session_id: &str) -> Value {
    sessions
        .iter()
        .find(|session| session["sessionId"] == json!(session_id))
        .cloned()
        .unwrap_or_else(|| panic!("会话列表里应有 {session_id}：{sessions:?}"))
}

fn register(session_id: &str, session_file: Option<&str>) -> String {
    let mut data = serde_json::Map::new();
    data.insert("cwd".to_string(), json!("C:/work/agent"));
    if let Some(session_file) = session_file {
        data.insert("sessionFile".to_string(), json!(session_file));
    }
    json!({ "sessionId": session_id, "kind": "register", "data": data }).to_string()
}

fn heartbeat(session_id: &str) -> String {
    json!({ "sessionId": session_id, "kind": "heartbeat", "data": {} }).to_string()
}

fn state(session_id: &str, value: &str) -> String {
    json!({ "sessionId": session_id, "kind": "state", "data": { "value": value } }).to_string()
}

fn unregister(session_id: &str) -> String {
    json!({ "sessionId": session_id, "kind": "unregister" }).to_string()
}

/// 读到指定的命名事件为止（跳过 keep-alive 注释帧）；超时即用例失败。
///
/// 慢用例里两个窗口串起来约 45 秒，期间必然出现 keep-alive，因此不能只取「下一帧」。
async fn next_named_event_within(
    connection: &mut SseConnection,
    expected: &str,
    timeout: Duration,
) -> Value {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let Some(frame) = connection.next_frame_within(remaining).await else {
            panic!("等待 {expected} 帧超时（{timeout:?}）");
        };
        if frame.event.is_none() {
            continue;
        }
        frame.named(expected);
        return frame.json();
    }
}

/// 验收 P8、P9：上报年龄恰好等于在线有效期时仍为在线，超过一秒即离线；
/// `working` 与 `awaiting-input` 的最后上报状态各自保持，不由过期改写。
///
/// 期望值取自默认策略的独立字面值（10 秒在线有效期），时刻由受控时钟精确驱动。
#[tokio::test]
async fn a_registration_stays_online_through_the_online_window_and_then_keeps_its_last_state() {
    let server = LifecycleServer::start().await;
    assert_accepted(&server.post(&register("sess-working", None)).await);
    assert_accepted(&server.post(&state("sess-working", "working")).await);
    assert_accepted(&server.post(&register("sess-awaiting", None)).await);
    assert_accepted(&server.post(&state("sess-awaiting", "awaiting-input")).await);

    // t=10：年龄恰好等于在线有效期，仍在在线一侧。
    server.advance(Duration::from_secs(10));
    let sessions = server.read_sessions().await;
    assert_eq!(
        session_of(&sessions, "sess-working")["online"],
        json!(true),
        "恰好等于在线有效期仍必须在线：{sessions:?}"
    );
    assert_eq!(
        session_of(&sessions, "sess-awaiting")["online"],
        json!(true),
        "边界对所有会话同样成立：{sessions:?}"
    );

    // t=11：超过在线有效期一秒，读取为离线；登记仍在，最后上报的 agent 状态原样保留。
    server.advance(Duration::from_secs(1));
    let sessions = server.read_sessions().await;
    assert_eq!(
        sessions.len(),
        2,
        "离线只是上报过期，登记仍保留：{sessions:?}"
    );
    let working = session_of(&sessions, "sess-working");
    assert_eq!(
        working["online"],
        json!(false),
        "超过窗口一秒即离线：{working}"
    );
    assert_eq!(
        working["state"],
        json!("working"),
        "过期不得改写最后上报的 agent 状态：{working}"
    );
    let awaiting = session_of(&sessions, "sess-awaiting");
    assert_eq!(awaiting["online"], json!(false));
    assert_eq!(
        awaiting["state"],
        json!("awaiting-input"),
        "等待用户输入同样按最后上报值保留：{awaiting}"
    );
}

/// 验收 P20：新 register 还没有 agent 状态时，外部读取为在线、agent 状态缺省——
/// 既不填 `awaiting-input`，也不冒充离线。
#[tokio::test]
async fn a_new_registration_without_a_state_reads_online_with_no_agent_state() {
    let server = LifecycleServer::start().await;
    assert_accepted(&server.post(&register("sess-fresh", None)).await);

    let sessions = server.read_sessions().await;
    let fresh = session_of(&sessions, "sess-fresh");
    assert_eq!(fresh["online"], json!(true), "刚注册的登记在线：{fresh}");
    assert!(
        fresh.get("state").is_none(),
        "还没有状态时必须省略 state，而不是填它：{fresh}"
    );
    assert_ne!(
        fresh["state"],
        json!("awaiting-input"),
        "状态同步中不得被当成轮次结束：{fresh}"
    );
}

/// 验收 P10、P11：默认策略下 t=40 维护后登记仍在且离线，t=41 维护后读取无该登记、
/// 观察流发布移除；重复维护不复活记录。
///
/// 计时只取最后有效上报：离线是在 t=40 观察到的，但清除仍在 t=41 发生（而不是从「首次发现离线」再等一个窗口）。
#[tokio::test]
async fn maintenance_retains_through_both_windows_and_clears_one_second_later() {
    let server = LifecycleServer::start().await;
    assert_accepted(&server.post(&register("sess-bounded", None)).await);
    // 刚写入：维护无话可说（窗口内不发布任何可见变化）。
    server.maintain().await;
    assert_eq!(server.read_sessions().await.len(), 1);

    let mut observer = server.observe().await;

    // t=40：恰好等于两个窗口之和（10 + 30），登记仍保留、可观察为离线。
    server.advance(Duration::from_secs(40));
    server.maintain().await;
    let flipped = observer.next_event("upsert").await;
    assert_eq!(
        flipped.json()["session"]["online"],
        json!(false),
        "超过在线有效期即离线：{flipped:?}"
    );
    let retained = server.read_sessions().await;
    assert_eq!(
        retained.len(),
        1,
        "恰好等于两个窗口之和仍保留登记：{retained:?}"
    );
    assert_eq!(
        session_of(&retained, "sess-bounded")["online"],
        json!(false)
    );

    // t=41：超过两个窗口之和一秒，维护清除登记并发布移除。
    server.advance(Duration::from_secs(1));
    server.maintain().await;
    let removed = observer.next_event("remove").await;
    assert_eq!(
        removed.json()["sessionId"],
        json!("sess-bounded"),
        "清除必须向观察者发布移除：{removed:?}"
    );
    assert!(
        server.read_sessions().await.is_empty(),
        "清除之后读取不到该登记"
    );

    // 重复维护不复活记录，也不再发布任何帧。
    server.maintain().await;
    server.advance(Duration::from_secs(3600));
    server.maintain().await;
    observer
        .assert_no_named_frame_within(Duration::from_millis(300))
        .await;
    assert!(server.read_sessions().await.is_empty());
}

/// 任何被接受的写入都重新计时：先过期、再写入，读取回到在线；紧接着的维护不再重复发布。
#[tokio::test]
async fn any_accepted_write_restarts_the_registration_window() {
    let server = LifecycleServer::start().await;
    assert_accepted(&server.post(&register("sess-write", None)).await);
    assert_accepted(&server.post(&state("sess-write", "working")).await);

    let mut observer = server.observe().await;
    for write in [
        ("heartbeat", heartbeat("sess-write")),
        ("state", state("sess-write", "awaiting-input")),
        ("register", register("sess-write", None)),
    ] {
        let (kind, body) = write;
        // 先把它推到窗口之外：读取离线。
        server.advance(Duration::from_secs(11));
        server.maintain().await;
        let offline = observer.next_event("upsert").await;
        assert_eq!(
            offline.json()["session"]["online"],
            json!(false),
            "{kind} 之前该登记应已过期：{offline:?}"
        );

        // 写入重新计时：读取回到在线。
        assert_accepted(&server.post(&body).await);
        let online = observer.next_event("upsert").await;
        assert_eq!(
            online.json()["session"]["online"],
            json!(true),
            "{kind} 必须重新开始同一条有效期：{online:?}"
        );

        // 刚写过就维护：无话可说，也不重复发布。
        server.maintain().await;
        observer
            .assert_no_named_frame_within(Duration::from_millis(200))
            .await;
        assert_eq!(
            session_of(&server.read_sessions().await, "sess-write")["online"],
            json!(true)
        );
    }
}

/// 验收 P18：清除登记不删除、也不改写所关联的 agent 对话文件。
///
/// 隔离目录里的文件由用例自己写，随后经 register 的 `sessionFile` 关联；清除之后逐字节比对。
#[tokio::test]
async fn clearing_a_registration_leaves_the_isolated_conversation_file_untouched() {
    let dir = TempDir::new("isolated conversation");
    let conversation = dir.path().join("conversation.jsonl");
    let original = "{\"turn\":1,\"text\":\"第一轮\"}\r\n{\"turn\":2}\r\n";
    fs::write(&conversation, original).expect("写隔离对话文件");

    let server = LifecycleServer::start().await;
    assert_accepted(
        &server
            .post(&register(
                "sess-file",
                Some(&conversation.to_string_lossy()),
            ))
            .await,
    );
    server.advance(Duration::from_secs(41));
    server.maintain().await;
    assert!(server.read_sessions().await.is_empty(), "登记已被清除");

    let after = fs::read_to_string(&conversation).expect("清除之后对话文件仍必须存在");
    assert_eq!(after, original, "清除登记不得改写对话文件内容");
}

/// `unregister` 立即删除登记（不等窗口），herdr-bot 重启（另一份全新的会话表）即清空。
#[tokio::test]
async fn unregister_removes_immediately_and_a_restart_starts_empty() {
    let server = LifecycleServer::start().await;
    assert_accepted(&server.post(&register("sess-gone", None)).await);
    let mut observer = server.observe().await;

    assert_accepted(&server.post(&unregister("sess-gone")).await);
    let removed = observer.next_event("remove").await;
    assert_eq!(removed.json()["sessionId"], json!("sess-gone"));
    assert!(server.read_sessions().await.is_empty(), "注销立即删除登记");

    // 注销之后没有登记：心跳是未知会话，不隐式建行。
    let response = server.post(&heartbeat("sess-gone")).await;
    assert_eq!(
        response.status, 404,
        "未知登记的心跳必须被拒绝：{}",
        response.body
    );

    // 另一份全新的会话表就是「herdr-bot 重启」：内存态不落盘，重启即清空。
    let restarted = LifecycleServer::start().await;
    assert!(
        restarted.read_sessions().await.is_empty(),
        "重启后没有残留会话"
    );
}

/// 慢用例（真实挂钟，默认策略：10 秒在线 / 30 秒保留）：交付运行时那份**每 5 秒**的维护循环
/// 在超过在线有效期后发布离线、在超过两个窗口之和后约一个扫描周期内发布清除。
///
/// 这一遍跑的是后台任务本身（[`spawn_expiry_scan`]），不是把时刻注入某一次扫描；删除也不由前端计时。
#[tokio::test]
async fn the_real_maintenance_loop_publishes_offline_then_removal_after_the_default_windows() {
    let config = common::config(Path::new("unused.bat"), Path::new("."));
    let sessions = Arc::new(SessionsState::new());
    let (app, state) =
        herdr_bot::app_with_state(&config, Arc::new(NeverPowerOff), Arc::clone(&sessions));
    let server = TestServer::start_app(app).await;

    let registered_at = Instant::now();
    let response = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &register("sess-loop", None),
    )
    .await;
    assert_accepted(&response);

    spawn_expiry_scan(Arc::clone(&sessions), Arc::clone(&state.shutdown));

    let mut observer = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let first = observer.next_frame().await;
    first.named("snapshot");
    assert_eq!(
        first.json()["sessions"][0]["online"],
        json!(true),
        "刚注册的会话在线：{first:?}"
    );

    // 离线上界 = 在线有效期 10s + 扫描节奏 5s（另加调度余量）；也不得在窗口内提前翻。
    let offline =
        next_named_event_within(&mut observer, "upsert", Duration::from_secs(10 + 5 + 5)).await;
    assert_eq!(offline["session"]["online"], json!(false), "{offline:?}");
    let staleness = registered_at.elapsed();
    assert!(
        staleness > Duration::from_secs(10),
        "不得在 10 秒在线有效期内提前翻为离线：实际 {staleness:?}"
    );
    assert!(
        staleness <= Duration::from_secs(10) + EXPIRY_SCAN_INTERVAL + Duration::from_secs(2),
        "离线发布的默认上界是 10s + 5s 节奏：实际 {staleness:?}"
    );

    // 清除上界 = 两个窗口之和 40s + 扫描节奏 5s；观察流给出移除帧，读取随之为空。
    let removed =
        next_named_event_within(&mut observer, "remove", Duration::from_secs(30 + 5 + 5)).await;
    assert_eq!(removed["sessionId"], json!("sess-loop"));
    let lifetime = registered_at.elapsed();
    assert!(
        lifetime > Duration::from_secs(40),
        "不得在两个窗口之和内清除登记：实际 {lifetime:?}"
    );
    assert!(
        lifetime <= Duration::from_secs(40) + EXPIRY_SCAN_INTERVAL + Duration::from_secs(2),
        "清除发布的默认上界是 40s + 5s 节奏：实际 {lifetime:?}"
    );

    let mut reader = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let snapshot = reader.next_frame().await;
    assert_eq!(snapshot.json()["sessions"], json!([]));
}
