//! HTTP parsing and response mappings for zone Git commands.
use axum::{Json, body::to_bytes, extract::{Query, Request, State}, extract::rejection::QueryRejection, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use crate::AppState;
use super::MAX_ZONE_REQUEST_BYTES;
use crate::zones::git::{self, operations};
use super::{rejected_owned, extractors::zones::ZoneTarget, responses::{HttpError, zone_git::{CommitResponse, CheckResponse, SyncResponse}}};

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
        Err(error) => HttpError(error).into_response(),
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
        Err(error) => HttpError(error).into_response(),
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
        Err(error) => HttpError(error).into_response(),
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
        Ok((zone, outcome)) => CommitResponse(zone, outcome).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}







/// “检查远端”：真实 fetch 当前分支已有 upstream 的远端，成功后返回刷新后的完整 Git 状态。
///
/// 检查不要求工作树干净，也不修改工作树、索引或本地分支；普通目录、detached HEAD 与没有远端
/// upstream 的状态分别明确拒绝。检查与 Commit/Sync 共用同一份仓库写操作资格。
pub(crate) async fn check_zone_remote(State(state): State<AppState>, ZoneTarget(zone): ZoneTarget) -> Response {
    match operations::check(&state, &zone).await {
        Ok(outcome) => CheckResponse(outcome).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

/// “按既有 upstream 同步”：检查远端 → pull → 按需 push。
///
/// 页面与直接请求都拒绝普通目录、脏工作树、detached HEAD、冲突或未完成流程，以及没有远端
/// upstream 的状态。执行放在独立任务里：客户端断线不会取消已接受的同步，也不会重放。
pub(crate) async fn sync_zone(State(state): State<AppState>, ZoneTarget(zone): ZoneTarget) -> Response {
    match operations::sync(state, &zone).await {
        Ok((zone, outcome)) => SyncResponse(zone, outcome).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}




