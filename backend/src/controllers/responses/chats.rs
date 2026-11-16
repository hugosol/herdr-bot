use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use crate::chats::Error;
use crate::controllers::rejected_owned;
use super::{HttpError, tasks::TaskErrorContext};

impl IntoResponse for HttpError<Error> {
    fn into_response(self) -> Response {
        match self.0 {
            Error::Invalid(code, message) => rejected_owned(StatusCode::BAD_REQUEST, code, message),
            Error::Conflict(code, message) => rejected_owned(StatusCode::CONFLICT, code, message),
            Error::NotFound(code, message) => rejected_owned(StatusCode::NOT_FOUND, code, message),
            Error::Internal(code, message) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, code, message),
            Error::Task(error, context) => TaskErrorContext(error, context).into_response(),
            Error::Zone(error) => HttpError(error).into_response(),
            Error::Browse(error) => HttpError(error).into_response(),
            Error::Chief(error) => HttpError(error).into_response(),
            Error::ResumeFailure(value) => (StatusCode::BAD_GATEWAY, Json(value)).into_response(),
        }
    }
}
