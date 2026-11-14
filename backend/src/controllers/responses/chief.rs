use axum::{http::StatusCode, response::{IntoResponse, Response}};
use crate::chief::Error;
use crate::controllers::rejected_owned;
use super::HttpError;

impl IntoResponse for HttpError<Error> {
    fn into_response(self) -> Response {
        match self.0 {
            Error::Occupied => rejected_owned(StatusCode::CONFLICT,"chief-occupied","当前 chief 或启动占位尚未移除；本次未发起。".into()),
            Error::NotCurrent => rejected_owned(StatusCode::CONFLICT,"chief-not-current","该会话不是当前有效 chief；旧进程仍存活不授予职责，请由用户明确恢复。".into()),
            Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"chief-storage-failed",format!("无法确认 chief 权威状态：{error}")),
            Error::Settings(error) => HttpError(crate::chats::Error::Task(error, "")).into_response(),
        }
    }
}
