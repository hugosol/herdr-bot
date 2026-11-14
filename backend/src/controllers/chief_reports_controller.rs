use axum::{Json, extract::{State, Path, Query}};
use crate::{AppState, chief_reports};
use super::{
    extractors::{CommandJson, chief::{LocalReportSource, SubmitReport, ReceiveReport, ViewReport, ResendReport}},
    responses::HttpError,
};

type ReportHttpResult = Result<Json<serde_json::Value>, HttpError<chief_reports::Error>>;
pub(crate) async fn submit(
    State(state): State<AppState>,
    _: LocalReportSource,
    input: CommandJson<chief_reports::Submission, SubmitReport>,
) -> ReportHttpResult {
    chief_reports::submit(state, input.value).await.map(Json).map_err(HttpError)
}
pub(crate) async fn list(State(state): State<AppState>, Query(filter): Query<chief_reports::Filter>) -> ReportHttpResult {
    chief_reports::list(&state, filter).await.map(Json).map_err(HttpError)
}
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> ReportHttpResult {
    chief_reports::get(&state, id).await.map(Json).map_err(HttpError)
}
pub(crate) async fn receipt(State(state): State<AppState>, Path(id): Path<String>) -> ReportHttpResult {
    chief_reports::receipt(&state, id).await.map(Json).map_err(HttpError)
}
pub(crate) async fn receive(
    State(state): State<AppState>, Path(id): Path<String>,
    _: LocalReportSource,
    input: CommandJson<chief_reports::Action, ReceiveReport>,
) -> ReportHttpResult {
    chief_reports::receive(state, id, input.value).await.map(Json).map_err(HttpError)
}
pub(crate) async fn view(
    State(state): State<AppState>, Path(id): Path<String>,
    _: LocalReportSource,
    input: CommandJson<chief_reports::Action, ViewReport>,
) -> ReportHttpResult {
    chief_reports::view(state, id, input.value).await.map(Json).map_err(HttpError)
}
pub(crate) async fn resend(
    State(state): State<AppState>, Path(id): Path<String>,
    _: LocalReportSource,
    input: CommandJson<chief_reports::Action, ResendReport>,
) -> ReportHttpResult {
    chief_reports::resend(state, id, input.value).await.map(Json).map_err(HttpError)
}
