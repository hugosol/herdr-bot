//! HTTP parsing and response mappings for zone Git commands.
use axum::{Json, body::to_bytes, extract::{Query, Request, State}, extract::rejection::QueryRejection, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use crate::AppState;
use super::MAX_ZONE_REQUEST_BYTES;
use crate::zones::git::{self, commit, sync, operations};
use super::{rejected_owned, zones_controller::{zone_error_response, lookup_error_response}, zone_browser_controller::browse_error_response};

/// 一次 Git 状态请求的查询参数：只需要登记路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GitStatusQuery {
    zone: String,
    #[serde(default)]
    refresh: bool,
}

/// zone 列表上的角标概览：每个登记项的待提交项数或明确错误。
#[derive(Debug, Serialize)]
struct ZoneOverviewResponse {
    zones: Vec<git::ZoneSummary>,
}

pub(crate) async fn zones_overview(State(state): State<AppState>) -> Response {
    match operations::overview(&state).await {
        Ok(zones) => (StatusCode::OK, Json(ZoneOverviewResponse { zones })).into_response(),
        Err(error) => operation_error_response(error),
    }
}

/// “读取 Git 状态”：当前分支与待提交变更；普通目录返回 `repository: false`。
pub(crate) async fn zone_git_status(
    State(state): State<AppState>,
    query: Result<Query<GitStatusQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "读取 Git 状态需要 `zone` 参数，可选布尔参数 `refresh`，且不接受其他参数。"
                    .to_string(),
            );
        }
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    match operations::status(&state, &query.zone, query.refresh).await {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(error) => operation_error_response(error),
    }
}

fn git_error_response(error: git::GitError) -> Response {
    let status = match &error {
        git::GitError::ZoneUnavailable(_) => StatusCode::NOT_FOUND,
        git::GitError::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
        git::GitError::GitUnavailable(_) | git::GitError::Failed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    rejected_owned(status, error.code(), error.message())
}

fn diff_error_response(error: git::DiffError) -> Response {
    match error {
        git::DiffError::Browse(error) => browse_error_response(error),
        git::DiffError::Git(error) => git_error_response(error),
    }
}

/// 一次审查 diff 请求的查询参数：登记路径、新路径与可选的 Git 旧路径。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DiffQuery {
    zone: String,
    path: String,
    #[serde(default)]
    old_path: Option<String>,
}

/// “读取审查 diff”：旧侧取当前分支最新本地提交，新侧取最终本地文件。
///
/// 新增、删除、已识别重命名与部分暂存都使用同一入口；旧路径只用于定位 Git 历史内容，仍先经过
/// zone 根边界检查。任一侧超限或二进制时返回明确限制，不返回完整内容。
pub(crate) async fn zone_change_diff(
    State(state): State<AppState>,
    query: Result<Query<DiffQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-query",
                "审查 diff 需要 `zone` 与 `path` 参数，可选 `oldPath`，且不接受其他参数。"
                    .to_string(),
            );
        }
    };
    if query.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请通过 `zone` 参数指定已登记的 zone 路径。".to_string(),
        );
    }
    if query.path.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-query",
            "审查 diff 需要非空的 `path` 参数。".to_string(),
        );
    }

    match operations::diff(&state, &query.zone, query.path, query.old_path).await {
        Ok(diff) => (StatusCode::OK, Json(diff)).into_response(),
        Err(error) => operation_error_response(error),
    }
}

/// 一次提交请求：服务器目录、非空提交信息，以及页面展示的审查版本标识。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ZoneCommitRequest {
    zone: String,
    message: String,
    review_token: String,
}

/// 一次真实提交成功后的回执：只有 Git 确认创建了新的 HEAD 才返回。
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

/// “经审查复核的整仓提交”。
///
/// 页面与直接请求都拒绝空信息、普通目录、阻塞状态和失效审查；被拒绝的请求不会产生 Git 写入。
/// 请求被接受后，执行放在独立任务里：客户端断线不会取消已经开始的提交，也不会在别处重放。
pub(crate) async fn commit_zone(State(state): State<AppState>, request: Request) -> Response {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "提交请求不接受查询参数：zone、提交信息与审查版本只来自请求体。".to_string(),
        );
    }

    let (_, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_ZONE_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "payload-too-large",
                "提交请求体超过 64KB 上限。".to_string(),
            );
        }
    };
    let request: ZoneCommitRequest = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(_) => {
            return rejected_owned(
                StatusCode::BAD_REQUEST,
                "invalid-request",
                "无法解析提交请求：请发送 {\"zone\":\"服务器目录\",\"message\":\"提交信息\",\"reviewToken\":\"审查版本\"} 形式的 JSON。"
                    .to_string(),
            );
        }
    };
    if request.zone.trim().is_empty() {
        return rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请指定要提交的已登记 zone。".to_string(),
        );
    }
    match operations::commit(state, &request.zone, request.message, request.review_token).await {
        Ok((zone, outcome)) => commit_response(zone, outcome),
        Err(error) => operation_error_response(error),
    }
}

fn commit_response(
    zone: String,
    outcome: commit::CommitOutcome,
) -> Response {
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

/// 一次“只有 zone”的 POST 请求体；检查远端与 Sync 都只接收服务器目录。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneTargetRequest {
    zone: String,
}

/// 读取检查远端/Sync 的请求体；请求面封闭，不接受查询串或未知字段。
async fn zone_target(request: Request) -> Result<String, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "该命令不接受查询参数：zone 只来自请求体。".to_string(),
        ));
    }
    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let request: ZoneTargetRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析请求：请发送 {\"zone\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    if request.zone.trim().is_empty() {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "zone-required",
            "请指定要操作的已登记 zone。".to_string(),
        ));
    }
    Ok(request.zone)
}

/// “检查远端”：真实 fetch 当前分支已有 upstream 的远端，成功后返回刷新后的完整 Git 状态。
///
/// 检查不要求工作树干净，也不修改工作树、索引或本地分支；普通目录、detached HEAD 与没有远端
/// upstream 的状态分别明确拒绝。检查与 Commit/Sync 共用同一份仓库写操作资格。
pub(crate) async fn check_zone_remote(State(state): State<AppState>, request: Request) -> Response {
    let zone = match zone_target(request).await {
        Ok(zone) => zone,
        Err(response) => return response,
    };
    let outcome = operations::check(&state, &zone).await;
    match outcome {
        Ok(sync::CheckOutcome::Checked(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(sync::CheckOutcome::NotRepository) => rejected_owned(
            StatusCode::CONFLICT,
            "not-a-repository",
            "该 zone 不是 Git 工作树根：普通目录没有可检查的远端。".to_string(),
        ),
        Ok(sync::CheckOutcome::Busy) => rejected_owned(
            StatusCode::CONFLICT,
            "git-write-in-progress",
            "该仓库已有 Git 写操作在执行：本次检查没有被接受，也不会在它结束后补做。".to_string(),
        ),
        Ok(sync::CheckOutcome::Blocked(reason)) => {
            rejected_owned(StatusCode::CONFLICT, "sync-blocked", reason)
        }
        Ok(sync::CheckOutcome::NoUpstream) => rejected_owned(
            StatusCode::CONFLICT,
            "no-upstream",
            "当前分支没有配置 upstream：无法确定要检查的远端；本功能不会代替你配置 upstream。"
                .to_string(),
        ),
        Ok(sync::CheckOutcome::Failed(error)) => git_error_response(error),
        Err(error) => operation_error_response(error),
    }
}

/// “按既有 upstream 同步”：检查远端 → pull → 按需 push。
///
/// 页面与直接请求都拒绝普通目录、脏工作树、detached HEAD、冲突或未完成流程，以及没有远端
/// upstream 的状态。执行放在独立任务里：客户端断线不会取消已接受的同步，也不会重放。
pub(crate) async fn sync_zone(State(state): State<AppState>, request: Request) -> Response {
    let zone = match zone_target(request).await {
        Ok(zone) => zone,
        Err(response) => return response,
    };
    match operations::sync(state, &zone).await {
        Ok((zone, outcome)) => sync_response(zone, outcome),
        Err(error) => operation_error_response(error),
    }
}

fn sync_response(
    zone: String,
    outcome: sync::SyncOutcome,
) -> Response {
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

fn operation_error_response(error: operations::Error) -> Response {
    use operations::{Error, Operation};
    match error {
        Error::Registry(error) => zone_error_response(error),
        Error::Lookup(error) => lookup_error_response(error),
        Error::Git(error) => git_error_response(error),
        Error::Diff(error) => diff_error_response(error),
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
