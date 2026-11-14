use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::json;
use crate::{AppState, chief::{self, Error}};
use super::rejected_owned;

pub(crate) fn error_response(error: Error) -> Response {
    match error {
        Error::Occupied => rejected_owned(StatusCode::CONFLICT,"chief-occupied","当前 chief 或启动占位尚未移除；本次未发起。".into()),
        Error::NotCurrent => rejected_owned(StatusCode::CONFLICT,"chief-not-current","该会话不是当前有效 chief；旧进程仍存活不授予职责，请由用户明确恢复。".into()),
        Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"chief-storage-failed",format!("无法确认 chief 权威状态：{error}")),
        Error::Settings(error) => super::chats_controller::error_response(crate::chats::Error::Task(error, "")),
    }
}
pub(crate) async fn get_current(State(state): State<AppState>) -> Response {
    match chief::get_current(&state).await { Ok(current) => Json(json!({"current":current})).into_response(), Err(error) => error_response(error) }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReleaseRequest { claim_id: String }
pub(crate) async fn release(State(state): State<AppState>, Json(input): Json<ReleaseRequest>) -> Response {
    match chief::release(&state, &input.claim_id).await { Ok(released) => Json(json!({"current":null,"released":released})).into_response(), Err(error) => error_response(error) }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct QualifyRequest { session_id: String }
pub(crate) async fn qualify(State(state): State<AppState>, Json(input): Json<QualifyRequest>) -> Response {
    match chief::qualify(&state, &input.session_id).await { Ok(current) => Json(json!({"current":current})).into_response(), Err(error) => error_response(error) }
}
