//! Human-confirmed failure facts; never launches, prompts or stops a Chat.
use super::*;
use super::operation::{Action, Decision};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FailRequest {
    session_id: String,
    node_id: String,
    reason: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DispatchFinding {
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
    operation::execute(state, &input.session_id, Action::Reconcile { node_id: input.node_id, finding: input.finding, reason: input.reason }).await
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
    operation::execute(state, &input.session_id, Action::Recover { node_id: input.node_id, confirmed: input.confirmed, reason: input.reason }).await
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
    operation::execute(state, &input.session_id, Action::Fail { node_id: input.node_id, reason: input.reason }).await
}

pub(super) async fn reconcile(state: &AppState, run: &mut Run, node_id: String, finding: DispatchFinding, reason: String) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    let Some(node) = run.nodes.get(&node_id) else {
        return Decision::reject("node-unknown", "node 不在本次 Run 中。");
    };
    if node.status != "failed" || !node.failure.as_ref().is_some_and(|failure| failure.stage != "execution") {
        return Decision::reject("node-not-dispatch-failed", "仅派发阶段 failed 可核实；执行失败请 recover 或确认交接后 complete。");
    }
    if reason.trim().is_empty() {
        return Decision::reject("reconciliation-reason-required", "请说明人工核实的事实及依据；不确定时保持 failed。");
    }
    let failure = node.failure.clone();
    let execution = node.execution.clone();
    let changed = match finding {
        DispatchFinding::Uncertain => false,
        DispatchFinding::Created => {
            let Some(effects) = failure.as_ref().and_then(|failure| failure.effects.as_ref()) else {
                return Decision::reject("dispatch-chat-unverified", "缺少已记录派发效果；保持 failed，不猜测身份。");
            };
            let (Some(execution), Some(agent), Some(workspace), Some(tab), Some(pane)) = (execution.as_ref(), effects.agent_name.as_ref(), effects.workspace_id.as_ref(), effects.tab_id.as_ref(), effects.pane_id.as_ref()) else {
                return Decision::reject("dispatch-chat-unverified", "缺少原派发现实目标，无法关联真实 Chat；保持 failed，不猜测身份。");
            };
            let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
            let identity = match tokio::time::timeout_at(deadline, state.herdr.native_session(agent, execution.agent.label(), deadline)).await {
                Ok(Ok(identity)) => identity,
                Ok(Err(error)) => return Decision::reject("dispatch-chat-unverified", format!("未确认原 Chat：{}；保持 failed。", error.message())),
                Err(_) => return Decision::reject("dispatch-chat-unverified", "读取原 Chat 身份超时；保持 failed。"),
            };
            if !crate::zones::same_path(&identity.cwd, &execution.zone)
                || effects.session_id.as_ref().is_some_and(|session| session != &identity.session_id)
                || execution.chat.as_ref().is_some_and(|chat| chat.session_id != identity.session_id)
                || run.manager.as_ref().is_some_and(|chat| chat.session_id == identity.session_id)
                || run.nodes.values().any(|node| node.executions().any(|other| other.id != execution.id && other.chat.as_ref().is_some_and(|chat| chat.session_id == identity.session_id))) {
                return Decision::reject("dispatch-chat-mismatch", "原 Chat 的身份或 zone 不一致，或已属于其他执行；保持 failed，不替换执行。");
            }
            let chat = ChatReference { session_id: identity.session_id, workspace_id: workspace.clone(), tab_id: tab.clone(), pane_id: pane.clone(), agent_name: agent.clone() };
            let node = run.nodes.get_mut(&node_id).unwrap();
            node.execution.as_mut().unwrap().chat = Some(chat);
            node.status = "running".into();
            node.failure = None;
            true
        }
        DispatchFinding::NotCreated => {
            if execution.as_ref().and_then(|execution| execution.chat.as_ref()).is_some() || failure.as_ref().and_then(|failure| failure.effects.as_ref()).is_some_and(|effects| effects.session_id.is_some()) {
                return Decision::reject("dispatch-chat-already-known", "已取得真实 Chat 身份，不能核实为未创建；不要抹除已发生事实。");
            }
            let node = run.nodes.get_mut(&node_id).unwrap();
            node.status = "pending".into();
            node.execution = None;
            node.failure = None;
            true
        }
    };
    let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"reconcile_dispatch","nodeId":node_id,"before":"failed","after":run.nodes[&node_id].status,"finding":finding,"reason":reason,"failure":failure,"execution":execution,"currentExecution":run.nodes[&node_id].execution});
    Ok(if changed { Decision::Commit(event) } else { Decision::HistoryOnly(event) })
}

pub(super) fn recover(run: &mut Run, node_id: String, confirmed: bool, reason: String) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    let Some(node) = run.nodes.get(&node_id) else {
        return Decision::reject("node-unknown", "node 不在本次 Run 中。");
    };
    if node.status != "failed" || !node.failure.as_ref().is_some_and(|failure| failure.stage == "execution") || node.execution.as_ref().and_then(|execution| execution.chat.as_ref()).is_none() {
        return Decision::reject("node-not-recoverable", "仅有关联原 Chat 的执行阶段 failed 可 recover；派发失败请先核实。");
    }
    if !confirmed || reason.trim().is_empty() {
        return Decision::reject("recovery-confirmation-required", "请先由人工确认现场已修复、原 Chat 可继续，并说明依据。");
    }
    let node = run.nodes.get_mut(&node_id).unwrap();
    let failure = node.failure.take();
    node.status = "running".into();
    let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"recover","nodeId":node_id,"before":"failed","after":"running","reason":reason,"failure":failure,"execution":run.nodes[&node_id].execution});
    Ok(Decision::Commit(event))
}

pub(super) fn fail(run: &mut Run, node_id: String, reason: String) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    let Some(node) = run.nodes.get(&node_id) else {
        return Decision::reject("node-unknown", "node 不在本次 Run 中。");
    };
    if node.status == "failed" {
        return Ok(Decision::Unchanged);
    }
    if node.status != "running" {
        return Decision::reject("node-not-failable", "仅 running 执行可标记失败；重复 failed 保留首次事实。");
    }
    if reason.trim().is_empty() {
        return Decision::reject("failure-reason-required", "请记录已核实的执行失败原因。");
    }
    node.execution.as_ref().and_then(|execution| execution.chat.as_ref()).ok_or("执行缺少原 Chat 关联。")?;
    let failure = StartupFailure { stage: "execution".into(), message: reason.clone(), effects: None };
    let node = run.nodes.get_mut(&node_id).unwrap();
    node.status = "failed".into();
    node.failure = Some(failure);
    let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":"fail","nodeId":node_id,"before":"running","after":"failed","reason":reason,"failure":run.nodes[&node_id].failure,"execution":run.nodes[&node_id].execution});
    Ok(Decision::Commit(event))
}
