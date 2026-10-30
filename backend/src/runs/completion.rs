//! Explicit handoff is independent of reports, Chat activity and downstream launch.
use super::*;
use super::operation::{Action, Decision};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompleteRequest {
    session_id: String,
    node_id: String,
    reason: String,
}

pub async fn manager_complete(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: CompleteRequest = match crate::chat_command_body(request, "交接完成").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    operation::execute(state, &input.session_id, Action::Complete { node_id: input.node_id, reason: input.reason }).await
}

pub async fn manager_finish(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: StatusRequest = match crate::chat_command_body(request, "结束 Run").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    operation::execute(state, &input.session_id, Action::Finish).await
}

pub async fn manager_pause(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    dispatch_eligibility(state, peer, request, Action::Pause).await
}

pub async fn manager_resume(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    dispatch_eligibility(state, peer, request, Action::Resume).await
}

// Eligibility transitions never call the host: existing Chat activity and node
// facts are independent, and restoring eligibility is not a dispatch request.
async fn dispatch_eligibility(state: AppState, peer: SocketAddr, request: Request, action: Action) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: StatusRequest = match crate::chat_command_body(request, "暂停或恢复派发资格").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    operation::execute(state, &input.session_id, action).await
}

pub(super) fn complete(run: &mut Run, node_id: String, reason: String) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    let Some(node) = run.nodes.get(&node_id) else {
        return Decision::reject("node-unknown", "node 不在本次 Run 中。");
    };
    if node.status == "completed" {
        return Ok(Decision::Unchanged);
    }
    if node.status != "running" && !(node.status == "failed" && node.failure.as_ref().is_some_and(|failure| failure.stage == "execution")) {
        return Decision::reject("node-not-completable", "仅 running 或人工处理后可交接的执行阶段 failed 可完成；pending 和派发失败不可直接完成。");
    }
    if reason.trim().is_empty() {
        return Decision::reject("completion-reason-required", "请说明可交接结果及限制；完成说明不可为空。");
    }
    let completion = Completion {
        id: crate::chats::new_creation_id(),
        at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
        reason: reason,
    };
    let node = run.nodes.get_mut(&node_id).unwrap();
    let before = std::mem::replace(&mut node.status, "completed".into());
    node.completion = Some(completion);
    let completion = run.nodes[&node_id].completion.as_ref().unwrap();
    let event = serde_json::json!({"id":completion.id,"at":completion.at,"actor":completion.actor,"action":"complete","nodeId":node_id,"before":before,"after":"completed","reason":completion.reason,"execution":run.nodes[&node_id].execution});
    Ok(Decision::Commit(event))
}

pub(super) fn finish(run: &mut Run) -> Result<Decision, String> {
    if run.status == "completed" {
        return Ok(Decision::Unchanged);
    }
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-not-finishable", "仅 running 或 paused 的 Run 可结束。");
    }
    if run.nodes.values().any(|node| node.status != "completed") {
        return Decision::reject("nodes-incomplete", "仍有 node 未 completed；请显式完成交接后再 finish。");
    }
    let finished = Finished {
        id: crate::chats::new_creation_id(),
        at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
    };
    let before = std::mem::replace(&mut run.status, "completed".into());
    run.finished = Some(finished);
    let finished = run.finished.as_ref().unwrap();
    let event = serde_json::json!({"id":finished.id,"at":finished.at,"actor":finished.actor,"action":"finish","before":before,"after":"completed"});
    Ok(Decision::Commit(event))
}

pub(super) fn eligibility(run: &mut Run, action: &str, target: &str) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    if run.status == target {
        return Ok(Decision::Unchanged);
    }
    let before = std::mem::replace(&mut run.status, target.into());
    let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":action,"before":before,"after":target});
    Ok(Decision::Commit(event))
}
