//! Closed request contracts shared by zone controllers.

use axum::{body::to_bytes, extract::{FromRequest, FromRequestParts, Query, Request}, http::{request::Parts, StatusCode}, response::Response};
use serde::Deserialize;
use super::CommandAction;
use crate::controllers::{rejected_owned, MAX_ZONE_REQUEST_BYTES};

pub(crate) struct UnregisterZone;
impl CommandAction for UnregisterZone {
    const ACTION: &'static str = "取消 zone 登记";
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ZonePathRequest {
    pub id: String,
    pub path: String,
}

impl<S: Send + Sync> FromRequest<S> for ZonePathRequest {
    type Rejection = Response;
    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "zone 命令不接受查询参数：路径只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "zone 请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let body: ZonePathRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析 zone 命令：请发送 {\"id\":\"稳定 zone ID\",\"path\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    Ok(body)

    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneTargetRequest {
    zone: String,
}

pub(crate) struct ZoneTarget(pub String);

impl<S: Send + Sync> FromRequest<S> for ZoneTarget {
    type Rejection = Response;
    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "该命令不接受查询参数：zone 只来自请求体。".to_string(),
        ));
    }
    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let request: ZoneTargetRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析请求：请发送 {\"zone\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    if request.zone.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请指定要操作的已登记 zone。".to_string(),
        ));
    }
    Ok(Self(request.zone))

    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowseQuery {
    pub zone: String,
    #[serde(default)]
    pub path: String,
}

impl<S: Send + Sync> FromRequestParts<S> for BrowseQuery {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
    let query = Query::<Self>::from_request_parts(parts, state).await;
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
}
