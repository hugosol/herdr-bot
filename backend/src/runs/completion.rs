//! Explicit handoff is independent of reports, Chat activity and downstream launch.
use super::*;

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
        if node.status == "completed" {
            return outcome(&state, &run, false, None).await;
        }
        if node.status != "running" && !(node.status == "failed" && node.failure.as_ref().is_some_and(|failure| failure.stage == "execution")) {
            return rejection(&state, &run, "node-not-completable", "仅 running 或人工处理后可交接的执行阶段 failed 可完成；pending 和派发失败不可直接完成。").await;
        }
        if input.reason.trim().is_empty() {
            return rejection(&state, &run, "completion-reason-required", "请说明可交接结果及限制；完成说明不可为空。").await;
        }
        let completion = Completion {
            id: crate::chats::new_creation_id(),
            at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
            reason: input.reason,
        };
        let node = run.nodes.get_mut(&input.node_id).unwrap();
        let before = std::mem::replace(&mut node.status, "completed".into());
        node.completion = Some(completion);
        save_run(&state, &run).await?;
        let completion = run.nodes[&input.node_id].completion.as_ref().unwrap();
        let event = serde_json::json!({"id":completion.id,"at":completion.at,"actor":completion.actor,"action":"complete","nodeId":input.node_id,"before":before,"after":"completed","reason":completion.reason,"execution":run.nodes[&input.node_id].execution});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, true, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "complete-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "complete-internal", &join.to_string()),
    }
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
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        if run.status == "completed" {
            return outcome(&state, &run, false, None).await;
        }
        if !matches!(run.status.as_str(), "running" | "paused") {
            return rejection(&state, &run, "run-not-finishable", "仅 running 或 paused 的 Run 可结束。").await;
        }
        if run.nodes.values().any(|node| node.status != "completed") {
            return rejection(&state, &run, "nodes-incomplete", "仍有 node 未 completed；请显式完成交接后再 finish。").await;
        }
        let finished = Finished {
            id: crate::chats::new_creation_id(),
            at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
        };
        let before = std::mem::replace(&mut run.status, "completed".into());
        run.finished = Some(finished);
        save_run(&state, &run).await?;
        let finished = run.finished.as_ref().unwrap();
        let event = serde_json::json!({"id":finished.id,"at":finished.at,"actor":finished.actor,"action":"finish","before":before,"after":"completed"});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, true, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "finish-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "finish-internal", &join.to_string()),
    }
}

pub async fn manager_pause(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    dispatch_eligibility(state, peer, request, "pause", "paused").await
}

pub async fn manager_resume(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    dispatch_eligibility(state, peer, request, "resume", "running").await
}

// Eligibility transitions never call the host: existing Chat activity and node
// facts are independent, and restoring eligibility is not a dispatch request.
async fn dispatch_eligibility(state: AppState, peer: SocketAddr, request: Request, action: &'static str, target: &'static str) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: StatusRequest = match crate::chat_command_body(request, "暂停或恢复派发资格").await {
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
        if run.status == target {
            return outcome(&state, &run, false, None).await;
        }
        let before = std::mem::replace(&mut run.status, target.into());
        save_run(&state, &run).await?;
        let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":action,"before":before,"after":target});
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, true, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{action}-failed"), &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{action}-internal"), &join.to_string()),
    }
}

pub(super) async fn outcome(state: &AppState, run: &Run, changed: bool, history_error: Option<String>) -> Result<Response, String> {
    let mut value = snapshot(state, run, true).await?;
    value["changed"] = serde_json::json!(changed);
    history_feedback(&mut value, history_error);
    Ok(Json(value).into_response())
}

// History failure is a partial effect, not a failed/rolled-back state commit.
// Preserve the OS detail while giving every action caller an actionable result.
pub(super) fn history_feedback(value: &mut serde_json::Value, history_error: Option<String>) {
    if history_error.is_some() {
        value["code"] = serde_json::json!("history-append-failed");
        value["stateCommitted"] = serde_json::json!(true);
        value["message"] = serde_json::json!("当前状态已提交，但执行历史追加失败；状态未回滚。请先查询现场，不要盲目重试。");
    }
    value["historyError"] = serde_json::json!(history_error);
}

pub(super) async fn rejection(state: &AppState, run: &Run, code: &str, message: &str) -> Result<Response, String> {
    let mut value = snapshot(state, run, true).await?;
    value["changed"] = serde_json::json!(false);
    value["code"] = serde_json::json!(code);
    value["message"] = serde_json::json!(message);
    Ok((StatusCode::CONFLICT, Json(value)).into_response())
}

pub(super) async fn record_transition(state: &AppState, id: &str, event: &serde_json::Value) -> Result<(), String> {
    let paths = Runs::paths(state, id);
    let owned = state.clone();
    let id = id.to_owned();
    let event = event.clone();
    state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
        owned.runs.ensure_retained(&id).await?;
        access.append_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1], &event)
    })).await.map_err(|e| e.to_string())?
}
