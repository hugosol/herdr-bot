//! Cross-definition dispatch, delegation snapshots and shared file-access scope.
pub(crate) mod bots;
pub(crate) mod maps;
use bots::{Bot, read_bots};
use maps::{InputRule, OutputRule, read_map, map_error};
use crate::files::{FileAccess, FileStore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::{Path, PathBuf}};

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
pub(crate) struct DispatchTarget {
    pub title: String,
    pub input: String,
    pub input_rule: Vec<InputRule>,
    pub output_rule: Vec<OutputRule>,
    pub auto_approved: bool,
    pub auto_close: bool,
    pub end_node: bool,
    pub zone: String,
    pub zone_id: String,
    pub task: Option<crate::tasks::Task>,
    pub map: Option<String>,
    pub script: Option<String>,
}

pub(crate) async fn dispatch_target(state: &crate::AppState, run: &crate::runs::Run, id: &str, resolved_zone: Option<(String, String)>) -> Result<DispatchTarget, String> {
    let map = &run.map;
    let (node, bots) = with_definitions(
        &[state.map_directory.clone(), state.bot_directory.clone()], "map-read-failed", |access| {
            let mut definition = read_map(access, &state.map_directory, map)?;
            let node = definition.nodes.remove(id).ok_or_else(|| map_error("node 不在当前 Map 中"))?.0;
            Ok((node, read_bots(access, &state.bot_directory)?))
        },
    ).map_err(|e| e.message)?;
    let (zone_id, zone) = match resolved_zone {
        Some(zone) => zone,
        None => dispatch_zone(state, run, &node.zone, None).await?,
    };
    let bot = bots.bots.iter().find(|b| b.id == node.bot_id).ok_or("node 的 Bot 缺失或不唯一")?;
    if node.end_node && (bot.script.is_some() || bot.map_file.is_some() || bot.task_id.is_none() || node.manager_task_id.is_some()) {
        return Err("end_node 仅允许普通 Task Bot。".into());
    }
    if node.end_node && run.nodes.iter().any(|(other, state)| other != id && state.status != "completed") {
        return Err("end_node 必须等待所有其他节点 completed。".into());
    }
    if let Some(script) = &bot.script {
        if node.manager_task_id.is_some() || !node.auto_approved || node.auto_close {
            return Err("脚本节点仅允许自动批准、无自动关闭且不指定 manager Task。".into());
        }
        return Ok(DispatchTarget { title: node.title, input: node.input, input_rule: node.input_rule, output_rule: node.output_rule, auto_approved: node.auto_approved, auto_close: false, end_node: false, zone_id, zone, task: None, map: None, script: Some(script.clone()) });
    }
    let (task_id, required_role) = match (bot.task_id.as_deref(), bot.map_file.as_deref()) {
        (Some(task), None) if node.manager_task_id.is_none() => (task, crate::tasks::TaskType::Normal),
        (None, Some(_)) => (node.manager_task_id.as_deref().ok_or("Map 型 node 缺少 manager_task_id")?, crate::tasks::TaskType::Manager),
        _ => return Err("Bot 执行目标或节点 manager 配置无效".into()),
    };
    let task = state.tasks.get(task_id).await.map_err(|e| e.message())?;
    if task.task_type != required_role {
        return Err("node 的执行 Task 类型与目标不一致".into());
    }
    Ok(DispatchTarget { title: node.title, input: node.input, input_rule: node.input_rule, output_rule: node.output_rule, auto_approved: node.auto_approved, auto_close: node.auto_close && !node.end_node, end_node: node.end_node, zone_id, zone, task: Some(task), map: bot.map_file.clone(), script: None })
}

async fn dispatch_zone(state: &crate::AppState, run: &crate::runs::Run, declaration: &maps::NodeZone, selected_zone: Option<&str>) -> Result<(String, String), String> {
    let (zone_id, zone) = match declaration.fixed() {
        Some(id) => {
            if selected_zone.is_some() { return Err(format!("固定 zone node {id} 不接受 zone 覆盖")); }
            let root = run.zone_bindings.get(id).ok_or_else(|| format!("zone {id} 不在本次 Run 启动快照中；请启动新 Run"))?;
            (id.to_owned(), root.clone())
        }
        None => {
            let id = selected_zone.filter(|id| !id.trim().is_empty()).ok_or("动态 zone node 必须显式选择已登记的 zone ID")?;
            let zones = state.zones.list().await.map_err(|error| error.message())?;
            let root = zones.iter().find(|zone| zone.id == id).ok_or_else(|| format!("zone {id} 未登记"))?;
            (id.to_owned(), root.path.clone())
        }
    };
    if declaration.fixed().is_none() {
        crate::zones::browse::checked_root(&zone).map_err(|error| error.message())?;
    }
    Ok((zone_id, zone))
}

pub(crate) async fn validate_zone_selections(state: &crate::AppState, run: &crate::runs::Run, selected: &[String], overrides: &BTreeMap<String, String>) -> Result<BTreeMap<String, (String, String)>, String> {
    let map = with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| read_map(access, &state.map_directory, &run.map)).map_err(|error| error.message)?;
    for id in overrides.keys() {
        if !selected.contains(id) || !run.nodes.contains_key(id) || !map.nodes.contains_key(id) {
            return Err(format!("zoneIds 的 node {id} 未选择或不在本次 Run 中"));
        }
    }
    let mut resolved = BTreeMap::new();
    for id in selected {
        if let Some(node) = map.nodes.get(id) {
            if overrides.contains_key(id) || node.0.zone.fixed().is_none() {
                let zone = dispatch_zone(state, run, &node.0.zone, overrides.get(id).map(String::as_str)).await?;
                resolved.insert(id.clone(), zone);
            }
        }
    }
    Ok(resolved)
}
/// Selected definition closure for chief confirmation; unrelated catalog edits
/// must not invalidate approval. Preflight owns cycle/role/binding validation.
pub(crate) struct DelegationMapDefinition {
    pub definitions: serde_json::Value,
    pub task_ids: std::collections::BTreeSet<String>,
    pub zone_ids: std::collections::BTreeSet<String>,
    pub files: Vec<(String, String)>,
}

pub(crate) enum DelegationDefinitionError {
    Directory(DirectoryError),
    Prompt(String),
}

pub(crate) fn delegation_map_definition(
    state: &crate::AppState, file: &str, manager_zone: &str,
) -> Result<DelegationMapDefinition, DelegationDefinitionError> {
    let prompts = state.prompts.snapshot().map_err(|error| DelegationDefinitionError::Prompt(error.to_string()))?;
    with_definitions(&[state.map_directory.clone(), state.bot_directory.clone()], "map-read-failed", |access| {
        let catalog = read_bots(access, &state.bot_directory)?;
        let mut maps = BTreeMap::new();
        let mut bots = BTreeMap::new();
        let mut rendered = BTreeMap::new();
        let mut task_ids = std::collections::BTreeSet::new();
        let mut zone_ids = std::collections::BTreeSet::from([manager_zone.to_owned()]);
        let mut files = Vec::new();
        let mut pending = vec![(file.to_owned(), Some(manager_zone.to_owned()))];
        let mut visited = std::collections::BTreeSet::new();
        while let Some((file, manager_zone)) = pending.pop() {
            if !visited.insert((file.clone(), manager_zone.clone())) { continue; }
            let map = read_map(access, &state.map_directory, &file)?;
            let mut node_inputs = BTreeMap::new();
            for (id, node) in &map.nodes {
                node_inputs.insert(id.clone(), prompts.render(crate::prompts::Field::NodeInput, &node.0.input)
                    .map_err(|error| edit_error("prompt-dictionary-invalid", error.to_string()))?);
            }
            rendered.insert(file.clone(), serde_json::json!({
                "description":prompts.render(crate::prompts::Field::MapDescription, &map.description)
                    .map_err(|error| edit_error("prompt-dictionary-invalid", error.to_string()))?,
                "nodeInputs":node_inputs
            }));
            for node in map.nodes.values().map(|node| &node.0) {
                if let Some(zone) = node.zone.fixed() { zone_ids.insert(zone.to_owned()); }
                let bot = catalog.bots.iter().find(|bot| bot.id == node.bot_id)
                    .ok_or_else(|| edit_error("bot-not-found", "Map 的 Bot 已变化；请重新预览并确认。"))?;
                let mut value = serde_json::to_value(bot).expect("Bot serialization");
                let mut meanings = BTreeMap::new();
                for (key, field, source) in [
                    ("when", crate::prompts::Field::BotWhen, &bot.when),
                    ("description", crate::prompts::Field::BotDescription, &bot.description),
                    ("input_rule", crate::prompts::Field::BotInputRule, &bot.input_rule),
                    ("output_rule", crate::prompts::Field::BotOutputRule, &bot.output_rule),
                ] {
                    meanings.insert(key, prompts.render(field, source).map_err(|error| edit_error("prompt-dictionary-invalid", error.to_string()))?);
                }
                value["rendered"] = serde_json::to_value(meanings).expect("rendered Bot serialization");
                bots.insert(bot.id.clone(), value);
                if let Some(task) = &bot.task_id { task_ids.insert(task.clone()); }
                if let Some(task) = &node.manager_task_id { task_ids.insert(task.clone()); }
                if let Some(child) = &bot.map_file { pending.push((child.clone(), node.zone.fixed().map(str::to_owned))); }
                for rule in &node.input_rule {
                    if let InputRule::File { path, zone_id, .. } = rule {
                        if let Some(zone) = zone_id.as_deref().or(manager_zone.as_deref()) {
                            zone_ids.insert(zone.to_owned());
                            files.push((zone.to_owned(), path.clone()));
                        }
                    }
                }
            }
            maps.insert(file, serde_json::to_value(map).expect("Map serialization"));
        }
        Ok(DelegationMapDefinition { definitions: serde_json::json!({"maps":maps,"bots":bots,"renderedMaps":rendered}), task_ids, zone_ids, files })
    }).map_err(DelegationDefinitionError::Directory)
}
// Each definition operation owns its directory scope through final checks and commit.
// External editors do not participate, so publication still rechecks revisions.
pub(crate) fn with_definitions<R>(
    paths: &[PathBuf],
    code: &'static str,
    action: impl FnOnce(&FileAccess) -> Result<R, DirectoryError>,
) -> Result<R, DirectoryError> {
    FileStore::default().with(paths, action)
        .map_err(|error| edit_error(code, format!("定义文件访问失败：{error}")))?
}
#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct DefinitionRevision { pub(super) file: String, pub(super) original: String }
fn edit_error(code: &'static str, message: impl Into<String>) -> DirectoryError {
    DirectoryError { code, message: message.into() }
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
#[derive(Serialize)]
struct DefinitionFiles { files: Vec<String> }
pub(crate) struct DirectoryError { pub(crate) code: &'static str, pub(crate) message: String }
impl DirectoryError {
    fn io(code: &'static str, action: &str, error: std::io::Error) -> Self {
        Self { code, message: format!("Map 目录{action}失败：{error}") }
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

fn read_files(access: &FileAccess, directory: &Path) -> Result<DefinitionFiles, DirectoryError> {
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
    Ok(DefinitionFiles { files: files.into_iter().map(|(name, _)| name).collect() })
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
