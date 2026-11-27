//! Run startup, manager queries, explicit node dispatch and handoff/finish.
//! Manager startup errors are process-local; node dispatch failures are persisted.
use crate::{AppState, files::FileStore, scheduler::maps::WorkflowMap};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
pub(crate) mod reports;
pub(crate) mod preflight;
#[cfg(test)]
mod report_isolation_tests;
pub(crate) mod recovery;
pub(crate) mod retry;
mod startup;
pub(crate) mod completion;
mod operation;
pub(crate) mod preparation;
pub(crate) mod collect;
pub(crate) mod script_output;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatReference {
    pub session_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
    pub agent_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeState {
    pub status: String,
    pub execution: Option<NodeExecution>,
    pub failure: Option<StartupFailure>,
    pub completion: Option<Completion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auto_close_failure: Option<AutoCloseFailure>,
    pub previous_executions: Vec<ExecutionFact>,
}
#[derive(Clone, Serialize, Deserialize)]
struct AutoCloseFailure {
    code: String,
    message: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionFact {
    pub execution: Option<NodeExecution>,
    pub failure: Option<StartupFailure>,
}
impl NodeState {
    fn executions(&self) -> impl Iterator<Item = &NodeExecution> {
        self.execution.iter().chain(self.previous_executions.iter().filter_map(|fact| fact.execution.as_ref()))
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Completion {
    pub id: String,
    pub at: u64,
    pub actor: ChatReference,
    pub reason: String,
}
mod execution;
use execution::{ChatExecution, ExecutionTarget, Lifecycle, ScriptExecution, ScriptKind};
pub(crate) mod scripts;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalSource { Auto, Manual }

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExecution {
    pub id: String,
    #[serde(flatten)]
    pub target: ExecutionTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    pub zone: String,
    #[serde(default)]
    pub input_rule: Option<Vec<crate::scheduler::maps::InputRule>>,
    #[serde(default, deserialize_with = "crate::scheduler::optional_object_array")]
    pub output_rule: Option<Vec<crate::scheduler::maps::OutputRule>>,
    #[serde(default, deserialize_with = "crate::scheduler::optional_object_array")]
    pub inputs: Option<Vec<crate::scheduler::maps::ResolvedFile>>,
    pub auto_approved: bool,
    pub approved: bool,
    pub approval_source: Option<ApprovalSource>,
    #[serde(default)]
    pub auto_close: bool,
    #[serde(default)]
    pub end_node: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParentExecution {
    pub run_id: String,
    pub node_id: String,
    pub execution_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finished {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub id: String,
    pub at: u64,
    pub actor: ChatReference,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Run {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<RunNaming>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub map: String,
    pub created_at: u64,
    pub manager_task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ParentExecution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_zone_id: Option<String>,
    #[serde(default)]
    pub zone_bindings: BTreeMap<String, String>,
    pub zone: String,
    pub workspace_name: String,
    pub user_input: String,
    #[serde(default)]
    pub file_inputs: Vec<crate::scheduler::maps::ResolvedFile>,
    #[serde(default)]
    pub inputs_ready: bool,
    pub manager: Option<ChatReference>,
    pub status: String,
    pub finished: Option<Finished>,
    pub nodes: BTreeMap<String, NodeState>,
}
impl Run {
    pub(crate) fn manager_context<'a>(&'a self, state: &'a AppState) -> crate::prompts::ManagerContext<'a> {
        crate::prompts::ManagerContext {
            id: &self.id,
            map: Runs::map_path(state, &self.id),
            files: self.prompt_files(state),
        }
    }

    fn prompt_files<'a>(&self, state: &'a AppState) -> crate::prompts::RunFiles<'a> {
        crate::prompts::RunFiles {
            instructions: &state.run_instructions,
            records: Runs::paths(state, &self.id),
            materials: startup::material_directory(&self.zone, &self.id),
        }
    }

    fn session_claimed_by_other_execution(&self, session_id: &str, execution_id: &str) -> bool {
        self.manager.as_ref().is_some_and(|chat| chat.session_id == session_id)
            || self.nodes.values().any(|node| node.executions().any(|other| {
                other.id != execution_id && other.chat().is_some_and(|chat| chat.session_id == session_id)
            }))
    }
}

/// Display-only launch snapshot, independent of mutable Run names and definitions.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunNaming {
    pub map_path: Vec<String>,
    pub node_titles: BTreeMap<String, String>,
}

impl RunNaming {
    fn manager_title(&self) -> String {
        let (current, ancestors) = self.map_path.split_last().expect("new Run has a Map title");
        if ancestors.is_empty() {
            format!("[M]{current}")
        } else {
            format!("[S]{current}-{}", ancestors.join("/"))
        }
    }

    fn node_title(&self, id: &str, current_title: &str) -> String {
        let title = self.node_titles.get(id).map(String::as_str).unwrap_or(current_title);
        format!("{title}-{}", self.map_path.join("/"))
    }
}

// Resume needs durable identity, not the business schema of each Run.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunChatAssociations {
    id: String,
    manager_task_id: String,
    parent: Option<ParentExecution>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    manager: Option<ChatReference>,
    nodes: BTreeMap<String, ChatNodeAssociations>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatNodeAssociations {
    #[serde(deserialize_with = "Deserialize::deserialize")]
    execution: Option<ChatExecutionAssociation>,
    previous_executions: Vec<ChatExecutionFact>,
}

#[derive(Deserialize)]
struct ChatExecutionFact {
    #[serde(deserialize_with = "Deserialize::deserialize")]
    execution: Option<ChatExecutionAssociation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatExecutionAssociation {
    id: String,
    kind: Option<AssociationScriptKind>,
    task_id: Option<String>,
    agent: Option<crate::tasks::Agent>,
    #[serde(default, deserialize_with = "present_chat")]
    chat: Option<Option<ChatReference>>,
    child_run_id: Option<String>,
    #[serde(default)]
    end_node: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AssociationScriptKind { Typescript, RustScript }

// A missing Chat field is not the same as a recorded absence of native identity.
fn present_chat<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Option<ChatReference>>, D::Error> {
    Option::deserialize(deserializer).map(Some)
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    pub records_initialized: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_run_id: Option<String>,
    #[serde(default)]
    pub child_records_initialized: bool,
    pub workspace_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_started: bool,
    pub session_id: Option<String>,
    pub association_saved: bool,
    pub input_attempted: bool,
    pub input_submitted: bool,
    /// Missing on historical records; never infer a rejected delivery from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_outcome: Option<InputOutcome>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputOutcome { Submitted, Rejected, Unknown }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupFailure {
    pub stage: String,
    pub message: String,
    // Execution failure records its original Chat through NodeExecution; it
    // does not manufacture a new observation of startup/submission effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<Effects>,
}
#[derive(Default)]
pub struct Runs {
    files: FileStore,
    active: Mutex<BTreeMap<String, Option<StartupFailure>>>,
    operations: Mutex<BTreeMap<String, std::sync::Arc<Mutex<()>>>>,
    signals: Mutex<BTreeMap<String, std::sync::Arc<collect::Signals>>>,
    script_output: script_output::Store,
}
impl Runs {
    fn paths(state: &AppState, id: &str) -> [PathBuf; 3] {
        let root = state.run_directory.join(id);
        [
            root.join("run.json"),
            root.join("history.jsonl"),
            root.join("messages.jsonl"),
        ]
    }
    fn map_path(state: &AppState, id: &str) -> PathBuf {
        state.run_directory.join(id).join("map.json")
    }

    async fn read_map(&self, state: &AppState, run: &Run) -> Result<WorkflowMap, String> {
        let path = Self::map_path(state, &run.id);
        let source = self.files.with_async(std::slice::from_ref(&path), |access| {
            let path = path.clone();
            Box::pin(async move {
                for ancestor in path.ancestors() {
                    let metadata = access.symlink_metadata(ancestor)?;
                    if crate::zones::browse::is_link_like(&metadata)
                        || !(if ancestor == path { metadata.is_file() } else { metadata.is_dir() }) {
                        return Err(std::io::Error::other(format!("冻结定义路径不能经过链接或不匹配的文件类型：{}", ancestor.display())));
                    }
                }
                access.read_to_string(&path)
            })
        }).await.and_then(|result| result).map_err(|error| format!("冻结 Map {} 无法读取：{error}", path.display()))?;
        let map = crate::scheduler::maps::parse_map(&source)
            .map_err(|error| format!("冻结 Map {} 无效：{}", path.display(), error.message))?;
        if !map.nodes.keys().eq(run.nodes.keys()) {
            return Err(format!("冻结 Map {} 与已保存 Run 的节点标识不一致。", path.display()));
        }
        Ok(map)
    }

    /// Recover role from persisted execution identity, including retired attempts.
    pub(crate) async fn chat_role(&self, state: &AppState, session_id: &str, task_id: Option<&str>, agent: crate::tasks::Agent) -> Result<Option<crate::tasks::catalog::ExecutionRole>, String> {
        let root = state.run_directory.clone();
        let entries = match self.files.with_async(&[root.clone()], move |access| Box::pin(async move { access.read_dir(&root) })).await.map_err(|e| e.to_string())? {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        const INVALID_LINK: &str = "原生 Chat 的持久执行关联不唯一或缺少合法父子关系。";
        struct NodeMatch {
            node_id: String,
            execution_id: String,
            child_run_id: Option<String>,
            end_node: bool,
            chat: ChatReference,
        }
        let mut manager_match = None;
        let mut node_match = None;
        for entry in entries {
            if !entry.file_type().map_err(|error| error.to_string())?.is_dir() { continue; }
            let id = entry.file_name().to_string_lossy().into_owned();
            let run: RunChatAssociations = self.read_record(state, &id).await?;
            if run.id != id { return Err(format!("Run {id} 保存身份不一致：{}。", run.id)); }
            let mut run_match = None;
            for (node_id, node) in run.nodes {
                let mut matched = None;
                for execution in node.execution.into_iter().chain(node.previous_executions.into_iter().filter_map(|fact| fact.execution)) {
                    match (execution.kind, execution.task_id, execution.agent, execution.chat) {
                        (Some(_), None, None, None) if execution.child_run_id.is_none() => {},
                        (None, Some(source), Some(host), Some(chat)) => {
                            if let Some(chat) = chat.filter(|chat| chat.session_id == session_id) {
                                if task_id.is_some_and(|id| id != source) || host != agent {
                                    return Err("原生 Chat 的持久执行关联与来源 Task 或 agent 不一致。".into());
                                }
                                if matched.is_some() || run_match.is_some() || node_match.is_some() { return Err(INVALID_LINK.into()); }
                                matched = Some((execution.id, execution.child_run_id, execution.end_node, chat));
                            }
                        }
                        _ => return Err(format!("Run {id} / node {node_id} / execution {} 的 Chat 关联无效。", execution.id)),
                    }
                }
                if let Some((execution_id, child_run_id, end_node, chat)) = matched {
                    run_match = Some(NodeMatch { node_id, execution_id, child_run_id, end_node, chat });
                }
            }
            if let Some(matched) = run_match { node_match = Some((id, matched)); }
            if let Some(chat) = run.manager.filter(|chat| chat.session_id == session_id) {
                if task_id.is_some_and(|id| id != run.manager_task_id) { return Err("原生 Chat 的持久角色关联与来源 Task 不一致。".into()); }
                if manager_match.is_some() { return Err(INVALID_LINK.into()); }
                manager_match = Some((run.id, run.parent, chat));
            }
        }
        use crate::tasks::catalog::ExecutionRole;
        match (manager_match, node_match) {
            (None, None) => Ok(None),
            (Some((_, None, _)), None) => Ok(Some(ExecutionRole::Manager)),
            (None, Some((_, node))) if node.child_run_id.is_none() => Ok(Some(if node.end_node { ExecutionRole::Standalone } else { ExecutionRole::Node })),
            (Some((child_id, Some(link), manager)), Some((parent_id, node)))
                if child_id != parent_id && link.run_id == parent_id && link.node_id == node.node_id
                    && link.execution_id == node.execution_id && node.child_run_id.as_deref() == Some(child_id.as_str())
                    && manager == node.chat => Ok(Some(ExecutionRole::NestedManager)),
            _ => Err(INVALID_LINK.into()),
        }
    }

    async fn read(&self, state: &AppState, id: &str) -> Result<Run, String> {
        let run: Run = self.read_record(state, id).await?;
        if run.id != id {
            return Err(format!("Run {id} 保存身份不一致：{}。", run.id));
        }
        Ok(run)
    }

    async fn read_record<T: serde::de::DeserializeOwned + Send>(&self, state: &AppState, id: &str) -> Result<T, String> {
        let paths = Self::paths(state, id);
        self.files.with_async(&paths.clone(), move |access| Box::pin(async move {
            // Required files are never silently recreated. JSONL is independently readable.
            for path in &paths[1..] {
                access.check_readable(path).map_err(|error| format!("{}：{error}", path.display()))?;
            }
            access.read_json(&paths[0])
        })).await.map_err(|e| e.to_string())?
    }
}

pub(crate) enum RunOutcome {
    Value(serde_json::Value),
    Rejected(serde_json::Value),
    Started { failed: bool, value: serde_json::Value },
}
pub(crate) enum RunError {
    Invalid(serde_json::Value),
    Forbidden(serde_json::Value),
    Failed(serde_json::Value),
    Preflight(crate::runs::preflight::PreflightFailure),
    Task(crate::tasks::TaskError),
}
pub(crate) type RunResult = Result<RunOutcome, RunError>;
#[derive(Clone, Copy)]
enum FailureKind { Invalid, Forbidden, Failed }
fn error(kind: FailureKind, code: &str, message: &str) -> RunError {
    let value = serde_json::json!({"code":code,"message":message});
    match kind {
        FailureKind::Invalid => RunError::Invalid(value),
        FailureKind::Forbidden => RunError::Forbidden(value),
        FailureKind::Failed => RunError::Failed(value),
    }
}
pub(crate) fn authorize_source(source: std::net::IpAddr) -> Result<(), RunError> {
    if crate::sessions::is_local_source(source) { Ok(()) }
    else { Err(error(FailureKind::Forbidden, "forbidden", "运行工具只接受同机来源。")) }
}
async fn detail(state: &AppState, id: &str) -> Result<serde_json::Value, String> {
    let active = state.runs.active.lock().await;
    let managed = active.contains_key(id);
    let failure = active.get(id).cloned().flatten();
    drop(active);
    let (run, read_error) = match state.runs.read(state, id).await {
        Ok(run) => (Some(run), None),
        Err(error) if failure.is_some() => (None, Some(error)),
        Err(error) => return Err(error),
    };
    let mut value = serde_json::json!({"id":id,"run":run,"managed":managed,"startupError":failure,"readError":read_error});
    if let Some(run) = &run {
        let paths = Runs::paths(state, id);
        state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
            access.read_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1])
        })).await.map_err(|e| e.to_string())??;
        let reports = reports::summary(state, run, true).await?;
        value["reports"] = reports["reports"].clone();
        value["pendingReports"] = reports["pendingReports"].clone();
    }
    Ok(value)
}
pub(crate) async fn list_runs(state: AppState) -> RunResult {
    let active = state.runs.active.lock().await.clone();
    let mut ids: std::collections::BTreeSet<String> = active.keys().cloned().collect();
    let root = state.run_directory.clone();
    let entries = state.runs.files.with_async(&[root.clone()], move |access| Box::pin(async move { access.read_dir(&root) })).await.and_then(|entries| entries);
    match entries {
        Ok(entries) => {
            for entry in entries {
                let kind = match entry.file_type() { Ok(kind) => kind, Err(e) => return Err(error(FailureKind::Failed, "runs-read-failed", &e.to_string())) };
                let Some(id) = entry.file_name().to_str().map(str::to_owned) else { continue };
                if kind.is_dir() && !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') { ids.insert(id); }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(error(FailureKind::Failed, "runs-read-failed", &e.to_string())),
    }
    let mut runs = Vec::with_capacity(ids.len());
    for id in ids {
        let (run, read_error) = match state.runs.read(&state, &id).await {
            Ok(run) => (Some(run), None),
            Err(e) => (None, Some(e)),
        };
        runs.push(serde_json::json!({"id":id,"run":run,"managed":active.contains_key(&id),"readError":read_error}));
    }
    runs.sort_by(|a, b| b["run"]["createdAt"].as_u64().unwrap_or(0).cmp(&a["run"]["createdAt"].as_u64().unwrap_or(0)).then_with(|| a["id"].as_str().cmp(&b["id"].as_str())));
    Ok(RunOutcome::Value(serde_json::json!({"runs":runs,"pollIntervalSeconds":state.workflow_poll_interval_seconds})))
}

pub(crate) async fn get_run(state: AppState, id: String) -> RunResult {
    validate_id(&id)?;
    match detail(&state, &id).await {
        Ok(value) => Ok(RunOutcome::Value(value)),
        Err(message) => Err(error(
            FailureKind::Failed,
            "run-read-failed",
            &message,
        )),
    }
}

pub(crate) async fn get_run_map(state: AppState, id: String) -> RunResult {
    validate_id(&id)?;
    let run = state.runs.read(&state, &id).await
        .map_err(|message| error(FailureKind::Failed, "run-read-failed", &message))?;
    let map = state.runs.read_map(&state, &run).await
        .map_err(|message| error(FailureKind::Failed, "run-map-read-failed", &message))?;
    let value = serde_json::to_value(map)
        .map_err(|cause| error(FailureKind::Failed, "run-map-read-failed", &cause.to_string()))?;
    Ok(RunOutcome::Value(value))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LocateRequest {
    execution_id: Option<String>,
}

impl ChatReference {
    fn location(&self) -> crate::herdr::NavigationTarget {
        crate::herdr::NavigationTarget {
            workspace_id: self.workspace_id.clone(), tab_id: self.tab_id.clone(), pane_id: self.pane_id.clone(),
        }
    }

    async fn saved_history(&self, state: &AppState) -> Result<crate::chats::history::HistorySelection, crate::chats::Error> {
        use crate::chats::Error;
        state.history.selection(&self.session_id).await
            .map_err(|error| Error::Internal("history-unreadable", error.to_string()))?
            .ok_or_else(|| Error::NotFound("run-chat-not-found", "找不到关联 Chat 的原生历史。".into()))
    }

    async fn app_target(&self, state: &AppState, location: &crate::herdr::NavigationTarget) -> Result<std::sync::Arc<crate::herdr::LaunchTarget>, crate::chats::Error> {
        let selected = self.saved_history(state).await?;
        let kind = run_agent(&selected)?;
        let native = state.herdr.selected_native_id(kind, &selected).await.map_err(run_chat_lookup_failure)?;
        state.launches.app_target_at(&state.herdr, kind, &native, location).await
            .map_err(run_chat_lookup_failure)?.ok_or_else(run_chat_not_live)
    }
}

fn run_agent(selected: &crate::chats::history::HistorySelection) -> Result<&str, crate::chats::Error> {
    selected.agent_type.as_deref().filter(|kind| matches!(*kind, "omp" | "pi"))
        .ok_or_else(|| crate::chats::Error::Invalid("unsupported-agent", "无法确定目标 Chat 的 agent 类型。".into()))
}

fn run_chat_lookup_failure(error: crate::herdr::HerdrError) -> crate::chats::Error {
    crate::chats::Error::Internal("run-chat-lookup-failed", error.message())
}

fn run_chat_not_live() -> crate::chats::Error {
    crate::chats::Error::NotFound("run-chat-not-found", "所选 Run／执行关联的宿主目标不在运行或已变化；未使用其他目标。".into())
}

pub(crate) async fn locate_chat(state: AppState, id: &str, input: LocateRequest) -> Result<serde_json::Value, crate::chats::Error> {
    use crate::chats::Error;
    let run = state.runs.read(&state, id).await.map_err(|message| Error::Internal("run-read-failed", message))?;
    let chat = match input.execution_id {
        None => run.manager.as_ref(),
        Some(id) => {
            let mut executions = run.nodes.values().flat_map(NodeState::executions).filter(|execution| execution.id == id);
            let execution = executions.next();
            if executions.next().is_some() {
                return Err(Error::Conflict("run-chat-ambiguous", "同一执行 ID 有多个保存关联；尚未定位。".into()));
            }
            execution.and_then(NodeExecution::chat)
        }
    }.ok_or_else(|| Error::NotFound("run-chat-not-found", "没有所选 Run／执行的 Chat 关联；尚未定位。".into()))?;
    let location = chat.location();
    if state.herdr.is_app() {
        let target = chat.app_target(&state, &location).await?;
        state.herdr.focus_app_target(&target).await.map(drop)
    } else {
        let selected = chat.saved_history(&state).await?;
        let targets = state.herdr.matching_targets(run_agent(&selected)?, &selected).await.map_err(run_chat_lookup_failure)?;
        if !targets.contains(&location) { return Err(run_chat_not_live()); }
        state.herdr.focus_target(&location).await
    }.map_err(|error| Error::Internal("run-chat-focus-failed", error.message()))?;
    Ok(serde_json::json!({"outcome":"located","target":location,"message":"已聚焦所选宿主内的 Run Chat；不保证 Windows 窗口置前。"}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenameRequest {
    pub(crate) name: String,
}

pub(crate) async fn rename_run(state: AppState, id: String, input: RenameRequest) -> RunResult {
    validate_id(&id)?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(error(FailureKind::Invalid, "run-name-required", "Run 名称不能为空。"));
    }
    let lock = state.runs.operations.lock().await.entry(id.clone()).or_default().clone();
    let _operation = lock.lock().await;
    // Read after taking the same operation lock used by workflow mutations.
    // Never authorize from management status: saved historical Runs are editable.
    let mut run = match state.runs.read(&state, &id).await {
        Ok(run) => run,
        Err(message) => return Err(error(FailureKind::Failed, "run-read-failed", &message)),
    };
    // Validate the same independent records as GET before changing readable history.
    if let Err(message) = detail(&state, &id).await {
        return Err(error(FailureKind::Failed, "run-read-failed", &message));
    }
    run.name = Some(name.to_owned());
    if let Err(message) = save_run(&state, &run).await {
        return Err(error(FailureKind::Failed, "run-save-failed", &message));
    }
    match detail(&state, &id).await {
        Ok(value) => Ok(RunOutcome::Value(value)),
        Err(message) => Err(error(FailureKind::Failed, "run-read-failed", &message)),
    }
}

pub(crate) async fn start_run(state: AppState, map: String, input: crate::runs::preflight::RunPreflightRequest) -> RunResult {
    let checked = match crate::runs::preflight::check_run_preflight(&state, &map, &input).await {
        Ok(v) => v,
        Err(e) => return Err(RunError::Preflight(e)),
    };
    let task = match state.tasks.get(&input.manager_task_id).await {
        Ok(v) => v,
        Err(e) => return Err(RunError::Task(e)),
    };
    let suite_args = match state.tasks.prepare_environment(&task) {
        Ok(args) => args,
        Err(error) => return Err(RunError::Task(error)),
    };
    let mut preset = match state
        .tasks
        .catalog()
        .and_then(|catalog| catalog.resolve(&task))
    {
        Ok(v) => v,
        Err(e) => return Err(RunError::Task(e)),
    };
    preset.env.suite_args = suite_args;
    crate::herdr::native_status::select_host(&state.herdr, &mut preset.env);
    if let Err(message) = preset.env.check_entrypoints() {
        return Err(error(
            FailureKind::Failed,
            "extension-entrypoint-invalid",
            &message,
        ));
    }
    let mut input = input;
    let id = crate::chats::new_creation_id();
    if let Some(github) = input.github_exploration.take() {
        match startup::write_github_materials(&state, &checked.zone_bindings[&input.zone], &id, github).await {
            Ok(bindings) => input.file_inputs.extend(bindings),
            Err(message) => return Err(error(FailureKind::Failed, "run-material-write-failed", &message)),
        }
    }
    let run = root_run(map, input, checked, &task, id);
    launch_root(state, run, task, preset, None).await
}

pub(crate) struct PreparedRun {
    pub(crate) run: Run,
    map_source: String,
}

/// Construct a root Run from the same preflight for UI and chief delegation.
/// Previewing allocates no records and never invents a parent execution.
pub(crate) fn root_run(
    map: String,
    input: crate::runs::preflight::RunPreflightRequest,
    checked: crate::runs::preflight::RunPreflight,
    task: &crate::tasks::Task,
    id: String,
) -> PreparedRun {
    let run = Run {
        id,
        name: Some(checked.naming.manager_title()),
        naming: Some(checked.naming),
        map,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        manager_task_id: task.id.clone(),
        parent: None,
        zone: checked.zone_bindings[&input.zone].clone(),
        manager_zone_id: Some(input.zone),
        zone_bindings: checked.zone_bindings,
        workspace_name: checked.workspace_name,
        user_input: input.user_input,
        file_inputs: input.file_inputs,
        inputs_ready: false,
        manager: None,
        status: "running".into(),
        finished: None,
        nodes: checked
            .map.nodes
            .into_keys()
            .map(|id| {
                (
                    id,
                    NodeState {
                        status: "pending".into(),
                        execution: None,
                        failure: None,
                        completion: None,
                        auto_close_failure: None,
                        previous_executions: Vec::new(),
                    },
                )
            })
            .collect(),
    };
    PreparedRun { run, map_source: checked.map_source }
}

pub(crate) async fn launch_root(
    state: AppState,
    prepared: PreparedRun,
    task: crate::tasks::Task,
    preset: crate::tasks::catalog::ResolvedPreset,
    delegation: Option<crate::chief_delegation::Launch>,
) -> RunResult {
    let id = prepared.run.id.clone();
    state.runs.active.lock().await.insert(id.clone(), None);
    // Own accepted work independently of the browser request. A disconnected page
    // must not cancel a started native Chat or cause an implicit retry.
    let owned = state.clone();
    let result = tokio::spawn(async move { startup::manager(&owned, prepared, task, preset, delegation).await }).await;
    let failed = !matches!(result, Ok(Ok(())));
    if let Err(join) = result {
        startup::manager_interrupted(&state, &id, join).await;
    }
    match detail(&state, &id).await {
        Ok(value) => Ok(RunOutcome::Started { failed, value }),
        Err(message) => Err(error(
            FailureKind::Failed,
            "run-read-failed",
            &message,
        )),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StatusRequest {
    pub(crate) session_id: String,
}
pub(crate) async fn manager_status(state: AppState, input: StatusRequest) -> RunResult {
    let run = manager_run(&state, &input.session_id).await?;
    let map = state.runs.read_map(&state, &run).await
        .map_err(|message| error(FailureKind::Failed, "definition-read-failed", &message))?;
    snapshot(&state, &run, &map, true).await.map(RunOutcome::Value)
        .map_err(|message| error(FailureKind::Failed, "definition-read-failed", &message))
}

fn nested_pair(child: &Run, parent: &Run, node_id: &str, execution: &NodeExecution) -> bool {
    child.id != parent.id && child.parent.as_ref().is_some_and(|link|
        link.run_id == parent.id && link.node_id == node_id && link.execution_id == execution.id)
        && execution.child_run_id().map(String::as_str) == Some(child.id.as_str())
        && execution.task_id() == Some(child.manager_task_id.as_str())
        && child.manager.is_some() && child.manager.as_ref() == execution.chat()
}

async fn nested_source(state: &AppState, child: &Run, execution: &NodeExecution) -> bool {
    state.tasks.get(&child.manager_task_id).await.is_ok_and(|task|
        Some(task.id.as_str()) == execution.task_id() && Some(task.agent) == execution.agent() && task.task_type == crate::tasks::TaskType::Manager)
}

async fn manager_run(state: &AppState, session_id: &str) -> Result<Run, RunError> {
    let ids = state
        .runs
        .active
        .lock()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut matched = None;
    let mut read_error = None;
    let mut node_associations = Vec::new();
    for id in ids {
        let run = match state.runs.read(&state, &id).await {
            Ok(v) => v,
            Err(message) => {
                read_error.get_or_insert(message);
                continue;
            }
        };
        for (node_id, node) in &run.nodes {
            for execution in node.executions().filter(|execution| execution.chat().is_some_and(|chat| chat.session_id == session_id)) {
                node_associations.push((run.clone(), node_id.clone(), execution.clone()));
            }
        }
        if run
            .manager
            .as_ref()
            .is_some_and(|manager| manager.session_id == session_id)
        {
            if matched.is_some() {
                return Err(error(
                    FailureKind::Forbidden,
                    "manager-ambiguous",
                    "原生身份关联不唯一。",
                ));
            }
            matched = Some(run);
        }
    }
    match matched {
        Some(run) if !node_associations.is_empty() && !(node_associations.len() == 1 && nested_pair(&run, &node_associations[0].0, &node_associations[0].1, &node_associations[0].2)) => Err(error(
            FailureKind::Forbidden,
            "manager-ambiguous",
            "原生身份同时关联节点执行与管理者，归属不唯一。",
        )),
        Some(run) => {
            if run.parent.is_some() && node_associations.is_empty()
                || !node_associations.is_empty() && !nested_source(state, &run, &node_associations[0].2).await {
                return Err(error(FailureKind::Forbidden, "manager-ambiguous", "父子执行来源 Task 或 agent 不一致。"));
            }
            Ok(run)
        },
        None if read_error.is_some() => Err(error(
            FailureKind::Failed,
            "run-read-failed",
            read_error.as_deref().unwrap(),
        )),
        None => Err(error(
            FailureKind::Forbidden,
            "manager-unassociated",
            "当前原生身份没有唯一的管理者归属。",
        )),
    }
}

fn blocked(run: &Run, id: &str, map: &WorkflowMap) -> Vec<String> {
    let mut reasons = Vec::new();
    if run.status != "running" { reasons.push(format!("Run 状态为 {}", run.status)); }
    if !run.inputs_ready { reasons.push("Run 输入尚未由管理者检查并确认；请先 prepare_inputs。".into()); }
    match run.nodes.get(id) {
        None => reasons.push("node 不在本次 Run 中".into()),
        Some(node) if node.status != "pending" => reasons.push(format!("node 状态为 {}，仅 pending 可派发", node.status)),
        _ => {}
    }
    let mut check = |dependency: &str| {
        if !run.nodes.get(dependency).is_some_and(|node| node.status == "completed") {
            reasons.push(format!("前置 {dependency} 尚未 completed"));
        }
    };
    match map.nodes.get(id) {
        None => reasons.push("node 不在本次 Run 的 Map 中".into()),
        Some(node) if node.0.end_node => map.nodes.keys().filter(|other| other.as_str() != id).for_each(|dependency| check(dependency)),
        Some(node) => node.0.blocked_by.iter().for_each(|dependency| check(dependency)),
    }
    reasons
}
async fn snapshot(state: &AppState, run: &Run, map: &WorkflowMap, include_reports: bool) -> Result<serde_json::Value, String> {
    let blocked_by: BTreeMap<_, _> = run.nodes.keys().map(|id| (id, blocked(run, id, map))).collect();
    let ready: Vec<_> = blocked_by.iter().filter(|(_, reasons)| reasons.is_empty()).map(|(id, _)| *id).collect();
    let mut value = reports::summary(state, run, include_reports).await?;
    value["snapshot"] = serde_json::to_value(run).map_err(|e| e.to_string())?;
    value["ready"] = serde_json::json!(ready);
    value["blocked"] = serde_json::json!(blocked_by);
    value["canFinish"] = serde_json::json!(completion::can_finish(run, map));
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchRequest {
    pub(crate) session_id: String,
    pub(crate) nodes: Vec<String>,
    #[serde(default)]
    pub(crate) zone_ids: BTreeMap<String, String>,
}
pub(crate) async fn manager_dispatch(state: AppState, input: DispatchRequest) -> RunResult {
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return Err(response),
    };
    // Accepted external work outlives a disconnected tool request. The business
    // lock spans the batch, while file locks never span host calls.
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        let map = state.runs.read_map(&state, &run).await?;
        // Validate every dynamic selection before starting any external execution.
        let mut zones = crate::scheduler::validate_zone_selections(&state, &run, &map, &input.nodes, &input.zone_ids).await?;
        let mut results = Vec::with_capacity(input.nodes.len());
        for id in input.nodes {
            let reasons = blocked(&run, &id, &map);
            if !reasons.is_empty() {
                results.push(serde_json::json!({"nodeId":id,"status":"rejected","reasons":reasons}));
                continue;
            }
            let target = crate::scheduler::dispatch_target(&state, &run, &map, &id, zones.remove(&id)).await;
            results.push(startup::node(&state, &mut run, &id, None, target).await);
        }
        let mut response = snapshot(&state, &run, &map, true).await?;
        response["results"] = serde_json::Value::Array(results);
        Ok::<_, String>(response)
    }).await;
    match work {
        Ok(Ok(value)) => Ok(RunOutcome::Value(value)),
        Ok(Err(message)) => Err(error(FailureKind::Failed, "dispatch-failed", &message)),
        Err(join) => Err(error(FailureKind::Failed, "dispatch-internal", &join.to_string())),
    }
}

pub(crate) async fn definition_in_use(state: &AppState, task_id: Option<&str>, map_file: Option<&str>) -> Result<bool, String> {
    let ids: Vec<_> = state.runs.active.lock().await.keys().cloned().collect();
    for id in ids {
        let run = state.runs.read(state, &id).await?;
        if run.status == "completed" { continue; }
        if map_file == Some(run.map.as_str()) || task_id == Some(run.manager_task_id.as_str())
            || run.nodes.values().any(|node| node.executions().any(|execution| task_id.is_some() && execution.task_id() == task_id)) {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn save_run(state: &AppState, run: &Run) -> Result<(), String> {
    let paths = Runs::paths(state, &run.id);
    let owned = state.clone();
    let run = run.clone();
    state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
        access.replace(&paths[0], &serde_json::to_vec_pretty(&run).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        owned.runs.script_output.retain_current(&run);
        owned.runs.changed(&run.id, false).await;
        Ok(())
    })).await.map_err(|e| e.to_string())?
}


pub(crate) fn validate_id(id: &str) -> Result<(), RunError> {
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return Err(error(FailureKind::Invalid, "run-id-invalid", "Run 标识无效。"));
    }
    Ok(())
}
