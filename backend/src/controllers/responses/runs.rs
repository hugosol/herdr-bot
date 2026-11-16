use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use crate::runs::{RunError, RunOutcome};
use super::HttpError;

pub(crate) struct RunResponse(pub(crate) RunOutcome);

impl IntoResponse for RunResponse {
    fn into_response(self) -> Response {
        match self.0 {
            RunOutcome::Value(value) => Json(value).into_response(),
            RunOutcome::Rejected(value) => (StatusCode::CONFLICT, Json(value)).into_response(),
            RunOutcome::Started { failed, value } => (
                if failed { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::CREATED },
                Json(value),
            ).into_response(),
        }
    }
}

impl IntoResponse for HttpError<RunError> {
    fn into_response(self) -> Response {
        match self.0 {
            RunError::Invalid(value) => (StatusCode::BAD_REQUEST, Json(value)).into_response(),
            RunError::Forbidden(value) => (StatusCode::FORBIDDEN, Json(value)).into_response(),
            RunError::Failed(value) => (StatusCode::INTERNAL_SERVER_ERROR, Json(value)).into_response(),
            RunError::Preflight(error) => HttpError(error).into_response(),
            RunError::Task(error) => HttpError(error).into_response(),
        }
    }
}
