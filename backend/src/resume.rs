//! Resume first locates a native conversation. Only a successful empty query
//! permits a new process; restoration never submits input or applies a Bot preset.
use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::json;
use crate::{AppState, herdr::HerdrError};
use std::{collections::HashMap, sync::{Arc, Mutex}};

/// Exact saved Chat identities share a non-waiting permit within one app instance.
/// Only selected history records enter this map; unrelated Chats never share a permit.
#[derive(Default)]
pub(crate) struct ResumeExecutions {
    chats: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl ResumeExecutions {
    fn try_qualify(&self, session_id: &str) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        let permit = {
            let mut chats = self.chats.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            match chats.get(session_id) {
                Some(permit) => Arc::clone(permit),
                None => {
                    let permit = Arc::new(tokio::sync::Mutex::new(()));
                    chats.insert(session_id.to_owned(), Arc::clone(&permit));
                    permit
                }
            }
        };
        permit.try_lock_owned().ok()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResumeRequest { session_id: String }

pub(crate) async fn post_resume(State(state): State<AppState>, Json(request): Json<ResumeRequest>) -> Response {
    let selected = match state.history.selection(&request.session_id).await {
        Ok(Some(selected)) => selected,
        Ok(None) => return crate::rejected_owned(StatusCode::NOT_FOUND, "unknown-chat", "找不到所选历史 Chat。".into()),
        Err(error) => return crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "history-unreadable", format!("读取历史会话失败：{error}")),
    };
    let Some(permit) = state.resume_executions.try_qualify(&selected.session_id) else {
        return crate::rejected_owned(StatusCode::CONFLICT, "resume-in-progress",
            "同一 Chat 已有 Resume 正在执行；本次请求未执行，也不会排队。请等待完成后由用户重新发起。".into());
    };
    // The accepted operation owns its permit, not the HTTP connection. Dropping
    // the request detaches this task rather than releasing a still-active Resume.
    match tokio::spawn(async move {
        let _permit = permit;
        resume_selected(&state, &selected).await
    }).await {
        Ok(response) => response,
        Err(_) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "resume-result-unavailable",
            "无法确认 Resume 执行结果；请检查 Herdr 后再重试。".into()),
    }
}

async fn resume_selected(state: &AppState, selected: &crate::history::HistorySelection) -> Response {
    let kind = match selected.agent_type.as_deref() {
        Some(kind @ ("pi" | "omp")) => kind,
        _ => return crate::rejected_owned(StatusCode::BAD_REQUEST, "unsupported-agent", "无法确定受支持的 agent 类型；尚未恢复。".into()),
    };
    let targets = match state.herdr.matching_targets(kind, &selected.session_id).await {
        Ok(targets) => targets,
        Err(error) => return failure("lookup", &error),
    };
    let Some(target) = targets.first() else {
        if kind == "pi" { return restore_pi(state, selected).await; }
        return Json(json!({"outcome":"not-found","matchCount":0,"message":"已查询 Herdr，未找到匹配目标；尚未恢复，也没有启动新进程。"})).into_response();
    };
    if let Err(error) = state.herdr.focus_target(target).await { return failure("focus", &error); }
    let message = if targets.len() > 1 {
        format!("找到 {} 个匹配目标，已按 Workspace、Tab、Pane ID 的稳定顺序自动选择并定位。仅确认 Herdr 内部导航，不表示 Windows 窗口已置前。", targets.len())
    } else { "已定位已有对话；仅确认 Herdr 内部导航，不表示 Windows 窗口已置前。".into() };
    Json(json!({"outcome":"located","target":target,"matchCount":targets.len(),"message":message})).into_response()
}

async fn restore_pi(state: &AppState, selected: &crate::history::HistorySelection) -> Response {
    use crate::{catalog::{ExtensionKind, ResolvedPreset}, herdr::{NavigationTarget, read_native_id}};
    let cwd = match selected.cwd.as_deref().filter(|cwd| std::path::Path::new(cwd).is_absolute() && std::path::Path::new(cwd).is_dir()) {
        Some(cwd) => cwd,
        None => return restore_failure("cwd", "invalid-cwd", "历史工作目录缺失、不是绝对路径或已不可用。", None),
    };
    let id = if std::path::Path::new(&selected.session_id).is_absolute() {
        match read_native_id(&selected.session_id, "pi").await {
            Ok(id) => id,
            Err(error) => return restore_failure("session", error.code(), &error.message(), None),
        }
    } else { selected.session_id.clone() };
    let mut load = id.clone();
    if let Some(file) = selected.session_file.as_deref() {
        match read_native_id(file, "pi").await {
            Ok(file_id) if file_id == id => load = file.to_owned(),
            Ok(_) => return restore_failure("session", "native-session-mismatch", "历史会话文件属于另一原生对话。", None),
            Err(_) => {} // Only the exact saved native ID may replace an unavailable file.
        }
    } else if std::path::Path::new(&selected.session_id).is_absolute() { load = selected.session_id.clone(); }
    let catalog = match state.bots.catalog() {
        Ok(catalog) => catalog,
        Err(error) => return restore_failure("settings", "invalid-agent-settings", &error.message(), None),
    };
    // Resume does not resolve any Bot. Keep only the current mandatory integrations.
    let preset = ResolvedPreset { provider_model: None, thinking: None, native_skill: None,
        extensions: catalog.extensions.into_iter().filter(|extension| extension.agent.label() == "pi" && extension.kind == ExtensionKind::Fixed).collect() };
    if let Err(error) = preset.check_entrypoints() { return restore_failure("settings", "invalid-agent-settings", &error, None); }
    let workspace = match state.herdr.workspace(state.herdr.workspace_name()).await {
        Ok(workspace) => workspace,
        Err(error) => return restore_failure("workspace", error.code(), &error.message(), None),
    };
    let tab = match state.herdr.create_tab(&workspace.id, cwd, selected.session_name.as_deref().unwrap_or("Resume Pi Chat")).await {
        Ok(tab) => tab,
        Err(error) => return restore_failure("tab", error.code(), &error.message(), None),
    };
    let target = NavigationTarget { workspace_id: workspace.id, tab_id: tab.tab_id, pane_id: tab.pane_id };
    let name = crate::chats::new_agent_name();
    let mut args = crate::chats::agent_launch_args(&preset);
    args.extend(["--session".into(), load]);
    if let Err(error) = state.herdr.start_agent(&name, "pi", &target.pane_id, &args).await {
        return restore_failure("start", error.code(), &error.message(), Some(&target));
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match state.herdr.restored_session(&name, "pi", deadline).await {
            Ok(native) if native.session_id == id && std::path::Path::new(&native.cwd) == std::path::Path::new(cwd) => break,
            Ok(_) => return restore_failure("confirm", "native-session-mismatch", "Pi 加载的原生对话或实际工作目录与历史不符。", Some(&target)),
            Err(error) if tokio::time::Instant::now() >= deadline => return restore_failure("confirm", "restore-unconfirmed", &error.message(), Some(&target)),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    if let Err(error) = state.herdr.focus_target(&target).await { return restore_failure("focus", error.code(), &error.message(), Some(&target)); }
    Json(json!({"outcome":"restored","stage":"focused","target":target,"matchCount":0,"fallback":true,
        "message":"缺少原 Workspace 信息，已回退到配置的默认 Workspace；已确认恢复同一 Pi 原生对话并完成 Herdr 内部导航，不表示 Windows 窗口已置前。未提交输入或重新应用 Bot 预设。"})).into_response()
}

fn restore_failure(stage: &str, code: &str, message: &str, target: Option<&crate::herdr::NavigationTarget>) -> Response {
    (StatusCode::BAD_GATEWAY, Json(json!({"outcome":"failed","stage":stage,"code":code,"target":target,
        "message":format!("Pi 恢复在 {stage} 阶段失败：{message} 尚未确认恢复成功；已创建的目标保留供检查，不自动另开或提交输入。") }))).into_response()
}

fn failure(stage: &str, error: &HerdrError) -> Response {
    let code = if stage == "lookup" && matches!(error, HerdrError::UnexpectedResponse { command: "agent get native identity", .. }) {
        "identity-verification-failed"
    } else { error.code() };
    (StatusCode::BAD_GATEWAY, Json(json!({"outcome":"failed","stage":stage,"code":code,
        "message":format!("{}失败：{} 尚未恢复；没有创建 Tab、启动 agent 或提交输入。", if stage == "focus" { "Herdr 聚焦" } else { "Herdr 查找／身份核实" }, error.message())}))).into_response()
}
