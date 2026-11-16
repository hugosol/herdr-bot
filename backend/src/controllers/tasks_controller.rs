use axum::{Json, extract::{Path as UrlPath, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
use crate::{AppState, tasks};
use super::{extractors::catalog::{TaskDelete, TaskInput}, responses::HttpError};
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
        Err(error) => HttpError(error).into_response(),
    }
}

/// “新增 Task”：固定字段逐个校验；只有存储成功后返回当前完整列表。
pub(crate) async fn create_task(State(state): State<AppState>, TaskInput(input): TaskInput) -> Response {
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
    TaskInput(input): TaskInput,
) -> Response {
    task_list_response(&state, state.tasks.update(&id, input).await)
}

/// “删除 Task”：只移除 Task，不改变任何已经创建的 Chat。
pub(crate) async fn delete_task(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    _: TaskDelete,
) -> Response {
    task_list_response(&state, state.tasks.delete(&id).await)
}

fn task_list_response(state: &AppState, result: Result<Vec<tasks::Task>, tasks::TaskError>) -> Response {
    catalog_response(state.tasks.catalog_result(result))
}

fn catalog_response(result: Result<tasks::TaskListResponse, tasks::CatalogError>) -> Response {
    match result {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

