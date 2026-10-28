//! In-memory notifications are checks, never read receipts or handoff facts.
use super::*;
use std::sync::Arc;
use tokio::sync::watch;

pub(super) struct Signals {
    // Report generation is retained even when nobody is receiving. State-only
    // changes wake receivers without manufacturing a report notification.
    changed: watch::Sender<u64>,
    checked: Mutex<u64>,
}
impl Default for Signals {
    fn default() -> Self {
        Self { changed: watch::channel(0).0, checked: Mutex::new(0) }
    }
}
impl Runs {
    async fn signals(&self, id: &str) -> Arc<Signals> {
        self.signals.lock().await.entry(id.to_owned()).or_default().clone()
    }
    pub(super) async fn changed(&self, id: &str, report: bool) {
        self.signals(id).await.changed.send_modify(|generation| {
            if report { *generation += 1; }
        });
    }
}

// The operation permit covers only a coherent check, never the wait. Report
// save + notification and state save use the same permit as this check.
async fn check(state: &AppState, id: &str, signals: &Signals, expired: bool) -> Result<Option<serde_json::Value>, String> {
    let lock = state.runs.operations.lock().await.entry(id.to_owned()).or_default().clone();
    let _operation = lock.lock().await;
    let run = state.runs.read(state, id).await?;
    let generation = *signals.changed.borrow();
    let mut checked = signals.checked.lock().await;
    let reason = if run.status == "paused" { "paused" }
        else if run.status == "completed" { "completed" }
        else if generation > *checked { "notification" }
        else if !run.nodes.values().any(|node| node.status == "running") { "no-running" }
        else if expired { "timeout" }
        else { return Ok(None); };
    let mut value = snapshot(state, &run, false).await?;
    value["reason"] = serde_json::json!(reason);
    *checked = generation;
    Ok(Some(value))
}

pub async fn manager_collect(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: StatusRequest = match crate::chat_command_body(request, "收取报告").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return response,
    };
    let signals = state.runs.signals(&run.id).await;
    // Subscribe before checking, so a save between the check and wait is seen.
    let mut changed = signals.changed.subscribe();
    match check(&state, &run.id, &signals, false).await {
        Ok(Some(value)) => return Json(value).into_response(),
        Ok(None) => {},
        Err(message) => return error(StatusCode::INTERNAL_SERVER_ERROR, "collect-read-failed", &message),
    }
    let deadline = tokio::time::Instant::now() + state.collect_wait;
    // The response body owns the wait, not a spawned worker. Hyper drops it on
    // disconnect; the watch receiver and any operation acquisition go with it.
    // No snapshot is fabricated if a late read fails: the body fails instead.
    let body = futures_util::stream::once(async move {
        loop {
            let expired = tokio::select! {
                _ = tokio::time::sleep_until(deadline) => true,
                result = changed.changed() => {
                    result.map_err(std::io::Error::other)?;
                    false
                },
            };
            if let Some(value) = check(&state, &run.id, &signals, expired).await.map_err(std::io::Error::other)? {
                return serde_json::to_vec(&value).map_err(std::io::Error::other);
            }
        }
    });
    ([(axum::http::header::CONTENT_TYPE, "application/json")], axum::body::Body::from_stream(body)).into_response()
}
