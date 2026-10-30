//! Single-stage Run actions own their rules; this module owns their commit protocol.
use super::*;
use std::borrow::Cow;

pub(super) enum Action {
    Complete { node_id: String, reason: String },
    Finish,
    Pause,
    Resume,
    Fail { node_id: String, reason: String },
    Recover { node_id: String, confirmed: bool, reason: String },
    Reconcile { node_id: String, finding: recovery::DispatchFinding, reason: String },
}

impl Action {
    fn name(&self) -> &'static str {
        match self {
            Self::Complete { .. } => "complete",
            Self::Finish => "finish",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Fail { .. } => "fail",
            Self::Recover { .. } => "recover",
            Self::Reconcile { .. } => "reconcile-dispatch",
        }
    }

    async fn apply(self, state: &AppState, run: &mut Run) -> Result<Decision, String> {
        match self {
            Self::Complete { node_id, reason } => completion::complete(run, node_id, reason),
            Self::Finish => completion::finish(run),
            Self::Pause => completion::eligibility(run, "pause", "paused"),
            Self::Resume => completion::eligibility(run, "resume", "running"),
            Self::Fail { node_id, reason } => recovery::fail(run, node_id, reason),
            Self::Recover { node_id, confirmed, reason } => recovery::recover(run, node_id, confirmed, reason),
            Self::Reconcile { node_id, finding, reason } => recovery::reconcile(state, run, node_id, finding, reason).await,
        }
    }
}

/// A rejected or unchanged action never writes. History-only reconciliation is
/// distinct from a committed transition, even though both can fail to append.
pub(super) enum Decision {
    Rejected { code: &'static str, message: Cow<'static, str> },
    Unchanged,
    HistoryOnly(serde_json::Value),
    Commit(serde_json::Value),
}

impl Decision {
    pub(super) fn reject(code: &'static str, message: impl Into<Cow<'static, str>>) -> Result<Self, String> {
        Ok(Self::Rejected { code, message: message.into() })
    }
}

pub(super) async fn execute(state: AppState, session_id: &str, action: Action) -> Response {
    let run = match manager_run(&state, session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let name = action.name();
    // Keep accepted work alive if its HTTP caller disconnects. The operation
    // lock spans identity reconciliation too; file coordination never does.
    let work = tokio::spawn(async move {
        let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
        let _operation = lock.lock().await;
        let mut run = state.runs.read(&state, &run.id).await?;
        let completed_node = match &action {
            Action::Complete { node_id, .. } => Some(node_id.clone()),
            _ => None,
        };
        let decision = action.apply(&state, &mut run).await?;
        let (changed, event) = match decision {
            Decision::Rejected { code, message } => return rejection(&state, &run, code, &message).await,
            Decision::Unchanged => return outcome(&state, &run, false, None).await,
            Decision::HistoryOnly(event) => (false, event),
            Decision::Commit(event) => {
                // save_run publishes the current fact and notifies observers
                // before history, preserving the existing partial-effect contract.
                save_run(&state, &run).await?;
                if let Some(node_id) = completed_node {
                    completion::close_completed_tab(&state, &run, &node_id);
                }
                (true, event)
            }
        };
        let history_error = record_transition(&state, &run.id, &event).await.err();
        outcome(&state, &run, changed, history_error).await
    }).await;
    match work {
        Ok(Ok(response)) => response,
        Ok(Err(message)) => error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{name}-failed"), &message),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, &format!("{name}-internal"), &join.to_string()),
    }
}

async fn outcome(state: &AppState, run: &Run, changed: bool, history_error: Option<String>) -> Result<Response, String> {
    let mut value = snapshot(state, run, true).await?;
    value["changed"] = serde_json::json!(changed);
    history_feedback(&mut value, history_error);
    Ok(Json(value).into_response())
}

// Preserve the public feedback for both single-stage actions and multi-stage
// dispatch/retry. History failure never implies rollback or permission to retry.
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

async fn record_transition(state: &AppState, id: &str, event: &serde_json::Value) -> Result<(), String> {
    let paths = Runs::paths(state, id);
    let owned = state.clone();
    let id = id.to_owned();
    let event = event.clone();
    state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
        owned.runs.ensure_retained(&id).await?;
        access.append_jsonl::<BTreeMap<String, serde_json::Value>>(&paths[1], &event)
    })).await.map_err(|e| e.to_string())?
}
