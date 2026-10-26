//! Service-level read-only Map directory; no client-supplied filesystem paths.
use std::fs;
use std::path::{Path, PathBuf};

use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowMap {
    schema_version: u32,
    title: String,
    description: String,
    nodes: BTreeMap<String, JsonObject<MapNode>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MapNode {
    title: String,
    node_template_id: String,
    parameters: BTreeMap<String, serde_json::Value>,
    blocked_by: Vec<String>,
}

// Serde's derived structs also accept positional arrays. Map and node definitions
// require named JSON objects; forward only map access without copying the input.
#[derive(Serialize)]
#[serde(transparent)]
struct JsonObject<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for JsonObject<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
            type Value = JsonObject<T>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object with named fields")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(JsonObject)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
    }
}

fn map_error(message: impl Into<String>) -> DirectoryError {
    DirectoryError { code: "map-read-failed", message: message.into() }
}

fn read_map(directory: &Path, name: &str) -> Result<WorkflowMap, DirectoryError> {
    // A filename is not a path. Reject both platform separators, Windows streams
    // and links, using the same ancestor boundary as the directory listing.
    if name.is_empty() || name.contains(['/', '\\', ':']) ||
        !Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
        return Err(map_error("Map 读取只接受目录内的 JSON 文件名。"));
    }
    let file = prepare_directory(directory)?.join(name);
    let metadata = fs::symlink_metadata(&file).map_err(|e| map_error(format!("Map 文件读取失败：{e}")))?;
    if !metadata.is_file() || crate::browse::is_link_like(&metadata) {
        return Err(map_error("Map 文件不能是符号链接、重解析点或目录。"));
    }
    let bytes = fs::read(file).map_err(|e| map_error(format!("Map 文件读取失败：{e}")))?;
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let JsonObject(map): JsonObject<WorkflowMap> = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|e| map_error(format!("Map 结构无效：{e}")))?;
    deserializer.end().map_err(|e| map_error(format!("Map JSON 无效：{e}")))?;
    if map.schema_version != 1 { return Err(map_error("Map schema_version 必须为 1。")); }
    // Reject invalid dependencies rather than dropping an edge or manufacturing
    // a successful graph. Kahn's traversal also bounds layout work on bad input.
    let mut remaining = BTreeMap::new();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, node) in &map.nodes {
        let node = &node.0;
        remaining.insert(id.as_str(), node.blocked_by.len());
        for prerequisite in &node.blocked_by {
            if prerequisite == id {
                return Err(map_error(format!("node {id} 的 blocked_by 引用了自身：{prerequisite}")));
            }
            if !map.nodes.contains_key(prerequisite) {
                return Err(map_error(format!("node {id} 的 blocked_by 引用了未知 node：{prerequisite}")));
            }
            dependents.entry(prerequisite).or_default().push(id);
        }
    }
    let mut ready: VecDeque<&str> = remaining.iter().filter_map(|(id, count)| (*count == 0).then_some(*id)).collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for dependent in dependents.get(id).into_iter().flatten() {
            let count = remaining.get_mut(dependent).expect("dependent is a Map node");
            *count -= 1;
            if *count == 0 { ready.push_back(dependent); }
        }
    }
    if visited != map.nodes.len() {
        let blocked = remaining.iter().filter_map(|(id, count)| (*count > 0).then_some(*id)).collect::<Vec<_>>().join("、");
        return Err(map_error(format!("Map blocked_by 存在依赖环；无法解除前置依赖的 node：{blocked}。")));
    }
    Ok(map)
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
    match tokio::task::spawn_blocking(move || {
        read_map(&state.map_directory, &name).map_err(|mut error| {
            error.message = format!("Map 文件 {name}：{}", error.message);
            error
        })
    }).await {
        Ok(Ok(map)) => Json(map).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => map_error(format!("Map 文件读取失败：{error}")).into_response(),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ImplementationType { Chat }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TemplateImplementation { r#type: ImplementationType }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeTemplate {
    schema_version: u32,
    id: String,
    title: String,
    use_when: String,
    description: String,
    inputs: BTreeMap<String, String>,
    outputs: BTreeMap<String, String>,
    implementation: TemplateImplementation,
}

#[derive(Serialize)]
struct TemplateCatalog { templates: Vec<NodeTemplate> }

fn template_directory_error(error: DirectoryError) -> DirectoryError {
    DirectoryError {
        code: match error.code {
            "map-directory-create-failed" => "template-directory-create-failed",
            "map-directory-link-rejected" => "template-directory-link-rejected",
            _ => "template-directory-read-failed",
        },
        message: error.message.replacen("Map 目录", "共享模板目录", 1),
    }
}

fn read_templates(directory: &Path) -> Result<TemplateCatalog, DirectoryError> {
    // Reuse the Map directory boundary: only direct regular JSON files, no links
    // or reparse-point ancestors. Identity belongs to the parsed id, not the name.
    let files = read_files(directory).map_err(template_directory_error)?;
    let mut templates = Vec::with_capacity(files.files.len());
    for name in files.files {
        let bytes = fs::read(directory.join(&name)).map_err(|e| DirectoryError {
            code: "template-read-failed", message: format!("共享模板 {name} 读取失败：{e}"),
        })?;
        let template: NodeTemplate = serde_json::from_slice(&bytes).map_err(|e| DirectoryError {
            code: "template-read-failed", message: format!("共享模板 {name} 结构无效：{e}"),
        })?;
        if template.schema_version != 1 {
            return Err(DirectoryError { code: "template-read-failed", message: format!("共享模板 {name} 的 schema_version 必须为 1。") });
        }
        templates.push(template);
    }
    Ok(TemplateCatalog { templates })
}

pub async fn list_templates(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "共享模板读取不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    match tokio::task::spawn_blocking(move || read_templates(&state.node_template_directory)).await {
        Ok(Ok(catalog)) => Json(catalog).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => DirectoryError { code: "template-directory-read-failed", message: format!("共享模板目录读取失败：{error}") }.into_response(),
    }
}

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
