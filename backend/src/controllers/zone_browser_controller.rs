//! HTTP query parsing and response mapping for read-only zone browsing.

use axum::extract::{Query, State, rejection::QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::AppState;
use crate::zones::browse;
use super::rejected_owned;
use super::zones_controller::lookup_error_response;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowseQuery {
    zone: String,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ZoneTreeQuery {
    zone: String,
}

pub(crate) async fn read_zone_tree(
    State(state): State<AppState>,
    query: Result<Query<ZoneTreeQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => return rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-query",
            "读取 zone 目录树需要唯一的 `zone` 参数，且不接受其他参数。".to_string(),
        ),
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    match browse::tree(&state, &query.zone).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => read_error_response(error),
    }
}

pub(crate) async fn list_zone_entries(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(response) => return response,
    };
    match browse::directory(&state, &query.zone, query.path).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => read_error_response(error),
    }
}

pub(crate) async fn read_zone_file(
    State(state): State<AppState>,
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Response {
    let query = match browse_query(query) {
        Ok(query) => query,
        Err(response) => return response,
    };
    match browse::file(&state, &query.zone, query.path).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => read_error_response(error),
    }
}

fn browse_query(
    query: Result<Query<BrowseQuery>, QueryRejection>,
) -> Result<BrowseQuery, Response> {
    let Query(query) = query.map_err(|_| rejected_owned(
        StatusCode::BAD_REQUEST,
        "invalid-query",
        "读取 zone 需要 `zone` 参数与可选的 `path` 参数，且不接受其他参数。".to_string(),
    ))?;
    if query.zone.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        ));
    }
    Ok(query)
}

pub(crate) fn read_error_response(error: browse::ReadError) -> Response {
    match error {
        browse::ReadError::Zone(error) => lookup_error_response(error),
        browse::ReadError::Browse(error) => browse_error_response(error),
        browse::ReadError::Interrupted(message) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR, "read-failed", message.to_string(),
        ),
    }
}

pub(crate) fn browse_error_response(error: browse::BrowseError) -> Response {
    use browse::BrowseError;
    let status = match &error {
        BrowseError::ZoneNotRegistered | BrowseError::PathNotFound => StatusCode::NOT_FOUND,
        BrowseError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        BrowseError::PathOutside | BrowseError::Link { .. } => StatusCode::FORBIDDEN,
        BrowseError::NotADirectory | BrowseError::NotAFile | BrowseError::InvalidPath(_) => StatusCode::BAD_REQUEST,
        BrowseError::ReadFailed(_) | BrowseError::GitIgnoreFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())
}
