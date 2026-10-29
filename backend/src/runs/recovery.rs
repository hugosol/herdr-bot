//! Human-confirmed failure facts; never launches, prompts or stops a Chat.
use super::*;
use super::completion::{outcome, rejection, record_transition};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FailRequest {
    session_id: String,
    node_id: String,
    reason: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum DispatchFinding {
    Uncertain,
    NotCreated,
    Created,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReconcileRequest {
    session_id: String,
    node_id: String,
    finding: DispatchFinding,
    reason: String,
}

pub async fn manager_reconcile_dispatch(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: ReconcileRequest = match crate::chat_command_body(request, "核实派发事实").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        if !matches!(run.status.as_str(), "running" | "paused") {
            return rejection(&state, &run, "run-terminal", "Run 已结束，不接受状态变更。").await;
        }
        let Some(node) = run.nodes.get(&input.node_id) else {
            return rejection(&state, &run, "node-unknown", "node 不在本次 Run 中。").await;
        };
        if node.status != "failed" || !node.failure.as_ref().is_some_and(|failure| failure.stage != "execution") {
            return rejection(&state, &run, "node-not-dispatch-failed", "仅派发阶段 failed 可核实；执行失败请 recover 或确认交接后 complete。").await;
        }
        if input.reason.trim().is_empty() {
            return rejection(&state, &run, "reconciliation-reason-required", "请说明人工核实的事实及依据；不确定时保持 failed。").await;
        }
        let failure = node.failure.clone();
        let execution = node.execution.clone();
        let changed = match input.finding {
            DispatchFinding::Uncertain => false,
            DispatchFinding::Created => {
                let Some(effects) = failure.as_ref().and_then(|failure| failure.effects.as_ref()) else {
                    return rejection(&state, &run, "dispatch-chat-unverified", "缺少已记录派发效果；保持 failed，不猜测身份。").await;
                };
                let (Some(execution), Some(agent), Some(workspace), Some(tab), Some(pane)) = (execution.as_ref(), effects.agent_name.as_ref(), effects.workspace_id.as_ref(), effects.tab_id.as_ref(), effects.pane_id.as_ref()) else {
                    return rejection(&state, &run, "dispatch-chat-unverified", "缺少原派发现实目标，无法关联真实 Chat；保持 failed，不猜测身份。").await;
                };
                let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
                let identity = match tokio::time::timeout_at(deadline, state.herdr.native_session(agent, execution.agent.label(), deadline)).await {
                    Ok(Ok(identity)) => identity,
                    Ok(Err(error)) => return rejection(&state, &run, "dispatch-chat-unverified", &format!("未确认原 Chat：{}；保持 failed。", error.message())).await,
                    Err(_) => return rejection(&state, &run, "dispatch-chat-unverified", "读取原 Chat 身份超时；保持 failed。").await,
                };
                if !crate::zones::same_path(&identity.cwd, &execution.zone)
                    || effects.session_id.as_ref().is_some_and(|session| session != &identity.session_id)
                    || execution.chat.as_ref().is_some_and(|chat| chat.session_id != identity.session_id)
                    || run.manager.as_ref().is_some_and(|chat| chat.session_id == identity.session_id)
                    || run.nodes.values().any(|node| node.executions().any(|other| other.id != execution.id && other.chat.as_ref().is_some_and(|chat| chat.session_id == identity.session_id))) {
                    return rejection(&state, &run, "dispatch-chat-mismatch", "原 Chat 的身份或 zone 不一致，或已属于其他执行；保持 failed，不替换执行。").await;
                }
                let chat = ChatReference { session_id: identity.session_id, workspace_id: workspace.clone(), tab_id: tab.clone(), pane_id: pane.clone(), agent_name: agent.clone() };
                let node = run.nodes.get_mut(&input.node_id).unwrap();
                node.execution.as_mut().unwrap().chat = Some(chat);
                node.status = "running".into();
                node.failure = None;
                true
            }
            DispatchFinding::NotCreated => {
                if execution.as_ref().and_then(|execution| execution.chat.as_ref()).is_some() || failure.as_ref().and_then(|failure| failure.effects.as_ref()).is_some_and(|effects| effects.session_id.is_some()) {
                    return rejection(&state, &run, "dispatch-chat-already-known", "已取得真实 Chat 身份，不能核实为未创建；不要抹除已发生事实。").await;
                }
                let node = run.nodes.get_mut(&input.node_id).unwrap();
                node.status = "pending".into();
                node.execution = None;
                node.failure = None;
                true
            }
        };
        if changed { save_run(&state, &run).await?; }
        let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"reconcile_dispatch","nodeId":input.node_id,"before":"failed","after":run.nodes[&input.node_id].status,"finding":input.finding,"reason":input.reason,"failure":failure,"execution":execution,"currentExecution":run.nodes[&input.node_id].execution});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, changed, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "reconcile-dispatch-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "reconcile-dispatch-internal", &join.to_string()),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoverRequest {
    session_id: String,
    node_id: String,
    confirmed: bool,
    reason: String,
}

pub async fn manager_recover(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: RecoverRequest = match crate::chat_command_body(request, "恢复原执行跟踪").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        if !matches!(run.status.as_str(), "running" | "paused") {
            return rejection(&state, &run, "run-terminal", "Run 已结束，不接受状态变更。").await;
        }
        let Some(node) = run.nodes.get(&input.node_id) else {
            return rejection(&state, &run, "node-unknown", "node 不在本次 Run 中。").await;
        };
        if node.status != "failed" || !node.failure.as_ref().is_some_and(|failure| failure.stage == "execution") || node.execution.as_ref().and_then(|execution| execution.chat.as_ref()).is_none() {
            return rejection(&state, &run, "node-not-recoverable", "仅有关联原 Chat 的执行阶段 failed 可 recover；派发失败请先核实。").await;
        }
        if !input.confirmed || input.reason.trim().is_empty() {
            return rejection(&state, &run, "recovery-confirmation-required", "请先由人工确认现场已修复、原 Chat 可继续，并说明依据。").await;
        }
        let node = run.nodes.get_mut(&input.node_id).unwrap();
        let failure = node.failure.take();
        node.status = "running".into();
        save_run(&state, &run).await?;
        let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"recover","nodeId":input.node_id,"before":"failed","after":"running","reason":input.reason,"failure":failure,"execution":run.nodes[&input.node_id].execution});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, true, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "recover-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "recover-internal", &join.to_string()),
    }
}

pub async fn manager_fail(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: FailRequest = match crate::chat_command_body(request, "记录执行失败").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        if !matches!(run.status.as_str(), "running" | "paused") {
            return rejection(&state, &run, "run-terminal", "Run 已结束，不接受状态变更。").await;
        }
        let Some(node) = run.nodes.get(&input.node_id) else {
            return rejection(&state, &run, "node-unknown", "node 不在本次 Run 中。").await;
        };
        if node.status == "failed" {
            return outcome(&state, &run, false, None).await;
        }
        if node.status != "running" {
            return rejection(&state, &run, "node-not-failable", "仅 running 执行可标记失败；重复 failed 保留首次事实。").await;
        }
        if input.reason.trim().is_empty() {
            return rejection(&state, &run, "failure-reason-required", "请记录已核实的执行失败原因。").await;
        }
        node.execution.as_ref().and_then(|execution| execution.chat.as_ref()).ok_or("执行缺少原 Chat 关联。")?;
        let failure = StartupFailure { stage: "execution".into(), message: input.reason.clone(), effects: None };
        let node = run.nodes.get_mut(&input.node_id).unwrap();
        node.status = "failed".into();
        node.failure = Some(failure);
        save_run(&state, &run).await?;
        let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"fail","nodeId":input.node_id,"before":"running","after":"failed","reason":input.reason,"failure":run.nodes[&input.node_id].failure,"execution":run.nodes[&input.node_id].execution});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, true, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "fail-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "fail-internal", &join.to_string()),
    }
}
