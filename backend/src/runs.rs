//! Run startup, manager queries, explicit node dispatch and handoff/finish.
//! Manager startup errors are process-local; node dispatch failures are persisted.
use crate::{AppState, files::FileStore};
use axum::{
    Json,
    extract::{ConnectInfo, Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
mod reports;
mod recovery;
mod retry;
mod startup;
pub use retry::manager_retry;
pub use recovery::{manager_fail, manager_recover, manager_reconcile_dispatch};
mod completion;
mod operation;
mod collect;
pub use collect::manager_collect;
pub use reports::node_report;
pub use completion::{manager_complete, manager_finish, manager_pause, manager_resume};

#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeExecution {
    pub id: String,
    pub task_id: String,
    pub agent: crate::tasks::Agent,
    pub zone: String,
    #[serde(default)]
    pub report_policy: crate::scheduler::ReportPolicy,
    #[serde(default)]
    pub auto_close: bool,
    pub chat: Option<ChatReference>,
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
    pub zone: String,
    pub workspace_name: String,
    pub user_input: String,
    pub manager: Option<ChatReference>,
    pub status: String,
    pub finished: Option<Finished>,
    pub nodes: BTreeMap<String, NodeState>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    pub records_initialized: bool,
    pub workspace_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_started: bool,
    pub session_id: Option<String>,
    pub association_saved: bool,
    pub input_attempted: bool,
    pub input_submitted: bool,
}
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
        Ok::<_, String>(candidates)
        })).await.map_err(|e| e.to_string())??;
        for id in candidates {
            let paths = Self::paths(state, &id);
            let directory = paths[0].parent().unwrap().to_path_buf();
            let map = map.to_owned();
            let associated = self.files.with_async(&[directory.clone()], move |access| Box::pin(async move {
                // Damaged/missing detail files must not prevent removing safely associated history.
                validate_history_tree(access, &directory)?;
                let record: serde_json::Value = access.read_json(&paths[0])?;
                Ok::<_, String>(record.get("map").and_then(|v| v.as_str()) == Some(map.as_str()))
            })).await.map_err(|e| e.to_string())??;
            if associated { ids.insert(id); }
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
        let root = state.run_directory.clone();
        let entries = match self.files.with_async(&[root.clone()], move |access| Box::pin(async move { access.read_dir(&root) })).await.map_err(|e| e.to_string())? {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let mut role = None;
        for entry in entries {
            if !entry.file_type().map_err(|error| error.to_string())?.is_dir() { continue; }
            let id = entry.file_name().to_string_lossy().into_owned();
            let run = self.read(state, &id).await?;
            let manager = run.manager.as_ref().is_some_and(|chat| chat.session_id == session_id);
            let nodes = run.nodes.values().flat_map(NodeState::executions)
                .filter(|execution| execution.chat.as_ref().is_some_and(|chat| chat.session_id == session_id));
            for execution in nodes {
                if role.is_some() || manager || execution.task_id != task_id || execution.agent != agent {
                    return Err("原生 Chat 的持久执行关联不唯一或与来源 Task 不一致。".into());
                }
                role = Some(crate::tasks::catalog::ExecutionRole::Node);
            }
            if manager {
                if role.is_some() || run.manager_task_id != task_id { return Err("原生 Chat 的持久角色关联不唯一或与来源 Task 不一致。".into()); }
                role = Some(crate::tasks::catalog::ExecutionRole::Manager);
            }
        }
        Ok(role)
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

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"code":code,"message":message})),
    )
        .into_response()
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
pub async fn list_runs(State(state): State<AppState>) -> Response {
    let active = state.runs.active.lock().await.clone();
    let mut ids: std::collections::BTreeSet<String> = active.keys().cloned().collect();
    let root = state.run_directory.clone();
    let entries = state.runs.files.with_async(&[root.clone()], move |access| Box::pin(async move { access.read_dir(&root) })).await.and_then(|entries| entries);
    match entries {
        Ok(entries) => {
            for entry in entries {
                let kind = match entry.file_type() { Ok(kind) => kind, Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, "runs-read-failed", &e.to_string()) };
                let Some(id) = entry.file_name().to_str().map(str::to_owned) else { continue };
                if kind.is_dir() && !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') { ids.insert(id); }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, "runs-read-failed", &e.to_string()),
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
    Json(serde_json::json!({"runs":runs,"pollIntervalSeconds":state.workflow_poll_interval_seconds})).into_response()
}

pub async fn get_run(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return error(StatusCode::BAD_REQUEST, "run-id-invalid", "Run 标识无效。");
    }
    match detail(&state, &id).await {
        Ok(value) => Json(value).into_response(),
        Err(message) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "run-read-failed",
            &message,
        ),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameRequest {
    name: String,
}

pub async fn rename_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return error(StatusCode::BAD_REQUEST, "run-id-invalid", "Run 标识无效。");
    }
    let input: RenameRequest = match crate::chat_command_body(request, "重命名 Run").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let name = input.name.trim();
    if name.is_empty() {
        return error(StatusCode::BAD_REQUEST, "run-name-required", "Run 名称不能为空。");
    }
    let lock = state.runs.operations.lock().await.entry(id.clone()).or_default().clone();
    let _operation = lock.lock().await;
    // Read after taking the same operation lock used by workflow mutations.
    // Never authorize from management status: saved historical Runs are editable.
    let mut run = match state.runs.read(&state, &id).await {
        Ok(run) => run,
        Err(message) => return error(StatusCode::INTERNAL_SERVER_ERROR, "run-read-failed", &message),
    };
    // Validate the same independent records as GET before changing readable history.
    if let Err(message) = detail(&state, &id).await {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "run-read-failed", &message);
    }
    run.name = Some(name.to_owned());
    if let Err(message) = save_run(&state, &run).await {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "run-save-failed", &message);
    }
    match detail(&state, &id).await {
        Ok(value) => Json(value).into_response(),
        Err(message) => error(StatusCode::INTERNAL_SERVER_ERROR, "run-read-failed", &message),
    }
}

pub async fn start_run(
    State(state): State<AppState>,
    Path(map): Path<String>,
    request: Request,
) -> Response {
    let input: crate::scheduler::RunPreflightRequest =
        match crate::chat_command_body(request, "发起 Run").await {
            Ok(v) => v,
            Err(e) => return e,
        };
    let map_lock = state.runs.map_lock(&map).await;
    let map_operation = map_lock.lock().await;
    let checked = match crate::scheduler::check_run_preflight(&state, &map, &input).await {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let task = match state.tasks.get(&input.manager_task_id).await {
        Ok(v) => v,
        Err(e) => return crate::task_error_response(e),
    };
    let preset = match state
        .tasks
        .catalog()
        .and_then(|catalog| catalog.resolve(&task))
    {
        Ok(v) => v,
        Err(e) => return crate::task_error_response(e),
    };
    if let Err(message) = preset.env.check_entrypoints() {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "extension-entrypoint-invalid",
            &message,
        );
    }
    let id = crate::chats::new_creation_id();
    let run = Run {
        id: id.clone(),
        name: None,
        map,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        manager_task_id: task.id.clone(),
        zone: input.zone,
        workspace_name: checked.workspace_name,
        user_input: input.user_input,
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
    };
    state.runs.registered_maps.lock().await.insert(id.clone(), run.map.clone());
    state.runs.active.lock().await.insert(id.clone(), None);
    drop(map_operation);
    // Own accepted work independently of the browser request. A disconnected page
    // must not cancel a started native Chat or cause an implicit retry.
    let owned = state.clone();
    let result = tokio::spawn(async move { startup::manager(&owned, run, task, preset).await }).await;
    let failed = !matches!(result, Ok(Ok(())));
    if let Err(join) = result {
        if let Err(message) = startup::manager_interrupted(&state, &id, join).await {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "run-read-failed", &message);
        }
    }
    match detail(&state, &id).await {
        Ok(value) => (
            if failed {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::CREATED
            },
            Json(value),
        )
            .into_response(),
        Err(message) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "run-read-failed",
            &message,
        ),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusRequest {
    session_id: String,
}
pub async fn manager_status(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "运行工具只接受同机来源。",
        );
    }
    let input: StatusRequest = match crate::chat_command_body(request, "运行状态").await {
        Ok(v) => v,
        Err(e) => return e,
    };
    match manager_run(&state, &input.session_id).await {
        Ok(run) => match snapshot(&state, &run, true).await {
            Ok(value) => Json(value).into_response(),
            Err(message) => error(StatusCode::INTERNAL_SERVER_ERROR, "definition-read-failed", &message),
        },
        Err(response) => response,
    }
}

async fn manager_run(state: &AppState, session_id: &str) -> Result<Run, Response> {
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
    let mut node_associated = false;
    for id in ids {
        let run = match state.runs.read(&state, &id).await {
            Ok(v) => v,
            Err(message) => {
                read_error.get_or_insert(message);
                continue;
            }
        };
        node_associated |= run.nodes.values().any(|node| {
            node.executions().any(|execution| {
                execution.chat.as_ref().is_some_and(|chat| chat.session_id == session_id)
            })
        });
        if run
            .manager
            .as_ref()
            .is_some_and(|manager| manager.session_id == session_id)
        {
            if matched.is_some() {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    "manager-ambiguous",
                    "原生身份关联不唯一。",
                ));
            }
            matched = Some(run);
        }
    }
    match matched {
        Some(_) if node_associated => Err(error(
            StatusCode::FORBIDDEN,
            "manager-ambiguous",
            "原生身份同时关联节点执行与管理者，归属不唯一。",
        )),
        Some(run) => Ok(run),
        None if read_error.is_some() => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "run-read-failed",
            read_error.as_deref().unwrap(),
        )),
        None => Err(error(
            StatusCode::FORBIDDEN,
            "manager-unassociated",
            "当前原生身份没有唯一的管理者归属。",
        )),
    }
}

fn blocked(run: &Run, id: &str, dependencies: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut reasons = Vec::new();
    if run.status != "running" { reasons.push(format!("Run 状态为 {}", run.status)); }
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
    let dependencies = crate::scheduler::node_dependencies(&state.map_directory, &run.map)?;
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
struct DispatchRequest {
    session_id: String,
    nodes: Vec<String>,
}
pub async fn manager_dispatch(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: DispatchRequest = match crate::chat_command_body(request, "派发节点").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    // Accepted external work outlives a disconnected tool request. The business
    // lock spans the batch, while file locks never span host calls.
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        let mut results = Vec::with_capacity(input.nodes.len());
        for id in input.nodes {
            let dependencies = crate::scheduler::node_dependencies(&state.map_directory, &run.map)?;
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
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "dispatch-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "dispatch-internal", &join.to_string()),
    }
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

