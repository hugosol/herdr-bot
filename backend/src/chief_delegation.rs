//! Chief Task-Bot delegation: preview facts, launch through Chat, and persist
//! provenance before sending business input. No synthetic workflow identities.
use crate::{
    AppState,
    chats::{self, ChatCreationView},
    files::FileStore,
    tasks::{Task, TaskType, catalog::ResolvedPreset},
};
use axum::{
    Json,
    extract::Request,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Material {
    id: String,
    zone_id: String,
    path: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DispatchRequest {
    session_id: String,
    bot_id: String,
    zone_id: String,
    input: String,
    #[serde(default)]
    materials: Vec<Material>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Facts {
    request: DispatchRequest,
    bot: Value,
    task: Task,
    cwd: String,
    name: String,
    materials: Vec<Material>,
    prompt: String,
}
struct Prepared {
    facts: Facts,
    preset: ResolvedPreset,
    chat_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    id: String,
    facts: Facts,
    creation: ChatCreationView,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

pub(crate) struct Delegations {
    file: PathBuf,
    files: FileStore,
    previews: Mutex<HashMap<String, Prepared>>,
}
impl Delegations {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            files: FileStore,
            previews: Mutex::new(HashMap::new()),
        }
    }
    fn access<T>(
        &self,
        write: bool,
        action: impl FnOnce(&mut Vec<Record>) -> io::Result<T>,
    ) -> io::Result<T> {
        self.files
            .with(std::slice::from_ref(&self.file), |access| {
                let mut records = match access.read(&self.file) {
                    Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
                    Err(error) => return Err(error),
                };
                let result = action(&mut records)?;
                if write {
                    if let Some(parent) = self.file.parent() {
                        access.create_dir_all(parent)?;
                    }
                    access.replace(
                        &self.file,
                        &serde_json::to_vec_pretty(&records).map_err(io::Error::other)?,
                    )?;
                }
                Ok(result)
            })?
    }
    fn insert(&self, record: Record) -> io::Result<()> {
        self.access(true, |records| {
            records.push(record);
            Ok(())
        })
    }
    pub fn bind(&self, id: &str, session_id: &str) -> io::Result<()> {
        self.access(true, |records| {
            if records
                .iter()
                .any(|r| r.id != id && r.session_id.as_deref() == Some(session_id))
            {
                return Err(io::Error::other(
                    "native Chat already belongs to another delegation",
                ));
            }
            let record = records
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| io::Error::other("delegation missing"))?;
            if record
                .session_id
                .as_deref()
                .is_some_and(|old| old != session_id)
            {
                return Err(io::Error::other("delegation identity changed"));
            }
            record.session_id = Some(session_id.into());
            Ok(())
        })
    }
    fn finish(&self, creation: ChatCreationView) -> io::Result<()> {
        self.access(true, |records| {
            let record = records
                .iter_mut()
                .find(|r| r.id == creation.id)
                .ok_or_else(|| io::Error::other("delegation missing"))?;
            record.creation = creation;
            Ok(())
        })
    }
}

pub(crate) struct Launch {
    pub store: Arc<Delegations>,
    pub id: String,
    pub prompt: String,
}
fn rejected(code: &'static str, message: impl Into<String>) -> Response {
    crate::rejected_owned(StatusCode::CONFLICT, code, message.into())
}
fn storage(error: io::Error) -> Response {
    crate::rejected_owned(
        StatusCode::INTERNAL_SERVER_ERROR,
        "delegation-storage-failed",
        format!("委派记录无法可靠保存或读取：{error}"),
    )
}
async fn resolve(
    state: &AppState,
    request: DispatchRequest,
    id: &str,
    chat_id: &str,
) -> Result<Prepared, Response> {
    if request.input.trim().is_empty() {
        return Err(rejected(
            "delegation-input-required",
            "必须明确本次业务输入。",
        ));
    }
    let cwd = crate::chat_zone(state, &request.zone_id).await?;
    let (bot, task_id) = crate::scheduler::delegation_bot(&state.bot_directory, &request.bot_id)?;
    let task = state
        .tasks
        .get(&task_id)
        .await
        .map_err(crate::task_error_response)?;
    if task.task_type != TaskType::Normal {
        return Err(rejected(
            "delegation-task-role",
            "Task 型 Bot 只能委派普通 Task；chief 与 manager 不可冒充独立执行者。",
        ));
    }
    state
        .tasks
        .validate_suite(&task)
        .map_err(crate::task_error_response)?;
    let preset = state
        .tasks
        .catalog()
        .and_then(|catalog| catalog.resolve(&task))
        .map_err(crate::task_error_response)?;
    preset
        .env
        .check_entrypoints()
        .map_err(|error| rejected("extension-entrypoint-invalid", error))?;
    let mut materials = Vec::with_capacity(request.materials.len());
    for material in &request.materials {
        if material.id.trim().is_empty() || materials.iter().any(|m: &Material| m.id == material.id)
        {
            return Err(rejected(
                "delegation-material-invalid",
                "材料 ID 必须非空且唯一。",
            ));
        }
        let root = crate::chat_zone(state, &material.zone_id).await?;
        let path = crate::zones::browse::checked_file(&root, &material.path)
            .map_err(crate::browse_error_response)?;
        crate::scheduler::check_input_file(&path)
            .map_err(|error| rejected("delegation-material-invalid", error))?;
        materials.push(Material {
            id: material.id.clone(),
            zone_id: material.zone_id.clone(),
            path,
        });
    }
    let name = chats::render_chat_name(&task.chat_name, &cwd, &task, chat_id);
    if name.trim().is_empty() || name.contains(['\n', '\r']) {
        return Err(rejected(
            "chat-name-required",
            "Task 的命名规则必须生成有效单行名称。",
        ));
    }
    let context = json!({"delegationId":id,"botId":request.bot_id,"sourceChiefSessionId":request.session_id,"zoneId":request.zone_id,"cwd":cwd,"materials":materials});
    let business_input = format!(
        "{}\n\n[Chief delegation]\n{}",
        request.input,
        serde_json::to_string_pretty(&context).expect("context serialization")
    );
    let snapshot = state
        .prompts
        .snapshot()
        .map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
    let fixed = snapshot
        .render(crate::prompts::Field::TaskPrompt, &task.trigger.prompt)
        .map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
    let prompt = chats::compose_prompt(
        &fixed,
        preset.trigger.native_skill.as_deref(),
        &business_input,
        &snapshot.instructions(&task.trigger.prompt),
    );
    Ok(Prepared {
        facts: Facts {
            request,
            bot,
            task,
            cwd,
            name,
            materials,
            prompt,
        },
        preset,
        chat_id: chat_id.to_owned(),
    })
}

pub(crate) async fn preview(State(state): State<AppState>, request: Request) -> Response {
    let input: DispatchRequest = match crate::chat_command_body(request, "预览 Task Bot 委派").await
    {
        Ok(input) => input,
        Err(error) => return error,
    };
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return error.response();
    }
    let id = chats::new_creation_id();
    let prepared = match resolve(&state, input, &id, &chats::new_chat_id()).await {
        Ok(prepared) => prepared,
        Err(error) => return error,
    };
    let response = json!({"previewId":id,"facts":prepared.facts});
    state
        .delegations
        .previews
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, prepared);
    Json(response).into_response()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommitRequest {
    session_id: String,
    preview_id: String,
}
pub(crate) async fn commit(State(state): State<AppState>, request: Request) -> Response {
    let input: CommitRequest = match crate::chat_command_body(request, "发起 Task Bot 委派").await
    {
        Ok(input) => input,
        Err(error) => return error,
    };
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return error.response();
    }
    let prepared = state
        .delegations
        .previews
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&input.preview_id);
    let Some(mut prepared) = prepared else {
        return rejected(
            "delegation-preview-missing",
            "预览不存在或已使用；请重新核实并确认。",
        );
    };
    if prepared.facts.request.session_id != input.session_id {
        return rejected("chief-not-current", "预览不属于当前调用会话。");
    }
    let fresh = match resolve(
        &state,
        prepared.facts.request.clone(),
        &input.preview_id,
        &prepared.chat_id,
    )
    .await
    {
        Ok(fresh) => fresh,
        Err(error) => return error,
    };
    if fresh.facts != prepared.facts
        || chats::agent_launch_args(&fresh.preset.env, Some(&fresh.preset.trigger))
            != chats::agent_launch_args(&prepared.preset.env, Some(&prepared.preset.trigger))
    {
        return rejected(
            "delegation-preview-stale",
            "目标、Task、zone、材料地址或完整输入已变化；本次未发起，请重新预览并确认。",
        );
    }
    // Approval pins these values. Later edits cannot silently redirect the job.
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return error.response();
    }
    prepared.preset.env.suite_args = match state.tasks.prepare_environment(&prepared.facts.task) {
        Ok(args) => args,
        Err(error) => return crate::task_error_response(error),
    };
    let view = ChatCreationView::starting(
        input.preview_id.clone(),
        prepared.facts.name.clone(),
        prepared.facts.cwd.clone(),
        prepared.facts.task.id.clone(),
    );
    if let Err(error) = state.delegations.insert(Record {
        id: input.preview_id.clone(),
        facts: prepared.facts.clone(),
        creation: view.clone(),
        session_id: None,
    }) {
        return storage(error);
    }
    let job = state.chat_creations.insert(view.clone()).await;
    let store = Arc::clone(&state.delegations);
    tokio::spawn(async move {
        let launch = Launch {
            store: Arc::clone(&store),
            id: input.preview_id,
            prompt: prepared.facts.prompt,
        };
        chats::run_creation(
            Arc::clone(&state.herdr),
            Arc::clone(&state.history),
            state.prompts.clone(),
            Arc::clone(&job),
            prepared.facts.task,
            prepared.preset,
            prepared.facts.cwd,
            String::new(),
            state.herdr.workspace_name().to_owned(),
            None,
            Some(launch),
        )
        .await;
        let final_view = job.lock().await.clone();
        if let Err(error) = store.finish(final_view) {
            let mut view = job.lock().await;
            let previous = view.failure.clone();
            view.stage = chats::Stage::Failed;
            view.failure = Some(chats::Failure {
                stage: chats::FailureStage::Submit,
                code: "delegation-storage-failed".into(),
                message: format!(
                    "启动现场保留，但最终委派状态未保存：{error}；先前失败：{previous:?}。不要重试业务启动。"
                ),
            });
        }
    });
    (StatusCode::ACCEPTED, Json(view)).into_response()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SenderQuery {
    session_id: String,
}
pub(crate) async fn list(
    State(state): State<AppState>,
    Query(query): Query<SenderQuery>,
) -> Response {
    match state.delegations.access(false, |records| {
        Ok(records
            .iter()
            .filter(|r| r.session_id.as_deref() == Some(&query.session_id))
            .cloned()
            .collect::<Vec<_>>())
    }) {
        Ok(records) => Json(json!({"delegations":records})).into_response(),
        Err(error) => storage(error),
    }
}
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.delegations.access(false, |records| {
        Ok(records.iter().find(|r| r.id == id).cloned())
    }) {
        Ok(Some(record)) => Json(record).into_response(),
        Ok(None) => crate::rejected_owned(
            StatusCode::NOT_FOUND,
            "delegation-not-found",
            "委派记录不存在。".into(),
        ),
        Err(error) => storage(error),
    }
}
