use axum::{Json, extract::{State, Path, Query, Request, ConnectInfo}, http::StatusCode, response::{IntoResponse, Response}};
use std::net::SocketAddr;
use crate::{AppState, chief_reports::{self, Error}};
use super::{chat_command_body, rejected_owned};
fn error_response(error: Error) -> Response {
    match error {
        Error::Invalid(code, message) => rejected_owned(StatusCode::BAD_REQUEST, code, message),
        Error::Forbidden(code, message) => rejected_owned(StatusCode::FORBIDDEN, code, message),
        Error::NotFound(code, message) => rejected_owned(StatusCode::NOT_FOUND, code, message),
        Error::Internal(code, message) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, code, message),
        Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"chief-report-storage-failed",format!("无法读取报告或其委派关联：{error}")),
        Error::Chief(error) => super::chief_controller::error_response(error),
        Error::Save(message) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"code":"report-save-failed","message":message,"saved":false,"notification":{"status":"not-attempted"}}))).into_response(),
    }
}
fn respond(result: Result<serde_json::Value, Error>) -> Response {
    match result { Ok(value) => Json(value).into_response(), Err(error) => error_response(error) }
}
pub(crate) async fn submit(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = chief_reports::authorize_source(peer.ip()) { return error_response(error); }
    let input = match chat_command_body(request, "提交 chief 报告").await { Ok(input) => input, Err(error) => return error };
    respond(chief_reports::submit(state, input).await)
}
pub(crate) async fn list(State(state): State<AppState>, Query(filter): Query<chief_reports::Filter>) -> Response { respond(chief_reports::list(&state, filter).await) }
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response { respond(chief_reports::get(&state, id).await) }
pub(crate) async fn receipt(State(state): State<AppState>, Path(id): Path<String>) -> Response { respond(chief_reports::receipt(&state, id).await) }
pub(crate) async fn receive(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = chief_reports::authorize_source(peer.ip()) { return error_response(error); }
    let input = match chat_command_body(request, "可靠接收 chief 报告").await { Ok(input) => input, Err(error) => return error };
    respond(chief_reports::receive(state, id, input).await)
}
pub(crate) async fn view(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = chief_reports::authorize_source(peer.ip()) { return error_response(error); }
    let input = match chat_command_body(request, "明确查看 chief 原报告").await { Ok(input) => input, Err(error) => return error };
    respond(chief_reports::view(state, id, input).await)
}
pub(crate) async fn resend(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if let Err(error) = chief_reports::authorize_source(peer.ip()) { return error_response(error); }
    let input = match chat_command_body(request, "人工重发 chief 报告").await { Ok(input) => input, Err(error) => return error };
    respond(chief_reports::resend(state, id, input).await)
}
