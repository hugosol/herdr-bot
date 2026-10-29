//! Service-level static definitions and shared bot editing; no client-supplied filesystem paths.
use std::fs;
use std::path::{Path, PathBuf};

use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowMap {
    title: String,
    description: String,
    nodes: BTreeMap<String, JsonObject<MapNode>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MapNode {
    title: String,
    bot_id: String,
    input: String,
    zone: String,
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

pub(crate) fn node_dependencies(directory: &Path, name: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let map = read_map(directory, name).map_err(|e| e.message)?;
    Ok(map.nodes.into_iter().map(|(id, node)| (id, node.0.blocked_by)).collect())
}

pub(crate) struct DispatchTarget {
    pub title: String,
    pub input: String,
    pub zone: String,
    pub task: crate::tasks::Task,
}

pub(crate) async fn dispatch_target(state: &crate::AppState, map: &str, id: &str) -> Result<DispatchTarget, String> {
    let mut definition = read_map(&state.map_directory, map).map_err(|e| e.message)?;
    let node = definition.nodes.remove(id).ok_or("node 不在当前 Map 中")?.0;
    let bots = read_bots(&state.bot_directory).map_err(|e| e.message)?;
    let bot = bots.bots.iter().find(|b| b.id == node.bot_id).ok_or("node 的 Bot 缺失或不唯一")?;
    let task_id = bot.task_id.as_deref().ok_or("Bot 未绑定 Task")?;
    let task = state.tasks.get(task_id).await.map_err(|e| e.message())?;
    if task.task_type != crate::tasks::TaskType::Normal {
        return Err("node 必须绑定 normal 类型的 Task".into());
    }
    let zones = state.zones.list().await.map_err(|e| e.message())?;
    let zone = check_zone(&zones, &node.zone, "node").map_err(|e| e.message)?.to_owned();
    Ok(DispatchTarget { title: node.title, input: node.input, zone, task })
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

/// Launch configuration is not a definition override or a Run allocation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunPreflightRequest {
    pub manager_task_id: String,
    pub zone: String,
    pub user_input: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunPreflight {
    pub workspace_name: String,
    #[serde(skip)]
    pub node_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreflightIssue {
    code: &'static str,
    target: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
}

impl PreflightIssue {
    fn new(code: &'static str, target: &'static str, message: impl Into<String>) -> Self {
        Self { code, target, message: message.into(), node_id: None, bot_id: None, task_id: None }
    }
}

pub(crate) struct PreflightFailure(Vec<PreflightIssue>);
impl IntoResponse for PreflightFailure {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "run-preflight-rejected", "message": "启动预检未通过；请返回对应原配置修正后重新校验。",
            "issues": self.0,
        }))).into_response()
    }
}

fn check_role(tasks: &[crate::tasks::Task], id: &str, role: crate::tasks::TaskType, target: &'static str) -> Option<PreflightIssue> {
    use crate::tasks::TaskType;
    let role_name = if role == TaskType::Manager { "manager" } else { "normal" };
    let mut issue = match tasks.iter().find(|task| task.id == id) {
        None => PreflightIssue::new("task-missing", target, format!("Task {id} 不存在；请在 task 入口配置并修正引用。")),
        Some(task) if task.task_type != role => PreflightIssue::new("task-role-invalid", target, format!("Task {id} 必须是 {role_name} 类型；请回到此 Task 修正。")),
        Some(_) => return None,
    };
    issue.task_id = Some(id.to_owned());
    Some(issue)
}

fn check_zone<'a>(zones: &'a [crate::zones::RegisteredZone], requested: &str, target: &'static str) -> Result<&'a str, PreflightIssue> {
    if requested.trim().is_empty() {
        return Err(PreflightIssue::new("zone-required", target, "请选择或填写已登记 zone 的路径；请回到 zone 登记或 node 定义修正。"));
    }
    let root = crate::browse::registered_root(zones, requested).map_err(|error| PreflightIssue::new(error.code(), target, error.message()))?;
    crate::browse::checked_root(root).map_err(|error| PreflightIssue::new(error.code(), target, error.message()))?;
    Ok(root)
}

/// Shared by the read-only form and the real launch operation. Every call reads
/// current definitions, Task roles and registered roots; it never allocates a
/// Run, Chat identity, Tab, or fallback binding. user_input remains opaque.
pub(crate) async fn check_run_preflight(state: &crate::AppState, name: &str, input: &RunPreflightRequest) -> Result<RunPreflight, PreflightFailure> {
    let map_directory = state.map_directory.clone();
    let bot_directory = state.bot_directory.clone();
    let name = name.to_owned();
    let (map, bots) = tokio::task::spawn_blocking(move || {
        let map = read_map(&map_directory, &name)?;
        let bots = if map.nodes.is_empty() { BotCatalog { bots: Vec::new(), problems: Vec::new() } } else { read_bots(&bot_directory)? };
        Ok::<_, DirectoryError>((map, bots))
    }).await.map_err(|error| PreflightFailure(vec![PreflightIssue::new("definition-read-failed", "map", error.to_string())]))?
        .map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code, "map", error.message)]))?;
    if map.nodes.is_empty() {
        return Err(PreflightFailure(vec![PreflightIssue::new("map-empty", "map", "空 Map 不能启动；请返回 Map 新增 node。")]));
    }
    let tasks = state.tasks.list().await.map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code(), "task", error.message())]))?;
    let zones = state.zones.list().await.map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code(), "zone", error.message())]))?;
    let mut issues = Vec::new();
    if let Some(issue) = check_role(&tasks, &input.manager_task_id, crate::tasks::TaskType::Manager, "manager") { issues.push(issue); }
    let manager_zone = match check_zone(&zones, &input.zone, "manager") {
        Ok(zone) => Some(zone),
        Err(issue) => { issues.push(issue); None },
    };
    for (id, node) in &map.nodes {
        let node = &node.0;
        let mut binding_issue = match bots.bots.iter().find(|bot| bot.id == node.bot_id) {
            None => Some(PreflightIssue::new("bot-missing", "node", format!("node {id} 的 Bot {} 缺失或无效；请回到 Bot 管理修正引用或文件。", node.bot_id))),
            Some(bot) => match bot.task_id.as_deref().filter(|id| !id.trim().is_empty()) {
                None => Some(PreflightIssue::new("bot-task-required", "bot", format!("Bot {} 尚未绑定 Task；请回到此 Bot 配置 task_id。", bot.id))),
                Some(task_id) => check_role(&tasks, task_id, crate::tasks::TaskType::Normal, "task"),
            },
        };
        if let Some(issue) = &mut binding_issue { issue.node_id = Some(id.clone()); issue.bot_id = Some(node.bot_id.clone()); }
        issues.extend(binding_issue);
        if let Err(mut issue) = check_zone(&zones, &node.zone, "node") { issue.node_id = Some(id.clone()); issues.push(issue); }
    }
    if !issues.is_empty() { return Err(PreflightFailure(issues)); }
    let zone = manager_zone.expect("valid manager zone when no issues");
    let workspace_name = Path::new(&zone).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    if workspace_name.is_empty() {
        return Err(PreflightFailure(vec![PreflightIssue::new("workspace-name-unavailable", "manager", "管理者 zone 没有可推导的目录名称；请选择有目录名的 zone。")]));
    }
    // Reading the field is intentional: it is accepted verbatim, with no input
    // quality gate or rewriting into node.input / Task.prompt.
    let _ = &input.user_input;
    Ok(RunPreflight { workspace_name, node_ids: map.nodes.into_keys().collect() })
}

pub async fn preflight_run(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    let input = match crate::chat_command_body(request, "Run 启动预检").await {
        Ok(input) => input,
        Err(error) => return error,
    };
    match check_run_preflight(&state, &name, &input).await {
        Ok(checked) => Json(checked).into_response(),
        Err(error) => error.into_response(),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Bot {
    id: String,
    name: String,
    when: String,
    plan_guide: String,
    node_guide: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
}

#[derive(Serialize)]
struct BotProblem {
    code: &'static str,
    message: String,
    files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot_id: Option<String>,
}

#[derive(Serialize)]
struct BotCatalog {
    bots: Vec<Bot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    problems: Vec<BotProblem>,
}

fn bot_directory_error(error: DirectoryError) -> DirectoryError {
    DirectoryError {
        code: match error.code {
            "map-directory-create-failed" => "bot-directory-create-failed",
            "map-directory-link-rejected" => "bot-directory-link-rejected",
            _ => "bot-directory-read-failed",
        },
        message: error.message.replacen("Map 目录", "共享 Bot 目录", 1),
    }
}

fn read_bots(directory: &Path) -> Result<BotCatalog, DirectoryError> {
    // Reuse the Map directory boundary: only direct regular JSON files, no links
    // or reparse-point ancestors. Identity belongs to the parsed id, not the name.
    let files = read_files(directory).map_err(bot_directory_error)?;
    let mut bots = Vec::with_capacity(files.files.len());
    let mut problems = Vec::new();
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in files.files {
        let bytes = match fs::read(directory.join(&name)) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(BotProblem { code: "bot-read-failed", message: format!("共享 Bot {name} 读取失败：{error}"), files: vec![name], bot_id: None });
                continue;
            }
        };
        let bot: Bot = match serde_json::from_slice::<JsonObject<Bot>>(&bytes) {
            Ok(JsonObject(bot)) => bot,
            Err(error) => {
                let bot_id = serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                    .and_then(|value| value.get("id").and_then(|id| id.as_str()).map(str::to_owned));
                problems.push(BotProblem { code: "bot-invalid", message: format!("共享 Bot {name} 结构无效：{error}"), files: vec![name], bot_id });
                continue;
            }
        };
        identities.entry(bot.id.clone()).or_default().push(name);
        bots.push(bot);
    }
    // Even an invalid definition can declare an identity. Never hide that
    // ambiguity by choosing the remaining valid file as a winner.
    for problem in &problems {
        if let Some(id) = &problem.bot_id {
            identities.entry(id.clone()).or_default().extend(problem.files.iter().cloned());
        }
    }
    bots.retain(|bot| identities[&bot.id].len() == 1);
    let mut conflicts = Vec::new();
    for (id, mut names) in identities {
        if names.len() > 1 {
            names.sort();
            conflicts.push(BotProblem { code: "bot-id-conflict", message: format!("共享 Bot ID 冲突：{id}；相关文件：{}", names.join("、")), files: names, bot_id: Some(id) });
        }
    }
    conflicts.extend(problems);
    Ok(BotCatalog { bots, problems: conflicts })
}

pub async fn list_bots(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "共享 Bot 读取不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    match tokio::task::spawn_blocking(move || read_bots(&state.bot_directory)).await {
        Ok(Ok(catalog)) => Json(catalog).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => DirectoryError { code: "bot-directory-read-failed", message: format!("共享 Bot 目录读取失败：{error}") }.into_response(),
    }
}

// All scheduler mutations must hold this lock through final checks and commit.
// External file editors do not participate; recheck their changes before commit.
pub(crate) static DEFINITION_COMMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
static NEXT_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotIdentity { id: String }

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BotRevision { file: String, original: String }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotSave {
    id: String,
    content: String,
    revision: Option<BotRevision>,
}

fn edit_error(code: &'static str, message: impl Into<String>) -> DirectoryError {
    DirectoryError { code, message: message.into() }
}

// Resolve identity from current files, never from an ID interpreted as a path.
fn bot_target(directory: &Path, id: &str) -> Result<Option<BotRevision>, DirectoryError> {
    let catalog = read_bots(directory)?;
    if let Some(problem) = catalog.problems.iter().find(|problem| problem.bot_id.as_deref() == Some(id)) {
        return Err(edit_error("bot-conflict", format!("{}；请重新加载。", problem.message)));
    }
    for file in read_files(directory).map_err(bot_directory_error)?.files {
        let path = directory.join(&file);
        let metadata = fs::symlink_metadata(&path).map_err(|e| edit_error("bot-conflict", format!("Bot 已变化：{e}；请重新加载。")))?;
        if !metadata.is_file() || crate::browse::is_link_like(&metadata) {
            return Err(edit_error("bot-conflict", "Bot 文件已变化；请重新加载。"));
        }
        let original = fs::read_to_string(path).map_err(|e| edit_error("bot-read-failed", format!("Bot 读取失败：{e}")))?;
        if serde_json::from_str::<serde_json::Value>(&original).ok().and_then(|value| value.get("id").and_then(|v| v.as_str()).map(str::to_owned)).as_deref() == Some(id) {
            return Ok(Some(BotRevision { file, original }));
        }
    }
    Ok(None)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotDelete {
    id: String,
    revision: BotRevision,
}

fn check_bot_references(directory: &Path, id: &str) -> Result<(), DirectoryError> {
    let root = if directory.is_absolute() { directory.to_owned() } else {
        std::env::current_dir().map_err(|error| edit_error("bot-reference-check-failed",
            format!("无法定位 Map 目录 {}：{error}", directory.display())))?.join(directory)
    };
    let directory = root.as_path();
    let files = read_files(directory).map_err(|error| edit_error("bot-reference-check-failed",
        format!("无法完整检查 Map 目录 {}：{}", directory.display(), error.message)))?;
    let mut references = Vec::new();
    let mut failures = Vec::new();
    for file in files.files {
        match read_map(directory, &file) {
            Ok(map) => {
                for (node_id, node) in map.nodes {
                    if node.0.bot_id == id {
                        references.push(format!("Map {file} / node {node_id}"));
                    }
                }
            }
            Err(error) => failures.push(format!("{}：{}", directory.join(file).display(), error.message)),
        }
    }
    if !failures.is_empty() {
        return Err(edit_error("bot-reference-check-failed", format!("无法完整检查 Bot 引用，禁止删除：{}{}",
            failures.join("；"), if references.is_empty() { String::new() } else { format!("；已发现引用：{}", references.join("；")) })));
    }
    if !references.is_empty() {
        return Err(edit_error("bot-referenced", format!("Bot {id} 仍被引用，不能删除：{}", references.join("；"))));
    }
    Ok(())
}

pub async fn check_bot_delete(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("bot-delete-failed", "定义提交锁不可用。"))?;
        let revision = bot_target(&state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        check_bot_references(&state.map_directory, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": revision }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-delete-failed", error.to_string()).into_response(),
    }
}

pub async fn delete_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(target): Json<BotDelete>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        // The same commit boundary protects node/bot saves and deletions.
        // Never carry the pre-confirmation reference result into this request.
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("bot-delete-failed", "定义提交锁不可用。"))?;
        let root = prepare_directory(&state.bot_directory).map_err(bot_directory_error)?;
        let check_target = || -> Result<(), DirectoryError> {
            if bot_target(&root, &target.id)?.as_ref() != Some(&target.revision) {
                return Err(edit_error("bot-conflict", "Bot 文件已变化或移除；请重新加载后再删除。"));
            }
            Ok(())
        };
        check_target()?;
        check_bot_references(&state.map_directory, &target.id)?;
        check_target()?;
        fs::remove_file(root.join(&target.revision.file))
            .map_err(|error| edit_error("bot-delete-failed", format!("Bot {} 删除失败：{error}", target.revision.file)))?;
        read_bots(&root)
    }).await {
        Ok(Ok(catalog)) => Json(catalog).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-delete-failed", error.to_string()).into_response(),
    }
}

pub async fn new_bot(request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return edit_error("invalid-request", "新增草稿不接受查询参数或请求体。").into_response();
    }
    // Compile the documentation example into the application; no runtime repo access.
    let mut example: serde_json::Value = serde_json::from_str(include_str!("../../docs/example/bots/weather-capability.json")).expect("bundled weather example");
    example.as_object_mut().expect("example object").remove("id");
    Json(example).into_response()
}

pub async fn edit_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("bot-write-failed", "定义提交锁不可用。"))?;
        let revision = bot_target(&state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        let mut content: serde_json::Value = serde_json::from_str(&revision.original)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 必须为 JSON 对象。"))?.remove("id");
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": content, "revision": revision }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-read-failed", error.to_string()).into_response(),
    }
}

pub async fn save_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(draft): Json<BotSave>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        if draft.id.is_empty() { return Err(edit_error("bot-invalid", "Bot ID 不能为空。")); }
        let mut content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        let object = content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("bot-invalid", "请移除 Bot 内容中的顶层 id；使用独立 Bot ID。")); }
        object.insert("id".into(), serde_json::Value::String(draft.id.clone()));
        let JsonObject(bot): JsonObject<Bot> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot 结构无效：{e}")))?;
        let bytes = serde_json::to_vec_pretty(&bot).map_err(|e| edit_error("bot-write-failed", e.to_string()))?;
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("bot-write-failed", "定义提交锁不可用。"))?;
        let root = prepare_directory(&state.bot_directory).map_err(bot_directory_error)?;
        let check = || -> Result<(), DirectoryError> {
            prepare_directory(&root).map_err(bot_directory_error)?;
            let current = bot_target(&root, &draft.id)?;
            if draft.revision.is_none() && current.is_some() {
                return Err(edit_error("bot-conflict", "Bot ID 已存在；请使用不同 ID 或重新加载。"));
            }
            if draft.revision != current { return Err(edit_error("bot-conflict", "Bot 文件已变化或移除；请重新加载后再编辑。")); }
            Ok(())
        };
        check()?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let serial = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!("bot-{stamp}-{}-{serial}", std::process::id());
        let temporary = root.join(format!("{name}.tmp"));
        let target = root.join(draft.revision.as_ref().map_or_else(|| format!("{name}.json"), |revision| revision.file.clone()));
        let result = (|| {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
                .map_err(|e| edit_error("bot-write-failed", format!("Bot 写入失败：{e}")))?;
            file.write_all(&bytes).and_then(|_| file.sync_all())
                .map_err(|e| edit_error("bot-write-failed", format!("Bot 写入失败：{e}")))?;
            drop(file);
            check()?;
            if draft.revision.is_some() { fs::rename(&temporary, &target) } else { fs::hard_link(&temporary, &target) }
                .map_err(|e| edit_error("bot-write-failed", format!("Bot 写入失败：{e}")))?;
            Ok(bot)
        })();
        let _ = fs::remove_file(temporary);
        result
    }).await {
        Ok(Ok(bot)) => Json(bot).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-write-failed", error.to_string()).into_response(),
    }
}

// Both preflight and final deletion validate the entire owning Map. A failed
// read/validation is never evidence that the target has no dependents.
fn deletable_node(map: &WorkflowMap, id: &str) -> Result<(), DirectoryError> {
    if !map.nodes.contains_key(id) {
        return Err(edit_error("node-conflict", "node 已移除或无法识别；请重新加载。"));
    }
    let dependents: Vec<_> = map.nodes.iter()
        .filter(|(_, node)| node.0.blocked_by.iter().any(|parent| parent == id))
        .map(|(id, node)| format!("{}（{id}）", node.0.title)).collect();
    if !dependents.is_empty() {
        return Err(edit_error("node-referenced", format!("阻止删除：以下 node 的 blocked_by 依赖此 node：{}。不会级联删除或清除依赖。", dependents.join("、"))));
    }
    Ok(())
}

pub async fn check_delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("node-write-failed", "定义提交锁不可用。"))?;
        let original = read_map_source(&state.map_directory, &name)?;
        deletable_node(&parse_map(&original)?, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": { "file": name, "original": original } }))
    }).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("node-read-failed", error.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeDelete {
    id: String,
    revision: BotRevision,
}

pub async fn delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(target): Json<NodeDelete>,
) -> Response {
    match tokio::task::spawn_blocking(move || {
        let _commit = DEFINITION_COMMIT.lock().map_err(|_| edit_error("node-write-failed", "定义提交锁不可用。"))?;
        let check = || -> Result<WorkflowMap, DirectoryError> {
            let original = read_map_source(&state.map_directory, &name)?;
            let map = parse_map(&original)?;
            deletable_node(&map, &target.id)?;
            if target.revision.file != name || target.revision.original != original {
                return Err(edit_error("node-conflict", "Map 文件已变化；请重新加载后再确认删除。"));
            }
            Ok(map)
        };
        let mut map = check()?;
        map.nodes.remove(&target.id);
        let bytes = serde_json::to_vec_pretty(&map).map_err(|e| edit_error("node-write-failed", e.to_string()))?;
        let root = prepare_directory(&state.map_directory)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let serial = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temporary = root.join(format!("node-delete-{stamp}-{}-{serial}.tmp", std::process::id()));
        let result: Result<_, DirectoryError> = (|| {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
                .map_err(|e| edit_error("node-write-failed", format!("node 删除写入失败：{e}")))?;
            file.write_all(&bytes).and_then(|_| file.sync_all())
                .map_err(|e| edit_error("node-write-failed", format!("node 删除写入失败：{e}")))?;
            drop(file);
            // External editors cannot take our lock: re-read and revalidate at
            // the commit boundary, rather than trusting even the first check.
            check()?;
            fs::rename(&temporary, root.join(&name))
                .map_err(|e| edit_error("node-write-failed", format!("node 删除写入失败：{e}")))?;
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSave {
    id: String,
    content: String,
    revision: BotRevision,
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
    Json(identity): Json<BotIdentity>,
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
