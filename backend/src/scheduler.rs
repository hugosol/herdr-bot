//! Service-level static definitions and shared template editing; no client-supplied filesystem paths.
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

fn read_map_source(directory: &Path, name: &str) -> Result<String, DirectoryError> {
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
    fs::read_to_string(file).map_err(|e| map_error(format!("Map 文件读取失败：{e}")))
}

fn read_map(directory: &Path, name: &str) -> Result<WorkflowMap, DirectoryError> {
    parse_map(&read_map_source(directory, name)?)
}

fn parse_map(source: &str) -> Result<WorkflowMap, DirectoryError> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let JsonObject(map): JsonObject<WorkflowMap> = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|e| map_error(format!("Map 结构无效：{e}")))?;
    deserializer.end().map_err(|e| map_error(format!("Map JSON 无效：{e}")))?;
    validate_map(&map)?;
    Ok(map)
}

fn validate_map(map: &WorkflowMap) -> Result<(), DirectoryError> {
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
    Ok(())
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
    implementation: JsonObject<TemplateImplementation>,
}

#[derive(Serialize)]
struct TemplateProblem {
    code: &'static str,
    message: String,
    files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<String>,
}

#[derive(Serialize)]
struct TemplateCatalog {
    templates: Vec<NodeTemplate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    problems: Vec<TemplateProblem>,
}

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
    let mut problems = Vec::new();
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in files.files {
        let bytes = match fs::read(directory.join(&name)) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(TemplateProblem { code: "template-read-failed", message: format!("共享模板 {name} 读取失败：{error}"), files: vec![name], template_id: None });
                continue;
            }
        };
        let template: NodeTemplate = match serde_json::from_slice(&bytes) {
            Ok(template) => template,
            Err(error) => {
                let template_id = serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                    .and_then(|value| value.get("id").and_then(|id| id.as_str()).map(str::to_owned));
                problems.push(TemplateProblem { code: "template-invalid", message: format!("共享模板 {name} 结构无效：{error}"), files: vec![name], template_id });
                continue;
            }
        };
        if template.schema_version != 1 {
            problems.push(TemplateProblem { code: "template-invalid", message: format!("共享模板 {name} 的 schema_version 必须为 1。"), files: vec![name], template_id: Some(template.id) });
            continue;
        }
        identities.entry(template.id.clone()).or_default().push(name);
        templates.push(template);
    }
    // Even an invalid definition can declare an identity. Never hide that
    // ambiguity by choosing the remaining valid file as a winner.
    for problem in &problems {
        if let Some(id) = &problem.template_id {
            identities.entry(id.clone()).or_default().extend(problem.files.iter().cloned());
        }
    }
    templates.retain(|template| identities[&template.id].len() == 1);
    let mut conflicts = Vec::new();
    for (id, mut names) in identities {
        if names.len() > 1 {
            names.sort();
            conflicts.push(TemplateProblem { code: "template-id-conflict", message: format!("共享模板 ID 冲突：{id}；相关文件：{}", names.join("、")), files: names, template_id: Some(id) });
        }
    }
    conflicts.extend(problems);
    Ok(TemplateCatalog { templates, problems: conflicts })
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

// All scheduler mutations must hold this lock through final checks and commit.
// External file editors do not participate; recheck their changes before commit.
pub(crate) static DEFINITION_COMMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
static NEXT_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateIdentity { id: String }

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct TemplateRevision { file: String, original: String }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateSave {
    id: String,
    content: String,
    revision: Option<TemplateRevision>,
}

fn edit_error(code: &'static str, message: impl Into<String>) -> DirectoryError {
    DirectoryError { code, message: message.into() }
}

// Resolve identity from current files, never from an ID interpreted as a path.
fn template_target(directory: &Path, id: &str) -> Result<Option<TemplateRevision>, DirectoryError> {
    let catalog = read_templates(directory)?;
    if let Some(problem) = catalog.problems.iter().find(|problem| problem.template_id.as_deref() == Some(id)) {
        return Err(edit_error("template-conflict", format!("{}；请重新加载。", problem.message)));
    }
    for file in read_files(directory).map_err(template_directory_error)?.files {
        let path = directory.join(&file);
        let metadata = fs::symlink_metadata(&path).map_err(|e| edit_error("template-conflict", format!("模板已变化：{e}；请重新加载。")))?;
        if !metadata.is_file() || crate::browse::is_link_like(&metadata) {
            return Err(edit_error("template-conflict", "模板文件已变化；请重新加载。"));
        }
        let original = fs::read_to_string(path).map_err(|e| edit_error("template-read-failed", format!("模板读取失败：{e}")))?;
        if serde_json::from_str::<serde_json::Value>(&original).ok().and_then(|value| value.get("id").and_then(|v| v.as_str()).map(str::to_owned)).as_deref() == Some(id) {
            return Ok(Some(TemplateRevision { file, original }));
        }
    }
    Ok(None)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDelete {
    id: String,
    revision: TemplateRevision,
}

fn check_template_references(directory: &Path, id: &str) -> Result<(), DirectoryError> {
    let root = if directory.is_absolute() { directory.to_owned() } else {
        std::env::current_dir().map_err(|error| edit_error("template-reference-check-failed",
            format!("无法定位 Map 目录 {}：{error}", directory.display())))?.join(directory)
    };
    let directory = root.as_path();
    let files = read_files(directory).map_err(|error| edit_error("template-reference-check-failed",
        format!("无法完整检查 Map 目录 {}：{}", directory.display(), error.message)))?;
    let mut references = Vec::new();
    let mut failures = Vec::new();
    for file in files.files {
        match read_map(directory, &file) {
            Ok(map) => {
                for (node_id, node) in map.nodes {
                    if node.0.node_template_id == id {
                        references.push(format!("Map {file} / node {node_id}"));
                    }
                }
            }
            Err(error) => failures.push(format!("{}：{}", directory.join(file).display(), error.message)),
        }
    }
    if !failures.is_empty() {
        return Err(edit_error("template-reference-check-failed", format!("无法完整检查模板引用，禁止删除：{}{}",
            failures.join("；"), if references.is_empty() { String::new() } else { format!("；已发现引用：{}", references.join("；")) })));
    }
    if !references.is_empty() {
        return Err(edit_error("template-referenced", format!("模板 {id} 仍被引用，不能删除：{}", references.join("；"))));
    }
    Ok(())
}

pub async fn check_template_delete(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<TemplateIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("template-delete-failed", "定义提交锁不可用。"))?;
        let revision = template_target(&state.node_template_directory, &identity.id)?
            .ok_or_else(|| edit_error("template-conflict", "模板已移除；请重新加载。"))?;
        check_template_references(&state.map_directory, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": revision }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("template-delete-failed", error.to_string()).into_response(),
    }
}

pub async fn delete_template(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(target): Json<TemplateDelete>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        // The same commit boundary protects node/template saves and deletions.
        // Never carry the pre-confirmation reference result into this request.
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("template-delete-failed", "定义提交锁不可用。"))?;
        let root = prepare_directory(&state.node_template_directory).map_err(template_directory_error)?;
        let check_target = || -> Result<(), DirectoryError> {
            if template_target(&root, &target.id)?.as_ref() != Some(&target.revision) {
                return Err(edit_error("template-conflict", "模板文件已变化或移除；请重新加载后再删除。"));
            }
            Ok(())
        };
        check_target()?;
        check_template_references(&state.map_directory, &target.id)?;
        check_target()?;
        fs::remove_file(root.join(&target.revision.file))
            .map_err(|error| edit_error("template-delete-failed", format!("模板 {} 删除失败：{error}", target.revision.file)))?;
        read_templates(&root)
    }).await {
        Ok(Ok(catalog)) => Json(catalog).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("template-delete-failed", error.to_string()).into_response(),
    }
}

pub async fn new_template(request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return edit_error("invalid-request", "新增草稿不接受查询参数或请求体。").into_response();
    }
    // Compile the documentation example into the application; no runtime repo access.
    let mut example: serde_json::Value = serde_json::from_str(include_str!("../../docs/example/node-templates/weather-capability.json")).expect("bundled weather example");
    example.as_object_mut().expect("example object").remove("id");
    Json(example).into_response()
}

pub async fn edit_template(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<TemplateIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("template-write-failed", "定义提交锁不可用。"))?;
        let revision = template_target(&state.node_template_directory, &identity.id)?
            .ok_or_else(|| edit_error("template-conflict", "模板已移除；请重新加载。"))?;
        let mut content: serde_json::Value = serde_json::from_str(&revision.original)
            .map_err(|e| edit_error("template-invalid", format!("模板 JSON 无效：{e}")))?;
        content.as_object_mut().ok_or_else(|| edit_error("template-invalid", "模板必须为 JSON 对象。"))?.remove("id");
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": content, "revision": revision }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("template-read-failed", error.to_string()).into_response(),
    }
}

pub async fn save_template(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(draft): Json<TemplateSave>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        if draft.id.is_empty() { return Err(edit_error("template-invalid", "模板 ID 不能为空。")); }
        let mut content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("template-invalid", format!("模板 JSON 无效：{e}")))?;
        let object = content.as_object_mut().ok_or_else(|| edit_error("template-invalid", "模板内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("template-invalid", "请移除模板内容中的顶层 id；使用独立模板 ID。")); }
        object.insert("id".into(), serde_json::Value::String(draft.id.clone()));
        let JsonObject(template): JsonObject<NodeTemplate> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("template-invalid", format!("模板结构无效：{e}")))?;
        if template.schema_version != 1 { return Err(edit_error("template-invalid", "模板 schema_version 必须为 1。")); }
        let bytes = serde_json::to_vec_pretty(&template).map_err(|e| edit_error("template-write-failed", e.to_string()))?;
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("template-write-failed", "定义提交锁不可用。"))?;
        let root = prepare_directory(&state.node_template_directory).map_err(template_directory_error)?;
        let check = || -> Result<(), DirectoryError> {
            prepare_directory(&root).map_err(template_directory_error)?;
            let current = template_target(&root, &draft.id)?;
            if draft.revision.is_none() && current.is_some() {
                return Err(edit_error("template-conflict", "模板 ID 已存在；请使用不同 ID 或重新加载。"));
            }
            if draft.revision != current { return Err(edit_error("template-conflict", "模板文件已变化或移除；请重新加载后再编辑。")); }
            Ok(())
        };
        check()?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let serial = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!("template-{stamp}-{}-{serial}", std::process::id());
        let temporary = root.join(format!("{name}.tmp"));
        let target = root.join(draft.revision.as_ref().map_or_else(|| format!("{name}.json"), |revision| revision.file.clone()));
        let result = (|| {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
                .map_err(|e| edit_error("template-write-failed", format!("模板写入失败：{e}")))?;
            file.write_all(&bytes).and_then(|_| file.sync_all())
                .map_err(|e| edit_error("template-write-failed", format!("模板写入失败：{e}")))?;
            drop(file);
            check()?;
            if draft.revision.is_some() { fs::rename(&temporary, &target) } else { fs::hard_link(&temporary, &target) }
                .map_err(|e| edit_error("template-write-failed", format!("模板写入失败：{e}")))?;
            Ok(template)
        })();
        let _ = fs::remove_file(temporary);
        result
    }).await {
        Ok(Ok(template)) => Json(template).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("template-write-failed", error.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSave {
    id: String,
    content: String,
    revision: TemplateRevision,
    create: bool,
}

pub async fn new_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return edit_error("invalid-request", "新增草稿不接受查询参数或请求体。").into_response();
    }
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("node-write-failed", "定义提交锁不可用。"))?;
        let original = read_map_source(&state.map_directory, &name)?;
        parse_map(&original)?;
        let mut example: serde_json::Value = serde_json::from_str(include_str!("../../docs/example/maps/trip-readiness.json")).expect("bundled Map example");
        let content = example["nodes"]["A"].take();
        Ok::<_, DirectoryError>(serde_json::json!({ "id": "", "content": content, "revision": { "file": name, "original": original }, "create": true }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("node-read-failed", error.to_string()).into_response(),
    }
}

pub async fn edit_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<TemplateIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("node-write-failed", "定义提交锁不可用。"))?;
        let original = read_map_source(&state.map_directory, &name)?;
        let map = parse_map(&original)?;
        let node = map.nodes.get(&identity.id).ok_or_else(|| edit_error("node-conflict", "node 已移除；请重新加载。"))?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": node, "revision": { "file": name, "original": original }, "create": false }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("node-read-failed", error.to_string()).into_response(),
    }
}

pub async fn save_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<NodeSave>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        if draft.id.is_empty() { return Err(edit_error("node-invalid", "node ID 不能为空。")); }
        let content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("node-invalid", format!("node JSON 无效：{e}")))?;
        let object = content.as_object().ok_or_else(|| edit_error("node-invalid", "node 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("node-invalid", "请移除 node 内容中的顶层 id；使用独立 node ID。")); }
        let node: JsonObject<MapNode> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("node-invalid", format!("node 结构无效：{e}")))?;
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("node-write-failed", "定义提交锁不可用。"))?;
        let check = || -> Result<(), DirectoryError> {
            let current = read_map_source(&state.map_directory, &name)
                .map_err(|error| edit_error("node-conflict", format!("Map 文件已变化或移除：{}；请重新加载。", error.message)))?;
            if draft.revision.file != name || draft.revision.original != current {
                return Err(edit_error("node-conflict", "Map 文件已变化或移除；请重新加载后再编辑。"));
            }
            Ok(())
        };
        check()?;
        let mut map = parse_map(&draft.revision.original)?;
        if draft.create && map.nodes.contains_key(&draft.id) {
            return Err(edit_error("node-conflict", "当前 Map 的 node ID 已存在；请使用不同 ID 或重新加载。"));
        }
        if !draft.create && !map.nodes.contains_key(&draft.id) {
            return Err(edit_error("node-conflict", "node 已移除；请重新加载。"));
        }
        map.nodes.insert(draft.id, node);
        validate_map(&map)?;
        let bytes = serde_json::to_vec_pretty(&map).map_err(|e| edit_error("node-write-failed", e.to_string()))?;
        let root = prepare_directory(&state.map_directory)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let serial = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temporary = root.join(format!("node-{stamp}-{}-{serial}.tmp", std::process::id()));
        let result = (|| {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
                .map_err(|e| edit_error("node-write-failed", format!("node 写入失败：{e}")))?;
            file.write_all(&bytes).and_then(|_| file.sync_all())
                .map_err(|e| edit_error("node-write-failed", format!("node 写入失败：{e}")))?;
            drop(file);
            check()?;
            fs::rename(&temporary, root.join(&name)).map_err(|e| edit_error("node-write-failed", format!("node 写入失败：{e}")))?;
            Ok(map)
        })();
        let _ = fs::remove_file(temporary);
        result
    }).await {
        Ok(Ok(map)) => Json(map).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("node-write-failed", error.to_string()).into_response(),
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
