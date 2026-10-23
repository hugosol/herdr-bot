//! Resume first locates an existing native conversation. No input or creation is
//! performed by this path; a successful empty query is explicitly not a resume.
use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::json;
use crate::{AppState, herdr::HerdrError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResumeRequest { session_id: String }

pub(crate) async fn post_resume(State(state): State<AppState>, Json(request): Json<ResumeRequest>) -> Response {
    let selected = match state.history.selection(&request.session_id).await {
        Ok(Some(selected)) => selected,
        Ok(None) => return crate::rejected_owned(StatusCode::NOT_FOUND, "unknown-chat", "找不到所选历史 Chat。".into()),
        Err(error) => return crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "history-unreadable", format!("读取历史会话失败：{error}")),
    };
    let kind = match selected.agent_type.as_deref() {
        Some(kind @ ("pi" | "omp")) => kind,
        _ => return crate::rejected_owned(StatusCode::BAD_REQUEST, "unsupported-agent", "无法确定受支持的 agent 类型；尚未恢复。".into()),
    };
    let targets = match state.herdr.matching_targets(kind, &selected.session_id).await {
        Ok(targets) => targets,
        Err(error) => return failure("lookup", &error),
    };
    let Some(target) = targets.first() else {
        return Json(json!({"outcome":"not-found","matchCount":0,"message":"已查询 Herdr，未找到匹配目标；尚未恢复，也没有启动新进程。"})).into_response();
    };
    if let Err(error) = state.herdr.focus_target(target).await { return failure("focus", &error); }
    let message = if targets.len() > 1 {
        format!("找到 {} 个匹配目标，已按 Workspace、Tab、Pane ID 的稳定顺序自动选择并定位。仅确认 Herdr 内部导航，不表示 Windows 窗口已置前。", targets.len())
    } else { "已定位已有对话；仅确认 Herdr 内部导航，不表示 Windows 窗口已置前。".into() };
    Json(json!({"outcome":"located","target":target,"matchCount":targets.len(),"message":message})).into_response()
}

fn failure(stage: &str, error: &HerdrError) -> Response {
    let code = if stage == "lookup" && matches!(error, HerdrError::UnexpectedResponse { command: "agent get native identity", .. }) {
        "identity-verification-failed"
    } else { error.code() };
    (StatusCode::BAD_GATEWAY, Json(json!({"outcome":"failed","stage":stage,"code":code,
        "message":format!("{}失败：{} 尚未恢复；没有创建 Tab、启动 agent 或提交输入。", if stage == "focus" { "Herdr 聚焦" } else { "Herdr 查找／身份核实" }, error.message())}))).into_response()
}
