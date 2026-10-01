//! omp-bot 主界面后端。
//!
//! 同一个 Axum 服务提供三件东西：打包后的 React 页面（静态资源）、固定操作 HTTP interface，
//! 以及会话事件 HTTP interface。页面与接口同源同端口，本机与局域网设备访问同一服务。
//!
//! 会话事件接口（见 [`sessions`]）的写入侧是 `POST /api/sessions/events`：扩展上报会话的
//! register / heartbeat / state / unregister，服务端把它们写进**进程内**会话表；同一路径的读侧
//! 长连接由浏览器订阅会话变化。写入只接受本机来源（TCP 对端为回环或本机网卡地址），其余得到
//! `403 forbidden`；读侧对局域网全开、不引入 token。在线与否只由服务端按 30s 判定，后台过期扫描把
//! 超时的在线会话翻为离线并作为可见变化推给浏览器，离线行保留到显式 `unregister`（扫描节奏与启动位置
//! 见 [`sessions`]）。会话表与固定操作、关机路径完全解耦——写事件既不取执行资格，也不接触任何脚本。
//!
//! 操作接口只暴露“启动”和“停止”两个固定操作，它们执行哪个脚本、用什么工作目录只由服务端
//! 配置决定。客户端只能提交操作名：请求面是封闭的，其他操作名、怪路径，以及携带命令、脚本
//! 路径或参数的请求都会被拒绝，且拒绝不产生任何脚本执行副作用。
//!
//! 另有 `POST /api/shutdown`：无参数调用即触发本机 Windows 优雅关机（见 [`shutdown`]）。
//! 它与固定操作**完全解耦**——不获取执行资格、不排队、不执行“停止”脚本，因此脚本操作执行期间
//! 它照样被接受；它的请求面同样封闭，带查询串或请求体的调用被拒绝且不触发任何动作。
//!
//! 同一实例内任意时刻至多一个脚本操作在执行：两个操作共用一个执行资格，资格从操作被接受到脚本
//! 确认结束一直持有，期间到达的其他请求直接得到忙碌结果（HTTP 409 `operation-in-progress`），
//! 既不等待也不在之后补做。互斥范围限单个后端实例，覆盖整个脚本执行期间，不覆盖脚本另开的长期进程。
//!
//! 执行资格由**执行本身**持有，而不是由提交请求的那条连接持有：客户端中途断开既不取消已接受的操作，
//! 也不提前释放资格，脚本继续执行到它自己的结束；断线只意味着这一次的结果没有接收者（不恢复、不重放、不留操作历史）。

pub mod config;
pub mod mdns;
pub mod operations;
pub mod sessions;
pub mod shutdown;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::to_bytes;
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use serde::Serialize;
use tokio::sync::{Mutex, oneshot};
use tower_http::services::ServeDir;

pub use config::{Config, MdnsConfig, OperationBinding, Operations};
pub use operations::{ScriptOperation, console_available};
pub use shutdown::{PowerOff, PowerOffFailure, SystemPowerOff};

use operations::Invocation;
use shutdown::ShutdownOutcome;

/// 操作请求的请求体读取上限。操作请求不接受请求体，任何请求体都注定被拒绝，
/// 因此只要够读完空请求体即可，不必为一个越界请求缓冲任意大的内容。
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024;

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
    /// 关机动作接缝。交付运行时是真实的 Win32 动作（[`SystemPowerOff`]），自动化测试注入记录型
    /// 实现；关机路径**只**经由它，因此它不接触上面那一个执行资格。
    power_off: Arc<dyn PowerOff>,
    /// 进程内会话表：扩展写事件到这里，浏览器读路径从这里订阅。
    ///
    /// 它与固定操作、关机路径完全解耦：会话写入既不取执行资格，也不接触任何脚本。
    pub sessions: Arc<sessions::SessionsState>,
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
/// 会话表用新建的空表（见 [`app_with_sessions`]）。
pub fn app_with_power_off(config: &Config, power_off: Arc<dyn PowerOff>) -> Router {
    app_with_sessions(config, power_off, Arc::new(sessions::SessionsState::new()))
}

/// 构造使用**指定**关机动作与**指定**会话表的应用。
///
/// 交付运行时（`main.rs`）用本函数持有会话表，并把同一份表交给后台过期扫描；需要观察会话变化的
/// 调用方（会话过期扫描的自动化测试等）注入自己持有的会话表，于是写入的可见变化可以被订阅到，
/// 而路由注册与交付运行时完全一致。
pub fn app_with_sessions(
    config: &Config,
    power_off: Arc<dyn PowerOff>,
    sessions: Arc<sessions::SessionsState>,
) -> Router {
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
        power_off,
        sessions,
    };

    // 打包页面与操作接口由同一个服务提供：静态目录挂在 fallback 上，
    // "/" 由目录索引提供 index.html；页面没有前端路由，未知路径即 404。
    let page = ServeDir::new(static_dir());

    Router::new()
        // 只有这两个操作名可用；执行目标与工作目录只来自服务端配置。
        .route("/api/operations/start", post(start_operation))
        .route("/api/operations/stop", post(stop_operation))
        // 关机永远注册：只要服务在运行，无参数 POST 就能触发本机优雅关机。
        .route("/api/shutdown", post(shutdown))
        // 会话事件：同一路径上写入（POST，扩展上报的唯一入口）与只读长连接（GET，SSE）各占一半。
        .route(
            "/api/sessions/events",
            get(sessions::sse_session_events).post(sessions::post_session_event),
        )
        // 其余 /api/ 请求（其他操作名、怪路径）一律以可理解的 JSON 类别拒绝，
        // 既不交给静态页面，也不执行任何脚本。
        .route("/api/{*path}", any(unknown_api_request))
        .fallback_service(page)
        .with_state(state)
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

/// 关机成功的结果类别（HTTP 200）；失败类别是 HTTP 500 的 `shutdown-failed`，两者互相区分。
const SHUTDOWN_ISSUED: &str = "shutdown-issued";

/// 一次成功的关机请求：只说明“关机已发出”。
#[derive(Debug, Serialize)]
struct ShutdownResponse {
    /// 结果类别：`shutdown-issued`。
    outcome: &'static str,
}

/// “关机”：无参数 POST 触发本机优雅关机。
///
/// 与固定操作的执行资格**完全解耦**：不获取、不释放、不排队，也不执行“停止”脚本，因此脚本操作
/// 执行期间它照样被接受，也不改变忙碌语义。关机动作是同步的：`Ok` 回 200（关机已发出），
/// 失败回 500 `shutdown-failed`，原始错误只写诊断输出。
async fn shutdown(State(state): State<AppState>, request: Request) -> Response {
    // 请求面与其他操作一样封闭，且拒绝发生在调用动作之前：被拒绝的请求不触发任何关机动作。
    if let Some(rejection) = reject_unclosed_request(request).await {
        return rejection;
    }

    match state.power_off.power_off() {
        Ok(()) => {
            shutdown::report(&ShutdownOutcome::Invoked);
            (StatusCode::OK, Json(ShutdownResponse { outcome: SHUTDOWN_ISSUED })).into_response()
        }
        Err(failure) => {
            shutdown::report(&ShutdownOutcome::Failed(failure));
            shutdown_failed()
        }
    }
}

/// 关机动作失败的响应：与请求面拒绝同一个响应形状（只给类别，不回显任何执行目标或原始错误），
/// 但语义是“请求被接受、动作失败”，因此不复用请求拒绝的诊断输出——类别事实由
/// [`shutdown::report`] 的 `failed` 一行给出。
fn shutdown_failed() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(RejectedResponse {
            code: "shutdown-failed",
            message: "关机动作失败：本机没有接受这次关机。",
        }),
    )
        .into_response()
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
