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

use futures_util::stream;
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

/// Typed observation data; the HTTP adapter alone chooses SSE framing.
pub(crate) enum ObservationEvent {
    Snapshot { sessions: Vec<SessionView>, shutdown: ShutdownView },
    Change(Observation),
}

/// Subscribe before the immediate snapshot; recover lag with the current complete view.
pub(crate) async fn observe(state: &AppState) -> impl futures_util::Stream<Item = ObservationEvent> + Send + use<> {
    let sessions = state.sessions.clone();
    let shutdown = state.shutdown.clone();
    // 先订阅、再取首帧：订阅之前的可见变化都在快照里，之后的都会作为帧送达。
    let receiver = sessions.observation().subscribe();
    let first = (sessions.snapshot().await, shutdown.snapshot().await);
    stream::unfold((receiver, Some(first)), move |(mut receiver, pending)| {
        let sessions = sessions.clone();
        let shutdown = shutdown.clone();
        async move {
            let (views, shutdown_view) = match pending {
                Some(snapshot) => snapshot,
                None => match receiver.recv().await {
                    Ok(frame) => {
                        return Some((ObservationEvent::Change(frame), (receiver, None)));
                    }
                    Err(RecvError::Lagged(_)) => {
                        (sessions.snapshot().await, shutdown.snapshot().await)
                    }
                    Err(RecvError::Closed) => return None,
                },
            };
            Some((
                ObservationEvent::Snapshot { sessions: views, shutdown: shutdown_view },
                (receiver, None),
            ))
        }
    })
}
