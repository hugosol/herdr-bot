//! Shared save-before-notify orchestration; report paths own storage and policy.
//!
//! The save future uses the existing `files` authority and returns only after
//! releasing its file/business guards. No external effect runs before it succeeds.
//! An accepted submission outlives its HTTP caller, but creates no retry or queue.
use crate::chats::herdr::HerdrClient;
use serde::Serialize;
use std::{future::Future, io::Write, sync::Arc};

/// A saved fact plus the single notification chosen by its report path.
/// `diagnostic` contains only body-free identity fields and the event name.
pub(crate) struct SavedReport<T> {
    pub report: T,
    pub target: String,
    pub text: String,
    pub diagnostic: serde_json::Value,
}

#[derive(Serialize)]
pub(crate) struct Notification {
    status: &'static str,
    reason: String,
}

#[derive(Serialize)]
pub(crate) struct Diagnostic {
    status: &'static str,
}

pub(crate) struct SubmittedReport<T> {
    pub report: T,
    pub notification: Notification,
    pub diagnostic: Diagnostic,
}

pub(crate) enum SubmitError {
    Save(String),
    Internal(tokio::task::JoinError),
}

/// Finish the accepted save and attempt notification once, independently of the
/// request lifetime. A notification/diagnostic failure never changes the fact.
pub(crate) async fn save_and_notify<T: Send + 'static>(
    herdr: Arc<HerdrClient>,
    save: impl Future<Output = Result<SavedReport<T>, String>> + Send + 'static,
) -> Result<SubmittedReport<T>, SubmitError> {
    tokio::spawn(async move {
        let SavedReport { report, target, text, mut diagnostic } = save.await?;
        let (status, reason, diagnostic_reason) = match herdr.prompt(&target, &text).await {
            Ok(()) => ("submitted", "Herdr 已确认输入提交；不代表已阅读。".to_owned(), "input-submission-confirmed"),
            // Host details may echo the input. Only the typed category belongs
            // in diagnostics; the caller still receives the detailed failure.
            Err(error) => ("failed", error.message(), error.code()),
        };
        diagnostic["result"] = status.into();
        diagnostic["reason"] = diagnostic_reason.into();
        // A broken diagnostic sink must not panic or cause another attempt.
        let diagnostic_status = if writeln!(std::io::stderr().lock(), "{diagnostic}").is_ok() { "written" } else { "failed" };
        Ok(SubmittedReport {
            report,
            notification: Notification { status, reason },
            diagnostic: Diagnostic { status: diagnostic_status },
        })
    }).await.map_err(SubmitError::Internal)?.map_err(SubmitError::Save)
}
