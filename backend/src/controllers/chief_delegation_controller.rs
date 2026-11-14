use axum::{Json, extract::{State, Path, Query}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, chief_delegation::{self, Outcome}};
use super::{
    extractors::{CommandJson, chief::{PreviewDelegation, CommitDelegation}},
    responses::{chief_delegation::error_response, runs::RunResponse},
};


pub(crate) async fn preview(State(state): State<AppState>, input: CommandJson<chief_delegation::DispatchRequest, PreviewDelegation>) -> Response {
    let input = input.value;
    match chief_delegation::preview(&state, input).await { Ok(value) => Json(value).into_response(), Err(error) => error_response(error).await }
}
pub(crate) async fn commit(State(state): State<AppState>, input: CommandJson<chief_delegation::CommitRequest, CommitDelegation>) -> Response {
    let input = input.value;
    match chief_delegation::commit(state, input).await {
        Ok(Outcome::Created(view)) => (StatusCode::ACCEPTED, Json(view)).into_response(),
        Ok(Outcome::Run(outcome)) => RunResponse(outcome).into_response(),
        Err(error) => error_response(error).await,
    }
}
pub(crate) async fn list(State(state): State<AppState>, Query(query): Query<chief_delegation::SenderQuery>) -> Response {
    match chief_delegation::list(&state, query) { Ok(value) => Json(value).into_response(), Err(error) => error_response(error).await }
}
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match chief_delegation::get(&state, &id) { Ok(record) => Json(record).into_response(), Err(error) => error_response(error).await }
}
