//! Shared save-before-notify orchestration; report paths own storage and policy.
//!
//! The save future uses the existing `files` authority and returns only after
//! releasing its file/business guards. No external effect runs before it succeeds.
//! An accepted submission outlives its HTTP caller, but creates no retry or queue.
use crate::herdr::HerdrClient;
use serde::Serialize;
use std::{future::Future, io::Write, sync::Arc};

/// A saved fact plus the single notification chosen by its report path.
/// `diagnostic` contains only body-free identity fields and the event name.
pub(crate) struct SavedReport<T> {
    pub report: T,
    pub target: Result<String, Notification>,
    pub text: String,
    pub diagnostic: serde_json::Value,
}

#[derive(Serialize)]
pub(crate) struct Notification {
    status: &'static str,
    reason: String,
}

impl Notification {
    pub(crate) fn not_attempted(reason: impl Into<String>) -> Self {
        Self { status: "not-attempted", reason: reason.into() }
    }
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
        let (notification, diagnostic_reason) = match target {
            Ok(target) => match herdr.prompt(&target, &text).await {
                Ok(()) => (Notification { status: "submitted", reason: "Herdr 已确认输入提交；不代表可靠接收、已呈现或已阅读。".into() }, "input-submission-confirmed"),
                // Host details may echo the input. Only the typed category
                // belongs in diagnostics; the caller receives the full failure.
                Err(error) => {
                    use crate::herdr::HerdrError;
                    let status = match &error {
                        HerdrError::NotStarted(_) | HerdrError::CommandFailed { .. } => "failed",
                        _ => "uncertain",
                    };
                    (Notification { status, reason: error.message() }, error.code())
                }
            },
            Err(notification) => (notification, "no-authorized-recipient"),
        };
        diagnostic["result"] = notification.status.into();
        diagnostic["reason"] = diagnostic_reason.into();
        // A broken diagnostic sink must not panic or cause another attempt.
        let diagnostic_status = if writeln!(std::io::stderr().lock(), "{diagnostic}").is_ok() { "written" } else { "failed" };
        Ok(SubmittedReport {
            report,
            notification,
            diagnostic: Diagnostic { status: diagnostic_status },
        })
    }).await.map_err(SubmitError::Internal)?.map_err(SubmitError::Save)
}
