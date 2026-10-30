//! 共享观察流：会话与关机状态走同一条只读 SSE 长连接（契约 P46；ADR 0005）。
//!
//! 批准契约选择复用既有会话观察流，而不是新开一条关机观察连接：一条连接、一次恢复、一个 adapter。
//! 因此两类状态共用同一个广播通道（[`Hub`]）：每次（重）连的**首帧**同时带上会话快照与关机状态，
//! 之后的会话增量保持原有 `upsert` / `remove` 语义，关机变化则以独立命名事件 `shutdown` 携带**完整**
//! 关机状态（阶段、安排标识、截止时间、服务端时间与当前监听集合）。
//!
//! 首帧与增量既不漏也不重：读路径**先订阅、再取两份快照**，而两类状态的写入都在各自的状态锁内发布，
//! 因此订阅之后发生的每一次可见变化都会作为帧送达；快照里已经含有的变化再送一次只是幂等的重复。
//! 订阅者落后太多（[`RecvError::Lagged`]）时丢弃错过的增量、重发一帧完整快照，读连接不追赶历史。
//!
//! 时间编码只有一种：截止时间与服务端时间都是**服务端时钟的 Unix 毫秒**。前端据此对齐自己与后端的
//! 时钟偏差，后端到期执行只按自己的时钟推进。

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;
use crate::shutdown::ShutdownView;
use crate::sessions::SessionView;

/// 浏览器可见的一帧：某个会话被建行／变化／注销，或关机状态发生变化。
///
/// 会话与关机是两类独立的状态，共用一条通道只是复用同一次读取；帧本身不合并两者。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    /// 某个会话被建行或发生了变化。
    SessionUpsert(SessionView),
    /// 某个会话被注销。
    SessionRemove {
        /// 被注销的会话身份。
        session_id: String,
    },
    /// 关机状态发生变化：携带**完整**关机状态，而不是差分。
    Shutdown(ShutdownView),
}

/// 每个订阅者的有界帧缓冲：落后太多时丢弃增量（读路径用重发快照收敛），不拖慢写入方。
const FRAME_BUFFER: usize = 64;

/// 共享观察流的发布端：会话表与关机安排都往这里发帧，读路径从这里订阅。
///
/// 通道由一方创建、另一方共享；发布发生在各自的状态锁内，因此帧顺序与可见变化的发生顺序一致。
pub struct Hub {
    tx: broadcast::Sender<Observation>,
}

impl Hub {
    /// 空通道：还没有任何订阅者，也没有任何可见变化。
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(FRAME_BUFFER);
        Self { tx }
    }

    /// 订阅可见变化。每个读连接各有一条独立的有界缓冲。
    pub fn subscribe(&self) -> broadcast::Receiver<Observation> {
        self.tx.subscribe()
    }

    /// 发布一帧。发送是同步的、不跨 `.await`，因此在状态锁内调用不会把锁带进等待。
    pub(crate) fn publish(&self, frame: Observation) {
        // 没有订阅者时发送失败，那只意味着没有人在看：可见变化本身照常留在状态里。
        let _ = self.tx.send(frame);
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
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

/// `GET /api/sessions/events`：浏览器侧的**只读**长连接（`text/event-stream`）。
///
/// 每次（重）连的第一帧都是全量 `snapshot`（会话 + 关机状态），之后只推 `upsert` / `remove` /
/// `shutdown` 增量：页面不需要事件日志、不需要 `Last-Event-ID` 或任何游标，断线期间错过的事件也不需要
/// 补——重连（或落后太多时重发的快照）就足以收敛到最新视图。服务端不重放、不落盘，herdr-bot 重启即清空。
///
/// 订阅先于快照：订阅之后发生的每一次可见变化都会作为帧送达；快照里已经含有的变化再送一次只是幂等重复。
/// 两个域各自在自己的锁下取值，因此两条快照分别是两个域各自的合法时刻，谁也不冒充对方的状态。
pub(crate) async fn sse_observation(State(state): State<AppState>) -> Response {
    let sessions = state.sessions.clone();
    let shutdown = state.shutdown.clone();
    // 先订阅、再取首帧：订阅之前的可见变化都在快照里，之后的都会作为帧送达。
    let receiver = sessions.observation().subscribe();
    let first = (sessions.snapshot().await, shutdown.snapshot().await);
    let stream = stream::unfold((receiver, Some(first)), move |(mut receiver, pending)| {
        let sessions = sessions.clone();
        let shutdown = shutdown.clone();
        async move {
            let (views, shutdown_view) = match pending {
                Some(snapshot) => snapshot,
                None => match receiver.recv().await {
                    Ok(frame) => {
                        return Some((stream_item(frame_event(frame)), (receiver, None)));
                    }
                    Err(RecvError::Lagged(_)) => {
                        (sessions.snapshot().await, shutdown.snapshot().await)
                    }
                    Err(RecvError::Closed) => return None,
                },
            };
            Some((
                stream_item(snapshot_event(views, shutdown_view)),
                (receiver, None),
            ))
        }
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(SSE_KEEP_ALIVE))
        .into_response()
}
