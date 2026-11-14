use axum::{Json, extract::{State, Path, Query, Request}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, chief_delegation::{self, Error, Outcome}};
use super::{chat_command_body, rejected_owned};

async fn error_response(error: Error) -> Response {
    match error {
        Error::Conflict(code, message) => rejected_owned(StatusCode::CONFLICT, code, message),
        Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"delegation-storage-failed",format!("委派记录无法可靠保存或读取：{error}")),
        Error::Chief(error) => super::chief_controller::error_response(error),
        Error::Chat(error) => super::chats_controller::error_response(error),
        Error::Task(error) => super::tasks_controller::task_error_response(error),
        Error::Directory(error) => super::scheduler_controller::definition_error(error),
        Error::Definition(crate::scheduler::DelegationDefinitionError::Directory(error)) => super::scheduler_controller::definition_error(error),
        Error::Definition(crate::scheduler::DelegationDefinitionError::Prompt(message)) => rejected_owned(StatusCode::CONFLICT,"prompt-dictionary-invalid",message),
        Error::Preflight(error) => (StatusCode::BAD_REQUEST, Json(error.into_payload())).into_response(),
        Error::Run(error) => super::runs_controller::respond(Err(error)),
        Error::NotFound => rejected_owned(StatusCode::NOT_FOUND,"delegation-not-found","委派记录不存在。".into()),
        Error::Stale(cause) => {
            let cause = Box::pin(error_response(*cause)).await;
            let status = cause.status();
            let cause = match axum::body::to_bytes(cause.into_body(), 1024 * 1024).await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(error) => format!("无法读取原始拒绝原因：{error}"),
            };
            rejected_owned(StatusCode::CONFLICT,"delegation-preview-stale",format!("已确认的 Bot、Map、manager、zone 或材料已失效；本次未发起，请重新预览并确认。原始错误 {status}：{cause}"))
        }
    }
}
pub(crate) async fn preview(State(state): State<AppState>, request: Request) -> Response {
    let input = match chat_command_body(request,"预览 Bot 委派").await { Ok(input) => input, Err(error) => return error };
    match chief_delegation::preview(&state, input).await { Ok(value) => Json(value).into_response(), Err(error) => error_response(error).await }
}
pub(crate) async fn commit(State(state): State<AppState>, request: Request) -> Response {
    let input = match chat_command_body(request,"发起 Bot 委派").await { Ok(input) => input, Err(error) => return error };
    match chief_delegation::commit(state, input).await {
        Ok(Outcome::Created(view)) => (StatusCode::ACCEPTED, Json(view)).into_response(),
        Ok(Outcome::Run(outcome)) => super::runs_controller::respond(Ok(outcome)),
        Err(error) => error_response(error).await,
    }
}
pub(crate) async fn list(State(state): State<AppState>, Query(query): Query<chief_delegation::SenderQuery>) -> Response {
    match chief_delegation::list(&state, query) { Ok(value) => Json(value).into_response(), Err(error) => error_response(error).await }
}
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match chief_delegation::get(&state, &id) { Ok(record) => Json(record).into_response(), Err(error) => error_response(error).await }
}
