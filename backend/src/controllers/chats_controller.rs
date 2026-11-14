use axum::{Json, extract::{State, Path, Query, Request}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, chats::{self, Error}};
use super::{chat_command_body, rejected, rejected_owned};
use super::tasks_controller::task_error_with_context;
pub(crate) fn error_response(error: Error) -> Response {
    match error {
        Error::Invalid(code, message) => rejected_owned(StatusCode::BAD_REQUEST, code, message),
        Error::Conflict(code, message) => rejected_owned(StatusCode::CONFLICT, code, message),
        Error::NotFound(code, message) => rejected_owned(StatusCode::NOT_FOUND, code, message),
        Error::Internal(code, message) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, code, message),
        Error::Task(error, context) => task_error_with_context(error, context),
        Error::Zone(error) => super::zones_controller::zone_error_response(error),
        Error::Browse(error) => super::zone_browser_controller::browse_error_response(error),
        Error::Chief(error) => super::chief_controller::error_response(error),
        Error::ResumeFailure(value) => (StatusCode::BAD_GATEWAY, Json(value)).into_response(),
    }
}
pub(crate) async fn preview_chat(State(state): State<AppState>, request: Request) -> Response {
    let input = match chat_command_body(request, "Chat 名称预览").await { Ok(input) => input, Err(error) => return error };
    match chats::preview_chat(&state, input).await { Ok(view) => Json(view).into_response(), Err(error) => error_response(error) }
}
pub(crate) async fn create_chat(State(state): State<AppState>, request: Request) -> Response {
    let input = match chat_command_body(request, "创建 Chat").await { Ok(input) => input, Err(error) => return error };
    match chats::create_chat(state, input).await { Ok(view) => (StatusCode::ACCEPTED, Json(view)).into_response(), Err(error) => error_response(error) }
}
pub(crate) async fn get_chat_creation(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Response {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected(StatusCode::BAD_REQUEST,"unexpected-parameters","创建状态读取不接受查询参数：目标只由路径中的创建标识决定。");
    }
    match state.chat_creations.get(&id).await {
        Some(view) => Json(view).into_response(),
        None => rejected(StatusCode::NOT_FOUND,"creation-not-found","找不到这次创建；它可能不属于当前后端实例。"),
    }
}
pub(crate) async fn get_history(State(state): State<AppState>, Query(query): Query<chats::history::HistoryQuery>) -> Response {
    match chats::history::get_history(&state, query).await { Ok(value) => Json(value).into_response(), Err(error) => error_response(error) }
}
pub(crate) async fn put_memo(State(state): State<AppState>, Json(input): Json<chats::history::MemoRequest>) -> Response {
    match chats::history::put_memo(&state, &input).await { Ok(()) => Json(input).into_response(), Err(error) => error_response(error) }
}
pub(crate) async fn post_resume(State(state): State<AppState>, Json(input): Json<chats::resume::ResumeRequest>) -> Response {
    match chats::resume::post_resume(state, input).await { Ok(value) => Json(value).into_response(), Err(error) => error_response(error) }
}
