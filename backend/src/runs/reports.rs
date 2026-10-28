//! Reports are append-only facts. Delivery and diagnostics cannot undo a save.
use super::*;
use std::io::Write;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Report {
    pub id: String,
    pub r#type: String,
    pub at: u64,
    pub run_id: String,
    pub node_id: String,
    pub execution_id: String,
    pub source: ChatReference,
    pub target: ChatReference,
    pub body: String,
}

fn read_reports(path: &std::path::Path) -> Result<Vec<Report>, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        return Err("报告记录尾部残缺；拒绝读取或继续追加。".into());
    }
    bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).map_err(|e| e.to_string())).collect()
}

pub(super) async fn summary(state: &AppState, run: &Run, include_reports: bool) -> Result<serde_json::Value, String> {
    let paths = Runs::paths(state, &run.id);
    let _guards = state.runs.files.lock(&paths).await;
    let reports = read_reports(&paths[2])?;
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
}

// Resolve only system-persisted current associations, never request metadata.
async fn node_run(state: &AppState, session_id: &str) -> Result<(Run, String), Response> {
    let ids: Vec<_> = state.runs.active.lock().await.keys().cloned().collect();
    let mut matched = None;
    for id in ids {
        let run = match state.runs.read(state, &id).await {
            Ok(run) => run,
            Err(_) => continue,
        };
        if run.manager.as_ref().is_some_and(|chat| chat.session_id == session_id) {
            return Err(error(StatusCode::FORBIDDEN, "node-role-invalid", "管理者身份不能提交节点报告。"));
        }
        for (node_id, node) in &run.nodes {
            if node.execution.as_ref().and_then(|e| e.chat.as_ref()).is_some_and(|chat| chat.session_id == session_id) {
                if matched.is_some() {
                    return Err(error(StatusCode::FORBIDDEN, "node-ambiguous", "原生身份关联不唯一。"));
                }
                matched = Some((run.clone(), node_id.clone()));
            }
        }
    }
    matched.ok_or_else(|| error(StatusCode::FORBIDDEN, "node-unassociated", "当前原生身份没有唯一的当前节点执行归属。"))
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
    let (run, node_id) = match node_run(&state, &input.session_id).await {
        Ok(binding) => binding,
        Err(response) => return response,
    };
    // Once accepted, persist and attempt notification exactly once even if the
    // HTTP caller disconnects. No file/business lock spans host delivery.
    let work = tokio::spawn(async move {
        let report = {
            let lock = state.runs.operations.lock().await.entry(run.id.clone()).or_default().clone();
            let _operation = lock.lock().await;
            let run = state.runs.read(&state, &run.id).await?;
            let execution = run.nodes.get(&node_id).and_then(|n| n.execution.as_ref()).ok_or("当前执行关联已改变。")?;
            let source = execution.chat.as_ref().filter(|chat| chat.session_id == input.session_id).ok_or("当前执行关联已改变。")?;
            let target = run.manager.as_ref().ok_or("本 Run 缺少管理者真实 Chat 关联。")?;
            let report = Report {
                id: crate::chats::new_creation_id(), r#type: "report".into(),
                at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                run_id: run.id.clone(), node_id, execution_id: execution.id.clone(),
                source: source.clone(), target: target.clone(), body: input.body,
            };
            let paths = Runs::paths(&state, &run.id);
            let _guards = state.runs.files.lock(&paths).await;
            read_reports(&paths[2])?;
            let mut bytes = serde_json::to_vec(&report).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            let mut file = fs::OpenOptions::new().append(true).open(&paths[2]).map_err(|e| e.to_string())?;
            file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
            state.runs.changed(&run.id, true).await;
            report
        };
        let text = format!("Workflow report\nRun: {}\nnode: {}\nReport: {}\n\n{}", report.run_id, report.node_id, report.id, report.body);
        let delivery = state.herdr.prompt(&report.target.agent_name, &text).await;
        let (status, reason, diagnostic_reason) = match delivery {
            Ok(()) => ("submitted", "Herdr 已确认输入提交；不代表已阅读。".to_owned(), "input-submission-confirmed"),
            // Host error details may echo input; only the typed failure category
            // is safe for body-free diagnostics. The caller still gets detail.
            Err(error) => ("failed", error.message(), error.code()),
        };
        // Fallible stderr writes, not eprintln!: a broken diagnostic sink must
        // not panic, roll back the report or trigger another delivery attempt.
        let diagnostic = serde_json::json!({"event":"workflow-report-delivery","runId":report.run_id,"reportId":report.id,"targetChat":report.target,"result":status,"reason":diagnostic_reason});
        let diagnostic_status = if writeln!(std::io::stderr().lock(), "{diagnostic}").is_ok() { "written" } else { "failed" };
        Ok::<_, String>(serde_json::json!({"saved":true,"report":report,"notification":{"status":status,"reason":reason},"diagnostic":{"status":diagnostic_status},"guidance":"报告已保存。不要仅因通知失败重复 report；报告不等于完成，管理者仍须显式交接。"}))
    }).await;
    match work {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(message)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"code":"report-save-failed","message":message,"saved":false,"notification":{"status":"not-attempted"}}))).into_response(),
        Err(join) => error(StatusCode::INTERNAL_SERVER_ERROR, "report-internal", &join.to_string()),
    }
}
