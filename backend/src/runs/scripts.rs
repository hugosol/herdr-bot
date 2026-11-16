//! Accepted script work owns its lifetime, independently of the HTTP caller.
use super::*;
use serde_json::{Value, json};
use std::{io::{Read, Write}, path::{Path, Component}, process::{Command, Stdio}};

pub(super) fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }

pub(crate) struct PreparedScript { path: PathBuf, source: String }

pub(crate) fn prepare_script(state: &AppState, script: &str) -> Result<PreparedScript, String> {
    let relative = Path::new(script);
    if relative.extension().and_then(|value| value.to_str()) != Some("ts") || !relative.components().all(|part| matches!(part, Component::Normal(_))) {
        return Err("script 必须是 scripts 目录内的相对 .ts 路径，不得包含父目录。".into());
    }
    let root = state.bot_directory.parent().ok_or("Bot 目录没有父目录")?.join("scripts").canonicalize().map_err(|error| format!("无法读取 scripts 目录：{error}"))?;
    let path = root.join(relative).canonicalize().map_err(|error| format!("无法读取脚本 {script}：{error}"))?;
    if !path.starts_with(&root) { return Err("脚本路径逃离 scripts 目录。".into()); }
    if !path.metadata().map_err(|error| error.to_string())?.is_file() { return Err("脚本入口必须是普通文件。".into()); }
    let bytes = crate::files::FileStore::default().read(&path).map_err(|error| format!("无法读取脚本 {script}：{error}"))?;
    let source = String::from_utf8(bytes).map_err(|error| format!("脚本 {script} 不是有效 UTF-8：{error}"))?;
    // Rust canonicalization produces verbatim Windows paths; Bun's module loader
    // resolves ordinary drive/UNC paths. Keep both sides of onLoad identical.
    #[cfg(windows)]
    let path = {
        let text = path.to_str().ok_or("脚本路径不是有效 Unicode")?;
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") { PathBuf::from(format!(r"\\{unc}")) }
        else { PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text)) }
    };
    Ok(PreparedScript { path, source })
}

pub(crate) fn validate_binding(state: &AppState, script: &str) -> Result<(), String> {
    prepare_script(state, script).map(|_| ())
}

const PREFIX: &[u8] = b"\x1eherdr-script:";
const LOG_LIMIT: usize = 16 * 1024;

fn diagnostic(code: &str, message: impl ToString) -> Value {
    json!({"stage":"execution","code":code,"message":message.to_string()})
}
fn retain_log(log: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() >= LOG_LIMIT { log.clear(); log.extend_from_slice(&bytes[bytes.len() - LOG_LIMIT..]); }
    else {
        let remove = (log.len() + bytes.len()).saturating_sub(LOG_LIMIT);
        if remove > 0 { log.drain(..remove); }
        log.extend_from_slice(bytes);
    }
}

// Only ordinary stdout is bounded. Trusted protocol payloads can contain full
// business documents and must not acquire a separate transport size limit.
fn records(mut reader: impl Read, mut visit: impl FnMut(&[u8], bool) -> Result<(), String>) -> Result<(), String> {
    let mut buffer = [0; 8192];
    let mut line = Vec::new();
    let mut protocol = None;
    loop {
        let count = reader.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 { if !line.is_empty() { visit(&line, protocol.unwrap_or(false))?; } return Ok(()); }
        for chunk in buffer[..count].split_inclusive(|byte| *byte == b'\n') {
            let newline = chunk.last() == Some(&b'\n');
            let mut bytes = if newline { &chunk[..chunk.len() - 1] } else { chunk };
            if protocol.is_none() {
                let prefix_count = (PREFIX.len() - line.len()).min(bytes.len());
                line.extend_from_slice(&bytes[..prefix_count]);
                bytes = &bytes[prefix_count..];
                if line.len() == PREFIX.len() { protocol = Some(line.as_slice() == PREFIX); }
            }
            if protocol == Some(true) { line.extend_from_slice(bytes); }
            else { retain_log(&mut line, bytes); }
            if newline { visit(&line, protocol.unwrap_or(false))?; line.clear(); protocol = None; }
        }
    }
}

struct Outcome { failure: Option<Value>, outputs: Vec<crate::scheduler::maps::ResolvedFile> }
struct ReapOnDrop(std::process::Child);
impl Drop for ReapOnDrop {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

fn execute(state: &AppState, run: &Run, execution: &NodeExecution, prepared: PreparedScript, input: String, runtime: tokio::runtime::Handle) -> Outcome {
    let mut outputs = Vec::new();
    let mut logs = Vec::new();
    let mut result = None;
    let work = (|| -> Result<(), Value> {
        let child = Command::new("bun").args(["--no-install", "--eval", include_str!("script-runner.ts")]).current_dir(&execution.zone).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            .map_err(|error| diagnostic("bun-unavailable", format!("无法启动 Bun；请安装 Bun 并确保服务进程 PATH 包含 bun（不自动下载）：{error}")))?;
        let mut child = ReapOnDrop(child);
        let mut stdin = child.0.stdin.take().expect("piped stdin");
        let stdout = child.0.stdout.take().expect("piped stdout");
        let mut stderr = child.0.stderr.take().expect("piped stderr");
        let stderr_thread = std::thread::spawn(move || {
            let mut log = Vec::new(); let mut bytes = [0;8192];
            loop { match stderr.read(&mut bytes) { Ok(0) | Err(_) => break, Ok(count) => retain_log(&mut log, &bytes[..count]) } }
            log
        });
        let (sender, receiver) = std::sync::mpsc::sync_channel(16);
        let stdout_thread = std::thread::spawn(move || {
            let mut log = Vec::new();
            let read = records(stdout, |line, protocol| {
                if !protocol { retain_log(&mut log, line); retain_log(&mut log, b"\n"); return Ok(()); }
                let message: Value = serde_json::from_slice(&line[PREFIX.len()..]).map_err(|error| error.to_string())?;
                sender.send(Ok(message)).map_err(|_| "协议接收端已关闭".to_string())
            });
            if let Err(error) = read { let _ = sender.send(Err(error)); }
            log
        });
        let request = json!({"script":prepared.path,"source":prepared.source,"context":{"runId":run.id,"executionId":execution.id,"zone":execution.zone,"input":input,"inputs":execution.inputs.as_deref().unwrap_or_default(),"outputRule":execution.output_rule.as_deref().unwrap_or_default(),"outputDirectory":Path::new(&run.zone).join(".manager-zone").join(".workflow").join(&run.id).join(&execution.id)}});
        let protocol = (|| -> Result<(), String> {
            serde_json::to_writer(&mut stdin, &request).map_err(|error| error.to_string())?;
            stdin.write_all(b"\n").map_err(|error| error.to_string())?;
            for message in &receiver {
                let message = message?;
                match message.get("type").and_then(Value::as_str) {
                    Some("call") if result.is_none() => {
                        let id = message.get("id").and_then(Value::as_u64).ok_or("无效 host call id")?;
                        let operation = message.get("operation").and_then(Value::as_str).ok_or("缺少 host operation")?;
                        let response = match runtime.block_on(crate::scheduler::bots::script_host::call(state, run, execution, operation, message.get("args").cloned().unwrap_or(Value::Null))) {
                            Ok(value) => json!({"id":id,"ok":true,"value":value}),
                            Err(error) => json!({"id":id,"ok":false,"error":error}),
                        };
                        serde_json::to_writer(&mut stdin, &response).map_err(|error| error.to_string())?;
                        stdin.write_all(b"\n").map_err(|error| error.to_string())?;
                    }
                    Some("result") if result.is_none() => { result = Some(message.get("value").cloned().unwrap_or(Value::Null)); }
                    _ => return Err("无效或重复脚本协议消息。".into()),
                }
            }
            Ok(())
        })();
        drop(receiver);
        drop(stdin);
        if protocol.is_err() { let _ = child.0.kill(); }
        let status = child.0.wait();
        if let Ok(stdout_log) = stdout_thread.join() { retain_log(&mut logs, &stdout_log); }
        if let Ok(stderr_log) = stderr_thread.join() { retain_log(&mut logs, &stderr_log); }
        protocol.map_err(|error| diagnostic("script-protocol", error))?;
        let status = status.map_err(|error| diagnostic("execution-failed", error))?;
        if !status.success() { return Err(diagnostic("script-process-failed", format!("Bun 退出：{status}"))); }
        Ok(())
    })();
    let mut failure = work.err();
    if let Some(value) = result {
        let status = value.get("status").and_then(Value::as_str);
        let mut validation = None;
        if let Some(files) = value.get("outputs") {
            if let Some(files) = files.as_array() {
                let mut seen = std::collections::BTreeSet::new();
                for file in files {
                    let checked = (|| -> Result<crate::scheduler::maps::ResolvedFile,String> {
                        let file: crate::scheduler::maps::ResolvedFile = serde_json::from_value(file.clone()).map_err(|error| error.to_string())?;
                        if !seen.insert(file.id.clone()) { return Err(format!("重复输出 id：{}", file.id)); }
                        if !execution.output_rule.as_deref().unwrap_or_default().iter().any(|rule| rule.id == file.id) { return Err(format!("未声明的输出 id：{}", file.id)); }
                        if !Path::new(&file.path).is_absolute() { return Err(format!("输出路径必须是绝对路径：{}", file.id)); }
                        if !std::fs::metadata(&file.path).map_err(|error| format!("输出 {} 无法读取：{error}",file.id))?.is_file() { return Err(format!("输出 {} 必须是普通文件",file.id)); }
                        let opened = std::fs::File::open(&file.path).map_err(|error| format!("输出 {} 无法读取：{error}",file.id))?;
                        if !opened.metadata().map_err(|error| error.to_string())?.is_file() { return Err(format!("输出 {} 必须是普通文件",file.id)); }
                        Ok(file)
                    })();
                    match checked { Ok(file) => outputs.push(file), Err(message) => { validation.get_or_insert_with(|| diagnostic("invalid-output", message)); } }
                }
            } else { validation = Some(diagnostic("invalid-output", "outputs 必须是数组")); }
        }
        if status == Some("success") && execution.output_rule.as_deref().unwrap_or_default().iter().any(|rule| !outputs.iter().any(|file| file.id == rule.id)) {
            validation.get_or_insert_with(|| diagnostic("missing-output", "成功结果缺少声明的输出文件"));
        }
        let script_failure = value.get("failure").filter(|failure| failure.is_object()).cloned();
        if failure.is_none() {
            failure = match status {
                Some("success") => validation.take(),
                Some("failure") => Some(script_failure.clone().unwrap_or_else(|| diagnostic("script-failed", "脚本返回 failure"))),
                _ => Some(diagnostic("invalid-result", "脚本结果必须包含 status: success 或 failure")),
            };
        } else if let (Some(failure), Some(script_failure)) = (&mut failure, script_failure) {
            failure["scriptFailure"] = script_failure;
        }
        if let (Some(failure), Some(validation)) = (&mut failure, validation) { failure["outputValidation"] = validation; }
    } else if failure.is_none() { failure = Some(diagnostic("missing-result", "脚本没有返回结果")); }
    if let Some(failure) = &mut failure { if !logs.is_empty() { failure["logs"] = String::from_utf8_lossy(&logs).into_owned().into(); } }
    Outcome { failure, outputs }
}

pub(super) fn launch(state: AppState, run: Run, node_id: String, prepared: PreparedScript, input: String) {
    let execution = run.nodes[&node_id].execution.as_ref().expect("saved script execution").clone();
    let script = execution.script().expect("only TS targets launch");
    let started_at = match script.lifecycle { Lifecycle::Running { started_at } => started_at, _ => unreachable!("new script must be running") };
    let binding = script.script.clone();
    tokio::spawn(async move {
        let runtime = tokio::runtime::Handle::current();
        let owned = state.clone();
        let run_id = run.id.clone();
        let execution_id = execution.id.clone();
        let outcome = tokio::task::spawn_blocking(move || execute(&owned, &run, &execution, prepared, input, runtime)).await;
        let Outcome { failure, outputs } = outcome.unwrap_or_else(|error| Outcome { failure: Some(diagnostic("execution-failed", error)), outputs: Vec::new() });
        let ended_at = now().max(started_at);
        let (lifecycle, body, can_complete) = match failure {
            Some(failure) => {
                let body = format!("TypeScript {binding} 失败；未自动完成节点，也未回滚实际效果。\n{failure}");
                (Lifecycle::Failed { started_at, ended_at, failure }, body, false)
            }
            None => (Lifecycle::Succeeded { started_at, ended_at }, format!("TypeScript {binding} 成功；输出已验证，尚待 manager 显式 complete。"), true),
        };
        if let Err(error) = reports::submit(state, run_id.clone(), node_id.clone(), execution_id.clone(), reports::ReportSubmission {
            origin: reports::ReportOrigin::Script(lifecycle), body, can_complete, outputs,
        }).await {
            let message = match error { crate::report_delivery::SubmitError::Save(message) => message, crate::report_delivery::SubmitError::Internal(error) => error.to_string() };
            eprintln!("[workflow] run={run_id} node={node_id} execution={execution_id} script-report-save-failed={message}；不重投业务或伪造成功报告。");
        }
    });
}
