//! Retry is a new execution, never recovery or rollback of an earlier Chat.
use super::*;
use super::operation::rejection;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RetryRequest {
    pub(crate) session_id: String,
    pub(crate) node_id: String,
    pub(crate) confirmed: bool,
    pub(crate) reason: String,
}

pub(crate) async fn manager_retry(state: AppState, input: RetryRequest) -> RunResult {
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return Err(response),
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
        if !run.inputs_ready {
            return rejection(&state, &run, "run-inputs-not-ready", "Run 输入尚未检查并确认；请先 prepare_inputs。").await;
        }
        let result = startup::node(&state, &mut run, &input.node_id, Some(&input.reason)).await;
        let mut response = snapshot(&state, &run, true).await?;
        response["result"] = result;
        Ok(RunOutcome::Value(response))
    }).await;
    match work {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(message)) => Err(error(FailureKind::Failed, "retry-failed", &message)),
        Err(join) => Err(error(FailureKind::Failed, "retry-internal", &join.to_string())),
    }
}
