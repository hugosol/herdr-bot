use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use crate::chief_reports::Error;
use crate::controllers::rejected_owned;
use super::HttpError;

impl IntoResponse for HttpError<Error> {
    fn into_response(self) -> Response {
        match self.0 {
            Error::Invalid(code, message) => rejected_owned(StatusCode::BAD_REQUEST, code, message),
            Error::Forbidden(code, message) => rejected_owned(StatusCode::FORBIDDEN, code, message),
            Error::NotFound(code, message) => rejected_owned(StatusCode::NOT_FOUND, code, message),
            Error::Internal(code, message) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, code, message),
            Error::Storage(error) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"chief-report-storage-failed",format!("无法读取报告或其委派关联：{error}")),
            Error::Chief(error) => HttpError(error).into_response(),
            Error::Save(message) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"code":"report-save-failed","message":message,"saved":false,"notification":{"status":"not-attempted"}}))).into_response(),
        }
    }
}
