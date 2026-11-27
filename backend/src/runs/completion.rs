//! Explicit handoff requires a current ready report, never Chat activity or downstream launch.
use super::*;
use super::operation::{Action, Decision};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompleteRequest {
    pub(crate) session_id: String,
    pub(crate) node_id: String,
    pub(crate) reason: String,
}

pub(crate) async fn manager_complete(state: AppState, input: CompleteRequest) -> RunResult {
    operation::execute(state, &input.session_id, Action::Complete { node_id: input.node_id, reason: input.reason }).await
}

pub(crate) async fn manager_finish(state: AppState, input: StatusRequest) -> RunResult {
    operation::execute(state, &input.session_id, Action::Finish).await
}

pub(crate) async fn manager_pause(state: AppState, input: StatusRequest) -> RunResult {

    operation::execute(state, &input.session_id, Action::Pause).await
}

pub(crate) async fn manager_resume(state: AppState, input: StatusRequest) -> RunResult {

    operation::execute(state, &input.session_id, Action::Resume).await
}

pub(super) async fn complete(state: &AppState, run: &mut Run, node_id: String, reason: String) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    let Some(node) = run.nodes.get(&node_id) else {
        return Decision::reject("node-unknown", "node 不在本次 Run 中。");
    };
    if node.execution.as_ref().is_some_and(|execution| execution.end_node) {
        return Decision::reject("end-node-not-completable", "end Chat 不接受 complete；成功派发后请显式 finish Run。");
    }
    if node.status == "completed" {
        return Ok(Decision::Unchanged);
    }
    if node.status != "running" && !(node.status == "failed" && node.failure.as_ref().is_some_and(|failure| failure.stage == "execution")) {
        return Decision::reject("node-not-completable", "仅 running 或人工处理后可交接的执行阶段 failed 可完成；pending 和派发失败不可直接完成。");
    }
    if !node.execution.as_ref().is_some_and(|execution| execution.approved) {
        return Decision::reject("node-not-approved", "当前 execution 尚未批准；报告不代替成果批准。");
    }
    if reason.trim().is_empty() {
        return Decision::reject("completion-reason-required", "请说明可交接结果及限制；完成说明不可为空。");
    }
    // The caller holds the same Run operation permit as report append. Read
    // committed order, not timestamps, and never fall back to another execution.
    let reports = reports::read(state, run).await?;
    let latest = reports.iter().rev().find(|report| {
        report.node_id == node_id && node.execution.as_ref().is_some_and(|execution| execution.id == report.execution_id)
    });
    match latest {
        Some(report) if report.can_complete == Some(true) => {},
        Some(report) => return Decision::reject("report-not-completable", format!(
            "当前 execution 最新报告 {} 的 can_complete={}；仅严格 true 可首次完成。", report.id,
            if report.can_complete == Some(false) { "false" } else { "缺失" }
        )),
        None => return Decision::reject("report-not-completable", "当前 execution 没有报告；须先提交 can_complete=true 的报告，再显式 complete。"),
    }
    let execution = node.execution.as_ref().ok_or("缺少当前 execution。")?;
    let rules = execution.output_rule.as_ref().ok_or("缺少冻结输出规则。")?;
    for rule in rules {
        if !reports.iter().any(|report| report.node_id == node_id && report.execution_id == execution.id && report.outputs.iter().any(|file| file.id == rule.id)) {
            return Decision::reject("output-missing", format!("当前 execution 尚未登记约定交付 {}。", rule.id));
        }
    }
    let completion = Completion {
        id: crate::chats::new_creation_id(),
        at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
        reason: reason,
    };
    let node = run.nodes.get_mut(&node_id).unwrap();
    let before = std::mem::replace(&mut node.status, "completed".into());
    node.completion = Some(completion);
    let completion = run.nodes[&node_id].completion.as_ref().unwrap();
    let event = serde_json::json!({"id":completion.id,"at":completion.at,"actor":completion.actor,"action":"complete","nodeId":node_id,"before":before,"after":"completed","reason":completion.reason,"execution":run.nodes[&node_id].execution});
    Ok(Decision::Commit(event))
}

// Called only after a new completion fact was saved. The detached task holds
// no Run operation guard during host interaction; a failure is recorded after it.
pub(super) fn close_completed_tab(state: &AppState, run: &Run, node_id: &str) {
    let node = &run.nodes[node_id];
    let Some(execution) = node.execution.as_ref().filter(|execution| execution.auto_close) else { return; };
    let Some(chat) = execution.chat().filter(|chat| !chat.tab_id.trim().is_empty()) else {
        eprintln!("[workflow] run={} node={} execution={} auto-close=skipped reason=missing-tab-association", run.id, node_id, execution.id);
        return;
    };
    let state = state.clone();
    let run_id = run.id.clone();
    let node_id = node_id.to_owned();
    let execution_id = execution.id.clone();
    let completion_id = node.completion.as_ref().expect("new completion was saved").id.clone();
    let chat = chat.clone();
    tokio::spawn(async move {
        let tab_id = &chat.tab_id;
        eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} tab={tab_id} auto-close=attempt");
        match close_execution_tab(&state, &chat).await {
            Ok(()) => eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} tab={tab_id} auto-close=succeeded result=host-acknowledged"),
            Err((code, message)) => {
                eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} tab={tab_id} auto-close=failed code={code} error={message}");
                let failure = AutoCloseFailure { code: code.into(), message };
                if let Err(error) = record_close_failure(&state, &run_id, &node_id, &completion_id, failure).await {
                    eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} tab={tab_id} close-feedback-save-failed error={error}");
                }
            }
        }
    });
}

async fn record_close_failure(state: &AppState, run_id: &str, node_id: &str, completion_id: &str, failure: AutoCloseFailure) -> Result<(), String> {
    let lock = state.runs.operations.lock().await.get(run_id).cloned().ok_or("缺少 Run 操作关联。")?;
    let _operation = lock.lock().await;
    let mut run = state.runs.read(state, run_id).await?;
    let Some(node) = run.nodes.get_mut(node_id).filter(|node| {
        node.completion.as_ref().is_some_and(|completion| completion.id == completion_id)
    }) else { return Err("节点完成关联已变化；未附加关闭失败。".into()); };
    node.auto_close_failure = Some(failure);
    save_run(state, &run).await
}

async fn close_execution_tab(state: &AppState, chat: &ChatReference) -> Result<(), (&'static str, String)> {
    let result = if state.herdr.is_app() {
        let target = chat.app_target(state, &chat.location()).await.map_err(|error| match error {
            crate::chats::Error::Invalid(code, message) | crate::chats::Error::NotFound(code, message)
            | crate::chats::Error::Internal(code, message) => (code, message),
            other => ("run-chat-lookup-failed", format!("{other:?}")),
        })?;
        state.herdr.close_app_target(&target).await
    } else {
        state.herdr.close_tui_tab(&chat.workspace_id, &chat.tab_id).await
    };
    result.map_err(|error| (error.code(), error.message()))
}

pub(super) fn can_finish(run: &Run, map: &WorkflowMap) -> bool {
    matches!(run.status.as_str(), "running" | "paused") && map.nodes.iter().all(|(id, definition)| {
        run.nodes.get(id).is_some_and(|node| node.status == "completed" || (definition.0.end_node && handed_to_user(node)))
    })
}

fn handed_to_user(node: &NodeState) -> bool {
    node.status == "running" && node.failure.is_none() && node.execution.as_ref().is_some_and(|execution| execution.end_node && execution.chat().is_some())
}

pub(super) fn finish(run: &mut Run, map: &WorkflowMap) -> Result<Decision, String> {
    if run.status == "completed" {
        return Ok(Decision::Unchanged);
    }
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-not-finishable", "仅 running 或 paused 的 Run 可结束。");
    }
    let handed_to_user = map.nodes.iter().any(|(id, definition)| {
        definition.0.end_node && run.nodes.get(id).is_some_and(handed_to_user)
    });
    if !can_finish(run, map) {
        return Decision::reject("nodes-incomplete", "仍有 node 未 completed；请显式完成交接后再 finish。");
    }
    let finished = Finished {
        reason: handed_to_user.then(|| "handed-to-user".into()),
        id: crate::chats::new_creation_id(),
        at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        actor: run.manager.clone().ok_or("缺少管理者真实关联。")?,
    };
    let before = std::mem::replace(&mut run.status, "completed".into());
    run.finished = Some(finished);
    let finished = run.finished.as_ref().unwrap();
    let mut event = serde_json::json!({"id":finished.id,"at":finished.at,"actor":finished.actor,"action":"finish","before":before,"after":"completed"});
    if let Some(reason) = &finished.reason { event["reason"] = serde_json::json!(reason); }
    Ok(Decision::Commit(event))
}

pub(super) fn eligibility(run: &mut Run, action: &str, target: &str) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受状态变更。");
    }
    if run.status == target {
        return Ok(Decision::Unchanged);
    }
    let before = std::mem::replace(&mut run.status, target.into());
    let event = serde_json::json!({"id":crate::chats::new_creation_id(),"at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"actor":run.manager,"action":action,"before":before,"after":target});
    Ok(Decision::Commit(event))
}
