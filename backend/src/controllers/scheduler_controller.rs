//! HTTP adapters for shared definitions; business operations remain in scheduler.
use axum::{Json, response::{IntoResponse, Response}};
use crate::{runs, scheduler::{self, maps::{NodeIdentity, MapInputsSave, MapTitleSave, NodeDelete, NodeSave}}};

use super::{extractors::{CommandJson, catalog::{DeleteMap, MapList, MapRead, NewDraft, PreflightRun}}, responses::HttpError};

fn result<T: serde::Serialize>(value: Result<T, scheduler::DirectoryError>) -> Response {
    match value {
        Ok(value) => Json(value).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

pub async fn get_map(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    _: MapRead,
) -> Response {
    result(scheduler::maps::get_map(state, name).await)
}

pub async fn edit_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    result(scheduler::maps::edit_map_inputs(state, name).await)
}

pub async fn save_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<MapInputsSave>,
) -> Response {
    result(scheduler::maps::save_map_inputs(state, name, draft).await)
}

pub async fn edit_map_title(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    result(scheduler::maps::edit_map_title(state, name).await)
}

pub async fn save_map_title(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<MapTitleSave>,
) -> Response {
    result(scheduler::maps::save_map_title(state, name, draft).await)
}

pub async fn check_delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<NodeIdentity>,
) -> Response {
    result(scheduler::maps::check_delete_node(state, name, identity).await)
}

pub async fn delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(target): Json<NodeDelete>,
) -> Response {
    result(scheduler::maps::delete_node(state, name, target).await)
}

pub async fn new_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    _: NewDraft,
) -> Response {
    result(scheduler::maps::new_node(state, name).await)
}

pub async fn edit_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<NodeIdentity>,
) -> Response {
    result(scheduler::maps::edit_node(state, name, identity).await)
}

pub async fn save_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<NodeSave>,
) -> Response {
    result(scheduler::maps::save_node(state, name, draft).await)
}

pub async fn delete_map(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    input: CommandJson<scheduler::maps::MapDelete, DeleteMap>,
) -> Response {
    result(scheduler::maps::delete_map(state, name, input.value).await)
}

pub(crate) async fn set_map_group(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(input): Json<crate::tasks::GroupInput>,
) -> Response {
    result(scheduler::maps::set_map_group(state, name, input).await)
}

pub async fn list_maps(axum::extract::State(state): axum::extract::State<crate::AppState>, _: MapList) -> Response {
    result(scheduler::maps::list_maps(state).await)
}

pub async fn preflight_run(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    input: CommandJson<runs::preflight::RunPreflightRequest, PreflightRun>,
) -> Response {
    match runs::preflight::check_run_preflight(&state, &name, &input.value).await {
        Ok(checked) => Json(checked).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}

