//! Chief Bot delegation: preview facts, launch real Chats/Runs, and persist
//! provenance before sending business input. No synthetic workflow identities.
use crate::{
    AppState,
    chats::{self, ChatCreationView},
    files::FileStore,
    tasks::{Task, TaskType, catalog::ResolvedPreset},
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    manager_task_id: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    map: Option<Value>,
}
struct Prepared {
    facts: Facts,
    preset: ResolvedPreset,
    chat_id: String,
    run: Option<crate::runs::Run>,
    material_contents: Vec<Vec<u8>>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) id: String,
    facts: Facts,
    creation: ChatCreationView,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) run_id: Option<String>,
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
    pub(crate) fn sender(&self, session_id: &str) -> io::Result<Option<Record>> {
        self.access(false, |records| {
            let mut matches = records.iter().filter(|record| record.session_id.as_deref() == Some(session_id));
            let record = matches.next().cloned();
            if matches.next().is_some() {
                return Err(io::Error::other("native Chat has ambiguous delegation provenance"));
            }
            Ok(record)
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
    pub name: String,
}
impl Launch {
    pub fn finish_run(&self, run: &crate::runs::Run, effects: &crate::runs::Effects, failure: Option<&str>) -> io::Result<()> {
        let mut view = ChatCreationView::starting(self.id.clone(), self.name.clone(), run.zone.clone(), run.manager_task_id.clone());
        view.stage = if failure.is_some() { chats::Stage::Failed } else { chats::Stage::Succeeded };
        view.tab_id = effects.tab_id.clone();
        view.pane_id = effects.pane_id.clone();
        view.agent_name = effects.agent_name.clone();
        view.input_submitted = effects.input_submitted;
        view.failure = failure.map(|message| chats::Failure { stage: chats::FailureStage::Submit, code: "run-startup-failed".into(), message: message.to_owned() });
        self.store.finish(view)
    }
}

pub(crate) enum Error {
    Conflict(&'static str, String), Storage(io::Error), Chief(crate::chief::Error), Chat(chats::Error),
    Task(crate::tasks::TaskError), Directory(crate::scheduler::DirectoryError), Definition(crate::scheduler::DelegationDefinitionError),
    Preflight(crate::runs::preflight::PreflightFailure), Stale(Box<Error>), Run(crate::runs::RunError), NotFound,
}
pub(crate) enum Outcome { Created(ChatCreationView), Run(crate::runs::RunOutcome) }
fn rejected(code: &'static str, message: impl Into<String>) -> Error { Error::Conflict(code, message.into()) }
async fn resolve(
    state: &AppState,
    request: DispatchRequest,
    id: &str,
    chat_id: &str,
) -> Result<Prepared, Error> {
    if request.input.trim().is_empty() {
        return Err(rejected(
            "delegation-input-required",
            "必须明确本次业务输入。",
        ));
    }
    let cwd = chats::chat_zone(state, &request.zone_id).await.map_err(Error::Chat)?;
    let (bot, target) = crate::scheduler::bots::delegation_bot(&state.bot_directory, &request.bot_id).map_err(Error::Directory)?;
    let (task_id, map_file) = match target {
        crate::scheduler::bots::DelegationTarget::Task(id) => {
            if request.manager_task_id.is_some() {
                return Err(rejected("delegation-manager-unexpected", "Task 型 Bot 不接受 manager Task。"));
            }
            (id, None)
        }
        crate::scheduler::bots::DelegationTarget::Map(file) => {
            let manager = request.manager_task_id.as_ref().filter(|id| !id.trim().is_empty())
                .ok_or_else(|| rejected("delegation-manager-required", "Map 型 Bot 必须明确指定 manager Task。"))?;
            (manager.clone(), Some(file))
        }
    };
    let task = state
        .tasks
        .get(&task_id)
        .await
        .map_err(Error::Task)?;
    if task.task_type != if map_file.is_some() { TaskType::Manager } else { TaskType::Normal } {
        return Err(rejected(
            "delegation-task-role",
            "Task 型 Bot 必须使用普通 Task；Map 型 Bot 必须明确使用 manager Task。",
        ));
    }
    state
        .tasks
        .validate_suite(&task)
        .map_err(Error::Task)?;
    let preset = state
        .tasks
        .catalog()
        .and_then(|catalog| catalog.resolve_for_role(&task, if map_file.is_some() {
            crate::tasks::catalog::ExecutionRole::Manager
        } else {
            crate::tasks::catalog::ExecutionRole::ChiefDelegated
        }))
        .map_err(Error::Task)?;
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
        let root = chats::chat_zone(state, &material.zone_id).await.map_err(Error::Chat)?;
        let path = crate::zones::browse::checked_file(&root, &material.path)
            .map_err(|error| Error::Chat(chats::Error::Browse(error)))?;
        crate::runs::preflight::check_input_file(&path)
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
    let run = if let Some(file) = &map_file {
        let input = crate::runs::preflight::RunPreflightRequest {
            manager_task_id: task.id.clone(), zone: request.zone_id.clone(), user_input: request.input.clone(),
            file_inputs: materials.iter().map(|m| crate::scheduler::maps::ResolvedFile { id: m.id.clone(), path: m.path.clone() }).collect(),
        };
        let checked = crate::runs::preflight::check_run_preflight(state, file, &input).await.map_err(Error::Preflight)?;
        if let Some(message) = crate::runs::preflight::check_prepared_inputs(&state.map_directory, file, &input.file_inputs, true)
            .map_err(|message| rejected("delegation-material-invalid", message))? {
            return Err(rejected("delegation-material-invalid", message));
        }
        Some(crate::runs::root_run(file.clone(), input, checked, &task, id.to_owned()))
    } else { None };
    let mut material_contents = Vec::new();
    let map = if let Some(file) = &map_file {
        let closure = crate::scheduler::delegation_map_definition(state, file, &request.zone_id).map_err(Error::Definition)?;
        let mut tasks = std::collections::BTreeMap::new();
        let prompts = state.prompts.snapshot().map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
        let mut bot_meaning = std::collections::BTreeMap::new();
        for (key, field) in [("when", crate::prompts::Field::BotWhen), ("plan_guide", crate::prompts::Field::BotPlanGuide), ("node_guide", crate::prompts::Field::BotNodeGuide)] {
            bot_meaning.insert(key, prompts.render(field, bot[key].as_str().expect("validated Bot guidance"))
                .map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?);
        }
        for id in closure.task_ids {
            let task = state.tasks.get(&id).await.map_err(Error::Task)?;
            let preset = state.tasks.catalog().and_then(|catalog| catalog.resolve(&task)).map_err(Error::Task)?;
            let fixed = prompts.render(crate::prompts::Field::TaskPrompt, &task.trigger.prompt)
                .map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
            tasks.insert(id, json!({"task":task,"renderedPrompt":fixed,"promptInstructions":prompts.instructions(&task.trigger.prompt),"launchArgs":chats::agent_launch_args(&preset.env, Some(&preset.trigger))}));
        }
        let mut bindings = std::collections::BTreeMap::new();
        for id in closure.zone_ids {
            bindings.insert(id.clone(), chats::chat_zone(state, &id).await.map_err(Error::Chat)?);
        }
        let mut direct_materials = Vec::new();
        for (zone, relative) in closure.files {
            let path = crate::zones::browse::checked_file(&bindings[&zone], &relative).map_err(|error| Error::Chat(chats::Error::Browse(error)))?;
            direct_materials.push(json!({"zoneId":zone,"path":path}));
            material_contents.push(std::fs::read(&path).map_err(|e| rejected("delegation-material-invalid", e.to_string()))?);
        }
        for material in &materials {
            material_contents.push(std::fs::read(&material.path).map_err(|e| rejected("delegation-material-invalid", e.to_string()))?);
        }
        Some(json!({"file":file,"definitions":closure.definitions,"rootBotMeaning":bot_meaning,"tasks":tasks,"zoneBindings":bindings,"directMaterials":direct_materials}))
    } else { None };
    let prompt = if let Some(run) = &run {
        crate::runs::manager_prompt(state, run, &task, &preset)
            .map_err(|error| rejected("prompt-dictionary-invalid", error))?
    } else {
        let context = json!({"delegationId":id,"botId":request.bot_id,"sourceChiefSessionId":request.session_id,"zoneId":request.zone_id,"cwd":cwd,"materials":materials});
        let business_input = format!("{}\n\n[Chief delegation]\n{}", request.input,
            serde_json::to_string_pretty(&context).expect("context serialization"));
        let snapshot = state.prompts.snapshot().map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
        let fixed = snapshot.render(crate::prompts::Field::TaskPrompt, &task.trigger.prompt)
            .map_err(|error| rejected("prompt-dictionary-invalid", error.to_string()))?;
        chats::compose_prompt(&fixed, preset.trigger.native_skill.as_deref(), &business_input, &snapshot.instructions(&task.trigger.prompt))
    };
    Ok(Prepared {
        facts: Facts {
            request,
            bot,
            task,
            cwd,
            name,
            materials,
            prompt,
            map,
        },
        run,
        material_contents,
        preset,
        chat_id: chat_id.to_owned(),
    })
}

pub(crate) async fn preview(state: &AppState, input: DispatchRequest) -> Result<Value, Error> {
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return Err(Error::Chief(error));
    }
    let id = chats::new_creation_id();
    let prepared = match resolve(&state, input, &id, &chats::new_chat_id()).await {
        Ok(prepared) => prepared,
        Err(error) => return Err(error),
    };
    let response = json!({"previewId":id,"facts":prepared.facts});
    state
        .delegations
        .previews
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, prepared);
    Ok(response)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CommitRequest {
    session_id: String,
    preview_id: String,
}
pub(crate) async fn commit(state: AppState, input: CommitRequest) -> Result<Outcome, Error> {
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return Err(Error::Chief(error));
    }
    let prepared = state
        .delegations
        .previews
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&input.preview_id);
    let Some(mut prepared) = prepared else {
        return Err(rejected(
            "delegation-preview-missing",
            "预览不存在或已使用；请重新核实并确认。",
        ));
    };
    if prepared.facts.request.session_id != input.session_id {
        return Err(rejected("chief-not-current", "预览不属于当前调用会话。"));
    }
    let map_operation = if let Some(run) = &prepared.run {
        Some(state.runs.map_lock(&run.map).await.lock_owned().await)
    } else { None };
    let fresh = match resolve(
        &state,
        prepared.facts.request.clone(),
        &input.preview_id,
        &prepared.chat_id,
    )
    .await
    {
        Ok(fresh) => fresh,
        Err(error) => {
            return Err(Error::Stale(Box::new(error)));
        }
    };
    if fresh.facts != prepared.facts
        || fresh.material_contents != prepared.material_contents
        || chats::agent_launch_args(&fresh.preset.env, Some(&fresh.preset.trigger))
            != chats::agent_launch_args(&prepared.preset.env, Some(&prepared.preset.trigger))
    {
        return Err(rejected(
            "delegation-preview-stale",
            "目标、Task、zone、材料地址或完整输入已变化；本次未发起，请重新预览并确认。",
        ));
    }
    // Approval pins these values. Later edits cannot silently redirect the job.
    if let Err(error) = state.chief.qualify(&input.session_id) {
        return Err(Error::Chief(error));
    }
    prepared.preset.env.suite_args = match state.tasks.prepare_environment(&prepared.facts.task) {
        Ok(args) => args,
        Err(error) => return Err(Error::Task(error)),
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
        run_id: prepared.run.as_ref().map(|run| run.id.clone()),
    }) {
        return Err(Error::Storage(error));
    }
    if let Some(run) = prepared.run {
        crate::runs::register_root(&state, &run).await;
        drop(map_operation);
        return crate::runs::launch_root(state.clone(), run, prepared.facts.task, prepared.preset, Some(Launch {
            store: Arc::clone(&state.delegations), id: input.preview_id,
            prompt: prepared.facts.prompt, name: prepared.facts.name,
        })).await.map(Outcome::Run).map_err(Error::Run);
    }
    let job = state.chat_creations.insert(view.clone()).await;
    let store = Arc::clone(&state.delegations);
    tokio::spawn(async move {
        let launch = Launch {
            store: Arc::clone(&store),
            id: input.preview_id,
            prompt: prepared.facts.prompt,
            name: prepared.facts.name.clone(),
        };
        chats::run_creation(
            Arc::clone(&state.herdr),
            Arc::clone(&state.history),
            state.prompts.clone(),
            Arc::clone(&job),
            prepared.facts.task,
            prepared.preset,
            crate::herdr::placement::Scenario::ChiefDelegatedTask { cwd: &prepared.facts.cwd },
            String::new(),
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
    Ok(Outcome::Created(view))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SenderQuery { session_id: String }
pub(crate) fn list(state: &AppState, query: SenderQuery) -> Result<Value, Error> {
    let records = state.delegations.access(false, |records| {
        Ok(records.iter().filter(|r| r.session_id.as_deref() == Some(&query.session_id)).cloned().collect::<Vec<_>>())
    }).map_err(Error::Storage)?;
    Ok(json!({"delegations":records}))
}
pub(crate) fn get(state: &AppState, id: &str) -> Result<Record, Error> {
    state.delegations.access(false, |records| Ok(records.iter().find(|r| r.id == id).cloned()))
        .map_err(Error::Storage)?.ok_or(Error::NotFound)
}
