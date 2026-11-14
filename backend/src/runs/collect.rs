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

pub(crate) async fn manager_collect(state: AppState, input: StatusRequest) -> Result<CollectOutcome, RunError> {
    let run = match manager_run(&state, &input.session_id).await {
        Ok(run) => run,
        Err(response) => return Err(response),
    };
    let signals = state.runs.signals(&run.id).await;
    // Subscribe before checking, so a save between the check and wait is seen.
    let changed = signals.changed.subscribe();
    match check(&state, &run.id, &signals, false).await {
        Ok(Some(value)) => return Ok(CollectOutcome::Ready(value)),
        Ok(None) => {},
        Err(message) => return Err(error(FailureKind::Failed, "collect-read-failed", &message)),
    }
    let deadline = tokio::time::Instant::now() + state.collect_wait;
    Ok(CollectOutcome::Waiting(CollectWait { state, run_id: run.id, signals, changed, deadline }))
}

pub(crate) struct CollectWait {
    state: AppState,
    run_id: String,
    signals: Arc<Signals>,
    changed: watch::Receiver<u64>,
    deadline: tokio::time::Instant,
}
impl CollectWait {
    // The caller owns this wait. Dropping it cancels any operation acquisition
    // and releases the receiver; no detached work or fabricated snapshot.
    pub(crate) async fn wait(mut self) -> Result<serde_json::Value, String> {
        loop {
            let expired = tokio::select! {
                _ = tokio::time::sleep_until(self.deadline) => true,
                result = self.changed.changed() => {
                    result.map_err(|error| error.to_string())?;
                    false
                },
            };
            if let Some(value) = check(&self.state, &self.run_id, &self.signals, expired).await? {
                return Ok(value);
            }
        }
    }
}

pub(crate) enum CollectOutcome {
    Ready(serde_json::Value),
    Waiting(CollectWait),
}
