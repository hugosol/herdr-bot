//! Retry is a new execution, never recovery or rollback of an earlier Chat.
use super::*;
use super::completion::rejection;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryRequest {
    session_id: String,
    node_id: String,
    confirmed: bool,
    reason: String,
}

pub async fn manager_retry(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: RetryRequest = match crate::chat_command_body(request, "重试独立新执行").await {
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
            return rejection(&state, &run, "run-terminal", "Run 已结束，不接受 retry。").await;
        }
        if run.status == "paused" {
            return rejection(&state, &run, "run-paused", "paused 禁止 retry；请先显式 resume。").await;
        }
        let Some(node) = run.nodes.get(&input.node_id) else {
            return rejection(&state, &run, "node-unknown", "node 不在本次 Run 中。").await;
        };
        if node.status != "failed" {
            return rejection(&state, &run, "node-not-retryable", "仅 failed 执行可 retry。").await;
        }
        if !node.failure.as_ref().is_some_and(|failure| failure.stage == "execution") {
            return rejection(&state, &run, "dispatch-reconciliation-required", "派发失败必须先 reconcile_dispatch 核实原 Chat；不确定时保持 failed，不盲目 retry。").await;
        }
        if !input.confirmed || input.reason.trim().is_empty() {
            return rejection(&state, &run, "retry-confirmation-required", "请先由人工核实原 Chat 与副作用，确认需要独立新执行并说明依据；retry 不撤销旧副作用。").await;
        }
        let result = dispatch_one(&state, &mut run, &input.node_id, Some(&input.reason)).await;
        let mut response = snapshot(&state, &run, true).await?;
        response["result"] = result;
        Ok(Json(response).into_response())
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, "retry-failed", &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "retry-internal", &join.to_string()),
    }
}
