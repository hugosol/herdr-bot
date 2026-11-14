use axum::{Json, extract::{Path, Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, suites::{self, Suite, SuiteError, SuiteErrorKind}};

pub(crate) fn error_response(error: SuiteError) -> Response {
    let status = match error.kind {
        SuiteErrorKind::Invalid => StatusCode::BAD_REQUEST,
        SuiteErrorKind::Storage => StatusCode::INTERNAL_SERVER_ERROR,
        SuiteErrorKind::NotFound => StatusCode::NOT_FOUND,
        SuiteErrorKind::Conflict => StatusCode::CONFLICT,
    };
    crate::controllers::rejected_owned(status, error.code, error.message)
}

pub(crate) async fn list(State(state): State<AppState>) -> Response {
    match suites::listing(&state) {
        Ok(list) => Json(list).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.suites.get(&id) {
        Ok(suite) => Json(serde_json::json!({ "suite": suite })).into_response(),
        Err(error) => error_response(error),
    }
}

async fn input(request: Request) -> Result<Suite, SuiteError> {
    if request.uri().query().is_some() { return Err(SuiteError::invalid("Suite 命令不接受查询参数。")); }
    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024).await.map_err(|error| SuiteError::invalid(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| SuiteError::invalid(error.to_string()))
}

pub(crate) async fn create(State(state): State<AppState>, request: Request) -> Response {
    let result = match input(request).await {
        Ok(suite) => suites::save(&state, None, suite),
        Err(error) => Err(error),
    };
    match result {
        Ok(list) => Json(list).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn update(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Response {
    let result = match input(request).await {
        Ok(suite) => suites::save(&state, Some(&id), suite),
        Err(error) => Err(error),
    };
    match result {
        Ok(list) => Json(list).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn delete(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Response {
    if request.uri().query().is_some() { return error_response(SuiteError::invalid("删除 Suite 不接受查询参数。")); }
    let bytes = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(error) => return error_response(SuiteError::invalid(error.to_string())),
    };
    if !bytes.is_empty() { return error_response(SuiteError::invalid("删除 Suite 不接受请求体。")); }
    match suites::remove(&state, &id) {
        Ok(list) => Json(list).into_response(),
        Err(error) => error_response(error),
    }
}
