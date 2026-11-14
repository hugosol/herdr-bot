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
//! （见 [`shutdown`]），到期后由后端自己调用本机 Windows 优雅关机（见 [`shutdown::power_off`]）。
//! 关机与固定操作**完全解耦**——不获取执行资格、不排队、不执行“停止”脚本，因此脚本操作执行期间
//! 它照样被接受；两条命令的请求面同样封闭，带查询串或请求体的调用被拒绝且不产生任何副作用。
//!
//! 还有一条 `POST /api/shutdown/monitor`：为某个会话设置或撤销**完成后关机监听**（明确的目标选中值，
//! 不按当前状态翻转）。集合与自动触发规则同样由 [`shutdown`] 在一把锁内裁定：只有当前在线且工作中的
//! 未监听会话可新增，集合非空且全部成员当前等待用户输入才成立唯一的自动安排。
//!
//! 浏览器读取会话与关机状态用同一条只读长连接（见 [`sessions::observation`]）：首帧同时给出会话快照与关机状态，
//! 之后关机变化以独立命名事件携带完整状态。没有第二条关机观察连接。
//!
//! 同一实例内任意时刻至多一个脚本操作在执行：两个操作共用一个执行资格，资格从操作被接受到脚本
//! 确认结束一直持有，期间到达的其他请求直接得到忙碌结果（HTTP 409 `operation-in-progress`），
//! 既不等待也不在之后补做。互斥范围限单个后端实例，覆盖整个脚本执行期间，不覆盖脚本另开的长期进程。
//!
//! 执行资格由**执行本身**持有，而不是由提交请求的那条连接持有：客户端中途断开既不取消已接受的操作，
//! 也不提前释放资格，脚本继续执行到它自己的结束；断线只意味着这一次的结果没有接收者（不恢复、不重放、不留操作历史）。

#[cfg(test)]
extern crate self as herdr_bot;

pub mod chats;
pub(crate) mod controllers;
pub mod herdr;
mod chief;
mod chief_delegation;
mod chief_reports;
pub mod clock;
pub mod config;
pub mod paths;
pub mod mdns;
pub mod operations;
pub mod sessions;
pub mod scheduler;
pub mod runs;
mod report_delivery;
pub mod files;
pub mod prompts;
pub mod shutdown;
pub mod tasks;
pub mod suites;
pub mod zones;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use axum::Router;
use axum::routing::{any, get, post};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

pub use config::{
    TasksConfig, CommandTimeoutSeconds, Config, CountdownSeconds, GitConfig, HerdrConfig,
    MdnsConfig, OperationBinding, Operations, SessionWindowSeconds, SessionsConfig, ShutdownConfig,
    StatusCacheTtlSeconds, ZonesConfig, SchedulerConfig, CollectWaitSeconds,
};
use chats::{history, resume};
use zones::watch;
use zones::git::{commit, sync};

pub use shutdown::{CancelOutcome, MonitorRefusal, ShutdownPhase, ShutdownState, ShutdownView};
pub use operations::{ScriptOperation, console_available};
pub use sessions::SessionPolicy;
pub use shutdown::power_off::{PowerOff, PowerOffFailure, SystemPowerOff};

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
    /// Default native history roots for Retro reference text (Pi, OMP).
    pub(crate) native_session_directories: [Option<PathBuf>; 2],
    pub(crate) chief: Arc<chief::Authority>,
    pub(crate) delegations: Arc<chief_delegation::Delegations>,
    pub(crate) chief_reports: Arc<chief_reports::Reports>,
    /// 后端权威的关机倒计时：手动开始意图建立安排，到期由后端自己调用 [`PowerOff`]。
    ///
    /// 与执行资格完全解耦：既有脚本操作在执行，关机安排照样能被建立，也不执行“停止”脚本。
    /// 实际关机动作接缝由它持有（注入的记录型实现只用于自动化与人工验收的安全外壳）。
    pub shutdown: Arc<shutdown::ShutdownState>,
    /// zone 登记列表：由后端以文件持久保存，电脑与手机共享同一份。
    ///
    /// 与固定操作、关机路径完全解耦：登记只读取与写入登记文件，不执行任何脚本。
    pub zones: Arc<zones::Zones>,
    /// Task：由后端以文件持久保存，电脑与手机共享同一份。
    ///
    /// 与固定操作、关机路径完全解耦：Task 管理只读取与写入 Task 存储文件，不执行任何脚本、
    /// 不创建也不关闭 Chat。
    pub tasks: Arc<tasks::Tasks>,
    pub suites: Arc<suites::Suites>,
    /// Service-owned Map root, never derived from zone or request data.
    pub map_directory: PathBuf,
    /// Service-owned shared capability definitions, independent of Map storage.
    pub bot_directory: PathBuf,
    pub run_directory: PathBuf,
    pub run_instructions: PathBuf,
    pub prompts: prompts::PromptDictionary,
    pub collect_wait: Duration,
    pub workflow_poll_interval_seconds: u32,
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
    let shutdown = shutdown::ShutdownState::new(
        Arc::clone(&sessions),
        Arc::clone(&power_off),
        config.shutdown.countdown_seconds.duration(),
    );
    let sync_state = Arc::new(sync::SyncState::with_ttl(
        config.git.status_cache_ttl_seconds.duration(),
    ));
    let suites = Arc::new(suites::Suites::new(config.suites.definition_file.clone(), config.suites.mcp_file.clone(), config.suites.generated_directory.clone()));
    let chief = Arc::new(chief::Authority::new(config.sessions.history_file.with_extension("chief.json")));
    sessions.attach_chief(Arc::clone(&chief));
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
        native_session_directories: ["${roots.pi_agent}/sessions", "${roots.omp_agent}/sessions"]
            .map(|path| config.roots.resolve(Path::new(path), Path::new(".")).ok()),
        chief,
        delegations: Arc::new(chief_delegation::Delegations::new(config.sessions.history_file.with_extension("delegations.json"))),
        chief_reports: Arc::new(chief_reports::Reports::new(config.sessions.history_file.with_extension("chief-reports.jsonl"))),
        sessions,
        shutdown,
        zones: Arc::new(zones::Zones::new(config.zones.registry_file.clone())),
        tasks: Arc::new(tasks::Tasks::new(
            config.tasks.storage_file.clone(),
            config.tasks.metadata_file.clone(),
            config.roots.clone(),
            Arc::clone(&suites),
        )),
        suites,
        map_directory: std::path::absolute(&config.scheduler.map_directory).expect("resolve Map root"),
        bot_directory: std::path::absolute(&config.scheduler.bot_directory).expect("resolve Bot root"),
        run_directory: std::path::absolute(&config.scheduler.run_directory).expect("resolve Run root"),
        run_instructions: std::path::absolute(&config.scheduler.run_instructions).expect("resolve Run instructions"),
        prompts: prompts::PromptDictionary::new(config.prompts_file.clone()),
        collect_wait: config.scheduler.collect_wait_seconds.duration(),
        workflow_poll_interval_seconds: config.scheduler.workflow_poll_interval_seconds.get(),
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
    let page = ServeDir::new(&config.static_directory);

    let router = Router::new()
        // 只有这两个操作名可用；执行目标与工作目录只来自服务端配置。
        .route("/api/operations/start", post(controllers::server_controller::start_operation))
        .route("/api/operations/stop", post(controllers::server_controller::stop_operation))
        // 关机命令永远注册：开始意图只建立后端权威倒计时，取消是全局清空意图。
        // 两条命令都不直接关机，也没有绕过倒计时的即时关机别名。
        .route("/api/shutdown/start", post(controllers::shutdown_controller::start_shutdown))
        .route("/api/shutdown/cancel", post(controllers::shutdown_controller::cancel_shutdown))
        // 完成后关机监听：设置或撤销某个会话的明确目标选中值。
        .route("/api/shutdown/monitor", post(controllers::shutdown_controller::monitor_shutdown))
        // 会话与关机的只读长连接：同一路径上写入（POST，扩展上报的唯一入口）与读取（GET，SSE）各占一半。
        .route(
            "/api/sessions/events",
            get(controllers::sessions_controller::sse_observation).post(controllers::sessions_controller::post_session_event),
        )
        // 历史与备忘沿用浏览器局域网读写边界；不使用扩展上报的本机来源限制。
        .route("/api/sessions/history", get(controllers::chats_controller::get_history))
        .route("/api/sessions/history/memo", axum::routing::put(controllers::chats_controller::put_memo))
        // zone 登记列表：同一条路径读取与登记，取消登记是另一条明确命令。
        // 请求只携带服务器上的目录路径；执行目标仍只来自服务端配置。
        .route("/api/zones", get(controllers::zones_controller::list_zones).post(controllers::zones_controller::register_zone).put(controllers::zones_controller::rebind_zone).delete(controllers::zones_controller::unregister_zone))
        .route("/api/suites", get(controllers::suites_controller::list).post(controllers::suites_controller::create))
        .route("/api/suites/{id}", get(controllers::suites_controller::get).put(controllers::suites_controller::update).delete(controllers::suites_controller::delete))
        .route("/api/scheduler/maps", get(controllers::scheduler_controller::list_maps))
        .route("/api/scheduler/maps/{name}", get(controllers::scheduler_controller::get_map))
        .route("/api/scheduler/maps/{name}/group", axum::routing::patch(controllers::scheduler_controller::set_map_group))
        .route("/api/scheduler/bots/{id}/group", axum::routing::patch(controllers::bots_controller::set_bot_group))
        .route("/api/tasks/{id}/group", axum::routing::patch(controllers::tasks_controller::set_task_group))
        .route("/api/scheduler/maps/{name}/delete", post(controllers::scheduler_controller::delete_map))
        .route("/api/scheduler/maps/{name}/inputs", get(controllers::scheduler_controller::edit_map_inputs).post(controllers::scheduler_controller::save_map_inputs))
        .route("/api/scheduler/maps/{name}/title", get(controllers::scheduler_controller::edit_map_title).post(controllers::scheduler_controller::save_map_title))
        .route("/api/scheduler/maps/{name}/preflight", post(controllers::scheduler_controller::preflight_run))
        .route("/api/scheduler/maps/{name}/runs", post(controllers::runs_controller::start_run))
        .route("/api/workflow/runs", get(controllers::runs_controller::list_runs))
        .route("/api/workflow/runs/{id}", get(controllers::runs_controller::get_run).patch(controllers::runs_controller::rename_run))
        .route("/api/workflow/manager/status", post(controllers::runs_controller::manager_status))
        .route("/api/workflow/manager/prepare_inputs", post(controllers::runs_controller::manager_prepare_inputs))
        .route("/api/workflow/manager/collect", post(controllers::runs_controller::manager_collect))
        .route("/api/workflow/manager/dispatch", post(controllers::runs_controller::manager_dispatch))
        .route("/api/workflow/manager/complete", post(controllers::runs_controller::manager_complete))
        .route("/api/workflow/manager/fail", post(controllers::runs_controller::manager_fail))
        .route("/api/workflow/manager/recover", post(controllers::runs_controller::manager_recover))
        .route("/api/workflow/manager/retry", post(controllers::runs_controller::manager_retry))
        .route("/api/workflow/manager/reconcile_dispatch", post(controllers::runs_controller::manager_reconcile_dispatch))
        .route("/api/workflow/manager/finish", post(controllers::runs_controller::manager_finish))
        .route("/api/workflow/manager/pause", post(controllers::runs_controller::manager_pause))
        .route("/api/workflow/manager/resume", post(controllers::runs_controller::manager_resume))
        .route("/api/workflow/node/context", post(controllers::runs_controller::node_context))
        .route("/api/workflow/node/report", post(controllers::runs_controller::node_report))
        .route("/api/scheduler/maps/{name}/nodes/new", get(controllers::scheduler_controller::new_node))
        .route("/api/scheduler/maps/{name}/nodes/edit", post(controllers::scheduler_controller::edit_node))
        .route("/api/scheduler/maps/{name}/nodes/save", post(controllers::scheduler_controller::save_node))
        .route("/api/scheduler/maps/{name}/nodes/check-delete", post(controllers::scheduler_controller::check_delete_node))
        .route("/api/scheduler/maps/{name}/nodes/delete", post(controllers::scheduler_controller::delete_node))
        .route("/api/scheduler/bots", get(controllers::bots_controller::list_bots))
        .route("/api/scheduler/bots/new", get(controllers::bots_controller::new_bot))
        .route("/api/scheduler/bots/edit", post(controllers::bots_controller::edit_bot))
        .route("/api/scheduler/bots/save", post(controllers::bots_controller::save_bot))
        .route("/api/scheduler/bots/check-delete", post(controllers::bots_controller::check_bot_delete))
        .route("/api/scheduler/bots/delete", post(controllers::bots_controller::delete_bot))
        // 新 zone 只使用配置的父目录与一个直接子目录名，客户端不能提交路径。
        .route("/api/zones/create", post(controllers::zones_controller::create_zone))
        // Task：同一条路径读取列表与新增；单个 Task 按标识读取、编辑与删除。
        // Task 只写入后端存储文件，不执行脚本、不创建 Chat。
        .route(
            "/api/tasks",
            get(controllers::tasks_controller::list_tasks).post(controllers::tasks_controller::create_task),
        )
        .route(
            "/api/tasks/{id}",
            get(controllers::tasks_controller::get_task)
                .put(controllers::tasks_controller::update_task)
                .delete(controllers::tasks_controller::delete_task),
        )
        // 从已登记 zone 创建 Chat：预览只生成名称与实例标识；创建请求返回后端拥有的生命周期，
        // 之后由页面轮询同一次创建。请求只选择默认目标或按 zone 目录名匹配，不能指定任意名称。
        .route("/api/chief", get(controllers::chief_controller::get_current))
        .route("/api/chief/qualify", post(controllers::chief_controller::qualify))
        .route("/api/chief/release", post(controllers::chief_controller::release))
        .route("/api/chief/delegations/preview", post(controllers::chief_delegation_controller::preview))
        .route("/api/chief/delegations", get(controllers::chief_delegation_controller::list).post(controllers::chief_delegation_controller::commit))
        .route("/api/chief/delegations/{id}", get(controllers::chief_delegation_controller::get))
        .route("/api/chief/reports", get(controllers::chief_reports_controller::list).post(controllers::chief_reports_controller::submit))
        .route("/api/chief/reports/{id}", get(controllers::chief_reports_controller::get))
        .route("/api/chief/reports/{id}/receipt", get(controllers::chief_reports_controller::receipt))
        .route("/api/chief/reports/{id}/receive", post(controllers::chief_reports_controller::receive))
        .route("/api/chief/reports/{id}/resend", post(controllers::chief_reports_controller::resend))
        .route("/api/chief/reports/{id}/view", post(controllers::chief_reports_controller::view))
        .route("/api/chats/resume", post(controllers::chats_controller::post_resume))
        .route("/api/chats/preview", post(controllers::chats_controller::preview_chat))
        .route("/api/chats", post(controllers::chats_controller::create_chat))
        .route("/api/chats/{id}", get(controllers::chats_controller::get_chat_creation))
        // 只读浏览：整树一次读取、单目录读取、单文件读取。三者都从登记根重新核对边界。
        // 整树读取对 Git zone 应用 Git 原生忽略规则；单目录入口保留给直接调用者。
        .route("/api/zones/tree", get(controllers::zone_browser_controller::read_zone_tree))
        .route("/api/zones/entries", get(controllers::zone_browser_controller::list_zone_entries))
        .route("/api/zones/file", get(controllers::zone_browser_controller::read_zone_file))
        // HTML 阅读模式的原始资源入口：zone 身份编码在路径段中，HTML 的相对 URL 因此
        // 能自然解析到同一 zone 内的 CSS、图片等资源；每次读取仍重新核对登记根与链接边界。
        .route(
            "/api/zones/reader/{zone}/{*path}",
            get(controllers::zone_reader_controller::read_zone_reader_resource),
        )
        // Git 状态只读查询：当前分支与待提交变更；普通目录明确返回 repository=false。
        .route("/api/zones/git-status", get(controllers::zone_git_controller::zone_git_status))
        // 单个变更的审查 diff：旧侧是 HEAD，新侧是最终本地内容；两侧各自应用预览上限。
        .route("/api/zones/diff", get(controllers::zone_git_controller::zone_change_diff))
        // 经审查复核的整仓提交：确认后重查审查版本，再暂存全部符合 Git 规则的最终变更并提交。
        .route("/api/zones/commit", post(controllers::zone_git_controller::commit_zone))
        // 按需检查远端：真实 fetch 当前分支的既有 upstream，并记录最近检查时间。
        .route("/api/zones/check-remote", post(controllers::zone_git_controller::check_zone_remote))
        // 按既有 upstream 与 Git 配置同步：检查远端 → pull → 按需 push。
        .route("/api/zones/sync", post(controllers::zone_git_controller::sync_zone))
        // 登记列表的角标概览：每个 zone 的待提交项数或明确错误。
        .route("/api/zones/overview", get(controllers::zone_git_controller::zones_overview))
        // 自动更新的只读长连接：真实文件系统事件按 zone 合并后通知页面重新读取；
        // 同一 zone 的所有观察者共享一条监听，监听失效会明确下发错误而不是保持静默。
        .route("/api/zones/watch", get(controllers::zone_watch_controller::watch_zones))
        // 其余 /api/ 请求（其他操作名、怪路径）一律以可理解的 JSON 类别拒绝，
        // 既不交给静态页面，也不执行任何脚本。
        .route("/api/{*path}", any(controllers::server_controller::unknown_api_request))
        .fallback_service(page)
        .with_state(state.clone());

    (router, state)
}

