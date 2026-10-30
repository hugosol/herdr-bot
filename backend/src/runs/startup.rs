//! Run-only Chat preparation, startup protocol and failure finalization.
use super::*;

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
        protocol(state, &mut run, &task, &preset, Role::Manager, &mut effects, &mut stage).await
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

pub(super) async fn node(state: &AppState, run: &mut Run, id: &str, retry_reason: Option<&str>) -> serde_json::Value {
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
        let node = candidate.nodes.get_mut(id).unwrap();
        node.execution = Some(NodeExecution { id: crate::chats::new_creation_id(), task_id: target.task.id.clone(), agent: target.task.agent, zone: target.zone.clone(), report_policy: target.report_policy, chat: None });
        let preset = state.tasks.catalog().and_then(|catalog| catalog.resolve_for_role(&target.task, crate::tasks::catalog::ExecutionRole::Node)).map_err(|e| e.message())?;
        preset.env.check_entrypoints()?;
        protocol(state, &mut candidate, &target.task, &preset,
            Role::Node { id, target: &target, committed: run }, &mut effects, &mut stage).await
    }.await;
    if let Err(message) = result {
        let failure = StartupFailure { stage: stage.into(), message, effects: Some(effects.clone()) };
        let node = candidate.nodes.get_mut(id).unwrap();
        if retry_reason.is_some() && node.execution.is_none() {
            // Resolution failed before a target was usable. Retain a distinct
            // attempt referencing the failed source, never the old Chat.
            node.execution = node.previous_executions.last().and_then(|fact| fact.execution.as_ref()).map(|previous| NodeExecution {
                id: crate::chats::new_creation_id(), task_id: previous.task_id.clone(), agent: previous.agent, zone: previous.zone.clone(), report_policy: crate::scheduler::node_report_policy(&state.map_directory, &run.map, id).unwrap_or_default(), chat: None,
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
    effects: &mut Effects,
    stage: &mut &'static str,
) -> Result<(), String> {
    *stage = "workspace";
    let workspace = match &role {
        Role::Manager => state.herdr.workspace(&run.workspace_name).await.map_err(|e| e.message())?,
        Role::Node { committed, .. } => {
            let workspace_id = &committed.manager.as_ref().ok_or("管理者没有真实 Chat 关联")?.workspace_id;
            state.herdr.workspace_by_id(workspace_id).await.map_err(|e| e.message())?
                .ok_or("管理者实际 Workspace 不可用；不回退默认 Workspace")?
        }
    };
    effects.workspace_id = Some(workspace.id.clone());
    let zone = match &role {
        Role::Manager => &run.zone,
        Role::Node { target, .. } => &target.zone,
    };
    let name = crate::chats::render_chat_name(&task.chat_name, zone, task, &crate::chats::new_chat_id());
    *stage = "tab";
    let tab = state.herdr.create_tab(&workspace.id, Some(zone), &name).await.map_err(|e| e.message())?;
    effects.tab_id = Some(tab.tab_id.clone());
    effects.pane_id = Some(tab.pane_id.clone());
    let agent = crate::chats::new_agent_name();
    effects.agent_name = Some(agent.clone());
    *stage = "start";
    state.herdr.start_agent(&agent, task.agent.label(), &tab.pane_id,
        &crate::chats::agent_launch_args(&preset.env, Some(&preset.trigger)))
        .await.map_err(|e| e.message())?;
    effects.agent_started = true;
    *stage = "identity";
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    // started_session owns the restricted pending-native-file identity rules.
    let identity = tokio::time::timeout_at(deadline,
        state.herdr.started_session(&agent, task.agent.label(), deadline))
        .await.map_err(|_| "获取原生身份超时".to_owned())?.map_err(|e| e.message())?;
    effects.session_id = Some(identity.session_id.clone());
    if let Role::Node { target, committed, .. } = &role {
        if committed.manager.as_ref().is_some_and(|chat| chat.session_id == identity.session_id)
            || committed.nodes.values().any(|node| node.executions().any(|execution|
                execution.chat.as_ref().is_some_and(|chat| chat.session_id == identity.session_id))) {
            return Err("宿主未返回独立的新原生 Chat 身份".into());
        }
        if !crate::zones::same_path(&identity.cwd, &target.zone) {
            return Err(format!("原生 Chat cwd 与定义 zone 不一致：{}", identity.cwd));
        }
    }
    let chat = ChatReference {
        session_id: identity.session_id.clone(), workspace_id: workspace.id.clone(),
        tab_id: tab.tab_id, pane_id: tab.pane_id, agent_name: agent.clone(),
    };
    match &role {
        Role::Manager => run.manager = Some(chat),
        Role::Node { id, .. } => run.nodes.get_mut(*id).unwrap().execution.as_mut().unwrap().chat = Some(chat),
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
    *stage = "history";
    state.history.record_zone(&identity.session_id, &name, task.agent.label(), &identity.cwd,
        identity.session_file.as_deref(), Some(&workspace.id), &task.id,
        identity.startup_identity_path.as_deref()).await.map_err(|e| e.to_string())?;
    *stage = "submit";
    let paths = Runs::paths(state, &run.id);
    let input = match &role {
        Role::Manager => {
            let mut input = format!(
                "Run ID: {}\nMap: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}",
                run.id, state.map_directory.join(&run.map).display(), state.run_instructions.display(),
                paths[0].display(), paths[1].display(), paths[2].display());
            if !run.user_input.is_empty() {
                input.push_str("\n\n");
                input.push_str(&run.user_input);
            }
            input
        }
        Role::Node { id, target, .. } => format!(
            "Node ID: {id}\nNode: {}\n运行说明: {}\n当前状态: {}\n执行历史: {}\n报告记录: {}\n\n[Report policy control]\n{}\n[/Report policy control]\n\n[Business input]\n{}",
            target.title, state.run_instructions.display(), paths[0].display(), paths[1].display(),
            paths[2].display(), target.report_policy.control_section(), target.input),
    };
    let text = crate::chats::compose_initial_input(task, preset.trigger.native_skill.as_deref(), &input);
    effects.input_attempted = true;
    state.herdr.prompt(&agent, &text).await.map_err(|e| e.message())?;
    effects.input_submitted = true;
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

