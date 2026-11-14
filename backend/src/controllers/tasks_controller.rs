use axum::{Json, body::to_bytes, extract::{Path as UrlPath, Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
use crate::{AppState, tasks};
use super::{rejected, rejected_owned};
const MAX_TASK_REQUEST_BYTES: usize = 1024 * 1024;
#[derive(Serialize)]
struct TaskView { task: tasks::Task }
/// “列出 Task 与固定选项”：读取后端持久保存的同一份列表。
pub(crate) async fn list_tasks(State(state): State<AppState>) -> Response {
    catalog_response(state.tasks.list_catalog().await)
}

/// “读取单个 Task”：删除后由独立客户端按标识选择时得到明确的不存在。
pub(crate) async fn get_task(State(state): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    match state.tasks.get_validated(&id).await {
        Ok(task) => (StatusCode::OK, Json(TaskView { task })).into_response(),
        Err(error) => task_error_response(error),
    }
}

/// “新增 Task”：固定字段逐个校验；只有存储成功后返回当前完整列表。
pub(crate) async fn create_task(State(state): State<AppState>, request: Request) -> Response {
    let input = match task_input(request).await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    task_list_response(&state, state.tasks.create(input).await)
}

pub(crate) async fn set_task_group(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(input): Json<tasks::GroupInput>,
) -> Response {
    task_list_response(&state, state.tasks.set_group(&id, input.group).await)
}

/// “编辑 Task”：标识不变；只有存储成功后返回当前完整列表。
pub(crate) async fn update_task(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    let input = match task_input(request).await {
        Ok(input) => input,
        Err(rejection) => return rejection,
    };
    task_list_response(&state, state.tasks.update(&id, input).await)
}

/// “删除 Task”：只移除 Task，不改变任何已经创建的 Chat。
pub(crate) async fn delete_task(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    request: Request,
) -> Response {
    if let Some(rejection) = reject_task_delete_request(request).await {
        return rejection;
    }
    task_list_response(&state, state.tasks.delete(&id).await)
}

/// 读取 Task 命令的请求体；请求面封闭（不接受查询串或未知字段）。
async fn task_input(request: Request) -> Result<tasks::TaskInput, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "Task命令不接受查询参数：字段只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_TASK_REQUEST_BYTES)
        .await
        .map_err(|_| {
            rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                "Task请求体超过 1 MiB 上限。".to_string(),
            )
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            format!(
                "无法解析Task命令：{error}；请发送包含 name、chatName、agent、model、thinking、skill、prompt 的 JSON。"
            ),
        )
    })
}

/// 删除 Task 的请求面：不接受查询串或请求体。
async fn reject_task_delete_request(request: Request) -> Option<Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "删除Task不接受查询参数：目标只由路径中的Task标识决定。",
        ));
    }

    let (_, body) = request.into_parts();
    match to_bytes(body, MAX_TASK_REQUEST_BYTES).await {
        Ok(bytes) if bytes.is_empty() => None,
        _ => Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "删除Task不接受请求体：目标只由路径中的Task标识决定。",
        )),
    }
}

fn task_list_response(state: &AppState, result: Result<Vec<tasks::Task>, tasks::TaskError>) -> Response {
    catalog_response(state.tasks.catalog_result(result))
}

fn catalog_response(result: Result<tasks::TaskListResponse, tasks::CatalogError>) -> Response {
    match result {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(tasks::CatalogError::Task(error)) => task_error_response(error),
        Err(tasks::CatalogError::Suite(error)) => super::suites_controller::error_response(error),
    }
}

pub(crate) fn task_error_response(error: tasks::TaskError) -> Response {
    task_error_with_context(error, "")
}

pub(crate) fn task_error_with_context(error: tasks::TaskError, context: &str) -> Response {
    use tasks::TaskError;
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
    rejected_owned(
        status,
        error.code(),
        format!("{}{context}", error.message()),
    )
}

