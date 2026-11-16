use std::net::SocketAddr;
use axum::{extract::{ConnectInfo, FromRequestParts, Path}, http::request::Parts, response::{IntoResponse, Response}};
use crate::{runs, controllers::responses::HttpError};
use super::CommandAction;

pub(crate) struct LocalRunSource;

impl<S: Send + Sync> FromRequestParts<S> for LocalRunSource {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ConnectInfo(peer) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await.map_err(IntoResponse::into_response)?;
        runs::authorize_source(peer.ip()).map_err(|error| HttpError(error).into_response())?;
        Ok(Self)
    }
}

/// Rename rejects an invalid identity before reading its command body.
pub(crate) struct RunId(pub(crate) String);

impl<S: Send + Sync> FromRequestParts<S> for RunId {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(id) = Path::<String>::from_request_parts(parts, state)
            .await.map_err(IntoResponse::into_response)?;
        runs::validate_id(&id).map_err(|error| HttpError(error).into_response())?;
        Ok(Self(id))
    }
}

pub(crate) struct PrepareInputs;
impl CommandAction for PrepareInputs {
    const ACTION: &'static str = "准备 Run 输入";
}

pub(crate) struct Collect;
impl CommandAction for Collect {
    const ACTION: &'static str = "收取报告";
}

pub(crate) struct NodeContext;
impl CommandAction for NodeContext {
    const ACTION: &'static str = "节点上下文";
}

pub(crate) struct NodeReport;
impl CommandAction for NodeReport {
    const ACTION: &'static str = "节点报告";
}

pub(crate) struct Retry;
impl CommandAction for Retry {
    const ACTION: &'static str = "重试独立新执行";
}

pub(crate) struct ReconcileDispatch;
impl CommandAction for ReconcileDispatch {
    const ACTION: &'static str = "核实派发事实";
}

pub(crate) struct Recover;
impl CommandAction for Recover {
    const ACTION: &'static str = "恢复原执行跟踪";
}

pub(crate) struct Fail;
impl CommandAction for Fail {
    const ACTION: &'static str = "记录执行失败";
}

pub(crate) struct Complete;
impl CommandAction for Complete {
    const ACTION: &'static str = "交接完成";
}

pub(crate) struct Finish;
impl CommandAction for Finish {
    const ACTION: &'static str = "结束 Run";
}

pub(crate) struct PauseOrResume;
impl CommandAction for PauseOrResume {
    const ACTION: &'static str = "暂停或恢复派发资格";
}

pub(crate) struct Status;
impl CommandAction for Status {
    const ACTION: &'static str = "运行状态";
}

pub(crate) struct Dispatch;
impl CommandAction for Dispatch {
    const ACTION: &'static str = "派发节点";
}

pub(crate) struct Rename;
impl CommandAction for Rename {
    const ACTION: &'static str = "重命名 Run";
}

pub(crate) struct Start;
impl CommandAction for Start {
    const ACTION: &'static str = "发起 Run";
}
