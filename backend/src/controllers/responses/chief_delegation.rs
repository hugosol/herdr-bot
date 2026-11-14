use axum::{http::StatusCode, response::{IntoResponse, Response}};
use crate::chief_delegation::Error;
use crate::controllers::rejected_owned;
use super::HttpError;

// Unlike ordinary IntoResponse mappings, a stale preview embeds the complete
// serialized cause response. Keep its asynchronous body read and limit intact.
pub(crate) async fn error_response(error: Error) -> Response {
    match error {
        Error::Conflict(code, message) => rejected_owned(StatusCode::CONFLICT, code, message),
        Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"delegation-storage-failed",format!("委派记录无法可靠保存或读取：{error}")),
        Error::Chief(error) => HttpError(error).into_response(),
        Error::Chat(error) => HttpError(error).into_response(),
        Error::Task(error) => HttpError(error).into_response(),
        Error::Directory(error) => HttpError(error).into_response(),
        Error::Definition(crate::scheduler::DelegationDefinitionError::Directory(error)) => HttpError(error).into_response(),
        Error::Definition(crate::scheduler::DelegationDefinitionError::Prompt(message)) => rejected_owned(StatusCode::CONFLICT,"prompt-dictionary-invalid",message),
        Error::Preflight(error) => HttpError(error).into_response(),
        Error::Run(error) => HttpError(error).into_response(),
        Error::NotFound => rejected_owned(StatusCode::NOT_FOUND,"delegation-not-found","委派记录不存在。".into()),
        Error::Stale(cause) => {
            let cause = Box::pin(error_response(*cause)).await;
            let status = cause.status();
            let cause = match axum::body::to_bytes(cause.into_body(), 1024 * 1024).await {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(error) => format!("无法读取原始拒绝原因：{error}"),
            };
            rejected_owned(StatusCode::CONFLICT,"delegation-preview-stale",format!("已确认的 Bot、Map、manager、zone 或材料已失效；本次未发起，请重新预览并确认。原始错误 {status}：{cause}"))
        }
    }
}
