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
    pub(super) title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) group: Option<String>,
    pub(super) description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(super) input_requirements: String,
    #[serde(default, deserialize_with = "object_array", skip_serializing_if = "Vec::is_empty")]
    pub(crate) input_files: Vec<MapFileInput>,
    pub(crate) nodes: BTreeMap<String, JsonObject<MapNode>>,
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
pub struct MapFileInput {
    pub id: String,
    pub required: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFile { pub id: String, pub path: String }

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MapNode {
    pub(super) title: String,
    pub(crate) bot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) manager_task_id: Option<String>,
    pub(super) input: String,
    #[serde(default)]
    pub(crate) input_rule: Vec<InputRule>,
    #[serde(default, deserialize_with = "object_array")]
    pub(super) output_rule: Vec<OutputRule>,
    #[serde(default)]
    pub(crate) report_policy: ReportPolicy,
    #[serde(default)]
    pub(crate) auto_close: bool,
    pub(crate) zone: String,
    pub(super) blocked_by: Vec<String>,
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

fn read_map_source(access: &FileAccess, directory: &Path, name: &str) -> Result<String, DirectoryError> {
    access.read_to_string(map_file_path(access, directory, name)?)
        .map_err(|e| map_error(format!("Map 文件读取失败：{e}")))
}

pub(crate) fn read_map(access: &FileAccess, directory: &Path, name: &str) -> Result<WorkflowMap, DirectoryError> {
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
    if !crate::tasks::valid_group(map.group.as_deref()) {
        return Err(map_error("分组名称不能为空或包含首尾空白。"));
    }
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

pub(crate) async fn edit_map_inputs(state: crate::AppState, name: String) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
        let original = read_map_source(access, &state.map_directory, &name)?;
        let map = parse_map(&original)?;
        Ok::<_, DirectoryError>(serde_json::json!({"input_files": map.input_files, "input_requirements": map.input_requirements, "revision": {"file": name, "original": original}}))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(map_error(error.to_string())),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapInputsSave {
    input_requirements: String,
    #[serde(deserialize_with = "object_array")]
    input_files: Vec<MapFileInput>,
    revision: DefinitionRevision,
}

pub(crate) async fn save_map_inputs(state: crate::AppState, name: String, draft: MapInputsSave) -> Result<WorkflowMap, DirectoryError> {
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
        map.input_requirements = draft.input_requirements;
        validate_map(&map).map_err(|error| edit_error("map-input-invalid", error.message))?;
        let bytes = serde_json::to_vec_pretty(&map).map_err(|error| edit_error("map-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.map_directory)?;
        access.publish_checked(&root.join(&name), &bytes, PublishMode::Replace,
            |_| check(), |error| edit_error("map-write-failed", format!("Map 输入声明写入失败：{error}")))?;
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
        content["report_policy"] = serde_json::json!(ReportPolicy::default());
        content["auto_close"] = serde_json::json!(false);
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

pub(crate) async fn save_node(state: crate::AppState, name: String, draft: NodeSave) -> Result<WorkflowMap, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "node-write-failed", |access| {
        if draft.id.is_empty() { return Err(edit_error("node-invalid", "node ID 不能为空。")); }
        let content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("node-invalid", format!("node JSON 无效：{e}")))?;
        let object = content.as_object().ok_or_else(|| edit_error("node-invalid", "node 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("node-invalid", "请移除 node 内容中的顶层 id；使用独立 node ID。")); }
        let node: JsonObject<MapNode> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("node-invalid", format!("node 结构无效：{e}")))?;
        if node.0.manager_task_id.is_some() || node.0.report_policy != ReportPolicy::Auto || node.0.auto_close {
            let bots = read_bots(access, &state.bot_directory)?;
            if bots.bots.iter().any(|bot| bot.id == node.0.bot_id && bot.rust_script.is_some()) {
                return Err(edit_error("script-policy-invalid", "脚本节点仅允许 report_policy:auto、auto_close:false，且无 manager_task_id。"));
            }
        }
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
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MapDelete {
    delete_history: bool,
}

pub(crate) async fn delete_map(state: crate::AppState, name: String, input: MapDelete) -> Result<MapCatalog, DirectoryError> {
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
    validation?;
    if input.delete_history {
        if let Err(message) = state.runs.delete_map_history(&state, &name).await {
            return Err(edit_error("map-history-delete-failed", format!("Map 文件尚未删除；Run 历史删除未完成（部分历史可能已删除）：{message}")));
        }
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-delete-failed", |access| {
        let path = map_file_path(access, &state.map_directory, &name)?;
        check_map_references(access, &state.bot_directory, &name)?;
        access.remove_file(path).map_err(|error| edit_error("map-delete-failed", format!("Map {name} 文件删除失败：{error}；所选 Run 历史处理已完成。")))?;
        read_map_catalog(access, &state.map_directory).map_err(|error| edit_error("map-list-after-delete-failed", format!("Map {name} 已删除，但剩余 Map 列表读取失败：{}", error.message)))
    })).await {
        Ok(Ok(files)) => Ok(files),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("map-delete-failed", format!("Map 删除任务失败：{error}；所选 Run 历史处理已完成，请重新加载确认文件状态。" ))),
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
