use axum::{http::StatusCode, response::{IntoResponse, Response}};
use crate::suites::{SuiteError, SuiteErrorKind};
use super::HttpError;

impl IntoResponse for HttpError<SuiteError> {
    fn into_response(self) -> Response {
        let error = self.0;
        let status = match error.kind {
            SuiteErrorKind::Invalid => StatusCode::BAD_REQUEST,
            SuiteErrorKind::Storage => StatusCode::INTERNAL_SERVER_ERROR,
            SuiteErrorKind::NotFound => StatusCode::NOT_FOUND,
            SuiteErrorKind::Conflict => StatusCode::CONFLICT,
        };
        crate::controllers::rejected_owned(status, error.code, error.message)
    }
}
