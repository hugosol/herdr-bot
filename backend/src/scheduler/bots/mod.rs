//! Shared Bot identities, catalog and revision-checked editing.
pub(crate) mod script_bots;
use super::{DirectoryError, DefinitionRevision, JsonObject, edit_error, prepare_directory, read_files, with_definitions, check_bot_references};
use crate::files::{FileAccess, PublishMode};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};


#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bot {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    name: String,
    pub(super) when: String,
    pub(super) plan_guide: String,
    pub(super) node_guide: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) map_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rust_script: Option<String>,
}

impl Bot {
    pub(super) fn validate_target(&self) -> Result<(), DirectoryError> {
        if !crate::tasks::valid_group(self.group.as_deref()) {
            return Err(edit_error("group-invalid", "分组名称不能为空或包含首尾空白。"));
        }
        if [self.task_id.is_some(), self.map_file.is_some(), self.rust_script.is_some()].into_iter().filter(|present| *present).count() > 1 {
            return Err(edit_error("bot-invalid", "Bot 的 task_id、map_file 与 rust_script 互斥；请只选择一个执行目标。"));
        }
        if self.rust_script.as_deref().is_some_and(|capability| !matches!(capability, "create_blank_bot" | "save_bot_guides")) {
            return Err(edit_error("bot-invalid", "未知或尚未实现的固定 Rust 能力。"));
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub(crate) struct BotProblem {
    code: &'static str,
    pub(crate) message: String,
    files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bot_id: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct BotCatalog {
    pub(crate) bots: Vec<Bot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) problems: Vec<BotProblem>,
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

pub(crate) fn read_bots(access: &FileAccess, directory: &Path) -> Result<BotCatalog, DirectoryError> {
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

/// Resolve the shared identity with the same conflict and directory checks as
/// the public catalog; a Bot ID is never a filename or a Task name.
pub(crate) enum DelegationTarget {
    Task(String),
    Map(String),
}

pub(crate) fn delegation_bot(directory: &Path, id: &str) -> Result<(serde_json::Value, DelegationTarget), DirectoryError> {
    with_definitions(&[directory.to_owned()], "bot-directory-read-failed", |access| {
        let catalog = read_bots(access, directory)?;
        if let Some(problem) = catalog.problems.iter().find(|problem| problem.bot_id.as_deref() == Some(id) || problem.bot_id.is_none()) {
            return Err(edit_error("bot-invalid", problem.message.clone()));
        }
        let bot = catalog.bots.into_iter().find(|bot| bot.id == id)
            .ok_or_else(|| edit_error("bot-not-found", "目标 Bot 不存在。"))?;
        let target = match (&bot.task_id, &bot.map_file) {
            (Some(id), None) if !id.trim().is_empty() => DelegationTarget::Task(id.clone()),
            (None, Some(file)) if !file.trim().is_empty() => DelegationTarget::Map(file.clone()),
            _ => return Err(edit_error("bot-target-required", "Bot 必须具有明确的 Task 或 Map 目标。")),
        };
        Ok((serde_json::to_value(bot).expect("Bot serialization"), target))
    })
}
pub(crate) async fn list_bots(state: crate::AppState) -> Result<BotCatalog, DirectoryError> {

    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-directory-read-failed", |access| read_bots(access, &state.bot_directory))).await {
        Ok(Ok(catalog)) => Ok(catalog),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(DirectoryError { code: "bot-directory-read-failed", message: format!("共享 Bot 目录读取失败：{error}") }),
    }
}
static NEXT_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotIdentity { id: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotSave {
    id: String,
    content: String,
    revision: Option<DefinitionRevision>,
}
// Resolve identity from current files, never from an ID interpreted as a path.
fn bot_target(access: &FileAccess, directory: &Path, id: &str) -> Result<Option<DefinitionRevision>, DirectoryError> {
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
            return Ok(Some(DefinitionRevision { file, original }));
        }
    }
    Ok(None)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotDelete {
    id: String,
    revision: DefinitionRevision,
}
pub(crate) async fn check_bot_delete(state: crate::AppState, identity: BotIdentity) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone(), state.map_directory.clone()], "bot-delete-failed", |access| {
        let revision = bot_target(access, &state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        check_bot_references(access, &state.map_directory, &identity.id)?;
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "revision": revision }))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("bot-delete-failed", error.to_string())),
    }
}

pub(crate) async fn delete_bot(state: crate::AppState, target: BotDelete) -> Result<BotCatalog, DirectoryError> {
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
        Ok(Ok(catalog)) => Ok(catalog),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("bot-delete-failed", error.to_string())),
    }
}

pub(crate) fn new_bot() -> serde_json::Value {
    // Compile the documentation example into the application; no runtime repo access.
    let mut example: serde_json::Value = serde_json::from_str(include_str!("../../../../docs/example/bots/weather-capability.json")).expect("bundled weather example");
    example.as_object_mut().expect("example object").remove("id");
    example
}

pub(crate) async fn edit_bot(state: crate::AppState, identity: BotIdentity) -> Result<serde_json::Value, DirectoryError> {
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-write-failed", |access| {
        let revision = bot_target(access, &state.bot_directory, &identity.id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        let mut content: serde_json::Value = serde_json::from_str(&revision.original)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 必须为 JSON 对象。"))?.remove("id");
        Ok::<_, DirectoryError>(serde_json::json!({ "id": identity.id, "content": content, "revision": revision }))
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("bot-read-failed", error.to_string())),
    }
}

pub(crate) async fn save_bot(state: crate::AppState, draft: BotSave) -> Result<Bot, DirectoryError> {
    match tokio::task::spawn_blocking(move || save_bot_definition(&state, draft, false)).await {
        Ok(Ok(bot)) => Ok(bot),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("bot-write-failed", error.to_string())),
    }
}

fn save_bot_definition(state: &crate::AppState, draft: BotSave, unique_binding: bool) -> Result<Bot, DirectoryError> {
    with_definitions(&[state.bot_directory.clone()], "bot-write-failed", |access| {
        if draft.id.is_empty() { return Err(edit_error("bot-invalid", "Bot ID 不能为空。")); }
        let mut content: serde_json::Value = serde_json::from_str(&draft.content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot JSON 无效：{e}")))?;
        let object = content.as_object_mut().ok_or_else(|| edit_error("bot-invalid", "Bot 内容必须为 JSON 对象。"))?;
        if object.contains_key("id") { return Err(edit_error("bot-invalid", "请移除 Bot 内容中的顶层 id；使用独立 Bot ID。")); }
        if !object.contains_key("group") {
            if let Some(revision) = bot_target(access, &state.bot_directory, &draft.id)? {
                let JsonObject(existing): JsonObject<Bot> = serde_json::from_str(&revision.original)
                    .map_err(|error| edit_error("bot-invalid", error.to_string()))?;
                if let Some(group) = existing.group { object.insert("group".into(), group.into()); }
            }
        }
        object.insert("id".into(), serde_json::Value::String(draft.id.clone()));
        let JsonObject(bot): JsonObject<Bot> = serde_path_to_error::deserialize(content)
            .map_err(|e| edit_error("bot-invalid", format!("Bot 结构无效：{e}")))?;
        bot.validate_target()?;
        let bytes = serde_json::to_vec_pretty(&bot).map_err(|e| edit_error("bot-write-failed", e.to_string()))?;
        let root = prepare_directory(access, &state.bot_directory).map_err(bot_directory_error)?;
        let check = || -> Result<(), DirectoryError> {
            prepare_directory(access, &root).map_err(bot_directory_error)?;
            let current = bot_target(access, &root, &draft.id)?;
            if unique_binding {
                let catalog = read_bots(access, &root)?;
                if !catalog.problems.is_empty() { return Err(edit_error("ambiguous-request", "Bot 目录包含无法排除身份或绑定冲突的定义。")); }
                if catalog.bots.iter().any(|existing| (draft.revision.is_none() || existing.id != bot.id) && ((bot.task_id.is_some() && bot.task_id == existing.task_id) || (bot.map_file.is_some() && bot.map_file == existing.map_file))) {
                    return Err(edit_error("target-conflict", "该目标已有 Bot 绑定；不复用或覆盖。"));
                }
            }
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
    })
}
pub(crate) async fn set_bot_group(state: crate::AppState, id: String, input: crate::tasks::GroupInput) -> Result<serde_json::Value, DirectoryError> {
    if !crate::tasks::valid_group(input.group.as_deref()) {
        return Err(edit_error("group-invalid", "分组名称不能为空或包含首尾空白。"));
    }
    match tokio::task::spawn_blocking(move || with_definitions(&[state.bot_directory.clone()], "bot-write-failed", |access| {
        let revision = bot_target(access, &state.bot_directory, &id)?
            .ok_or_else(|| edit_error("bot-conflict", "Bot 已移除；请重新加载。"))?;
        let JsonObject(mut bot): JsonObject<Bot> = serde_json::from_str(&revision.original)
            .map_err(|error| edit_error("bot-invalid", error.to_string()))?;
        bot.validate_target()?;
        bot.group = input.group;
        let bytes = serde_json::to_vec_pretty(&bot).map_err(|error| edit_error("bot-write-failed", error.to_string()))?;
        let root = prepare_directory(access, &state.bot_directory).map_err(bot_directory_error)?;
        access.publish_checked(&root.join(&revision.file), &bytes, PublishMode::Replace, |_| {
            if bot_target(access, &state.bot_directory, &id)?.as_ref() != Some(&revision) {
                return Err(edit_error("bot-conflict", "Bot 文件已变化；请重新加载。"));
            }
            Ok(())
        }, |error| edit_error("bot-write-failed", error.to_string()))?;
        let mut value = serde_json::to_value(bot).map_err(|error| edit_error("bot-write-failed", error.to_string()))?;
        value["revision"] = serde_json::json!({"file":revision.file,"original":String::from_utf8(bytes).expect("JSON is UTF-8")});
        Ok(value)
    })).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error),
        Err(error) => Err(edit_error("bot-write-failed", error.to_string())),
    }
}
