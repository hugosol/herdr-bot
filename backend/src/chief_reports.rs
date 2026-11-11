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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    report_id: String,
    session_id: String,
    claim_id: String,
    at: u64,
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
    fn receipt(&self, id: &str) -> io::Result<Option<Receipt>> {
        let file = self.file.with_extension("receipts.jsonl");
        self.files.with(std::slice::from_ref(&file), |access| {
            if !access.try_exists(&file)? { return Ok(None); }
            let receipts: Vec<Receipt> = access.read_jsonl(&file).map_err(io::Error::other)?;
            Ok(receipts.into_iter().find(|receipt| receipt.report_id == id))
        })?
    }
    fn receive(&self, id: &str, claim: &crate::chief::Claim) -> io::Result<(Receipt, bool)> {
        let file = self.file.with_extension("receipts.jsonl");
        self.files.with(std::slice::from_ref(&file), |access| {
            if access.try_exists(&file)? {
                let receipts: Vec<Receipt> = access.read_jsonl(&file).map_err(io::Error::other)?;
                if let Some(receipt) = receipts.into_iter().find(|receipt| receipt.report_id == id) {
                    return Ok((receipt, false));
                }
            } else {
                if let Some(parent) = file.parent() { access.create_dir_all(parent)?; }
                access.create(&file, b"")?;
            }
            let receipt = Receipt {
                report_id: id.into(), session_id: claim.session_id.clone().expect("qualified chief"),
                claim_id: claim.claim_id.clone(), at: now(),
            };
            access.append_jsonl::<Receipt>(&file, &receipt).map_err(io::Error::other)?;
            Ok((receipt, true))
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
        let target = recipient(&state, &report.id).await;
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

async fn recipient(state: &AppState, report_id: &str) -> Result<String, Notification> {
    match state.chief_reports.receipt(report_id) {
        Ok(Some(_)) => return Err(Notification::not_attempted("报告已可靠接收；未再次通知，也不表示已呈现或验收。")),
        Ok(None) => {},
        Err(error) => return Err(Notification::not_attempted(format!("无法核实持久接收状态；未通知：{error}"))),
    }
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
    let eligible = state.chief.authorize(session_id, |current| {
        if current.claim_id != claim.claim_id { return Err(crate::chief::Error::NotCurrent); }
        // The receipt commit also holds chief authorization: this is ordered
        // with concurrent receipt/revocation, without holding files over I/O.
        state.chief_reports.receipt(report_id).map(|receipt| receipt.is_none()).map_err(crate::chief::Error::from)
    }).map_err(|error| Notification::not_attempted(format!("当前 chief 已失去接收资格或接收状态不可核实：{error:?}")))?;
    if !eligible { return Err(Notification::not_attempted("报告已可靠接收；未再次通知。")); }
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
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action { session_id: String }

fn find(state: &AppState, id: &str) -> Result<Report, Response> {
    state.chief_reports.read().map_err(storage)?.into_iter().find(|report| report.id == id)
        .ok_or_else(|| crate::rejected_owned(StatusCode::NOT_FOUND, "chief-report-not-found", "报告不存在。".into()))
}

pub(crate) async fn receipt(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if let Err(error) = find(&state, &id) { return error; }
    match state.chief_reports.receipt(&id) {
        Ok(receipt) => Json(json!({"received":receipt.is_some(),"receipt":receipt})).into_response(),
        Err(error) => storage(error),
    }
}

pub(crate) async fn receive(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return crate::rejected_owned(StatusCode::FORBIDDEN, "forbidden", "报告工具只接受同机来源。".into());
    }
    let input: Action = match crate::chat_command_body(request, "可靠接收 chief 报告").await {
        Ok(input) => input, Err(error) => return error,
    };
    let report = match find(&state, &id) { Ok(report) => report, Err(error) => return error };
    // Chief replacement and receipt commit share this linearization point. A
    // response lost after commit does not authorize processing a second time.
    match state.chief.authorize(&input.session_id, |claim| {
        state.chief_reports.receive(&id, claim).map_err(crate::chief::Error::from)
    }) {
        Ok((receipt, process)) => Json(json!({"received":true,"process":process,"receipt":receipt,"report":report,
            "guidance":"接收已持久保存，不代表已呈现、用户已读或验收。仅 process=true 授权本次处理；重复接收不得自动呈现。呈现中断后由用户明确要求查看原报告。"})).into_response(),
        Err(error) => error.response(),
    }
}

/// Explicit user-requested recovery, never scheduled on start/resume. Reading
/// is not another receive grant and cannot change an existing receipt.
pub(crate) async fn view(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return crate::rejected_owned(StatusCode::FORBIDDEN, "forbidden", "报告工具只接受同机来源。".into());
    }
    let input: Action = match crate::chat_command_body(request, "明确查看 chief 原报告").await {
        Ok(input) => input, Err(error) => return error,
    };
    let report = match find(&state, &id) { Ok(report) => report, Err(error) => return error };
    match state.chief.authorize(&input.session_id, |_| state.chief_reports.receipt(&id).map_err(crate::chief::Error::from)) {
        Ok(receipt) => Json(json!({"report":report,"received":receipt.is_some(),"receipt":receipt,"process":false,
            "guidance":"这是用户明确请求查看的原报告，不是自动补投或再次接收授权；接收记录不代表已呈现、已读或验收。"})).into_response(),
        Err(error) => error.response(),
    }
}

pub(crate) async fn resend(State(state): State<AppState>, Path(id): Path<String>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return crate::rejected_owned(StatusCode::FORBIDDEN, "forbidden", "报告工具只接受同机来源。".into());
    }
    // No body, recipient or new identity is accepted: approval refers to this
    // exact saved report. Revised content requires a separate approved submit.
    let input: Action = match crate::chat_command_body(request, "人工重发 chief 报告").await {
        Ok(input) => input, Err(error) => return error,
    };
    let report = match find(&state, &id) { Ok(report) => report, Err(error) => return error };
    match state.delegations.sender(&input.session_id) {
        Ok(Some(delegation)) if report.from == input.session_id && report.delegation_id == delegation.id => {},
        Ok(_) => return crate::rejected_owned(StatusCode::FORBIDDEN, "chief-report-unassociated", "只能由原被委派 Chat 明确确认后重发原报告。".into()),
        Err(error) => return storage(error),
    }
    let delivery_state = state.clone();
    let submitted = save_and_notify(state.herdr.clone(), async move {
        let text = format!("Chief report\nReport: {}\n\n{}", report.id, serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?);
        let target = recipient(&delivery_state, &report.id).await;
        let diagnostic = json!({"event":"chief-report-resend","reportId":report.id,"delegationId":report.delegation_id});
        Ok(SavedReport { report, target, text, diagnostic })
    }).await;
    match submitted {
        Ok(submitted) => match state.chief_reports.receipt(&id) {
            Ok(receipt) => Json(json!({"saved":true,"report":submitted.report,"received":receipt.is_some(),"receipt":receipt,
                "notification":submitted.notification,"diagnostic":submitted.diagnostic,
                "guidance":"原报告保持不变。已接收不代表已呈现、用户已读或验收；不会自动重投。呈现中断时请用户在当前 chief 明确要求查看。"})).into_response(),
            Err(error) => storage(error),
        },
        Err(SubmitError::Save(message)) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "report-resend-failed", message),
        Err(SubmitError::Internal(error)) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "report-result-uncertain", format!("无法确认重发结果；原报告仍保留，不要自动重试：{error}")),
    }
}

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }

fn storage(error: io::Error) -> Response {
    crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "chief-report-storage-failed", format!("无法读取报告或其委派关联：{error}"))
}
