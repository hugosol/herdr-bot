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
    fs,
    net::SocketAddr,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
mod reports;
mod recovery;
mod retry;
pub use retry::manager_retry;
pub use recovery::{manager_fail, manager_recover, manager_reconcile_dispatch};
mod completion;
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
    /// Recover role from persisted execution identity, including retired attempts.
    pub(crate) async fn chat_role(&self, state: &AppState, session_id: &str, task_id: &str, agent: crate::tasks::Agent) -> Result<Option<crate::catalog::ExecutionRole>, String> {
        let entries = match fs::read_dir(&state.run_directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let mut role = None;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
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
                role = Some(crate::catalog::ExecutionRole::Node);
            }
            if manager {
                if role.is_some() || run.manager_task_id != task_id { return Err("原生 Chat 的持久角色关联不唯一或与来源 Task 不一致。".into()); }
                role = Some(crate::catalog::ExecutionRole::Manager);
            }
        }
        Ok(role)
    }

    async fn read(&self, state: &AppState, id: &str) -> Result<Run, String> {
        let paths = Self::paths(state, id);
        let _guards = self.files.lock(&paths).await;
        // Required files are never silently recreated. JSONL is independently readable.
        for path in &paths[1..] {
            fs::File::open(path).map_err(|error| format!("{}：{error}", path.display()))?;
        }
        FileStore::read_json(&paths[0])
    }
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
        {
            let _guards = state.runs.files.lock(&paths).await;
            FileStore::read_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1])?;
        }
        let reports = reports::summary(state, run, true).await?;
        value["reports"] = reports["reports"].clone();
        value["pendingReports"] = reports["pendingReports"].clone();
    }
    Ok(value)
}
pub async fn list_runs(State(state): State<AppState>) -> Response {
    let active = state.runs.active.lock().await.clone();
    let mut ids: std::collections::BTreeSet<String> = active.keys().cloned().collect();
    match fs::read_dir(&state.run_directory) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry { Ok(entry) => entry, Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, "runs-read-failed", &e.to_string()) };
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
    state.runs.active.lock().await.insert(id.clone(), None);
    // Own accepted work independently of the browser request. A disconnected page
    // must not cancel a started native Chat or cause an implicit retry.
    let owned = state.clone();
    let result = tokio::spawn(async move { launch(&owned, run, task, preset).await }).await;
    let failed = !matches!(result, Ok(Ok(())));
    if let Err(join) = result {
        state.runs.active.lock().await.insert(
            id.clone(),
            Some(StartupFailure {
                stage: "internal".into(),
                message: join.to_string(),
                effects: Some(Effects::default()),
            }),
        );
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
async fn launch(
    state: &AppState,
    mut run: Run,
    task: crate::tasks::Task,
    preset: crate::catalog::ResolvedPreset,
) -> Result<(), ()> {
    let mut effects = Effects::default();
    let mut stage = "initialize";
    let result: Result<(), String> = async {
        let paths = Runs::paths(state, &run.id);
        {
            let _guards = state.runs.files.lock(&paths).await;
            fs::create_dir_all(paths[0].parent().unwrap()).map_err(|e| e.to_string())?;
            FileStore::create(
                &paths[0],
                &serde_json::to_vec_pretty(&run).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            FileStore::create(&paths[1], b"").map_err(|e| e.to_string())?;
            FileStore::create(&paths[2], b"").map_err(|e| e.to_string())?;
        }
        effects.records_initialized = true;
        stage = "workspace";
        let workspace = state
            .herdr
            .workspace(&run.workspace_name)
            .await
            .map_err(|e| e.message())?;
        effects.workspace_id = Some(workspace.id.clone());
        let name = crate::chats::render_chat_name(
            &task.chat_name,
            &run.zone,
            &task,
            &crate::chats::new_chat_id(),
        );
        stage = "tab";
        let tab = state
            .herdr
            .create_tab(&workspace.id, Some(&run.zone), &name)
            .await
            .map_err(|e| e.message())?;
        effects.tab_id = Some(tab.tab_id.clone());
        effects.pane_id = Some(tab.pane_id.clone());
        let agent = crate::chats::new_agent_name();
        effects.agent_name = Some(agent.clone());
        stage = "start";
        state
            .herdr
            .start_agent(
                &agent,
                task.agent.label(),
                &tab.pane_id,
                &crate::chats::agent_launch_args(&preset.env, Some(&preset.trigger)),
            )
            .await
            .map_err(|e| e.message())?;
        effects.agent_started = true;
        stage = "identity";
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let identity = tokio::time::timeout_at(
            deadline,
            state.herdr.started_session(&agent, task.agent.label(), deadline),
        )
        .await
        .map_err(|_| "获取原生身份超时".to_owned())?
        .map_err(|e| e.message())?;
        effects.session_id = Some(identity.session_id.clone());
        run.manager = Some(ChatReference {
            session_id: identity.session_id.clone(),
            workspace_id: workspace.id.clone(),
            tab_id: tab.tab_id,
            pane_id: tab.pane_id,
            agent_name: agent.clone(),
        });
        stage = "association";
        {
            let _guards = state.runs.files.lock(&paths).await;
            FileStore::replace(
                &paths[0],
                &serde_json::to_vec_pretty(&run).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        effects.association_saved = true;
        state
            .history
            .record_zone(
                &identity.session_id,
                &name,
                task.agent.label(),
                &identity.cwd,
                identity.session_file.as_deref(),
                Some(&workspace.id),
                &task.id,
                identity.startup_identity_path.as_deref(),
            )
            .await
            .map_err(|e| e.to_string())?;
        stage = "submit";
        let mut input = format!(
            "Run ID: {}\nMap: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}",
            run.id,
            state.map_directory.join(&run.map).display(),
            state.run_instructions.display(),
            paths[0].display(),
            paths[1].display(),
            paths[2].display()
        );
        if !run.user_input.is_empty() {
            input.push_str("\n\n");
            input.push_str(&run.user_input);
        }
        let text = crate::chats::compose_initial_input(
            &task,
            preset.trigger.native_skill.as_deref(),
            &input,
        );
        effects.input_attempted = true;
        state
            .herdr
            .prompt(&agent, &text)
            .await
            .map_err(|e| e.message())?;
        effects.input_submitted = true;
        Ok(())
    }
    .await;
    if let Err(message) = result {
        eprintln!(
            "[workflow] run={} manager-start stage={} effects={} error={}；已发生的外部效果未回滚，不自动重试。",
            run.id,
            stage,
            serde_json::to_string(&effects).unwrap_or_default(),
            message
        );
        state.runs.active.lock().await.insert(
            run.id,
            Some(StartupFailure {
                stage: stage.into(),
                message,
                effects: Some(effects),
            }),
        );
        return Err(());
    }
    Ok(())
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
            results.push(dispatch_one(&state, &mut run, &id, None).await);
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
    let _guards = state.runs.files.lock(&paths).await;
    FileStore::replace(&paths[0], &serde_json::to_vec_pretty(run).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    state.runs.changed(&run.id, false).await;
    Ok(())
}

async fn record_dispatch(state: &AppState, run: &Run, id: &str, retry_reason: Option<&str>) -> Result<(), String> {
    let paths = Runs::paths(state, &run.id);
    let _guards = state.runs.files.lock(&paths).await;
    let node = &run.nodes[id];
    let mut entry = serde_json::json!({"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"action":"dispatch","nodeId":id,"status":node.status,"execution":node.execution,"failure":node.failure});
    if let Some(reason) = retry_reason {
        entry["action"] = serde_json::json!("retry");
        entry["reason"] = serde_json::json!(reason);
        entry["actor"] = serde_json::json!(run.manager);
        if let Some(previous) = node.previous_executions.last() {
            entry["previousExecution"] = serde_json::json!(previous.execution);
            entry["previousFailure"] = serde_json::json!(previous.failure);
        }
    }
    FileStore::append_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1], &entry)
}

async fn dispatch_one(state: &AppState, run: &mut Run, id: &str, retry_reason: Option<&str>) -> serde_json::Value {
    let mut effects = Effects { records_initialized: true, ..Effects::default() };
    let mut stage = "resolve";
    let mut candidate = run.clone();
    if retry_reason.is_some() {
        let node = candidate.nodes.get_mut(id).unwrap();
        node.previous_executions.push(ExecutionFact { execution: node.execution.take(), failure: node.failure.take() });
        node.completion = None;
    }
    let result: Result<(), String> = async {
        let target = crate::scheduler::dispatch_target(state, &run.map, id).await?;
        let preset = state.tasks.catalog().and_then(|catalog| catalog.resolve_for_role(&target.task, crate::catalog::ExecutionRole::Node)).map_err(|e| e.message())?;
        preset.env.check_entrypoints()?;
        let node = candidate.nodes.get_mut(id).unwrap();
        node.execution = Some(NodeExecution { id: crate::chats::new_creation_id(), task_id: target.task.id.clone(), agent: target.task.agent, zone: target.zone.clone(), chat: None });
        stage = "workspace";
        let workspace_id = &run.manager.as_ref().ok_or("管理者没有真实 Chat 关联")?.workspace_id;
        let workspace = state.herdr.workspace_by_id(workspace_id).await.map_err(|e| e.message())?.ok_or("管理者实际 Workspace 不可用；不回退默认 Workspace")?;
        effects.workspace_id = Some(workspace.id.clone());
        stage = "tab";
        let name = crate::chats::render_chat_name(&target.task.chat_name, &target.zone, &target.task, &crate::chats::new_chat_id());
        let tab = state.herdr.create_tab(&workspace.id, Some(&target.zone), &name).await.map_err(|e| e.message())?;
        effects.tab_id = Some(tab.tab_id.clone());
        effects.pane_id = Some(tab.pane_id.clone());
        let agent = crate::chats::new_agent_name();
        effects.agent_name = Some(agent.clone());
        stage = "start";
        state.herdr.start_agent(&agent, target.task.agent.label(), &tab.pane_id, &crate::chats::agent_launch_args(&preset.env, Some(&preset.trigger))).await.map_err(|e| e.message())?;
        effects.agent_started = true;
        stage = "identity";
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let identity = tokio::time::timeout_at(deadline, state.herdr.started_session(&agent, target.task.agent.label(), deadline)).await.map_err(|_| "获取原生身份超时".to_owned())?.map_err(|e| e.message())?;
        effects.session_id = Some(identity.session_id.clone());
        if run.manager.as_ref().is_some_and(|chat| chat.session_id == identity.session_id) || run.nodes.values().any(|node| node.executions().any(|execution| execution.chat.as_ref().is_some_and(|chat| chat.session_id == identity.session_id))) {
            return Err("宿主未返回独立的新原生 Chat 身份".into());
        }
        if !crate::zones::same_path(&identity.cwd, &target.zone) {
            return Err(format!("原生 Chat cwd 与定义 zone 不一致：{}", identity.cwd));
        }
        node.execution.as_mut().unwrap().chat = Some(ChatReference { session_id: identity.session_id.clone(), workspace_id: workspace.id.clone(), tab_id: tab.tab_id, pane_id: tab.pane_id, agent_name: agent.clone() });
        stage = "association";
        save_run(state, &candidate).await?;
        *run = candidate.clone();
        effects.association_saved = true;
        stage = "history";
        state.history.record_zone(&identity.session_id, &name, target.task.agent.label(), &identity.cwd, identity.session_file.as_deref(), Some(&workspace.id), &target.task.id, identity.startup_identity_path.as_deref()).await.map_err(|e| e.to_string())?;
        stage = "submit";
        let paths = Runs::paths(state, &run.id);
        let input = format!("Node ID: {id}\nNode: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\n{}", target.title, state.run_instructions.display(), paths[0].display(), paths[1].display(), paths[2].display(), target.input);
        let text = crate::chats::compose_initial_input(&target.task, preset.trigger.native_skill.as_deref(), &input);
        effects.input_attempted = true;
        state.herdr.prompt(&agent, &text).await.map_err(|e| e.message())?;
        effects.input_submitted = true;
        candidate.nodes.get_mut(id).unwrap().status = "running".into();
        stage = "state";
        save_run(state, &candidate).await?;
        Ok(())
    }.await;
    if let Err(message) = result {
        let failure = StartupFailure { stage: stage.into(), message, effects: Some(effects.clone()) };
        let node = candidate.nodes.get_mut(id).unwrap();
        if retry_reason.is_some() && node.execution.is_none() {
            // Resolution failed before a target was usable. Retain a distinct
            // attempt referencing the failed source, never the old Chat.
            node.execution = node.previous_executions.last().and_then(|fact| fact.execution.as_ref()).map(|previous| NodeExecution {
                id: crate::chats::new_creation_id(), task_id: previous.task_id.clone(), agent: previous.agent, zone: previous.zone.clone(), chat: None,
            });
        }
        node.status = "failed".into();
        node.failure = Some(failure.clone());
        let execution = serde_json::json!(node.execution);
        let persistence_error = save_run(state, &candidate).await.err();
        let history_error = if persistence_error.is_none() {
            *run = candidate;
            record_dispatch(state, run, id, retry_reason).await.err()
        } else { None };
        eprintln!("[workflow] run={} node={} action={} execution={} stage={} effects={} error={} persistence-error={:?} history-error={:?}；已发生外部效果未回滚，不自动重试。", run.id, id, if retry_reason.is_some() { "retry-failed" } else { "dispatch-failed" }, execution, stage, serde_json::to_string(&effects).unwrap_or_default(), failure.message, persistence_error, history_error);
        let mut value = serde_json::json!({"nodeId":id,"status":"failed","execution":execution,"failure":failure,"effects":effects,"persistenceError":persistence_error});
        completion::history_feedback(&mut value, history_error);
        return value;
    }
    *run = candidate;
    let history_error = record_dispatch(state, run, id, retry_reason).await.err();
    if let Some(message) = &history_error {
        eprintln!("[workflow] run={} node={} dispatch-history error={}；状态已提交，未回滚。", run.id, id, message);
    }
    let mut value = serde_json::json!({"nodeId":id,"status":"running","effects":effects});
    completion::history_feedback(&mut value, history_error);
    value
}
