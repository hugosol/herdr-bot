//! App owns the layout; core owns the terminal. A core address such as `t1`
//! is local to one core and cannot identify the connected instance by itself.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::{
    NavigationTarget,
    client::{CreatedTab, HerdrError, QUERY_TIMEOUT, WorkspaceInventory, nonempty, run_cli, run_bound_core_cli},
    launch::process_is_descendant,
};

#[path = "app_navigation.rs"]
mod navigation;
#[path = "app_receipt.rs"]
mod receipt;
use receipt::Receipt;

const LIST: &str = "herdr-app ls";
const CREATE_TAB: &str = "herdr-app tab.new";
const CORE_STATUS: &str = "hc server status";
const CORE_GET: &str = "hc get";

#[derive(Debug, Clone)]
pub(super) struct AppHost {
    executable: PathBuf,
    core: PathBuf,
}

#[derive(Debug)]
pub(super) struct AppTerminal {
    endpoint: String,
    core_id: String,
    core_pid: u32,
    address: String,
    nonce: String,
    root_pid: u32,
    bootstrap_pid: u32,
    correlation: Option<(String, String)>,
}

impl AppTerminal {
    pub(super) fn reported_bootstrap_pid(&self) -> Option<u32> {
        self.correlation.as_ref().map(|_| self.bootstrap_pid)
    }
}

impl AppHost {
    pub(super) fn new(executable: PathBuf, core: PathBuf) -> Self {
        Self { executable, core }
    }

    pub(super) async fn inventory(&self) -> Result<WorkspaceInventory, HerdrError> {
        Ok(WorkspaceInventory {
            records: self.spaces().await?, command: LIST, id_key: "id", name_key: "name",
        })
    }

    pub(super) async fn spaces(&self) -> Result<Vec<Value>, HerdrError> {
        let mut value = run_cli(&self.executable, LIST, &["ls"], QUERY_TIMEOUT).await?;
        match value.get_mut("spaces").map(Value::take) {
            Some(Value::Array(spaces)) => Ok(spaces),
            _ => Err(unexpected(LIST, "响应缺少 spaces 数组")),
        }
    }

    pub(super) async fn create(
        &self, space_id: &str, cwd: Option<&str>, name: &str, agent: &str, args: &[String],
        correlation: Option<(&str, &str)>,
    ) -> Result<(CreatedTab, AppTerminal), HerdrError> {
        let spaces = self.spaces().await?;
        let space = find_id(&spaces, space_id).ok_or_else(|| denied("所选 App Space 不再存在或 ID 不唯一"))?;
        if !space["machine"].is_null() {
            return Err(denied("本机 zone 不能放入远程 App Space"));
        }
        array(space, "tabs", LIST)?;

        let receipt = Receipt::new()?;
        let mut request = json!({ "space": space_id, "name": name, "focus": false, "argv": receipt.argv(agent, args, correlation)? });
        if let Some(cwd) = cwd {
            request["folder"] = cwd.into();
        }
        let request = request.to_string();
        let created = run_cli(&self.executable, CREATE_TAB, &["do", "tab.new", &request], Duration::from_secs(20)).await?;
        let tab_id = required(&created, "tab")?;
        let terminal = required(&created, "terminal")?;
        if required(&created, "space")? != space_id || !local_address(terminal) {
            return Err(unexpected(CREATE_TAB, "回执未确认所选 Space 的本机终端"));
        }

        let updated = self.spaces().await.map_err(|error| unexpected(CREATE_TAB, &error.message()))?;
        let pane_id = bound_pane(&updated, space_id, tab_id, terminal)
            .map_err(|error| unexpected(CREATE_TAB, &error.message()))?;
        let receipt = receipt.receive(terminal).await?;
        if !local_endpoint(&receipt.socket) {
            return Err(unexpected(CREATE_TAB, "启动回执未提供本机 core 端点"));
        }
        // App has already acknowledged creation. A failed observation does not
        // prove absence and must not authorize another tab.new attempt.
        let after_create = |error: HerdrError| unexpected(CREATE_TAB, &error.message());
        let status = self.core_status(&receipt.socket).await.map_err(after_create)?;
        let (core_id, core_pid) = core_identity(&status)?;
        let record = self.core_get(&receipt.socket, terminal).await.map_err(after_create)?;
        let correlation = correlation.map(|(launch, target)| (launch.into(), target.into()));
        let root_pid = verify_process(&record, terminal, &receipt.nonce, receipt.pid, correlation.as_ref()).await?;
        Ok((CreatedTab { tab_id: tab_id.into(), pane_id: pane_id.into() }, AppTerminal {
            endpoint: receipt.socket, core_id: core_id.into(), core_pid, address: receipt.terminal,
            nonce: receipt.nonce, root_pid, bootstrap_pid: receipt.pid, correlation,
        }))
    }

    pub(super) async fn observe_launch(&self, location: &NavigationTarget, target: &AppTerminal) -> Result<Value, HerdrError> {
        let spaces = self.spaces().await?;
        if bound_pane(&spaces, &location.workspace_id, &location.tab_id, &target.address)? != location.pane_id {
            return Err(unexpected(LIST, "启动 Pane 与终端的关联已改变"));
        }
        let status = self.core_status(&target.endpoint).await?;
        if core_identity(&status)? != (target.core_id.as_str(), target.core_pid) {
            return Err(unexpected(CORE_STATUS, "启动所用的 core 实例已改变"));
        }
        let record = self.core_get(&target.endpoint, &target.address).await?;
        if pid(&record) != Some(target.root_pid) {
            return Err(unexpected(CORE_GET, "终端根进程已改变"));
        }
        verify_process(&record, &target.address, &target.nonce, target.bootstrap_pid, target.correlation.as_ref()).await?;
        Ok(record)
    }

    pub(super) async fn close_tab(&self, location: &NavigationTarget, target: &AppTerminal) -> Result<(), HerdrError> {
        const CLOSE: &str = "herdr-app tab.close";
        self.observe_launch(location, target).await?;
        // App checks the Tab count at execution. This receipt does not prove that
        // the Tab was still in this Space at execution or that its process exited.
        let args = json!({"id":location.tab_id,"confirmed":true}).to_string();
        let reply = run_cli(&self.executable, CLOSE, &["do", "tab.close", &args], QUERY_TIMEOUT).await?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(unexpected(CLOSE, "缺少 App 关闭回执"));
        }
        // App kills terminals asynchronously. This is only its command receipt.
        Ok(())
    }

    pub(super) async fn submit(&self, target: &AppTerminal, text: &str) -> Result<(), HerdrError> {
        const SEND: &str = "hc send";
        let result = run_bound_core_cli(&self.core, &target.endpoint, SEND, &["send", "--json", &target.address, "--", text], Duration::from_secs(75)).await?;
        if result.get("started").and_then(Value::as_bool) == Some(false) {
            return Err(unexpected(SEND, "宿主回执 started:false，不能确认初始指示的提交结果"));
        }
        if result.get("turn").and_then(Value::as_u64).is_none()
            || !matches!(result.get("started"), Some(Value::Bool(true) | Value::Null))
        {
            return Err(unexpected(SEND, "未能确认初始指示的提交结果"));
        }
        Ok(())
    }

    async fn core_status(&self, endpoint: &str) -> Result<Value, HerdrError> {
        run_bound_core_cli(&self.core, endpoint, CORE_STATUS, &["server", "status", "--json"], QUERY_TIMEOUT).await
    }

    async fn core_get(&self, endpoint: &str, terminal: &str) -> Result<Value, HerdrError> {
        run_bound_core_cli(&self.core, endpoint, CORE_GET, &["get", terminal, "--json"], QUERY_TIMEOUT).await
    }
}

fn bound_pane<'a>(spaces: &'a [Value], space_id: &str, tab_id: &str, terminal: &str) -> Result<&'a str, HerdrError> {
    let space = find_id(spaces, space_id).filter(|space| space["machine"].is_null())
        .ok_or_else(|| unexpected(LIST, "未确认启动所用的本机 Space"))?;
    let tab = find_id(array(space, "tabs", LIST)?, tab_id)
        .ok_or_else(|| unexpected(LIST, "未在启动 Space 中确认唯一的 Tab"))?;
    let terminals = array(tab, "terminals", LIST)?;
    let mut members = terminals.iter().filter(|entry| nonempty(entry.get("terminal")) == Some(terminal));
    members.next().and_then(|entry| nonempty(entry.get("pane")))
        .filter(|pane| members.next().is_none() && find_pane_once(terminals, pane))
        .ok_or_else(|| unexpected(LIST, "未在启动 Tab 中确认唯一的终端 Pane"))
}

fn find_pane_once(terminals: &[Value], pane: &str) -> bool {
    terminals.iter().filter(|entry| nonempty(entry.get("pane")) == Some(pane)).nth(1).is_none()
}

fn local_address(address: &str) -> bool {
    let mut bytes = address.bytes();
    bytes.next() == Some(b't') && matches!(bytes.next(), Some(b'1'..=b'9' | b'a'..=b'z'))
        && bytes.all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

#[cfg(windows)]
fn local_endpoint(endpoint: &str) -> bool {
    const LOCAL_PIPE: &str = r"\\.\pipe\";
    let Some(prefix) = endpoint.get(..LOCAL_PIPE.len()) else { return false; };
    let name = &endpoint[LOCAL_PIPE.len()..];
    prefix.eq_ignore_ascii_case(LOCAL_PIPE) && !name.is_empty()
        && !name.contains(['\\', '/', ':']) && !name.chars().any(char::is_control)
}

#[cfg(unix)]
fn local_endpoint(endpoint: &str) -> bool {
    std::path::Path::new(endpoint).is_absolute() && !endpoint.chars().any(char::is_control)
}

fn core_identity(status: &Value) -> Result<(&str, u32), HerdrError> {
    nonempty(status.pointer("/core/id")).zip(pid(status))
        .ok_or_else(|| unexpected(CORE_STATUS, "响应缺少 core 身份或进程 PID"))
}

fn pid(value: &Value) -> Option<u32> {
    u32::try_from(value.get("pid")?.as_u64()?).ok().filter(|pid| *pid != 0)
}

async fn verify_process(
    record: &Value, terminal: &str, nonce: &str, bootstrap_pid: u32, correlation: Option<&(String, String)>,
) -> Result<u32, HerdrError> {
    let argv = record.get("argv").and_then(Value::as_array)
        .ok_or_else(|| unexpected(CORE_GET, "终端缺少启动程序 argv"))?;
    let has_token = |token: &str| argv.iter().filter_map(Value::as_str).any(|arg| {
        // App passes separate arguments on Unix and one quoted command to PowerShell.
        arg == token || cfg!(windows) && arg.split_ascii_whitespace().any(|word| {
            word.strip_prefix('\'').and_then(|word| word.strip_suffix('\'')) == Some(token)
        })
    });
    if nonempty(record.get("address")) != Some(terminal) || nonempty(record.get("state")) != Some("live")
        || !has_token(nonce) || correlation.is_some_and(|(launch, target)| !has_token(launch) || !has_token(target))
    {
        return Err(unexpected(CORE_GET, "终端未运行本次 App 启动程序"));
    }
    let root_pid = pid(record).ok_or_else(|| unexpected(CORE_GET, "终端缺少活跃根进程 PID"))?;
    if root_pid != bootstrap_pid && !tokio::task::spawn_blocking(move || process_is_descendant(root_pid, bootstrap_pid)).await.unwrap_or(false) {
        return Err(unexpected(CORE_GET, "启动回执 PID 不属于终端根进程"));
    }
    Ok(root_pid)
}

fn find_id<'a>(rows: &'a [Value], id: &str) -> Option<&'a Value> {
    let mut matches = rows.iter().filter(|row| nonempty(row.get("id")) == Some(id));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn array<'a>(value: &'a Value, key: &str, command: &'static str) -> Result<&'a [Value], HerdrError> {
    value.get(key).and_then(Value::as_array).map(Vec::as_slice)
        .ok_or_else(|| unexpected(command, &format!("响应缺少 {key} 数组")))
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, HerdrError> {
    nonempty(value.get(key)).ok_or_else(|| unexpected(CREATE_TAB, &format!("回执缺少 {key}")))
}

fn denied(message: &str) -> HerdrError {
    HerdrError::CommandFailed { command: CREATE_TAB, message: message.into() }
}

fn unexpected(command: &'static str, detail: &str) -> HerdrError {
    HerdrError::UnexpectedResponse { command, detail: detail.into() }
}
