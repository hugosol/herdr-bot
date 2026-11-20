//! Resume first locates a native conversation. Only a successful empty query
//! permits a new process. Restoration loads the latest trusted Task environment,
//! but never submits input or replays launch-only trigger configuration.
use super::Error;
use serde_json::Value;
use serde::Deserialize;
use serde_json::json;
use crate::{AppState, herdr::HerdrError, tasks::{Task, TaskError}};
use std::{collections::HashMap, sync::{Arc, Mutex}};

type SourceTask = Option<Result<Task, TaskError>>;

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
pub(crate) struct ResumeRequest { pub(crate) session_id: String }

pub(crate) async fn post_resume(state: AppState, request: ResumeRequest) -> Result<Value, Error> {
    let selected = match state.history.selection(&request.session_id).await {
        Ok(Some(selected)) => selected,
        Ok(None) => return Err(Error::NotFound( "unknown-chat", "找不到所选历史 Chat。".into())),
        Err(error) => return Err(Error::Internal( "history-unreadable", format!("读取历史会话失败：{error}"))),
    };
    let Some(permit) = state.resume_executions.try_qualify(&selected.session_id) else {
        return Err(Error::Conflict( "resume-in-progress",
            "同一 Chat 已有 Resume 正在执行；本次请求未执行，也不会排队。请等待完成后由用户重新发起。".into()));
    };
    let source = match selected.source_task_id.as_deref() {
        Some(id) => Some(state.tasks.get(id).await),
        None => None,
    };
    let known_chief = state.chief.is_known(&selected.session_id).map_err(Error::Chief)?;
    let chief_claim = if known_chief || matches!(&source, Some(Ok(task)) if task.task_type == crate::tasks::TaskType::Chief) {
        Some(state.chief.reserve_resume(&selected.session_id).map_err(Error::Chief)?)
    } else { None };
    // The accepted operation owns its permit, not the HTTP connection. Dropping
    // the request detaches this task rather than releasing a still-active Resume.
    match tokio::spawn(async move {
        let _permit = permit;
        let response = resume_selected(&state, &selected, &source).await;
        if let Some(claim) = chief_claim {
            let confirmed = response.as_ref().is_ok_and(|value| matches!(value["outcome"].as_str(), Some("located" | "restored")));
            if let Err(error) = state.chief.resume_finished(&claim, confirmed) { return Err(Error::Chief(error)); }
        }
        response
    }).await {
        Ok(response) => response,
        Err(_) => Err(Error::Internal( "resume-result-unavailable",
            "无法确认 Resume 执行结果；请检查 Herdr 后再重试。".into())),
    }
}

async fn resume_selected(state: &AppState, selected: &crate::chats::history::HistorySelection, source: &SourceTask) -> Result<Value, Error> {
    let kind = match selected.agent_type.as_deref() {
        Some("pi") => "pi",
        Some("omp") => "omp",
        _ => return Err(Error::Invalid( "unsupported-agent", "无法确定受支持的 agent 类型；尚未恢复。".into())),
    };
    if state.herdr.is_app() {
        let id = match state.herdr.selected_native_id(kind, selected).await {
            Ok(id) => id,
            Err(error) => return failure("lookup", &error),
        };
        let mut targets = match state.launches.app_target(&state.herdr, kind, &id).await {
            Ok(targets) => targets,
            Err(error) => return failure("lookup", &error),
        };
        targets.sort_unstable_by(|a, b| a.location.cmp(&b.location));
        targets.dedup_by(|a, b| a.location == b.location);
        if let Some(target) = targets.first() {
            verify_live_history(state, selected, kind, &id).await?;
            if let Err(error) = state.herdr.focus_app_target(target).await { return failure("focus", &error); }
            return located(&target.location, targets.len());
        }
        return restore_history(state, selected, kind, source).await;
    }
    let targets = match state.herdr.matching_targets(kind, selected).await {
        Ok(targets) => targets,
        Err(error) => return failure("lookup", &error),
    };
    let Some(target) = targets.first() else {
        return restore_history(state, selected, kind, source).await;
    };
    if selected.session_file.is_some() {
        let id = match state.herdr.selected_native_id(kind, selected).await {
            Ok(id) => id,
            Err(error) => return failure("lookup", &error),
        };
        verify_live_history(state, selected, kind, &id).await?;
    }
    if let Err(error) = state.herdr.focus_target(target).await { return failure("focus", &error); }
    located(target, targets.len())
}

async fn verify_live_history(state: &AppState, selected: &crate::chats::history::HistorySelection, kind: &str, id: &str) -> Result<(), Error> {
    if let Some(file) = selected.session_file.as_deref()
        && let Err((code, message)) = verify_saved_file(file, kind, id).await
    {
        return restore_failure(&degradation::Feedback::for_host(state.herdr.workspace_kind()), agent_label(kind), "session", code, &message, None).map(drop);
    }
    Ok(())
}

async fn verify_saved_file(file: &str, kind: &str, id: &str) -> Result<bool, (&'static str, String)> {
    match crate::herdr::read_existing_native_id(file, kind).await {
        Ok(Some(file_id)) if file_id == id => Ok(true),
        Ok(Some(_)) => Err(("native-session-mismatch", "历史会话文件属于另一原生对话。".into())),
        Ok(None) => Ok(false),
        Err(error) => Err((error.code(), format!("读取历史会话文件 {file} 失败：{}", error.message()))),
    }
}

fn agent_label(kind: &str) -> &'static str {
    if kind == "omp" { "OMP" } else { "Pi" }
}

fn located(target: &crate::herdr::NavigationTarget, match_count: usize) -> Result<Value, Error> {
    Ok(json!({"outcome":"located","target":target,"matchCount":match_count,"message":"已切换到当前herdr焦点"}))
}

async fn restore_history(state: &AppState, selected: &crate::chats::history::HistorySelection, kind: &'static str, source: &SourceTask) -> Result<Value, Error> {
    let mut feedback = degradation::Feedback::for_host(state.herdr.workspace_kind());
    restore_with_feedback(state, selected, kind, source, &mut feedback).await
}

async fn restore_with_feedback(state: &AppState, selected: &crate::chats::history::HistorySelection, kind: &'static str, source: &SourceTask, feedback: &mut degradation::Feedback) -> Result<Value, Error> {
    use crate::herdr::NavigationTarget;
    let label = agent_label(kind);
    let scenario = crate::herdr::placement::Scenario::Resume {
        historical_cwd: selected.cwd.as_deref(),
        original_workspace_id: selected.original_workspace_id.as_deref(),
        git_timeout: state.git_timeout,
    };
    let mut placement = match scenario.directory(&state.herdr).await {
        Ok(placement) => placement,
        Err(error) => return restore_failure(feedback, label, "cwd", error.code(), &error.message(), None),
    };
    if let Some(item) = placement.degradation.take() {
        feedback.record(item.element, item.reason, item.strategy);
    }
    let cwd = placement.directory.as_deref();
    let (id, load) = match feedback.native_load(selected, kind).await {
        Ok(identity) => identity,
        Err((code, message)) => return restore_failure(feedback, label, "session", code, &message, None),
    };
    let environment = match feedback.environment_args(state, selected, kind, source).await {
        Ok(environment) => environment,
        Err((code, message)) => return restore_failure(feedback, label, "settings", code, &message, None),
    };

    let mapped = match placement.workspace(&state.herdr).await {
        Ok(mapped) => mapped,
        Err(error) => return restore_failure(feedback, label, "workspace", error.code(), &error.message(), None),
    };
    if let Some(item) = mapped.degradation {
        feedback.record(item.element, item.reason, item.strategy);
    }
    let workspace = mapped.workspace;
    let name = selected.session_name.as_deref().unwrap_or(if kind == "omp" { "Resume OMP Chat" } else { "Resume Pi Chat" });
    let configured = environment.is_some();
    let mut args = environment.unwrap_or_default();
    args.extend([if kind == "omp" { "--resume" } else { "--session" }.into(), load]);
    let expected = ExpectedNative {
        id: &id,
        file: args.last().filter(|load| std::path::Path::new(load).is_absolute()).map(String::as_str),
        cwd,
    };
    if let Err(error) = state.chief.restoring_effect(&selected.session_id,false) { return Err(Error::Chief(error)); }
    if state.herdr.is_app() {
        return restore_app(state, selected, kind, label, &workspace.id, name, expected, &args, configured, feedback).await;
    }
    let tab = match state.herdr.create_tab(&workspace.id, cwd, name).await {
        Ok(tab) => tab,
        Err(error @ HerdrError::Timeout(_)) => return startup_unconfirmed(feedback, label, "tab", &error.message(), json!({"workspaceId":workspace.id}), None),
        Err(error) => {
            if matches!(error,HerdrError::CommandFailed {..} | HerdrError::NotStarted(_)) {
                if let Err(error) = state.chief.restoring_effect(&selected.session_id,true) { return Err(Error::Chief(error)); }
            }
            return restore_failure(feedback, label, "tab", error.code(), &error.message(), None);
        }
    };
    let target = NavigationTarget { workspace_id: workspace.id, tab_id: tab.tab_id, pane_id: tab.pane_id };
    let name = crate::chats::new_agent_name();
    if let Err(error) = state.herdr.start_agent(&name, kind, &target.pane_id, &args).await {
        if matches!(error, HerdrError::Timeout(_)) {
            return startup_unconfirmed(feedback, label, "start", &error.message(), json!(target), Some(&name));
        }
        if matches!(error,HerdrError::CommandFailed {..} | HerdrError::NotStarted(_)) {
            if let Err(error) = state.chief.restoring_effect(&selected.session_id,true) { return Err(Error::Chief(error)); }
        }
        return restore_failure(feedback, label, "start", error.code(), &error.message(), Some(&target));
    }
    use crate::herdr::RestoreEvidence;
    use tokio::time::{Instant, sleep_until, timeout_at};
    use std::time::Duration;
    let deadline = Instant::now() + Duration::from_secs(10);
    let absent = "未取得可靠的原生会话身份。";
    let mut last_cwd = None;
    let (actual_cwd, confirmed) = loop {
        let observation = timeout_at(deadline, state.herdr.restored_session(&name, kind, &target, deadline)).await;
        match observation {
            Ok(Ok(RestoreEvidence::Identified(native))) => {
                if !expected.matches(&native) {
                    return restore_failure(feedback, label, "confirm", "native-session-mismatch", &format!("{label} 加载的原生对话、会话文件或实际工作目录与历史不符。"), Some(&target));
                }
                break (native.cwd, true);
            }
            Ok(Ok(RestoreEvidence::Unidentified(actual_cwd))) => last_cwd = actual_cwd,
            Ok(Err(error)) => return restore_failure(feedback, label, "confirm", error.code(), &error.message(), Some(&target)),
            Err(_) => {},
        }
        let next = Instant::now() + Duration::from_millis(100);
        if next >= deadline {
            if !configured && let Some(actual_cwd) = last_cwd {
                if !recovery_cwd_matches(&actual_cwd, cwd) {
                    return restore_failure(feedback, label, "confirm", "resume-cwd-mismatch", "Herdr 未确认有效的实际工作目录。", Some(&target));
                }
                break (actual_cwd, false);
            }
            return startup_unconfirmed(feedback, label, "confirm", absent, json!(target), Some(&name));
        }
        sleep_until(next).await;
    };
    if let Err(error) = state.herdr.focus_target(&target).await { return restore_failure(feedback, label, "focus", error.code(), &error.message(), Some(&target)); }
    if confirmed { restored(feedback, label, &target, &actual_cwd) }
    else { focused_unconfirmed(feedback, label, &target, &actual_cwd, Some(&name)) }
}

async fn restore_app(
    state: &AppState, selected: &crate::chats::history::HistorySelection, kind: &'static str, label: &str,
    workspace: &str, name: &str, expected: ExpectedNative<'_>, args: &[String], configured: bool,
    feedback: &mut degradation::Feedback,
) -> Result<Value, Error> {
    let cwd = expected.cwd;
    let launch = configured.then(|| state.launches.begin_app_restore(kind, cwd));
    let result = match &launch {
        Some(launch) => state.herdr.launch_tab(workspace, cwd, name, &crate::chats::new_agent_name(), kind, args, launch).await,
        None => state.herdr.launch_unreported_app(workspace, cwd, name, kind, args).await,
    };
    let target = match result {
        Ok(target) => target,
        Err(error @ HerdrError::Timeout(_)) => return startup_unconfirmed(feedback, label, "tab", &error.message(), json!({"workspaceId":workspace}), None),
        Err(error) => {
            if matches!(error, HerdrError::CommandFailed { .. } | HerdrError::NotStarted(_)) {
                state.chief.restoring_effect(&selected.session_id, true).map_err(Error::Chief)?;
            }
            return restore_failure(feedback, label, "tab", error.code(), &error.message(), None);
        }
    };
    let location = &target.location;
    let native = if let Some(launch) = &launch {
        match launch.identity(&state.herdr).await {
            Ok(native) if expected.matches(&native) => {
                launch.retain_app_target();
                Some(native)
            }
            Ok(_) => return restore_failure(feedback, label, "confirm", "native-session-mismatch", &format!("{label} 加载的原生对话、会话文件或实际工作目录与历史不符。"), Some(location)),
            Err(error @ HerdrError::Timeout(_)) => return startup_unconfirmed(feedback, label, "confirm", &error.message(), json!(location), None),
            Err(error) => return restore_failure(feedback, label, "confirm", error.code(), &error.message(), Some(location)),
        }
    } else { None };
    let core = match state.herdr.focus_app_target(&target).await {
        Ok(core) => core,
        Err(error) => return restore_failure(feedback, label, "focus", error.code(), &error.message(), Some(location)),
    };
    if let Some(native) = native {
        return restored(feedback, label, location, &native.cwd);
    }
    let actual_cwd = core.pointer("/foreground/cwd").and_then(Value::as_str)
        .or_else(|| core.get("cwd").and_then(Value::as_str));
    let Some(actual_cwd) = actual_cwd.filter(|actual| recovery_cwd_matches(actual, cwd)) else {
        return restore_failure(feedback, label, "confirm", "resume-cwd-mismatch", "App/core 未确认有效的实际工作目录。", Some(location));
    };
    focused_unconfirmed(feedback, label, location, actual_cwd, None)
}

fn focused_unconfirmed(feedback: &degradation::Feedback, label: &str, target: &crate::herdr::NavigationTarget, actual_cwd: &str, agent_name: Option<&str>) -> Result<Value, Error> {
    feedback.response(true, json!({"outcome":"unconfirmed","stage":"focused","code":"restore-unconfirmed",
        "target":target,"matchCount":0,"actualCwd":actual_cwd,"agentName":agent_name,
        "progress":{"tabCreated":true,"startSubmitted":true,"conversationConfirmed":false,"focused":true},
        "message":format!("已请求恢复并定位 {label} 终端，原生身份未确认。实际工作目录：{actual_cwd}。不表示 Windows 窗口已置前。") }))
}

struct ExpectedNative<'a> {
    id: &'a str,
    file: Option<&'a str>,
    cwd: Option<&'a str>,
}

impl ExpectedNative<'_> {
    fn matches(&self, native: &crate::herdr::NativeSession) -> bool {
        native.session_id == self.id && recovery_cwd_matches(&native.cwd, self.cwd)
            && !matches!((self.file, native.session_file.as_deref()), (Some(expected), Some(actual)) if !recovery_path_matches(std::path::Path::new(actual), expected))
    }
}

fn recovery_path_matches(actual: &std::path::Path, expected: &str) -> bool {
    use crate::zones::{display_path, same_path};
    same_path(&display_path(actual), &display_path(std::path::Path::new(expected)))
}

fn recovery_cwd_matches(actual: &str, expected: Option<&str>) -> bool {
    let actual = std::path::Path::new(actual);
    actual.is_absolute() && actual.is_dir() && expected.is_none_or(|cwd| recovery_path_matches(actual, cwd))
}

fn restored(feedback: &degradation::Feedback, label: &str, target: &crate::herdr::NavigationTarget, actual_cwd: &str) -> Result<Value, Error> {
    feedback.response(true, json!({"outcome":"restored","stage":"focused","target":target,"matchCount":0,"actualCwd":actual_cwd,
        "message":format!("已确认恢复同一 {label} 原生对话并完成 Herdr 内部导航，实际工作目录：{actual_cwd}。不表示 Windows 窗口已置前。未重放发起配置或提交输入。") }))
}

// Recovery policy and feedback live together, independently of history enrichment.
// Recording a degradation means its replacement strategy has actually been selected.
mod degradation {
    use super::*;
    use serde::Serialize;
    use crate::tasks::{Agent, catalog::ExecutionRole};

    fn role_extension_ids(agent: Agent, role: ExecutionRole) -> &'static [&'static str] {
        use ExecutionRole::{Chief, ChiefDelegated, Manager, NestedManager, Node, Standalone};
        match (agent, role) {
            (_, Standalone | Chief) => &[],
            (Agent::Omp, Manager) => &["omp-manager-chat"],
            (Agent::Omp, Node) => &["omp-node-chat"],
            (Agent::Omp, NestedManager) => &["omp-manager-chat", "omp-node-chat"],
            (Agent::Omp, ChiefDelegated) => &["omp-chief-report"],
            (Agent::Pi, Manager) => &["pi-manager-chat"],
            (Agent::Pi, Node) => &["pi-node-chat"],
            (Agent::Pi, NestedManager) => &["pi-manager-chat", "pi-node-chat"],
            (Agent::Pi, ChiefDelegated) => &["pi-chief-report"],
        }
    }

    #[derive(Serialize)]
    struct Item {
        element: &'static str,
        reason: String,
        strategy: String,
    }

    #[derive(Default)]
    pub(super) struct Feedback { items: Vec<Item>, workspace_kind: &'static str }

    impl Feedback {
        pub(super) fn for_host(workspace_kind: &'static str) -> Self { Self { items: Vec::new(), workspace_kind } }

        pub(super) fn record(&mut self, element: &'static str, reason: String, strategy: String) {
            self.items.push(Item { element, reason, strategy });
        }

        fn default_discovery(&mut self, reason: String, agent: Agent, associated_role: Option<ExecutionRole>) -> Option<Vec<String>> {
            self.record("extensions", reason,
                "采用 agent 原生默认扩展发现；不关闭自动发现，不显式追加 Task 固定扩展或 Suite 资源，也不强制接入 connect-bot。默认资源范围可能变化，不代表恢复了原 Suite。".into());
            for id in associated_role.into_iter().flat_map(|role| role_extension_ids(agent, role)) {
                self.record("extension", format!("当前恢复未显式装配角色扩展「{id}」。"),
                    "不改变原执行归属；不保证原生自动发现包含该角色扩展。".into());
            }
            None
        }

        pub(super) async fn environment_args(&mut self, state: &AppState, selected: &crate::chats::history::HistorySelection, kind: &str, source: &SourceTask) -> Result<Option<Vec<String>>, (&'static str, String)> {
            let agent = if kind == "omp" { Agent::Omp } else { Agent::Pi };
            if matches!(source, Some(Ok(task)) if task.agent != agent) {
                return Err(("task-agent-mismatch", "来源 Task 的 agent 类型与所选历史不一致；尚未恢复。".into()));
            }
            // Execution ownership comes from saved Chat identity even when the
            // current environment cannot be read or assembled.
            let source_id = selected.source_task_id.as_deref();
            let associated_role = match state.runs.chat_role(state, &selected.session_id, source_id, agent).await
                .map_err(|message| ("execution-association-invalid", message))? {
                Some(role) => Some(role),
                None => state.delegations.sender(&selected.session_id)
                    .map_err(|error| ("chief-delegation-storage", error.to_string()))?
                    .filter(|record| record.run_id.is_none())
                    .map(|_| ExecutionRole::ChiefDelegated),
            };
            let task = match (source_id, source) {
                (_, Some(Ok(task))) => task,
                (Some(id), Some(Err(TaskError::NotFound))) => return Ok(self.default_discovery(
                    format!("来源 Task 已删除（ID {id}，成功读取存储确认不存在）。"), agent, associated_role)),
                (Some(id), Some(Err(error))) => return Ok(self.default_discovery(
                    format!("无法读取来源 Task（ID {id}）的会话环境（env config）：{}", error.message()), agent, associated_role)),
                _ => return Ok(self.default_discovery(
                    "历史记录缺少来源 Task 关联；不按名称、cwd 或当前 Task 推测来源。".into(), agent, associated_role)),
            };
            let catalog = match state.tasks.catalog() {
                Ok(catalog) => catalog,
                Err(error) => return Ok(self.default_discovery(
                    format!("无法读取来源 Task（ID {}）的会话环境（env config）：{}", task.id, error.message()), agent, associated_role)),
            };
            let suite_args = match state.tasks.prepare_environment(task) {
                Ok(args) => args,
                Err(error) => return Ok(self.default_discovery(
                    format!("来源 Task（ID {}）的 Suite 配置不可用：{}", task.id, error.message()), agent, associated_role)),
            };
            let role = associated_role.unwrap_or_else(|| task.task_type.into());
            let env = self.environment(&catalog, agent, role, suite_args, &state.herdr);
            Ok(Some(crate::chats::agent_launch_args(&env, None)))
        }

        fn environment(&mut self, catalog: &crate::tasks::catalog::Catalog, agent: Agent, role: ExecutionRole, suite_args: Vec<String>, herdr: &crate::herdr::HerdrClient) -> crate::tasks::catalog::ResolvedEnvConfig {
            use crate::tasks::catalog::{ExtensionKind, ResolvedEnvConfig};
            for &id in role_extension_ids(agent, role) {
                if !catalog.extensions.iter().any(|extension| extension.id == id) {
                    self.record("extension", format!("角色扩展「{id}」的当前固定映射缺失。"), "跳过此扩展；保留其余可用项，仍关闭扩展自动发现。".into());
                }
            }
            let mut env = ResolvedEnvConfig {
                suite_args,
                extensions: catalog.extensions.iter()
                    .filter(|extension| extension.applies_to(agent, role) && extension.kind == ExtensionKind::Fixed)
                    .cloned().collect(),
            };
            crate::herdr::native_status::select_host(herdr, &mut env);
            env.extensions.retain(|extension| match extension.check_entrypoint() {
                Ok(()) => true,
                Err(reason) => {
                    self.record("extension", reason, "跳过此扩展；保留其余可用项，仍关闭扩展自动发现。".into());
                    false
                }
            });
            env
        }

        pub(super) async fn native_load(&mut self, selected: &crate::chats::history::HistorySelection, kind: &str) -> Result<(String, String), (&'static str, String)> {
            let is_path = std::path::Path::new(&selected.session_id).is_absolute();
            let id = if is_path {
                crate::herdr::read_native_id(&selected.session_id, kind).await
                    .map_err(|error| (error.code(), error.message()))?
            } else { selected.session_id.clone() };
            let reason = if let Some(file) = selected.session_file.as_deref() {
                if verify_saved_file(file, kind, &id).await? { return Ok((id, file.to_owned())); }
                format!("会话文件 {file} 不可用：找不到文件。")
            } else if is_path {
                return Ok((id, selected.session_id.clone()));
            } else { "历史记录未提供会话文件。".into() };
            self.record("session-file", reason, format!("仅使用精确原生 ID {id} 加载同一对话；不扫描、改写或修复原生文件。"));
            Ok((id.clone(), id))
        }

        pub(super) fn response(&self, success: bool, mut value: serde_json::Value) -> Result<Value, Error> {
            value["fallback"] = json!(self.items.iter().any(|item| matches!(item.element, "workspace" | "cwd")));
            value["degradations"] = json!(self.items);
            if value["outcome"] == "restored" && !self.items.iter().any(|item| item.element == "workspace") {
                value["message"] = json!(format!("已在原 {} 恢复；{}", self.workspace_kind, value["message"].as_str().unwrap_or_default()));
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
            if success { Ok(value) } else { Err(Error::ResumeFailure(value)) }
        }
    }
}

fn startup_unconfirmed(feedback: &degradation::Feedback, label: &str, stage: &str, detail: &str, target: serde_json::Value, agent_name: Option<&str>) -> Result<Value, Error> {
    feedback.response(false, json!({"outcome":"unconfirmed","stage":stage,"code":"restore-unconfirmed",
        "target":target,"agentName":agent_name,
        "progress":{"tabCreated":(stage != "tab").then_some(true),"startSubmitted":stage != "tab","conversationConfirmed":false,"focused":false},
        "message":format!("{label} 启动结果未确认（{stage} 阶段等待超时）：{detail} 创建 Tab 或提交启动命令不等于已确认加载正确对话并完成聚焦，不能据此判断未启动。已取得的目标保留供检查，不自动关闭、终止、再次创建、启动或提交输入。请检查 Herdr；用户再次 Resume 时会重新查找，不重放上次启动。")
    }))
}

fn restore_failure(feedback: &degradation::Feedback, label: &str, stage: &str, code: &str, message: &str, target: Option<&crate::herdr::NavigationTarget>) -> Result<Value, Error> {
    feedback.response(false, json!({"outcome":"failed","stage":stage,"code":code,"target":target,
        "message":format!("{label} 恢复在 {stage} 阶段失败：{message} 尚未确认恢复成功；已创建的目标保留供检查，不自动另开或提交输入。") }))
}

fn failure(stage: &str, error: &HerdrError) -> Result<Value, Error> {
    let code = if stage == "lookup" && matches!(error, HerdrError::UnexpectedResponse { command: "agent get native identity", .. }) {
        "identity-verification-failed"
    } else { error.code() };
    Err(Error::ResumeFailure(json!({"outcome":"failed","stage":stage,"code":code,
        "message":format!("{}失败：{} 尚未恢复；没有创建 Tab、启动 agent 或提交输入。", if stage == "focus" { "Herdr 聚焦" } else { "Herdr 查找／身份核实" }, error.message())})))
}
