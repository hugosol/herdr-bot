//! HTTP request contracts. Domain validation remains in the domain modules.
pub(crate) mod catalog;
pub(crate) mod chats;
pub(crate) mod chief;
pub(crate) mod runs;
pub(crate) mod system;
pub(crate) mod zones;

use std::marker::PhantomData;
use axum::{body::to_bytes, extract::{FromRequest, Request}, http::StatusCode, response::Response};
use serde::de::DeserializeOwned;
use super::{MAX_TASK_REQUEST_BYTES, rejected_owned};

/// Only the action label varies across the existing 1 MiB command contract.
pub(crate) trait CommandAction {
    const ACTION: &'static str;
}

/// JSON commands intentionally do not require a Content-Type header. Unlike
/// axum::Json, all deserialization failures retain the existing 400 response.
pub(crate) struct CommandJson<T, A> {
    pub(crate) value: T,
    action: PhantomData<A>,
}

impl<S, T, A> FromRequest<S> for CommandJson<T, A>
where
    S: Send + Sync,
    T: DeserializeOwned,
    A: CommandAction,
{
    type Rejection = Response;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected_owned(
                StatusCode::BAD_REQUEST,
                "unexpected-parameters",
                format!("{}不接受查询参数：字段只来自请求体。", A::ACTION),
            ));
        }
        let bytes = to_bytes(request.into_body(), MAX_TASK_REQUEST_BYTES)
            .await
            .map_err(|_| rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                format!("{}的请求体超过 1 MiB 上限。", A::ACTION),
            ))?;
        let value = serde_json::from_slice(&bytes).map_err(|error| rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            format!("无法解析{}请求：{error}", A::ACTION),
        ))?;
        Ok(Self { value, action: PhantomData })
    }
}
