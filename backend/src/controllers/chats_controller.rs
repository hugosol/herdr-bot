use axum::{Json, extract::{State, Path, Query, Request}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, chats};
use super::{rejected, extractors::{CommandJson, chats::{PreviewChat, CreateChat}}, responses::HttpError};
pub(crate) async fn preview_chat(State(state): State<AppState>, input: CommandJson<chats::ChatPreviewRequest, PreviewChat>) -> Response {
    let input = input.value;
    match chats::preview_chat(&state, input).await { Ok(view) => Json(view).into_response(), Err(error) => HttpError(error).into_response() }
}
pub(crate) async fn create_chat(State(state): State<AppState>, input: CommandJson<chats::CreateChatRequest, CreateChat>) -> Response {
    let input = input.value;
    match chats::create_chat(state, input).await { Ok(view) => (StatusCode::ACCEPTED, Json(view)).into_response(), Err(error) => HttpError(error).into_response() }
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
    match chats::history::get_history(&state, query).await { Ok(value) => Json(value).into_response(), Err(error) => HttpError(error).into_response() }
}
pub(crate) async fn put_memo(State(state): State<AppState>, Json(input): Json<chats::history::MemoRequest>) -> Response {
    match chats::history::put_memo(&state, &input).await { Ok(()) => Json(input).into_response(), Err(error) => HttpError(error).into_response() }
}
pub(crate) async fn post_resume(State(state): State<AppState>, Json(input): Json<chats::resume::ResumeRequest>) -> Response {
    match chats::resume::post_resume(state, input).await { Ok(value) => Json(value).into_response(), Err(error) => HttpError(error).into_response() }
}
