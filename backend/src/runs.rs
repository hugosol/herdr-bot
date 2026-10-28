//! Run startup and read-only facts. Startup errors are intentionally process-local.
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
    pub nodes: BTreeMap<String, NodeState>,
}
#[derive(Clone, Default, Serialize)]
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
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupFailure {
    pub stage: String,
    pub message: String,
    pub effects: Effects,
}
#[derive(Default)]
pub struct Runs {
    files: FileStore,
    active: Mutex<BTreeMap<String, Option<StartupFailure>>>,
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
    async fn read(&self, state: &AppState, id: &str) -> Result<Run, String> {
        let paths = Self::paths(state, id);
        let _guards = self.files.lock(&paths).await;
        // Required files are never silently recreated. JSONL is independently readable.
        for path in &paths[1..] {
            fs::File::open(path).map_err(|e| e.to_string())?;
        }
        serde_json::from_slice(&fs::read(&paths[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
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
    Ok(
        serde_json::json!({"id":id,"run":run,"managed":managed,"startupError":failure,"readError":read_error}),
    )
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
        nodes: checked
            .node_ids
            .into_iter()
            .map(|id| {
                (
                    id,
                    NodeState {
                        status: "pending".into(),
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
                effects: Effects::default(),
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
                "omp",
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
            state.herdr.native_session(&agent, "omp", deadline),
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
                "omp",
                &identity.cwd,
                identity.session_file.as_deref(),
                Some(&workspace.id),
                &task.id,
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
                effects,
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
    let ids = state
        .runs
        .active
        .lock()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut matched = None;
    for id in ids {
        let run = match state.runs.read(&state, &id).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        if run
            .manager
            .as_ref()
            .is_some_and(|manager| manager.session_id == input.session_id)
        {
            if matched.is_some() {
                return error(
                    StatusCode::FORBIDDEN,
                    "manager-ambiguous",
                    "原生身份关联不唯一。",
                );
            }
            matched = Some(run);
        }
    }
    match matched {
        Some(run) => Json(serde_json::json!({"snapshot":run})).into_response(),
        None => error(
            StatusCode::FORBIDDEN,
            "manager-unassociated",
            "当前原生身份没有唯一的管理者归属。",
        ),
    }
}
