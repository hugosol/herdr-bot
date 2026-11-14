use axum::{Json, extract::State, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::json;
use crate::{AppState, chief};
use super::responses::HttpError;
pub(crate) async fn get_current(State(state): State<AppState>) -> Response {
    match chief::get_current(&state).await { Ok(current) => Json(json!({"current":current})).into_response(), Err(error) => HttpError(error).into_response() }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReleaseRequest { claim_id: String }
pub(crate) async fn release(State(state): State<AppState>, Json(input): Json<ReleaseRequest>) -> Response {
    match chief::release(&state, &input.claim_id).await { Ok(released) => Json(json!({"current":null,"released":released})).into_response(), Err(error) => HttpError(error).into_response() }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct QualifyRequest { session_id: String }
pub(crate) async fn qualify(State(state): State<AppState>, Json(input): Json<QualifyRequest>) -> Response {
    match chief::qualify(&state, &input.session_id).await { Ok(current) => Json(json!({"current":current})).into_response(), Err(error) => HttpError(error).into_response() }
}
