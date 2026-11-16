use axum::{Json, extract::{Path, State}, response::{IntoResponse, Response}};
use crate::{AppState, suites};
use super::{extractors::catalog::{SuiteDelete, SuiteInput}, responses::HttpError};

pub(crate) async fn list(State(state): State<AppState>) -> Response {
    match suites::listing(&state) {
        Ok(list) => Json(list).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.suites.get(&id) {
        Ok(suite) => Json(serde_json::json!({ "suite": suite })).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn create(State(state): State<AppState>, SuiteInput(suite): SuiteInput) -> Response {
    let result = suites::save(&state, None, suite);
    match result {
        Ok(list) => Json(list).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn update(State(state): State<AppState>, Path(id): Path<String>, SuiteInput(suite): SuiteInput) -> Response {
    let result = suites::save(&state, Some(&id), suite);
    match result {
        Ok(list) => Json(list).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn delete(State(state): State<AppState>, Path(id): Path<String>, _: SuiteDelete) -> Response {
    match suites::remove(&state, &id) {
        Ok(list) => Json(list).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}
