//! 会话事件的读写两端与会话内存态。
//!
//! omp-bot 服务端的「会话」第一次在这里落地：扩展把每个会话的生命周期事件（`register` /
//! `heartbeat` / `state` / `unregister`）都发到同一条 `POST /api/sessions/events`，
//! 服务端把它们写进**进程内**的会话表，并在同一把锁内把浏览器可见的变化发布给订阅者
//! （读路径只做只读转发，不重放、不落盘，omp-bot 重启即清空）。
//!
//! 事件信封是 `{sessionId, kind, data}`：没有版本位、没有时间戳，`kind` 是封闭枚举，未知字段一律忽略。
//! `register` 是**唯一**的建行入口且幂等——重复注册不新起行，只刷新注册时间与展示字段快照；
//! 非 `register` 事件引用未知会话时得到 `404 unknown-session`（扩展据此判别 omp-bot 重启过）。
//!
//! 在线与否只由服务端判定：[`is_online`] 是唯一判定式，[`ONLINE_WINDOW`] 是唯一的阈值常量，
//! 快照（读路径）与过期扫描共用，避免两处阈值分叉。过期扫描（[`SessionsState::scan_expired`]，
//! 节奏见 [`EXPIRY_SCAN_INTERVAL`]）把超时的在线行翻为离线并广播这一可见变化；离线行保留、不老化，
//! 只在显式 `unregister` 时移除。
//!
//! 写入侧做的是**来源校验、建行、改行与错误分类**：写入只接受本机来源（TCP 对端为回环或本机网卡
//! 地址，见 [`is_local_source`]），否则 `403 forbidden`——这是会话写入的访问边界（局域网设备不能
//! 伪造或刷面板），不是认证：同机任意进程仍可写。拒绝类别复用服务既有的 `{code, message}` 形状，
//! 每一条拒绝都发生在建行或改动任何会话之前。
//!
//! 读侧是同一路径上的 `GET` 长连接（`text/event-stream`）：每次（重）连的第一帧是**全量快照**，
//! 之后只推增量。读侧不读来源，对局域网全开、不引入 token。两个方向共用那把锁（读侧先订阅、再读表；
//! 写侧在锁内广播），因此读连接既不需要版本位，也不需要游标或事件日志——重连即收敛。订阅者落后太多时
//! 丢弃错过的增量、重发一帧快照。

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::to_bytes;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;

use crate::{AppState, MAX_REQUEST_BODY_BYTES, rejected};

/// 在线判定窗口：最后一次写入起 30s 内视为在线。
///
/// 快照（读路径）与过期扫描共用这一个常量，阈值不会在两处分叉。
pub const ONLINE_WINDOW: Duration = Duration::from_secs(30);

/// 会话是否在线：`now - last_seen <= 30s`。
///
/// 只有服务端判定在线/离线：任何被接受的写事件刷新 `last_seen`，快照与过期扫描都经本函数现算。
pub fn is_online(last_seen: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_seen) <= ONLINE_WINDOW
}

/// 过期扫描的节奏：后台任务每 5s 遍历一次内存会话表。
///
/// 它与 [`ONLINE_WINDOW`] 一起给出「离线」这一可见变化的上界：最后一次写入起最迟
/// `ONLINE_WINDOW + EXPIRY_SCAN_INTERVAL`（35s）就会出现离线的 `upsert`（进程死亡另加这一个扫描
/// 周期，最坏约 45s）。节奏不再收紧——更频繁的扫描只让上界变小，不改变判定。
pub const EXPIRY_SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// agent 状态：恰三个值的封闭枚举。
///
/// 缺省（`None`）表示服务端还没收到任何状态，与三个取值都不是一回事；不引入 `unknown` 哨兵。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StateValue {
    /// 会话在线、agent 没在跑，且本次接入后尚未观察到轮次结束。
    Idle,
    /// agent 正在执行一轮对话。
    Working,
    /// 一次轮次已经结束，会话停下来等用户回应。
    AwaitingInput,
}

/// 浏览器可见的会话视图：面板上一张会话卡显示的全部字段。
///
/// 可空字段缺省即**省略**（不发 `null`）；`pid` 与 `sessionFile` 只存在会话表里备查，不进视图，
/// 因此也不进任何下发帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    /// 会话身份，来自扩展上报的 `sessionId`。
    pub session_id: String,
    /// 会话名，来自注册快照；为空即省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    /// 会话的工作目录，注册时必填。
    pub cwd: String,
    /// 模型的展示名，来自注册快照；为空即省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 服务端判定的 keepAlive：仍在收到该会话的定时心跳。
    pub online: bool,
    /// agent 状态；缺省表示还没收到过状态。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<StateValue>,
}

/// 发布给订阅者的一帧：写入侧只产生增量帧，全量快照帧由读路径在同一把锁下自己组装。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// 某个会话被建行或发生了变化。
    Upsert(SessionView),
    /// 某个会话被注销。
    Remove {
        /// 被注销的会话身份。
        session_id: String,
    },
}

/// 会话表里的一行：展示字段、存下备查的字段，以及生命周期字段。
///
/// `session_file` 与 `pid` 只存下来便于诊断，不下发；`registered_at` 每次 `register` 刷新，
/// 即「注册顺序」的依据；`online` 与 [`is_online`] 的判定保持一致，同时是「这个值已经广播过」的标记
/// （见 [`SessionsState::scan_expired`]）——视图里的在线与否仍按 [`SessionRow::view`] 现算。
struct SessionRow {
    session_id: String,
    cwd: String,
    session_name: Option<String>,
    session_file: Option<String>,
    model: Option<String>,
    pid: Option<u32>,
    state: Option<StateValue>,
    last_seen: Instant,
    online: bool,
    registered_at: Instant,
}

impl SessionRow {
    /// 从当前行取出浏览器可见视图。
    ///
    /// `online` 按 [`is_online`] 现算（与快照同一判定），因此过期扫描还没来得及把旗标翻成离线时，
    /// 视图也不会假「在线」。
    fn view(&self, now: Instant) -> SessionView {
        SessionView {
            session_id: self.session_id.clone(),
            session_name: self.session_name.clone(),
            cwd: self.cwd.clone(),
            model: self.model.clone(),
            online: is_online(self.last_seen, now),
            state: self.state,
        }
    }
}

/// 进程内会话表与广播通道。
///
/// 会话表放一把 `tokio::sync::Mutex`：写入与（读路径的）订阅 + 快照在同一把锁下完成，因此帧不需要
/// 版本位、前端不需要重放。临界区只做 map 读写与广播发送，**绝不跨 `.await` 持锁**。
pub struct SessionsState {
    inner: Mutex<Inner>,
    tx: broadcast::Sender<Frame>,
}

struct Inner {
    rows: HashMap<String, SessionRow>,
}

/// 一把锁的临界区内取出的全量视图：按注册顺序升序。
///
/// 顺序的唯一依据是 `registered_at`（每次 `register` 刷新），因此重注册会把该行移到末位；同一瞬间
/// 注册的两行按 `sessionId` 升序，顺序于是不随 `HashMap` 的迭代顺序变化。tier 排序（在线等待输入置顶、
/// 离线沉底）是前端的事，服务端只给注册顺序。
fn snapshot_locked(inner: &Inner, now: Instant) -> Vec<SessionView> {
    let mut rows: Vec<&SessionRow> = inner.rows.values().collect();
    rows.sort_by(|left, right| {
        left.registered_at
            .cmp(&right.registered_at)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    rows.into_iter().map(|row| row.view(now)).collect()
}

/// 每个订阅者的有界帧缓冲：落后太多时丢弃增量（读路径用重发快照收敛），不拖慢写入方。
const FRAME_BUFFER: usize = 64;

impl SessionsState {
    /// 空会话表：omp-bot 启动时没有任何会话，v1 不落盘。
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(FRAME_BUFFER);
        Self {
            inner: Mutex::new(Inner {
                rows: HashMap::new(),
            }),
            tx,
        }
    }

    /// 订阅浏览器可见的变化帧。每个读连接各有一条独立的有界缓冲。
    pub fn subscribe(&self) -> broadcast::Receiver<Frame> {
        self.tx.subscribe()
    }

    /// 当前全部会话的最新视图，按注册顺序升序。
    ///
    /// 在线与否按 [`is_online`] 现算，因此订阅者落后太多时重发的快照不会假「在线」，也不需要过期
    /// 扫描先跑过一轮。
    async fn snapshot(&self) -> Vec<SessionView> {
        let now = Instant::now();
        let inner = self.inner.lock().await;
        snapshot_locked(&inner, now)
    }

    /// 订阅读连接，并**在同一把锁的临界区内**取出当时的全量快照。
    ///
    /// 两个动作同在一个临界区，而写入也在这把锁内广播，因此这条读连接的「快照」与「随后收到的帧」
    /// 既不漏也不重：快照之后收到的每一帧都发生在它之后，快照之前的一切都已经包含在快照里。
    /// 于是读路径不需要版本位、不需要事件日志，也不需要 `Last-Event-ID` 之类的游标。
    pub async fn subscribe_with_snapshot(&self) -> (broadcast::Receiver<Frame>, Vec<SessionView>) {
        let now = Instant::now();
        let inner = self.inner.lock().await;
        let receiver = self.tx.subscribe();
        let snapshot = snapshot_locked(&inner, now);
        (receiver, snapshot)
    }

    /// 把与 [`is_online`] 判定不一致的行翻正，并把这次可见变化广播出去。
    ///
    /// 判定只有一处：[`is_online`]（30s 窗口）。扫描只做「把行的 `online` 旗标对齐到这个判定」，
    /// 因此不会出现第二处阈值，也不会对仍在窗口内的行误翻。
    ///
    /// `now` 由调用方**注入**：生产路径每 [`EXPIRY_SCAN_INTERVAL`] 传当前时刻，自动化用例直接把时刻
    /// 推进到窗口之外，于是过期的观察不需要真的等 30s 过去。
    ///
    /// 只广播**变化**：窗口内的行什么也不发，已经翻过的行再扫一遍也不重复发——行自己的 `online` 就是
    /// 「这个值已经广播过」的标记。反过来，刷新与恢复在线是写入路径的事：任何被接受的写事件都把
    /// `last_seen` 与 `online` 一起刷到当下。
    ///
    /// 离线行不老化、不移除：行只在收到 `unregister` 时消失（v1 也不落盘）。
    pub async fn scan_expired(&self, now: Instant) {
        // 整次扫描在一把锁内完成：map 读写与广播发送都不跨 `.await`，广播顺序与可见变化的发生顺序一致。
        let mut inner = self.inner.lock().await;
        let mut flipped = Vec::new();
        for row in inner.rows.values_mut() {
            let online = is_online(row.last_seen, now);
            if row.online == online {
                continue;
            }
            row.online = online;
            flipped.push(row.view(now));
        }
        for view in flipped {
            let _ = self.tx.send(Frame::Upsert(view));
        }
    }

    /// 应用一次事件。
    ///
    /// 返回 `None` 表示这是引用未知会话的非 `register` 事件：调用方据此回 `404 unknown-session`，
    /// 且**不建行**（`register` 是唯一建行入口）。
    async fn apply(&self, event: SessionEvent) -> Option<Applied> {
        // 一次事件只取一次时间：注册时间与最后写入时间取自同一刻。
        let now = Instant::now();
        // 整次事件在一把锁内完成：map 读写与广播发送都不跨 `.await`。
        let mut inner = self.inner.lock().await;

        match event {
            SessionEvent::Register { session_id, data } => {
                let RegisterData {
                    cwd,
                    session_name,
                    session_file,
                    model,
                    pid,
                } = data;
                let frame = match inner.rows.get_mut(&session_id) {
                    // 幂等重注册：不新起行，只刷新注册时间与展示字段快照。
                    Some(row) => {
                        let before = row.view(now);
                        row.cwd = cwd;
                        row.session_name = session_name;
                        row.session_file = session_file;
                        row.model = model;
                        row.pid = pid;
                        row.registered_at = now;
                        row.last_seen = now;
                        row.online = true;
                        // `register` 不带状态，因此已有状态原样保留：重注册不让面板回到「状态同步中」。
                        let after = row.view(now);
                        (after != before).then_some(Frame::Upsert(after))
                    }
                    None => {
                        let row = SessionRow {
                            session_id: session_id.clone(),
                            cwd,
                            session_name,
                            session_file,
                            model,
                            pid,
                            state: None,
                            last_seen: now,
                            online: true,
                            registered_at: now,
                        };
                        let frame = Frame::Upsert(row.view(now));
                        inner.rows.insert(session_id, row);
                        Some(frame)
                    }
                };
                if let Some(frame) = frame {
                    let _ = self.tx.send(frame);
                }
                let online_count = inner
                    .rows
                    .values()
                    .filter(|row| is_online(row.last_seen, now))
                    .count();
                Some(Applied::Registered { online_count })
            }
            SessionEvent::Heartbeat { session_id, data } => {
                let row = inner.rows.get_mut(&session_id)?;
                let before = row.view(now);
                row.last_seen = now;
                row.online = true;
                // 心跳携带的状态与 `state` 事件同一处理：同值即无变化，不广播。
                if let Some(value) = data.value {
                    row.state = Some(value);
                }
                let after = row.view(now);
                if after != before {
                    let _ = self.tx.send(Frame::Upsert(after));
                }
                Some(Applied::Accepted)
            }
            SessionEvent::State { session_id, data } => {
                let row = inner.rows.get_mut(&session_id)?;
                let before = row.view(now);
                row.last_seen = now;
                row.online = true;
                row.state = Some(data.value);
                let after = row.view(now);
                if after != before {
                    let _ = self.tx.send(Frame::Upsert(after));
                }
                Some(Applied::Accepted)
            }
            SessionEvent::Unregister { session_id } => {
                inner.rows.remove(&session_id)?;
                let _ = self.tx.send(Frame::Remove { session_id });
                Some(Applied::Accepted)
            }
        }
    }
}

/// 启动过期扫描：每 [`EXPIRY_SCAN_INTERVAL`] 用**当前时刻**跑一遍 [`SessionsState::scan_expired`]。
///
/// 由主入口在监听绑定成功之后调用，router 构造不 spawn 任何任务：构造一个应用不产生后台行为，
/// 扫描是「服务已经在监听」之后的事。
///
/// 扫描是纯内存遍历（没有 I/O、没有 `Result`），构造上不可失败，因此循环里没有错误分支，也不需要
/// watchdog；服务器关停时随运行时一起结束。
pub fn spawn_expiry_scan(sessions: Arc<SessionsState>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(EXPIRY_SCAN_INTERVAL);
        loop {
            ticker.tick().await;
            sessions.scan_expired(Instant::now()).await;
        }
    })
}

/// 一次被接受的事件给调用者的结果。
enum Applied {
    /// `register`：回带当前在线会话数（含本会话）。
    Registered {
        /// 在线会话数。
        online_count: usize,
    },
    /// 其余 kind：只回「事件已被接受」。
    Accepted,
}

/// 事件信封：`{sessionId, kind, data}`。
///
/// `kind` 是封闭枚举（`register` / `heartbeat` / `state` / `unregister`），`data` 的形状按 kind 而定；
/// 没有版本位、没有时间戳，未知字段一律忽略（将来加字段不破坏旧扩展）。
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum SessionEvent {
    /// 注册（或幂等重注册）一个会话；唯一的建行入口。
    Register {
        /// 会话身份。
        #[serde(rename = "sessionId")]
        session_id: String,
        /// 注册字段。
        data: RegisterData,
    },
    /// 心跳：刷新最后写入时间，并（若带状态）把状态幂等 upsert 进会话行。
    Heartbeat {
        /// 会话身份。
        #[serde(rename = "sessionId")]
        session_id: String,
        /// 心跳字段；可整省（视为 `{}`）。
        #[serde(default)]
        data: HeartbeatData,
    },
    /// 状态变化：agent 状态三态之一，必填。
    State {
        /// 会话身份。
        #[serde(rename = "sessionId")]
        session_id: String,
        /// 状态字段。
        data: StateData,
    },
    /// 注销：移除会话行；只带会话身份，不带其它字段。
    Unregister {
        /// 会话身份。
        #[serde(rename = "sessionId")]
        session_id: String,
    },
}

/// `register` 的请求字段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterData {
    /// 会话的工作目录，必填。
    cwd: String,
    /// 会话名；缺省即省略。
    session_name: Option<String>,
    /// 会话文件路径；存下备查，不下发帧。
    session_file: Option<String>,
    /// 模型的展示名（注册时快照）；缺省即省略。
    model: Option<String>,
    /// 扩展进程的 PID；存下备查，不下发帧。
    pid: Option<u32>,
}

/// `heartbeat` 的请求字段。
#[derive(Debug, Default, Deserialize)]
struct HeartbeatData {
    /// 当前状态；缺省表示只刷心跳、不改状态。
    value: Option<StateValue>,
}

/// `state` 的请求字段。
#[derive(Debug, Deserialize)]
struct StateData {
    /// 新状态，必填。
    value: StateValue,
}

/// `register` 的成功结果类别。
const REGISTERED: &'static str = "registered";

/// 其余 kind 的成功结果类别。
const ACCEPTED: &'static str = "accepted";

/// `register` 的成功回执：回带当前在线会话数，于是扩展 arm 时一次往返就能显示「当前 N 个在线会话」。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterAccepted {
    /// 结果类别：`registered`。
    outcome: &'static str,
    /// 当前在线会话数（含本会话）。
    online_count: usize,
}

/// 其余 kind 的成功回执：只说明「事件已被接受」。
#[derive(Debug, Serialize)]
struct EventAccepted {
    /// 结果类别：`accepted`。
    outcome: &'static str,
}

/// 写入边界的唯一判定式：TCP 对端地址是否为本机来源。
///
/// `is_loopback()` 覆盖回环（`127.0.0.0/8` 与 `::1`）；其余地址现用 [`if_addrs::get_if_addrs`]
/// 枚举本机网卡地址集合——每次写入都现枚举，DHCP 换址后旧地址不会残留成误放行的口子。枚举失败
/// （本机没有可读的网卡信息）时按「不是本机」拒绝：边界默认关闭。
///
/// 同机经 mDNS 主机名接入时源地址就是本机网卡地址，因此这一条同时保住了默认端点
/// `omp-bot.local:8787` 的同机可用性。
fn is_local_source(ip: IpAddr) -> bool {
    ip.is_loopback()
        || if_addrs::get_if_addrs().is_ok_and(|interfaces| {
            interfaces
                .iter()
                .any(|interface| interface.addr.ip() == ip)
        })
}

/// `POST /api/sessions/events`：会话事件的唯一写入入口。
///
/// 请求面是封闭的：只接受本机来源，不接受查询参数，请求体必须是 `{sessionId, kind, data}` 形式的
/// 事件。成功回执只说明「事件已被接受」——`register` 回带在线会话数，其余 kind 只回 `accepted`；
/// 被拒绝的请求不建行、不改动任何会话。
///
/// 拒绝类别（复用服务既有的 `{code, message}` 形状，kebab-case code、中文 message）：
/// `403 forbidden`（来源不是本机）、`413 payload-too-large`（请求体超过 8KB）、
/// `400 unexpected-parameters`（带查询串）、`400 invalid-event`（解析失败 / 缺字段 / 非法 kind /
/// 非法状态值）、`404 unknown-session`（非 register 事件引用未知会话）。
pub(crate) async fn post_session_event(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    // 来源校验先于一切：局域网设备不能伪造或刷会话行。它发生在读请求体、建行或改动任何会话之前。
    if !is_local_source(peer.ip()) {
        return rejected(
            StatusCode::FORBIDDEN,
            "forbidden",
            "会话事件写入只接受本机来源：局域网设备不能伪造或刷面板。",
        );
    }

    // 事件内容只来自请求体：带查询串的调用一律按越界请求拒绝。
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "会话事件请求不接受查询参数：事件内容只来自请求体。",
        );
    }

    let (_, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        // 读不完请求体只可能是超过上限：不为越界的请求缓冲任意大的内容。
        Err(_) => {
            return rejected(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                "会话事件请求体超过 8KB 上限。",
            );
        }
    };

    let Ok(event) = serde_json::from_slice::<SessionEvent>(&bytes) else {
        return rejected(
            StatusCode::BAD_REQUEST,
            "invalid-event",
            "会话事件无法解析：请发送 {sessionId, kind, data} 形式的合法事件。",
        );
    };

    match state.sessions.apply(event).await {
        Some(Applied::Registered { online_count }) => (
            StatusCode::OK,
            Json(RegisterAccepted {
                outcome: REGISTERED,
                online_count,
            }),
        )
            .into_response(),
        Some(Applied::Accepted) => (
            StatusCode::OK,
            Json(EventAccepted {
                outcome: ACCEPTED,
            }),
        )
            .into_response(),
        None => rejected(
            StatusCode::NOT_FOUND,
            "unknown-session",
            "该会话尚未注册：非 register 事件不能为未知会话建行。",
        ),
    }
}

/// 重连提示：每次（重）连的**首帧**带上它，浏览器断线后约 2s 重新连上。
///
/// 首帧是快照，提示就挂在同一帧上（SSE 允许 `retry` 与 `event`/`data` 同帧），于是「第一帧必为
/// `snapshot`」与「首帧带重连提示」同时成立，不需要先发一帧什么也不带的控制帧。
const SSE_RETRY: Duration = Duration::from_secs(2);

/// 长连接的 keep-alive 间隔：没有帧可推时约每 15s 发一个注释帧，只读连接不被中间设备掐断。
///
/// keep-alive 只是心跳，不携带任何会话内容，也不是命名事件：页面不因它改变视图。
const SSE_KEEP_ALIVE: Duration = Duration::from_secs(15);

/// 命名事件 `snapshot`：每次（重）连的第一帧，全部会话的最新视图。
const SNAPSHOT_EVENT: &str = "snapshot";

/// 命名事件 `upsert`：某个会话被建行、注销之外地发生了变化。
const UPSERT_EVENT: &str = "upsert";

/// 命名事件 `remove`：某个会话被注销。
const REMOVE_EVENT: &str = "remove";

/// `snapshot` 帧的内容：全部会话的最新视图。
#[derive(Debug, Serialize)]
struct SnapshotData {
    /// 全部会话的视图，按注册顺序升序。
    sessions: Vec<SessionView>,
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

/// 帧流的一项：帧的组装不可能失败，因此流的错误类型是 [`Infallible`]。
fn stream_item(event: Event) -> Result<Event, Infallible> {
    Ok(event)
}

/// 组装一帧命名事件。
///
/// 会话帧只含字符串、布尔与三态枚举，序列化不可能失败；帧里没有版本位、没有时间戳，
/// 也没有 `id:` 游标字段。
fn named_event(name: &'static str, data: &impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(data)
        .expect("会话帧只含字符串与布尔，序列化不会失败")
}

/// 把写入侧发布的一帧增量转成下发帧。
fn frame_event(frame: Frame) -> Event {
    match frame {
        Frame::Upsert(view) => named_event(UPSERT_EVENT, &UpsertData { session: view }),
        Frame::Remove { session_id } => named_event(REMOVE_EVENT, &RemoveData { session_id }),
    }
}

/// 把一份快照转成下发帧。
///
/// 首帧与「订阅者落后太多」的重发走的是同一个形状，因此重连与重发在页面上是同一条收敛路径。
fn snapshot_event(sessions: Vec<SessionView>) -> Event {
    named_event(SNAPSHOT_EVENT, &SnapshotData { sessions }).retry(SSE_RETRY)
}

/// `GET /api/sessions/events`：浏览器侧的**只读**长连接（`text/event-stream`）。
///
/// 每次（重）连的第一帧都是全量 `snapshot`，之后只推 `upsert` / `remove` 增量：页面不需要事件日志、
/// 不需要 `Last-Event-ID` 或任何游标，断线期间错过的事件也不需要补——重连（或落后太多时重发的快照）
/// 就足以收敛到最新视图。服务端不重放、不落盘，omp-bot 重启即清空。
///
/// 首帧与订阅在同一把锁下取出（见 [`SessionsState::subscribe_with_snapshot`]），所以快照与随后的增量
/// 既不漏也不重。订阅者落后太多（[`RecvError::Lagged`]）时丢弃错过的增量、重发一帧新的快照，
/// 而不是让读连接去追赶历史。
pub(crate) async fn sse_session_events(State(state): State<AppState>) -> Response {
    let sessions = state.sessions.clone();
    let (receiver, snapshot) = sessions.subscribe_with_snapshot().await;
    let stream = stream::unfold(
        (receiver, Some(snapshot)),
        move |(mut receiver, pending)| {
            let sessions = sessions.clone();
            async move {
                let views = match pending {
                    Some(views) => views,
                    None => match receiver.recv().await {
                        Ok(frame) => {
                            return Some((stream_item(frame_event(frame)), (receiver, None)));
                        }
                        // 有界缓冲溢出：错过的增量无法补，用一个新快照让读者直接收敛到当下。
                        Err(RecvError::Lagged(_)) => sessions.snapshot().await,
                        // 写入侧已全部消失（服务端关停）：这条只读长连接到此结束。
                        Err(RecvError::Closed) => return None,
                    },
                };
                Some((stream_item(snapshot_event(views)), (receiver, None)))
            }
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(SSE_KEEP_ALIVE))
        .into_response()
}
