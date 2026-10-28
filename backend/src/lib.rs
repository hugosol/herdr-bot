//! herdr-bot 主界面后端。
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

pub mod chats;
pub mod browse;
pub mod catalog;
pub mod clock;
pub mod commit;
pub mod config;
pub mod countdown;
pub mod git;
pub mod herdr;
pub mod history;
pub mod resume;
pub mod mdns;
pub mod observation;
pub mod operations;
pub mod sessions;
pub mod scheduler;
pub mod runs;
pub mod files;
pub mod shutdown;
mod status_cache;
pub mod sync;
pub mod tasks;
pub mod watch;
pub mod zone_create;
pub mod zones;

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path as UrlPath, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, oneshot};
use tower_http::services::ServeDir;

pub use config::{
    TasksConfig, CommandTimeoutSeconds, Config, CountdownSeconds, GitConfig, HerdrConfig,
    MdnsConfig, OperationBinding, Operations, SessionWindowSeconds, SessionsConfig, ShutdownConfig,
    StatusCacheTtlSeconds, ZonesConfig, SchedulerConfig,
};
pub use countdown::{CancelOutcome, MonitorRefusal, ShutdownPhase, ShutdownState, ShutdownView};
pub use operations::{ScriptOperation, console_available};
pub use sessions::SessionPolicy;
pub use shutdown::{PowerOff, PowerOffFailure, SystemPowerOff};

use operations::Invocation;
use zones::{RegisteredZone, ZoneError};

/// 操作请求的请求体读取上限。操作请求不接受请求体，任何请求体都注定被拒绝，
/// 因此只要够读完空请求体即可，不必为一个越界请求缓冲任意大的内容。
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024;

/// zone 命令的请求体上限：只含一个服务器目录路径。
const MAX_ZONE_REQUEST_BYTES: usize = 64 * 1024;

/// Task 请求的请求体上限：固定 prompt 可多行，给足以容纳正常文本。
const MAX_TASK_REQUEST_BYTES: usize = 1024 * 1024;

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
    /// 持久历史目录，与当前会话登记和临时关机意图独立。
    pub history: Arc<history::History>,
    /// 后端权威的关机倒计时：手动开始意图建立安排，到期由后端自己调用 [`PowerOff`]。
    ///
    /// 与执行资格完全解耦：既有脚本操作在执行，关机安排照样能被建立，也不执行“停止”脚本。
    /// 实际关机动作接缝由它持有（注入的记录型实现只用于自动化与人工验收的安全外壳）。
    pub shutdown: Arc<countdown::ShutdownState>,
    /// zone 登记列表：由后端以文件持久保存，电脑与手机共享同一份。
    ///
    /// 与固定操作、关机路径完全解耦：登记只读取与写入登记文件，不执行任何脚本。
    pub zones: Arc<zones::Zones>,
    /// Task：由后端以文件持久保存，电脑与手机共享同一份。
    ///
    /// 与固定操作、关机路径完全解耦：Task 管理只读取与写入 Task 存储文件，不执行任何脚本、
    /// 不创建也不关闭 Chat。
    pub tasks: Arc<tasks::Tasks>,
    /// Service-owned Map root, never derived from zone or request data.
    pub map_directory: PathBuf,
    /// Service-owned shared capability definitions, independent of Map storage.
    pub bot_directory: PathBuf,
    pub run_directory: PathBuf,
    pub run_instructions: PathBuf,
    pub runs: Arc<runs::Runs>,
    /// Herdr 公开控制接口的生产 adapter；默认 Workspace 名称来自启动期配置，当前模式按 zone 根目录名匹配。
    ///
    /// 每个创建各自调用它，不共享创建资格：两个独立客户端同时提交时分别创建。
    pub herdr: Arc<herdr::HerdrClient>,
    /// 已接受创建的进程内生命周期；只供提交创建的页面轮询同一次创建，不是 Chat 列表。
    pub chat_creations: Arc<chats::ChatCreations>,
    /// 同一历史 Chat 的实例内 Resume 资格；立即拒绝重复请求，不排队，与会话登记独立。
    pub(crate) resume_executions: Arc<resume::ResumeExecutions>,
    /// 新 zone 的创建父目录；未显式配置时只禁用创建，不影响登记与浏览。
    pub create_parent_directory: Option<PathBuf>,
    /// 单个文件文本预览的大小上限（启动期配置，缺省 1 MiB）。
    ///
    /// 浏览读取只读；上限只影响内容预览，不影响目录列表与文件元数据。
    pub preview_limit_bytes: u64,
    /// 每条 Git 命令的执行时限（启动期配置，缺省 300 秒）。
    pub git_timeout: Duration,
    /// 同一实际仓库的 Git 写操作资格；与固定操作的执行资格完全独立。
    pub repository_locks: Arc<commit::RepositoryLocks>,
    /// 本地状态缓存与远端检查事实；只有显式检查、Sync 与应用成功提交后的后台检查访问远端。
    pub sync_state: Arc<sync::SyncState>,
    /// zone 文件事件监听与按 zone 合并的自动刷新通知。
    ///
    /// 每个已登记 zone 只有一条真实监听和一份节流状态；它只观察本地状态，不触发远端 fetch。
    pub zone_watch: Arc<watch::ZoneWatchManager>,
}

/// 构造使用真实关机动作、新建空会话表的应用：固定操作接口 + 关机端点 + 会话事件接口 + 打包页面。
///
/// 交付运行时不用本函数，而是用 [`app_with_sessions`] 自己持有会话表（于是能把同一份表交给后台过期
/// 扫描，见 `herdr_bot::sessions::spawn_expiry_scan`）；本函数固定空表，供只关心路由形状的调用方使用。
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
        Arc::new(sessions::SessionsState::with_policy(
            config.sessions.policy(),
        )),
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
/// （`herdr_bot::sessions::spawn_expiry_scan`）；于是「过期扫描 + 监听重判」的接线与用例看到的
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
    let sync_state = Arc::new(sync::SyncState::with_ttl(
        config.git.status_cache_ttl_seconds.duration(),
    ));
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
        history: Arc::new(history::History::new(config.sessions.history_file.clone(), sessions.clock())),
        sessions,
        shutdown,
        zones: Arc::new(zones::Zones::new(config.zones.registry_file.clone())),
        tasks: Arc::new(tasks::Tasks::new(
            config.tasks.storage_file.clone(),
            config.tasks.metadata_file.clone(),
        )),
        map_directory: std::path::absolute(&config.scheduler.map_directory).expect("resolve Map root"),
        bot_directory: std::path::absolute(&config.scheduler.bot_directory).expect("resolve Bot root"),
        run_directory: std::path::absolute(&config.scheduler.run_directory).expect("resolve Run root"),
        run_instructions: std::path::absolute(&config.scheduler.run_instructions).expect("resolve Run instructions"),
        runs: Arc::new(runs::Runs::default()),
        herdr: Arc::new(herdr::HerdrClient::new(&config.herdr)),
        chat_creations: Arc::new(chats::ChatCreations::new()),
        resume_executions: Arc::new(resume::ResumeExecutions::default()),
        create_parent_directory: config.zones.create_parent_directory.clone(),
        preview_limit_bytes: config.zones.preview_limit_bytes.get(),
        git_timeout: config.git.command_timeout_seconds.duration(),
        repository_locks: Arc::new(commit::RepositoryLocks::new()),
        sync_state: Arc::clone(&sync_state),
        zone_watch: Arc::new(watch::ZoneWatchManager::with_sync_state(
            config.zones.refresh_interval_seconds.duration(),
            sync_state,
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
        // 历史与备忘沿用浏览器局域网读写边界；不使用扩展上报的本机来源限制。
        .route("/api/sessions/history", get(history::get_history))
        .route("/api/sessions/history/memo", axum::routing::put(history::put_memo))
        // zone 登记列表：同一条路径读取与登记，取消登记是另一条明确命令。
        // 请求只携带服务器上的目录路径；执行目标仍只来自服务端配置。
        .route("/api/zones", get(list_zones).post(register_zone))
        .route("/api/scheduler/maps", get(scheduler::list_maps))
        .route("/api/scheduler/maps/{name}", get(scheduler::get_map))
        .route("/api/scheduler/maps/{name}/preflight", post(scheduler::preflight_run))
        .route("/api/scheduler/maps/{name}/runs", post(runs::start_run))
        .route("/api/workflow/runs/{id}", get(runs::get_run))
        .route("/api/workflow/manager/status", post(runs::manager_status))
        .route("/api/workflow/manager/dispatch", post(runs::manager_dispatch))
        .route("/api/workflow/node/report", post(runs::node_report))
        .route("/api/scheduler/maps/{name}/nodes/new", get(scheduler::new_node))
        .route("/api/scheduler/maps/{name}/nodes/edit", post(scheduler::edit_node))
        .route("/api/scheduler/maps/{name}/nodes/save", post(scheduler::save_node))
        .route("/api/scheduler/maps/{name}/nodes/check-delete", post(scheduler::check_delete_node))
        .route("/api/scheduler/maps/{name}/nodes/delete", post(scheduler::delete_node))
        .route("/api/scheduler/bots", get(scheduler::list_bots))
        .route("/api/scheduler/bots/new", get(scheduler::new_bot))
        .route("/api/scheduler/bots/edit", post(scheduler::edit_bot))
        .route("/api/scheduler/bots/save", post(scheduler::save_bot))
        .route("/api/scheduler/bots/check-delete", post(scheduler::check_bot_delete))
        .route("/api/scheduler/bots/delete", post(scheduler::delete_bot))
        // 新 zone 只使用配置的父目录与一个直接子目录名，客户端不能提交路径。
        .route("/api/zones/create", post(create_zone))
        // Task：同一条路径读取列表与新增；单个 Task 按标识读取、编辑与删除。
        // Task 只写入后端存储文件，不执行脚本、不创建 Chat。
        .route(
            "/api/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/tasks/{id}",
            get(get_task)
                .put(update_task)
                .delete(delete_task),
        )
        // 从已登记 zone 创建 Chat：预览只生成名称与实例标识；创建请求返回后端拥有的生命周期，
        // 之后由页面轮询同一次创建。请求只选择默认目标或按 zone 目录名匹配，不能指定任意名称。
        .route("/api/chats/resume", post(resume::post_resume))
        .route("/api/chats/preview", post(preview_chat))
        .route("/api/chats", post(create_chat))
        .route("/api/chats/{id}", get(get_chat_creation))
        // 只读浏览：整树一次读取、单目录读取、单文件读取。三者都从登记根重新核对边界。
        // 整树读取对 Git zone 应用 Git 原生忽略规则；单目录入口保留给直接调用者。
        .route("/api/zones/tree", get(read_zone_tree))
        .route("/api/zones/entries", get(list_zone_entries))
        .route("/api/zones/file", get(read_zone_file))
        // HTML 阅读模式的原始资源入口：zone 身份编码在路径段中，HTML 的相对 URL 因此
        // 能自然解析到同一 zone 内的 CSS、图片等资源；每次读取仍重新核对登记根与链接边界。
        .route(
            "/api/zones/reader/{zone}/{*path}",
            get(read_zone_reader_resource),
        )
        // Git 状态只读查询：当前分支与待提交变更；普通目录明确返回 repository=false。
        .route("/api/zones/git-status", get(zone_git_status))
        // 单个变更的审查 diff：旧侧是 HEAD，新侧是最终本地内容；两侧各自应用预览上限。
        .route("/api/zones/diff", get(zone_change_diff))
        // 经审查复核的整仓提交：确认后重查审查版本，再暂存全部符合 Git 规则的最终变更并提交。
        .route("/api/zones/commit", post(commit_zone))
        // 按需检查远端：真实 fetch 当前分支的既有 upstream，并记录最近检查时间。
        .route("/api/zones/check-remote", post(check_zone_remote))
        // 按既有 upstream 与 Git 配置同步：检查远端 → pull → 按需 push。
        .route("/api/zones/sync", post(sync_zone))
        // 登记列表的角标概览：每个 zone 的待提交项数或明确错误。
        .route("/api/zones/overview", get(zones_overview))
        // 自动更新的只读长连接：真实文件系统事件按 zone 合并后通知页面重新读取；
        // 同一 zone 的所有观察者共享一条监听，监听失效会明确下发错误而不是保持静默。
        .route("/api/zones/watch", get(watch_zones))
        .route("/api/zones/remove", post(unregister_zone))
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
                outcome: if command.selected {
                    SELECTED
                } else {
                    UNSELECTED
                },
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

/// zone 登记列表的响应：当前完整列表（登记与取消登记也返回同一形状）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ZoneCreationView {
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_directory: Option<String>,
}

#[derive(Debug, Serialize)]
struct ZoneListResponse {
    zones: Vec<RegisteredZone>,
    creation: ZoneCreationView,
}

/// zone 请求的明确失败：机器可读类别 + 面向使用者的说明。
#[derive(Debug, Serialize)]
struct ZoneErrorResponse {
    code: &'static str,
    message: String,
}

/// 一次 zone 命令的请求体：服务器上的目录路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZonePathRequest {
    path: String,
}

/// 新建 zone 的请求体：只接受一个目录名称，父目录只来自服务端配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateZoneRequest {
    name: String,
}

/// Task 列表的响应：当前完整列表与固定选项。
#[derive(Debug, Serialize)]
struct TaskListResponse {
    tasks: Vec<tasks::Task>,
    options: catalog::TaskOptions,
}

/// 单个 Task 的响应。
#[derive(Debug, Serialize)]
struct TaskView {
    task: tasks::Task,
}

/// “列出 zone 登记”：读取后端持久保存的同一份列表，不经过任何页面缓存。
async fn list_zones(State(state): State<AppState>) -> Response {
    zone_list_response(&state, state.zones.list().await)
}

/// “登记 zone”：路径必须真实存在且是服务器上的绝对目录；只有持久化成功后返回成功。
async fn register_zone(State(state): State<AppState>, request: Request) -> Response {
    match zone_path(request).await {
        Ok(path) => match state.zones.register(&path).await {
            Ok(zones) => {
                // 新登记的 zone 立即开始监听；监听失败会作为健康错误交给观察连接。
                state.zone_watch.reconcile(&zones).await;
                zone_list_response(&state, Ok(zones))
            }
            Err(error) => zone_error_response(error),
        },
        Err(rejection) => rejection,
    }
}

/// “新建 zone”：在配置父目录下创建直接子目录、执行 `git init`，随后登记。
///
/// 客户端只提交名称，父目录固定来自服务端配置。目录创建或 Git 初始化失败会保留已产生的目录；
/// 登记失败也保留目录，并明确告诉调用者目录与 Git 初始化已经完成、可是登记没有成功。
async fn create_zone(State(state): State<AppState>, request: Request) -> Response {
    let name = match zone_create_name(request).await {
        Ok(name) => name,
        Err(rejection) => return rejection,
    };
    let Some(parent) = state.create_parent_directory.clone() else {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "create-parent-not-configured",
            "未配置 [zones] create_parent_directory；本次没有创建任何目录。".to_string(),
        );
    };
    let timeout = state.git_timeout;
    let created = match tokio::task::spawn_blocking(move || {
        zone_create::create_and_init(&parent, &name, timeout)
    })
    .await
    {
        Ok(Ok(path)) => path,
        Ok(Err(error)) => return zone_create_error_response(error),
        Err(_) => {
            return rejected_owned(
                StatusCode::INTERNAL_SERVER_ERROR,
                "create-task-failed",
                "新建 zone 的任务异常结束，无法确认目录或 Git 状态。".to_string(),
            );
        }
    };
    let path = zones::display_path(&created);
    match state.zones.register(&path).await {
        Ok(zones) => {
            state.zone_watch.reconcile(&zones).await;
            zone_list_response(&state, Ok(zones))
        }
        Err(error) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "zone-registration-failed",
            format!(
                "目录与 Git 初始化已完成，但 zone 登记失败，目录已保留：{path}；{}",
                error.message()
            ),
        ),
    }
}

/// “取消登记”：只移除登记项，不删除目录或文件。
async fn unregister_zone(State(state): State<AppState>, request: Request) -> Response {
    match zone_path(request).await {
        Ok(path) => match state.zones.unregister(&path).await {
            Ok(zones) => {
                state.zone_watch.reconcile(&zones).await;
                zone_list_response(&state, Ok(zones))
            }
            Err(error) => zone_error_response(error),
        },
        Err(rejection) => rejection,
    }
}

/// 读取 zone 命令的请求体；请求面封闭（不接受查询串或未知字段）。
async fn zone_path(request: Request) -> Result<String, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "zone 命令不接受查询参数：路径只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "zone 请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let body: ZonePathRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析 zone 命令：请发送 {\"path\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    Ok(body.path)
}

/// 读取新建 zone 的请求体；请求面同样封闭，父目录不会从请求中接受。
async fn zone_create_name(request: Request) -> Result<String, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "新建 zone 不接受查询参数：名称只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "新建 zone 的请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let body: CreateZoneRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析新建 zone 请求：请发送 {\"name\":\"目录名称\"} 形式的 JSON。".to_string(),
        )
    })?;
    Ok(body.name)
}

/// 把创建 zone 的失败翻译成稳定 HTTP 类别；失败消息保留目录路径与失败阶段。
fn zone_create_error_response(error: zone_create::CreateZoneError) -> Response {
    use zone_create::CreateZoneError;
    let status = match &error {
        CreateZoneError::AlreadyExists { .. } => StatusCode::CONFLICT,
        CreateZoneError::ParentNotAbsolute(_)
        | CreateZoneError::ParentNotFound(_)
        | CreateZoneError::ParentNotDirectory(_)
        | CreateZoneError::ParentInaccessible { .. }
        | CreateZoneError::NameRequired
        | CreateZoneError::InvalidName(_) => StatusCode::BAD_REQUEST,
        CreateZoneError::CreateFailed { .. }
        | CreateZoneError::GitInitFailed { .. }
        | CreateZoneError::FinalizeFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())
}

/// “列出 Task 与固定选项”：读取后端持久保存的同一份列表。
async fn list_tasks(State(state): State<AppState>) -> Response {
    task_list_response(&state, state.tasks.list().await)
}

/// “读取单个 Task”：删除后由独立客户端按标识选择时得到明确的不存在。
async fn get_task(State(state): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    // Metadata health is global; stale selections are not. Keep the original IDs readable
    // when the current catalog is valid, without pretending a broken catalog is usable.
    if let Err(error) = state.tasks.catalog() {
        return task_error_response(error);
    }
    match state.tasks.get(&id).await {
        Ok(task) => (StatusCode::OK, Json(TaskView { task })).into_response(),
        Err(error) => task_error_response(error),
    }
}

/// “新增 Task”：固定字段逐个校验；只有存储成功后返回当前完整列表。
async fn create_task(State(state): State<AppState>, request: Request) -> Response {
    let input = match task_input(request).await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    task_list_response(&state, state.tasks.create(input).await)
}

/// “编辑 Task”：标识不变；只有存储成功后返回当前完整列表。
async fn update_task(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    let input = match task_input(request).await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    task_list_response(&state, state.tasks.update(&id, input).await)
}

/// “删除 Task”：只移除 Task，不改变任何已经创建的 Chat。
async fn delete_task(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    if let Some(rejection) = reject_task_delete_request(request).await {
        return rejection;
    }
    task_list_response(&state, state.tasks.delete(&id).await)
}

/// 读取 Task 命令的请求体；请求面封闭（不接受查询串或未知字段）。
async fn task_input(request: Request) -> Result<tasks::TaskInput, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "Task命令不接受查询参数：字段只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_TASK_REQUEST_BYTES)
        .await
        .map_err(|_| {
            rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                "Task请求体超过 1 MiB 上限。".to_string(),
            )
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            format!(
                "无法解析Task命令：{error}；请发送包含 name、chatName、agent、model、thinking、skill、prompt 的 JSON。"
            ),
        )
    })
}

/// 删除 Task 的请求面：不接受查询串或请求体。
async fn reject_task_delete_request(request: Request) -> Option<Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "删除Task不接受查询参数：目标只由路径中的Task标识决定。",
        ));
    }

    let (_, body) = request.into_parts();
    match to_bytes(body, MAX_TASK_REQUEST_BYTES).await {
        Ok(bytes) if bytes.is_empty() => None,
        _ => Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "删除Task不接受请求体：目标只由路径中的Task标识决定。",
        )),
    }
}

/// 把 Task 列表结果翻译成 HTTP 响应：成功附带与页面同一份固定选项。
fn task_list_response(
    state: &AppState,
    result: Result<Vec<tasks::Task>, tasks::TaskError>,
) -> Response {
    let catalog = match state.tasks.catalog() {
        Ok(catalog) => catalog,
        Err(error) => return task_error_response(error),
    };
    match result {
        Ok(tasks) => (
            StatusCode::OK,
            Json(TaskListResponse {
                tasks,
                options: catalog.options(),
            }),
        )
            .into_response(),
        Err(error) => task_error_response(error),
    }
}

fn task_error_response(error: tasks::TaskError) -> Response {
    task_error_with_context(error, "")
}

fn task_error_with_context(error: tasks::TaskError, context: &str) -> Response {
    use tasks::TaskError;
    let status = match &error {
        TaskError::NotFound => StatusCode::NOT_FOUND,
        TaskError::Unreadable(_)
        | TaskError::Metadata(_)
        | TaskError::Corrupt(_)
        | TaskError::WriteFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        TaskError::NameRequired
        | TaskError::ChatNameRequired
        | TaskError::ChatNameNotSingleLine
        | TaskError::AgentRequired
        | TaskError::UnknownAgent(_)
        | TaskError::InvalidTaskType
        | TaskError::UnknownModel(_)
        | TaskError::UnknownThinking(_)
        | TaskError::ThinkingRequiresDefaultModel
        | TaskError::ThinkingUnsupported { .. }
        | TaskError::UnknownSkill(_)
        | TaskError::InvalidExtension(_) => StatusCode::BAD_REQUEST,
    };
    rejected_owned(
        status,
        error.code(),
        format!("{}{context}", error.message()),
    )
}

/// Chat 名称预览请求：只指定已登记 zone 与 Task，不携带名称、Workspace 或其他覆盖字段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChatPreviewRequest {
    zone: String,
    task_id: String,
}

/// 本次创建的目标选择；客户端不能指定任意 Workspace 名称。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum WorkspaceTarget {
    #[default]
    Default,
    Current,
}

/// 创建 Chat 的请求：实例标识与预览名称沿用预览结果；input 可为空。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateChatRequest {
    zone: String,
    task_id: String,
    chat_id: String,
    #[serde(default)]
    workspace_target: WorkspaceTarget,
    #[serde(default)]
    input: String,
    /// 仅用于本次创建的命名规则；省略时使用 Task 规则，不写回 Task。
    #[serde(default)]
    chat_name_rule: Option<String>,
    /// 预览时看到的名称；若有效规则渲染出的名称变化，明确拒绝而不是悄悄改名。
    #[serde(default)]
    expected_name: Option<String>,
}

/// Chat 名称预览的成功响应：本次创建实际会使用的名称与短实例标识。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatPreviewResponse {
    chat_id: String,
    name: String,
    zone: String,
    zone_name: String,
    task_id: String,
    task_name: String,
    chat_name_rule: String,
    agent: &'static str,
}

/// “预览 Chat 名称”：只生成名称与短实例标识，不创建 Tab、不启动 agent。
async fn preview_chat(State(state): State<AppState>, request: Request) -> Response {
    let input: ChatPreviewRequest = match chat_command_body(request, "Chat 名称预览").await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    let zone = match chat_zone(&state, &input.zone).await {
        Ok(zone) => zone,
        Err(rejection) => return rejection,
    };
    let task = match state.tasks.get(&input.task_id).await {
        Ok(task) => task,
        Err(error) => return task_error_response(error),
    };

    let chat_id = chats::new_chat_id();
    let name = chats::render_chat_name(&task.chat_name, &zone, &task, &chat_id);
    let agent = task.agent.label();
    (
        StatusCode::OK,
        Json(ChatPreviewResponse {
            chat_id,
            name,
            zone_name: std::path::Path::new(&zone)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            zone,
            task_id: task.id,
            task_name: task.name,
            chat_name_rule: task.chat_name,
            agent,
        }),
    )
        .into_response()
}

/// “创建 Chat”：接受后由后端拥有的任务继续执行，HTTP 响应只给出本次创建的生命周期标识。
///
/// Task 与目标名称在请求被接受时固定为快照；之后编辑或删除 Task 不改变本次创建。
/// 默认目标来自服务端配置，当前目标来自已登记 zone 根目录名；请求不能提供任意名称。
async fn create_chat(State(state): State<AppState>, request: Request) -> Response {
    let input: CreateChatRequest = match chat_command_body(request, "创建 Chat").await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    if !chats::is_valid_chat_id(&input.chat_id) {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "chat-id-invalid",
            "Chat 短实例标识必须来自名称预览（6 个小写十六进制字符）。".to_string(),
        );
    }
    let zone = match chat_zone(&state, &input.zone).await {
        Ok(zone) => zone,
        Err(rejection) => return rejection,
    };
    let task = match state.tasks.get(&input.task_id).await {
        Ok(task) => task,
        Err(error) => {
            return task_error_with_context(
                error,
                " 未创建 Herdr Tab、未启动 agent、未提交输入。",
            );
        }
    };
    let workspace_name = match input.workspace_target {
        WorkspaceTarget::Default => state.herdr.workspace_name().to_string(),
        WorkspaceTarget::Current => match std::path::Path::new(&zone).file_name().and_then(|name| name.to_str()) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => return rejected_owned(
                StatusCode::BAD_REQUEST,
                "workspace-name-unavailable",
                "该 zone 无可用目录名，请选择默认 Workspace。未创建 Herdr Tab、未启动 agent、未提交输入。".to_string(),
            ),
        },
    };
    let rule = input.chat_name_rule.as_deref().unwrap_or(&task.chat_name);
    if rule.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "chat-name-rule-required",
            "本次 chat-name 命名规则不能为空。未创建 Herdr Tab、未启动 agent、未提交输入。"
                .to_string(),
        );
    }
    if rule.contains(|c| c == '\n' || c == '\r') {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "chat-name-rule-not-single-line",
            "本次 chat-name 必须是单行规则，不能包含换行。未创建 Herdr Tab、未启动 agent、未提交输入。".to_string(),
        );
    }
    let name = chats::render_chat_name(rule, &zone, &task, &input.chat_id);
    if name.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "chat-name-required",
            "命名规则生成的 Chat 名称不能为空。未创建 Herdr Tab、未启动 agent、未提交输入。"
                .to_string(),
        );
    }
    if let Some(expected) = input.expected_name {
        if expected != name {
            return rejected_owned(
                StatusCode::CONFLICT,
                "name-preview-stale",
                "Task在预览之后发生了变化，本次名称已不同；请重新预览后再创建。".to_string(),
            );
        }
    }

    let preset = match state
        .tasks
        .catalog()
        .and_then(|catalog| catalog.resolve(&task))
    {
        Ok(preset) => preset,
        Err(error) => {
            return task_error_with_context(
                error,
                " 未创建 Herdr Tab、未启动 agent、未提交输入。",
            );
        }
    };
    if let Err(message) = preset.env.check_entrypoints() {
        return rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "extension-entrypoint-invalid",
            format!("{message} 未创建 Herdr Tab、未启动 agent、未提交输入。"),
        );
    }
    let creation_id = chats::new_creation_id();
    let view = chats::ChatCreationView::starting(creation_id, name, zone.clone(), task.id.clone());
    let job = state.chat_creations.insert(view.clone()).await;
    let herdr = Arc::clone(&state.herdr);
    let history = Arc::clone(&state.history);
    tokio::spawn(async move {
        chats::run_creation(
            herdr,
            history,
            job,
            task,
            preset,
            zone,
            input.input,
            workspace_name,
        )
        .await;
    });
    (StatusCode::ACCEPTED, Json(view)).into_response()
}

/// “读取一次创建的状态”：同一次创建在请求结束后仍可被提交它的页面观察。
async fn get_chat_creation(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "创建状态读取不接受查询参数：目标只由路径中的创建标识决定。",
        );
    }
    match state.chat_creations.get(&id).await {
        Some(view) => (StatusCode::OK, Json(view)).into_response(),
        None => rejected(
            StatusCode::NOT_FOUND,
            "creation-not-found",
            "找不到这次创建；它可能不属于当前后端实例。",
        ),
    }
}

/// 解析 Chat 命令的 JSON 请求体；查询串与未知字段都被拒绝。
async fn chat_command_body<T: serde::de::DeserializeOwned>(
    request: Request,
    action: &str,
) -> Result<T, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            format!("{action}不接受查询参数：字段只来自请求体。"),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_TASK_REQUEST_BYTES)
        .await
        .map_err(|_| {
            rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                format!("{action}的请求体超过 1 MiB 上限。"),
            )
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            format!("无法解析{action}请求：{error}"),
        )
    })
}

/// 核对请求中的 zone 确实已登记，并返回登记列表里的规范路径。
///
/// Chat 的 cwd 只能来自已登记 zone；未登记的路径不会被当作工作目录。
async fn chat_zone(state: &AppState, requested: &str) -> Result<String, Response> {
    let trimmed = requested.trim();
    if trimmed.is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请选择已登记的 zone。".to_string(),
        ));
    }
    let zones = state.zones.list().await.map_err(|error| {
        rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "zones-unreadable",
            format!("读取 zone 登记列表失败：{}", error.message()),
        )
    })?;
    zones
        .into_iter()
        .find(|entry| zones::same_path(&entry.path, trimmed))
        .map(|entry| entry.path)
        .ok_or_else(|| {
            rejected_owned(
                StatusCode::BAD_REQUEST,
                "zone-not-registered",
                "该目录没有登记；只能从已登记的 zone 创建 Chat。".to_string(),
            )
        })
}

/// 一次只读浏览请求的查询参数：登记路径 + 可选相对路径（空字符串表示根目录）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowseQuery {
    zone: String,
    #[serde(default)]
    path: String,
}

/// 整树读取的查询参数：只需要登记路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneTreeQuery {
    zone: String,
}

/// “读整棵树”：一次返回过滤后的目录快照。Git zone 按 Git 原生忽略规则省略 ignored 路径；
/// 普通目录完整返回。子目录读取失败随响应带回，根目录或 Git 忽略查询失败则整次请求明确失败。
async fn read_zone_tree(
    State(state): State<AppState>,
    query: Result<Query<ZoneTreeQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "读取 zone 目录树需要唯一的 `zone` 参数，且不接受其他参数。".to_string(),
            );
        }
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    let registered = match registered_zone(&state, &query.zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let timeout = state.git_timeout;
    match tokio::task::spawn_blocking(move || browse::read_tree(&registered, timeout)).await {
        Ok(Ok(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(Err(error)) => browse_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "read-failed",
            "读取目录树的任务异常结束，无法确认目录内容。".to_string(),
        ),
    }
}

/// “列目录”：返回该目录的直接子项；隐藏文件、忽略文件和 Git 元数据都不过滤。
async fn list_zone_entries(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(rejection) => return rejection,
    };
    let registered = match registered_zone(&state, &query.zone).await {
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
async fn read_zone_file(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(rejection) => return rejection,
    };
    let registered = match registered_zone(&state, &query.zone).await {
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

/// HTML 阅读模式读取原始资源：HTML、CSS、图片、字体等都以真实字节返回。
///
/// `zone` 是登记路径的 UTF-8 十六进制编码，只用于把相对 URL 稳定地解析回同一个 zone；
/// 是否允许读取仍由登记列表与浏览边界决定。响应总是 `nosniff`，HTML 与 SVG 还带禁止脚本、表单、
/// 嵌入和外部资源自动加载的 CSP。
async fn read_zone_reader_resource(
    State(state): State<AppState>,
    UrlPath((zone_token, path)): UrlPath<(String, String)>,
) -> Response {
    let requested = match decode_zone_token(&zone_token) {
        Ok(zone) => zone,
        Err(message) => {
            return rejected_owned(StatusCode::BAD_REQUEST, "invalid-zone-token", message);
        }
    };
    let registered = match registered_zone(&state, &requested).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let content_type = resource_content_type(&path);
    let relative = path.clone();
    match tokio::task::spawn_blocking(move || browse::read_resource(&registered, &relative)).await {
        Ok(Ok(bytes)) => resource_response(bytes, content_type),
        Ok(Err(error)) => browse_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "read-failed",
            "读取 zone 原始资源的任务异常结束，无法确认文件内容。".to_string(),
        ),
    }
}

fn decode_zone_token(token: &str) -> Result<String, String> {
    if token.is_empty() || token.len() % 2 != 0 {
        return Err("HTML 阅读资源地址中的 zone 编码无效。".to_string());
    }
    let mut bytes = Vec::with_capacity(token.len() / 2);
    for pair in token.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair)
            .map_err(|_| "HTML 阅读资源地址中的 zone 编码无效。".to_string())?;
        let byte = u8::from_str_radix(text, 16)
            .map_err(|_| "HTML 阅读资源地址中的 zone 编码无效。".to_string())?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| "HTML 阅读资源地址中的 zone 编码不是 UTF-8。".to_string())
}

fn resource_response(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src 'self' data: blob:; media-src 'self' data: blob:; \
             font-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'none'; \
             object-src 'none'; frame-src 'none'; child-src 'none'; connect-src 'none'; \
             form-action 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let mut response = (StatusCode::OK, Body::from(bytes)).into_response();
    *response.headers_mut() = headers;
    response
}

fn resource_content_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" | "md" | "markdown" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// 一次 Git 状态请求的查询参数：只需要登记路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GitStatusQuery {
    zone: String,
    #[serde(default)]
    refresh: bool,
}

/// zone 列表上的角标概览：每个登记项的待提交项数或明确错误。
#[derive(Debug, Serialize)]
struct ZoneOverviewResponse {
    zones: Vec<git::ZoneSummary>,
}

/// “读取 zone 概览”：为登记列表中每个 zone 给出角标所需的事实。
async fn zones_overview(State(state): State<AppState>) -> Response {
    let zones = match state.zones.list().await {
        Ok(zones) => zones,
        Err(error) => return zone_error_response(error),
    };
    state.zone_watch.reconcile(&zones).await;
    let timeout = state.git_timeout;
    let sync_state = Arc::clone(&state.sync_state);
    match tokio::task::spawn_blocking(move || git::overview(&zones, timeout, &sync_state)).await {
        Ok(zones) => (StatusCode::OK, Json(ZoneOverviewResponse { zones })).into_response(),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "git-command-failed",
            "读取 zone 概览的任务异常结束，无法确认待提交数量。".to_string(),
        ),
    }
}

/// “读取 Git 状态”：当前分支与待提交变更；普通目录返回 `repository: false`。
async fn zone_git_status(
    State(state): State<AppState>,
    query: Result<Query<GitStatusQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "读取 Git 状态需要 `zone` 参数，可选布尔参数 `refresh`，且不接受其他参数。"
                    .to_string(),
            );
        }
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    let registered = match registered_zone(&state, &query.zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let timeout = state.git_timeout;
    let sync_state = Arc::clone(&state.sync_state);
    match tokio::task::spawn_blocking(move || {
        sync_state.cached_status(&registered, timeout, query.refresh)
    })
    .await
    {
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
/// 首帧是全部被监听 zone 的快照（实际生效的间隔、健康与最近刷新），之后同一 zone 的所有
/// 观察者共享同一条监听与同一份节流状态，只收到合并后的刷新通知或监听健康变化。
/// 下发本地事件及后台远端检查的开始/结束通知；订阅本身不会触发 fetch。
async fn watch_zones(State(state): State<AppState>) -> Response {
    let zones = match state.zones.list().await {
        Ok(zones) => zones,
        Err(error) => return zone_error_response(error),
    };
    // 先订阅再对齐监听：对齐期间新建立的监听健康与刷新都不会漏掉。
    let receiver = state.zone_watch.subscribe();
    state.zone_watch.reconcile(&zones).await;
    let manager = Arc::clone(&state.zone_watch);
    let first = manager.snapshot().await;

    let stream = stream::unfold((receiver, Some(first)), move |(mut receiver, pending)| {
        let manager = Arc::clone(&manager);
        async move {
            let snapshot = match pending {
                Some(snapshot) => snapshot,
                None => match receiver.recv().await {
                    Ok(frame) => {
                        return Some((
                            Ok::<Event, Infallible>(watch::frame_event(frame)),
                            (receiver, None),
                        ));
                    }
                    Err(RecvError::Lagged(_)) => manager.snapshot().await,
                    Err(RecvError::Closed) => return None,
                },
            };
            Some((Ok(watch::snapshot_event(snapshot)), (receiver, None)))
        }
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

fn git_error_response(error: git::GitError) -> Response {
    let status = match &error {
        git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
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
    zone: String,
    path: String,
    #[serde(default)]
    old_path: Option<String>,
}

/// “读取审查 diff”：旧侧取当前分支最新本地提交，新侧取最终本地文件。
///
/// 新增、删除、已识别重命名与部分暂存都使用同一入口；旧路径只用于定位 Git 历史内容，仍先经过
/// zone 根边界检查。任一侧超限或二进制时返回明确限制，不返回完整内容。
async fn zone_change_diff(
    State(state): State<AppState>,
    query: Result<Query<DiffQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "审查 diff 需要 `zone` 与 `path` 参数，可选 `oldPath`，且不接受其他参数。"
                    .to_string(),
            );
        }
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    if query.path.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-query",
            "审查 diff 需要非空的 `path` 参数。".to_string(),
        );
    }

    let registered = match registered_zone(&state, &query.zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let limit = state.preview_limit_bytes;
    let timeout = state.git_timeout;
    match tokio::task::spawn_blocking(move || {
        git::change_diff(
            &registered,
            &query.path,
            query.old_path.as_deref(),
            limit,
            timeout,
        )
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

/// 一次提交请求：服务器目录、非空提交信息，以及页面展示的审查版本标识。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ZoneCommitRequest {
    zone: String,
    message: String,
    review_token: String,
}

/// 一次真实提交成功后的回执：只有 Git 确认创建了新的 HEAD 才返回。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ZoneCommitResponse {
    zone: String,
    outcome: &'static str,
    commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    message: String,
}

/// “经审查复核的整仓提交”。
///
/// 页面与直接请求都拒绝空信息、普通目录、阻塞状态和失效审查；被拒绝的请求不会产生 Git 写入。
/// 请求被接受后，执行放在独立任务里：客户端断线不会取消已经开始的提交，也不会在别处重放。
async fn commit_zone(State(state): State<AppState>, request: Request) -> Response {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "提交请求不接受查询参数：zone、提交信息与审查版本只来自请求体。".to_string(),
        );
    }

    let (_, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_ZONE_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "payload-too-large",
                "提交请求体超过 64KB 上限。".to_string(),
            );
        }
    };
    let request: ZoneCommitRequest = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-request",
                "无法解析提交请求：请发送 {\"zone\":\"服务器目录\",\"message\":\"提交信息\",\"reviewToken\":\"审查版本\"} 形式的 JSON。"
                    .to_string(),
            );
        }
    };
    if request.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请指定要提交的已登记 zone。".to_string(),
        );
    }
    if request.message.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "empty-message",
            "提交信息不能为空；本次没有执行任何 Git 写入。".to_string(),
        );
    }
    if request.review_token.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "review-required",
            "缺少审查版本标识；请先读取并确认当前审查版本。".to_string(),
        );
    }

    let registered = match registered_zone(&state, &request.zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let response_zone = registered.clone();
    let message = request.message;
    let review_token = request.review_token;
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let operation_zone = registered.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            commit::execute(
                &registered,
                &message,
                &review_token,
                timeout,
                &locks,
                &sync_state,
            )
        })
        .await;
        state.zone_watch.notify(&operation_zone).await;
        if matches!(&outcome, Ok(commit::CommitOutcome::Committed { .. })) {
            let background_state = state.clone();
            tokio::spawn(async move {
                let check_zone = operation_zone.clone();
                let sync_state = Arc::clone(&background_state.sync_state);
                let locks = Arc::clone(&background_state.repository_locks);
                let result = tokio::task::spawn_blocking(move || {
                    sync::check_background(&check_zone, timeout, &locks, &sync_state)
                })
                .await;
                let error = match result {
                    Ok(sync::CheckOutcome::Checked(_)) => None,
                    Ok(sync::CheckOutcome::NoUpstream) => Some("当前分支没有配置 upstream，无法检查远端；本功能不会自动发布分支或配置 upstream。".to_string()),
                    Ok(sync::CheckOutcome::Blocked(reason)) => Some(reason),
                    Ok(sync::CheckOutcome::Failed(error)) => Some(error.message()),
                    Ok(sync::CheckOutcome::Busy) => Some("仓库已有 Git 写操作，提交后的远端检查未执行。".to_string()),
                    Ok(sync::CheckOutcome::NotRepository) => Some("提交后的目录不再是 Git 工作树根，无法检查远端。".to_string()),
                    Err(_) => Some("提交后的远端检查任务异常结束，无法确认远端状态。".to_string()),
                };
                if error.is_some() {
                    background_state
                        .sync_state
                        .cache
                        .invalidate(&operation_zone);
                }
                background_state
                    .sync_state
                    .finish_remote_check(&operation_zone, error);
                background_state.zone_watch.notify(&operation_zone).await;
            });
        }
        let response = commit_response(response_zone, outcome);
        // 连接已经断开时接收端不存在，结果丢弃；已经开始的提交不会被取消或重放。
        let _ = sender.send(response);
    });

    match receiver.await {
        Ok(response) => response,
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commit-result-unavailable",
            "本次提交没有送回结果：无法确认执行结果，请按实际仓库状态核对。".to_string(),
        ),
    }
}

fn commit_response(
    zone: String,
    outcome: Result<commit::CommitOutcome, tokio::task::JoinError>,
) -> Response {
    match outcome {
        Ok(commit::CommitOutcome::Committed {
            commit,
            branch,
            message,
        }) => (
            StatusCode::OK,
            Json(ZoneCommitResponse {
                zone,
                outcome: "committed",
                commit,
                branch,
                message,
            }),
        )
            .into_response(),
        Ok(commit::CommitOutcome::NotRepository) => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可执行的 Commit。".to_string(),
        ),
        Ok(commit::CommitOutcome::Busy) => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次请求没有被接受，也不会在它结束后补做。".to_string(),
        ),
        Ok(commit::CommitOutcome::Blocked(reason)) => {
            rejected_owned(StatusCode::CONFLICT, "commit-blocked", reason)
        }
        Ok(commit::CommitOutcome::ReviewInvalid(message)) => {
            rejected_owned(StatusCode::CONFLICT, "review-invalid", message)
        }
        Ok(commit::CommitOutcome::NothingToCommit) => rejected_owned(
            StatusCode::CONFLICT,
            "nothing-to-commit",
            "当前没有待提交变更：本次没有执行 Git 写入。".to_string(),
        ),
        Ok(commit::CommitOutcome::Failed(error)) => {
            let status = match &error {
                git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
                git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
                git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            rejected_owned(status, error.code(), error.message())
        }
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commit-result-unavailable",
            "提交任务异常结束，无法确认执行结果；请按实际仓库状态核对。".to_string(),
        ),
    }
}

/// 一次“只有 zone”的 POST 请求体；检查远端与 Sync 都只接收服务器目录。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneTargetRequest {
    zone: String,
}

/// 读取检查远端/Sync 的请求体；请求面封闭，不接受查询串或未知字段。
async fn zone_target(request: Request) -> Result<String, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "该命令不接受查询参数：zone 只来自请求体。".to_string(),
        ));
    }
    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let request: ZoneTargetRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析请求：请发送 {\"zone\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    if request.zone.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请指定要操作的已登记 zone。".to_string(),
        ));
    }
    Ok(request.zone)
}

/// “检查远端”：真实 fetch 当前分支已有 upstream 的远端，成功后返回刷新后的完整 Git 状态。
///
/// 检查不要求工作树干净，也不修改工作树、索引或本地分支；普通目录、detached HEAD 与没有远端
/// upstream 的状态分别明确拒绝。检查与 Commit/Sync 共用同一份仓库写操作资格。
async fn check_zone_remote(State(state): State<AppState>, request: Request) -> Response {
    let zone = match zone_target(request).await {
        Ok(zone) => zone,
        Err(response) => return response,
    };
    let registered = match registered_zone(&state, &zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let refresh_zone = registered.clone();
    let outcome =
        tokio::task::spawn_blocking(move || sync::check(&registered, timeout, &locks, &sync_state))
            .await;
    state.zone_watch.notify(&refresh_zone).await;
    match outcome {
        Ok(sync::CheckOutcome::Checked(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(sync::CheckOutcome::NotRepository) => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可检查的远端。".to_string(),
        ),
        Ok(sync::CheckOutcome::Busy) => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次检查没有被接受，也不会在它结束后补做。".to_string(),
        ),
        Ok(sync::CheckOutcome::Blocked(reason)) => {
            rejected_owned(StatusCode::CONFLICT, "sync-blocked", reason)
        }
        Ok(sync::CheckOutcome::NoUpstream) => rejected_owned(
            StatusCode::CONFLICT,
            "no-upstream",
            "当前分支没有配置 upstream：无法确定要检查的远端；本功能不会代替你配置 upstream。"
                .to_string(),
        ),
        Ok(sync::CheckOutcome::Failed(error)) => git_error_response(error),
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "git-command-failed",
            "检查远端的任务异常结束，无法确认远端状态。".to_string(),
        ),
    }
}

/// “按既有 upstream 同步”：检查远端 → pull → 按需 push。
///
/// 页面与直接请求都拒绝普通目录、脏工作树、detached HEAD、冲突或未完成流程，以及没有远端
/// upstream 的状态。执行放在独立任务里：客户端断线不会取消已接受的同步，也不会重放。
async fn sync_zone(State(state): State<AppState>, request: Request) -> Response {
    let zone = match zone_target(request).await {
        Ok(zone) => zone,
        Err(response) => return response,
    };
    let registered = match registered_zone(&state, &zone).await {
        Ok(registered) => registered,
        Err(response) => return response,
    };
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let response_zone = registered.clone();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let outcome = tokio::task::spawn_blocking(move || {
            sync::execute(&registered, timeout, &locks, &sync_state)
        })
        .await;
        state.zone_watch.notify(&response_zone).await;
        let response = sync_response(response_zone, outcome);
        let _ = sender.send(response);
    });

    match receiver.await {
        Ok(response) => response,
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "sync-result-unavailable",
            "本次同步没有送回结果：无法确认执行结果，请按实际仓库状态核对。".to_string(),
        ),
    }
}

fn sync_response(
    zone: String,
    outcome: Result<sync::SyncOutcome, tokio::task::JoinError>,
) -> Response {
    match outcome {
        Ok(sync::SyncOutcome::Synced(mut accepted)) => {
            accepted.zone = zone;
            (StatusCode::OK, Json(accepted)).into_response()
        }
        Ok(sync::SyncOutcome::NotRepository) => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可执行的 Sync。".to_string(),
        ),
        Ok(sync::SyncOutcome::Busy) => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次请求没有被接受，也不会在它结束后补做。".to_string(),
        ),
        Ok(sync::SyncOutcome::Blocked(reason)) => {
            rejected_owned(StatusCode::CONFLICT, "sync-blocked", reason)
        }
        Ok(sync::SyncOutcome::Dirty) => rejected_owned(
            StatusCode::CONFLICT,
            "sync-dirty-worktree",
            "工作树有未提交变更：Sync 只对干净工作树可用；本功能不会 stash 或隐式提交。"
                .to_string(),
        ),
        Ok(sync::SyncOutcome::NoUpstream) => rejected_owned(
            StatusCode::CONFLICT,
            "no-upstream",
            "当前分支没有配置 upstream：Sync 不可用；本功能不会配置 upstream 或发布分支。"
                .to_string(),
        ),
        Ok(sync::SyncOutcome::Failed {
            step,
            error,
            completed,
        }) => {
            let mut message = format!("同步在 {step} 步骤失败：{}", error.message());
            if !completed.is_empty() {
                message.push_str(&format!(
                    " 已完成的步骤（{}）不会被自动撤销。",
                    completed.join(" → ")
                ));
            }
            let status = match &error {
                git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
                git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
                git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            rejected_owned(status, error.code(), message)
        }
        Err(_) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "sync-result-unavailable",
            "同步任务异常结束，无法确认执行结果；请按实际仓库状态核对。".to_string(),
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
            "读取 zone 需要 `zone` 参数与可选的 `path` 参数，且不接受其他参数。".to_string(),
        )
    })?;
    if query.zone.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        ));
    }
    Ok(query)
}

/// 按登记列表核对 zone；未登记与登记文件读取失败都明确返回，不继续读取文件系统。
async fn registered_zone(state: &AppState, requested: &str) -> Result<String, Response> {
    match state.zones.list().await {
        Ok(zones) => {
            state.zone_watch.reconcile(&zones).await;
            match browse::registered_root(&zones, requested) {
                Ok(root) => Ok(root.to_string()),
                Err(error) => Err(browse_error_response(error)),
            }
        }
        Err(error) => Err(zone_error_response(error)),
    }
}

fn browse_error_response(error: browse::BrowseError) -> Response {
    use browse::BrowseError;
    let status = match &error {
        BrowseError::ZoneNotRegistered | BrowseError::PathNotFound => StatusCode::NOT_FOUND,
        BrowseError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        BrowseError::PathOutside | BrowseError::Link { .. } => StatusCode::FORBIDDEN,
        BrowseError::NotADirectory | BrowseError::NotAFile | BrowseError::InvalidPath(_) => {
            StatusCode::BAD_REQUEST
        }
        BrowseError::ReadFailed(_) | BrowseError::GitIgnoreFailed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    rejected_owned(status, error.code(), error.message())
}

/// 把登记操作的结果翻译成 HTTP 响应：成功返回当前完整列表与创建能力，失败不伪装成空列表。
fn zone_list_response(
    state: &AppState,
    result: Result<Vec<RegisteredZone>, ZoneError>,
) -> Response {
    match result {
        Ok(zones) => {
            let creation = ZoneCreationView {
                available: state.create_parent_directory.is_some(),
                parent_directory: state
                    .create_parent_directory
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            };
            (StatusCode::OK, Json(ZoneListResponse { zones, creation })).into_response()
        }
        Err(error) => zone_error_response(error),
    }
}

fn zone_error_response(error: ZoneError) -> Response {
    let status = match &error {
        ZoneError::AlreadyRegistered => StatusCode::CONFLICT,
        ZoneError::NotRegistered => StatusCode::NOT_FOUND,
        ZoneError::Unreadable(_) | ZoneError::Corrupt(_) | ZoneError::WriteFailed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ZoneError::Required
        | ZoneError::NotAbsolute
        | ZoneError::NotFound
        | ZoneError::NotADirectory
        | ZoneError::Inaccessible(_) => StatusCode::BAD_REQUEST,
    };
    rejected_owned(status, error.code(), error.message())
}

fn rejected_owned(status: StatusCode, code: &'static str, message: String) -> Response {
    (status, Json(ZoneErrorResponse { code, message })).into_response()
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
                "[herdr-bot] {name} 调用已结束：{}，{status}",
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
                "[herdr-bot] {name} 调用已结束：无法观测脚本进程的结束结果（{}）：{error}",
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
                "[herdr-bot] {name} 调用已结束：脚本无法启动（{}，工作目录 {}）：{reason}",
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
    eprintln!("[herdr-bot] 请求被拒绝：{code}（HTTP {}）", status.as_u16());
    (status, Json(RejectedResponse { code, message })).into_response()
}
