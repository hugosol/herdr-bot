//! Persistent metadata directory, independent of current registrations and shutdown intent.
//! Only explicit accepted registration events write it; native session content is never read.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    agent_type: Option<String>,
    cwd: Option<String>,
    first_recorded_at: u64,
    // Kept only in the on-disk document, never in the public read model.
    session_file: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryView<'a> {
    session_id: &'a str,
    session_name: Option<&'a str>,
    agent_type: Option<&'a str>,
    cwd: Option<&'a str>,
    first_recorded_at: u64,
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

pub struct History {
    file: PathBuf,
    // One instance serializes the entire read/merge/replace transaction, not just writes.
    lock: Mutex<()>,
}

impl History {
    pub fn new(file: PathBuf) -> Self {
        Self { file, lock: Mutex::new(()) }
    }

    fn read(&self) -> io::Result<Vec<HistoryRecord>> {
        match fs::read(&self.file) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    pub async fn register(
        &self,
        session_id: &str,
        session_name: Option<&str>,
        agent_type: Option<&str>,
        cwd: &str,
        session_file: Option<&str>,
    ) -> io::Result<()> {
        let _guard = self.lock.lock().await;
        let mut records = self.read()?;
        let session_name = valid_text(session_name);
        let agent_type = valid_text(agent_type);
        let cwd = valid_text(Some(cwd));
        let session_file = valid_text(session_file)
            .filter(|value| Path::new(value).is_absolute());

        if let Some(record) = records.iter_mut().find(|record| record.session_id == session_id) {
            update(&mut record.session_name, session_name);
            update(&mut record.agent_type, agent_type);
            update(&mut record.cwd, cwd);
            update(&mut record.session_file, session_file);
        } else {
            let first_recorded_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_millis() as u64;
            records.push(HistoryRecord {
                session_id: session_id.to_owned(),
                session_name: session_name.map(str::to_owned),
                agent_type: agent_type.map(str::to_owned),
                cwd: cwd.map(str::to_owned),
                first_recorded_at,
                session_file: session_file.map(str::to_owned),
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
