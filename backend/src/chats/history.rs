//! Persistent metadata directory, independent of current registrations and shutdown intent.
//! Accepted registration and confirmed zone creation write metadata only; native bodies are never imported.

use std::io;
use crate::files::{FileAccess, FileStore};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::clock::Clock;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

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
    // Exact path accepted before the first native write. Never supplied by register.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    startup_identity_path: Option<String>,
    // Trusted creation provenance is write-once and never accepted from register.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_task_id: Option<String>,
    // Reject the retired association rather than silently losing trusted provenance.
    // Other unknown history fields remain ignored, as before the terminology cutover.
    #[serde(rename = "sourceBotId", default, skip_serializing, deserialize_with = "reject_legacy_source_association")]
    _legacy_source_association: (),
    // Original location is write-once metadata, never updated by Resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memo: Option<String>,
}

fn reject_legacy_source_association<'de, D: serde::Deserializer<'de>>(
    _: D,
) -> Result<(), D::Error> {
    Err(serde::de::Error::custom(
        "sourceBotId is no longer supported; migrate history to sourceTaskId before starting the service",
    ))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    original_workspace_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memo: Option<&'a str>,
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
    pub startup_identity_path: Option<String>,
    pub session_name: Option<String>,
    pub original_workspace_id: Option<String>,
    pub source_task_id: Option<String>,
}

pub struct History {
    file: PathBuf,
    clock: Arc<dyn Clock>,
    files: FileStore,
}

impl History {
    pub fn new(file: PathBuf, clock: Arc<dyn Clock>) -> Self {
        Self { file, clock, files: FileStore::default() }
    }

    fn read(&self) -> io::Result<Vec<HistoryRecord>> {
        self.files.with(std::slice::from_ref(&self.file), |access| self.read_scoped(access))?
    }

    fn read_scoped(&self, access: &FileAccess) -> io::Result<Vec<HistoryRecord>> {
        match access.read(&self.file) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// Resolve only a persisted Chat selection, never current registration state.
    pub(crate) async fn selection(&self, session_id: &str) -> io::Result<Option<HistorySelection>> {
        Ok(self.read()?.into_iter().find(|record| record.session_id == session_id)
            .map(|record| HistorySelection { session_id: record.session_id, agent_type: record.agent_type,
                cwd: record.cwd, session_file: record.session_file, startup_identity_path: record.startup_identity_path, session_name: record.session_name,
                original_workspace_id: record.original_workspace_id, source_task_id: record.source_task_id }))
    }

    pub(crate) async fn record_zone(
        &self,
        session_id: &str,
        name: &str,
        agent_type: &str,
        cwd: &str,
        session_file: Option<&str>,
        original_workspace_id: Option<&str>,
        source_task_id: &str,
        startup_identity_path: Option<&str>,
    ) -> io::Result<()> {
        self.record(session_id, Some(name), Some(agent_type), cwd, session_file, original_workspace_id, Some(source_task_id), true, startup_identity_path).await
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
        self.record(session_id, session_name, agent_type, cwd, session_file, original_workspace_id, None, false, None).await
    }

    async fn record(
        &self,
        session_id: &str,
        session_name: Option<&str>,
        agent_type: Option<&str>,
        cwd: &str,
        session_file: Option<&str>,
        original_workspace_id: Option<&str>,
        source_task_id: Option<&str>,
        from_zone: bool,
        startup_identity_path: Option<&str>,
    ) -> io::Result<()> {
        self.files.with(std::slice::from_ref(&self.file), |access| {
            access.update_json(
                &self.file,
                |access| self.read_scoped(access),
                |records| {
                    let session_name = valid_text(session_name);
                    let agent_type = valid_text(agent_type);
                    let cwd = valid_text(Some(cwd));
                    let original_workspace_id = valid_text(original_workspace_id);
                    let source_task_id = valid_text(source_task_id);
                    let session_file = valid_text(session_file)
                        .filter(|value| Path::new(value).is_absolute());
                    let startup_identity_path = valid_text(startup_identity_path)
                        .filter(|path| from_zone && matches!(agent_type, Some("omp" | "pi")) && Some(*path) == session_file);

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
                        if record.source_task_id.is_none() {
                            record.source_task_id = source_task_id.map(str::to_owned);
                        }
                        if record.startup_identity_path.is_none() {
                            record.startup_identity_path = startup_identity_path.map(str::to_owned);
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
                            startup_identity_path: startup_identity_path.map(str::to_owned),
                            source_task_id: source_task_id.map(str::to_owned),
                            _legacy_source_association: (),
                            original_workspace_id: original_workspace_id.map(str::to_owned),
                            memo: None,
                        });
                    }
                    Ok(())
                },
                |records| self.encode(access, records),
                |error| error,
            ).map(|(_, result)| result)
        })?
    }

    async fn set_memo(&self, session_id: &str, memo: &str) -> io::Result<bool> {
        self.files.with(std::slice::from_ref(&self.file), |access| {
            let records = self.read_scoped(access)?;
            let Some(index) = records.iter().position(|record| record.session_id == session_id) else {
                return Ok(false);
            };
            access.update_json(
                &self.file,
                |_| Ok(records),
                |records| {
                    records[index].memo = Some(memo.to_owned());
                    Ok(true)
                },
                |records| self.encode(access, records),
                |error| error,
            ).map(|(_, result)| result)
        })?
    }

    // Parent creation remains specific to history; publication belongs to FileAccess.
    fn encode(&self, access: &FileAccess, records: &[HistoryRecord]) -> io::Result<Vec<u8>> {
        if let Some(parent) = self.file.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            access.create_dir_all(parent)?;
        }
        serde_json::to_vec(records).map_err(io::Error::other)
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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoRequest {
    session_id: String,
    memo: String,
}

pub(crate) async fn put_memo(
    State(state): State<AppState>,
    Json(request): Json<MemoRequest>,
) -> Response {
    match state.history.set_memo(&request.session_id, &request.memo).await {
        Ok(true) => Json(request).into_response(),
        Ok(false) => crate::rejected_owned(StatusCode::NOT_FOUND, "unknown-chat", "找不到所选历史 Chat。".into()),
        Err(error) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "history-save-failed", format!("保存备忘失败：{error}")),
    }
}

pub(crate) async fn get_history(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> Response {
    let snapshot = state.history.read();
    match snapshot {
        Ok(mut records) => {
            records.sort_by(|a, b| {
                b.first_recorded_at.cmp(&a.first_recorded_at)
                    .then_with(|| a.session_id.cmp(&b.session_id))
            });
            let total = records.len();
            let total_pages = total.div_ceil(PAGE_SIZE).max(1);
            let page = query.page.unwrap_or(1).clamp(1, total_pages);
            let workspaces = if records.iter().skip((page - 1) * PAGE_SIZE).take(PAGE_SIZE)
                .any(|record| record.original_workspace_id.is_some()) {
                state.herdr.workspace_labels().await.unwrap_or_default()
            } else {
                Vec::new()
            };
            let sessions = records.iter().skip((page - 1) * PAGE_SIZE).take(PAGE_SIZE).map(|record| HistoryView {
                session_id: &record.session_id,
                session_name: record.session_name.as_deref(),
                agent_type: record.agent_type.as_deref(),
                cwd: record.cwd.as_deref(),
                first_recorded_at: record.first_recorded_at,
                original_workspace_id: record.original_workspace_id.as_deref(),
                original_workspace_name: record.original_workspace_id.as_deref()
                    .and_then(|id| workspaces.iter().find(|workspace| workspace.id == id))
                    .map(|workspace| workspace.label.as_str()),
                memo: record.memo.as_deref(),
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
