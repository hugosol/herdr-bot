//! 会话事件写入端点的来源边界（`POST /api/sessions/events`）。
//!
//! 写入只接受**本机来源**——回环，或本机网卡地址集合里的地址；其余一律 `403 forbidden` 且不建行。
//! 读取（页面 + SSE）不受这一校验影响：对局域网全开，也不要求任何 token（本文件的读请求都不带凭据）。
//!
//! 来源经 `ConnectInfo` 这一接缝驱动，两侧各用一半：
//! - **真 TCP**：用例起一个真实监听端口，连接它的源地址由内核决定——回环地址一份、本机网卡地址一份
//!   （正是扩展经 `herdr-bot.local` 接入时服务端看到的样子），断言同机来源通过校验。
//! - **受控来源**：在应用上套 axum 的 `MockConnectInfo`，把对端地址设成 RFC 5737 的文档地址（本机
//!   不可能拥有），代表局域网设备，断言写入被拒、读取照常。
//!
//! 覆盖：回环来源写入 `200`、本机网卡地址写入 `200`、局域网来源写入 `403 forbidden` 且不建行、
//! 局域网来源读页面与 SSE 都是 `200`（且读到本机写入的会话）。

mod common;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use common::{TestServer, post_json_over_tcp};
use herdr_bot::PowerOff;
use herdr_bot::sessions::SessionsState;
use serde_json::{Value, json};
use tower::ServiceExt;

/// 写入边界用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> {
        panic!("写入边界用例不应触发关机");
    }
}

/// 代表局域网设备的来源地址：RFC 5737 的 TEST-NET-3 文档地址，本机不可能拥有它。
const LAN_SOURCE: &str = "203.0.113.7:40000";

/// 与会话用例同一份装配：路由注册与交付运行时一致，会话表由用例持有。
fn app_with(sessions: &Arc<SessionsState>) -> Router {
    let config = common::config(Path::new("unused.bat"), Path::new("."));
    herdr_bot::app_with_sessions(&config, Arc::new(NeverPowerOff), Arc::clone(sessions))
}

/// 把应用装上受控的 TCP 对端地址：这批请求不来自真 socket，来源就是这里指定的地址。
///
/// 用 axum 自带的 `MockConnectInfo`（它只往请求里放 `ConnectInfo`），接缝与真连接信息同一条，
/// 因此校验读到的来源与真 TCP 情形是同一个东西。
fn from_source(app: &Router, source: &str) -> Router {
    app.clone()
        .layer(MockConnectInfo(source.parse::<SocketAddr>().expect("受控来源地址")))
}

/// 本机某个可发布的 IPv4（与 mDNS 发布同一枚举口径）；本机只有回环时不构造网卡地址用例。
fn local_nic_ipv4() -> Option<Ipv4Addr> {
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

/// 一条 `register` 事件；展示字段只需一个必填的 `cwd`。
fn register(session_id: &str, cwd: &str) -> String {
    json!({"sessionId": session_id, "kind": "register", "data": {"cwd": cwd}}).to_string()
}

/// 一条 `heartbeat` 事件。
fn heartbeat(session_id: &str) -> String {
    json!({"sessionId": session_id, "kind": "heartbeat", "data": {}}).to_string()
}

/// 响应体必须是 JSON 对象。
fn body_of(status: u16, body: &str) -> Value {
    serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("响应体必须是 JSON（HTTP {status}）：{e}：{body}"))
}

/// 不经真 socket 提交一次请求（来源由 [`from_source`] 决定），返回状态码与响应体。
async fn send(app: Router, method: &str, uri: &str, body: String) -> (StatusCode, String) {
    let mut builder = Request::builder().method(method).uri(uri);
    if !body.is_empty() {
        builder = builder.header("content-type", "application/json");
    }
    let response = app
        .oneshot(builder.body(Body::from(body)).expect("构造请求"))
        .await
        .expect("请求应用");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// 建立只读长连接并读出首帧（到帧尾的空行为止），返回状态码与首帧原文。
async fn first_sse_frame(app: Router, uri: &str) -> (StatusCode, String) {
    use futures_util::StreamExt;

    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .expect("构造请求");
    let response = app.oneshot(request).await.expect("请求应用");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "SSE 响应的 Content-Type 不对：{content_type}"
    );

    let mut stream = response.into_body().into_data_stream();
    let mut frame: Vec<u8> = Vec::new();
    while !frame.ends_with(b"\n\n") {
        let chunk = tokio::time::timeout(Duration::from_secs(10), stream.next())
            .await
            .expect("SSE 首帧应在超时前到达")
            .expect("SSE 流不应在首帧前结束")
            .expect("读取 SSE 首帧失败");
        frame.extend_from_slice(&chunk);
    }
    (status, String::from_utf8_lossy(&frame).into_owned())
}

/// 回环来源（真 TCP）的写入被接受：这也是「运行入口与测试基座都改用带连接信息的 make-service」的
/// 端到端证明——少了那层装配，handler 取不到来源，回执不会是 200。
#[tokio::test]
async fn loopback_source_write_is_accepted() {
    let sessions = Arc::new(SessionsState::new());
    let server = TestServer::start_app(app_with(&sessions)).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &register("loopback-1", "C:\\work"),
    )
    .await;

    assert_eq!(
        response.status, 200,
        "回环来源的写入应被接受：{} {}",
        response.status, response.body
    );
    assert_eq!(
        body_of(response.status, &response.body)["outcome"],
        "registered"
    );
}

/// 同机经本机网卡地址接入（扩展连 `herdr-bot.local:8787` 时的真实形态）能通过校验，不被误拒。
#[tokio::test]
async fn same_host_access_via_local_nic_address_is_accepted() {
    let Some(address) = local_nic_ipv4() else {
        eprintln!("跳过：本机没有可发布的局域网 IPv4，无法构造经网卡地址的同机访问");
        return;
    };

    let sessions = Arc::new(SessionsState::new());
    // 监听在网卡地址上：同机连这个地址时，TCP 源地址就是那个网卡地址（不是 127.0.0.1）。
    let server =
        TestServer::start_app_on(SocketAddr::from((address, 0)), app_with(&sessions)).await;

    let response = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &register("nic-1", "C:\\work"),
    )
    .await;

    assert_eq!(
        response.status, 200,
        "源 IP 为本机网卡地址的同机访问应通过校验（{address}）：{} {}",
        response.status, response.body
    );
    assert_eq!(
        body_of(response.status, &response.body)["outcome"],
        "registered"
    );
}

/// 局域网来源的写入被拒绝，且不建行——被拒的 `register` 之后，本机来源的同会话 `heartbeat` 仍报未知会话。
#[tokio::test]
async fn lan_source_write_is_forbidden_and_creates_no_session() {
    let sessions = Arc::new(SessionsState::new());
    let app = app_with(&sessions);
    let server = TestServer::start_app(app.clone()).await;

    let (status, body) = send(
        from_source(&app, LAN_SOURCE),
        "POST",
        "/api/sessions/events",
        register("lan-1", "C:\\work"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "局域网来源的写入必须被拒绝：{status} {body}"
    );
    let rejection = body_of(status.as_u16(), &body);
    assert_eq!(rejection["code"], "forbidden");
    assert!(
        rejection["message"].as_str().is_some_and(|m| !m.is_empty()),
        "拒绝响应应沿用 {{code, message}} 形状：{body}"
    );

    // 拒绝发生在建行之前：这条会话在本机来源看来仍不存在。
    let afterwards = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &heartbeat("lan-1"),
    )
    .await;
    assert_eq!(
        afterwards.status, 404,
        "被拒的 register 不得建行：{} {}",
        afterwards.status, afterwards.body
    );
    assert_eq!(
        body_of(afterwards.status, &afterwards.body)["code"],
        "unknown-session"
    );
}

/// 读取（页面 + SSE）对局域网全开、无 token：局域网来源既读得到打包页面，也收得到本机写入的会话。
#[tokio::test]
async fn reads_are_open_to_lan_sources() {
    let sessions = Arc::new(SessionsState::new());
    let app = app_with(&sessions);
    let server = TestServer::start_app(app.clone()).await;

    // 先用本机来源写入一行；读路径随后应把它原样交给局域网来源的读者。
    let written = post_json_over_tcp(
        server.addr(),
        "/api/sessions/events",
        &register("lan-read-1", "C:\\work"),
    )
    .await;
    assert_eq!(
        written.status, 200,
        "本机来源的写入应被接受：{} {}",
        written.status, written.body
    );

    let lan = from_source(&app, LAN_SOURCE);

    let (status, page) = send(lan.clone(), "GET", "/", String::new()).await;
    assert_eq!(status, StatusCode::OK, "页面读取不该受来源校验影响");
    assert!(
        page.contains("<title>herdr-bot</title>"),
        "局域网来源读到的应是打包页面：{page}"
    );

    let (status, frame) = first_sse_frame(lan, "/api/sessions/events").await;
    assert_eq!(status, StatusCode::OK, "SSE 读取不该受来源校验影响");
    assert!(frame.contains("event: snapshot"), "首帧必须是快照：{frame:?}");
    assert!(
        frame.contains("lan-read-1"),
        "首帧快照里应有本机写入的会话：{frame:?}"
    );
}
