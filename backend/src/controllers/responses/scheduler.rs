use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use crate::scheduler::DirectoryError;
use crate::runs::preflight::PreflightFailure;
use super::HttpError;

impl IntoResponse for HttpError<DirectoryError> {
    fn into_response(self) -> Response {
        let error = self.0;
        let status = match error.code {
            "map-input-invalid" | "map-title-invalid" | "group-invalid" => StatusCode::BAD_REQUEST,
            "map-conflict" | "bot-conflict" | "script-policy-invalid" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({"code": error.code, "message": error.message}))).into_response()
    }
}

impl IntoResponse for HttpError<PreflightFailure> {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self.0.into_payload())).into_response()
    }
}
