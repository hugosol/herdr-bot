//! omp-bot 主界面后端。
//!
//! 同一个 Axum 服务提供三件东西：打包后的 React 页面（静态资源）、固定操作 HTTP interface，
//! 以及会话事件 HTTP interface。页面与接口同源同端口，本机与局域网设备访问同一服务。
//!
//! 会话事件接口（见 [`sessions`]）的写入侧是 `POST /api/sessions/events`：扩展上报会话的
//! register / heartbeat / state / unregister，服务端把它们写进**进程内**会话表；同一路径的读侧
//! 长连接由浏览器订阅会话变化。写入只接受本机来源（TCP 对端为回环或本机网卡地址），其余得到
//! `403 forbidden`；读侧对局域网全开、不引入 token。在线与否只由服务端按**两个窗口**判定
//! （缺省在线有效期 10 秒、离线保留期 30 秒，见 [`sessions::SessionPolicy`]）：后台维护把超过在线
//! 有效期的会话翻为离线、把超过两个窗口之和的登记清除，两者都作为可见变化推给浏览器；离线登记在
//! 保留期内保留，显式 `unregister` 立即移除（维护节奏与启动位置见 [`sessions`]）。会话表与固定操作、
//! 关机路径完全解耦——写事件既不取执行资格，也不接触任何脚本。
//!
//! 操作接口只暴露“启动”和“停止”两个固定操作，它们执行哪个脚本、用什么工作目录只由服务端
//! 配置决定。客户端只能提交操作名：请求面是封闭的，其他操作名、怪路径，以及携带命令、脚本
//! 路径或参数的请求都会被拒绝，且拒绝不产生任何脚本执行副作用。
//!
//! 另有两条关机命令：`POST /api/shutdown/start`（手动确认后的开始意图）与
//! `POST /api/shutdown/cancel`（全局取消）。它们不直接关机：开始意图只在后端建立一个**权威倒计时**
//! （见 [`countdown`]），到期后由后端自己调用本机 Windows 优雅关机（见 [`shutdown`]）。
//! 关机与固定操作**完全解耦**——不获取执行资格、不排队、不执行“停止”脚本，因此脚本操作执行期间
//! 它照样被接受；两条命令的请求面同样封闭，带查询串或请求体的调用被拒绝且不产生任何副作用。
//!
//! 还有一条 `POST /api/shutdown/monitor`：为某个会话设置或撤销**完成后关机监听**（明确的目标选中值，
//! 不按当前状态翻转）。集合与自动触发规则同样由 [`countdown`] 在一把锁内裁定：只有当前在线且工作中的
//! 未监听会话可新增，集合非空且全部成员当前等待用户输入才成立唯一的自动安排。
//!
//! 浏览器读取会话与关机状态用同一条只读长连接（见 [`observation`]）：首帧同时给出会话快照与关机状态，
//! 之后关机变化以独立命名事件携带完整状态。没有第二条关机观察连接。
//!
//! 同一实例内任意时刻至多一个脚本操作在执行：两个操作共用一个执行资格，资格从操作被接受到脚本
//! 确认结束一直持有，期间到达的其他请求直接得到忙碌结果（HTTP 409 `operation-in-progress`），
//! 既不等待也不在之后补做。互斥范围限单个后端实例，覆盖整个脚本执行期间，不覆盖脚本另开的长期进程。
//!
//! 执行资格由**执行本身**持有，而不是由提交请求的那条连接持有：客户端中途断开既不取消已接受的操作，
//! 也不提前释放资格，脚本继续执行到它自己的结束；断线只意味着这一次的结果没有接收者（不恢复、不重放、不留操作历史）。

pub mod browse;
pub mod clock;
pub mod config;
pub mod countdown;
pub mod git;
pub mod mdns;
pub mod observation;
pub mod operations;
pub mod sessions;
pub mod shutdown;
pub mod watch;
pub mod workspaces;

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::to_bytes;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path as UrlPath, Query, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, oneshot};
use tower_http::services::ServeDir;

pub use config::{
    Config, CountdownSeconds, MdnsConfig, OperationBinding, Operations, SessionWindowSeconds,
    SessionsConfig, ShutdownConfig, WorkspacesConfig,
};
pub use countdown::{CancelOutcome, MonitorRefusal, ShutdownPhase, ShutdownState, ShutdownView};
pub use operations::{ScriptOperation, console_available};
pub use sessions::SessionPolicy;
pub use shutdown::{PowerOff, PowerOffFailure, SystemPowerOff};

use operations::Invocation;
use workspaces::{RegisteredWorkspace, WorkspaceError};

/// 操作请求的请求体读取上限。操作请求不接受请求体，任何请求体都注定被拒绝，
/// 因此只要够读完空请求体即可，不必为一个越界请求缓冲任意大的内容。
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024;

/// workspace 命令的请求体上限：只含一个服务器目录路径。
const MAX_WORKSPACE_REQUEST_BYTES: usize = 64 * 1024;

/// 提供给 HTTP handler 的运行状态：只有服务端配置绑定的两个固定操作，以及两个操作共用的执行资格。
#[derive(Clone)]
pub struct AppState {
    /// 配置绑定的“启动”操作。
    pub start_operation: ScriptOperation,
    /// 配置绑定的“停止”操作。
    pub stop_operation: ScriptOperation,
    /// 两个固定操作共用的执行资格：同一后端实例内任意时刻**至多一个**脚本操作在执行。
    ///
    /// 互斥量只表达“至多一个持有者”，并且只做“立即尝试获取、绝不等待”：拿不到资格的请求直接得到
    /// 忙碌结果，因此不会在当前操作结束后被补做。持有者从操作被接受到脚本确认结束一直持有资格，
    /// 覆盖整个脚本执行期间（不是只到进程创建完成），而且由**执行这次操作的任务**持有，
    /// 与提交它的 HTTP 连接无关：连接断开时资格继续被持有到脚本确认结束。
    execution: Arc<Mutex<()>>,
    /// 进程内会话表：扩展写事件到这里，浏览器读路径从这里订阅。
    ///
    /// 它与固定操作、关机路径完全解耦：会话写入既不取执行资格，也不接触任何脚本。
    pub sessions: Arc<sessions::SessionsState>,
    /// 后端权威的关机倒计时：手动开始意图建立安排，到期由后端自己调用 [`PowerOff`]。
    ///
    /// 与执行资格完全解耦：既有脚本操作在执行，关机安排照样能被建立，也不执行“停止”脚本。
    /// 实际关机动作接缝由它持有（注入的记录型实现只用于自动化与人工验收的安全外壳）。
    pub shutdown: Arc<countdown::ShutdownState>,
    /// workspace 登记列表：由后端以文件持久保存，电脑与手机共享同一份。
    ///
    /// 与固定操作、关机路径完全解耦：登记只读取与写入登记文件，不执行任何脚本。
    pub workspaces: Arc<workspaces::Workspaces>,
    /// 单个文件文本预览的大小上限（启动期配置，缺省 1 MiB）。
    ///
    /// 浏览读取只读；上限只影响内容预览，不影响目录列表与文件元数据。
    pub preview_limit_bytes: u64,
    /// workspace 文件事件监听与按 workspace 合并的自动刷新通知。
    ///
    /// 每个已登记 workspace 只有一条真实监听和一份节流状态；它只观察本地状态，不触发远端 fetch。
    pub workspace_watch: Arc<watch::WorkspaceWatchManager>,
}

/// 构造使用真实关机动作、新建空会话表的应用：固定操作接口 + 关机端点 + 会话事件接口 + 打包页面。
///
/// 交付运行时不用本函数，而是用 [`app_with_sessions`] 自己持有会话表（于是能把同一份表交给后台过期
/// 扫描，见 `omp_bot::sessions::spawn_expiry_scan`）；本函数固定空表，供只关心路由形状的调用方使用。
pub fn app(config: &Config) -> Router {
    app_with_power_off(config, Arc::new(SystemPowerOff))
}

/// 构造使用**指定**关机动作的应用。
///
/// 交付运行时用 [`app`]（真实的 Win32 动作）；自动化测试用本函数注入记录型动作，于是测试里
/// `POST /api/shutdown` 的语义（200 / 500、请求面拒绝）能被完整驱动，却永远不会真的关机。
/// 两个构造函数注册的路由完全一致：关机端点不是能力接口，也不由任何配置或前端决定是否可用。
/// 会话表用新建的空表，窗口取启动期配置的 `[sessions]`（见 [`app_with_sessions`]）。
pub fn app_with_power_off(config: &Config, power_off: Arc<dyn PowerOff>) -> Router {
    app_with_sessions(
        config,
        power_off,
        Arc::new(sessions::SessionsState::with_policy(config.sessions.policy())),
    )
}

/// 构造使用**指定**关机动作与**指定**会话表的应用。
///
/// 交付运行时（`main.rs`）用本函数持有会话表，并把同一份表交给后台过期扫描；需要观察会话变化的
/// 调用方（会话过期扫描的自动化测试等）注入自己持有的会话表，于是写入的可见变化可以被订阅到，
/// 而路由注册与交付运行时完全一致。调用方持有的会话表自带窗口策略——交付运行时与 [`app_with_power_off`]
/// 都用 `config.sessions.policy()` 建表，因此实例里只有一份启动期读到的窗口。关机倒计时用注入的同一个
/// 动作接缝，时长取启动期配置。
pub fn app_with_sessions(
    config: &Config,
    power_off: Arc<dyn PowerOff>,
    sessions: Arc<sessions::SessionsState>,
) -> Router {
    app_with_state(config, power_off, sessions).0
}

/// 与 [`app_with_sessions`] 完全同一份装配，但把运行状态（[`AppState`]）一并交回调用者。
///
/// 交付运行时（`main.rs`）与安全外壳用它拿到关机倒计时的句柄，再交给时钟驱动的维护循环
/// （`omp_bot::sessions::spawn_expiry_scan`）；于是「过期扫描 + 监听重判」的接线与用例看到的
/// 是同一份，不需要为测试另造一条装配。
pub fn app_with_state(
    config: &Config,
    power_off: Arc<dyn PowerOff>,
    sessions: Arc<sessions::SessionsState>,
) -> (Router, AppState) {
    let shutdown = countdown::ShutdownState::new(
        Arc::clone(&sessions),
        Arc::clone(&power_off),
        config.shutdown.countdown_seconds.duration(),
    );
    let state = AppState {
        start_operation: ScriptOperation::new(
            config.operations.start.script.clone(),
            config.operations.start.working_directory.clone(),
        ),
        stop_operation: ScriptOperation::new(
            config.operations.stop.script.clone(),
            config.operations.stop.working_directory.clone(),
        ),
        execution: Arc::new(Mutex::new(())),
        sessions,
        shutdown,
        workspaces: Arc::new(workspaces::Workspaces::new(
            config.workspaces.registry_file.clone(),
        )),
        preview_limit_bytes: config.workspaces.preview_limit_bytes.get(),
        workspace_watch: Arc::new(watch::WorkspaceWatchManager::new(
            config.workspaces.refresh_interval_seconds.duration(),
        )),
    };

    // 打包页面与操作接口由同一个服务提供：静态目录挂在 fallback 上，
    // "/" 由目录索引提供 index.html；页面没有前端路由，未知路径即 404。
    let page = ServeDir::new(static_dir());

    let router = Router::new()
        // 只有这两个操作名可用；执行目标与工作目录只来自服务端配置。
        .route("/api/operations/start", post(start_operation))
        .route("/api/operations/stop", post(stop_operation))
        // 关机命令永远注册：开始意图只建立后端权威倒计时，取消是全局清空意图。
        // 两条命令都不直接关机，也没有绕过倒计时的即时关机别名。
        .route("/api/shutdown/start", post(start_shutdown))
        .route("/api/shutdown/cancel", post(cancel_shutdown))
        // 完成后关机监听：设置或撤销某个会话的明确目标选中值。
        .route("/api/shutdown/monitor", post(monitor_shutdown))
        // 会话与关机的只读长连接：同一路径上写入（POST，扩展上报的唯一入口）与读取（GET，SSE）各占一半。
        .route(
            "/api/sessions/events",
            get(observation::sse_observation).post(sessions::post_session_event),
        )
        // workspace 登记列表：同一条路径读取与登记，取消登记是另一条明确命令。
        // 请求只携带服务器上的目录路径；执行目标仍只来自服务端配置。
        .route(
            "/api/workspaces",
            get(list_workspaces).post(register_workspace),
        )
        // 只读浏览：列一个目录的直接子项、读取单个文件内容。两者都从登记根重新核对边界。
        .route("/api/workspaces/entries", get(list_workspace_entries))
        .route("/api/workspaces/file", get(read_workspace_file))
        // Git 状态只读查询：当前分支与待提交变更；普通目录明确返回 repository=false。
        .route("/api/workspaces/git-status", get(workspace_git_status))
        // 单个变更的审查 diff：旧侧是 HEAD，新侧是最终本地内容；两侧各自应用预览上限。
        .route("/api/workspaces/diff", get(workspace_change_diff))
        // 登记列表的角标概览：每个 workspace 的待提交项数或明确错误。
        .route("/api/workspaces/overview", get(workspaces_overview))
        // 自动更新的只读长连接：真实文件系统事件按 workspace 合并后通知页面重新读取；
        // 同一 workspace 的所有观察者共享一条监听，监听失效会明确下发错误而不是保持静默。
        .route("/api/workspaces/watch", get(watch_workspaces))
        .route("/api/workspaces/remove", post(unregister_workspace))
        // 其余 /api/ 请求（其他操作名、怪路径）一律以可理解的 JSON 类别拒绝，
        // 既不交给静态页面，也不执行任何脚本。
        .route("/api/{*path}", any(unknown_api_request))
        .fallback_service(page)
        .with_state(state.clone());

    (router, state)
}

/// 打包页面所在目录：前端构建产物（`frontend` 的 `npm run build` 输出）。
pub fn static_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("static")
}

/// 一次已结束的固定操作调用的结果：本次调用已结束的事实、脚本进程的原始观测结果，
/// 以及由它得出的结果类别。
///
/// 结果类别只看脚本进程本身能否启动及其退出结果，不看脚本打印了什么，也不推断后台 Python 进程的
/// 状态：脚本结束不等于后台服务已就绪。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationResponse {
    /// 本次调用的固定操作名：`start` 或 `stop`。
    operation: &'static str,
    /// 本次调用是否已经结束（HTTP 响应返回时必然为 true）。
    ended: bool,
    /// 脚本进程是否成功创建。
    started: bool,
    /// 脚本进程退出码；未创建或非正常退出结束时为 null。
    exit_code: Option<i32>,
    /// 结果类别：脚本成功退出为 `completed`，无法启动或失败退出为 `failed`。
    outcome: OperationOutcome,
    /// 失败的子类别；仅当 `outcome` 为 `failed` 时出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<FailureKind>,
}

/// 一次调用的结果类别。
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum OperationOutcome {
    /// 脚本以成功退出码结束：本次脚本执行完成。
    Completed,
    /// 脚本无法启动，或运行后没有以成功退出码结束。
    Failed,
}

/// 失败结果的子类别；“无法启动”与“运行后失败退出”因此可互相区分。
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum FailureKind {
    /// 脚本无法启动：目标不存在、目标不可执行或工作目录不存在。
    NotStarted,
    /// 脚本进程已创建，但没有以成功退出码结束（非零退出码、没有退出码，或结束结果无法观测）。
    FailedExit,
}

/// 被拒绝的请求：只说明“哪一类请求被拒绝”，不改变、也不回显任何执行目标。
#[derive(Debug, Serialize)]
struct RejectedResponse {
    /// 机器可读的拒绝类别：`unknown-operation`、`unknown-endpoint`、`unexpected-parameters`。
    code: &'static str,
    /// 面向使用者的说明。
    message: &'static str,
}

/// “启动”操作：执行配置绑定的启动批处理。
async fn start_operation(State(state): State<AppState>, request: Request) -> Response {
    run_fixed_operation(request, "start", &state.start_operation, &state.execution).await
}

/// “停止”操作：执行配置绑定的关闭批处理。
async fn stop_operation(State(state): State<AppState>, request: Request) -> Response {
    run_fixed_operation(request, "stop", &state.stop_operation, &state.execution).await
}

/// “手动关机”：确认后提交开始意图，进入后端权威倒计时。
///
/// 它与固定操作的执行资格**完全解耦**：不获取、不释放、不排队，也不执行“停止”脚本，因此脚本操作
/// 执行期间它照样被接受。回执只说明开始意图被接受（`outcome` 是安排当前阶段），**不**表示关机指令
/// 已经发出；到期执行由后端自己负责（见 [`countdown`]）。
async fn start_shutdown(State(state): State<AppState>, request: Request) -> Response {
    // 请求面与其他命令一样封闭，且拒绝发生在建立安排之前：被拒绝的请求不产生任何关机意图。
    if let Some(rejection) = reject_unclosed_request(request).await {
        return rejection;
    }

    let view = state.shutdown.start_manual().await;
    let outcome = view.phase;
    (
        StatusCode::OK,
        Json(StartAccepted {
            outcome,
            shutdown: view,
        }),
    )
        .into_response()
}

/// “取消关机”：全局清空当前监听集合，并撤销当前尚可取消的倒计时。
///
/// 取消作用于**收到请求时**的当前安排，不按调用者看到的安排标识校验（已批准的取舍）。执行已经开始时
/// 得到明确拒绝（HTTP 409 `shutdown-not-cancellable`），前端不得把它显示成取消成功。
async fn cancel_shutdown(State(state): State<AppState>, request: Request) -> Response {
    if let Some(rejection) = reject_unclosed_request(request).await {
        return rejection;
    }

    match state.shutdown.cancel().await {
        CancelOutcome::Cancelled => (
            StatusCode::OK,
            Json(CancelAccepted {
                outcome: CancelOutcome::Cancelled,
                shutdown: state.shutdown.snapshot().await,
            }),
        )
            .into_response(),
        CancelOutcome::NotCancellable => rejected(
            StatusCode::CONFLICT,
            "shutdown-not-cancellable",
            "关机执行已经开始：这次取消不会生效，电脑仍可能关闭。",
        ),
    }
}

/// 一次成功的开始意图：结果类别是安排当前阶段，另附命令之后的权威关机状态。
#[derive(Debug, Serialize)]
struct StartAccepted {
    /// 结果类别：`counting-down`（开始意图已被接受）或 `executing`（到期执行已经开始）。
    outcome: ShutdownPhase,
    /// 命令之后的权威关机状态（与观察流下发的是同一份内容）。
    shutdown: ShutdownView,
}

/// 一次成功的取消：结果类别固定，另附命令之后的权威关机状态。
#[derive(Debug, Serialize)]
struct CancelAccepted {
    /// 结果类别：`cancelled`。
    outcome: CancelOutcome,
    /// 命令之后的权威关机状态（与观察流下发的是同一份内容）。
    shutdown: ShutdownView,
}

/// “完成后关机监听”：为一个会话提交明确的**目标选中值**（选中或撤销），不按当前状态翻转。
///
/// 资格按**命令到达时**的当前会话状态裁定（P7、P44）：页面曾经看到「工作中」不参与判定。被拒绝时给出
/// 明确的类别码（`unknown-session` / `session-not-eligible` / `shutdown-in-progress`），与「没有回应」
/// 的未确认互不混淆（P47）。回执里的关机状态是命令之后的权威集合，因此调用者不必猜自己有没有被接受。
async fn monitor_shutdown(State(state): State<AppState>, request: Request) -> Response {
    // 请求面是封闭的：目标会话与目标选中值只来自请求体；拒绝发生在改动集合之前。
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "监听命令不接受查询参数：目标会话与目标选中值只来自请求体。",
        );
    }

    let (_, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return rejected(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                "监听命令请求体超过 8KB 上限。",
            );
        }
    };

    let Ok(command) = serde_json::from_slice::<MonitoringCommand>(&bytes) else {
        return rejected(
            StatusCode::BAD_REQUEST,
            "invalid-monitoring",
            "监听命令无法解析：请发送 {sessionId, selected} 形式的合法命令。",
        );
    };

    match state
        .shutdown
        .set_monitoring(&command.session_id, command.selected)
        .await
    {
        Ok(shutdown) => (
            StatusCode::OK,
            Json(MonitoringAccepted {
                outcome: if command.selected { SELECTED } else { UNSELECTED },
                shutdown,
            }),
        )
            .into_response(),
        Err(MonitorRefusal::UnknownSession) => rejected(
            StatusCode::NOT_FOUND,
            "unknown-session",
            "该会话尚未注册：监听只针对当前存在的会话。",
        ),
        Err(MonitorRefusal::NotEligible) => rejected(
            StatusCode::CONFLICT,
            "session-not-eligible",
            "该会话当前不满足新增监听的条件：只有在线且工作中的会话可以开始监听。",
        ),
        Err(MonitorRefusal::CountdownActive) => rejected(
            StatusCode::CONFLICT,
            "shutdown-in-progress",
            "已有活动倒计时：期间不接受新增监听，需要先取消当前安排。",
        ),
    }
}

/// 一次监听命令的请求体：明确的目标会话与明确的目标选中值。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonitoringCommand {
    /// 目标会话身份。
    session_id: String,
    /// 目标选中值：`true` 开始监听，`false` 撤销监听。
    selected: bool,
}

/// 监听命令被接受的结果类别：目标选中值已经成立（重申同一目标值也算成立）。
const SELECTED: &'static str = "selected";

/// 监听命令被接受的结果类别：未选中已经成立。
const UNSELECTED: &'static str = "unselected";

/// 一次成功的监听命令：结果类别是目标选中值，另附命令之后的权威关机状态。
#[derive(Debug, Serialize)]
struct MonitoringAccepted {
    /// 结果类别：`selected` 或 `unselected`。
    outcome: &'static str,
    /// 命令之后的权威关机状态（含当前监听集合，与观察流下发的是同一份内容）。
    shutdown: ShutdownView,
}

/// workspace 登记列表的响应：当前完整列表（登记与取消登记也返回同一形状）。
#[derive(Debug, Serialize)]
struct WorkspaceListResponse {
    workspaces: Vec<RegisteredWorkspace>,
}

/// workspace 请求的明确失败：机器可读类别 + 面向使用者的说明。
#[derive(Debug, Serialize)]
struct WorkspaceErrorResponse {
    code: &'static str,
    message: String,
}

/// 一次 workspace 命令的请求体：服务器上的目录路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspacePathRequest {
    path: String,
}

/// “列出 workspace 登记”：读取后端持久保存的同一份列表，不经过任何页面缓存。
async fn list_workspaces(State(state): State<AppState>) -> Response {
    workspace_list_response(state.workspaces.list().await)
}

/// “登记 workspace”：路径必须真实存在且是服务器上的绝对目录；只有持久化成功后返回成功。
async fn register_workspace(State(state): State<AppState>, request: Request) -> Response {
    match workspace_path(request).await {
        Ok(path) => match state.workspaces.register(&path).await {
            Ok(workspaces) => {
                // 新登记的 workspace 立即开始监听；监听失败会作为健康错误交给观察连接。
                state.workspace_watch.reconcile(&workspaces).await;
                workspace_list_response(Ok(workspaces))
            }
            Err(error) => workspace_error_response(error),
        },
        Err(rejection) => rejection,
    }
}

/// “取消登记”：只移除登记项，不删除目录或文件。
async fn unregister_workspace(State(state): State<AppState>, request: Request) -> Response {
    match workspace_path(request).await {
        Ok(path) => match state.workspaces.unregister(&path).await {
            Ok(workspaces) => {
                state.workspace_watch.reconcile(&workspaces).await;
                workspace_list_response(Ok(workspaces))
            }
            Err(error) => workspace_error_response(error),
        },
        Err(rejection) => rejection,
    }
}

/// 读取 workspace 命令的请求体；请求面封闭（不接受查询串或未知字段）。
async fn workspace_path(request: Request) -> Result<String, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "workspace 命令不接受查询参数：路径只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_WORKSPACE_REQUEST_BYTES)
        .await
        .map_err(|_| {
            rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                "workspace 请求体超过 64KB 上限。".to_string(),
            )
        })?;
    let body: WorkspacePathRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析 workspace 命令：请发送 {\"path\":\"服务器目录\"} 形式的 JSON。"
                .to_string(),
        )
    })?;
    Ok(body.path)
}

/// 一次只读浏览请求的查询参数：登记路径 + 可选相对路径（空字符串表示根目录）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowseQuery {
    workspace: String,
    #[serde(default)]
    path: String,
}

/// “列目录”：返回该目录的直接子项；隐藏文件、忽略文件和 Git 元数据都不过滤。
async fn list_workspace_entries(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(rejection) => return rejection,
    };
    let registered = match registered_workspace(&state, &query.workspace).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let path = query.path;
    match tokio::task::spawn_blocking(move || browse::list_directory(&registered, &path)).await {
        Ok(Ok(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(Err(error)) => browse_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "read-failed",
            "读取目录的任务异常结束，无法确认目录内容。".to_string(),
        ),
    }
}

/// “读文件”：返回文件元数据与内容预览；超限或二进制不返回内容，但仍返回元数据。
async fn read_workspace_file(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(rejection) => return rejection,
    };
    let registered = match registered_workspace(&state, &query.workspace).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let path = query.path;
    let limit = state.preview_limit_bytes;
    match tokio::task::spawn_blocking(move || browse::read_file(&registered, &path, limit)).await {
        Ok(Ok(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(Err(error)) => browse_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "read-failed",
            "读取文件的任务异常结束，无法确认文件内容。".to_string(),
        ),
    }
}

/// 一次 Git 状态请求的查询参数：只需要登记路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GitStatusQuery {
    workspace: String,
}

/// workspace 列表上的角标概览：每个登记项的待提交项数或明确错误。
#[derive(Debug, Serialize)]
struct WorkspaceOverviewResponse {
    workspaces: Vec<git::WorkspaceSummary>,
}

/// “读取 workspace 概览”：为登记列表中每个 workspace 给出角标所需的事实。
async fn workspaces_overview(State(state): State<AppState>) -> Response {
    let workspaces = match state.workspaces.list().await {
        Ok(workspaces) => workspaces,
        Err(error) => return workspace_error_response(error),
    };
    match tokio::task::spawn_blocking(move || git::overview(&workspaces)).await {
        Ok(workspaces) => (
            StatusCode::OK,
            Json(WorkspaceOverviewResponse { workspaces }),
        )
            .into_response(),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "git-command-failed",
            "读取 workspace 概览的任务异常结束，无法确认待提交数量。".to_string(),
        ),
    }
}

/// “读取 Git 状态”：当前分支与待提交变更；普通目录返回 `repository: false`。
async fn workspace_git_status(
    State(state): State<AppState>,
    query: Result<Query<GitStatusQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "读取 Git 状态需要 `workspace` 参数，且不接受其他参数。".to_string(),
            )
        }
    };
    if query.workspace.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "workspace-required",
            "请通过 `workspace` 参数指定已登记的 workspace 路径。".to_string(),
        );
    }
    let registered = match registered_workspace(&state, &query.workspace).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    match tokio::task::spawn_blocking(move || git::status(&registered)).await {
        Ok(Ok(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(Err(error)) => git_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "git-command-failed",
            "读取 Git 状态的任务异常结束，无法确认当前分支与待提交变更。".to_string(),
        ),
    }
}

/// “订阅自动更新”：一条只读 SSE 长连接。
///
/// 首帧是全部被监听 workspace 的快照（实际生效的间隔、健康与最近刷新），之后同一 workspace 的所有
/// 观察者共享同一条监听与同一份节流状态，只收到合并后的刷新通知或监听健康变化。
/// 这里只下发本地事件；远端状态仍需页面显式检查，不因本连接自动 fetch。
async fn watch_workspaces(State(state): State<AppState>) -> Response {
    let workspaces = match state.workspaces.list().await {
        Ok(workspaces) => workspaces,
        Err(error) => return workspace_error_response(error),
    };
    // 先订阅再对齐监听：对齐期间新建立的监听健康与刷新都不会漏掉。
    let receiver = state.workspace_watch.subscribe();
    state.workspace_watch.reconcile(&workspaces).await;
    let manager = Arc::clone(&state.workspace_watch);
    let first = manager.snapshot().await;

    let stream = stream::unfold(
        (receiver, Some(first)),
        move |(mut receiver, pending)| {
            let manager = Arc::clone(&manager);
            async move {
                let snapshot = match pending {
                    Some(snapshot) => snapshot,
                    None => match receiver.recv().await {
                        Ok(frame) => {
                            return Some((
                                Ok::<Event, Infallible>(watch::frame_event(frame)),
                                (receiver, None),
                            ))
                        }
                        Err(RecvError::Lagged(_)) => manager.snapshot().await,
                        Err(RecvError::Closed) => return None,
                    },
                };
                Some((
                    Ok(watch::snapshot_event(snapshot)),
                    (receiver, None),
                ))
            }
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

fn git_error_response(error: git::GitError) -> Response {
    let status = match &error {
        git::GitError::WorkspaceUnavailable(_) => StatusCode::NOT_FOUND,
        git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    rejected_owned(status, error.code(), error.message())
}

fn diff_error_response(error: git::DiffError) -> Response {
    match error {
        git::DiffError::Browse(error) => browse_error_response(error),
        git::DiffError::Git(error) => git_error_response(error),
    }
}

/// 一次审查 diff 请求的查询参数：登记路径、新路径与可选的 Git 旧路径。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiffQuery {
    workspace: String,
    path: String,
    #[serde(default)]
    old_path: Option<String>,
}

/// “读取审查 diff”：旧侧取当前分支最新本地提交，新侧取最终本地文件。
///
/// 新增、删除、已识别重命名与部分暂存都使用同一入口；旧路径只用于定位 Git 历史内容，仍先经过
/// workspace 根边界检查。任一侧超限或二进制时返回明确限制，不返回完整内容。
async fn workspace_change_diff(
    State(state): State<AppState>,
    query: Result<Query<DiffQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "审查 diff 需要 `workspace` 与 `path` 参数，可选 `oldPath`，且不接受其他参数。"
                    .to_string(),
            );
        }
    };
    if query.workspace.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "workspace-required",
            "请通过 `workspace` 参数指定已登记的 workspace 路径。".to_string(),
        );
    }
    if query.path.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-query",
            "审查 diff 需要非空的 `path` 参数。".to_string(),
        );
    }

    let registered = match registered_workspace(&state, &query.workspace).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let limit = state.preview_limit_bytes;
    match tokio::task::spawn_blocking(move || {
        git::change_diff(&registered, &query.path, query.old_path.as_deref(), limit)
    })
    .await
    {
        Ok(Ok(diff)) => (StatusCode::OK, Json(diff)).into_response(),
        Ok(Err(error)) => diff_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "git-command-failed",
            "读取审查 diff 的任务异常结束，无法确认旧新内容。".to_string(),
        ),
    }
}

/// 校验查询面：缺失、重复或未知参数都以 JSON 类别拒绝，不猜测请求意图。
fn browse_query(
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Result<BrowseQuery, Response> {
    let Query(query) = query.map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-query",
            "读取 workspace 需要 `workspace` 参数与可选的 `path` 参数，且不接受其他参数。"
                .to_string(),
        )
    })?;
    if query.workspace.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "workspace-required",
            "请通过 `workspace` 参数指定已登记的 workspace 路径。".to_string(),
        ));
    }
    Ok(query)
}

/// 按登记列表核对 workspace；未登记与登记文件读取失败都明确返回，不继续读取文件系统。
async fn registered_workspace(state: &AppState, requested: &str) -> Result<String, Response> {
    match state.workspaces.list().await {
        Ok(workspaces) => match browse::registered_root(&workspaces, requested) {
            Ok(root) => Ok(root.to_string()),
            Err(error) => Err(browse_error_response(error)),
        },
        Err(error) => Err(workspace_error_response(error)),
    }
}

fn browse_error_response(error: browse::BrowseError) -> Response {
    use browse::BrowseError;
    let status = match &error {
        BrowseError::WorkspaceNotRegistered | BrowseError::PathNotFound => StatusCode::NOT_FOUND,
        BrowseError::WorkspaceUnavailable(_) => StatusCode::NOT_FOUND,
        BrowseError::PathOutside | BrowseError::Link { .. } => StatusCode::FORBIDDEN,
        BrowseError::NotADirectory
        | BrowseError::NotAFile
        | BrowseError::InvalidPath(_) => StatusCode::BAD_REQUEST,
        BrowseError::ReadFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())
}

/// 把登记操作的结果翻译成 HTTP 响应：成功返回当前完整列表，失败不伪装成空列表。
fn workspace_list_response(result: Result<Vec<RegisteredWorkspace>, WorkspaceError>) -> Response {
    match result {
        Ok(workspaces) => (
            StatusCode::OK,
            Json(WorkspaceListResponse { workspaces }),
        )
            .into_response(),
        Err(error) => workspace_error_response(error),
    }
}

fn workspace_error_response(error: WorkspaceError) -> Response {
    let status = match &error {
        WorkspaceError::AlreadyRegistered => StatusCode::CONFLICT,
        WorkspaceError::NotRegistered => StatusCode::NOT_FOUND,
        WorkspaceError::Unreadable(_)
        | WorkspaceError::Corrupt(_)
        | WorkspaceError::WriteFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        WorkspaceError::Required
        | WorkspaceError::NotAbsolute
        | WorkspaceError::NotFound
        | WorkspaceError::NotADirectory
        | WorkspaceError::Inaccessible(_) => StatusCode::BAD_REQUEST,
    };
    rejected_owned(status, error.code(), error.message())
}

fn rejected_owned(status: StatusCode, code: &'static str, message: String) -> Response {
    (status, Json(WorkspaceErrorResponse { code, message })).into_response()
}

/// 两个固定操作共用的执行路径。
///
/// 先核对请求面是封闭的（不接受任何参数），再尝试取得两个操作共用的执行资格，最后调用服务端配置
/// 绑定的脚本并等待它结束，按“脚本能否启动 + 退出结果”给出结果类别（完成 / 失败）。
/// 拒绝发生在调用之前，因此被拒绝的请求不会产生任何脚本执行副作用。
///
/// 执行资格覆盖**整个脚本执行期间**：`invoke()` 返回（脚本确认结束）之前一直持有，因此脚本执行期间
/// 到达的其他请求只会得到忙碌结果。资格只尝试获取、不等待，所以这些请求不会被排队补做。
///
/// 资格与执行都放在独立任务里，而不是绑在这次请求的 handler future 上：客户端中途断开时 hyper 会
/// 丢弃 handler future，绑在上面就等于把断线当成“操作结束”——既提前释放资格（重叠请求会真的再执行
/// 一次脚本），也把脚本变成无人等待的进程。放在独立任务里，脚本继续执行到它自己的结束，资格同时保留。
/// 反过来，结果只沿这一次请求的通道送回：连接已经断开时接收端不存在，结果被丢弃而已，
/// 不做结果恢复、不留操作历史、也不重放。
async fn run_fixed_operation(
    request: Request,
    name: &'static str,
    operation: &ScriptOperation,
    execution: &Arc<Mutex<()>>,
) -> Response {
    if let Some(rejection) = reject_unclosed_request(request).await {
        return rejection;
    }

    // 已有脚本操作在执行：直接给出忙碌结果，既不等待，也不为此后留下任何待执行的操作。
    let Ok(execution_permit) = Arc::clone(execution).try_lock_owned() else {
        return rejected(
            StatusCode::CONFLICT,
            "operation-in-progress",
            "已有脚本操作正在执行：本次请求没有被执行，也不会在当前操作结束后被补做。",
        );
    };

    let operation = operation.clone();
    let (result_sender, result_receiver) = oneshot::channel();
    tokio::spawn(async move {
        // 资格与执行同一生命周期：本任务活到脚本确认结束，资格也保留到那时。
        let _execution_permit = execution_permit;
        let response = operation_response(name, &operation, operation.invoke().await);
        // 只有这一次请求能得到结果；没有接收者（连接已断开）时结果就此丢弃。
        let _ = result_sender.send(response);
    });

    match result_receiver.await {
        Ok(response) => Json(response).into_response(),
        // 执行任务没有送回结果（任务未能走到结尾，例如 panic）：不能凭它编造执行结果，
        // 只报告服务端没能给出结果。
        Err(_) => rejected(
            StatusCode::INTERNAL_SERVER_ERROR,
            "operation-result-unavailable",
            "本次操作没有送回结果：脚本的执行结果无法确认。",
        ),
    }
}

/// 把一次调用的原始观测结果转成结果类别，并写一条诊断输出。
fn operation_response(
    name: &'static str,
    operation: &ScriptOperation,
    invocation: Invocation,
) -> OperationResponse {
    match invocation {
        // 脚本进程已创建并结束：只有成功退出码算“执行完成”，其余都是“执行失败”。
        Invocation::Ended(status) => {
            eprintln!(
                "[omp-bot] {name} 调用已结束：{}，{status}",
                operation.script().display()
            );
            let exit_code = status.code();
            let succeeded = exit_code == Some(0);
            OperationResponse {
                operation: name,
                ended: true,
                started: true,
                exit_code,
                outcome: if succeeded {
                    OperationOutcome::Completed
                } else {
                    OperationOutcome::Failed
                },
                failure: (!succeeded).then_some(FailureKind::FailedExit),
            }
        }
        // 进程已创建，但结束结果无法观测：不能当作成功退出。
        Invocation::EndUnobserved(error) => {
            eprintln!(
                "[omp-bot] {name} 调用已结束：无法观测脚本进程的结束结果（{}）：{error}",
                operation.script().display()
            );
            OperationResponse {
                operation: name,
                ended: true,
                started: true,
                exit_code: None,
                outcome: OperationOutcome::Failed,
                failure: Some(FailureKind::FailedExit),
            }
        }
        // 目标没能启动：没有脚本进程，属于“执行失败”里的“无法启动”。
        Invocation::NotStarted(reason) => {
            eprintln!(
                "[omp-bot] {name} 调用已结束：脚本无法启动（{}，工作目录 {}）：{reason}",
                operation.script().display(),
                operation.working_directory().display()
            );
            OperationResponse {
                operation: name,
                ended: true,
                started: false,
                exit_code: None,
                outcome: OperationOutcome::Failed,
                failure: Some(FailureKind::NotStarted),
            }
        }
    }
}

/// 封闭请求面检查：固定操作只接受不带查询串、不带请求体的 POST。
///
/// 客户端没有指定执行目标、命令或参数的能力，所以任何查询参数或请求体都是越界请求，
/// 一律按同一个类别拒绝。
async fn reject_unclosed_request(request: Request) -> Option<Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "操作请求不接受查询参数：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
        ));
    }

    let (_, body) = request.into_parts();
    match to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) if bytes.is_empty() => None,
        // 读到内容、读不完（超出上限）或读取失败，都说明这不是一个封闭的请求。
        _ => Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "操作请求不接受请求体：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
        )),
    }
}

/// 固定操作之外的所有 `/api/` 请求：以 JSON 类别拒绝，不执行任何脚本，也不交给静态页面。
async fn unknown_api_request(UrlPath(path): UrlPath<String>) -> Response {
    if path == "operations" || path.starts_with("operations/") {
        rejected(
            StatusCode::NOT_FOUND,
            "unknown-operation",
            "本服务只提供“启动”与“停止”两个固定操作，操作名与脚本位置不能由客户端指定。",
        )
    } else {
        rejected(
            StatusCode::NOT_FOUND,
            "unknown-endpoint",
            "该接口路径不存在。",
        )
    }
}

/// 构造一次拒绝响应：请求面越界（`400` / `404`）、已有脚本操作在执行（`409`），
/// 或执行任务没能送回结果（`500`，内部故障）。
/// 这些路径都不接受请求、不排队，也从不接触服务端配置绑定的脚本。
fn rejected(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    eprintln!(
        "[omp-bot] 请求被拒绝：{code}（HTTP {}）",
        status.as_u16()
    );
    (status, Json(RejectedResponse { code, message })).into_response()
}
