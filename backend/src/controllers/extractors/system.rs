use std::net::SocketAddr;

use axum::{
    body::to_bytes,
    extract::{ConnectInfo, FromRequest, FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{controllers::{MAX_REQUEST_BODY_BYTES, rejected, responses::HttpError}, sessions::{self, SessionEvent}};

/// Fixed operations accept neither parameters nor a body, including unreadable bodies.
pub(crate) struct ClosedOperation;

impl<S: Send + Sync> FromRequest<S> for ClosedOperation {
    type Rejection = Response;

    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected(
                StatusCode::BAD_REQUEST,
                "unexpected-parameters",
                "操作请求不接受查询参数：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
            ));
        }
        match to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await {
            Ok(bytes) if bytes.is_empty() => Ok(Self),
            _ => Err(rejected(
                StatusCode::BAD_REQUEST,
                "unexpected-parameters",
                "操作请求不接受请求体：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
            )),
        }
    }
}

/// Source authorization runs before the event's query and body are inspected.
pub(crate) struct LocalSessionSource;

impl<S: Send + Sync> FromRequestParts<S> for LocalSessionSource {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ConnectInfo(peer) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        sessions::authorize_source(peer.ip()).map_err(|error| HttpError(error).into_response())?;
        Ok(Self)
    }
}

pub(crate) struct SessionEventBody(pub SessionEvent);

impl<S: Send + Sync> FromRequest<S> for SessionEventBody {
    type Rejection = Response;

    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected(StatusCode::BAD_REQUEST, "unexpected-parameters", "会话事件请求不接受查询参数：事件内容只来自请求体。"));
        }
        let bytes = to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await
            .map_err(|_| rejected(StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large", "会话事件请求体超过 8KB 上限。"))?;
        serde_json::from_slice(&bytes).map(Self)
            .map_err(|_| rejected(StatusCode::BAD_REQUEST, "invalid-event", "会话事件无法解析：请发送 {sessionId, kind, data} 形式的合法事件。"))
    }
}

/// A monitoring command states the target selection rather than toggling it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitoringCommand {
    pub session_id: String,
    pub selected: bool,
}

impl<S: Send + Sync> FromRequest<S> for MonitoringCommand {
    type Rejection = Response;

    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected(
                StatusCode::BAD_REQUEST,
                "unexpected-parameters",
                "监听命令不接受查询参数：目标会话与目标选中值只来自请求体。",
            ));
        }
        let bytes = to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await
            .map_err(|_| rejected(StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large", "监听命令请求体超过 8KB 上限。"))?;
        serde_json::from_slice(&bytes)
            .map_err(|_| rejected(StatusCode::BAD_REQUEST, "invalid-monitoring", "监听命令无法解析：请发送 {sessionId, selected} 形式的合法命令。"))
    }
}
