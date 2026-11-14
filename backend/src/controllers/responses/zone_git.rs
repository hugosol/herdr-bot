//! HTTP response mapping for zone Git operations.

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
use crate::zones::git::{self, commit, sync, operations};
use crate::controllers::rejected_owned;
use super::HttpError;

impl IntoResponse for HttpError<git::GitError> {
    fn into_response(self) -> Response {
        let error = self.0;
    let status = match &error {
        git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
        git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    rejected_owned(status, error.code(), error.message())

    }
}

impl IntoResponse for HttpError<git::DiffError> {
    fn into_response(self) -> Response {
        let error = self.0;
    match error {
        git::DiffError::Browse(error) => HttpError(error).into_response(),
        git::DiffError::Git(error) => HttpError(error).into_response(),
    }

    }
}

impl IntoResponse for HttpError<operations::Error> {
    fn into_response(self) -> Response {
        let error = self.0;
    use operations::{Error, Operation};
    match error {
        Error::Registry(error) => HttpError(error).into_response(),
        Error::Lookup(error) => HttpError(error).into_response(),
        Error::Git(error) => HttpError(error).into_response(),
        Error::Diff(error) => HttpError(error).into_response(),
        Error::EmptyMessage => rejected_owned(StatusCode::BAD_REQUEST, "empty-message", "提交信息不能为空；本次没有执行任何 Git 写入。".to_string()),
        Error::ReviewRequired => rejected_owned(StatusCode::BAD_REQUEST, "review-required", "缺少审查版本标识；请先读取并确认当前审查版本。".to_string()),
        Error::TaskFailed(operation) => {
            let (code, message) = match operation {
                Operation::Overview => ("git-command-failed", "读取 zone 概览的任务异常结束，无法确认待提交数量。"),
                Operation::Status => ("git-command-failed", "读取 Git 状态的任务异常结束，无法确认当前分支与待提交变更。"),
                Operation::Diff => ("git-command-failed", "读取审查 diff 的任务异常结束，无法确认旧新内容。"),
                Operation::Commit => ("commit-result-unavailable", "提交任务异常结束，无法确认执行结果；请按实际仓库状态核对。"),
                Operation::Check => ("git-command-failed", "检查远端的任务异常结束，无法确认远端状态。"),
                Operation::Sync => ("sync-result-unavailable", "同步任务异常结束，无法确认执行结果；请按实际仓库状态核对。"),
            };
            rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, code, message.to_string())
        }
        Error::CommitResultUnavailable => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commit-result-unavailable",
            "本次提交没有送回结果：无法确认执行结果，请按实际仓库状态核对。".to_string(),
        ),
        Error::SyncResultUnavailable => rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "sync-result-unavailable",
            "本次同步没有送回结果：无法确认执行结果，请按实际仓库状态核对。".to_string(),
        ),
    }

    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ZoneCommitResponse {
    zone: String,
    outcome: &'static str,
    commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    message: String,
}

pub(crate) struct CommitResponse(pub String, pub commit::CommitOutcome);
impl IntoResponse for CommitResponse {
    fn into_response(self) -> Response {
        let Self(zone, outcome) = self;
    match outcome {
        commit::CommitOutcome::Committed {
            commit,
            branch,
            message,
        } => (
            StatusCode::OK,
            Json(ZoneCommitResponse {
                zone,
                outcome: "committed",
                commit,
                branch,
                message,
            }),
        )
            .into_response(),
        commit::CommitOutcome::NotRepository => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可执行的 Commit。".to_string(),
        ),
        commit::CommitOutcome::Busy => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次请求没有被接受，也不会在它结束后补做。".to_string(),
        ),
        commit::CommitOutcome::Blocked(reason) => {
            rejected_owned(StatusCode::CONFLICT, "commit-blocked", reason)
        }
        commit::CommitOutcome::ReviewInvalid(message) => {
            rejected_owned(StatusCode::CONFLICT, "review-invalid", message)
        }
        commit::CommitOutcome::NothingToCommit => rejected_owned(
            StatusCode::CONFLICT,
            "nothing-to-commit",
            "当前没有待提交变更：本次没有执行 Git 写入。".to_string(),
        ),
        commit::CommitOutcome::Failed(error) => {
            let status = match &error {
                git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
                git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
                git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            rejected_owned(status, error.code(), error.message())
        }
    }

    }
}

pub(crate) struct SyncResponse(pub String, pub sync::SyncOutcome);
impl IntoResponse for SyncResponse {
    fn into_response(self) -> Response {
        let Self(zone, outcome) = self;
    match outcome {
        sync::SyncOutcome::Synced(mut accepted) => {
            accepted.zone = zone;
            (StatusCode::OK, Json(accepted)).into_response()
        }
        sync::SyncOutcome::NotRepository => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可执行的 Sync。".to_string(),
        ),
        sync::SyncOutcome::Busy => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次请求没有被接受，也不会在它结束后补做。".to_string(),
        ),
        sync::SyncOutcome::Blocked(reason) => {
            rejected_owned(StatusCode::CONFLICT, "sync-blocked", reason)
        }
        sync::SyncOutcome::Dirty => rejected_owned(
            StatusCode::CONFLICT,
            "sync-dirty-worktree",
            "工作树有未提交变更：Sync 只对干净工作树可用；本功能不会 stash 或隐式提交。"
                .to_string(),
        ),
        sync::SyncOutcome::NoUpstream => rejected_owned(
            StatusCode::CONFLICT,
            "no-upstream",
            "当前分支没有配置 upstream：Sync 不可用；本功能不会配置 upstream 或发布分支。"
                .to_string(),
        ),
        sync::SyncOutcome::Failed {
            step,
            error,
            completed,
        } => {
            let mut message = format!("同步在 {step} 步骤失败：{}", error.message());
            if !completed.is_empty() {
                message.push_str(&format!(
                    " 已完成的步骤（{}）不会被自动撤销。",
                    completed.join(" → ")
                ));
            }
            let status = match &error {
                git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
                git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
                git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            rejected_owned(status, error.code(), message)
        }
    }

    }
}

pub(crate) struct CheckResponse(pub sync::CheckOutcome);
impl IntoResponse for CheckResponse {
    fn into_response(self) -> Response {
        let outcome = self.0;
match outcome {
        sync::CheckOutcome::Checked(status) => (StatusCode::OK, Json(status)).into_response(),
        sync::CheckOutcome::NotRepository => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可检查的远端。".to_string(),
        ),
        sync::CheckOutcome::Busy => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次检查没有被接受，也不会在它结束后补做。".to_string(),
        ),
        sync::CheckOutcome::Blocked(reason) => {
            rejected_owned(StatusCode::CONFLICT, "sync-blocked", reason)
        }
        sync::CheckOutcome::NoUpstream => rejected_owned(
            StatusCode::CONFLICT,
            "no-upstream",
            "当前分支没有配置 upstream：无法确定要检查的远端；本功能不会代替你配置 upstream。"
                .to_string(),
        ),
        sync::CheckOutcome::Failed(error) => HttpError(error).into_response(),
    }
    }
}
