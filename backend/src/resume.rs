//! Resume first locates a native conversation. Only a successful empty query
//! permits a new process. Restoration loads the latest trusted Bot environment,
//! but never submits input or replays launch-only trigger configuration.
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
        return restore_history(&state, &selected, kind).await;
    };
    if let Err(error) = state.herdr.focus_target(target).await { return failure("focus", &error); }
    let message = "已切换到当前herdr焦点";
    Json(json!({"outcome":"located","target":target,"matchCount":targets.len(),"message":message})).into_response()
}

async fn restore_history(state: &AppState, selected: &crate::history::HistorySelection, kind: &str) -> Response {
    let mut feedback = degradation::Feedback::default();
    restore_with_feedback(state, selected, kind, &mut feedback).await
}

async fn restore_with_feedback(state: &AppState, selected: &crate::history::HistorySelection, kind: &str, feedback: &mut degradation::Feedback) -> Response {
    use crate::herdr::NavigationTarget;
    let label = if kind == "omp" { "OMP" } else { "Pi" };
    let cwd = match selected.cwd.as_deref().filter(|cwd| std::path::Path::new(cwd).is_absolute() && std::path::Path::new(cwd).is_dir()) {
        Some(cwd) => cwd,
        None => return restore_failure(feedback, label, "cwd", "invalid-cwd", "历史工作目录缺失、不是绝对路径或已不可用。", None),
    };
    let (id, load) = match feedback.native_load(selected, kind).await {
        Ok(identity) => identity,
        Err((code, message)) => return restore_failure(feedback, label, "session", code, &message, None),
    };
    let mut args = match feedback.environment_args(state, selected, kind).await {
        Ok(args) => args,
        Err((code, message)) => return restore_failure(feedback, label, "settings", code, &message, None),
    };

    let workspace = match feedback.workspace(&state.herdr, selected.original_workspace_id.as_deref()).await {
        Ok(workspace) => workspace,
        Err(error) => return restore_failure(feedback, label, "workspace", error.code(), &error.message(), None),
    };
    let tab = match state.herdr.create_tab(&workspace.id, cwd, selected.session_name.as_deref().unwrap_or(if kind == "omp" { "Resume OMP Chat" } else { "Resume Pi Chat" })).await {
        Ok(tab) => tab,
        Err(error @ HerdrError::Timeout(_)) => return startup_unconfirmed(feedback, label, "tab", &error.message(), json!({"workspaceId":workspace.id}), None),
        Err(error) => return restore_failure(feedback, label, "tab", error.code(), &error.message(), None),
    };
    let target = NavigationTarget { workspace_id: workspace.id, tab_id: tab.tab_id, pane_id: tab.pane_id };
    let name = crate::chats::new_agent_name();
    args.extend([if kind == "omp" { "--resume" } else { "--session" }.into(), load]);
    if let Err(error) = state.herdr.start_agent(&name, kind, &target.pane_id, &args).await {
        if matches!(error, HerdrError::Timeout(_)) {
            return startup_unconfirmed(feedback, label, "start", &error.message(), json!(target), Some(&name));
        }
        return restore_failure(feedback, label, "start", error.code(), &error.message(), Some(&target));
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match state.herdr.restored_session(&name, kind, deadline).await {
            Ok(native) if native.session_id == id && std::path::Path::new(&native.cwd) == std::path::Path::new(cwd) => break,
            Ok(_) => return restore_failure(feedback, label, "confirm", "native-session-mismatch", &format!("{label} 加载的原生对话或实际工作目录与历史不符。"), Some(&target)),
            Err(error) if tokio::time::Instant::now() >= deadline => return startup_unconfirmed(feedback, label, "confirm", &error.message(), json!(target), Some(&name)),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    if let Err(error) = state.herdr.focus_target(&target).await { return restore_failure(feedback, label, "focus", error.code(), &error.message(), Some(&target)); }
    feedback.response(StatusCode::OK, json!({"outcome":"restored","stage":"focused","target":target,"matchCount":0,
        "message":format!("已确认恢复同一 {label} 原生对话并完成 Herdr 内部导航，不表示 Windows 窗口已置前。未提交输入或重新应用 Bot 预设。")}))
}

// Recovery policy and feedback live together, independently of history enrichment.
// Recording a degradation means its replacement strategy has actually been selected.
mod degradation {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Item {
        element: &'static str,
        reason: String,
        strategy: String,
    }

    #[derive(Default)]
    pub(super) struct Feedback { items: Vec<Item> }

    impl Feedback {
        pub(super) fn record(&mut self, element: &'static str, reason: String, strategy: String) {
            self.items.push(Item { element, reason, strategy });
        }

        pub(super) async fn environment_args(&mut self, state: &AppState, selected: &crate::history::HistorySelection, kind: &str) -> Result<Vec<String>, (&'static str, String)> {
            use crate::bots::{Agent, BotError};
            let agent = if kind == "omp" { Agent::Omp } else { Agent::Pi };
            let bot = match selected.source_bot_id.as_deref() {
                Some(id) => match state.bots.get(id).await {
                    Ok(bot) => Some(bot),
                    Err(BotError::NotFound) => {
                        self.record("extensions", format!("来源 Bot 已删除（ID {id}，成功读取存储确认不存在）。"),
                            "采用 agent 原生默认扩展发现；不关闭自动发现，也不显式追加 Bot 固定／可选扩展。".into());
                        None
                    }
                    Err(error) => return Err((error.code(), error.message())),
                },
                None => {
                    self.record("extensions", "历史记录缺少来源 Bot 关联；不按名称、cwd 或当前 Bot 推测来源。".into(),
                        "采用 agent 原生默认扩展发现；不关闭自动发现，也不显式追加 Bot 固定／可选扩展。".into());
                    None
                }
            };
            let Some(bot) = bot else { return Ok(Vec::new()); };
            if bot.agent != agent {
                return Err(("bot-agent-mismatch", "来源 Bot 的 agent 类型与所选历史不一致；尚未恢复。".into()));
            }
            let catalog = state.bots.catalog().map_err(|error| ("invalid-agent-settings", error.message()))?;
            let env = self.environment(&catalog, agent, &bot.env.extensions);
            Ok(crate::chats::agent_launch_args(&env, None))
        }

        pub(super) fn environment(&mut self, catalog: &crate::catalog::Catalog, agent: crate::bots::Agent, selections: &[String]) -> crate::catalog::ResolvedEnvConfig {
            use crate::catalog::{ExtensionKind, ResolvedEnvConfig};
            for id in selections {
                if !catalog.extensions.iter().any(|extension| extension.id == *id && extension.agent == agent && extension.kind == ExtensionKind::Optional) {
                    self.record("extension", format!("扩展「{id}」的当前可选映射缺失。"), "跳过此扩展；保留其余可用项，仍关闭扩展自动发现。".into());
                }
            }
            ResolvedEnvConfig {
                extensions: catalog.extensions.iter().filter(|extension| extension.agent == agent && (extension.kind == ExtensionKind::Fixed || selections.contains(&extension.id))).filter_map(|extension| {
                    match extension.check_entrypoint() {
                        Ok(()) => Some(extension.clone()),
                        Err(reason) => {
                            self.record("extension", reason, "跳过此扩展；保留其余可用项，仍关闭扩展自动发现。".into());
                            None
                        }
                    }
                }).collect(),
            }
        }

        pub(super) async fn native_load(&mut self, selected: &crate::history::HistorySelection, kind: &str) -> Result<(String, String), (&'static str, String)> {
            let is_path = std::path::Path::new(&selected.session_id).is_absolute();
            let id = if is_path {
                crate::herdr::read_native_id(&selected.session_id, kind).await
                    .map_err(|error| (error.code(), error.message()))?
            } else { selected.session_id.clone() };
            let reason = if let Some(file) = selected.session_file.as_deref() {
                match crate::herdr::read_native_id(file, kind).await {
                    Ok(file_id) if file_id == id => return Ok((id, file.to_owned())),
                    Ok(_) => return Err(("native-session-mismatch", "历史会话文件属于另一原生对话。".into())),
                    Err(error) => format!("会话文件 {file} 不可用：{}", error.message()),
                }
            } else if is_path {
                return Ok((id, selected.session_id.clone()));
            } else { "历史记录未提供会话文件。".into() };
            self.record("session-file", reason, format!("仅使用精确原生 ID {id} 加载同一对话；不扫描、改写或修复原生文件。"));
            Ok((id.clone(), id))
        }

        pub(super) async fn workspace(&mut self, herdr: &crate::herdr::HerdrClient, original_id: Option<&str>) -> Result<crate::herdr::Workspace, HerdrError> {
            let reason = if let Some(id) = original_id {
                if let Some(workspace) = herdr.workspace_by_id(id).await? { return Ok(workspace); }
                format!("原 Workspace 已删除（ID {id}，成功查询确认）。")
            } else { "缺少原 Workspace 信息。".into() };
            let workspace = herdr.workspace(herdr.workspace_name()).await?;
            self.record("workspace", reason, format!("已回退到配置的唯一默认 Workspace {}（{}）；保留历史原位置。", workspace.label, workspace.id));
            Ok(workspace)
        }

        pub(super) fn response(&self, status: StatusCode, mut value: serde_json::Value) -> Response {
            value["fallback"] = json!(self.items.iter().any(|item| item.element == "workspace"));
            value["degradations"] = json!(self.items);
            if value["outcome"] == "restored" && !self.items.iter().any(|item| item.element == "workspace") {
                value["message"] = json!(format!("已在原 Workspace 恢复；{}", value["message"].as_str().unwrap_or_default()));
            }
            if !self.items.is_empty() {
                let mut message = String::from("恢复存在降级：");
                for item in &self.items {
                    message.push_str(&item.reason);
                    message.push(' ');
                    message.push_str(&item.strategy);
                    message.push(' ');
                }
                message.push_str(value["message"].as_str().unwrap_or_default());
                value["message"] = json!(message);
            }
            (status, Json(value)).into_response()
        }
    }
}

fn startup_unconfirmed(feedback: &degradation::Feedback, label: &str, stage: &str, detail: &str, target: serde_json::Value, agent_name: Option<&str>) -> Response {
    feedback.response(StatusCode::BAD_GATEWAY, json!({"outcome":"unconfirmed","stage":stage,"code":"restore-unconfirmed",
        "target":target,"agentName":agent_name,
        "progress":{"tabCreated":(stage != "tab").then_some(true),"startSubmitted":stage != "tab","conversationConfirmed":false,"focused":false},
        "message":format!("{label} 启动结果未确认（{stage} 阶段等待超时）：{detail} 创建 Tab 或提交启动命令不等于已确认加载正确对话并完成聚焦，不能据此判断未启动。已取得的目标保留供检查，不自动关闭、终止、再次创建、启动或提交输入。请检查 Herdr；用户再次 Resume 时会重新查找，不重放上次启动。")
    }))
}

fn restore_failure(feedback: &degradation::Feedback, label: &str, stage: &str, code: &str, message: &str, target: Option<&crate::herdr::NavigationTarget>) -> Response {
    feedback.response(StatusCode::BAD_GATEWAY, json!({"outcome":"failed","stage":stage,"code":code,"target":target,
        "message":format!("{label} 恢复在 {stage} 阶段失败：{message} 尚未确认恢复成功；已创建的目标保留供检查，不自动另开或提交输入。") }))
}

fn failure(stage: &str, error: &HerdrError) -> Response {
    let code = if stage == "lookup" && matches!(error, HerdrError::UnexpectedResponse { command: "agent get native identity", .. }) {
        "identity-verification-failed"
    } else { error.code() };
    (StatusCode::BAD_GATEWAY, Json(json!({"outcome":"failed","stage":stage,"code":code,
        "message":format!("{}失败：{} 尚未恢复；没有创建 Tab、启动 agent 或提交输入。", if stage == "focus" { "Herdr 聚焦" } else { "Herdr 查找／身份核实" }, error.message())}))).into_response()
}
