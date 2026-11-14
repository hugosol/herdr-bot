//! HTTP adapters for shared definitions; business operations remain in scheduler.
use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use crate::scheduler::{self, BotIdentity, BotDelete, BotSave};

use super::scheduler_controller::definition_error;

fn result<T: serde::Serialize>(value: Result<T, scheduler::DirectoryError>) -> Response {
    match value {
        Ok(value) => Json(value).into_response(),
        Err(error) => definition_error(error),
    }
}

pub async fn list_bots(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "共享 Bot 读取不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    result(scheduler::list_bots(state).await)
}

pub async fn check_bot_delete(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::check_bot_delete(state, identity).await)
}

pub async fn delete_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(target): Json<BotDelete>,
) -> Response {
    result(scheduler::delete_bot(state, target).await)
}

pub async fn new_bot(request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return definition_error(scheduler::DirectoryError { code: "invalid-request", message: "新增草稿不接受查询参数或请求体。".into() });
    }
    Json(scheduler::new_bot()).into_response()
}

pub async fn edit_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::edit_bot(state, identity).await)
}

pub async fn save_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(draft): Json<BotSave>,
) -> Response {
    result(scheduler::save_bot(state, draft).await)
}

pub(crate) async fn set_bot_group(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(input): Json<crate::tasks::GroupInput>,
) -> Response {
    result(scheduler::set_bot_group(state, id, input).await)
}

