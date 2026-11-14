use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
use crate::{AppState, CancelOutcome, ShutdownPhase, ShutdownView};
use super::{rejected, extractors::system::{ClosedOperation, MonitoringCommand}, responses::HttpError};
/// “手动关机”：确认后提交开始意图，进入后端权威倒计时。
///
/// 它与固定操作的执行资格**完全解耦**：不获取、不释放、不排队，也不执行“停止”脚本，因此脚本操作
/// 执行期间它照样被接受。回执只说明开始意图被接受（`outcome` 是安排当前阶段），**不**表示关机指令
/// 已经发出；到期执行由后端自己负责（见 [`shutdown`]）。
pub(crate) async fn start_shutdown(State(state): State<AppState>, _: ClosedOperation) -> Response {
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
pub(crate) async fn cancel_shutdown(State(state): State<AppState>, _: ClosedOperation) -> Response {
    match state.shutdown.cancel_with_snapshot().await {
        Ok(shutdown) => (
            StatusCode::OK,
            Json(CancelAccepted {
                outcome: CancelOutcome::Cancelled,
                shutdown,
            }),
        )
            .into_response(),
        Err(_) => rejected(
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
pub(crate) async fn monitor_shutdown(State(state): State<AppState>, command: MonitoringCommand) -> Response {
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
        Err(error) => HttpError(error).into_response(),
    }
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

