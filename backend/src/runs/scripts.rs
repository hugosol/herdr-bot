//! Accepted compiled work owns its lifetime, independently of the HTTP caller.
use super::*;

pub(super) fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }

pub(super) fn launch(state: AppState, run: Run, node_id: String) {
    let execution = run.nodes[&node_id].execution.as_ref().expect("saved script execution").clone();
    let Some(script) = execution.script() else { unreachable!("only script targets launch compiled work") };
    let started_at = match script.lifecycle { Lifecycle::Running { started_at } => started_at, _ => unreachable!("new script must be running") };
    let capability = script.rust_script.clone();
    tokio::spawn(async move {
        // File reads/writes may block (including real Windows sharing/oplocks).
        // Keep both the async executor and the Run operation permit available.
        let runtime = tokio::runtime::Handle::current();
        let owned = state.clone();
        let run_id = run.id.clone();
        let execution_id = execution.id.clone();
        let outcome = tokio::task::spawn_blocking(move || runtime.block_on(async {
            match execution.script().expect("accepted script").rust_script.as_str() {
                "create_blank_bot" => crate::scheduler::script_bots::create_blank(&owned, &run, &execution).await,
                "save_bot_guides" => crate::scheduler::script_bots::save_guides(&owned, &run, &execution).await,
                _ => unreachable!("validated fixed capability"),
            }
        })).await;
        let (failure, outputs) = match outcome {
            Ok(outcome) => (outcome.failure, outcome.outputs),
            Err(error) => (Some(serde_json::json!({"stage":"validate","code":"execution-failed","message":error.to_string(),"effects":{"write":"unknown","readBack":"unknown","materials":[]}})), Vec::new()),
        };
        let ended_at = now().max(started_at);
        let (lifecycle, body, can_complete) = match failure {
            Some(failure) => {
                let body = format!("Rust {capability} 失败；未自动完成节点，也未回滚实际效果。\n{failure}");
                (Lifecycle::Failed { started_at, ended_at, failure }, body, false)
            },
            None => {
                let body = if capability == "save_bot_guides" { "三指南已原样汇总一次合法保存、完整读回并交付 saved 与独立 handoff；未执行目标业务。尚待 manager 显式 complete。" } else { "空白 Bot 已合法创建、完整读回并交付 baseline 与独立 handoff；未执行目标业务。尚待 manager 显式 complete。" };
                (Lifecycle::Succeeded { started_at, ended_at }, body.into(), true)
            },
        };
        if let Err(error) = reports::submit(state, run_id.clone(), node_id.clone(), execution_id.clone(), reports::ReportSubmission {
            origin: reports::ReportOrigin::Script(lifecycle), body, can_complete, outputs,
        }).await {
            let message = match error { crate::report_delivery::SubmitError::Save(message) => message, crate::report_delivery::SubmitError::Internal(error) => error.to_string() };
            eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} script-report-save-failed={message}；不重投业务或伪造成功报告。");
        }
    });
}
