//! 会话 SSE 读路径（`GET /api/sessions/events`）。
//!
//! 每个用例都经 [`TestServer`] 的真实监听端口建立一条真实 TCP 长连接，按 SSE 帧自己解析响应字节：
//! 断言的是页面实际会收到的帧序列（首帧快照、之后的 `upsert` / `remove` 增量、以及订阅者落后太多时
//! 重发的快照），而不是任何进程内的内部状态。
//!
//! 覆盖：`text/event-stream` 只读长连接与首帧 `retry`、首帧必为全量 `snapshot`、会话视图字段恰为契约
//! 那一组（`pid`/`sessionFile` 不下发、`state` 缺省即省略）、快照按注册顺序升序、纯心跳与同值状态不产生
//! 帧、新注册与注销的增量、重连（含带 `Last-Event-ID` 的重连）仍以完整快照收敛、写入进行中的读者不靠
//! 游标也收敛到最新视图、`Lagged` 后重发快照而不是追赶历史、空白期的 keep-alive，以及超过缺省在线
//! 有效期（10s）没有写入时快照直接现算出**离线**。
//!
//! 除最后一个用例（约 15s，真实挂钟推进）外，每个用例都是秒级。

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use common::sse::{SseConnection, SseFrame};
use common::{HttpResponse, TestServer, post_json_over_tcp};
use herdr_bot::PowerOff;
use herdr_bot::sessions::SessionsState;
use serde_json::{Value, json};

/// 读路径用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> {
        panic!("会话读路径用例不应触发关机");
    }
}

/// 一个用例独占的服务：真实监听端口 + 与交付运行时同一份路由。
struct SseServer {
    server: TestServer,
}

impl SseServer {
    async fn start() -> Self {
        let config = common::config(Path::new("unused.bat"), Path::new("."));
        let app = herdr_bot::app_with_sessions(
            &config,
            Arc::new(NeverPowerOff),
            Arc::new(SessionsState::new()),
        );
        Self {
            server: TestServer::start_app(app).await,
        }
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&self, body: &str) -> HttpResponse {
        post_json_over_tcp(self.server.addr(), "/api/sessions/events", body).await
    }

    /// 提交一条必然被接受的会话事件。
    async fn post_ok(&self, body: &str) {
        let response = self.post(body).await;
        assert_eq!(
            response.status, 200,
            "会话事件应被接受：{body} → {} {}",
            response.status, response.body
        );
    }

    /// 建立一条真实的只读长连接。
    async fn connect(&self) -> SseConnection {
        SseConnection::open(self.server.addr(), "/api/sessions/events", &[]).await
    }

    /// 带 `Last-Event-ID` 的重连：读路径不认游标，行为必须与普通（重）连完全一致。
    async fn reconnect_with_last_event_id(&self, last_event_id: &str) -> SseConnection {
        SseConnection::open(
            self.server.addr(),
            "/api/sessions/events",
            &[("Last-Event-ID", last_event_id)],
        )
        .await
    }
}

fn register(id: &str, cwd: &str) -> String {
    json!({ "sessionId": id, "kind": "register", "data": { "cwd": cwd } }).to_string()
}

fn state(id: &str, value: &str) -> String {
    json!({ "sessionId": id, "kind": "state", "data": { "value": value } }).to_string()
}

/// 纯心跳：只刷最后写入时间，不带状态。
fn heartbeat(id: &str) -> String {
    json!({ "sessionId": id, "kind": "heartbeat", "data": {} }).to_string()
}

fn unregister(id: &str) -> String {
    json!({ "sessionId": id, "kind": "unregister" }).to_string()
}

/// 断言一个会话视图恰好是契约里的字段：可空字段缺省即省略，`pid`/`sessionFile` 永不下发。
fn assert_view_shape(view: &Value) {
    let object = view.as_object().expect("会话视图必须是 JSON 对象");
    for key in object.keys() {
        assert!(
            matches!(
                key.as_str(),
                "sessionId" | "sessionName" | "cwd" | "model" | "online" | "state"
            ),
            "会话视图不得出现契约外的字段：{key}（视图：{view}）"
        );
    }
    assert!(object["sessionId"].is_string(), "视图必须带 sessionId：{view}");
    assert!(object["cwd"].is_string(), "视图必须带 cwd：{view}");
    assert!(object["online"].is_boolean(), "视图必须带 online：{view}");
    if let Some(state) = object.get("state") {
        assert!(
            matches!(state.as_str(), Some("idle" | "working" | "awaiting-input")),
            "状态只能是三态之一：{view}"
        );
    }
}

/// 取一帧增量里的会话视图，并断言帧的形状恰是 `{session:{…}}`。
fn upsert_session(frame: &SseFrame) -> Value {
    let data = frame.json();
    assert!(
        data.as_object().is_some_and(|object| object.len() == 1),
        "增量帧只带 session，不带版本/时间戳：{data}"
    );
    let view = data
        .get("session")
        .cloned()
        .unwrap_or_else(|| panic!("upsert 帧的 data 必须是 {{session:{{…}}}}：{data}"));
    assert_view_shape(&view);
    view
}

/// 取一帧快照里的会话列表（首帧同时带关机状态，这里只取会话那一半）。
fn snapshot_sessions(frame: &SseFrame) -> Vec<Value> {
    let data = frame.json();
    assert!(
        data.get("sessions").is_some_and(|s| s.is_array()),
        "快照帧的 data 必须是 {{sessions:[…], shutdown:{{…}}}}：{data}"
    );
    assert!(
        data.get("shutdown").is_some_and(|s| s.is_object()),
        "快照帧同时带关机状态（共享观察流）：{data}"
    );
    assert!(
        data.as_object().is_some_and(|object| object.len() == 2),
        "快照帧不带版本/时间戳，只带 sessions 与 shutdown：{data}"
    );
    data["sessions"].as_array().cloned().unwrap_or_default()
}

#[tokio::test]
async fn connect_upgrades_to_a_read_only_stream_whose_first_frame_is_the_retrying_snapshot() {
    let server = SseServer::start().await;
    let mut connection = server.connect().await;

    assert_eq!(connection.status, 200, "读路径必须可用");
    assert_eq!(
        connection.header("content-type"),
        Some("text/event-stream"),
        "读路径必须是 SSE：{:?}",
        connection.headers
    );

    let frame = connection.next_frame().await;
    frame.named("snapshot");
    assert_eq!(
        frame.retry,
        Some(2000),
        "首帧必须带 ≈2s 的重连提示：{frame:?}"
    );
    assert_eq!(
        snapshot_sessions(&frame),
        Vec::<Value>::new(),
        "没有会话时快照是空列表：{frame:?}"
    );

    // 只读长连接在没有任何写入时保持安静：不推增量、也不结束。
    connection
        .assert_no_named_frame_within(Duration::from_millis(300))
        .await;
}

#[tokio::test]
async fn the_snapshot_carries_every_view_in_registration_order() {
    let server = SseServer::start().await;
    // 注册顺序与 id 的字典序刻意不同：快照必须按注册顺序升序，而不是按 id 或哈希顺序。
    let registrations = [
        ("session-z", "/work/zeta"),
        ("session-a", "/work/alpha"),
        ("session-m", "/work/mu"),
        ("session-b", "/work/beta"),
        ("session-y", "/work/upsilon"),
        ("session-c", "/work/gamma"),
    ];
    for (id, cwd) in registrations {
        server.post_ok(&register(id, cwd)).await;
    }
    // 展示字段是可空的注册快照：给了才下发，`sessionFile`/`pid` 只存下备查。
    server
        .post_ok(
            &json!({
                "sessionId": "session-a",
                "kind": "register",
                "data": {
                    "cwd": "/work/alpha",
                    "sessionName": "一号会话",
                    "model": "展示名模型",
                    "sessionFile": "/sessions/a.jsonl",
                    "pid": 4242
                }
            })
            .to_string(),
        )
        .await;
    // 重注册刷新注册时间：它因此排到最后。
    server.post_ok(&register("session-y", "/work/upsilon")).await;

    let mut connection = server.connect().await;
    let frame = connection.next_frame().await;
    frame.named("snapshot");

    let sessions = snapshot_sessions(&frame);
    let ids: Vec<&str> = sessions
        .iter()
        .map(|view| view["sessionId"].as_str().expect("视图必须带 sessionId"))
        .collect();
    // 顺序 = 最后一次 register 的时间升序：首次注册的顺序是 z、a、m、b、y、c，随后 a 与 y 各重注册
    // 一次（重注册刷新注册时间），于是这两行按 a 之后、y 最后落到末位。
    assert_eq!(
        ids,
        vec![
            "session-z",
            "session-m",
            "session-b",
            "session-c",
            "session-a",
            "session-y",
        ],
        "快照按注册顺序升序（每次 register 刷新注册时间）：{sessions:?}"
    );

    for view in &sessions {
        assert_view_shape(view);
        assert_eq!(view["online"], json!(true), "刚注册的会话在线：{view}");
    }
    let named = sessions
        .iter()
        .find(|view| view["sessionId"] == json!("session-a"))
        .expect("快照必须包含 session-a");
    assert_eq!(named["sessionName"], json!("一号会话"));
    assert_eq!(named["model"], json!("展示名模型"));
    assert_eq!(named["cwd"], json!("/work/alpha"));
    assert!(
        named.get("state").is_none(),
        "还没收到状态时 `state` 缺省（不是 unknown 哨兵）：{named}"
    );
    assert!(
        !frame.raw.contains("sessionFile") && !frame.raw.contains("pid"),
        "`sessionFile`/`pid` 只存下备查，不下发：{:?}",
        frame.raw
    );
}

#[tokio::test]
async fn only_visible_changes_are_pushed_as_increments_after_the_snapshot() {
    let server = SseServer::start().await;
    server.post_ok(&register("s-one", "/work/one")).await;
    server.post_ok(&register("s-two", "/work/two")).await;

    let mut connection = server.connect().await;
    let snapshot = connection.next_frame().await;
    snapshot.named("snapshot");
    assert_eq!(snapshot_sessions(&snapshot).len(), 2, "快照含全部会话");

    // 状态真变化 → 一帧 upsert，内容是变化后的那一个会话视图。
    server.post_ok(&state("s-one", "working")).await;
    let frame = connection.next_named_frame().await;
    frame.named("upsert");
    let view = upsert_session(&frame);
    assert_eq!(view["sessionId"], json!("s-one"));
    assert_eq!(view["state"], json!("working"));
    assert_eq!(view["cwd"], json!("/work/one"));

    // 纯心跳只刷最后写入时间：状态没变，浏览器可见视图没变 → 不产生任何帧。
    server.post_ok(&heartbeat("s-one")).await;
    connection
        .assert_no_named_frame_within(Duration::from_millis(300))
        .await;

    // 同值状态重复上报同样不是可见变化。
    server.post_ok(&state("s-one", "working")).await;
    connection
        .assert_no_named_frame_within(Duration::from_millis(300))
        .await;

    // 新注册的会话在快照之后只以增量出现，且还没有状态（不是 unknown 哨兵）。
    server.post_ok(&register("s-three", "/work/three")).await;
    let frame = connection.next_named_frame().await;
    frame.named("upsert");
    let view = upsert_session(&frame);
    assert_eq!(view["sessionId"], json!("s-three"));
    assert_eq!(view["online"], json!(true));
    assert!(view.get("state").is_none(), "新会话还没有状态：{view}");

    // 注销是这里唯一的 remove：只带会话身份。
    server.post_ok(&unregister("s-two")).await;
    let frame = connection.next_named_frame().await;
    frame.named("remove");
    assert_eq!(frame.json(), json!({ "sessionId": "s-two" }));

    // 增量通道在两次「什么都不推」之后仍然活着：真变化照样到。
    server.post_ok(&state("s-one", "idle")).await;
    let frame = connection.next_named_frame().await;
    frame.named("upsert");
    assert_eq!(upsert_session(&frame)["state"], json!("idle"));
}

#[tokio::test]
async fn reconnecting_converges_on_a_fresh_snapshot_and_ignores_any_cursor() {
    let server = SseServer::start().await;
    server.post_ok(&register("s-one", "/work/one")).await;

    let mut first = server.connect().await;
    let snapshot = first.next_frame().await;
    snapshot.named("snapshot");
    assert_eq!(snapshot_sessions(&snapshot).len(), 1);
    drop(first);

    // 断线期间错过的写入：不补、不重放，靠重连的快照收敛。
    server.post_ok(&state("s-one", "awaiting-input")).await;
    server.post_ok(&register("s-two", "/work/two")).await;

    // 带 `Last-Event-ID` 的重连与普通重连完全同形：读路径不认游标，第一帧仍是完整快照。
    let mut second = server.reconnect_with_last_event_id("4711").await;
    let frame = second.next_frame().await;
    frame.named("snapshot");
    assert_eq!(frame.retry, Some(2000), "（重）连的首帧都带重连提示");
    let sessions = snapshot_sessions(&frame);
    assert_eq!(
        sessions.len(),
        2,
        "重连的快照必须覆盖断线期间的全部变化：{sessions:?}"
    );
    let one = sessions
        .iter()
        .find(|view| view["sessionId"] == json!("s-one"))
        .expect("快照必须包含 s-one");
    assert_eq!(
        one["state"],
        json!("awaiting-input"),
        "断线期间的状态变化由快照补齐：{one}"
    );

    // 重连之后仍然只收增量。
    server.post_ok(&state("s-two", "working")).await;
    let frame = second.next_named_frame().await;
    frame.named("upsert");
    assert_eq!(upsert_session(&frame)["sessionId"], json!("s-two"));
}

/// 把一帧应用到读者本地的视图表——这就是页面要做的事：快照全量替换，增量就地生效，
/// 注释帧（keep-alive）什么也不改。整个过程不需要事件日志，也不需要游标。
fn apply(views: &mut BTreeMap<String, Value>, frame: &SseFrame) {
    match frame.event.as_deref() {
        Some("snapshot") => {
            views.clear();
            for view in snapshot_sessions(frame) {
                let id = view["sessionId"].as_str().expect("视图必须带 sessionId");
                views.insert(id.to_string(), view);
            }
        }
        Some("upsert") => {
            let view = upsert_session(frame);
            let id = view["sessionId"].as_str().expect("视图必须带 sessionId");
            views.insert(id.to_string(), view);
        }
        Some("remove") => {
            let removed = frame.json()["sessionId"]
                .as_str()
                .expect("remove 帧必须带 sessionId")
                .to_string();
            views.remove(&removed);
        }
        None => {}
        other => panic!("读路径不得下发该命名事件：{other:?}"),
    }
}

#[tokio::test]
async fn a_reader_converges_without_a_cursor_while_writes_are_in_flight() {
    let server = SseServer::start().await;
    let ids = ["a", "b", "c", "d"];
    for id in ids {
        server.post_ok(&register(id, &format!("/work/{id}"))).await;
    }

    // 写入与订阅同时进行：任务在读，另一个任务在灌状态变化。
    let addr = server.server.addr();
    let writer = tokio::spawn(async move {
        for round in 0..30 {
            let value = if round % 2 == 0 { "working" } else { "awaiting-input" };
            for id in ids {
                let response =
                    post_json_over_tcp(addr, "/api/sessions/events", &state(id, value)).await;
                assert_eq!(response.status, 200, "写入应被接受：{}", response.body);
            }
        }
    });

    let mut connection = server.connect().await;
    let mut views = BTreeMap::new();
    apply(&mut views, &connection.next_frame().await);
    assert_eq!(views.len(), 4, "首帧快照就是全部会话：{views:?}");

    writer.await.expect("写入任务");
    // 写入全部结束后把剩下的帧读干净：视图必须收敛到最终状态，不靠任何补发或重放。
    while let Some(frame) = connection
        .next_frame_within(Duration::from_millis(300))
        .await
    {
        apply(&mut views, &frame);
    }

    assert_eq!(views.len(), 4, "视图表应仍是四个会话：{views:?}");
    for id in ids {
        // 最后一轮（第 29 轮）是奇数轮 → awaiting-input。
        assert_eq!(
            views.get(id).map(|view| &view["state"]),
            Some(&json!("awaiting-input")),
            "会话 {id} 的视图没有收敛到最新状态：{views:?}"
        );
    }
}

#[tokio::test]
async fn a_lagged_subscriber_gets_a_fresh_snapshot_instead_of_the_missed_increments() {
    let server = SseServer::start().await;
    // 每帧都很大：读者不读时，服务端写这条连接会很快堵在 socket 缓冲上，
    // 于是订阅者的有界缓冲（64 帧）必然溢出。
    let long_cwd = format!("/work/{}", "x".repeat(4000));
    server
        .post_ok(
            &json!({ "sessionId": "s-big", "kind": "register", "data": { "cwd": long_cwd } })
                .to_string(),
        )
        .await;

    let mut connection = server.connect().await;
    let snapshot = connection.next_frame().await;
    snapshot.named("snapshot");
    assert_eq!(snapshot_sessions(&snapshot).len(), 1);

    // 从这里起不再读这条连接，先把订阅者灌到落后（每一帧都是真实的可见变化）。
    let flood = 300;
    let mut last = "working";
    for index in 0..flood {
        last = if index % 2 == 0 { "working" } else { "awaiting-input" };
        server.post_ok(&state("s-big", last)).await;
    }

    // 落后太多时读路径丢弃错过的增量、重发一帧快照：读者于是直接收敛到当下，而不是去追历史。
    let mut frames = 0;
    let recovered = loop {
        frames += 1;
        assert!(
            frames <= flood / 2,
            "落后之后应重发快照，而不是把 {flood} 帧增量逐条推完（已读 {frames} 帧）"
        );
        let frame = connection.next_frame().await;
        if frame.event.as_deref() == Some("snapshot") {
            break frame;
        }
    };
    let sessions = snapshot_sessions(&recovered);
    assert_eq!(sessions.len(), 1, "重发的快照是当下的全量视图");
    assert_eq!(
        sessions[0]["state"],
        json!(last),
        "重发的快照必须反映最后一条写入：{sessions:?}"
    );
}

/// 慢用例（约 15s）：读路径与**时间**有关的两件事都由真实挂钟推进。
///
/// - 空白期只有注释帧（keep-alive，约 15s），会话内容不因心跳改变；
/// - 最后一次写入起超过缺省在线有效期（10s）后，新连接的快照直接说**离线**：离线由共用判定式现算，
///   不依赖任何维护先跑过一轮，行本身保留（登记仍在离线保留期内）。
#[tokio::test]
async fn an_idle_read_path_keeps_alive_and_reports_stale_sessions_as_offline() {
    let server = SseServer::start().await;
    let registered_at = tokio::time::Instant::now();
    server.post_ok(&register("s-idle", "/work/idle")).await;

    let mut connection = server.connect().await;
    let first = connection.next_frame().await;
    first.named("snapshot");
    assert_eq!(
        snapshot_sessions(&first)[0]["online"],
        json!(true),
        "刚注册的会话在线"
    );

    let keep_alive = connection
        .next_frame_within(Duration::from_secs(20))
        .await
        .unwrap_or_else(|| panic!("空白期应收到 keep-alive 注释帧"));
    assert_eq!(
        keep_alive.event, None,
        "keep-alive 不是命名事件：{keep_alive:?}"
    );
    assert_eq!(
        keep_alive.data, None,
        "keep-alive 不带任何会话内容：{keep_alive:?}"
    );
    assert!(
        tokio::time::Instant::now().duration_since(registered_at) >= Duration::from_secs(14),
        "keep-alive 约 15s 一次，不得提前到 14s 以内"
    );

    tokio::time::sleep_until(registered_at + Duration::from_secs(12)).await;
    let mut after = server.connect().await;
    let stale = after.next_frame().await;
    stale.named("snapshot");
    let sessions = snapshot_sessions(&stale);
    assert_eq!(sessions.len(), 1, "离线行保留到登记被清除：{sessions:?}");
    assert_eq!(
        sessions[0]["online"],
        json!(false),
        "超过缺省在线有效期 10s 没有写入即离线（快照现算）：{sessions:?}"
    );
    assert!(
        sessions[0].get("state").is_none(),
        "离线不改写 agent 状态：{sessions:?}"
    );
}
