//! 会话事件写入端点（`POST /api/sessions/events`）与会话内存态。
//!
//! 每个用例都经 [`TestServer`] 的真实监听端口发出请求，因此驱动的是交付运行时同一份路由；会话表由
//! 用例自己持有并经 `app_with_sessions` 注入，于是「浏览器可见的变化」能同时被订阅到——读路径（SSE）
//! 落地之前，这份订阅就是可见变化的观测面。
//!
//! 覆盖：四种 kind 的成功回执与形状、`register` 的唯一建行与幂等、可空字段缺省即省略（不发 `null`）、
//! `sessionFile`/`pid` 不进视图、状态幂等 upsert、未知会话 `404`、非法事件 `400`、超过 8KB `413`、
//! 带查询串 `400`、未知路径 `unknown-endpoint`。

mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{HttpResponse, TestServer, post_json_over_tcp, post_over_tcp};
use omp_bot::observation::Observation;
use omp_bot::sessions::{ONLINE_WINDOW, SessionsState, is_online};
use omp_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use tokio::sync::broadcast;

/// 会话事件用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        panic!("会话事件用例不应触发关机");
    }
}

/// 一个用例独占的服务：真实监听端口的应用，加上可订阅的同一份会话表。
struct SessionServer {
    server: TestServer,
    frames: broadcast::Receiver<Observation>,
}

impl SessionServer {
    async fn start() -> Self {
        let config = common::config(Path::new("unused.bat"), Path::new("."));
        let sessions = Arc::new(SessionsState::new());
        let frames = sessions.observation().subscribe();
        let app =
            omp_bot::app_with_sessions(&config, Arc::new(NeverPowerOff), Arc::clone(&sessions));
        Self {
            server: TestServer::start_app(app).await,
            frames,
        }
    }

    /// 经真 TCP 提交一条会话事件。
    async fn post(&mut self, body: &str) -> HttpResponse {
        post_json_over_tcp(self.server.addr(), "/api/sessions/events", body).await
    }

    /// 取走当前已发布的全部帧。
    fn drain_frames(&mut self) -> Vec<Observation> {
        let mut frames = Vec::new();
        while let Ok(frame) = self.frames.try_recv() {
            frames.push(frame);
        }
        frames
    }

    /// 取走写入之后必然已经发布的那一帧：写入在返回响应之前发出，所以读到响应就必然能收到。
    fn take_frame(&mut self) -> Observation {
        let mut frames = self.drain_frames();
        assert_eq!(frames.len(), 1, "本次写入应恰好发布一帧：{frames:?}");
        frames.remove(0)
    }

    /// 断言本次写入没有产生任何可见变化（幂等：纯心跳、同值状态）。
    fn assert_no_frame(&mut self) {
        let frames = self.drain_frames();
        assert!(frames.is_empty(), "本次写入不应发布任何帧：{frames:?}");
    }
}

/// 响应体必须是 JSON。
fn body_of(response: &HttpResponse) -> Value {
    serde_json::from_str(&response.body)
        .unwrap_or_else(|e| panic!("响应体必须是 JSON（HTTP {}）：{e}：{}", response.status, response.body))
}

/// 断言一次成功回执：HTTP 200 且恰好是 `{outcome}`。
fn assert_outcome(response: &HttpResponse, outcome: &str) {
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    assert_eq!(body_of(response), json!({ "outcome": outcome }));
}

/// 断言一次拒绝：HTTP 状态码 + 复用既有形状的 `{code, message}`（中文说明不为空）。
fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(response.status, status, "响应体：{}", response.body);
    let body = body_of(response);
    assert_eq!(body["code"], code, "响应体：{}", response.body);
    assert!(
        body["message"].as_str().is_some_and(|m| !m.is_empty()),
        "拒绝必须带中文说明：{}",
        response.body
    );
}

/// 取出一帧 upsert 的会话视图，并断言它恰好是契约里的形状。
fn upsert_view(frame: Observation) -> Value {
    let Observation::SessionUpsert(view) = frame else {
        panic!("应发布一帧会话 upsert：{frame:?}");
    };
    serde_json::to_value(view).expect("会话视图必须可序列化")
}

/// 在线判定窗口：同一判定式供快照与过期扫描共用，边界是 30s 且含 30s 本身。
#[test]
fn online_window_is_inclusive_at_thirty_seconds() {
    let last_seen = Instant::now();
    assert!(is_online(last_seen, last_seen), "刚写入必然在线");
    assert!(
        is_online(last_seen, last_seen + ONLINE_WINDOW),
        "恰 30s 仍在线"
    );
    assert!(
        !is_online(last_seen, last_seen + ONLINE_WINDOW + Duration::from_millis(1)),
        "超过 30s 即离线"
    );
}

/// register 建行并按契约回带在线会话数；视图只有契约字段，可空字段缺省即省略。
#[tokio::test]
async fn register_reports_online_count_and_publishes_the_view() {
    let mut server = SessionServer::start().await;

    let response = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(response.status, 200, "响应体：{}", response.body);
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );

    // 视图字段与契约逐字一致：没有 pid/sessionFile、没有 state，可空字段一个 null 都不出现。
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "online": true})
    );
}

/// 注册时的展示字段是快照：`sessionName`/`model` 进视图，`sessionFile`/`pid` 只存下不下发；
/// 缺省或显式 `null` 的可空字段一律省略。
#[tokio::test]
async fn register_keeps_optional_fields_out_of_the_view_unless_present() {
    let mut server = SessionServer::start().await;

    let response = server
        .post(
            r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work","sessionName":"面板调试",
                "sessionFile":"C:/s.jsonl","model":"opus-4.6","pid":4242}}"#,
        )
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({
            "sessionId": "s1",
            "sessionName": "面板调试",
            "cwd": "C:/work",
            "model": "opus-4.6",
            "online": true
        })
    );

    // 显式 `null` 与缺省同义：都不进视图（不发 null）。
    let response = server
        .post(
            r#"{"sessionId":"s2","kind":"register","data":{"cwd":"C:/w2","sessionName":null,"model":null}}"#,
        )
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 2})
    );
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s2", "cwd": "C:/w2", "online": true})
    );
}

/// 未知字段（信封上的 `v`/`ts`、data 里的多余字段）一律忽略，不改变任何结果。
#[tokio::test]
async fn unknown_fields_are_ignored() {
    let mut server = SessionServer::start().await;

    let response = server
        .post(
            r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work","extra":1},"v":2,"ts":123}"#,
        )
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "online": true})
    );
}

/// 重复 register 幂等：不新起行（在线数不涨），仍是 `registered`，且重注册按最新一次注册刷新展示快照。
#[tokio::test]
async fn repeated_register_is_idempotent_and_refreshes_the_snapshot() {
    let mut server = SessionServer::start().await;

    let first = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work","model":"opus"}}"#)
        .await;
    assert_eq!(
        body_of(&first),
        json!({"outcome": "registered", "onlineCount": 1})
    );
    server.drain_frames();

    // 同一次注册重发：不新起行，在线数仍是 1，也没有任何可见变化。
    let repeated = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work","model":"opus"}}"#)
        .await;
    assert_eq!(
        body_of(&repeated),
        json!({"outcome": "registered", "onlineCount": 1})
    );
    server.assert_no_frame();

    // 展示快照按最新一次注册刷新（不是只认第一次）。
    server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work","model":"sonnet"}}"#)
        .await;
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "model": "sonnet", "online": true})
    );

    // 另一个会话是新行：在线数才涨到 2。
    let other = server
        .post(r#"{"sessionId":"s2","kind":"register","data":{"cwd":"C:/other"}}"#)
        .await;
    assert_eq!(
        body_of(&other),
        json!({"outcome": "registered", "onlineCount": 2})
    );
}

/// 重注册不改已有状态：`register` 不带状态，重注册既不回到「状态同步中」，也不产生任何可见变化。
#[tokio::test]
async fn repeated_register_preserves_the_observed_state_without_a_frame() {
    let mut server = SessionServer::start().await;

    server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    server.drain_frames();
    server
        .post(r#"{"sessionId":"s1","kind":"state","data":{"value":"awaiting-input"}}"#)
        .await;
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "online": true, "state": "awaiting-input"})
    );

    // 若重注册清掉了状态，这里会看到一帧「没有 state」的 upsert；没有帧即证明状态被原样保留。
    let response = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
    server.assert_no_frame();
}

/// state 与带状态的心跳都写进同一个状态字段；同值（含纯心跳）不产生任何可见变化。
#[tokio::test]
async fn state_and_heartbeat_upsert_the_state_idempotently() {
    let mut server = SessionServer::start().await;

    server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    server.drain_frames();

    let response = server
        .post(r#"{"sessionId":"s1","kind":"state","data":{"value":"working"}}"#)
        .await;
    assert_outcome(&response, "accepted");
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "online": true, "state": "working"})
    );

    // 心跳携带的状态与 state 事件同一处理（丢 state 靠心跳自愈）。
    let response = server
        .post(r#"{"sessionId":"s1","kind":"heartbeat","data":{"value":"awaiting-input"}}"#)
        .await;
    assert_outcome(&response, "accepted");
    assert_eq!(
        upsert_view(server.take_frame()),
        json!({"sessionId": "s1", "cwd": "C:/work", "online": true, "state": "awaiting-input"})
    );

    // 同值心跳与纯心跳都只是刷新在线，没有任何可见变化。
    let repeated = server
        .post(r#"{"sessionId":"s1","kind":"heartbeat","data":{"value":"awaiting-input"}}"#)
        .await;
    assert_outcome(&repeated, "accepted");
    server.assert_no_frame();

    let bare = server.post(r#"{"sessionId":"s1","kind":"heartbeat"}"#).await;
    assert_outcome(&bare, "accepted");
    server.assert_no_frame();
}

/// unregister 只带 sessionId，回 `accepted` 并发布移除帧；行移除后同一 id 才算未知会话，可再注册。
#[tokio::test]
async fn unregister_removes_the_row_and_a_later_register_builds_a_new_one() {
    let mut server = SessionServer::start().await;

    server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    server.drain_frames();

    let response = server
        .post(r#"{"sessionId":"s1","kind":"unregister"}"#)
        .await;
    assert_outcome(&response, "accepted");
    assert_eq!(
        server.take_frame(),
        Observation::SessionRemove {
            session_id: "s1".to_string()
        }
    );

    // 行确已移除：心跳得到未知会话。
    let heartbeat = server
        .post(r#"{"sessionId":"s1","kind":"heartbeat"}"#)
        .await;
    assert_rejected(&heartbeat, 404, "unknown-session");

    // 再注册即重新建行。
    let response = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
}

/// 非 register 事件引用未知会话：一律 `404 unknown-session`，且不建行。
#[tokio::test]
async fn non_register_events_for_unknown_sessions_are_rejected_without_building_rows() {
    let mut server = SessionServer::start().await;

    let unknown_events = [
        r#"{"sessionId":"ghost","kind":"heartbeat"}"#,
        r#"{"sessionId":"ghost","kind":"heartbeat","data":{"value":"working"}}"#,
        r#"{"sessionId":"ghost","kind":"state","data":{"value":"working"}}"#,
        r#"{"sessionId":"ghost","kind":"unregister"}"#,
    ];
    for event in unknown_events {
        let response = server.post(event).await;
        assert_rejected(&response, 404, "unknown-session");
    }
    server.assert_no_frame();

    // 前面的事件一个都没建行：真正注册的会话是第 1 个在线会话。
    let response = server
        .post(r#"{"sessionId":"real","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
}

/// 解析失败、缺字段、非法 kind、状态值非法都是 `400 invalid-event`，且都不建行。
#[tokio::test]
async fn malformed_or_incomplete_events_are_invalid_event() {
    let mut server = SessionServer::start().await;

    let invalid_events = [
        ("空请求体", ""),
        ("不是 JSON", "not json at all"),
        ("不是对象", "[1,2,3]"),
        ("缺 sessionId", r#"{"kind":"heartbeat"}"#),
        ("非法 kind", r#"{"sessionId":"s","kind":"ping"}"#),
        ("kind 不是字符串", r#"{"sessionId":"s","kind":7}"#),
        ("register 缺 data", r#"{"sessionId":"s","kind":"register"}"#),
        (
            "register 缺 cwd",
            r#"{"sessionId":"s","kind":"register","data":{"model":"m"}}"#,
        ),
        (
            "register cwd 不是字符串",
            r#"{"sessionId":"s","kind":"register","data":{"cwd":7}}"#,
        ),
        ("state 缺 data", r#"{"sessionId":"s","kind":"state"}"#),
        (
            "state 缺 value",
            r#"{"sessionId":"s","kind":"state","data":{"reason":"x"}}"#,
        ),
        (
            "state 值非法",
            r#"{"sessionId":"s","kind":"state","data":{"value":"busy"}}"#,
        ),
        (
            "heartbeat 值非法",
            r#"{"sessionId":"s","kind":"heartbeat","data":{"value":"在线"}}"#,
        ),
    ];
    for (case, event) in invalid_events {
        let response = server.post(event).await;
        assert_eq!(response.status, 400, "用例「{case}」：{}", response.body);
        assert_eq!(body_of(&response)["code"], "invalid-event", "用例「{case}」");
    }
    server.assert_no_frame();

    // 一个都没建行：被拒绝的事件不产生任何会话行。
    let response = server
        .post(r#"{"sessionId":"s","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
}

/// 超过 8KB 的请求体是 `413 payload-too-large`；8KB 以内的照常接受。
#[tokio::test]
async fn oversized_event_payload_is_rejected() {
    let mut server = SessionServer::start().await;

    let small_cwd = "a".repeat(8_000);
    let response = server
        .post(&format!(
            r#"{{"sessionId":"s1","kind":"register","data":{{"cwd":"{small_cwd}"}}}}"#
        ))
        .await;
    assert_eq!(response.status, 200, "8KB 以内必须照常接受");

    let large_cwd = "a".repeat(8_200);
    let oversized = format!(r#"{{"sessionId":"s2","kind":"register","data":{{"cwd":"{large_cwd}"}}}}"#);
    assert!(oversized.len() > 8 * 1024, "用例本身必须超过 8KB");
    let response = server.post(&oversized).await;
    assert_rejected(&response, 413, "payload-too-large");
}

/// 带查询串的调用是越界请求（事件内容只来自请求体），与请求体是否合法无关。
#[tokio::test]
async fn query_strings_are_rejected_as_unexpected_parameters() {
    let mut server = SessionServer::start().await;

    let bare = post_over_tcp(server.server.addr(), "/api/sessions/events?proxy=false").await;
    assert_rejected(&bare, 400, "unexpected-parameters");

    let with_body = post_json_over_tcp(
        server.server.addr(),
        "/api/sessions/events?proxy=false",
        r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#,
    )
    .await;
    assert_rejected(&with_body, 400, "unexpected-parameters");

    // 被拒绝的请求没有建行。
    let response = server
        .post(r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#)
        .await;
    assert_eq!(
        body_of(&response),
        json!({"outcome": "registered", "onlineCount": 1})
    );
}

/// 会话端点之外的路径仍然是可理解的 `unknown-endpoint`，不落到静态页面。
#[tokio::test]
async fn unknown_session_paths_are_unknown_endpoint() {
    let server = SessionServer::start().await;

    for path in ["/api/sessions/nope", "/api/sessions", "/api/session/events"] {
        let response = post_json_over_tcp(
            server.server.addr(),
            path,
            r#"{"sessionId":"s1","kind":"register","data":{"cwd":"C:/work"}}"#,
        )
        .await;
        assert_eq!(response.status, 404, "路径 {path}：{}", response.body);
        assert_eq!(body_of(&response)["code"], "unknown-endpoint", "路径 {path}");
    }
}
