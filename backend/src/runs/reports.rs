//! Reports are append-only facts. Delivery and diagnostics cannot undo a save.
use super::*;
use crate::report_delivery::{SavedReport, SubmitError, save_and_notify};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub id: String,
    pub r#type: String,
    pub at: u64,
    pub run_id: String,
    pub node_id: String,
    pub execution_id: String,
    pub from: Option<ChatReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ScriptSource>,
    pub to: ChatReference,
    pub body: String,
    // Absence is retained for historical reports, never interpreted as true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_complete: Option<bool>,
    #[serde(default, deserialize_with = "crate::scheduler::object_array")]
    pub outputs: Vec<crate::scheduler::ResolvedFile>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptSource {
    kind: ScriptKind,
    rust_script: String,
}

pub(super) async fn read(state: &AppState, run: &Run) -> Result<Vec<Report>, String> {
    let paths = Runs::paths(state, &run.id);
    let reports = state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
        access.read_jsonl::<Report>(&paths[2])
    })).await.map_err(|e| e.to_string())??;
    for report in &reports {
        if report.run_id != run.id { return Err("报告 Run 归属不一致。".into()); }
        let execution = run.nodes.get(&report.node_id).and_then(|node| node.executions().find(|execution| execution.id == report.execution_id)).ok_or("报告缺少所属 execution。")?;
        match &execution.target {
            ExecutionTarget::Script(script) if report.from.is_none() && report.source.as_ref().is_some_and(|source| source.rust_script == script.rust_script) => {},
            ExecutionTarget::Chat(_) if report.source.is_none() && report.from.is_some() && report.from.as_ref() == execution.chat() => {},
            _ => return Err("报告来源与所属 execution 类型、能力或真实 Chat 不一致。".into()),
        }
    }
    Ok(reports)
}

pub(super) async fn summary(state: &AppState, run: &Run, include_reports: bool) -> Result<serde_json::Value, String> {
    let reports = read(state, run).await?;
    let mut pending = BTreeMap::new();
    for report in &reports {
        if run.nodes.get(&report.node_id).is_some_and(|node| {
            (node.status == "running" || (node.status == "failed" && node.failure.as_ref().is_some_and(|f| f.stage == "execution")))
                && node.execution.as_ref().is_some_and(|execution| execution.id == report.execution_id)
        }) {
            pending.insert(&report.node_id, &report.id);
        }
    }
    let mut value = serde_json::json!({"pendingReports":pending});
    if include_reports { value["reports"] = serde_json::to_value(&reports).map_err(|e| e.to_string())?; }
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReportRequest {
    session_id: String,
    body: String,
    can_complete: bool,
    #[serde(default, deserialize_with = "crate::scheduler::object_array")]
    outputs: Vec<crate::scheduler::ResolvedFile>,
}

// Retained execution identities may append history, never control current work.
async fn node_run(state: &AppState, session_id: &str) -> Result<(Run, String, String), Response> {
    let ids: Vec<_> = state.runs.active.lock().await.keys().cloned().collect();
    let mut matched = None;
    let mut managers = Vec::new();
    let mut read_error = None;
    for id in ids {
        let run = match state.runs.read(state, &id).await {
            Ok(run) => run,
            Err(message) => {
                read_error.get_or_insert(message);
                continue;
            }
        };
        if run.manager.as_ref().is_some_and(|chat| chat.session_id == session_id) {
            managers.push(run.clone());
        }
        for (node_id, node) in &run.nodes {
            for execution in node.executions() {
                if execution.chat().is_some_and(|chat| chat.session_id == session_id) {
                    if matched.is_some() {
                        return Err(error(StatusCode::FORBIDDEN, "node-ambiguous", "原生身份关联不唯一。"));
                    }
                    matched = Some((run.clone(), node_id.clone(), execution.id.clone()));
                }
            }
        }
    }
    let child_execution = matched.as_ref().is_some_and(|(parent, node_id, execution_id)|
        parent.nodes[node_id].executions().any(|execution| execution.id == *execution_id && execution.child_run_id().is_some()));
    if child_execution && managers.is_empty() {
        return Err(error(StatusCode::FORBIDDEN, "node-role-invalid", "子 Run 缺少对应管理者真实 Chat 关联。"));
    }
    if !managers.is_empty() {
        let authorized = managers.len() == 1 && matched.as_ref().is_some_and(|(parent, node_id, execution_id)| {
            parent.nodes[node_id].executions().find(|execution| execution.id == *execution_id)
                .is_some_and(|execution| nested_pair(&managers[0], parent, node_id, execution))
        });
        let source_valid = if let Some((parent, node_id, execution_id)) = &matched {
            if let Some(execution) = parent.nodes[node_id].executions().find(|execution| execution.id == *execution_id) {
                nested_source(state, &managers[0], execution).await
            } else { false }
        } else { false };
        if !authorized || !source_valid { return Err(error(StatusCode::FORBIDDEN, "node-role-invalid", "管理者身份没有唯一合法的父执行关联。")); }
    }
    matched.ok_or_else(|| match read_error {
        Some(message) => error(StatusCode::INTERNAL_SERVER_ERROR, "run-read-failed", &message),
        None => error(StatusCode::FORBIDDEN, "node-unassociated", "当前原生身份没有唯一的当前节点执行归属。"),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextRequest {
    session_id: String,
}

pub async fn node_context(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: ContextRequest = match crate::chat_command_body::<crate::scheduler::JsonObject<ContextRequest>>(request, "节点上下文").await {
        Ok(input) => input.0,
        Err(response) => return response,
    };
    let (run, node_id, execution_id) = match node_run(&state, &input.session_id).await {
        Ok(binding) => binding,
        Err(response) => return response,
    };
    let Some(execution) = run.nodes[&node_id].executions().find(|execution| execution.id == execution_id) else {
        return error(StatusCode::FORBIDDEN, "node-unassociated", "当前原生身份没有唯一的节点执行归属。");
    };
    Json(serde_json::json!({"runId":run.id,"nodeId":node_id,"executionId":execution_id,"reportPolicy":execution.report_policy})).into_response()
}

pub async fn node_report(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    if !crate::sessions::is_local_source(peer.ip()) {
        return error(StatusCode::FORBIDDEN, "forbidden", "运行工具只接受同机来源。");
    }
    let input: ReportRequest = match crate::chat_command_body(request, "节点报告").await {
        Ok(input) => input,
        Err(response) => return response,
    };
    let mut ids = std::collections::BTreeSet::new();
    for file in &input.outputs {
        if file.id.trim().is_empty() || !ids.insert(&file.id) {
            return error(StatusCode::BAD_REQUEST, "invalid-output-id", "输出 ID 必须非空且不得重复。");
        }
        if file.path.contains('\0') || !std::path::Path::new(&file.path).is_absolute() {
            return error(StatusCode::BAD_REQUEST, "invalid-output-path", &format!("输出 {} 必须使用本机绝对文件地址。", file.id));
        }
    }
    let (run, node_id, execution_id) = match node_run(&state, &input.session_id).await {
        Ok(binding) => binding,
        Err(response) => return response,
    };
    let execution = run.nodes[&node_id].executions().find(|execution| execution.id == execution_id).unwrap();
    let Some(output_rule) = execution.output_rule.as_ref() else {
        return error(StatusCode::BAD_REQUEST, "output-rule-unknown", "历史执行没有已知的冻结 outputRule，不能提交新报告。");
    };
    if input.outputs.iter().any(|file| !output_rule.iter().any(|rule| rule.id == file.id)) {
        return error(StatusCode::BAD_REQUEST, "undeclared-output", "输出 ID 未在当前执行的 output_rule 中声明。");
    }
    if input.can_complete {
        if let Some(child_id) = execution.child_run_id() {
            let child = match state.runs.read(&state, child_id).await {
                Ok(child) => child,
                Err(message) => return error(StatusCode::INTERNAL_SERVER_ERROR, "child-run-read-failed", &message),
            };
            if child.status != "completed" || child.finished.is_none() {
                return error(StatusCode::BAD_REQUEST, "child-run-not-finished", "子 Run 必须显式 finish 后才能提交 canComplete=true；进展报告请使用 false。");
            }
        }
    }
    let work = submit(state, run.id, node_id, execution_id, ReportSubmission {
        origin: ReportOrigin::Chat(input.session_id), body: input.body, can_complete: input.can_complete, outputs: input.outputs,
    }).await;
    match work {
        Ok(submitted) => Json(serde_json::json!({"saved":true,"report":submitted.report,"notification":submitted.notification,"diagnostic":submitted.diagnostic,"guidance":"报告已保存。不要仅因通知失败重复 report；报告不等于完成，管理者仍须显式交接。"})).into_response(),
        Err(SubmitError::Save(message)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"code":"report-save-failed","message":message,"saved":false,"notification":{"status":"not-attempted"}}))).into_response(),
        Err(SubmitError::Internal(join)) => error(StatusCode::INTERNAL_SERVER_ERROR, "report-internal", &join.to_string()),
    }
}

pub(super) enum ReportOrigin { Chat(String), Script(Lifecycle) }
pub(super) struct ReportSubmission {
    pub origin: ReportOrigin,
    pub body: String,
    pub can_complete: bool,
    pub outputs: Vec<crate::scheduler::ResolvedFile>,
}

// Both real origins use one report authority, output validation and delivery.
// No business or file coordination spans the external notification.
pub(super) async fn submit(state: AppState, run_id: String, node_id: String, execution_id: String, input: ReportSubmission) -> Result<crate::report_delivery::SubmittedReport<Report>, SubmitError> {
    save_and_notify(state.herdr.clone(), async move {
        let report = {
            let lock = state.runs.operations.lock().await.entry(run_id.clone()).or_default().clone();
            let _operation = lock.lock().await;
            let mut run = state.runs.read(&state, &run_id).await?;
            let node = run.nodes.get_mut(&node_id).ok_or("node 关联已改变。")?;
            let execution = node.execution.iter_mut().chain(node.previous_executions.iter_mut().filter_map(|fact| fact.execution.as_mut())).find(|execution| execution.id == execution_id).ok_or("执行关联已改变。")?;
            let rules = execution.output_rule.as_ref().ok_or("缺少冻结输出规则。")?;
            let mut ids = std::collections::BTreeSet::new();
            for file in &input.outputs {
                if file.id.trim().is_empty() || !ids.insert(&file.id) || !rules.iter().any(|rule| rule.id == file.id) {
                    return Err("输出 ID 未声明或在同份报告重复。".into());
                }
                if file.path.contains('\0') || !std::path::Path::new(&file.path).is_absolute() { return Err("输出必须为本机绝对路径。".into()); }
            }
            let (from, source, lifecycle_changed) = match input.origin {
                ReportOrigin::Chat(session) => (Some(execution.chat().filter(|chat| chat.session_id == session).ok_or("当前执行关联已改变。")?.clone()), None, false),
                ReportOrigin::Script(lifecycle) => {
                    let ExecutionTarget::Script(script) = &mut execution.target else { return Err("脚本报告不能归属 Chat 执行。".into()); };
                    script.lifecycle = lifecycle;
                    (None, Some(ScriptSource { kind: ScriptKind::RustScript, rust_script: script.rust_script.clone() }), true)
                }
            };
            if lifecycle_changed { save_run(&state, &run).await?; }
            let target = run.manager.as_ref().ok_or("本 Run 缺少管理者真实 Chat 关联。")?;
            let report = Report {
                id: crate::chats::new_creation_id(), r#type: "report".into(),
                at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                run_id: run.id.clone(), node_id, execution_id,
                from, source, to: target.clone(), body: input.body,
                can_complete: Some(input.can_complete), outputs: input.outputs,
            };
            let paths = Runs::paths(&state, &run.id);
            let owned = state.clone();
            state.runs.files.with_async(&paths.clone(), move |access| Box::pin(async move {
                owned.runs.ensure_retained(&run.id).await?;
                access.append_jsonl::<Report>(&paths[2], &report)?;
                owned.runs.changed(&run.id, true).await;
                Ok::<_, String>(report)
            })).await.map_err(|e| e.to_string())??
        };
        let text = format!("Workflow report\nRun: {}\nnode: {}\nReport: {}\n\n{}", report.run_id, report.node_id, report.id, report.body);
        let diagnostic = serde_json::json!({"event":"workflow-report-delivery","runId":report.run_id,"reportId":report.id,"targetChat":report.to});
        Ok(SavedReport { target: Ok(report.to.agent_name.clone()), report, text, diagnostic })
    }).await
}
