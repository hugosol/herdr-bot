//! Run-only Chat preparation, startup protocol and failure finalization.
use super::*;

async fn resolve_inputs(state: &AppState, run: &Run, rules: &[crate::scheduler::maps::InputRule]) -> Result<Vec<crate::scheduler::maps::ResolvedFile>, String> {
    let reports = if rules.iter().any(|rule| matches!(rule, crate::scheduler::maps::InputRule::Node { .. })) { reports::read(state, run).await? } else { Vec::new() };
    let mut inputs = Vec::with_capacity(rules.len());
    for rule in rules {
        let (id, path) = match rule {
            crate::scheduler::maps::InputRule::File { id, path, zone_id } => {
                if !crate::scheduler::maps::valid_relative_file(path) { return Err(format!("输入 {id} 必须是 zone 内相对路径；请迁移 Map 后启动新 Run。")); }
                let zone_id = zone_id.as_deref().or(run.manager_zone_id.as_deref()).ok_or("旧 Run 缺少 zone 绑定快照；请迁移 Map 后启动新 Run")?;
                let root = run.zone_bindings.get(zone_id).ok_or_else(|| format!("zone {zone_id} 不在本次 Run 启动快照中；请启动新 Run"))?;
                let absolute = crate::zones::browse::checked_file(root, path).map_err(|e| e.message())?;
                (id, absolute)
            },
            crate::scheduler::maps::InputRule::Run { id, input_id } => {
                let Some(file) = run.file_inputs.iter().find(|file| file.id == *input_id) else { continue; };
                (id, file.path.clone())
            }
            crate::scheduler::maps::InputRule::Node { id, node_id, output_id } => {
                let execution = run.nodes.get(node_id).and_then(|node| node.execution.as_ref()).ok_or_else(|| format!("输入 {id} 缺少上游执行：{node_id}"))?;
                let path = reports.iter().rev().filter(|report| report.run_id == run.id && report.node_id == *node_id && report.execution_id == execution.id)
                    .find_map(|report| report.outputs.iter().find(|file| file.id == *output_id)).map(|file| file.path.clone())
                    .ok_or_else(|| format!("输入 {id} 缺少上游输出：{node_id}/{output_id}"))?;
                (id, path)
            }
        };
        crate::runs::preflight::check_input_file(&path).map_err(|error| format!("输入 {id}：{error}"))?;
        inputs.push(crate::scheduler::maps::ResolvedFile { id: id.clone(), path });
    }
    Ok(inputs)
}

pub(super) fn material_directory(zone: &str, id: &str) -> PathBuf {
    std::path::Path::new(zone).join(".manager-zone").join(".workflow").join(id)
}

pub(super) async fn write_github_materials(
    state: &AppState,
    zone: &str,
    id: &str,
    github: preflight::GitHubExploration,
) -> Result<Vec<crate::scheduler::maps::ResolvedFile>, String> {
    use crate::scheduler::maps::ResolvedFile;
    use preflight::GitHubExploration as GitHub;
    use std::io::{Error, ErrorKind};

    #[derive(Serialize)]
    struct CloneRequest<'a> {
        github_url: &'a str,
        parent_directory: &'a str,
    }

    let clone = github.has_clone().then(|| serde_json::to_vec(&CloneRequest {
        github_url: &github.github_url,
        parent_directory: &github.parent_directory,
    })).transpose().map_err(|e| e.to_string())?;
    let brief = github.has_brief().then_some(github.exploration_direction);
    if clone.is_none() && brief.is_none() { return Ok(Vec::new()); }

    let directory = material_directory(zone, id);
    let scope = directory.parent().expect("material parent").to_path_buf();
    let zone = PathBuf::from(zone);
    let state = state.clone();
    let failed = |e| format!("无法写入 Run 启动材料，未发起管理者 Chat：{e}");
    tokio::task::spawn_blocking(move || state.runs.files.with(&[scope.clone()], |access| {
        // Do not follow a link or recreate a removed manager zone.
        for path in scope.ancestors() {
            match access.symlink_metadata(path) {
                Ok(metadata) if metadata.is_dir() && !crate::zones::browse::is_link_like(&metadata) => {},
                Ok(_) => return Err(Error::new(ErrorKind::PermissionDenied, format!("材料目录不能经过链接或非目录：{}", path.display()))),
                Err(e) if e.kind() == ErrorKind::NotFound && path != zone && path.starts_with(&zone) => {},
                Err(e) => return Err(e),
            }
        }
        access.create_dir_all(&scope)?;
        access.create_dir(&directory)?;
        let mut bindings = Vec::with_capacity(2);
        for (id, filename, bytes) in [
            (GitHub::CLONE, "clone_request.json", clone.as_deref()),
            (GitHub::BRIEF, "exploration_brief.txt", brief.as_deref().map(str::as_bytes)),
        ] {
            if let Some(bytes) = bytes {
                let path = directory.join(filename);
                access.create(&path, bytes)?;
                let path = path.into_os_string().into_string().map_err(|_| Error::new(ErrorKind::InvalidData, "材料路径不是有效的 UTF-8"))?;
                bindings.push(ResolvedFile { id: id.into(), path });
            }
        }
        Ok(bindings)
    })).await.map_err(|e| format!("无法写入 Run 启动材料，未发起管理者 Chat：{e}"))?.and_then(|result| result).map_err(failed)
}

fn manager_input(state: &AppState, run: &Run) -> String {
    let paths = Runs::paths(state, &run.id);
    let materials = material_directory(&run.zone, &run.id);
    let mut input = format!(
        "Run ID: {}\nMap: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n中间资料位置: {}",
        run.id, state.map_directory.join(&run.map).display(), state.run_instructions.display(),
        paths[0].display(), paths[1].display(), paths[2].display(), materials.display());
    input.push_str("\n\n[End node contract] Map may explicitly mark one end_node. It must wait for every other node to be completed. After successful dispatch, that Chat is handed to the user and remains running/open: do not request reports or complete it. Explicitly finish the Run when canFinish is true; this records handed-to-user, not completion of that Chat. Never finish after a failed dispatch. [/End node contract]");
    if !run.user_input.is_empty() {
        input.push_str("\n\n");
        input.push_str(&run.user_input);
    }
    input
}

pub(crate) fn manager_prompt(
    state: &AppState,
    run: &Run,
    task: &crate::tasks::Task,
    preset: &crate::tasks::catalog::ResolvedPreset,
) -> Result<String, String> {
    let prompts = state.prompts.snapshot().map_err(|e| e.to_string())?;
    let fixed = prompts.render(crate::prompts::Field::TaskPrompt, &task.trigger.prompt).map_err(|e| e.to_string())?;
    Ok(crate::chats::compose_prompt(&fixed, preset.trigger.native_skill.as_deref(), &manager_input(state, run), &prompts.instructions(&task.trigger.prompt)))
}

pub(super) async fn manager_interrupted(
    state: &AppState,
    id: &str,
    join: tokio::task::JoinError,
) -> Result<(), String> {
    let paths = Runs::paths(state, id);
    let owned = state.clone();
    let retained_id = id.to_owned();
    state.runs.files.with_async(&paths, move |_access| Box::pin(async move {
        if owned.runs.ensure_retained(&retained_id).await.is_ok() {
            owned.runs.active.lock().await.insert(
                retained_id,
                Some(StartupFailure {
                    stage: "internal".into(),
                    message: join.to_string(),
                    effects: Some(Effects::default()),
                }),
            );
        }
    })).await.map_err(|e| e.to_string())
}

pub(super) async fn manager(
    state: &AppState,
    mut run: Run,
    task: crate::tasks::Task,
    preset: crate::tasks::catalog::ResolvedPreset,
    delegation: Option<crate::chief_delegation::Launch>,
) -> Result<(), ()> {
    let mut effects = Effects::default();
    let mut stage = "initialize";
    let result: Result<(), String> = async {
        let paths = Runs::paths(state, &run.id);
        {
            let owned = state.clone();
            let initial = run.clone();
            let initial_paths = paths.clone();
            state.runs.files.with_async(&[paths[0].parent().unwrap().to_path_buf()], move |access| Box::pin(async move {
                owned.runs.ensure_retained(&initial.id).await?;
                access.create_dir_all(initial_paths[0].parent().unwrap()).map_err(|e| e.to_string())?;
                access.create(&initial_paths[0], &serde_json::to_vec_pretty(&initial).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                access.create(&initial_paths[1], b"").map_err(|e| e.to_string())?;
                access.create(&initial_paths[2], b"").map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })).await.map_err(|e| e.to_string())??;
        }
        effects.records_initialized = true;
        protocol(state, &mut run, &task, &preset, Role::Manager, None, delegation.as_ref(), &mut effects, &mut stage).await
    }
    .await;
    let result = if let Some(launch) = &delegation {
        match launch.finish_run(&run, &effects, result.as_ref().err().map(String::as_str)) {
            Ok(()) => result,
            Err(error) => Err(format!("最终委派状态无法保存：{error}；先前失败：{result:?}。现场保留，不自动重试。")),
        }
    } else { result };
    if let Err(message) = result {
        eprintln!(
            "[workflow] run={} manager-start stage={} effects={} error={}；已发生的外部效果未回滚，不自动重试。",
            run.id,
            stage,
            serde_json::to_string(&effects).unwrap_or_default(),
            message
        );
        let paths = Runs::paths(state, &run.id);
        let owned = state.clone();
        let stage = stage.to_owned();
        state.runs.files.with_async(&paths, move |_access| Box::pin(async move {
            if owned.runs.ensure_retained(&run.id).await.is_ok() {
                owned.runs.active.lock().await.insert(
                    run.id,
                    Some(StartupFailure { stage, message, effects: Some(effects) }),
                );
            }
        })).await.map_err(|_| ())?;
        return Err(());
    }
    Ok(())
}

pub(super) async fn node(state: &AppState, run: &mut Run, id: &str, retry_reason: Option<&str>, target: Result<crate::scheduler::DispatchTarget, String>) -> serde_json::Value {
    let mut effects = Effects { records_initialized: true, ..Effects::default() };
    let mut stage = "resolve";
    let mut candidate = run.clone();
    if retry_reason.is_some() {
        let node = candidate.nodes.get_mut(id).unwrap();
        node.previous_executions.push(ExecutionFact { execution: node.execution.take(), failure: node.failure.take() });
        node.completion = None;
    }
    let result: Result<(), String> = async {
        let target = target.as_ref().map_err(Clone::clone)?;
        crate::zones::browse::checked_root(&target.zone).map_err(|error| error.message())?;
        let prepared = match target.script.clone() {
            Some(script) => {
                let owned = state.clone();
                Some(tokio::task::spawn_blocking(move || scripts::prepare_script(&owned, &script)).await.map_err(|error| error.to_string())??)
            }
            None => None,
        };
        let identity = match &target.script {
            Some(script) => ExecutionTarget::Script(ScriptExecution { kind: ScriptKind::Typescript, script: script.clone(), lifecycle: Lifecycle::Running { started_at: scripts::now() } }),
            None => {
                let task = target.task.as_ref().ok_or("缺少 Chat 执行 Task")?;
                ExecutionTarget::Chat(ChatExecution { task_id: task.id.clone(), agent: task.agent, chat: None, child_run_id: None })
            }
        };
        let node = candidate.nodes.get_mut(id).unwrap();
        node.execution = Some(NodeExecution { id: crate::chats::new_creation_id(), target: identity, zone_id: Some(target.zone_id.clone()), zone: target.zone.clone(), input_rule: Some(target.input_rule.clone()), output_rule: Some(target.output_rule.clone()), inputs: Some(Vec::new()), auto_approved: target.auto_approved, approved: false, approval_source: None, auto_close: target.auto_close, end_node: target.end_node });
        let inputs = resolve_inputs(state, &candidate, &target.input_rule).await?;
        candidate.nodes.get_mut(id).unwrap().execution.as_mut().unwrap().inputs = Some(inputs);
        if let Some(prepared) = prepared {
            candidate.nodes.get_mut(id).unwrap().status = "running".into();
            save_run(state, &candidate).await?;
            *run = candidate.clone();
            scripts::launch(state.clone(), candidate.clone(), id.to_owned(), prepared, target.input.clone()).await;
            return Ok(());
        }
        let task = target.task.as_ref().ok_or("缺少 Chat 执行 Task")?;
        let suite_args = state.tasks.prepare_environment(task).map_err(|error| error.message())?;
        let role = if target.map.is_some() { crate::tasks::catalog::ExecutionRole::NestedManager } else if target.end_node { crate::tasks::catalog::ExecutionRole::Standalone } else { crate::tasks::catalog::ExecutionRole::Node };
        let mut preset = state.tasks.catalog().and_then(|catalog| catalog.resolve_for_role(task, role)).map_err(|e| e.message())?;
        preset.env.suite_args = suite_args;
        preset.env.select_host(&state.herdr);
        preset.env.check_entrypoints()?;
        let mut child = match &target.map {
            Some(map) => Some(prepare_child(state, &mut candidate, id, map, &target, &mut effects, &mut stage).await?),
            None => None,
        };
        let result = protocol(state, &mut candidate, task, &preset,
            Role::Node { id, target: &target, committed: run }, child.as_mut(), None, &mut effects, &mut stage).await;
        if let (Some(child), Err(message)) = (child, &result) {
            state.runs.active.lock().await.insert(child.id, Some(StartupFailure { stage: stage.into(), message: message.clone(), effects: Some(effects.clone()) }));
        }
        result
    }.await;
    if let Err(message) = result {
        let failure = StartupFailure { stage: stage.into(), message, effects: Some(effects.clone()) };
        let node = candidate.nodes.get_mut(id).unwrap();
        if retry_reason.is_some() && node.execution.is_none() {
            // Resolution failed before a target was usable. Retain a distinct
            // attempt referencing the failed source, never the old Chat.
            node.execution = node.previous_executions.last().and_then(|fact| fact.execution.as_ref()).and_then(|previous| {
                let identity = match &previous.target {
                    ExecutionTarget::Chat(chat) => ExecutionTarget::Chat(ChatExecution { task_id: chat.task_id.clone(), agent: chat.agent, chat: None, child_run_id: None }),
                    ExecutionTarget::Script(script) => ExecutionTarget::Script(ScriptExecution { kind: ScriptKind::Typescript, script: script.script.clone(), lifecycle: Lifecycle::Running { started_at: scripts::now() } }),
                    ExecutionTarget::LegacyScript(_) => return None,
                };
                Some(NodeExecution { id: crate::chats::new_creation_id(), target: identity, zone_id: target.as_ref().ok().map(|target| target.zone_id.clone()).or_else(|| previous.zone_id.clone()), zone: target.as_ref().map(|target| target.zone.clone()).unwrap_or_else(|_| previous.zone.clone()), input_rule: None, output_rule: None, inputs: None, auto_approved: target.as_ref().map_or(previous.auto_approved, |target| target.auto_approved), approved: false, approval_source: None, auto_close: false, end_node: target.as_ref().map_or(previous.end_node, |target| target.end_node) })
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
        operation::history_feedback(&mut value, history_error);
        return value;
    }
    *run = candidate;
    let history_error = record_dispatch(state, run, id, retry_reason).await.err();
    if let Some(message) = &history_error {
        eprintln!("[workflow] run={} node={} dispatch-history error={}；状态已提交，未回滚。", run.id, id, message);
    }
    let mut value = serde_json::json!({"nodeId":id,"status":"running","effects":effects});
    operation::history_feedback(&mut value, history_error);
    value
}

async fn prepare_child(state: &AppState, parent: &mut Run, node_id: &str, map: &str, target: &crate::scheduler::DispatchTarget, effects: &mut Effects, stage: &mut &'static str) -> Result<Run, String> {
    *stage = "child-preflight";
    let zone_id = target.zone_id.clone();
    let execution = parent.nodes[node_id].execution.as_ref().ok_or("缺少父 execution")?;
    let input = crate::runs::preflight::RunPreflightRequest {
        manager_task_id: target.task.as_ref().ok_or("子 Run 缺少 manager Task")?.id.clone(), zone: zone_id.clone(), user_input: target.input.clone(),
        file_inputs: execution.inputs.clone().ok_or("缺少父执行输入")?,
        github_exploration: None,
    };
    let gate = state.runs.map_lock(map).await;
    let _registration = gate.lock().await;
    // Keep parent reference + child initialization indivisible to deletion,
    // but release both gates before any Herdr call or association wait.
    let _references = state.runs.history_references.lock().await;
    let checked = crate::runs::preflight::check_run_preflight(state, map, &input).await.map_err(|failure| failure.message())?;
    // Legacy trees retain their original naming behavior; do not invent ancestry.
    let naming = parent.naming.as_ref().map(|parent_naming| {
        let mut naming = checked.naming;
        let mut path = parent_naming.map_path.clone();
        path.append(&mut naming.map_path);
        naming.map_path = path;
        naming
    });
    let child = Run {
        id: crate::chats::new_creation_id(), name: naming.as_ref().map(RunNaming::manager_title), naming, map: map.to_owned(),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        manager_task_id: target.task.as_ref().ok_or("子 Run 缺少 manager Task")?.id.clone(),
        parent: Some(ParentExecution { run_id: parent.id.clone(), node_id: node_id.to_owned(), execution_id: execution.id.clone() }),
        manager_zone_id: Some(zone_id.clone()), zone: target.zone.clone(),
        zone_bindings: { let mut bindings = checked.zone_bindings; bindings.insert(zone_id, target.zone.clone()); bindings },
        workspace_name: parent.workspace_name.clone(), user_input: target.input.clone(), file_inputs: input.file_inputs,
        inputs_ready: false, manager: None, status: "running".into(), finished: None,
        nodes: checked.node_ids.into_iter().map(|id| (id, NodeState { status: "pending".into(), execution: None, failure: None, completion: None, auto_close_failure: None, previous_executions: Vec::new() })).collect(),
    };
    *stage = "child-register";
    *parent.nodes.get_mut(node_id).unwrap().execution.as_mut().unwrap().child_run_id_mut() = Some(child.id.clone());
    save_run(state, parent).await?;
    effects.child_run_id = Some(child.id.clone());
    state.runs.registered_maps.lock().await.insert(child.id.clone(), child.map.clone());
    state.runs.active.lock().await.insert(child.id.clone(), None);
    *stage = "child-initialize";
    let paths = Runs::paths(state, &child.id);
    let initial = child.clone();
    let initialized = state.runs.files.with_async(&[paths[0].parent().unwrap().to_path_buf()], move |access| Box::pin(async move {
        access.create_dir_all(paths[0].parent().unwrap()).map_err(|e| e.to_string())?;
        access.create(&paths[0], &serde_json::to_vec_pretty(&initial).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        access.create(&paths[1], b"").map_err(|e| e.to_string())?;
        access.create(&paths[2], b"").map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    })).await.map_err(|e| e.to_string()).and_then(|result| result);
    if let Err(message) = initialized {
        state.runs.active.lock().await.insert(child.id, Some(StartupFailure { stage: (*stage).into(), message: message.clone(), effects: Some(effects.clone()) }));
        return Err(message);
    }
    effects.child_records_initialized = true;
    Ok(child)
}

// These are the two Run startup roles, not extension points. Node callers hold
// the Run operation lock; only manager association acquires it here.
enum Role<'a> {
    Manager,
    Node {
        id: &'a str,
        target: &'a crate::scheduler::DispatchTarget,
        committed: &'a mut Run,
    },
}

async fn protocol(
    state: &AppState,
    run: &mut Run,
    task: &crate::tasks::Task,
    preset: &crate::tasks::catalog::ResolvedPreset,
    mut role: Role<'_>,
    mut child: Option<&mut Run>,
    delegation: Option<&crate::chief_delegation::Launch>,
    effects: &mut Effects,
    stage: &mut &'static str,
) -> Result<(), String> {
    *stage = "workspace";
    let (workspace, directory) = {
        use crate::herdr::placement::Scenario;
        let scenario = match &role {
            Role::Manager => match delegation {
                None => Scenario::RootMap { cwd: &run.zone, workspace_name: &run.workspace_name },
                Some(_) => Scenario::ChiefDelegatedMap { cwd: &run.zone, workspace_name: &run.workspace_name },
            },
            Role::Node { target, committed, .. } => {
                let manager_workspace_id = committed.manager.as_ref().map(|chat| chat.workspace_id.as_str());
                if child.is_some() {
                    Scenario::NestedMap { cwd: &target.zone, manager_workspace_id }
                } else {
                    Scenario::Node { cwd: &target.zone, manager_workspace_id }
                }
            }
        };
        let mapping = scenario.directory(&state.herdr).await.map_err(|e| e.message())?;
        let workspace = mapping.workspace(&state.herdr).await.map_err(|e| e.message())?.workspace;
        (workspace, mapping.directory)
    };
    effects.workspace_id = Some(workspace.id.clone());
    let zone = directory.as_deref().expect("Run launch always specifies an execution directory");
    crate::zones::browse::checked_root(zone).map_err(|e| e.message())?;
    let naming = child.as_deref().unwrap_or(run).naming.as_ref();
    let name = match (naming, &role) {
        (Some(naming), Role::Node { id, target, .. }) if child.is_none() => naming.node_title(id, &target.title),
        (Some(naming), _) => naming.manager_title(),
        (None, _) => delegation.map(|launch| launch.name.clone()).unwrap_or_else(|| crate::chats::render_chat_name(&task.chat_name, zone, task, &crate::chats::new_chat_id())),
    };
    *stage = "tab";
    let kind = task.agent.label();
    let alias = crate::chats::new_agent_name();
    let args = crate::chats::agent_launch_args(&preset.env, Some(&preset.trigger));
    let launch = state.launches.begin(kind, Some(zone));
    let hosted = state.herdr.launch_tab(&workspace.id, Some(zone), &name, &alias, kind, &args, &launch)
        .await.map_err(|e| e.message())?;
    effects.tab_id = Some(hosted.location.tab_id.clone());
    effects.pane_id = Some(hosted.location.pane_id.clone());
    if !state.herdr.is_app() { effects.agent_name = Some(alias.clone()); }
    *stage = "start";
    hosted.start_agent(&state.herdr, kind, &args).await.map_err(|e| e.message())?;
    effects.agent_started = true;
    *stage = "identity";
    let identity = launch.ready(&state.herdr).await.map_err(|e| e.message())?;
    effects.session_id = Some(identity.session_id.clone());
    if let Role::Node { id, .. } = &role {
        let execution = run.nodes[*id].execution.as_ref().ok_or("缺少冻结执行。")?;
        if run.session_claimed_by_other_execution(&identity.session_id, &execution.id) {
            return Err("原生 Chat 已属于本 Run 的管理者或其他执行，不能重复关联。".into());
        }
    }
    let chat = ChatReference {
        session_id: identity.session_id.clone(), workspace_id: workspace.id.clone(),
        tab_id: hosted.location.tab_id.clone(), pane_id: hosted.location.pane_id.clone(),
        // App launches the native agent directly; navigation never uses a TUI alias there.
        agent_name: effects.agent_name.clone().unwrap_or_else(|| kind.to_owned()),
    };
    if let Some(child) = child.as_deref_mut() {
        child.manager = Some(chat.clone());
        *stage = "child-association";
        save_run(state, child).await?;
    }
    match &role {
        Role::Manager => run.manager = Some(chat),
        Role::Node { id, .. } => *run.nodes.get_mut(*id).unwrap().execution.as_mut().unwrap().chat_mut() = Some(chat),
    }
    *stage = "association";
    match &mut role {
        Role::Manager => {
            // Merge names saved while host startup was pending, under the same
            // operation lock as rename. No file coordination spans host calls.
            let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
            let _operation = lock.lock().await;
            let paths = Runs::paths(state, &run.id);
            let owned = state.clone();
            let mut associated = run.clone();
            let association_paths = paths.clone();
            *run = state.runs.files.with_async(&paths, move |access| Box::pin(async move {
                owned.runs.ensure_retained(&associated.id).await?;
                associated.name = access.read_json::<Run>(&association_paths[0])?.name;
                access.replace(&association_paths[0], &serde_json::to_vec_pretty(&associated).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                Ok::<_, String>(associated)
            })).await.map_err(|e| e.to_string())??;
        }
        Role::Node { committed, .. } => {
            save_run(state, run).await?;
            **committed = run.clone();
        }
    }
    effects.association_saved = true;
    if let Some(launch) = delegation {
        *stage = "delegation-association";
        launch.store.bind(&launch.id, &identity.session_id).map_err(|error| error.to_string())?;
    }
    *stage = "history";
    state.history.record_zone(&identity.session_id, &name, task.agent.label(), &identity.cwd,
        identity.session_file.as_deref(), Some(&workspace.id), &task.id,
        identity.startup_identity_path.as_deref()).await.map_err(|e| e.to_string())?;
    *stage = "submit";
    let text = if let Some(launch) = delegation {
        std::borrow::Cow::Borrowed(launch.prompt.as_str())
    } else {
    let paths = Runs::paths(state, &run.id);
    let materials = material_directory(&run.zone, &run.id);
    let prompts = state.prompts.snapshot().map_err(|e| e.to_string())?;
    let fixed_prompt = prompts.render(crate::prompts::Field::TaskPrompt, &task.trigger.prompt).map_err(|e| e.to_string())?;
    let mut input = match &role {
        Role::Manager => manager_input(state, run),
        Role::Node { id, target, .. } => {
            use std::fmt::Write;
            let execution = run.nodes[*id].execution.as_ref().ok_or("缺少冻结执行。")?;
            if execution.end_node {
                let mut input = format!("[Run background / userInput]\n{}\n[/Run background / userInput]\n\n[Named file inputs]\n", run.user_input);
                for file in execution.inputs.as_ref().ok_or("缺少冻结输入地址。")? { writeln!(input, "{}: {}", file.id, file.path).unwrap(); }
                write!(input, "[/Named file inputs]\n\n[Business input]\n{}", prompts.render(crate::prompts::Field::NodeInput, &target.input).map_err(|e| e.to_string())?).unwrap();
                input
            } else {
            let mut input = format!(
                "Node ID: {id}\nNode: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\n中间资料位置: {}\n\n[Run background / userInput]\n{}\n[/Run background / userInput]\n\n[Named file inputs]\n",
                target.title, state.run_instructions.display(), paths[0].display(), paths[1].display(),
                paths[2].display(), materials.display(), run.user_input);
            for file in execution.inputs.as_ref().ok_or("缺少冻结输入地址。")? { writeln!(input, "{}: {}", file.id, file.path).unwrap(); }
            writeln!(input, "[/Named file inputs]\n\n[Output rules]\n{}\n[/Output rules]", serde_json::to_string(&execution.output_rule).map_err(|e| e.to_string())?).unwrap();
            write!(input, "\n[Result approval control]\nauto_approved: {}\nApprove the current session result before the final handoff report. Approval and reporting are independent; progress, problems and failures may be reported at any time without sending confirmation. Approval never completes work or notifies the manager; the manager must explicitly complete after a latest ready report and declared deliveries. Use the native approval tool; manual mode requires explicit native Yes.\n[/Result approval control]\n\n[Business input]\n{}",
                target.auto_approved, prompts.render(crate::prompts::Field::NodeInput, &target.input).map_err(|e| e.to_string())?).unwrap();
            input
            }
        }
    };
    if let Some(child) = child {
        use std::fmt::Write;
        let paths = Runs::paths(state, &child.id);
        write!(input, "\n\n[Child Run manager context]\nRun ID: {}\nMap: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n中间资料位置: {}\n[/Child Run manager context]\n\n[End node contract] An explicit end_node waits for all other nodes completed. Successful dispatch hands its open Chat to the user without report or complete; explicitly finish this child Run when canFinish=true, then report normally to the parent manager. Failed dispatch cannot finish. [/End node contract]", child.id, state.map_directory.join(&child.map).display(), paths[0].display(), paths[1].display(), paths[2].display(), std::path::Path::new(&child.zone).join(".manager-zone").join(".workflow").join(&child.id).display()).unwrap();
    }
    let text = crate::chats::compose_prompt(&fixed_prompt, preset.trigger.native_skill.as_deref(), &input, &prompts.instructions(&task.trigger.prompt));
    std::borrow::Cow::Owned(text)
    };
    launch.current(&state.herdr, &hosted).await.map_err(|e| e.message())?;
    // A saved association stays locatable when the submission outcome is uncertain.
    if state.herdr.is_app() { launch.retain_app_target(); }
    effects.input_attempted = true;
    effects.input_outcome = Some(InputOutcome::Unknown);
    if let Err(error) = hosted.submit(&state.herdr, &text).await {
        if matches!(error, crate::herdr::HerdrError::NotStarted(_) | crate::herdr::HerdrError::CommandFailed { .. }) {
            effects.input_outcome = Some(InputOutcome::Rejected);
        }
        return Err(error.message());
    }
    effects.input_submitted = true;
    effects.input_outcome = Some(InputOutcome::Submitted);
    if let Role::Node { id, .. } = role {
        run.nodes.get_mut(id).unwrap().status = "running".into();
        *stage = "state";
        save_run(state, run).await?;
    }
    Ok(())
}

async fn record_dispatch(state: &AppState, run: &Run, id: &str, retry_reason: Option<&str>) -> Result<(), String> {
    let paths = Runs::paths(state, &run.id);
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
    let owned = state.clone();
    let run_id = run.id.clone();
    state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
        owned.runs.ensure_retained(&run_id).await?;
        access.append_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1], &entry)
    })).await.map_err(|e| e.to_string())?
}

