//! HTTP adapters for shared definitions; business operations remain in scheduler.
use axum::{Json, response::{IntoResponse, Response}};
use crate::scheduler::{self, bots::{BotIdentity, BotDelete, BotSave}};

use super::{extractors::catalog::{BotList, NewDraft}, responses::HttpError};

fn result<T: serde::Serialize>(value: Result<T, scheduler::DirectoryError>) -> Response {
    match value {
        Ok(value) => Json(value).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub async fn list_bots(axum::extract::State(state): axum::extract::State<crate::AppState>, _: BotList) -> Response {
    result(scheduler::bots::list_bots(state).await)
}

pub async fn check_bot_delete(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::bots::check_bot_delete(state, identity).await)
}

pub async fn delete_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(target): Json<BotDelete>,
) -> Response {
    result(scheduler::bots::delete_bot(state, target).await)
}

pub async fn new_bot(_: NewDraft) -> Response {
    Json(scheduler::bots::new_bot()).into_response()
}

pub async fn edit_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::bots::edit_bot(state, identity).await)
}

pub async fn save_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(draft): Json<BotSave>,
) -> Response {
    result(scheduler::bots::save_bot(state, draft).await)
}

pub(crate) async fn set_bot_group(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(input): Json<crate::tasks::GroupInput>,
) -> Response {
    result(scheduler::bots::set_bot_group(state, id, input).await)
}

