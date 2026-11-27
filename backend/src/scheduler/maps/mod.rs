//! Map definitions, graph validation and atomic Map/node editing.
use super::{DirectoryError, DefinitionRevision, JsonObject, edit_error, object_array, prepare_directory, read_files, with_definitions, check_map_references};
use super::bots::read_bots;
use crate::files::{FileAccess, PublishMode};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, VecDeque}, path::{Path, PathBuf}};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIdentity { id: String }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkflowMap {
    pub(crate) title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) group: Option<String>,
    pub(super) description: String,
    #[serde(default, deserialize_with = "present_object", skip_serializing_if = "Option::is_none")]
    pub(crate) map_input_rule: Option<MapRule<MapFileInput>>,
    #[serde(default, deserialize_with = "present_object", skip_serializing_if = "Option::is_none")]
    pub(crate) map_output_rule: Option<MapRule<MapFileOutput>>,
    pub(crate) nodes: BTreeMap<String, JsonObject<MapNode>>,
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
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRule { pub id: String }
impl WorkflowMap {
    pub(crate) fn input_files(&self) -> &[MapFileInput] {
        self.map_input_rule.as_ref().map_or(&[], |rule| rule.files.as_slice())
    }
}

// Default applies to an omitted field only; a present rule is a named object.
fn present_object<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<T>, D::Error> {
    JsonObject::<T>::deserialize(deserializer).map(|value| Some(value.0))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, bound(deserialize = "F: Deserialize<'de>"))]
pub(crate) struct MapRule<F> {
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default, deserialize_with = "object_array")]
    pub(crate) files: Vec<F>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapFileInput {
    pub id: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MapFileOutput {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFile { pub id: String, pub path: String }

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum NodeZone {
    Fixed(String),
    Dynamic(JsonObject<DynamicZone>),
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DynamicZone {
    #[serde(rename = "type")]
    kind: DynamicZoneType,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum DynamicZoneType { Dynamic }
impl NodeZone {
    pub(crate) fn fixed(&self) -> Option<&str> {
        match self { Self::Fixed(id) => Some(id), Self::Dynamic(_) => None }
    }
}

pub(crate) fn default_auto_approved() -> bool { true }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MapNode {
    pub(crate) title: String,
    pub(crate) bot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) manager_task_id: Option<String>,
    pub(super) input: String,
    #[serde(default)]
    pub(crate) input_rule: Vec<InputRule>,
    #[serde(default, deserialize_with = "object_array")]
    pub(super) output_rule: Vec<OutputRule>,
    #[serde(default = "default_auto_approved")]
    pub(crate) auto_approved: bool,
    #[serde(default)]
    pub(crate) auto_close: bool,
    #[serde(default)]
    pub(crate) end_node: bool,
    pub(crate) zone: NodeZone,
    pub(crate) blocked_by: Vec<String>,
}

pub(super) fn map_error(message: impl Into<String>) -> DirectoryError {
    DirectoryError { code: "map-read-failed", message: message.into() }
}

pub(super) fn map_file_path(access: &FileAccess, directory: &Path, name: &str) -> Result<PathBuf, DirectoryError> {
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

pub(crate) fn read_map_source(access: &FileAccess, directory: &Path, name: &str) -> Result<String, DirectoryError> {
    access.read_to_string(map_file_path(access, directory, name)?)
        .map_err(|e| map_error(format!("Map 文件读取失败：{e}")))
}

pub(crate) fn read_map(access: &FileAccess, directory: &Path, name: &str) -> Result<WorkflowMap, DirectoryError> {
    parse_map(&read_map_source(access, directory, name)?)
}

pub(crate) fn parse_map(source: &str) -> Result<WorkflowMap, DirectoryError> {
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
    if !crate::tasks::valid_group(map.group.as_deref()) {
        return Err(map_error("分组名称不能为空或包含首尾空白。"));
    }
    let mut ends = map.nodes.iter().filter(|(_, node)| node.0.end_node);
    let end = ends.next();
    if ends.next().is_some() { return Err(map_error("Map 最多允许一个 end_node。")); }
    if let Some((id, _)) = end {
        if map.nodes.values().any(|node| node.0.blocked_by.contains(id) || node.0.input_rule.iter().any(|rule| matches!(rule, InputRule::Node { node_id, .. } if node_id == id))) {
            return Err(map_error("end_node 不允许下游节点或输出引用。"));
        }
    }
    let mut public_inputs = std::collections::BTreeSet::new();
    for input in map.input_files() {
        if input.id.trim().is_empty() || !public_inputs.insert(input.id.as_str()) {
            return Err(map_error(format!("Map map_input_rule.files ID 为空或重复：{}", input.id)));
        }
    }
    if let Some(rule) = &map.map_output_rule {
        let mut public_outputs = std::collections::BTreeSet::new();
        for output in &rule.files {
            if output.id.trim().is_empty() || !public_outputs.insert(&output.id) {
                return Err(map_error(format!("Map map_output_rule.files ID 为空或重复：{}", output.id)));
            }
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
                    if !public_inputs.contains(input_id.as_str()) {
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

pub(crate) async fn get_map(state: crate::AppState, name: String) -> Result<WorkflowMap, DirectoryError> {

    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        read_map(access, &state.map_directory, &name).map_err(|mut error| {
            error.message = format!("Map 文件 {name}：{}", error.message);
            error
        })
    })).await {
        Ok(Ok(map)) => Ok(map),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(map_error(format!("Map 文件读取失败：{error}"))),
    }
}

pub(crate) async fn edit_map_rules(state: crate::AppState, name: String) -> Result<MapRulesEdit, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        Ok::<_, DirectoryError>(MapRulesEdit {
            map_input_rule: map.map_input_rule,
            map_output_rule: map.map_output_rule,
            revision: DefinitionRevision { file: name, original },
        })
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(map_error(error.to_string())),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MapRulesEdit {
    #[serde(default, deserialize_with = "present_object", skip_serializing_if = "Option::is_none")]
    map_input_rule: Option<MapRule<MapFileInput>>,
    #[serde(default, deserialize_with = "present_object", skip_serializing_if = "Option::is_none")]
    map_output_rule: Option<MapRule<MapFileOutput>>,
    revision: DefinitionRevision,
}

pub(crate) async fn save_map_rules(state: crate::AppState, name: String, draft: MapRulesEdit) -> Result<WorkflowMap, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-write-failed", |access| {
        let check = || -> Result<(), DirectoryError> {
            let current = read_map_source(access, &state.map_directory, &name)?;
            if draft.revision.file != name || draft.revision.original != current {
                return Err(edit_error("map-conflict", "Map 文件已变化；请重新加载后再编辑公开规则。"));
            }
            Ok(())
        };
        check()?;
        let mut map = parse_map(&draft.revision.original)?;
        map.map_input_rule = draft.map_input_rule;
        map.map_output_rule = draft.map_output_rule;
        validate_map(&map).map_err(|error| edit_error("map-input-invalid", error.message))?;
        let bytes = serde_json::to_vec_pretty(&map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check(), |error| edit_error("map-write-failed", format!("Map 公开规则写入失败：{error}")))?;
        Ok(map)
    })).await {
        Ok(Ok(map)) => Ok(map),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("map-write-failed", error.to_string())),
    }
}

pub(crate) async fn edit_map_title(state: crate::AppState, name: String) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        Ok::<_, DirectoryError>(serde_json::json!({"title": map.title, "revision": {"file": name, "original": original}}))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(map_error(error.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapTitleSave {
    title: String,
    revision: DefinitionRevision,
}

pub(crate) async fn save_map_title(state: crate::AppState, name: String, draft: MapTitleSave) -> Result<serde_json::Value, DirectoryError> {
    let title = draft.title.trim().to_owned();
    if title.is_empty() {
        return Err(edit_error("map-title-invalid", "Map 显示名称不能为空。"));
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-write-failed", |access| {
        let check = || -> Result<(), DirectoryError> {
            let current = read_map_source(access, &state.map_directory, &name)?;
            if draft.revision.file != name || draft.revision.original != current {
                return Err(edit_error("map-conflict", "Map 文件已变化；请重新加载后再编辑名称。"));
            }
            Ok(())
        };
        check()?;
        parse_map(&draft.revision.original)?;
        // Preserve every other field exactly as represented, including omitted defaults.
        let mut map: serde_json::Value = serde_json::from_str(&draft.revision.original)
            .map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        map["title"] = serde_json::Value::String(title);
        let bytes = serde_json::to_vec_pretty(&map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check(), |error| edit_error("map-write-failed", format!("Map 名称写入失败：{error}")))?;
        Ok(map)
    })).await {
        Ok(Ok(map)) => Ok(map),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("map-write-failed", error.to_string())),
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

pub(crate) async fn check_delete_node(state: crate::AppState, name: String, identity: NodeIdentity) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        deletable_node(&parse_map(&original)?, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": { "file": name, "original": original } }))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("node-read-failed", error.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeDelete {
    id: String,
    revision: DefinitionRevision,
}

pub(crate) async fn delete_node(state: crate::AppState, name: String, target: NodeDelete) -> Result<WorkflowMap, DirectoryError> {
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
        Ok(Ok(map)) => Ok(map),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("node-write-failed", error.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSave {
    id: String,
    content: String,
    revision: DefinitionRevision,
    create: bool,
}

pub(crate) async fn new_node(state: crate::AppState, name: String) -> Result<serde_json::Value, DirectoryError> {

    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        parse_map(&original)?;
        let mut example: serde_json::Value = serde_json::from_str(include_str!("../../../../docs/example/maps/trip-readiness.json")).expect("bundled Map example");
        let mut content = example["nodes"]["A"].take();
        content["auto_approved"] = serde_json::json!(true);
        content["auto_close"] = serde_json::json!(false);
        content["end_node"] = serde_json::json!(false);
        Ok::<_, DirectoryError>(serde_json::json!({ "id": "", "content": content, "revision": { "file": name, "original": original }, "create": true }))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("node-read-failed", error.to_string())),
    }
}

pub(crate) async fn edit_node(state: crate::AppState, name: String, identity: NodeIdentity) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "node-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        let node = map.nodes.get(&identity.id).ok_or_else(|| edit_error("node-conflict", "node 已移除；请重新加载。"))?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": node, "revision": { "file": name, "original": original }, "create": false }))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("node-read-failed", error.to_string())),
    }
}

fn check_child_interface(child: &WorkflowMap, target: &str, node: &MapNode) -> Result<(), DirectoryError> {
    for input in &node.input_rule {
        let id = input.id();
        if !child.input_files().iter().any(|file| file.id == id) {
            return Err(edit_error("node-map-input-unknown", format!("子 Map {target} 未声明同名公开输入 ID：{id}")));
        }
    }
    let outputs = child.map_output_rule.as_ref().map_or(&[][..], |rule| rule.files.as_slice());
    for output in &node.output_rule {
        if !outputs.iter().any(|file| file.id == output.id) {
            return Err(edit_error("node-map-output-unknown", format!("子 Map {target} 未声明同名公开输出 ID：{}", output.id)));
        }
    }
    Ok(())
}

pub(crate) async fn save_node(state: crate::AppState, name: String, draft: NodeSave) -> Result<WorkflowMap, DirectoryError> {
    if draft.id.is_empty() { return Err(edit_error("node-invalid", "node ID 不能为空。")); }
    let content: serde_json::Value = serde_json::from_str(&draft.content)
        .map_err(|e| edit_error("node-invalid", format!("node JSON 无效：{e}")))?;
    let object = content.as_object().ok_or_else(|| edit_error("node-invalid", "node 内容必须为 JSON 对象。"))?;
    if object.contains_key("id") { return Err(edit_error("node-invalid", "请移除 node 内容中的顶层 id；使用独立 node ID。")); }
    let node: JsonObject<MapNode> = serde_path_to_error::deserialize(content)
        .map_err(|e| edit_error("node-invalid", format!("node 结构无效：{e}")))?;
    let tasks = if node.0.end_node {
        state.tasks.list().await.map_err(|e| edit_error("end-node-target-invalid", e.message()))?
    } else { Vec::new() };
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "node-write-failed", |access| {
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
        let definition = &node.0;
        let has_bindings = !definition.input_rule.is_empty() || !definition.output_rule.is_empty();
        let checks_script_policy = definition.manager_task_id.is_some() || !definition.auto_approved || definition.auto_close;
        if definition.end_node || has_bindings || checks_script_policy {
            let bots = read_bots(access, &state.bot_directory)?;
            let bot = bots.bots.iter().find(|bot| bot.id == definition.bot_id);
            if definition.end_node {
                let bot = bot.ok_or_else(|| edit_error("end-node-target-invalid", "end_node 的 Bot 缺失。"))?;
                if bot.script.is_some() || bot.map_file.is_some() || definition.manager_task_id.is_some()
                    || !tasks.iter().any(|task| bot.task_id.as_deref() == Some(task.id.as_str()) && task.task_type == crate::tasks::TaskType::Normal) {
                    return Err(edit_error("end-node-target-invalid", "end_node 仅允许普通 Task Bot。"));
                }
            }
            if checks_script_policy && bot.is_some_and(|bot| bot.script.is_some()) {
                return Err(edit_error("script-policy-invalid", "脚本节点仅允许 auto_approved:true、auto_close:false，且无 manager_task_id。"));
            }
            if has_bindings && let Some(target) = bot.and_then(|bot| bot.map_file.as_deref()) {
                let child = read_map(access, &state.map_directory, target)
                    .map_err(|error| edit_error(error.code, format!("子 Map {target} 无法读取：{}", error.message)))?;
                check_child_interface(&child, target, definition)?;
            }
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
        Ok(Ok(map)) => Ok(map),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("node-write-failed", error.to_string())),
    }
}

#[derive(Serialize)]
pub(crate) struct MapCatalog {
    files: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    titles: BTreeMap<String, String>,
    groups: BTreeMap<String, String>,
}

fn read_map_catalog(access: &FileAccess, directory: &Path) -> Result<MapCatalog, DirectoryError> {
    let files = read_files(access, directory)?.files;
    let mut titles = BTreeMap::new();
    let mut groups = BTreeMap::new();
    for name in &files {
        if let Ok(map) = read_map(access, directory, name) {
            if !map.title.trim().is_empty() { titles.insert(name.clone(), map.title); }
            if let Some(group) = map.group { groups.insert(name.clone(), group); }
        }
    }
    Ok(MapCatalog { files, titles, groups })
}
pub(crate) async fn delete_map(state: crate::AppState, name: String) -> Result<MapCatalog, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-delete-failed", |access| {
        // Broken JSON can still be removed if it is an identifiable Map file.
        let path = map_file_path(access, &state.map_directory, &name)?;
        check_map_references(access, &state.bot_directory, &name)?;
        if !read_files(access, &state.map_directory)?.files.contains(&name) {
            return Err(edit_error("map-delete-failed", "删除只接受列表中的确切 Map 文件名。"));
        }
        access.remove_file(path).map_err(|error| edit_error("map-delete-failed", format!("Map {name} 文件删除失败：{error}")))?;
        read_map_catalog(access, &state.map_directory).map_err(|error| edit_error("map-list-after-delete-failed", format!("Map {name} 已删除，但剩余 Map 列表读取失败：{}", error.message)))
    })).await {
        Ok(result) => result,
        Err(error) => Err(edit_error("map-delete-failed", format!("Map 删除任务失败：{error}；请重新加载确认文件状态。"))),
    }
}

pub(crate) async fn set_map_group(state: crate::AppState, name: String, input: crate::tasks::GroupInput) -> Result<serde_json::Value, DirectoryError> {
    if !crate::tasks::valid_group(input.group.as_deref()) {
        return Err(edit_error("group-invalid", "分组名称不能为空或包含首尾空白。"));
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-write-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let mut map = parse_map(&original)?;
        map.group = input.group;
        let bytes = serde_json::to_vec_pretty(&map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace, |_| {
            if read_map_source(access, &state.map_directory, &name)? != original {
                return Err(edit_error("map-conflict", "Map 文件已变化；请重新加载。"));
            }
            Ok(())
        }, |error| edit_error("map-write-failed", error.to_string()))?;
        let mut value = serde_json::to_value(map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        value["revision"] = serde_json::json!({"file":name,"original":String::from_utf8(bytes).expect("JSON is UTF-8")});
        Ok(value)
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("map-write-failed", error.to_string())),
    }
}
pub(crate) async fn list_maps(state: crate::AppState) -> Result<MapCatalog, DirectoryError> {

    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-directory-read-failed", |access| read_map_catalog(access, &state.map_directory))).await {
        Ok(Ok(files)) => Ok(files),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(DirectoryError { code: "map-directory-read-failed", message: format!("Map 目录读取失败：{error}") }),
    }
}
