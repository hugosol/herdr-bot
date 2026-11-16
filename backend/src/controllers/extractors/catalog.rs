use axum::{body::to_bytes, extract::{FromRequest, Request}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{scheduler, suites::{Suite, SuiteError}, tasks};
use crate::controllers::{rejected, rejected_owned, responses::HttpError};
use super::CommandAction;

const MAX_CATALOG_REQUEST_BYTES: usize = 1024 * 1024;

pub(crate) struct TaskInput(pub tasks::TaskInput);
pub(crate) struct TaskDelete;
pub(crate) struct SuiteInput(pub Suite);
pub(crate) struct SuiteDelete;
pub(crate) struct MapRead;
pub(crate) struct MapList;
pub(crate) struct BotList;
pub(crate) struct NewDraft;
pub(crate) struct DeleteMap;
pub(crate) struct PreflightRun;

impl CommandAction for DeleteMap { const ACTION: &'static str = "删除 Map"; }
impl CommandAction for PreflightRun { const ACTION: &'static str = "Run 启动预检"; }

impl<S: Send + Sync> FromRequest<S> for TaskInput {
    type Rejection = Response;
    async fn from_request(request: Request, _: &S) -> Result<Self, Response> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected_owned(StatusCode::BAD_REQUEST, "unexpected-parameters", "Task命令不接受查询参数：字段只来自请求体。".to_string()));
        }
        let bytes = to_bytes(request.into_body(), MAX_CATALOG_REQUEST_BYTES).await.map_err(|_| {
            rejected_owned(StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large", "Task请求体超过 1 MiB 上限。".to_string())
        })?;
        serde_json::from_slice(&bytes).map(Self).map_err(|error| {
            rejected_owned(StatusCode::BAD_REQUEST, "invalid-request", format!("无法解析Task命令：{error}；请发送包含 name、chatName、agent、model、thinking、skill、prompt 的 JSON。"))
        })
    }
}

impl<S: Send + Sync> FromRequest<S> for TaskDelete {
    type Rejection = Response;
    async fn from_request(request: Request, _: &S) -> Result<Self, Response> {
        if request.uri().query().is_some_and(|query| !query.is_empty()) {
            return Err(rejected(StatusCode::BAD_REQUEST, "unexpected-parameters", "删除Task不接受查询参数：目标只由路径中的Task标识决定。"));
        }
        match to_bytes(request.into_body(), MAX_CATALOG_REQUEST_BYTES).await {
            Ok(bytes) if bytes.is_empty() => Ok(Self),
            _ => Err(rejected(StatusCode::BAD_REQUEST, "unexpected-parameters", "删除Task不接受请求体：目标只由路径中的Task标识决定。")),
        }
    }
}

impl<S: Send + Sync> FromRequest<S> for SuiteInput {
    type Rejection = HttpError<SuiteError>;
    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some() {
            return Err(HttpError(SuiteError::invalid("Suite 命令不接受查询参数。")));
        }
        let bytes = to_bytes(request.into_body(), MAX_CATALOG_REQUEST_BYTES).await
            .map_err(|error| HttpError(SuiteError::invalid(error.to_string())))?;
        serde_json::from_slice(&bytes).map(Self)
            .map_err(|error| HttpError(SuiteError::invalid(error.to_string())))
    }
}

impl<S: Send + Sync> FromRequest<S> for SuiteDelete {
    type Rejection = HttpError<SuiteError>;
    async fn from_request(request: Request, _: &S) -> Result<Self, Self::Rejection> {
        if request.uri().query().is_some() {
            return Err(HttpError(SuiteError::invalid("删除 Suite 不接受查询参数。")));
        }
        let bytes = to_bytes(request.into_body(), MAX_CATALOG_REQUEST_BYTES).await
            .map_err(|error| HttpError(SuiteError::invalid(error.to_string())))?;
        if !bytes.is_empty() {
            return Err(HttpError(SuiteError::invalid("删除 Suite 不接受请求体。")));
        }
        Ok(Self)
    }
}

// These read endpoints deliberately reject even an empty query and any body.
macro_rules! definition_read {
    ($input:ty, $message:literal) => {
        impl<S: Send + Sync> FromRequest<S> for $input {
            type Rejection = Response;
            async fn from_request(request: Request, _: &S) -> Result<Self, Response> {
                if request.uri().query().is_some() || to_bytes(request.into_body(), 0).await.is_err() {
                    return Err(rejected(StatusCode::BAD_REQUEST, "invalid-request", $message));
                }
                Ok(Self)
            }
        }
    };
}
definition_read!(MapRead, "Map 读取不接受查询参数或请求体。");
definition_read!(MapList, "Map 列表不接受路径、查询参数或请求体。");
definition_read!(BotList, "共享 Bot 读取不接受路径、查询参数或请求体。");

impl<S: Send + Sync> FromRequest<S> for NewDraft {
    type Rejection = Response;
    async fn from_request(request: Request, _: &S) -> Result<Self, Response> {
        if request.uri().query().is_some() || to_bytes(request.into_body(), 0).await.is_err() {
            return Err(HttpError(scheduler::DirectoryError {
                code: "invalid-request", message: "新增草稿不接受查询参数或请求体。".into(),
            }).into_response());
        }
        Ok(Self)
    }
}
