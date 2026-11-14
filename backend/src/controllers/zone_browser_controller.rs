//! HTTP query parsing and response mapping for read-only zone browsing.

use axum::extract::{Query, State, rejection::QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::AppState;
use crate::zones::browse;
use super::rejected_owned;
use super::{extractors::zones::BrowseQuery, responses::HttpError};



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
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn list_zone_entries(
    State(state): State<AppState>,
    query: BrowseQuery,
) -> Response {
    match browse::directory(&state, &query.zone, query.path).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn read_zone_file(
    State(state): State<AppState>,
    query: BrowseQuery,
) -> Response {
    match browse::file(&state, &query.zone, query.path).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}






