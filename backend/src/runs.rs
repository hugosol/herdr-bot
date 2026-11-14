//! Run startup, manager queries, explicit node dispatch and handoff/finish.
//! Manager startup errors are process-local; node dispatch failures are persisted.
use crate::{AppState, files::FileStore};
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
    pub previous_executions: Vec<ExecutionFact>,
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
mod scripts;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExecution {
    pub id: String,
    #[serde(flatten)]
    pub target: ExecutionTarget,
    pub zone: String,
    #[serde(default)]
    pub input_rule: Option<Vec<crate::scheduler::maps::InputRule>>,
    #[serde(default, deserialize_with = "crate::scheduler::optional_object_array")]
    pub output_rule: Option<Vec<crate::scheduler::maps::OutputRule>>,
    #[serde(default, deserialize_with = "crate::scheduler::optional_object_array")]
    pub inputs: Option<Vec<crate::scheduler::maps::ResolvedFile>>,
    #[serde(default)]
    pub report_policy: crate::scheduler::maps::ReportPolicy,
    #[serde(default)]
    pub auto_close: bool,
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
    pub id: String,
    pub at: u64,
    pub actor: ChatReference,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Run {
    pub id: String,
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
    map_operations: Mutex<BTreeMap<String, std::sync::Arc<Mutex<()>>>>,
    registered_maps: Mutex<BTreeMap<String, String>>,
    // Serializes reference publication with history scans/removal, never host effects.
    history_references: Mutex<()>,
    deleted: Mutex<std::collections::BTreeSet<String>>,
}
impl Runs {
    pub(crate) async fn map_lock(&self, map: &str) -> std::sync::Arc<Mutex<()>> {
        self.map_operations.lock().await.entry(map.to_owned()).or_default().clone()
    }

    async fn ensure_retained(&self, id: &str) -> Result<(), String> {
        if self.deleted.lock().await.contains(id) { return Err("Run 历史已删除。".into()); }
        Ok(())
    }

    /// Caller holds this Map's registration gate, never a Chat/operation lock.
    pub(crate) async fn delete_map_history(&self, state: &AppState, map: &str) -> Result<(), String> {
        let _references = self.history_references.lock().await;
        let mut ids: std::collections::BTreeSet<String> = self.registered_maps.lock().await.iter()
            .filter(|(_, name)| name.as_str() == map).map(|(id, _)| id.clone()).collect();
        let root = std::path::absolute(&state.run_directory).map_err(|e| e.to_string())?;
        let candidates = self.files.with_async(&[root.clone()], move |access| Box::pin(async move {
        for ancestor in root.ancestors() {
            match access.symlink_metadata(ancestor) {
                Ok(metadata) if crate::zones::browse::is_link_like(&metadata) || !metadata.is_dir() =>
                    return Err(format!("Run 目录不能经过链接或非目录：{}", ancestor.display())),
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                Err(e) => return Err(e.to_string()),
            }
        }
        let mut candidates = Vec::new();
        match access.read_dir(&root) {
            Ok(entries) => for entry in entries {
                let metadata = access.symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
                if crate::zones::browse::is_link_like(&metadata) {
                    return Err(format!("无法安全检查 Run 关联：{} 是链接或重解析点。", entry.path().display()));
                }
                if !metadata.is_dir() { continue; }
                let id = entry.file_name().into_string().map_err(|_| "Run 目录名无效。".to_owned())?;
                if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') { continue; }
                candidates.push(id);
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.to_string()),
        }
        candidates.sort_unstable();
        Ok::<_, String>(candidates)
        })).await.map_err(|e| e.to_string())??;
        let mut records = Vec::with_capacity(candidates.len());
        for id in candidates {
            let paths = Self::paths(state, &id);
            let directory = paths[0].parent().unwrap().to_path_buf();
            let record = self.files.with_async(&[directory.clone()], move |access| Box::pin(async move {
                // Detail JSONL may be damaged, but the association must be fully readable.
                validate_history_tree(access, &directory)?;
                access.read_json::<Run>(&paths[0])
            })).await.map_err(|e| e.to_string())??;
            if record.id != id { return Err(format!("Run {id} 保存身份不一致：{}。", record.id)); }
            if record.map == map { ids.insert(id); }
            records.push(record);
        }
        // Check the complete selection before removing anything. Either side
        // alone is sufficient evidence; a missing counterpart is not permission
        // to delete, and retired executions keep their original child links.
        for run in &records {
            if let Some(parent) = &run.parent {
                if ids.contains(&run.id) || ids.contains(&parent.run_id) {
                    return Err(format!("Run 历史仍有父子引用，禁止删除：父 Run {} / node {} / execution {} → child Run {}。", parent.run_id, parent.node_id, parent.execution_id, run.id));
                }
            }
            for (node_id, node) in &run.nodes {
                for execution in node.executions() {
                    if let Some(child) = execution.child_run_id() {
                        if ids.contains(&run.id) || ids.contains(child) {
                            return Err(format!("Run 历史仍有父子引用，禁止删除：父 Run {} / node {} / execution {} → child Run {}。", run.id, node_id, execution.id, child));
                        }
                    }
                }
            }
        }
        for id in ids {
            let paths = Self::paths(state, &id);
            let directory = paths[0].parent().unwrap().to_path_buf();
            let owned = state.clone();
            self.files.with_async(&[directory.clone()], move |access| Box::pin(async move {
                match access.symlink_metadata(&directory) {
                    Ok(_) => validate_history_tree(access, &directory)?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                    Err(e) => return Err(e.to_string()),
                }
                // This scope also excludes launch initialization and late saves.
                owned.runs.deleted.lock().await.insert(id.clone());
                match access.remove_dir_all(&directory) {
                    Ok(()) => {},
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                    Err(e) => return Err(format!("Run {id} 历史删除失败：{e}")),
                }
                owned.runs.active.lock().await.remove(&id);
                owned.runs.registered_maps.lock().await.remove(&id);
                owned.runs.changed(&id, false).await;
                Ok::<_, String>(())
            })).await.map_err(|e| e.to_string())??;
        }
        Ok(())
    }

    fn paths(state: &AppState, id: &str) -> [PathBuf; 3] {
        let root = state.run_directory.join(id);
        [
            root.join("run.json"),
            root.join("history.jsonl"),
            root.join("messages.jsonl"),
        ]
    }
    /// Recover role from persisted execution identity, including retired attempts.
    pub(crate) async fn chat_role(&self, state: &AppState, session_id: &str, task_id: &str, agent: crate::tasks::Agent) -> Result<Option<crate::tasks::catalog::ExecutionRole>, String> {
        self.chat_role_with_current(state, session_id, task_id, agent, None).await
    }

    // A dispatch owns its current Run operation gate. Use that committed
    // snapshot for identity checks; a concurrent storage failure must still be
    // reported at association, with the actual newly started Chat preserved.
    async fn chat_role_with_current(&self, state: &AppState, session_id: &str, task_id: &str, agent: crate::tasks::Agent, current: Option<&Run>) -> Result<Option<crate::tasks::catalog::ExecutionRole>, String> {
        let root = state.run_directory.clone();
        let entries = match self.files.with_async(&[root.clone()], move |access| Box::pin(async move { access.read_dir(&root) })).await.map_err(|e| e.to_string())? {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let mut managers = Vec::new();
        let mut nodes = Vec::new();
        for entry in entries {
            if !entry.file_type().map_err(|error| error.to_string())?.is_dir() { continue; }
            let id = entry.file_name().to_string_lossy().into_owned();
            let run = match current.filter(|run| run.id == id) {
                Some(run) => run.clone(),
                None => self.read(state, &id).await?,
            };
            for (node_id, node) in &run.nodes {
                for execution in node.executions().filter(|execution| execution.chat().is_some_and(|chat| chat.session_id == session_id)) {
                    if execution.task_id() != Some(task_id) || execution.agent() != Some(agent) {
                        return Err("原生 Chat 的持久执行关联与来源 Task 或 agent 不一致。".into());
                    }
                    nodes.push((run.clone(), node_id.clone(), execution.clone()));
                }
            }
            if run.manager.as_ref().is_some_and(|chat| chat.session_id == session_id) {
                if run.manager_task_id != task_id { return Err("原生 Chat 的持久角色关联与来源 Task 不一致。".into()); }
                managers.push(run);
            }
        }
        use crate::tasks::catalog::ExecutionRole;
        match (managers.as_slice(), nodes.as_slice()) {
            ([], []) => Ok(None),
            ([manager], []) if manager.parent.is_none() => Ok(Some(ExecutionRole::Manager)),
            ([], [(_, _, execution)]) if execution.child_run_id().is_none() => Ok(Some(ExecutionRole::Node)),
            ([child], [(parent, node_id, execution)]) if nested_pair(child, parent, node_id, execution) => Ok(Some(ExecutionRole::NestedManager)),
            _ => Err("原生 Chat 的持久执行关联不唯一或缺少合法父子关系。".into()),
        }
    }

    async fn read(&self, state: &AppState, id: &str) -> Result<Run, String> {
        let paths = Self::paths(state, id);
        let owned = state.clone();
        let id = id.to_owned();
        self.files.with_async(&paths.clone(), move |access| Box::pin(async move {
            owned.runs.ensure_retained(&id).await?;
            // Required files are never silently recreated. JSONL is independently readable.
            for path in &paths[1..] {
                access.check_readable(path).map_err(|error| format!("{}：{error}", path.display()))?;
            }
            access.read_json(&paths[0])
        })).await.map_err(|e| e.to_string())?
    }
}
fn validate_history_tree(access: &crate::files::FileAccess, path: &std::path::Path) -> Result<(), String> {
    let metadata = access.symlink_metadata(path).map_err(|e| format!("{}：{e}", path.display()))?;
    if crate::zones::browse::is_link_like(&metadata) {
        return Err(format!("Run 历史不能包含链接或重解析点：{}", path.display()));
    }
    if metadata.is_dir() {
        for entry in access.read_dir(path).map_err(|e| e.to_string())? {
            validate_history_tree(access, &entry.path())?;
        }
    } else if !metadata.is_file() {
        return Err(format!("Run 历史文件类型无效：{}", path.display()));
    }
    Ok(())
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
    let map_lock = state.runs.map_lock(&map).await;
    let map_operation = map_lock.lock().await;
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
    if let Err(message) = preset.env.check_entrypoints() {
        return Err(error(
            FailureKind::Failed,
            "extension-entrypoint-invalid",
            &message,
        ));
    }
    let run = root_run(map, input, checked, &task, crate::chats::new_creation_id());
    register_root(&state, &run).await;
    drop(map_operation);
    launch_root(state, run, task, preset, None).await
}

/// Construct a root Run from the same preflight for UI and chief delegation.
/// Previewing allocates no records and never invents a parent execution.
pub(crate) fn root_run(
    map: String,
    input: crate::runs::preflight::RunPreflightRequest,
    checked: crate::runs::preflight::RunPreflight,
    task: &crate::tasks::Task,
    id: String,
) -> Run {
    Run {
        id: id.clone(),
        name: None,
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
            .node_ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    NodeState {
                        status: "pending".into(),
                        execution: None,
                        failure: None,
                        completion: None,
                        previous_executions: Vec::new(),
                    },
                )
            })
            .collect(),
    }
}

pub(crate) use startup::manager_prompt;

pub(crate) async fn register_root(state: &AppState, run: &Run) {
    state.runs.registered_maps.lock().await.insert(run.id.clone(), run.map.clone());
    state.runs.active.lock().await.insert(run.id.clone(), None);
}

pub(crate) async fn launch_root(
    state: AppState,
    run: Run,
    task: crate::tasks::Task,
    preset: crate::tasks::catalog::ResolvedPreset,
    delegation: Option<crate::chief_delegation::Launch>,
) -> RunResult {
    let id = run.id.clone();
    // Own accepted work independently of the browser request. A disconnected page
    // must not cancel a started native Chat or cause an implicit retry.
    let owned = state.clone();
    let result = tokio::spawn(async move { startup::manager(&owned, run, task, preset, delegation).await }).await;
    let failed = !matches!(result, Ok(Ok(())));
    if let Err(join) = result {
        if let Err(message) = startup::manager_interrupted(&state, &id, join).await {
            return Err(error(FailureKind::Failed, "run-read-failed", &message));
        }
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
    match manager_run(&state, &input.session_id).await {
        Ok(run) => match snapshot(&state, &run, true).await {
            Ok(value) => Ok(RunOutcome::Value(value)),
            Err(message) => Err(error(FailureKind::Failed, "definition-read-failed", &message)),
        },
        Err(response) => Err(response),
    }
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

fn blocked(run: &Run, id: &str, dependencies: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut reasons = Vec::new();
    if run.status != "running" { reasons.push(format!("Run 状态为 {}", run.status)); }
    if !run.inputs_ready { reasons.push("Run 输入尚未由管理者检查并确认；请先 prepare_inputs。".into()); }
    match run.nodes.get(id) {
        None => reasons.push("node 不在本次 Run 中".into()),
        Some(node) if node.status != "pending" => reasons.push(format!("node 状态为 {}，仅 pending 可派发", node.status)),
        _ => {}
    }
    match dependencies.get(id) {
        None => reasons.push("node 不在当前 Map 中".into()),
        Some(ids) => for dependency in ids {
            if !run.nodes.get(dependency).is_some_and(|node| node.status == "completed") {
                reasons.push(format!("前置 {dependency} 尚未 completed"));
            }
        }
    }
    reasons
}
async fn snapshot(state: &AppState, run: &Run, include_reports: bool) -> Result<serde_json::Value, String> {
    let dependencies = crate::scheduler::maps::node_dependencies(&state.map_directory, &run.map)?;
    let blocked_by: BTreeMap<_, _> = run.nodes.keys().map(|id| (id, blocked(run, id, &dependencies))).collect();
    let ready: Vec<_> = blocked_by.iter().filter(|(_, reasons)| reasons.is_empty()).map(|(id, _)| *id).collect();
    let mut value = reports::summary(state, run, include_reports).await?;
    value["snapshot"] = serde_json::to_value(run).map_err(|e| e.to_string())?;
    value["ready"] = serde_json::json!(ready);
    value["blocked"] = serde_json::json!(blocked_by);
    value["canFinish"] = serde_json::json!(matches!(run.status.as_str(), "running" | "paused") && run.nodes.values().all(|node| node.status == "completed"));
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchRequest {
    pub(crate) session_id: String,
    pub(crate) nodes: Vec<String>,
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
        let mut results = Vec::with_capacity(input.nodes.len());
        for id in input.nodes {
            let dependencies = crate::scheduler::maps::node_dependencies(&state.map_directory, &run.map)?;
            let reasons = blocked(&run, &id, &dependencies);
            if !reasons.is_empty() {
                results.push(serde_json::json!({"nodeId":id,"status":"rejected","reasons":reasons}));
                continue;
            }
            results.push(startup::node(&state, &mut run, &id, None).await);
        }
        let mut response = snapshot(&state, &run, true).await?;
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
        owned.runs.ensure_retained(&run.id).await?;
        access.replace(&paths[0], &serde_json::to_vec_pretty(&run).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
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
