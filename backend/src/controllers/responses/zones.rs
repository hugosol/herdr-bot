//! HTTP response mapping for zone registration and browsing.

use axum::{http::StatusCode, response::{IntoResponse, Response}};
use crate::zones::{self, browse, ZoneError};
use crate::controllers::rejected_owned;
use super::HttpError;

impl IntoResponse for HttpError<ZoneError> {
    fn into_response(self) -> Response {
        let error = self.0;
    let status = match &error {
        ZoneError::AlreadyRegistered => StatusCode::CONFLICT,
        ZoneError::NotRegistered => StatusCode::NOT_FOUND,
        ZoneError::Unreadable(_) | ZoneError::Corrupt(_) | ZoneError::WriteFailed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ZoneError::IdRequired
        | ZoneError::Required
        | ZoneError::NotAbsolute
        | ZoneError::NotFound
        | ZoneError::NotADirectory
        | ZoneError::Inaccessible(_) => StatusCode::BAD_REQUEST,
    };
    rejected_owned(status, error.code(), error.message())

    }
}

impl IntoResponse for HttpError<zones::LookupError> {
    fn into_response(self) -> Response {
        let error = self.0;
    match error {
        zones::LookupError::Registry(error) => HttpError(error).into_response(),
        zones::LookupError::Browse(error) => HttpError(error).into_response(),
    }

    }
}

impl IntoResponse for HttpError<zones::create::CreateZoneError> {
    fn into_response(self) -> Response {
        let error = self.0;
    use zones::create::CreateZoneError;
    let status = match &error {
        CreateZoneError::AlreadyExists { .. } => StatusCode::CONFLICT,
        CreateZoneError::ParentNotAbsolute(_)
        | CreateZoneError::ParentNotFound(_)
        | CreateZoneError::ParentNotDirectory(_)
        | CreateZoneError::ParentInaccessible { .. }
        | CreateZoneError::NameRequired
        | CreateZoneError::InvalidName(_) => StatusCode::BAD_REQUEST,
        CreateZoneError::CreateFailed { .. }
        | CreateZoneError::GitInitFailed { .. }
        | CreateZoneError::FinalizeFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())

    }
}

impl IntoResponse for HttpError<browse::ReadError> {
    fn into_response(self) -> Response {
        let error = self.0;
    match error {
        browse::ReadError::Zone(error) => HttpError(error).into_response(),
        browse::ReadError::Browse(error) => HttpError(error).into_response(),
        browse::ReadError::Interrupted(message) => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR, "read-failed", message.to_string(),
        ),
    }

    }
}

impl IntoResponse for HttpError<browse::BrowseError> {
    fn into_response(self) -> Response {
        let error = self.0;
    use browse::BrowseError;
    let status = match &error {
        BrowseError::ZoneNotRegistered | BrowseError::PathNotFound => StatusCode::NOT_FOUND,
        BrowseError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        BrowseError::PathOutside | BrowseError::Link { .. } => StatusCode::FORBIDDEN,
        BrowseError::NotADirectory | BrowseError::NotAFile | BrowseError::InvalidPath(_) => StatusCode::BAD_REQUEST,
        BrowseError::ReadFailed(_) | BrowseError::GitIgnoreFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())

    }
}

impl IntoResponse for HttpError<zones::CreationError> {
    fn into_response(self) -> Response {
        match self.0 {
        zones::CreationError::Registry(error) => HttpError(error).into_response(),
        zones::CreationError::Create(error) => HttpError(error).into_response(),
        zones::CreationError::NotConfigured => rejected_owned(StatusCode::BAD_REQUEST, "create-parent-not-configured", "未配置 [zones] create_parent_directory；本次没有创建任何目录。".to_string()),
        zones::CreationError::TaskFailed => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "create-task-failed", "新建 zone 的任务异常结束，无法确认目录或 Git 状态。".to_string()),
        zones::CreationError::Registration { path, error } => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "zone-registration-failed", format!("目录与 Git 初始化已完成，但 zone 登记失败，目录已保留：{path}；{}", error.message())),
        }
    }
}
