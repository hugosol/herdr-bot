use std::net::SocketAddr;
use axum::{extract::{ConnectInfo, FromRequestParts}, http::request::Parts, response::{IntoResponse, Response}};
use crate::{chief_reports, controllers::responses::HttpError};
use super::CommandAction;

pub(crate) struct LocalReportSource;

impl<S: Send + Sync> FromRequestParts<S> for LocalReportSource {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ConnectInfo(peer) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await.map_err(IntoResponse::into_response)?;
        chief_reports::authorize_source(peer.ip()).map_err(|error| HttpError(error).into_response())?;
        Ok(Self)
    }
}

pub(crate) struct PreviewDelegation;
impl CommandAction for PreviewDelegation {
    const ACTION: &'static str = "预览 Bot 委派";
}

pub(crate) struct CommitDelegation;
impl CommandAction for CommitDelegation {
    const ACTION: &'static str = "发起 Bot 委派";
}

pub(crate) struct SubmitReport;
impl CommandAction for SubmitReport {
    const ACTION: &'static str = "提交 chief 报告";
}

pub(crate) struct ReceiveReport;
impl CommandAction for ReceiveReport {
    const ACTION: &'static str = "可靠接收 chief 报告";
}

pub(crate) struct ViewReport;
impl CommandAction for ViewReport {
    const ACTION: &'static str = "明确查看 chief 原报告";
}

pub(crate) struct ResendReport;
impl CommandAction for ResendReport {
    const ACTION: &'static str = "人工重发 chief 报告";
}
