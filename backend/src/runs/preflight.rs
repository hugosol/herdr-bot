//! Read-only launch validation shared by previews and actual Run startup.
use crate::files::FileAccess;
use crate::scheduler::{DirectoryError, object_array, with_definitions};
use crate::scheduler::bots::{BotCatalog, read_bots};
use crate::scheduler::maps::{WorkflowMap, InputRule, MapFileInput, ReportPolicy, ResolvedFile, read_map};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};


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
impl PreflightFailure {
    pub(crate) fn into_payload(self) -> serde_json::Value {
        serde_json::json!({
            "code": "run-preflight-rejected", "message": "启动预检未通过；请返回对应原配置修正后重新校验。",
            "issues": self.0,
        })
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
            None => {
                let reason = bots.problems.iter().find(|problem| problem.bot_id.as_deref() == Some(node.bot_id.as_str()))
                    .map_or("共享 Bot 缺失或无效；请修正引用或文件。", |problem| problem.message.as_str());
                issues.push(PreflightIssue::new("bot-missing", "node", format!("node {id} 的 Bot {}：{reason}", node.bot_id)));
            }
            Some(bot) if bot.rust_script.is_some() => {
                if node.manager_task_id.is_some() || node.report_policy != ReportPolicy::Auto || node.auto_close {
                    issues.push(PreflightIssue::new("script-policy-invalid", "node", "脚本节点仅允许 report_policy:auto、auto_close:false，且无 manager_task_id。"));
                }
            }
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

fn input_issues(map_name: &str, declarations: &[MapFileInput], files: &[ResolvedFile], require_complete: bool) -> Vec<PreflightIssue> {
    let mut issues = Vec::new();
    let mut provided = std::collections::BTreeSet::new();
    for binding in files {
        let failure = if !provided.insert(&binding.id) {
            Some(("run-input-duplicate", format!("Run 文件输入 ID 重复：{}", binding.id)))
        } else if !declarations.iter().any(|declaration| declaration.id == binding.id) {
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
    if require_complete {
        for declaration in declarations {
            if declaration.required && !provided.contains(&declaration.id) {
                let mut issue = PreflightIssue::new("run-input-required", "run-input", format!("Map {map_name} 缺少必需文件输入：{}", declaration.id));
                issue.input_id = Some(declaration.id.clone());
                issues.push(issue);
            }
        }
    }
    issues
}

pub(crate) fn check_prepared_inputs(directory: &Path, name: &str, files: &[ResolvedFile], confirmed: bool) -> Result<Option<String>, String> {
    with_definitions(&[directory.to_path_buf()], "map-read-failed", |access| {
        let map = read_map(access, directory, name)?;
        let issues = input_issues(name, &map.input_files, files, confirmed);
        Ok((!issues.is_empty()).then(|| PreflightFailure(issues).message()))
    }).map_err(|error| error.message)
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
    issues.extend(input_issues(&map_name, &map.input_files, &input.file_inputs, false));
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
    let workspace_name = crate::herdr::placement::root_map_workspace_name(&zone).unwrap_or_default();
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
