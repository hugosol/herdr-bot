//! HTTP adapters for shared definitions; business operations remain in scheduler.
use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use crate::scheduler::{self, BotIdentity, MapInputsSave, MapTitleSave, NodeDelete, NodeSave};

pub(crate) fn definition_error(error: scheduler::DirectoryError) -> Response {
    let status = match error.code {
        "map-input-invalid" | "map-title-invalid" | "group-invalid" => StatusCode::BAD_REQUEST,
        "map-conflict" | "bot-conflict" | "script-policy-invalid" => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({"code": error.code, "message": error.message}))).into_response()
}

fn result<T: serde::Serialize>(value: Result<T, scheduler::DirectoryError>) -> Response {
    match value {
        Ok(value) => Json(value).into_response(),
        Err(error) => definition_error(error),
    }
}

pub async fn get_map(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "Map 读取不接受查询参数或请求体。",
        }))).into_response();
    }
    result(scheduler::get_map(state, name).await)
}

pub async fn edit_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    result(scheduler::edit_map_inputs(state, name).await)
}

pub async fn save_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<MapInputsSave>,
) -> Response {
    result(scheduler::save_map_inputs(state, name, draft).await)
}

pub async fn edit_map_title(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    result(scheduler::edit_map_title(state, name).await)
}

pub async fn save_map_title(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<MapTitleSave>,
) -> Response {
    result(scheduler::save_map_title(state, name, draft).await)
}

pub async fn check_delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::check_delete_node(state, name, identity).await)
}

pub async fn delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(target): Json<NodeDelete>,
) -> Response {
    result(scheduler::delete_node(state, name, target).await)
}

pub async fn new_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return definition_error(scheduler::DirectoryError { code: "invalid-request", message: "新增草稿不接受查询参数或请求体。".into() });
    }
    result(scheduler::new_node(state, name).await)
}

pub async fn edit_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    result(scheduler::edit_node(state, name, identity).await)
}

pub async fn save_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<NodeSave>,
) -> Response {
    result(scheduler::save_node(state, name, draft).await)
}

pub async fn delete_map(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    let input: scheduler::MapDelete = match crate::controllers::chat_command_body(request, "删除 Map").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    result(scheduler::delete_map(state, name, input).await)
}

pub(crate) async fn set_map_group(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(input): Json<crate::tasks::GroupInput>,
) -> Response {
    result(scheduler::set_map_group(state, name, input).await)
}

pub async fn list_maps(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "Map 列表不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    result(scheduler::list_maps(state).await)
}

pub async fn preflight_run(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    let input = match crate::controllers::chat_command_body(request, "Run 启动预检").await {
        Ok(input) => input,
        Err(error) => return error,
    };
    match scheduler::check_run_preflight(&state, &name, &input).await {
        Ok(checked) => Json(checked).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(error.into_payload())).into_response(),
    }
}

