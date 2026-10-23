//! Persistent metadata directory, independent of current registrations and shutdown intent.
//! Accepted registration and confirmed zone creation write metadata only; native bodies are never imported.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::clock::Clock;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    session_id: String,
    session_name: Option<String>,
    #[serde(default)]
    name_from_zone: bool,
    agent_type: Option<String>,
    cwd: Option<String>,
    first_recorded_at: u64,
    // Kept only in the on-disk document, never in the public read model.
    session_file: Option<String>,
    // Original location is write-once metadata, never updated by Resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_workspace_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryView<'a> {
    session_id: &'a str,
    session_name: Option<&'a str>,
    agent_type: Option<&'a str>,
    cwd: Option<&'a str>,
    first_recorded_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_workspace_id: Option<&'a str>,
}

const PAGE_SIZE: usize = 50;

#[derive(Deserialize, Default)]
pub(crate) struct HistoryQuery {
    page: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryList<'a> {
    sessions: Vec<HistoryView<'a>>,
    page: usize,
    page_size: usize,
    total: usize,
    total_pages: usize,
}

pub(crate) struct HistorySelection {
    pub session_id: String,
    pub agent_type: Option<String>,
    pub cwd: Option<String>,
    pub session_file: Option<String>,
    pub session_name: Option<String>,
    pub original_workspace_id: Option<String>,
}

pub struct History {
    file: PathBuf,
    clock: Arc<dyn Clock>,
    // One instance serializes the entire read/merge/replace transaction, not just writes.
    lock: Mutex<()>,
}

impl History {
    pub fn new(file: PathBuf, clock: Arc<dyn Clock>) -> Self {
        Self { file, clock, lock: Mutex::new(()) }
    }

    fn read(&self) -> io::Result<Vec<HistoryRecord>> {
        match fs::read(&self.file) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// Resolve only a persisted Chat selection, never current registration state.
    pub(crate) async fn selection(&self, session_id: &str) -> io::Result<Option<HistorySelection>> {
        let _guard = self.lock.lock().await;
        Ok(self.read()?.into_iter().find(|record| record.session_id == session_id)
            .map(|record| HistorySelection { session_id: record.session_id, agent_type: record.agent_type,
                cwd: record.cwd, session_file: record.session_file, session_name: record.session_name, original_workspace_id: record.original_workspace_id }))
    }

    pub async fn record_zone(
        &self,
        session_id: &str,
        name: &str,
        agent_type: &str,
        cwd: &str,
        session_file: Option<&str>,
        original_workspace_id: Option<&str>,
    ) -> io::Result<()> {
        self.record(session_id, Some(name), Some(agent_type), cwd, session_file, original_workspace_id, true).await
    }

    pub async fn register(
        &self,
        session_id: &str,
        session_name: Option<&str>,
        agent_type: Option<&str>,
        cwd: &str,
        session_file: Option<&str>,
        original_workspace_id: Option<&str>,
    ) -> io::Result<()> {
        self.record(session_id, session_name, agent_type, cwd, session_file, original_workspace_id, false).await
    }

    async fn record(
        &self,
        session_id: &str,
        session_name: Option<&str>,
        agent_type: Option<&str>,
        cwd: &str,
        session_file: Option<&str>,
        original_workspace_id: Option<&str>,
        from_zone: bool,
    ) -> io::Result<()> {
        let _guard = self.lock.lock().await;
        let mut records = self.read()?;
        let session_name = valid_text(session_name);
        let agent_type = valid_text(agent_type);
        let cwd = valid_text(Some(cwd));
        let original_workspace_id = valid_text(original_workspace_id);
        let session_file = valid_text(session_file)
            .filter(|value| Path::new(value).is_absolute());

        if let Some(record) = records.iter_mut().find(|record| record.session_id == session_id) {
            if from_zone || !record.name_from_zone {
                update(&mut record.session_name, session_name);
                if from_zone && session_name.is_some() { record.name_from_zone = true; }
            }
            update(&mut record.agent_type, agent_type);
            update(&mut record.cwd, cwd);
            update(&mut record.session_file, session_file);
            if record.original_workspace_id.is_none() {
                record.original_workspace_id = original_workspace_id.map(str::to_owned);
            }
        } else {
            let first_recorded_at = self.clock.unix_millis();
            records.push(HistoryRecord {
                session_id: session_id.to_owned(),
                session_name: session_name.map(str::to_owned),
                name_from_zone: from_zone && session_name.is_some(),
                agent_type: agent_type.map(str::to_owned),
                cwd: cwd.map(str::to_owned),
                first_recorded_at,
                session_file: session_file.map(str::to_owned),
                original_workspace_id: original_workspace_id.map(str::to_owned),
            });
        }
        self.save(&records)
    }

    // Read errors propagate before save is reached. The existing document is never
    // truncated in place, and the lock keeps concurrent accepted events from losing rows.
    fn save(&self, records: &[HistoryRecord]) -> io::Result<()> {
        if let Some(parent) = self.file.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut temporary_name = self.file.as_os_str().to_os_string();
        temporary_name.push(".tmp");
        let temporary = PathBuf::from(temporary_name);
        let result = (|| {
            let mut file = fs::File::create(&temporary)?;
            serde_json::to_writer(&mut file, records).map_err(io::Error::other)?;
            file.flush()?;
            file.sync_all()?;
            fs::rename(&temporary, &self.file)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn valid_text(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

fn update(target: &mut Option<String>, value: Option<&str>) {
    if let Some(value) = value {
        if target.as_deref() != Some(value) {
            *target = Some(value.to_owned());
        }
    }
}

pub(crate) async fn get_history(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> Response {
    let _guard = state.history.lock.lock().await;
    match state.history.read() {
        Ok(mut records) => {
            records.sort_by(|a, b| {
                b.first_recorded_at.cmp(&a.first_recorded_at)
                    .then_with(|| a.session_id.cmp(&b.session_id))
            });
            let total = records.len();
            let total_pages = total.div_ceil(PAGE_SIZE).max(1);
            let page = query.page.unwrap_or(1).clamp(1, total_pages);
            let sessions = records.iter().skip((page - 1) * PAGE_SIZE).take(PAGE_SIZE).map(|record| HistoryView {
                session_id: &record.session_id,
                session_name: record.session_name.as_deref(),
                agent_type: record.agent_type.as_deref(),
                cwd: record.cwd.as_deref(),
                first_recorded_at: record.first_recorded_at,
                original_workspace_id: record.original_workspace_id.as_deref(),
            }).collect();
            Json(HistoryList { sessions, page, page_size: PAGE_SIZE, total, total_pages }).into_response()
        }
        Err(error) => crate::rejected_owned(
            StatusCode::INTERNAL_SERVER_ERROR,
            "history-unreadable",
            format!("读取历史会话失败：{error}"),
        ),
    }
}
