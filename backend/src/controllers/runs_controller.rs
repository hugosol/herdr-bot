//! HTTP adapters for Run history and manager/node tools.
use crate::{AppState, runs::{self, RunOutcome, RunError, RunResult}};
use axum::{Json, extract::{State, Path, Request, ConnectInfo}, http::StatusCode, response::{IntoResponse, Response}};
use std::net::SocketAddr;

pub(crate) fn respond(result: RunResult) -> Response {
    match result {
        Ok(RunOutcome::Value(value)) => Json(value).into_response(),
        Ok(RunOutcome::Rejected(value)) => (StatusCode::CONFLICT, Json(value)).into_response(),
        Ok(RunOutcome::Started { failed, value }) => (if failed { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::CREATED }, Json(value)).into_response(),
        Err(RunError::Invalid(value)) => (StatusCode::BAD_REQUEST, Json(value)).into_response(),
        Err(RunError::Forbidden(value)) => (StatusCode::FORBIDDEN, Json(value)).into_response(),
        Err(RunError::Failed(value)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(value)).into_response(),
        Err(RunError::Preflight(error)) => (StatusCode::BAD_REQUEST, Json(error.into_payload())).into_response(),
        Err(RunError::Task(error)) => crate::controllers::tasks_controller::task_error_response(error),
    }
}

pub(crate) async fn manager_prepare_inputs(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::preparation::PrepareInputsRequest = match crate::controllers::chat_command_body(request, "准备 Run 输入").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::preparation::manager_prepare_inputs(state, input).await)
}

pub(crate) async fn manager_collect(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::StatusRequest = match crate::controllers::chat_command_body(request, "收取报告").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    match runs::collect::manager_collect(state, input).await {
        Ok(runs::collect::CollectOutcome::Ready(value)) => Json(value).into_response(),
        Ok(runs::collect::CollectOutcome::Waiting(wait)) => {
            // Dropping the HTTP body cancels the business wait and releases its receiver.
            let body = futures_util::stream::once(async move {
                let value = wait.wait().await.map_err(std::io::Error::other)?;
                serde_json::to_vec(&value).map_err(std::io::Error::other)
            });
            ([(axum::http::header::CONTENT_TYPE, "application/json")], axum::body::Body::from_stream(body)).into_response()
        },
        Err(error) => respond(Err(error)),
    }
}

pub(crate) async fn node_context(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::reports::ContextRequest = match crate::controllers::chat_command_body::<crate::scheduler::JsonObject<runs::reports::ContextRequest>>(request, "节点上下文").await {
        Ok(input) => input.0,
        Err(response) => return response,
    };
    respond(runs::reports::node_context(state, input).await)
}

pub(crate) async fn node_report(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::reports::ReportRequest = match crate::controllers::chat_command_body(request, "节点报告").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::reports::node_report(state, input).await)
}

pub(crate) async fn manager_retry(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::retry::RetryRequest = match crate::controllers::chat_command_body(request, "重试独立新执行").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::retry::manager_retry(state, input).await)
}

pub(crate) async fn manager_reconcile_dispatch(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::recovery::ReconcileRequest = match crate::controllers::chat_command_body(request, "核实派发事实").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::recovery::manager_reconcile_dispatch(state, input).await)
}

pub(crate) async fn manager_recover(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::recovery::RecoverRequest = match crate::controllers::chat_command_body(request, "恢复原执行跟踪").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::recovery::manager_recover(state, input).await)
}

pub(crate) async fn manager_fail(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::recovery::FailRequest = match crate::controllers::chat_command_body(request, "记录执行失败").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::recovery::manager_fail(state, input).await)
}

pub(crate) async fn manager_complete(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::completion::CompleteRequest = match crate::controllers::chat_command_body(request, "交接完成").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::completion::manager_complete(state, input).await)
}

pub(crate) async fn manager_finish(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::StatusRequest = match crate::controllers::chat_command_body(request, "结束 Run").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::completion::manager_finish(state, input).await)
}

pub(crate) async fn manager_pause(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::StatusRequest = match crate::controllers::chat_command_body(request, "暂停或恢复派发资格").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::completion::manager_pause(state, input).await)
}

pub(crate) async fn manager_resume(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::StatusRequest = match crate::controllers::chat_command_body(request, "暂停或恢复派发资格").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::completion::manager_resume(state, input).await)
}

pub(crate) async fn manager_status(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::StatusRequest = match crate::controllers::chat_command_body(request, "运行状态").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::manager_status(state, input).await)
}

pub(crate) async fn manager_dispatch(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = runs::authorize_source(peer.ip()) { return respond(Err(error)); }
    let input: runs::DispatchRequest = match crate::controllers::chat_command_body(request, "派发节点").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    respond(runs::manager_dispatch(state, input).await)
}

pub(crate) async fn list_runs(State(state): State<AppState>) -> Response {
    respond(runs::list_runs(state).await)
}
pub(crate) async fn get_run(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    respond(runs::get_run(state, id).await)
}
pub(crate) async fn rename_run(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Response {
    if let Err(error) = runs::validate_id(&id) { return respond(Err(error)); }
    let input = match crate::controllers::chat_command_body(request, "重命名 Run").await {
        Ok(input) => input, Err(response) => return response,
    };
    respond(runs::rename_run(state, id, input).await)
}
pub(crate) async fn start_run(State(state): State<AppState>, Path(map): Path<String>, request: Request) -> Response {
    let input = match crate::controllers::chat_command_body(request, "发起 Run").await {
        Ok(input) => input, Err(response) => return response,
    };
    respond(runs::start_run(state, map, input).await)
}
