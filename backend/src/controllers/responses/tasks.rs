use axum::{http::StatusCode, response::{IntoResponse, Response}};
use crate::tasks::{CatalogError, TaskError};
use super::HttpError;

pub(crate) struct TaskErrorContext<'a>(pub TaskError, pub &'a str);

impl IntoResponse for HttpError<TaskError> {
    fn into_response(self) -> Response {
        TaskErrorContext(self.0, "").into_response()
    }
}

impl IntoResponse for TaskErrorContext<'_> {
    fn into_response(self) -> Response {
        let Self(error, context) = self;
        let status = match &error {
            TaskError::NotFound => StatusCode::NOT_FOUND,
            TaskError::Unreadable(_)
            | TaskError::Metadata(_)
            | TaskError::Corrupt(_)
            | TaskError::WriteFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            TaskError::InvalidGroup
            | TaskError::NameRequired
            | TaskError::ChatNameRequired
            | TaskError::ChatNameNotSingleLine
            | TaskError::AgentRequired
            | TaskError::UnknownAgent(_)
            | TaskError::InvalidTaskType
            | TaskError::UnknownModel(_)
            | TaskError::UnknownThinking(_)
            | TaskError::ThinkingRequiresDefaultModel
            | TaskError::ThinkingUnsupported { .. }
            | TaskError::UnknownSkill(_)
            | TaskError::InvalidSuite(_) => StatusCode::BAD_REQUEST,
        };
        crate::controllers::rejected_owned(status, error.code(), format!("{}{context}", error.message()))
    }
}

impl IntoResponse for HttpError<CatalogError> {
    fn into_response(self) -> Response {
        match self.0 {
            CatalogError::Task(error) => HttpError(error).into_response(),
            CatalogError::Suite(error) => HttpError(error).into_response(),
        }
    }
}
