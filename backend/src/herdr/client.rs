//! Herdr 公开控制接口的生产 adapter：通过 `herdr` CLI 的新行分隔 JSON 输出执行控制命令。
//!
//! 这里刻意**不经过 Shell**：可执行文件与每个参数都作为独立进程参数提交，因此 Chat 名称、zone
//! 目录、Task prompt 与用户 input 中的引号、空格、换行或 Shell 元字符只作为数据到达 Herdr，
//! 不会被解释成命令。选择 CLI 而不是直接连 socket 只是一个 adapter 选择；对上层而言，它只暴露
//! Workspace 查询、Tab 创建、agent 启动/就绪、原生身份查找、已有目标聚焦与初始输入提交。
//!
//! 所有命令都是一次调用：失败、超时或无法解析都不自动重试，也不回退到其他 Workspace。成功响应
//! 只表示 Herdr 接受了那一步；`agent.start` 的 CLI 语义是目标 agent 已可交互，`agent.prompt`
//! 只表示提交被接受，不表示模型完成任务。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::config::{HerdrBackend, HerdrConfig};

use super::app::AppHost;

use super::identity::{
    candidate_native_id, native_error, native_header_or_pending, read_native_id,
    saved_pending_native_session,
};

/// 查询命令的时限：Workspace 列表必须很快返回，否则按 Herdr 不可达处理。
pub(super) const QUERY_TIMEOUT: Duration = Duration::from_secs(15);
/// 新建 Tab 的时限。
const TAB_TIMEOUT: Duration = Duration::from_secs(20);
/// 启动 agent 的时限；Herdr CLI 自身的等待就绪上限是 30 秒，这里给它留出返回错误的余量。
const START_TIMEOUT: Duration = Duration::from_secs(40);
/// 启动 agent 时交给 Herdr CLI 的等待就绪上限（毫秒，CLI 允许 3000..=300000）。
const START_READY_TIMEOUT_MS: u32 = 30_000;
/// 提交初始指示的时限。超时不证明未提交，调用方必须保留这一不确定性。
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// 与固定操作相同的无控制台窗口启动标志。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 一个 Herdr Workspace 的公开身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub label: String,
}

/// Owned wire rows, with each adapter identifying its two placement fields.
pub(super) struct WorkspaceInventory {
    pub records: Vec<Value>,
    pub command: &'static str,
    pub id_key: &'static str,
    pub name_key: &'static str,
}

impl WorkspaceInventory {
    fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.records.iter().filter_map(|entry| Some((
            nonempty(entry.get(self.id_key))?,
            nonempty(entry.get(self.name_key))?,
        )))
    }

    pub(super) fn error(&self, detail: &str) -> HerdrError {
        HerdrError::UnexpectedResponse { command: self.command, detail: detail.into() }
    }
}

/// 新建 Tab 后由 Herdr 返回的对象标识；两个标识都取自响应，不按名称或序号猜测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedTab {
    pub tab_id: String,
    pub pane_id: String,
}

/// Herdr 控制失败：每个类别都说明失败发生在哪个控制步骤，不把未知说成确定。
#[derive(Debug, Clone)]
pub enum HerdrError {
    /// CLI 未能生成进程，确认该次命令没有外部效果。
    NotStarted(String),
    /// CLI 已生成，但无法可靠观察通信或结束；外部效果不确定。
    Unavailable(String),
    /// 命令在时限内没有返回；不能据此断言操作没有发生。
    Timeout(&'static str),
    /// 配置名称没有匹配到任何 Workspace。
    WorkspaceNotFound { name: String },
    /// 配置名称匹配到多个 Workspace，不能任取一个。
    WorkspaceAmbiguous { name: String, matches: usize },
    /// Closing the selected Tab would also close other workspaces.
    UnsafeCloseScope,
    /// Herdr 明确返回了失败。
    CommandFailed {
        command: &'static str,
        message: String,
    },
    /// The input operation explicitly rejected a blocked target before writing.
    InputBlocked { command: &'static str, message: String },
    /// Herdr 成功退出但响应缺少本 adapter 需要的对象标识。
    UnexpectedResponse {
        command: &'static str,
        detail: String,
    },
}

impl HerdrError {
    /// 机器可读的稳定类别。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotStarted(_) | Self::Unavailable(_) => "herdr-unavailable",
            Self::Timeout(_) => "herdr-timeout",
            Self::WorkspaceNotFound { .. } => "workspace-not-found",
            Self::WorkspaceAmbiguous { .. } => "workspace-ambiguous",
            Self::UnsafeCloseScope => "close-scope-denied",
            Self::CommandFailed { .. } => "herdr-command-failed",
            Self::InputBlocked { .. } => "herdr-input-blocked",
            Self::UnexpectedResponse { .. } => "herdr-unexpected-response",
        }
    }

    /// 面向使用者的说明。
    pub fn message(&self) -> String {
        match self {
            Self::NotStarted(reason) => {
                format!("无法运行或连接 Herdr：{reason}。本次没有继续创建。")
            }
            Self::Unavailable(reason) => {
                format!("无法确认 Herdr 命令结果：{reason}。外部效果不确定，本次没有继续创建。")
            }
            Self::Timeout(command) => {
                format!("Herdr 的 {command} 在时限内没有返回；无法确认这一步是否已经产生效果。")
            }
            Self::WorkspaceNotFound { name } => {
                format!("找不到名称恰好为 “{name}” 的 Herdr Workspace（匹配到 0 个）。")
            }
            Self::WorkspaceAmbiguous { name, matches } => {
                format!(
                    "名称恰好为 “{name}” 的 Herdr Workspace 有 {matches} 个；不能任取一个，本次没有继续创建。"
                )
            }
            Self::UnsafeCloseScope => "关闭此 Tab 会扩大到 worktree group；未发送关闭命令。".into(),
            Self::CommandFailed { command, message } | Self::InputBlocked { command, message } => {
                format!("Herdr 的 {command} 失败：{message}")
            }
            Self::UnexpectedResponse { command, detail } => {
                format!("Herdr 的 {command} 返回了无法理解的响应：{detail}")
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Host {
    Tui(PathBuf),
    App(AppHost),
}

/// 生产用的 Herdr 控制；运行期间只持有所选宿主的可执行路径。
#[derive(Debug, Clone)]
pub struct HerdrClient {
    host: Host,
    workspace_name: String,
}

impl HerdrClient {
    pub fn new(config: &HerdrConfig) -> Self {
        Self {
            host: match config.backend {
                HerdrBackend::Tui => Host::Tui(config.executable.clone()),
                HerdrBackend::App => Host::App(AppHost::new(config.app_executable.clone(), config.core_executable.clone())),
            },
            workspace_name: config.workspace_name.clone(),
        }
    }

    /// 目标 Workspace 名称（启动日志与诊断使用）。
    pub fn workspace_name(&self) -> &str {
        &self.workspace_name
    }

    pub(crate) fn workspace_kind(&self) -> &'static str {
        match self.host {
            Host::Tui(_) => "Workspace",
            Host::App(_) => "Space",
        }
    }

    /// 按本次目标名称精确查找唯一容器；焦点与顺序都不参与选择。
    pub async fn workspace(&self, name: &str) -> Result<Workspace, HerdrError> {
        let inventory = self.workspace_inventory().await?;
        let mut matches = inventory.entries().filter(|(_, label)| *label == name);
        let (id, label) = matches.next().ok_or_else(|| HerdrError::WorkspaceNotFound { name: name.into() })?;
        let count = matches.count() + 1;
        if count != 1 {
            return Err(HerdrError::WorkspaceAmbiguous { name: name.into(), matches: count });
        }
        Ok(Workspace { id: id.into(), label: label.into() })
    }

    /// Display-only inventory; callers must retain saved IDs when labels are unavailable.
    pub(crate) async fn workspace_labels(&self) -> Result<Vec<Workspace>, HerdrError> {
        let inventory = self.workspace_inventory().await?;
        Ok(inventory.entries().map(|(id, label)| Workspace { id: id.into(), label: label.into() }).collect())
    }

    /// Only a successful inventory with valid IDs can prove the original ID is absent.
    pub(crate) async fn workspace_by_id(&self, id: &str) -> Result<Option<Workspace>, HerdrError> {
        let inventory = self.workspace_inventory().await?;
        let mut original = None;
        for entry in &inventory.records {
            let entry_id = nonempty(entry.get(inventory.id_key))
                .ok_or_else(|| inventory.error("容器缺少 ID，无法确认原位置"))?;
            let label = nonempty(entry.get(inventory.name_key))
                .ok_or_else(|| inventory.error("容器缺少名称，无法确认原位置"))?;
            if entry_id == id && original.replace(label).is_some() {
                return Err(inventory.error("原容器 ID 重复"));
            }
        }
        Ok(original.map(|label| Workspace { id: id.into(), label: label.into() }))
    }

    pub(super) async fn workspace_inventory(&self) -> Result<WorkspaceInventory, HerdrError> {
        if let Host::App(app) = &self.host {
            return app.inventory().await;
        }
        let mut value = self.run("workspace list", &["workspace".into(), "list".into()], QUERY_TIMEOUT).await?;
        let Some(Value::Array(records)) = value.pointer_mut("/result/workspaces").map(Value::take) else {
            return Err(HerdrError::UnexpectedResponse { command: "workspace list", detail: "响应缺少 result.workspaces 数组".into() });
        };
        Ok(WorkspaceInventory { records, command: "workspace list", id_key: "workspace_id", name_key: "label" })
    }

    /// Only exact checkout metadata proves ownership; labels and path basenames do not.
    pub(crate) async fn main_worktree_for(&self, cwd: &str, timeout: Duration) -> Result<Option<String>, HerdrError> {
        if matches!(self.host, Host::App(_)) {
            // App does not expose the exact checkout ownership evidence needed here.
            return Ok(None);
        }
        let inventory = self.workspace_inventory().await?;
        let mut roots = Vec::new();
        for entry in &inventory.records {
            let Some(checkout) = nonempty(entry.pointer("/worktree/checkout_path")) else { continue; };
            if !crate::zones::same_path(&crate::zones::display_path(Path::new(checkout)), &crate::zones::display_path(Path::new(cwd))) { continue; }
            let Some(key) = nonempty(entry.pointer("/worktree/repo_key")) else { continue; };
            let key = PathBuf::from(key);
            if !key.is_absolute() || !key.is_dir() { continue; }
            let root = tokio::task::spawn_blocking(move || {
                // Bind Git to the reported metadata directory itself: ordinary
                // discovery from a path inside a checkout is not ownership proof.
                let output = crate::zones::git::run_git(&key, &["--git-dir=.", "worktree", "list", "--porcelain", "-z"], timeout).ok()?;
                if !output.status.success() { return None; }
                let text = std::str::from_utf8(&output.stdout).ok()?;
                let mut fields = text.split('\0');
                let root = fields.next()?.strip_prefix("worktree ")?;
                if fields.take_while(|field| !field.is_empty()).any(|field| field == "bare") { return None; }
                let root = crate::zones::display_path(Path::new(root));
                (Path::new(&root).is_absolute() && Path::new(&root).is_dir()).then_some(root)
            }).await.ok().flatten();
            if let Some(root) = root
                && !roots.iter().any(|known: &String| crate::zones::same_path(known, &root)) {
                roots.push(root);
            }
        }
        Ok(if roots.len() == 1 { roots.pop() } else { None })
    }

    pub(crate) fn is_app(&self) -> bool { matches!(self.host, Host::App(_)) }

    /// App creates the tab and its program together. TUI creates an owned
    /// pane first and passes the same launch correlation through its env.
    pub(crate) async fn launch_tab(
        &self, workspace: &str, cwd: Option<&str>, label: &str, alias: &str,
        kind: &str, args: &[String], launch: &super::launch::Launch,
    ) -> Result<std::sync::Arc<LaunchTarget>, HerdrError> {
        let (launch_id, target_id) = launch.correlation();
        let (tab, control) = match &self.host {
            Host::Tui(_) => {
                let env = [format!("{}={launch_id}", super::launch::LAUNCH_ID_ENV), format!("{}={target_id}", super::launch::TARGET_ID_ENV)];
                let tab = self.create_tui_tab(workspace, cwd, label, &env).await?;
                (tab, LaunchControl::Tui { alias: alias.into() })
            }
            Host::App(app) => {
                let (tab, terminal) = app.create(workspace, cwd, label, kind, args, Some((launch_id, target_id))).await?;
                (tab, LaunchControl::App { terminal })
            }
        };
        let target = std::sync::Arc::new(LaunchTarget {
            location: NavigationTarget { workspace_id: workspace.into(), tab_id: tab.tab_id, pane_id: tab.pane_id },
            control,
        });
        launch.bind_target(target.clone());
        Ok(target)
    }

    pub(crate) async fn launch_unreported_app(&self, workspace: &str, cwd: Option<&str>, label: &str, kind: &str, args: &[String]) -> Result<std::sync::Arc<LaunchTarget>, HerdrError> {
        let Host::App(app) = &self.host else { return Err(native_error("launch requires selected App host")); };
        let (tab, terminal) = app.create(workspace, cwd, label, kind, args, None).await?;
        Ok(std::sync::Arc::new(LaunchTarget {
            location: NavigationTarget { workspace_id: workspace.into(), tab_id: tab.tab_id, pane_id: tab.pane_id },
            control: LaunchControl::App { terminal },
        }))
    }

    /// 在指定 TUI Workspace 中新建 Tab，不改变当前焦点。
    pub(crate) async fn create_tab(
        &self,
        workspace_id: &str,
        cwd: Option<&str>,
        label: &str,
    ) -> Result<CreatedTab, HerdrError> {
        self.create_tui_tab(workspace_id, cwd, label, &[]).await
    }

    async fn create_tui_tab(&self, workspace_id: &str, cwd: Option<&str>, label: &str, env: &[String]) -> Result<CreatedTab, HerdrError> {
        let mut args = vec!["tab".into(), "create".into(), "--workspace".into(), workspace_id.into()];
        if let Some(cwd) = cwd { args.extend(["--cwd".into(), cwd.into()]); }
        args.extend(["--label".into(), label.into(), "--no-focus".into()]);
        for value in env { args.extend(["--env".into(), value.clone()]); }
        let value = self
            .run(
                "tab create",
                &args,
                TAB_TIMEOUT,
            )
            .await?;

        let tab_id = value
            .pointer("/result/tab/tab_id")
            .and_then(Value::as_str)
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "tab create",
                detail: "响应缺少 result.tab.tab_id".to_string(),
            })?;
        let pane_id = value
            .pointer("/result/root_pane/pane_id")
            .and_then(Value::as_str)
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "tab create",
                detail: "响应缺少 result.root_pane.pane_id".to_string(),
            })?;

        Ok(CreatedTab {
            tab_id: tab_id.to_string(),
            pane_id: pane_id.to_string(),
        })
    }

    /// 在已存在的 pane 中启动 agent，并等待 Herdr 确认它已可交互。
    ///
    /// 返回成功就是“目标 agent 可以接收输入”的确认；失败或超时都不得继续提交初始指示。
    pub async fn start_agent(
        &self,
        name: &str,
        kind: &str,
        pane_id: &str,
        args: &[String],
    ) -> Result<(), HerdrError> {
        let mut command = vec![
            "agent".to_string(),
            "start".to_string(),
            name.to_string(),
            "--kind".to_string(),
            kind.to_string(),
            "--pane".to_string(),
            pane_id.to_string(),
            "--timeout".to_string(),
            START_READY_TIMEOUT_MS.to_string(),
        ];
        if !args.is_empty() {
            command.push("--".to_string());
            command.extend_from_slice(args);
        }

        let value = self.run("agent start", &command, START_TIMEOUT).await?;
        value
            .pointer("/result/agent")
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "agent start",
                detail: "响应缺少 result.agent".to_string(),
            })?;
        Ok(())
    }

    /// 把合成后的初始指示作为一次提交交给目标 agent。
    ///
    /// 不使用 `--wait`：创建成功只要求提交被确认，不等待模型完成一轮任务。超时或错误都不重试。
    pub async fn prompt(&self, target: &str, text: &str) -> Result<(), HerdrError> {
        let value = self.prompt_once(target, text).await.map_err(|error| match error {
            HerdrError::InputBlocked { command, message } => HerdrError::CommandFailed { command, message },
            error => error,
        })?;
        value
            .pointer("/result/agent")
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "agent prompt",
                detail: "响应缺少 result.agent".to_string(),
            })?;
        Ok(())
    }

    async fn prompt_once(&self, target: &str, text: &str) -> Result<Value, HerdrError> {
        self.run(
            "agent prompt",
            &["agent".into(), "prompt".into(), target.into(), text.into()],
            PROMPT_TIMEOUT,
        ).await
    }

    /// Verify the selected historical identity without querying either host.
    pub(crate) async fn selected_native_id<'a>(&self, kind: &str, selected: &'a crate::chats::history::HistorySelection) -> Result<std::borrow::Cow<'a, str>, HerdrError> {
        let session_id = selected.session_id.as_str();
        if matches!(kind, "omp" | "pi") && let Some(path) = selected.startup_identity_path.as_ref() {
            let path = PathBuf::from(path);
            let pending = saved_pending_native_session(selected);
            let deadline = tokio::time::Instant::now() + QUERY_TIMEOUT;
            let allow_title = kind == "omp";
            let identity = tokio::task::spawn_blocking(move || native_header_or_pending(path, allow_title, deadline, pending)).await
                .map_err(|error| native_error(&format!("native identity reader failed: {error}")))??;
            if identity.session_id != session_id {
                return Err(native_error("saved startup path belongs to another native session"));
            }
        }
        if Path::new(session_id).is_absolute() {
            Ok(std::borrow::Cow::Owned(read_native_id(session_id, kind).await?))
        } else { Ok(std::borrow::Cow::Borrowed(session_id)) }
    }

    /// Query the live TUI inventory on every attempt; names, cwd and
    /// registration liveness are never native conversation identities.
    pub(crate) async fn matching_targets(&self, kind: &str, selected: &crate::chats::history::HistorySelection) -> Result<Vec<NavigationTarget>, HerdrError> {
        let value = self.run("agent list", &["agent".into(), "list".into()], QUERY_TIMEOUT).await?;
        let entries = value.pointer("/result/agents").and_then(Value::as_array)
            .ok_or_else(|| native_error("agent list response has no result.agents array"))?;
        let id = self.selected_native_id(kind, selected).await?;
        let mut targets = Vec::new();
        for entry in entries {
            if entry.get("agent").and_then(Value::as_str) != Some(kind) { continue; }
            let Some(candidate) = candidate_native_id(entry, kind, selected).await? else { continue; };
            if candidate == id { targets.push(NavigationTarget::from_agent(entry)?); }
        }
        // Opaque public IDs, lexicographically ordered; independent of enumeration
        // and of display labels. Include all three location components.
        targets.sort();
        targets.dedup();
        Ok(targets)
    }

    pub(crate) async fn focus_target(&self, target: &NavigationTarget) -> Result<(), HerdrError> {
        let value = self.run("agent focus", &["agent".into(), "focus".into(), target.pane_id.clone()], QUERY_TIMEOUT).await?;
        if value.pointer("/result/type").and_then(Value::as_str) != Some("agent_info") {
            return Err(HerdrError::UnexpectedResponse { command: "agent focus", detail: "missing agent_info focus confirmation".into() });
        }
        let agent = value.pointer("/result/agent").ok_or_else(|| HerdrError::UnexpectedResponse {
            command: "agent focus", detail: "focus confirmation has no agent".into()
        })?;
        if NavigationTarget::from_agent(agent)? != *target || agent.get("focused").and_then(Value::as_bool) != Some(true) {
            return Err(HerdrError::UnexpectedResponse { command: "agent focus", detail: "target navigation was not confirmed".into() });
        }
        Ok(())
    }

    pub(crate) async fn focus_app_target(&self, target: &LaunchTarget) -> Result<Value, HerdrError> {
        match (&self.host, &target.control) {
            (Host::App(app), LaunchControl::App { terminal, .. }) => app.focus_bound_pane(&target.location, terminal).await,
            _ => Err(HerdrError::UnexpectedResponse { command: "App navigation", detail: "目标没有当前 App/core 的终端关联".into() }),
        }
    }

    pub(crate) async fn close_app_target(&self, target: &LaunchTarget) -> Result<(), HerdrError> {
        match (&self.host, &target.control) {
            (Host::App(app), LaunchControl::App { terminal }) => app.close_tab(&target.location, terminal).await,
            _ => Err(HerdrError::UnexpectedResponse { command: "App close", detail: "目标没有当前 App/core 的终端关联".into() }),
        }
    }

    /// TUI handles an empty Workspace. Never ask it to close a known worktree
    /// group, even on installations with the native confirmation disabled.
    pub(crate) async fn close_tui_tab(&self, workspace_id: &str, tab_id: &str) -> Result<(), HerdrError> {
        let inventory = self.workspace_inventory().await?;
        let mut owners = inventory.records.iter().filter(|row| row["workspace_id"].as_str() == Some(workspace_id));
        let owner = owners.next().filter(|_| owners.next().is_none())
            .ok_or_else(|| inventory.error("无法确认目标 Tab 保存的 Workspace"))?;
        if let Some(worktree) = owner.get("worktree").filter(|value| !value.is_null()) {
            let linked = worktree["is_linked_worktree"].as_bool().ok_or_else(|| inventory.error("缺少 worktree 类型"))?;
            if !linked && owner["tab_count"].as_u64().is_none_or(|count| count <= 1) {
                let repo = nonempty(worktree.get("repo_key")).ok_or_else(|| inventory.error("缺少 worktree 组身份"))?;
                if inventory.records.iter().any(|row| row["workspace_id"] != workspace_id
                    && nonempty(row.pointer("/worktree/repo_key")) == Some(repo))
                {
                    return Err(HerdrError::UnsafeCloseScope);
                }
            }
        }
        let value = self.run("tab close", &["tab".into(), "close".into(), tab_id.into()], QUERY_TIMEOUT).await?;
        if value.pointer("/result/type").and_then(Value::as_str) != Some("ok") {
            return Err(HerdrError::UnexpectedResponse { command: "tab close", detail: "missing ok confirmation".into() });
        }
        Ok(())
    }

    /// 执行一次 herdr CLI 调用；参数逐个传递，绝不经过 Shell。
    pub(super) async fn run(
        &self,
        command_name: &'static str,
        args: &[String],
        timeout: Duration,
    ) -> Result<Value, HerdrError> {
        let Host::Tui(executable) = &self.host else {
            return Err(HerdrError::CommandFailed {
                command: command_name,
                message: "此操作需要 TUI 控制协议；不会回退到 TUI。".into(),
            });
        };
        run_cli(executable, command_name, args, timeout).await
    }
}

/// Run one selected executable and decode its JSON response without a shell.
pub(super) async fn run_cli(
    executable: &Path,
    command_name: &'static str,
    args: &[impl AsRef<std::ffi::OsStr>],
    timeout: Duration,
) -> Result<Value, HerdrError> {
        run_cli_command(Command::new(executable), executable, command_name, args, timeout).await
    }

    pub(super) async fn run_bound_core_cli(
        executable: &Path, endpoint: &str, command_name: &'static str, args: &[&str], timeout: Duration,
    ) -> Result<Value, HerdrError> {
        // hc connects first. If the listener disappeared, its implicit startup
        // must fail at state directory creation, even on platforms where status
        // itself can start a core. A regular file cannot contain that directory.
        #[cfg(windows)]
        let no_start = std::env::current_exe().map_err(|error| HerdrError::NotStarted(format!("无法禁止 hc 自动启动：{error}")))?;
        #[cfg(unix)]
        let no_start = Path::new("/dev/null");
        let mut command = Command::new(executable);
        command.env("HC_SOCKET", endpoint)
            .env("XDG_STATE_HOME", no_start)
            .env_remove("HC_SESSION")
            .env_remove("HC_TERMINAL_ID")
            .env_remove("HC_TERMINAL_SOCKET");
        run_cli_command(command, executable, command_name, args, timeout).await
    }

    async fn run_cli_command(
        mut command: Command, executable: &Path, command_name: &'static str,
        args: &[impl AsRef<std::ffi::OsStr>], timeout: Duration,
    ) -> Result<Value, HerdrError> {
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let child = command.spawn().map_err(|error| {
            HerdrError::NotStarted(format!(
                "无法启动 {}（{}）：{error}",
                executable.display(),
                command_name
            ))
        })?;

        let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Err(_) => return Err(HerdrError::Timeout(command_name)),
            Ok(Err(error)) => {
                return Err(HerdrError::Unavailable(format!(
                    "{command_name} 的进程无法观察结束：{error}"
                )));
            }
            Ok(Ok(output)) => output,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            for stream in [stderr.trim(), stdout.trim()] {
                if let Ok(value) = serde_json::from_str::<Value>(stream) {
                    if let Some(error) = value.get("error") {
                        return Err(response_error(command_name, error));
                    }
                    if value.get("code").is_some() {
                        return Err(response_error(command_name, &value));
                    }
                }
            }
            let detail = stderr.trim().to_string();
            let detail = if detail.is_empty() {
                format!("进程退出状态为 {}", output.status)
            } else {
                detail
            };
            return Err(HerdrError::CommandFailed {
                command: command_name,
                message: detail,
            });
        }

        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Err(HerdrError::UnexpectedResponse {
                command: command_name,
                detail: "stdout 没有 JSON".to_string(),
            });
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|error| HerdrError::UnexpectedResponse {
            command: command_name,
            detail: format!("stdout 不是单个 JSON 响应：{error}"),
        })?;
        if let Some(error) = value.get("error") {
            return Err(response_error(command_name, error));
        }
        Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationTarget {
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
}
impl NavigationTarget {
    pub(super) fn from_agent(agent: &Value) -> Result<Self, HerdrError> {
        let field = |name| nonempty(agent.get(name)).map(str::to_owned)
            .ok_or_else(|| native_error("matched agent has incomplete navigation location"));
        Ok(Self { workspace_id: field("workspace_id")?, tab_id: field("tab_id")?, pane_id: field("pane_id")? })
    }
}

#[derive(Debug)]
enum LaunchControl {
    Tui { alias: String },
    App { terminal: super::app::AppTerminal },
}

#[derive(Debug)]
pub(crate) struct LaunchTarget {
    pub(crate) location: NavigationTarget,
    control: LaunchControl,
}

pub(super) enum LaunchVerificationError {
    Ended,
    Unproven(HerdrError),
    OfficialIdentity(HerdrError),
    ReportedIdentity(HerdrError),
}

impl From<HerdrError> for LaunchVerificationError {
    fn from(error: HerdrError) -> Self { Self::Unproven(error) }
}

impl From<LaunchVerificationError> for HerdrError {
    fn from(error: LaunchVerificationError) -> Self {
        match error {
            LaunchVerificationError::Ended => HerdrError::CommandFailed { command: "hc get", message: "启动终端已结束".into() },
            LaunchVerificationError::Unproven(error) | LaunchVerificationError::OfficialIdentity(error)
            | LaunchVerificationError::ReportedIdentity(error) => error,
        }
    }
}

impl LaunchTarget {
    pub(super) fn is_app(&self) -> bool { matches!(self.control, LaunchControl::App { .. }) }

    pub(crate) async fn start_agent(&self, herdr: &HerdrClient, kind: &str, args: &[String]) -> Result<(), HerdrError> {
        if let LaunchControl::Tui { alias } = &self.control {
            herdr.start_agent(alias, kind, &self.location.pane_id, args).await?;
        }
        Ok(())
    }

    pub(crate) async fn submit(&self, herdr: &HerdrClient, text: &str) -> Result<(), HerdrError> {
        if let (LaunchControl::App { terminal }, Host::App(app)) = (&self.control, &herdr.host) {
            app.observe_launch(&self.location, terminal).await?;
        }
        self.submit_observed(herdr, text).await
    }

    /// Initial input calls this after verifying the launch owner and host.
    pub(super) async fn submit_observed(&self, herdr: &HerdrClient, text: &str) -> Result<(), HerdrError> {
        match (&self.control, &herdr.host) {
            (LaunchControl::Tui { alias }, Host::Tui(_)) => {
                let value = herdr.prompt_once(alias, text).await?;
                if value.pointer("/result/type").and_then(Value::as_str) != Some("agent_prompted")
                    || value.pointer("/result/agent/name").and_then(Value::as_str) != Some(alias)
                {
                    return Err(HerdrError::UnexpectedResponse {
                        command: "agent prompt",
                        detail: "响应没有确认 result.agent 对应本次启动目标".into(),
                    });
                }
                Ok(())
            }
            (LaunchControl::App { terminal, .. }, Host::App(app)) => app.submit(terminal, text).await,
            _ => Err(native_error("launch target host changed")),
        }
    }

    pub(super) async fn verify(&self, herdr: &HerdrClient, kind: &str, cwd: &str, pid: u32, id: &str, file: Option<&str>) -> Result<super::NativeSession, LaunchVerificationError> {
        let valid_cwd = |path: &str| crate::zones::same_path(path, cwd);
        match (&self.control, &herdr.host) {
            (LaunchControl::Tui { alias }, Host::Tui(_)) => {
                let observation = herdr.run("agent get", &["agent".into(), "get".into(), alias.clone()], QUERY_TIMEOUT).await?;
                let agent = observation.pointer("/result/agent").ok_or_else(|| native_error("missing launched agent"))?;
                if NavigationTarget::from_agent(agent)? != self.location || nonempty(agent.get("agent")) != Some(kind) {
                    return Err(native_error("TUI launch has another target or agent").into());
                }
                let process = herdr.run("pane process-info", &["pane".into(), "process-info".into(), "--pane".into(), self.location.pane_id.clone()], QUERY_TIMEOUT).await?;
                let pane = process.pointer("/result/process_info").ok_or_else(|| native_error("missing pane process information"))?;
                if nonempty(pane.get("pane_id")) != Some(self.location.pane_id.as_str())
                    || !pane.get("foreground_processes").and_then(Value::as_array).is_some_and(|processes| processes.iter().any(|p| {
                        p.get("pid").and_then(Value::as_u64) == Some(u64::from(pid))
                            && nonempty(p.get("cwd")).or_else(|| nonempty(agent.get("foreground_cwd"))).is_some_and(valid_cwd)
                    }))
                {
                    return Err(native_error("reported owner is not the current TUI pane process").into());
                }
                if agent.get("agent_session").is_some_and(|value| !value.is_null()) {
                    // The pane/process is independently matched. A bad official
                    // reference is a host failure, not just an unusable report.
                    let official = super::identity::native_from_agent(agent, kind, tokio::time::Instant::now() + QUERY_TIMEOUT, true, true)
                        .await.map_err(LaunchVerificationError::OfficialIdentity)?;
                    if !valid_cwd(&official.cwd) {
                        return Err(LaunchVerificationError::OfficialIdentity(native_error("official TUI identity cwd conflicts with launch directory")));
                    }
                    if official.session_id != id
                        || matches!((official.session_file.as_deref(), file), (Some(a), Some(b)) if !crate::zones::same_path(a,b))
                    {
                        return Err(native_error("official TUI identity conflicts with launch report").into());
                    }
                    if official.session_file.is_some() || file.is_none() { return Ok(official); }
                }
            }
            (LaunchControl::App { terminal }, Host::App(app)) => {
                let bootstrap_pid = terminal.reported_bootstrap_pid().ok_or_else(|| native_error("App launch did not request native reports"))?;
                let record = app.observe_owner(&self.location, terminal).await?
                    .ok_or(LaunchVerificationError::Ended)?;
                if !nonempty(record.get("cwd")).is_some_and(valid_cwd) {
                    return Err(native_error("App terminal cwd conflicts with launch report").into());
                }
                if bootstrap_pid != pid && !tokio::task::spawn_blocking(move || super::launch::process_is_descendant(bootstrap_pid, pid)).await.unwrap_or(false) {
                    return Err(native_error("reported owner is not a process of this App launch").into());
                }
            }
            _ => return Err(native_error("launch target host changed").into()),
        }
        super::launch::reported_native_session(kind, cwd, id, file).await.map_err(LaunchVerificationError::ReportedIdentity)
    }
}

pub(super) fn nonempty(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|text| !text.trim().is_empty())
}

/// Herdr 自身的就绪等待也可超时；CLI 已退出不代表外部启动未发生。
/// 只按结构化 code 分类，不把错误描述中偶然出现的 timeout 当作超时。
fn response_error(command: &'static str, error: &Value) -> HerdrError {
    if error.get("code").and_then(Value::as_str) == Some("timeout") {
        return HerdrError::Timeout(command);
    }
    let code = error.get("code").and_then(Value::as_str);
    let kind = error.get("kind").and_then(Value::as_str);
    let detail = error.as_str().or_else(|| error.get("message").and_then(Value::as_str));
    let message = match (code.or(kind), detail) {
        (Some(category), Some(detail)) => format!("{category}: {detail}"),
        (Some(detail), None) | (None, Some(detail)) => detail.to_owned(),
        (None, None) => "Herdr returned an error response".to_owned(),
    };
    // Only these documented host errors guarantee a pre-write block. In
    // particular, agent_prompt_failed can follow partial terminal input.
    match (command, code, kind) {
        ("agent prompt", Some("agent_blocked"), None) | ("hc send", None, Some("blocked")) => {
            HerdrError::InputBlocked { command, message }
        }
        _ => HerdrError::CommandFailed { command, message },
    }
}
