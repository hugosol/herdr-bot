//! Immutable chief reports. Delegation provenance grants sender eligibility;
//! current chief authority chooses the recipient, independently of the report.
use crate::{AppState, files::FileStore, report_delivery::{Notification, SavedReport, SubmitError, save_and_notify}};
use axum::{Json, extract::{ConnectInfo, Path, Query, Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{io, net::SocketAddr, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Content {
    background: String,
    execution: String,
    result: Outcome,
    summary: String,
    deliverables: Vec<String>,
    pending_decisions: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome { Success, Failure, Blocked }

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Report {
    id: String,
    at: u64,
    delegation_id: String,
    from: String,
    body: Content,
}

pub(crate) struct Reports { file: PathBuf, files: FileStore }
impl Reports {
    pub(crate) fn new(file: PathBuf) -> Self { Self { file, files: FileStore } }
    fn read(&self) -> io::Result<Vec<Report>> {
        self.files.with(std::slice::from_ref(&self.file), |access| {
            if !access.try_exists(&self.file)? { return Ok(Vec::new()); }
            access.read_jsonl(&self.file).map_err(io::Error::other)
        })?
    }
    fn save(&self, report: &Report) -> io::Result<()> {
        self.files.with(std::slice::from_ref(&self.file), |access| {
            if let Some(parent) = self.file.parent() { access.create_dir_all(parent)?; }
            if !access.try_exists(&self.file)? { access.create(&self.file, b"")?; }
            access.append_jsonl::<Report>(&self.file, report).map_err(io::Error::other)
        })?
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Submission { session_id: String, body: Content }

pub(crate) async fn submit(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return crate::rejected_owned(StatusCode::FORBIDDEN, "forbidden", "报告工具只接受同机来源。".into());
    }
    let input: Submission = match crate::chat_command_body(request, "提交 chief 报告").await {
        Ok(input) => input,
        Err(error) => return error,
    };
    if [&input.body.background, &input.body.execution, &input.body.summary].iter().any(|text| text.trim().is_empty()) {
        return crate::rejected_owned(StatusCode::BAD_REQUEST, "invalid-report", "报告必须保留需求背景、执行对象及结果说明。".into());
    }
    let delegation = match state.delegations.sender(&input.session_id) {
        Ok(Some(delegation)) => delegation,
        Ok(None) => return crate::rejected_owned(StatusCode::FORBIDDEN, "chief-report-unassociated", "只有 chief 实际委派的独立 Chat 或直接 Map manager 可以提交 chief 报告。".into()),
        Err(error) => return storage(error),
    };
    let submitted = save_and_notify(state.herdr.clone(), async move {
        let report = Report {
            id: crate::chats::new_creation_id(),
            at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            delegation_id: delegation.id, from: input.session_id, body: input.body,
        };
        let text = format!("Chief report\nReport: {}\n\n{}", report.id, serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?);
        // Persistence completes and releases file coordination before any
        // inventory query or delivery. A lookup failure cannot undo this fact.
        state.chief_reports.save(&report).map_err(|error| error.to_string())?;
        let target = recipient(&state).await;
        let diagnostic = json!({"event":"chief-report-delivery","reportId":report.id,"delegationId":report.delegation_id});
        Ok(SavedReport { report, target, text, diagnostic })
    }).await;
    match submitted {
        Ok(submitted) => Json(json!({"saved":true,"report":submitted.report,"notification":submitted.notification,"diagnostic":submitted.diagnostic,
            "guidance":"报告已保存。通知不代表可靠接收、呈现、用户已读或业务验收；不要因失败或结果不确定重新提交报告，系统不会自动重投。"})).into_response(),
        Err(SubmitError::Save(message)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"code":"report-save-failed","message":message,"saved":false,"notification":{"status":"not-attempted"}}))).into_response(),
        Err(SubmitError::Internal(error)) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "report-result-uncertain", format!("无法确认报告操作结果；请先读取已保存报告，不要重新提交：{error}")),
    }
}

async fn recipient(state: &AppState) -> Result<String, Notification> {
    let claim = state.chief.current().map_err(|error| Notification::not_attempted(format!("无法读取当前 chief 权威：{error:?}")))?
        .filter(|claim| claim.phase == "registered")
        .ok_or_else(|| Notification::not_attempted("没有可接收的当前 chief；报告仍已保存。"))?;
    let session_id = claim.session_id.as_deref().ok_or_else(|| Notification::not_attempted("当前 chief 尚无真实 Chat 身份。"))?;
    let selected = state.history.selection(session_id).await
        .map_err(|error| Notification::not_attempted(format!("无法读取当前 chief Chat：{error}")))?
        .ok_or_else(|| Notification::not_attempted("当前 chief 缺少真实 Chat 记录。"))?;
    let kind = selected.agent_type.as_deref().ok_or_else(|| Notification::not_attempted("当前 chief 缺少宿主身份。"))?;
    let targets = state.herdr.matching_targets(kind, &selected).await
        .map_err(|error| Notification::not_attempted(error.message()))?;
    if targets.len() != 1 {
        return Err(Notification::not_attempted("当前 chief 没有唯一在线的 Herdr 目标；未发起通知。"));
    }
    // Inventory may await external work. Never use a stale claim after lookup.
    let current = state.chief.qualify(session_id).map_err(|error| Notification::not_attempted(format!("当前 chief 已失去接收资格：{error:?}")))?;
    if current.claim_id != claim.claim_id {
        return Err(Notification::not_attempted("chief 权威在查询期间发生变化；未发起通知。"));
    }
    Ok(targets.into_iter().next().expect("one target").pane_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Filter { session_id: Option<String> }
pub(crate) async fn list(State(state): State<AppState>, Query(filter): Query<Filter>) -> Response {
    match state.chief_reports.read() {
        Ok(reports) => Json(json!({"reports":reports.into_iter().filter(|report| filter.session_id.as_ref().is_none_or(|id| id == &report.from)).collect::<Vec<_>>()})).into_response(),
        Err(error) => storage(error),
    }
}
pub(crate) async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.chief_reports.read() {
        Ok(reports) => match reports.into_iter().find(|report| report.id == id) {
            Some(report) => Json(report).into_response(),
            None => crate::rejected_owned(StatusCode::NOT_FOUND, "chief-report-not-found", "报告不存在。".into()),
        },
        Err(error) => storage(error),
    }
}
fn storage(error: io::Error) -> Response {
    crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "chief-report-storage-failed", format!("无法读取报告或其委派关联：{error}"))
}
