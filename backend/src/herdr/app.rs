//! App owns the layout; core owns the terminal. A core address such as `t1`
//! is local to one core and cannot identify the connected instance by itself.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::client::{CreatedTab, HerdrError, QUERY_TIMEOUT, WorkspaceInventory, nonempty, run_cli};

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
    core_id: String,
    address: String,
}

impl AppTerminal {
    pub(super) fn coordinates(&self) -> (&str, &str) { (&self.core_id, &self.address) }
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

    pub(super) async fn create_tab(
        &self, space_id: &str, cwd: Option<&str>, name: &str,
    ) -> Result<CreatedTab, HerdrError> {
        self.create(space_id, cwd, name, None).await.map(|(tab, _)| tab)
    }

    pub(super) async fn create(
        &self, space_id: &str, cwd: Option<&str>, name: &str, argv: Option<&[String]>,
    ) -> Result<(CreatedTab, AppTerminal), HerdrError> {
        let spaces = self.spaces().await?;
        let space = find_id(&spaces, space_id).ok_or_else(|| denied("所选 App Space 不再存在或 ID 不唯一"))?;
        if !space["machine"].is_null() {
            return Err(denied("本机 zone 不能放入远程 App Space"));
        }

        // On Windows status queries an existing core without starting one, unlike
        // `hc get`. Do not let a missing core turn into an implicit startup.
        let status = run_cli(&self.core, CORE_STATUS, &["server", "status", "--json"], QUERY_TIMEOUT).await?;
        let core_id = nonempty(status.pointer("/core/id"))
            .ok_or_else(|| unexpected(CORE_STATUS, "响应缺少 core 身份"))?;

        // Check existing evidence before mutation. A mod-only Space has no
        // terminal to witness, so its first Tab must be verified after creation.
        let tabs = array(space, "tabs", LIST)?;
        if let Some((tab, terminal)) = tabs.iter().find_map(|tab| {
            let tab_id = nonempty(tab.get("id"))?;
            let terminals = tab.get("terminals")?.as_array()?;
            let terminal = terminals.iter().find_map(|term| nonempty(term.get("terminal")))?;
            Some((tab_id, terminal))
        }) {
            self.verify_terminal(space_id, tab, terminal).await.map_err(|error| denied(&error.message()))?;
        }

        let mut args = json!({ "space": space_id, "name": name, "focus": false });
        if let Some(argv) = argv { args["argv"] = json!(argv); }
        if let Some(cwd) = cwd {
            args["folder"] = cwd.into();
        }
        let args = args.to_string();
        let created = run_cli(&self.executable, CREATE_TAB, &["do", "tab.new", &args], Duration::from_secs(20)).await?;
        let tab_id = required(&created, "tab")?;
        let terminal = required(&created, "terminal")?;
        if required(&created, "space")? != space_id {
            return Err(unexpected(CREATE_TAB, "回执的 Space 与所选目标不一致"));
        }

        // The reply is not a layout observation, nor permission to use a core
        // terminal with the same short address. Require both public surfaces.
        let updated = self.spaces().await.map_err(|error| unexpected(CREATE_TAB, &error.message()))?;
        let space = find_id(&updated, space_id).ok_or_else(|| unexpected(CREATE_TAB, "未确认所选 Space"))?;
        let tab = find_id(array(space, "tabs", CREATE_TAB)?, tab_id)
            .ok_or_else(|| unexpected(CREATE_TAB, "未在所选 Space 中确认新 Tab"))?;
        let mut members = array(tab, "terminals", CREATE_TAB)?.iter()
            .filter(|entry| nonempty(entry.get("terminal")) == Some(terminal));
        let pane_id = members.next().and_then(|entry| nonempty(entry.get("pane")))
            .filter(|_| members.next().is_none())
            .ok_or_else(|| unexpected(CREATE_TAB, "未在新 Tab 中确认唯一的终端 Pane"))?;
        self.verify_terminal(space_id, tab_id, terminal).await
            .map_err(|error| unexpected(CREATE_TAB, &error.message()))?;
        Ok((CreatedTab { tab_id: tab_id.into(), pane_id: pane_id.into() }, AppTerminal {
            core_id: core_id.into(), address: terminal.into(),
        }))
    }

    pub(super) async fn observe_launch(&self, location: &super::NavigationTarget, target: &AppTerminal) -> Result<Value, HerdrError> {
        let spaces = self.spaces().await?;
        let space = find_id(&spaces, &location.workspace_id).ok_or_else(|| denied("启动 Space 已不存在"))?;
        let tab = find_id(array(space, "tabs", LIST)?, &location.tab_id).ok_or_else(|| denied("启动 Tab 已不存在"))?;
        let mut panes = array(tab, "terminals", LIST)?.iter().filter(|pane| {
            nonempty(pane.get("pane")) == Some(location.pane_id.as_str())
                && nonempty(pane.get("terminal")) == Some(target.address.as_str())
        });
        if panes.next().is_none() || panes.next().is_some() { return Err(denied("启动 Pane 与终端的关联已改变")); }
        let status = run_cli(&self.core, CORE_STATUS, &["server", "status", "--json"], QUERY_TIMEOUT).await?;
        if nonempty(status.pointer("/core/id")) != Some(target.core_id.as_str()) {
            return Err(unexpected(CORE_STATUS, "启动所用的 core 实例已改变"));
        }
        self.verify_terminal(&location.workspace_id, &location.tab_id, &target.address).await
    }

    pub(super) async fn submit(&self, target: &AppTerminal, text: &str) -> Result<(), HerdrError> {
        const SEND: &str = "hc send";
        let result = run_cli(&self.core, SEND, &["send", "--json", &target.address, "--", text], Duration::from_secs(75)).await?;
        if result.get("turn").and_then(Value::as_u64).is_none()
            || !matches!(result.get("started"), Some(Value::Bool(true) | Value::Null))
        {
            return Err(unexpected(SEND, "未能确认初始指示的提交结果"));
        }
        Ok(())
    }

    pub(super) async fn verify_terminal(&self, space: &str, tab: &str, terminal: &str) -> Result<Value, HerdrError> {
        let record = run_cli(&self.core, CORE_GET, &["get", terminal, "--json"], QUERY_TIMEOUT).await?;
        if nonempty(record.get("address")) != Some(terminal)
            || nonempty(record.pointer("/labels/herdr.space")) != Some(space)
            || nonempty(record.pointer("/labels/herdr.tab")) != Some(tab)
        {
            return Err(unexpected(CORE_GET, "App/core 实例不一致：终端不属于 App 返回的 Space 与 Tab"));
        }
        Ok(record)
    }
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
