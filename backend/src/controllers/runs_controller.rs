//! HTTP adapters for Run history and manager/node tools.
use crate::{AppState, runs};
use axum::{Json, extract::{State, Path}, response::{IntoResponse, Response}};
use super::{
    extractors::{CommandJson, runs::{self as input, LocalRunSource, RunId}},
    responses::{HttpError, runs::RunResponse},
};

type RunHttpResult = Result<RunResponse, HttpError<runs::RunError>>;

pub(crate) async fn manager_prepare_inputs(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::preparation::PrepareInputsRequest, input::PrepareInputs>,
) -> RunHttpResult {
    runs::preparation::manager_prepare_inputs(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_collect(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::StatusRequest, input::Collect>,
) -> Response {
    match runs::collect::manager_collect(state, input.value).await {
        Ok(runs::collect::CollectOutcome::Ready(value)) => Json(value).into_response(),
        Ok(runs::collect::CollectOutcome::Waiting(wait)) => {
            // Dropping the HTTP body cancels the business wait and releases its receiver.
            let body = futures_util::stream::once(async move {
                let value = wait.wait().await.map_err(std::io::Error::other)?;
                serde_json::to_vec(&value).map_err(std::io::Error::other)
            });
            ([(axum::http::header::CONTENT_TYPE, "application/json")], axum::body::Body::from_stream(body)).into_response()
        },
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn node_context(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<crate::scheduler::JsonObject<runs::reports::ContextRequest>, input::NodeContext>,
) -> RunHttpResult {
    runs::reports::node_context(state, input.value.0).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn node_report(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::reports::ReportRequest, input::NodeReport>,
) -> RunHttpResult {
    runs::reports::node_report(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_retry(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::retry::RetryRequest, input::Retry>,
) -> RunHttpResult {
    runs::retry::manager_retry(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_reconcile_dispatch(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::recovery::ReconcileRequest, input::ReconcileDispatch>,
) -> RunHttpResult {
    runs::recovery::manager_reconcile_dispatch(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_recover(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::recovery::RecoverRequest, input::Recover>,
) -> RunHttpResult {
    runs::recovery::manager_recover(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_fail(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::recovery::FailRequest, input::Fail>,
) -> RunHttpResult {
    runs::recovery::manager_fail(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_complete(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::completion::CompleteRequest, input::Complete>,
) -> RunHttpResult {
    runs::completion::manager_complete(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_finish(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::StatusRequest, input::Finish>,
) -> RunHttpResult {
    runs::completion::manager_finish(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_pause(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::StatusRequest, input::PauseOrResume>,
) -> RunHttpResult {
    runs::completion::manager_pause(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_resume(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::StatusRequest, input::PauseOrResume>,
) -> RunHttpResult {
    runs::completion::manager_resume(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_status(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::StatusRequest, input::Status>,
) -> RunHttpResult {
    runs::manager_status(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn manager_dispatch(
    State(state): State<AppState>,
    _: LocalRunSource,
    input: CommandJson<runs::DispatchRequest, input::Dispatch>,
) -> RunHttpResult {
    runs::manager_dispatch(state, input.value).await.map(RunResponse).map_err(HttpError)
}

pub(crate) async fn list_runs(State(state): State<AppState>) -> RunHttpResult {
    runs::list_runs(state).await.map(RunResponse).map_err(HttpError)
}
pub(crate) async fn get_run(State(state): State<AppState>, Path(id): Path<String>) -> RunHttpResult {
    runs::get_run(state, id).await.map(RunResponse).map_err(HttpError)
}
pub(crate) async fn rename_run(
    State(state): State<AppState>,
    RunId(id): RunId,
    input: CommandJson<runs::RenameRequest, input::Rename>,
) -> RunHttpResult {
    runs::rename_run(state, id, input.value).await.map(RunResponse).map_err(HttpError)
}
pub(crate) async fn start_run(
    State(state): State<AppState>,
    Path(map): Path<String>,
    input: CommandJson<crate::runs::preflight::RunPreflightRequest, input::Start>,
) -> RunHttpResult {
    runs::start_run(state, map, input.value).await.map(RunResponse).map_err(HttpError)
}
