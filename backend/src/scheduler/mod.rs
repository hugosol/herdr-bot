//! Service-level static definitions and shared bot editing; no client-supplied filesystem paths.
use crate::files::{FileAccess, FileStore, PublishMode};
use std::path::{Path, PathBuf};

use axum::{Json, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkflowMap {
    title: String,
    description: String,
    #[serde(default, deserialize_with = "object_array", skip_serializing_if = "Vec::is_empty")]
    input_files: Vec<MapFileInput>,
    nodes: BTreeMap<String, JsonObject<MapNode>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportPolicy {
    #[default]
    Auto,
    Manual,
}

impl<'de> Deserialize<'de> for ReportPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PolicyVisitor;
        impl serde::de::Visitor<'_> for PolicyVisitor {
            type Value = ReportPolicy;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string auto or manual")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                match value {
                    "auto" => Ok(ReportPolicy::Auto),
                    "manual" => Ok(ReportPolicy::Manual),
                    _ => Err(E::unknown_variant(value, &["auto", "manual"])),
                }
            }
        }
        deserializer.deserialize_str(PolicyVisitor)
    }
}

impl ReportPolicy {
    pub(crate) fn control_section(self) -> &'static str {
        match self {
            Self::Auto => "report_policy: auto\nReport handoff-ready results without waiting for user confirmation. Reporting does not complete the node; the manager still decides complete.",
            Self::Manual => "report_policy: manual\nInvoke report with the full intended payload to display it for explicit native user confirmation before sending. Natural-language agreement is not report authorization. Every report, including supplements, corrections and changed payloads, requires its own native confirmation. If confirmation is refused, times out or no confirmation UI is available, do not send and stop; wait for a renewed user request before invoking report again. Reporting does not complete the node; the manager still decides complete. Separate business approval requirements still apply.",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputRule {
    File { id: String, path: String, #[serde(default, skip_serializing_if = "Option::is_none")] zone_id: Option<String> },
    Node { id: String, node_id: String, output_id: String },
    Run { id: String, input_id: String },
}
impl InputRule {
    pub fn id(&self) -> &str { match self { Self::File { id, .. } | Self::Node { id, .. } | Self::Run { id, .. } => id } }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRule { pub id: String }
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapFileInput { pub id: String, pub required: bool }
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFile { pub id: String, pub path: String }

/// Check external business material without reading, copying or retaining a
/// handle. Like public file reads, reject links/reparse points and streams;
/// the file need not belong to any registered zone.
pub(crate) fn check_input_file(path: &str) -> Result<(), String> {
    let file = Path::new(path);
    if !file.is_absolute() { return Err("文件输入必须是绝对地址".into()); }
    let parent = file.parent().and_then(Path::to_str).ok_or("文件输入缺少有效父目录")?;
    let name = file.file_name().and_then(|name| name.to_str()).ok_or("文件输入缺少有效文件名")?;
    crate::zones::browse::checked_file(parent, name).map_err(|error| error.message())?;
    std::fs::File::open(file).map(drop).map_err(|error| format!("文件 {path} 不可读：{error}"))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MapNode {
    title: String,
    bot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    manager_task_id: Option<String>,
    input: String,
    #[serde(default)]
    input_rule: Vec<InputRule>,
    #[serde(default, deserialize_with = "object_array")]
    output_rule: Vec<OutputRule>,
    #[serde(default)]
    report_policy: ReportPolicy,
    #[serde(default)]
    auto_close: bool,
    zone: String,
    blocked_by: Vec<String>,
}

// Serde's derived structs also accept positional arrays. Map and node definitions
// require named JSON objects; forward only map access without copying the input.
#[derive(Serialize)]
#[serde(transparent)]
pub(crate) struct JsonObject<T>(pub(crate) T);

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

pub(crate) fn object_array<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Vec<T>, D::Error> {
    Vec::<JsonObject<T>>::deserialize(deserializer).map(|values| values.into_iter().map(|value| value.0).collect())
}

pub(crate) fn optional_object_array<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Vec<T>>, D::Error> {
    Option::<Vec<JsonObject<T>>>::deserialize(deserializer).map(|values| values.map(|values| values.into_iter().map(|value| value.0).collect()))
}

fn map_error(message: impl Into<String>) -> DirectoryError {
    DirectoryError { code: "map-read-failed", message: message.into() }
}

fn map_file_path(access: &FileAccess, directory: &Path, name: &str) -> Result<PathBuf, DirectoryError> {
    // A filename is not a path. Reject both platform separators, Windows streams
    // and links, using the same ancestor boundary as the directory listing.
    if name.is_empty() || name.contains(['/', '\\', ':']) ||
        !Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
        return Err(map_error("Map 读取只接受目录内的 JSON 文件名。"));
    }
    let file = prepare_directory(access, directory)?.join(name);
    let metadata = access.symlink_metadata(&file).map_err(|e| map_error(format!("Map 文件读取失败：{e}")))?;
    if !metadata.is_file() || crate::zones::browse::is_link_like(&metadata) {
        return Err(map_error("Map 文件不能是符号链接、重解析点或目录。"));
    }
    Ok(file)
}

fn read_map_source(access: &FileAccess, directory: &Path, name: &str) -> Result<String, DirectoryError> {
    access.read_to_string(map_file_path(access, directory, name)?)
        .map_err(|e| map_error(format!("Map 文件读取失败：{e}")))
}

fn read_map(access: &FileAccess, directory: &Path, name: &str) -> Result<WorkflowMap, DirectoryError> {
    parse_map(&read_map_source(access, directory, name)?)
}

pub(crate) fn node_dependencies(directory: &Path, name: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let map = with_definitions(&[directory.to_owned()], "map-read-failed", |access| read_map(access, directory, name)).map_err(|e| e.message)?;
    Ok(map.nodes.into_iter().map(|(id, node)| (id, node.0.blocked_by)).collect())
}

pub(crate) fn node_report_policy(directory: &Path, name: &str, id: &str) -> Result<ReportPolicy, String> {
    let map = with_definitions(&[directory.to_owned()], "map-read-failed", |access| read_map(access, directory, name)).map_err(|e| e.message)?;
    map.nodes.get(id).map(|node| node.0.report_policy).ok_or_else(|| "node 不在当前 Map 中".into())
}

pub(crate) struct DispatchTarget {
    pub title: String,
    pub input: String,
    pub input_rule: Vec<InputRule>,
    pub output_rule: Vec<OutputRule>,
    pub report_policy: ReportPolicy,
    pub auto_close: bool,
    pub zone: String,
    pub zone_id: String,
    pub task: crate::tasks::Task,
    pub map: Option<String>,
}

pub(crate) async fn dispatch_target(state: &crate::AppState, map: &str, id: &str) -> Result<DispatchTarget, String> {
    let (node, bots) = with_definitions(
        &[state.map_directory.clone(), state.bot_directory.clone()], "map-read-failed", |access| {
            let mut definition = read_map(access, &state.map_directory, map)?;
            let node = definition.nodes.remove(id).ok_or_else(|| map_error("node 不在当前 Map 中"))?.0;
            Ok((node, read_bots(access, &state.bot_directory)?))
        },
    ).map_err(|e| e.message)?;
    let bot = bots.bots.iter().find(|b| b.id == node.bot_id).ok_or("node 的 Bot 缺失或不唯一")?;
    let (task_id, required_role) = match (bot.task_id.as_deref(), bot.map_file.as_deref()) {
        (Some(task), None) if node.manager_task_id.is_none() => (task, crate::tasks::TaskType::Normal),
        (None, Some(_)) => (node.manager_task_id.as_deref().ok_or("Map 型 node 缺少 manager_task_id")?, crate::tasks::TaskType::Manager),
        _ => return Err("Bot 执行目标或节点 manager 配置无效".into()),
    };
    let task = state.tasks.get(task_id).await.map_err(|e| e.message())?;
    if task.task_type != required_role {
        return Err("node 的执行 Task 类型与目标不一致".into());
    }
    let zone = node.zone;
    Ok(DispatchTarget { title: node.title, input: node.input, input_rule: node.input_rule, output_rule: node.output_rule, report_policy: node.report_policy, auto_close: node.auto_close, zone_id: zone.clone(), zone, task, map: bot.map_file.clone() })
}

fn parse_map(source: &str) -> Result<WorkflowMap, DirectoryError> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let JsonObject(map): JsonObject<WorkflowMap> = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|e| map_error(format!("Map 结构无效：{e}")))?;
    deserializer.end().map_err(|e| map_error(format!("Map JSON 无效：{e}")))?;
    validate_map(&map)?;
    Ok(map)
}

pub(crate) fn valid_relative_file(path: &str) -> bool {
    !path.is_empty() && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
        && !Path::new(path).is_absolute()
}

fn validate_map(map: &WorkflowMap) -> Result<(), DirectoryError> {
    let mut input_ids = std::collections::BTreeSet::new();
    for input in &map.input_files {
        if input.id.trim().is_empty() || !input_ids.insert(&input.id) {
            return Err(map_error(format!("Map input_files ID 为空或重复：{}", input.id)));
        }
    }
    // Reject invalid dependencies rather than dropping an edge or manufacturing
    // a successful graph. Kahn's traversal also bounds layout work on bad input.
    let mut remaining = BTreeMap::new();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, node) in &map.nodes {
        let node = &node.0;
        let mut input_ids = std::collections::BTreeSet::new();
        let mut output_ids = std::collections::BTreeSet::new();
        for output in &node.output_rule {
            if output.id.trim().is_empty() || !output_ids.insert(&output.id) { return Err(map_error(format!("node {id} 的 output_rule ID 重复：{}", output.id))); }
        }
        for input in &node.input_rule {
            let name = input.id();
            if name.trim().is_empty() || !input_ids.insert(name) { return Err(map_error(format!("node {id} 的 input_rule ID 重复：{name}"))); }
            match input {
                InputRule::File { path, zone_id, .. } if !valid_relative_file(path) || zone_id.as_ref().is_some_and(|id| id.trim().is_empty()) => {
                    return Err(map_error(format!("node {id} 的输入 {name} 必须使用 zone 内的相对文件地址（/ 分隔，不允许绝对路径或 ..）；旧格式不受支持，请显式提供 zone ID 和相对路径。")));
                }
                InputRule::Run { input_id, .. } => {
                    if !map.input_files.iter().any(|input| input.id == *input_id) {
                        return Err(map_error(format!("node {id} 的输入 {name} 引用了未声明 Run 输入：{input_id}")));
                    }
                }
                InputRule::Node { node_id, output_id, .. } => {
                    let upstream = map.nodes.get(node_id).ok_or_else(|| map_error(format!("node {id} 的输入 {name} 引用了未知 node：{node_id}")))?;
                    if !upstream.0.output_rule.iter().any(|output| output.id == *output_id) {
                        return Err(map_error(format!("node {id} 的输入 {name} 引用了未声明输出：{node_id}/{output_id}")));
                    }
                }
                _ => {}
            }
        }
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        read_map(access, &state.map_directory, &name).map_err(|mut error| {
            error.message = format!("Map 文件 {name}：{}", error.message);
            error
        })
    })).await {
        Ok(Ok(map)) => Json(map).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => map_error(format!("Map 文件读取失败：{error}")).into_response(),
    }
}

pub async fn edit_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        Ok::<_, DirectoryError>(serde_json::json!({"input_files": map.input_files, "revision": {"file": name, "original": original}}))
    })).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => map_error(error.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapInputsSave {
    #[serde(deserialize_with = "object_array")]
    input_files: Vec<MapFileInput>,
    revision: BotRevision,
}

pub async fn save_map_inputs(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(draft): Json<MapInputsSave>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-write-failed", |access| {
        let check = || -> Result<(), DirectoryError> {
            let current = read_map_source(access, &state.map_directory, &name)?;
            if draft.revision.file != name || draft.revision.original != current {
                return Err(edit_error("map-conflict", "Map 文件已变化；请重新加载后再编辑输入声明。"));
            }
            Ok(())
        };
        check()?;
        let mut map = parse_map(&draft.revision.original)?;
        map.input_files = draft.input_files;
        validate_map(&map).map_err(|error| edit_error("map-input-invalid", error.message))?;
        let bytes = serde_json::to_vec_pretty(&map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check(), |error| edit_error("map-write-failed", format!("Map 输入声明写入失败：{error}")))?;
        Ok(map)
    })).await {
        Ok(Ok(map)) => Json(map).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("map-write-failed", error.to_string()).into_response(),
    }
}

/// Launch configuration is not a definition override or a Run allocation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RunPreflightRequest {
    pub manager_task_id: String,
    pub zone: String,
    pub user_input: String,
    #[serde(default, deserialize_with = "object_array")]
    pub file_inputs: Vec<ResolvedFile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunPreflight {
    pub workspace_name: String,
    #[serde(skip)]
    pub node_ids: Vec<String>,
    #[serde(skip)]
    pub zone_bindings: BTreeMap<String, String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    input_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    call_chain: Vec<String>,
}

impl PreflightIssue {
    fn new(code: &'static str, target: &'static str, message: impl Into<String>) -> Self {
        Self { code, target, message: message.into(), node_id: None, bot_id: None, task_id: None, input_id: None, call_chain: Vec::new() }
    }
}

pub(crate) struct PreflightFailure(Vec<PreflightIssue>);
impl PreflightFailure {
    pub(crate) fn message(&self) -> String {
        self.0.iter().map(|issue| issue.message.as_str()).collect::<Vec<_>>().join("；")
    }
}
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
        return Err(PreflightIssue::new("zone-required", target, "请选择或填写已登记的 zone ID；请回到 zone 登记或 node 定义修正。"));
    }
    let root = zones.iter().find(|zone| zone.id == requested).map(|zone| zone.path.as_str())
        .ok_or_else(|| PreflightIssue::new("zone-not-registered", target, format!("zone ID {requested} 未在本机绑定；请在 zone 登记中绑定目录。旧 Map 路径请先显式迁移。")))?;
    crate::zones::browse::checked_root(root).map_err(|error| PreflightIssue::new(error.code(), target, error.message()))?;
    Ok(root)
}

fn check_map_calls(
    access: &FileAccess, directory: &Path, map: &WorkflowMap, bots: &BotCatalog,
    tasks: &[crate::tasks::Task], zones: &[crate::zones::RegisteredZone],
    manager_zone: &str, path: &mut Vec<String>, issues: &mut Vec<PreflightIssue>,
) {
    for (id, node) in &map.nodes {
        let node = &node.0;
        let first_issue = issues.len();
        match bots.bots.iter().find(|bot| bot.id == node.bot_id) {
            None => issues.push(PreflightIssue::new("bot-missing", "node", format!("node {id} 的 Bot {} 缺失或无效；请修正引用或文件。", node.bot_id))),
            Some(bot) => match (&bot.task_id, &bot.map_file) {
                (Some(task), None) => {
                    if task.trim().is_empty() {
                        issues.push(PreflightIssue::new("bot-task-required", "bot", format!("Bot {} 尚未绑定 Task 或 Map 执行目标。", bot.id)));
                    } else {
                        issues.extend(check_role(tasks, task, crate::tasks::TaskType::Normal, "task"));
                    }
                    if node.manager_task_id.is_some() {
                        issues.push(PreflightIssue::new("node-manager-unexpected", "node", "普通 Task 型 node 不能指定 manager_task_id。"));
                    }
                }
                (None, Some(target)) => {
                    match node.manager_task_id.as_deref().filter(|id| !id.trim().is_empty()) {
                        Some(manager) => issues.extend(check_role(tasks, manager, crate::tasks::TaskType::Manager, "manager")),
                        None => issues.push(PreflightIssue::new("node-manager-required", "node", "Map 型 node 必须指定 manager_task_id。")),
                    }
                    if path.contains(target) {
                        let mut issue = PreflightIssue::new("map-call-cycle", "map", format!("Map 调用循环：{} → {target}", path.join(" → ")));
                        issue.call_chain = path.iter().cloned().chain(std::iter::once(target.clone())).collect();
                        issues.push(issue);
                    } else {
                        match read_map(access, directory, target) {
                            Ok(child) if child.nodes.is_empty() => issues.push(PreflightIssue::new("map-empty", "map", format!("目标 Map {target} 为空，不能启动。"))),
                            Ok(child) => {
                                path.push(target.clone());
                                check_map_calls(access, directory, &child, bots, tasks, zones, &node.zone, path, issues);
                                path.pop();
                            }
                            Err(error) => issues.push(PreflightIssue::new(error.code, "map", format!("目标 Map {target} 无法读取：{}", error.message))),
                        }
                    }
                }
                _ => issues.push(PreflightIssue::new("bot-task-required", "bot", format!("Bot {} 尚未绑定唯一的 Task 或 Map 执行目标。", bot.id))),
            },
        }
        if let Err(issue) = check_zone(zones, &node.zone, "node") { issues.push(issue); }
        for rule in &node.input_rule {
            if let InputRule::File { zone_id, .. } = rule {
                if let Err(issue) = check_zone(zones, zone_id.as_deref().unwrap_or(manager_zone), "file") { issues.push(issue); }
            }
        }
        for issue in &mut issues[first_issue..] {
            if issue.node_id.is_none() {
                issue.node_id = Some(id.clone());
                issue.bot_id = Some(node.bot_id.clone());
                if issue.call_chain.is_empty() { issue.call_chain = path.clone(); }
            }
        }
    }
}

/// Shared by the read-only form and the real launch operation. Every call reads
/// current definitions, Task roles and registered roots; it never allocates a
/// Run, Chat identity, Tab, or fallback binding. user_input remains opaque.
pub(crate) async fn check_run_preflight(state: &crate::AppState, name: &str, input: &RunPreflightRequest) -> Result<RunPreflight, PreflightFailure> {
    let map_directory = state.map_directory.clone();
    let bot_directory = state.bot_directory.clone();
    let map_name = name;
    let name = name.to_owned();
    let root_name = name.clone();
    let (map, bots) = tokio::task::spawn_blocking(move || with_definitions(
        &[map_directory.clone(), bot_directory.clone()], "map-read-failed", |access| {
        let map = read_map(access, &map_directory, &name)?;
        let bots = if map.nodes.is_empty() { BotCatalog { bots: Vec::new(), problems: Vec::new() } } else { read_bots(access, &bot_directory)? };
        Ok::<_, DirectoryError>((map, bots))
    })).await.map_err(|error| PreflightFailure(vec![PreflightIssue::new("definition-read-failed", "map", error.to_string())]))?
        .map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code, "map", error.message)]))?;
    if map.nodes.is_empty() {
        return Err(PreflightFailure(vec![PreflightIssue::new("map-empty", "map", "空 Map 不能启动；请返回 Map 新增 node。")]));
    }
    let tasks = state.tasks.list().await.map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code(), "task", error.message())]))?;
    let zones = state.zones.list().await.map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code(), "zone", error.message())]))?;
    let mut issues = Vec::new();
    let mut provided = std::collections::BTreeSet::new();
    for binding in &input.file_inputs {
        let failure = if !provided.insert(&binding.id) {
            Some(("run-input-duplicate", format!("Run 文件输入 ID 重复：{}", binding.id)))
        } else if !map.input_files.iter().any(|declaration| declaration.id == binding.id) {
            Some(("run-input-undeclared", format!("Map {map_name} 未声明文件输入：{}", binding.id)))
        } else if let Err(error) = check_input_file(&binding.path) {
            Some(("run-input-unreadable", format!("Run 文件输入 {}：{error}", binding.id)))
        } else { None };
        if let Some((code, message)) = failure {
            let mut issue = PreflightIssue::new(code, "run-input", message);
            issue.input_id = Some(binding.id.clone());
            issues.push(issue);
        }
    }
    for declaration in &map.input_files {
        if declaration.required && !provided.contains(&declaration.id) {
            let mut issue = PreflightIssue::new("run-input-required", "run-input", format!("Map {map_name} 缺少必需文件输入：{}", declaration.id));
            issue.input_id = Some(declaration.id.clone());
            issues.push(issue);
        }
    }
    if let Some(issue) = check_role(&tasks, &input.manager_task_id, crate::tasks::TaskType::Manager, "manager") { issues.push(issue); }
    let manager_zone = match check_zone(&zones, &input.zone, "manager") {
        Ok(zone) => Some(zone),
        Err(issue) => { issues.push(issue); None },
    };
    with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-read-failed", |access| {
        check_map_calls(access, &state.map_directory, &map, &bots, &tasks, &zones, &input.zone, &mut vec![root_name], &mut issues);
        Ok(())
    }).map_err(|error| PreflightFailure(vec![PreflightIssue::new(error.code, "map", error.message)]))?;
    if !issues.is_empty() { return Err(PreflightFailure(issues)); }
    let zone = manager_zone.expect("valid manager zone when no issues");
    let workspace_name = Path::new(&zone).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    if workspace_name.is_empty() {
        return Err(PreflightFailure(vec![PreflightIssue::new("workspace-name-unavailable", "manager", "管理者 zone 没有可推导的目录名称；请选择有目录名的 zone。")]));
    }
    // Reading the field is intentional: it is accepted verbatim, with no input
    // quality gate or rewriting into node.input / Task.prompt.
    let _ = &input.user_input;
    let mut zone_bindings = BTreeMap::new();
    zone_bindings.insert(input.zone.clone(), zone.to_owned());
    for node in map.nodes.values() {
        for id in std::iter::once(node.0.zone.as_str()).chain(node.0.input_rule.iter().filter_map(|rule| match rule {
            InputRule::File { zone_id: Some(id), .. } => Some(id.as_str()), _ => None,
        })) {
            let path = zones.iter().find(|zone| zone.id == id).expect("preflight checked binding").path.clone();
            zone_bindings.insert(id.to_owned(), path);
        }
    }
    Ok(RunPreflight { workspace_name, node_ids: map.nodes.into_keys().collect(), zone_bindings })
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
    #[serde(skip_serializing_if = "Option::is_none")]
    map_file: Option<String>,
}

impl Bot {
    fn validate_target(&self) -> Result<(), DirectoryError> {
        if self.task_id.is_some() && self.map_file.is_some() {
            return Err(edit_error("bot-invalid", "Bot 的 task_id 与 map_file 互斥；请只选择一个执行目标。"));
        }
        Ok(())
    }
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

fn read_bots(access: &FileAccess, directory: &Path) -> Result<BotCatalog, DirectoryError> {
    // Reuse the Map directory boundary: only direct regular JSON files, no links
    // or reparse-point ancestors. Identity belongs to the parsed id, not the name.
    let files = read_files(access, directory).map_err(bot_directory_error)?;
    let mut bots = Vec::with_capacity(files.files.len());
    let mut problems = Vec::new();
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in files.files {
        let bytes = match access.read(directory.join(&name)) {
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
        if let Err(error) = bot.validate_target() {
            problems.push(BotProblem { code: error.code, message: format!("共享 Bot {name}：{}", error.message), files: vec![name], bot_id: Some(bot.id) });
            continue;
        }
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-directory-read-failed", |access| read_bots(access, &state.bot_directory))).await {
        Ok(Ok(catalog)) => Json(catalog).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => DirectoryError { code: "bot-directory-read-failed", message: format!("共享 Bot 目录读取失败：{error}") }.into_response(),
    }
}

// Each definition operation owns its directory scope through final checks and commit.
// External editors do not participate, so publication still rechecks revisions.
fn with_definitions<R>(
    paths: &[PathBuf],
    code: &'static str,
    action: impl FnOnce(&FileAccess) -> Result<R, DirectoryError>,
) -> Result<R, DirectoryError> {
    FileStore::default().with(paths, action)
        .map_err(|error| edit_error(code, format!("定义文件访问失败：{error}")))?
}
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
fn bot_target(access: &FileAccess, directory: &Path, id: &str) -> Result<Option<BotRevision>, DirectoryError> {
    let catalog = read_bots(access, directory)?;
    if let Some(problem) = catalog.problems.iter().find(|problem| problem.bot_id.as_deref() == Some(id)) {
        return Err(edit_error("bot-conflict", format!("{}；请重新加载。", problem.message)));
    }
    for file in read_files(access, directory).map_err(bot_directory_error)?.files {
        let path = directory.join(&file);
        let metadata = access.symlink_metadata(&path).map_err(|e| edit_error("bot-conflict", format!("Bot 已变化：{e}；请重新加载。")))?;
        if !metadata.is_file() || crate::zones::browse::is_link_like(&metadata) {
            return Err(edit_error("bot-conflict", "Bot 文件已变化；请重新加载。"));
        }
        let original = access.read_to_string(path).map_err(|e| edit_error("bot-read-failed", format!("Bot 读取失败：{e}")))?;
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

fn check_bot_references(access: &FileAccess, directory: &Path, id: &str) -> Result<(), DirectoryError> {
    let root = if directory.is_absolute() { directory.to_owned() } else {
        std::env::current_dir().map_err(|error| edit_error("bot-reference-check-failed",
            format!("无法定位 Map 目录 {}：{error}", directory.display())))?.join(directory)
    };
    let directory = root.as_path();
    let files = read_files(access, directory).map_err(|error| edit_error("bot-reference-check-failed",
        format!("无法完整检查 Map 目录 {}：{}", directory.display(), error.message)))?;
    let mut references = Vec::new();
    let mut failures = Vec::new();
    for file in files.files {
        match read_map(access, directory, &file) {
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone(), state.map_directory.clone()], "bot-delete-failed", |access| {
        let revision = bot_target(access, &state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        check_bot_references(access, &state.map_directory, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": revision }))
    })).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-delete-failed", error.to_string()).into_response(),
    }
}

pub async fn delete_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(target): Json<BotDelete>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone(), state.map_directory.clone()], "bot-delete-failed", |access| {
        // Both directory scopes protect references against node/bot saves and deletions.
        // Never carry the pre-confirmation reference result into this request.
        let root = prepare_directory(access, &state.bot_directory).map_err(bot_directory_error)?;
        let check_target = || -> Result<(), DirectoryError> {
            if bot_target(access, &root, &target.id)?.as_ref() != Some(&target.revision) {
                return Err(edit_error("bot-conflict", "Bot 文件已变化或移除；请重新加载后再删除。"));
            }
            Ok(())
        };
        check_target()?;
        check_bot_references(access, &state.map_directory, &target.id)?;
        check_target()?;
        access.remove_file(root.join(&target.revision.file))
            .map_err(|error| edit_error("bot-delete-failed", format!("Bot {} 删除失败：{error}", target.revision.file)))?;
        read_bots(access, &root)
    })).await {
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
    let mut example: serde_json::Value = serde_json::from_str(include_str!("../../../docs/example/bots/weather-capability.json")).expect("bundled weather example");
    example.as_object_mut().expect("example object").remove("id");
    Json(example).into_response()
}

pub async fn edit_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-write-failed", |access| {
        let revision = bot_target(access, &state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        let mut content: serde_json::Value = serde_json::from_str(&revision.original)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 必须为 JSON 对象。"))?.remove("id");
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": content, "revision": revision }))
    })).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("bot-read-failed", error.to_string()).into_response(),
    }
}

pub async fn save_bot(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    Json(draft): Json<BotSave>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-write-failed", |access| {
        if draft.id.is_empty() { return Err(edit_error("bot-invalid", "Bot ID 不能为空。")); }
        let mut content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        let object = content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("bot-invalid", "请移除 Bot 内容中的顶层 id；使用独立 Bot ID。")); }
        object.insert("id".into(), serde_json::Value::String(draft.id.clone()));
        let JsonObject(bot): JsonObject<Bot> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot 结构无效：{e}")))?;
        bot.validate_target()?;
        let bytes = serde_json::to_vec_pretty(&bot).map_err(|e| edit_error("bot-write-failed", e.to_string()))?;
        let root = prepare_directory(access, &state.bot_directory).map_err(bot_directory_error)?;
        let check = || -> Result<(), DirectoryError> {
            prepare_directory(access, &root).map_err(bot_directory_error)?;
            let current = bot_target(access, &root, &draft.id)?;
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
        let target = root.join(draft.revision.as_ref().map_or_else(|| format!("{name}.json"), |revision| revision.file.clone()));
        let mode = if draft.revision.is_some() { PublishMode::Replace } else { PublishMode::Create };
        access.publish_checked(&target, &bytes, mode, |_| check(),
            |error| edit_error("bot-write-failed", format!("Bot 写入失败：{error}")))?;
        Ok(bot)
    })).await {
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
        .filter(|(other_id, node)| other_id.as_str() != id && (node.0.blocked_by.iter().any(|parent| parent == id)
            || node.0.input_rule.iter().any(|input| matches!(input, InputRule::Node { node_id, .. } if node_id == id))))
        .map(|(id, node)| format!("{}（{id}）", node.0.title)).collect();
    if !dependents.is_empty() {
        return Err(edit_error("node-referenced", format!("阻止删除：以下 node 的 blocked_by 或命名输入引用此 node：{}。不会级联删除或清除依赖。", dependents.join("、"))));
    }
    Ok(())
}

pub async fn check_delete_node(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(identity): Json<BotIdentity>,
) -> Response {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        deletable_node(&parse_map(&original)?, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": { "file": name, "original": original } }))
    })).await {
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let check = || -> Result<WorkflowMap, DirectoryError> {
            let original = read_map_source(access, &state.map_directory, &name)?;
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
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check().map(|_| ()),
            |error| edit_error("node-write-failed", format!("node 删除写入失败：{error}")))?;
        Ok(map)
    })).await {
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        parse_map(&original)?;
        let mut example: serde_json::Value = serde_json::from_str(include_str!("../../../docs/example/maps/trip-readiness.json")).expect("bundled Map example");
        let mut content = example["nodes"]["A"].take();
        content["report_policy"] = serde_json::json!(ReportPolicy::default());
        content["auto_close"] = serde_json::json!(false);
        Ok::<_, DirectoryError>(serde_json::json!({ "id": "", "content": content, "revision": { "file": name, "original": original }, "create": true }))
    })).await {
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        let node = map.nodes.get(&identity.id).ok_or_else(|| edit_error("node-conflict", "node 已移除；请重新加载。"))?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": node, "revision": { "file": name, "original": original }, "create": false }))
    })).await {
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
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        if draft.id.is_empty() { return Err(edit_error("node-invalid", "node ID 不能为空。")); }
        let content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("node-invalid", format!("node JSON 无效：{e}")))?;
        let object = content.as_object().ok_or_else(|| edit_error("node-invalid", "node 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("node-invalid", "请移除 node 内容中的顶层 id；使用独立 node ID。")); }
        let node: JsonObject<MapNode> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("node-invalid", format!("node 结构无效：{e}")))?;
        let check = || -> Result<(), DirectoryError> {
            let current = read_map_source(access, &state.map_directory, &name)
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
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check().map(|_| ()),
            |error| edit_error("node-write-failed", format!("node 写入失败：{error}")))?;
        Ok(map)
    })).await {
        Ok(Ok(map)) => Json(map).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("node-write-failed", error.to_string()).into_response(),
    }
}

#[derive(Serialize)]
struct MapFiles { files: Vec<String> }

#[derive(Serialize)]
struct MapCatalog {
    files: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    titles: BTreeMap<String, String>,
}

fn read_map_catalog(access: &FileAccess, directory: &Path) -> Result<MapCatalog, DirectoryError> {
    let files = read_files(access, directory)?.files;
    let titles = files.iter().filter_map(|name| {
        let map = read_map(access, directory, name).ok()?;
        if map.title.trim().is_empty() { return None; }
        Some((name.clone(), map.title))
    }).collect();
    Ok(MapCatalog { files, titles })
}

struct DirectoryError { code: &'static str, message: String }
impl DirectoryError {
    fn io(code: &'static str, action: &str, error: std::io::Error) -> Self {
        Self { code, message: format!("Map 目录{action}失败：{error}") }
    }
}
impl IntoResponse for DirectoryError {
    fn into_response(self) -> Response {
        let status = match self.code {
            "map-input-invalid" => StatusCode::BAD_REQUEST,
            "map-conflict" | "bot-conflict" => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({
            "code": self.code, "message": self.message,
        }))).into_response()
    }
}

// Inspect every existing ancestor before creating any missing component. Never
// follow symlinks, junctions or other reparse points, even above the configured root.
fn prepare_directory(access: &FileAccess, directory: &Path) -> Result<PathBuf, DirectoryError> {
    let absolute = if directory.is_absolute() { directory.to_owned() } else {
        std::env::current_dir().map_err(|e| DirectoryError::io("map-directory-create-failed", "创建", e))?.join(directory)
    };
    let mut checked = PathBuf::new();
    for component in absolute.components() {
        checked.push(component);
        let metadata = match access.symlink_metadata(&checked) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Map and Bot catalog reads can create a shared missing parent
                // concurrently. Recheck the winner's entry below, including the
                // link and directory checks; AlreadyExists alone is not success.
                match access.create_dir(&checked) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(DirectoryError::io("map-directory-create-failed", "创建", error)),
                }
                access.symlink_metadata(&checked).map_err(|e| DirectoryError::io("map-directory-create-failed", "检查", e))?
            }
            Err(error) => return Err(DirectoryError::io("map-directory-create-failed", "检查", error)),
        };
        if crate::zones::browse::is_link_like(&metadata) {
            return Err(DirectoryError { code: "map-directory-link-rejected", message: "Map 目录不能经过符号链接、目录联接或重解析点。".into() });
        }
        if !metadata.is_dir() {
            return Err(DirectoryError { code: "map-directory-create-failed", message: "Map 目录创建失败：配置位置或其父路径不是目录。".into() });
        }
    }
    Ok(absolute)
}

fn read_files(access: &FileAccess, directory: &Path) -> Result<MapFiles, DirectoryError> {
    let root = prepare_directory(access, directory)?;
    let entries = access.read_dir(root).map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.path();
        let metadata = access.symlink_metadata(&path).map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
        if !metadata.is_file() || crate::zones::browse::is_link_like(&metadata) { continue; }
        if path.extension().is_some_and(|value| value.eq_ignore_ascii_case("json")) {
            if let Some(name) = entry.file_name().to_str() {
                let modified = metadata.modified().map_err(|e| DirectoryError::io("map-directory-read-failed", "读取", e))?;
                files.push((name.to_owned(), modified));
            }
        }
    }
    files.sort_by(|(left_name, left_time), (right_name, right_time)| {
        right_time.cmp(left_time).then_with(|| left_name.cmp(right_name))
    });
    Ok(MapFiles { files: files.into_iter().map(|(name, _)| name).collect() })
}

fn check_map_references(access: &FileAccess, directory: &Path, target: &str) -> Result<(), DirectoryError> {
    let root = prepare_directory(access, directory).map_err(|error| edit_error("map-reference-check-failed", error.message))?;
    let entries = access.read_dir(&root).map_err(|error| edit_error("map-reference-check-failed", format!("无法检查 Bot 目录 {}：{error}", root.display())))?;
    let mut references = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json")) { continue; }
        let check = || -> Result<Bot, String> {
            let metadata = access.symlink_metadata(&path).map_err(|error| error.to_string())?;
            if !metadata.is_file() || crate::zones::browse::is_link_like(&metadata) {
                return Err("定义不能是链接、重解析点或目录".into());
            }
            let bytes = access.read(&path).map_err(|error| error.to_string())?;
            let JsonObject(bot): JsonObject<Bot> = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            bot.validate_target().map_err(|error| error.message)?;
            Ok(bot)
        };
        let bot = check().map_err(|error| edit_error("map-reference-check-failed", format!("无法可靠检查 Bot {}，禁止删除 Map {target}：{error}", path.display())))?;
        if bot.map_file.as_deref() == Some(target) {
            references.push(format!("Bot {} / {}", bot.id, path.display()));
        }
    }
    if !references.is_empty() {
        return Err(edit_error("map-referenced", format!("Map {target} 仍被引用，不能删除：{}", references.join("；"))));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MapDelete {
    delete_history: bool,
}

pub async fn delete_map(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    request: Request,
) -> Response {
    let input: MapDelete = match crate::chat_command_body(request, "删除 Map").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let lock = state.runs.map_lock(&name).await;
    let _operation = lock.lock().await;
    // Validate the file boundary without parsing its definition: broken JSON is
    // still a safely identifiable Map file which the user must be able to remove.
    let validation = with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-delete-failed", |access| {
        map_file_path(access, &state.map_directory, &name)?;
        check_map_references(access, &state.bot_directory, &name)?;
        let files = read_files(access, &state.map_directory)?;
        if !files.files.contains(&name) {
            return Err(edit_error("map-delete-failed", "删除只接受列表中的确切 Map 文件名。"));
        }
        Ok(())
    });
    if let Err(error) = validation { return error.into_response(); }
    if input.delete_history {
        if let Err(message) = state.runs.delete_map_history(&state, &name).await {
            return edit_error("map-history-delete-failed", format!("Map 文件尚未删除；Run 历史删除未完成（部分历史可能已删除）：{message}")).into_response();
        }
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-delete-failed", |access| {
        let path = map_file_path(access, &state.map_directory, &name)?;
        check_map_references(access, &state.bot_directory, &name)?;
        access.remove_file(path).map_err(|error| edit_error("map-delete-failed", format!("Map {name} 文件删除失败：{error}；所选 Run 历史处理已完成。")))?;
        read_map_catalog(access, &state.map_directory).map_err(|error| edit_error("map-list-after-delete-failed", format!("Map {name} 已删除，但剩余 Map 列表读取失败：{}", error.message)))
    })).await {
        Ok(Ok(files)) => Json(files).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => edit_error("map-delete-failed", format!("Map 删除任务失败：{error}；所选 Run 历史处理已完成，请重新加载确认文件状态。" )).into_response(),
    }
}

pub async fn list_maps(axum::extract::State(state): axum::extract::State<crate::AppState>, request: Request) -> Response {
    if request.uri().query().is_some() || axum::body::to_bytes(request.into_body(), 0).await.is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "code": "invalid-request", "message": "Map 列表不接受路径、查询参数或请求体。",
        }))).into_response();
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-directory-read-failed", |access| read_map_catalog(access, &state.map_directory))).await {
        Ok(Ok(files)) => Json(files).into_response(),
        Ok(Err(error)) => error.into_response(),
        Err(error) => DirectoryError { code: "map-directory-read-failed", message: format!("Map 目录读取失败：{error}") }.into_response(),
    }
}
