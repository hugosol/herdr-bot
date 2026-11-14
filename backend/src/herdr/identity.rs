//! Native session identity acquisition and exact-file metadata reading.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::client::{HerdrClient, HerdrError, QUERY_TIMEOUT, nonempty};

/// Identity supplied by the official integration for the exact queried agent.
#[derive(Debug)]
pub struct NativeSession {
    pub session_id: String,
    pub cwd: String,
    pub session_file: Option<String>,
    pub(crate) startup_identity_path: Option<String>,
}

impl HerdrClient {
    /// One read-only identity attempt. The caller owns the overall acquisition
    /// deadline; this method never scans directories or invokes agent commands.
    pub async fn native_session(&self, target: &str, kind: &str, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
        self.read_native_session(target, kind, deadline, false, false).await
    }

    /// Only a just-started agent waits for an absent agent_session until the
    /// caller's deadline, or uses OMP's allocated-but-unwritten path as identity
    /// evidence. Recovery/reconciliation must not inherit either rule.
    pub async fn started_session(&self, target: &str, kind: &str, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
        self.read_native_session(target, kind, deadline, false, true).await
    }

    /// A delegated launch must independently verify its approved working
    /// directory even when the agent reuses an existing native session file.
    pub(crate) async fn started_session_with_live_cwd(&self, target: &str, kind: &str, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
        self.read_native_session(target, kind, deadline, true, true).await
    }

    /// Restoration must observe the process cwd, not merely the old file header.
    pub(crate) async fn restored_session(&self, target: &str, kind: &str, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
        self.read_native_session(target, kind, deadline, true, false).await
    }

    async fn read_native_session(&self, target: &str, kind: &str, deadline: tokio::time::Instant, require_live_cwd: bool, newly_started: bool) -> Result<NativeSession, HerdrError> {
        let value = loop {
            if tokio::time::Instant::now() >= deadline { return Err(native_error("native identity acquisition deadline expired")); }
            let value = self.run("agent get", &["agent".into(), "get".into(), target.into()], QUERY_TIMEOUT).await?;
            if tokio::time::Instant::now() >= deadline { return Err(native_error("native identity acquisition deadline expired")); }
            let agent = value.pointer("/result/agent").ok_or_else(|| native_error("missing result.agent"))?;
            // agent start can complete before Herdr publishes agent_session.
            // Only a just-started agent may wait for this absent read-only fact;
            // malformed identities and command failures are never retried here.
            if newly_started && agent.is_object()
                && agent.get("agent_session").is_none_or(Value::is_null) {
                tokio::time::sleep_until(deadline.min(tokio::time::Instant::now() + Duration::from_millis(100))).await;
                continue;
            }
            break value;
        };
        let agent = value.pointer("/result/agent").ok_or_else(|| native_error("missing result.agent"))?;
        let reference = agent.get("agent_session").filter(|value| !value.is_null())
            .ok_or_else(|| native_error("native identity is not available"))?;
        if !matches!(kind, "pi" | "omp")
            || reference.get("agent").and_then(Value::as_str) != Some(kind)
            || reference.get("source").and_then(Value::as_str) != Some(if kind == "omp" { "herdr:omp" } else { "herdr:pi" })
        {
            return Err(native_error("native reference agent/source does not match this creation"));
        }
        let reference_value = nonempty(reference.get("value"))
            .ok_or_else(|| native_error("native reference has no value"))?;
        match reference.get("kind").and_then(Value::as_str) {
            Some("id") => {
                let cwd = nonempty(agent.get("foreground_cwd")).or_else(|| nonempty(agent.get("cwd")))
                    .ok_or_else(|| native_error("native ID has no reported cwd"))?;
                Ok(NativeSession { session_id: reference_value.to_owned(), cwd: cwd.to_owned(), session_file: None, startup_identity_path: None })
            }
            Some("path") => {
                let path = PathBuf::from(reference_value);
                if !path.is_absolute() { return Err(native_error("native reference path is not absolute")); }
                let allow_title_metadata = kind == "omp";
                let pending = if newly_started {
                    pending_native_session(&path, nonempty(agent.get("foreground_cwd")).or_else(|| nonempty(agent.get("cwd"))))
                } else { None };
                let mut native = tokio::task::spawn_blocking(move || native_header_or_pending(path, allow_title_metadata, deadline, pending)).await
                    .map_err(|error| native_error(&format!("native header reader failed: {error}")))??;
                if require_live_cwd {
                    native.cwd = nonempty(agent.get("foreground_cwd")).or_else(|| nonempty(agent.get("cwd")))
                        .ok_or_else(|| native_error("restored process has no reported cwd"))?.to_owned();
                }
                Ok(native)
            }
            _ => Err(native_error("unsupported native reference kind")),
        }
    }
}

/// Only candidate identity data is skippable. Inventory, selected-history,
/// navigation and blocking-task failures still propagate to the caller.
pub(super) async fn candidate_native_id(entry: &Value, kind: &str, selected: &crate::chats::history::HistorySelection) -> Result<Option<String>, HerdrError> {
    let reference = (|| {
        let reference = entry.get("agent_session").filter(|reference| !reference.is_null())
            .ok_or_else(|| native_error("matching agent kind has no verifiable native identity"))?;
        if reference.get("agent").and_then(Value::as_str) != Some(kind)
            || reference.get("source").and_then(Value::as_str) != Some(if kind == "omp" { "herdr:omp" } else { "herdr:pi" }) {
            return Err(native_error("native identity agent/source is inconsistent"));
        }
        let value = nonempty(reference.get("value")).ok_or_else(|| native_error("native reference has no value"))?;
        match reference.get("kind").and_then(Value::as_str) {
            Some("id") => Ok((value, false)),
            Some("path") if std::path::Path::new(value).is_absolute() => Ok((value, true)),
            Some("path") => Err(native_error("native reference path is not absolute")),
            _ => Err(native_error("unsupported native reference kind")),
        }
    })();
    let identity = match reference {
        Ok((value, false)) => Ok(value.to_owned()),
        Ok((value, true)) => {
            let path = PathBuf::from(value);
            let allow_title = kind == "omp";
            let deadline = tokio::time::Instant::now() + QUERY_TIMEOUT;
            // Await errors indicate a runtime/task failure, not bad candidate
            // data: deliberately propagate them before handling reader results.
            let saved_path = matches!(kind, "omp" | "pi") && selected.startup_identity_path.as_deref() == Some(value);
            let pending = saved_pending_native_session(selected)
                .filter(|identity| selected.agent_type.as_deref() == Some(kind) && identity.session_file.as_deref() == Some(value));
            let result = tokio::task::spawn_blocking(move || native_header_or_pending(path, allow_title, deadline, pending)).await
                .map_err(|error| native_error(&format!("native identity reader failed: {error}")))?;
            if saved_path {
                let identity = result?;
                if identity.session_id != selected.session_id {
                    return Err(native_error("saved startup path belongs to another native session"));
                }
                Ok(identity.session_id)
            } else {
                result.map(|identity| identity.session_id)
            }
        }
        Err(error) => Err(error),
    };
    match identity {
        Ok(id) => Ok(Some(id)),
        Err(error) => {
            eprintln!("[herdr-bot] warning: resume candidate skipped kind={kind} pane={:?} reference={:?}: {}",
                entry.get("pane_id").and_then(Value::as_str), entry.get("agent_session"), error.message());
            Ok(None)
        }
    }
}

pub(crate) async fn read_native_id(path: &str, kind: &str) -> Result<String, HerdrError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() { return Err(native_error("native reference path is not absolute")); }
    let allow_title = kind == "omp";
    let deadline = tokio::time::Instant::now() + QUERY_TIMEOUT;
    tokio::task::spawn_blocking(move || native_header(path, allow_title, deadline)).await
        .map_err(|error| native_error(&format!("native identity reader failed: {error}")))?
        .map(|identity| identity.session_id)
}

pub(super) fn saved_pending_native_session(selected: &crate::chats::history::HistorySelection) -> Option<NativeSession> {
    if !matches!(selected.agent_type.as_deref(), Some("omp" | "pi")) { return None; }
    let path = Path::new(selected.startup_identity_path.as_deref()?);
    pending_native_session(path, selected.cwd.as_deref())
        .filter(|identity| identity.session_id == selected.session_id)
}

/// OMP and Pi default native filenames embed the already-minted session ID. This
/// is only evidence for a new launch or an exact persisted startup mapping;
/// it must never turn an arbitrary missing history file into a live identity.
fn pending_native_session(path: &Path, cwd: Option<&str>) -> Option<NativeSession> {
    if !path.is_absolute() { return None; }
    let cwd = cwd.filter(|cwd| Path::new(cwd).is_absolute())?;
    let name = path.file_name()?.to_str()?.strip_suffix(".jsonl")?;
    let (timestamp, id) = name.split_once('_')?;
    let stamp = timestamp.as_bytes();
    if stamp.len() != 24 || !stamp.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 | 13 | 16 | 19 => *byte == b'-',
        10 => *byte == b'T',
        23 => *byte == b'Z',
        _ => byte.is_ascii_digit(),
    }) { return None; }
    let number = |range: std::ops::Range<usize>| timestamp.get(range)?.parse::<u32>().ok();
    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let days = match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return None,
    };
    if !(1..=days).contains(&day) || number(11..13)? > 23 || number(14..16)? > 59 || number(17..19)? > 59 {
        return None;
    }
    if id.len() != 36 || !id.bytes().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
    }) { return None; }
    let file = path.to_str()?.to_owned();
    Some(NativeSession { session_id: id.to_owned(), cwd: cwd.to_owned(), session_file: Some(file.clone()), startup_identity_path: Some(file) })
}

pub(super) fn native_error(detail: &str) -> HerdrError {
    HerdrError::UnexpectedResponse { command: "agent get native identity", detail: detail.to_owned() }
}

/// Read only the metadata prefix of this exact file. OMP reserves a title line
/// before the session header; Pi starts with the session header. Neither agent
/// is inferred from the JSON shape, and conversation entries are never imported.
fn native_header(path: PathBuf, allow_title_metadata: bool, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
    native_header_or_pending(path, allow_title_metadata, deadline, None)
}

pub(super) fn native_header_or_pending(path: PathBuf, allow_title_metadata: bool, deadline: tokio::time::Instant, pending: Option<NativeSession>) -> Result<NativeSession, HerdrError> {
    use std::io::Read;
    if tokio::time::Instant::now() >= deadline {
        return Err(native_error("native identity acquisition deadline expired"));
    }
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && pending.is_some() => {
            return Ok(pending.unwrap());
        }
        Err(error) => return Err(native_error(&format!("native header unavailable: {error}"))),
    };
    let mut line = Vec::with_capacity(512);
    let mut remaining = 64 * 1024;
    loop {
        line.clear();
        loop {
            // In-flight kernel I/O is not forcibly cancellable, but a completed
            // operation must not initiate further reads after the total budget.
            if tokio::time::Instant::now() >= deadline {
                return Err(native_error("native identity acquisition deadline expired"));
            }
            if remaining == 0 { return Err(native_error("native metadata prefix exceeds 64 KiB")); }
            remaining -= 1;
            // No read-ahead into conversation content; only identity metadata is needed.
            let mut byte = [0];
            if file.read(&mut byte).map_err(|error| native_error(&format!("native header read failed: {error}")))? == 0 {
                return Err(native_error("native header is not complete yet"));
            }
            if byte[0] == b'\n' { break; }
            line.push(byte[0]);
        }
        if line.iter().all(u8::is_ascii_whitespace) { continue; }
        let metadata: Value = serde_json::from_slice(&line)
            .map_err(|error| native_error(&format!("native metadata is not valid JSON: {error}")))?;
        match metadata.get("type").and_then(Value::as_str) {
            Some("title") if allow_title_metadata => continue,
            Some("session") => {
                let id = nonempty(metadata.get("id")).ok_or_else(|| native_error("native header has no ID"))?;
                let cwd = nonempty(metadata.get("cwd")).ok_or_else(|| native_error("native header has no cwd"))?;
                return Ok(NativeSession { session_id: id.to_owned(), cwd: cwd.to_owned(), session_file: Some(path.to_string_lossy().into_owned()), startup_identity_path: None });
            }
            _ => return Err(native_error("expected native session header, not conversation content")),
        }
    }
}

