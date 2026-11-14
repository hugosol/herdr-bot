use std::{convert::Infallible, net::SocketAddr};
use axum::{Json, body::to_bytes, extract::{ConnectInfo, Request, State}, http::StatusCode, response::{IntoResponse, Response, sse::{Event, KeepAlive, Sse}}};
use futures_util::StreamExt;
use serde::Serialize;
use super::MAX_REQUEST_BODY_BYTES;
use crate::{AppState, sessions::{self, SessionEvent, SessionEventError, SessionView, observation::{Observation, ObservationEvent}}, shutdown::ShutdownView};
use crate::controllers::rejected;

fn error_response(error: SessionEventError) -> Response {
    match error {
        SessionEventError::Forbidden => rejected(StatusCode::FORBIDDEN, "forbidden", "会话事件写入只接受本机来源：局域网设备不能伪造或刷面板。"),
        SessionEventError::UnknownSession => rejected(StatusCode::NOT_FOUND, "unknown-session", "该会话尚未注册：非 register 事件不能为未知会话建行。"),
        SessionEventError::Chief(error) => crate::controllers::chief_controller::error_response(error),
    }
}

pub(crate) async fn post_session_event(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = sessions::authorize_source(peer.ip()) { return error_response(error); }
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected(StatusCode::BAD_REQUEST, "unexpected-parameters", "会话事件请求不接受查询参数：事件内容只来自请求体。");
    }
    let bytes = match to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return rejected(StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large", "会话事件请求体超过 8KB 上限。"),
    };
    let Ok(event) = serde_json::from_slice::<SessionEvent>(&bytes) else {
        return rejected(StatusCode::BAD_REQUEST, "invalid-event", "会话事件无法解析：请发送 {sessionId, kind, data} 形式的合法事件。");
    };
    match sessions::accept_event(&state, event).await {
        Ok(accepted) => Json(accepted).into_response(),
        Err(error) => error_response(error),
    }
}

/// 重连提示：每次（重）连的**首帧**带上它，浏览器断线后约 2s 重新连上。
///
/// 首帧是快照，提示就挂在同一帧上（SSE 允许 `retry` 与 `event`/`data` 同帧），于是「第一帧必为
/// `snapshot`」与「首帧带重连提示」同时成立，不需要先发一帧什么也不带的控制帧。
const SSE_RETRY: std::time::Duration = std::time::Duration::from_secs(2);

/// 长连接的 keep-alive 间隔：没有帧可推时约每 15s 发一个注释帧，只读连接不被中间设备掐断。
///
/// keep-alive 只是心跳，不携带任何内容，也不是命名事件：页面不因它改变视图。
const SSE_KEEP_ALIVE: std::time::Duration = std::time::Duration::from_secs(15);

/// 命名事件 `snapshot`：每次（重）连的第一帧，会话与关机状态的全量视图。
const SNAPSHOT_EVENT: &str = "snapshot";

/// 命名事件 `upsert`：某个会话被建行、注销之外地发生了变化。
const UPSERT_EVENT: &str = "upsert";

/// 命名事件 `remove`：某个会话被注销。
const REMOVE_EVENT: &str = "remove";

/// 命名事件 `shutdown`：关机状态发生了变化（携带完整关机状态）。
const SHUTDOWN_EVENT: &str = "shutdown";

/// `snapshot` 帧的内容：会话快照在前、关机状态在后。
#[derive(Debug, Serialize)]
struct SnapshotData {
    /// 全部会话的视图，按注册顺序升序。
    sessions: Vec<SessionView>,
    /// 当前完整的关机状态。
    shutdown: ShutdownView,
}

/// `upsert` 帧的内容：变化后的那**一个**会话视图。
#[derive(Debug, Serialize)]
struct UpsertData {
    /// 变化后的会话视图。
    session: SessionView,
}

/// `remove` 帧的内容：被注销的会话身份。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoveData {
    /// 被注销的会话身份。
    session_id: String,
}

/// `shutdown` 帧的内容：完整关机状态（与首帧里的那一份同一形状）。
#[derive(Debug, Serialize)]
struct ShutdownData {
    /// 当前完整的关机状态。
    shutdown: ShutdownView,
}

/// 帧流的一项：帧的组装不可能失败，因此流的错误类型是 [`Infallible`]。
fn stream_item(event: Event) -> Result<Event, Infallible> {
    Ok(event)
}

/// 组装一帧命名事件。
///
/// 帧只含字符串、数字、布尔与封闭枚举，序列化不可能失败；帧里没有版本位、没有时间戳，
/// 也没有 `id:` 游标字段。
fn named_event(name: &'static str, data: &impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(data)
        .expect("观察帧只含字符串、数字与布尔，序列化不会失败")
}

/// 把写入侧发布的一帧增量转成下发帧。
fn frame_event(frame: Observation) -> Event {
    match frame {
        Observation::SessionUpsert(view) => {
            named_event(UPSERT_EVENT, &UpsertData { session: view })
        }
        Observation::SessionRemove { session_id } => {
            named_event(REMOVE_EVENT, &RemoveData { session_id })
        }
        Observation::Shutdown(view) => {
            named_event(SHUTDOWN_EVENT, &ShutdownData { shutdown: view })
        }
    }
}

/// 把一份快照转成下发帧。
///
/// 首帧与「订阅者落后太多」的重发走的是同一个形状，因此重连与重发在页面上是同一条收敛路径。
fn snapshot_event(sessions: Vec<SessionView>, shutdown: ShutdownView) -> Event {
    named_event(SNAPSHOT_EVENT, &SnapshotData { sessions, shutdown }).retry(SSE_RETRY)
}

pub(crate) async fn sse_observation(State(state): State<AppState>) -> Response {
    let stream = sessions::observation::observe(&state).await.map(|event| stream_item(match event {
        ObservationEvent::Snapshot { sessions, shutdown } => snapshot_event(sessions, shutdown),
        ObservationEvent::Change(frame) => frame_event(frame),
    }));
    Sse::new(stream).keep_alive(KeepAlive::new().interval(SSE_KEEP_ALIVE)).into_response()
}
