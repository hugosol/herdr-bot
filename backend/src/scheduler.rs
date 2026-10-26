//! Service-level read-only Map directory; no client-supplied filesystem paths.
use std::fs;
use std::path::{Path, PathBuf};

use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;

#[derive(Serialize)]
struct MapFiles { files: Vec<String> }

struct DirectoryError { code: &'static str, message: String }
impl DirectoryError {
    fn io(code: &'static str, action: &str, error: std::io::Error) -> Self {
        Self { code, message: format!("Map 目录{action}失败：{error}") }
    }
}
impl IntoResponse for DirectoryError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "code": self.code, "message": self.message,
        }))).into_response()
    }
}

// Inspect every existing ancestor before creating any missing component. Never
// follow symlinks, junctions or other reparse points, even above the configured root.
fn prepare_directory(directory: &Path) -> Result<PathBuf, DirectoryError> {
    let absolute = if directory.is_absolute() { directory.to_owned() } else {
        std::env::current_dir().map_err(|e| DirectoryError::io("map-directory-create-failed", "创建", e))?.join(directory)
    };
    let mut checked = PathBuf::new();
    for component in absolute.components() {
        checked.push(component);
        let metadata = match fs::symlink_metadata(&checked) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&checked).map_err(|e| DirectoryError::io("map-directory-create-failed", "创建", e))?;
                fs::symlink_metadata(&checked).map_err(|e| DirectoryError::io("map-directory-create-failed", "检查", e))?
            }
            Err(error) => return Err(DirectoryError::io("map-directory-create-failed", "检查", error)),
        };
        if crate::browse::is_link_like(&metadata) {
            return Err(DirectoryError { code: "map-directory-link-rejected", message: "Map 目录不能经过符号链接、目录联接或重解析点。".into() });
        }
        if !metadata.is_dir() {
            return Err(DirectoryError { code: "map-directory-create-failed", message: "Map 目录创建失败：配置位置或其父路径不是目录。".into() });
        }
    }
    Ok(absolute)
}

fn read_files(directory: &Path) -> Result<MapFiles, DirectoryError> {
    let root = prepare_directory(directory)?;
    let entries = fs::read_dir(root).map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
        if !metadata.is_file() || crate::browse::is_link_like(&metadata) { continue; }
        if path.extension().is_some_and(|value| value.eq_ignore_ascii_case("json")) {
            if let Some(name) = entry.file_name().to_str() { files.push(name.to_owned()); }
        }
    }
    files.sort();
    Ok(MapFiles { files })
}

pub async fn list_maps(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "Map 列表不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    match tokio::task::spawn_blocking(move || read_files(&state.map_directory)).await {
        Ok(Ok(files)) => Json(files).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => DirectoryError { code: "map-directory-read-failed", message: format!("Map 目录读取失败：{error}") }.into_response(),
    }
}
