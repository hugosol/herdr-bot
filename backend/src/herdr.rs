//! Herdr 公开控制接口的生产 adapter：通过 `herdr` CLI 的新行分隔 JSON 输出执行控制命令。
//!
//! 这里刻意**不经过 Shell**：可执行文件与每个参数都作为独立进程参数提交，因此 Chat 名称、zone
//! 目录、Bot prompt 与用户 input 中的引号、空格、换行或 Shell 元字符只作为数据到达 Herdr，
//! 不会被解释成命令。选择 CLI 而不是直接连 socket 只是一个 adapter 选择；对上层而言，它只暴露
//! Workspace 查询、Tab 创建、agent 启动/就绪、原生身份查找、已有目标聚焦与初始输入提交。
//!
//! 所有命令都是一次调用：失败、超时或无法解析都不自动重试，也不回退到其他 Workspace。成功响应
//! 只表示 Herdr 接受了那一步；`agent.start` 的 CLI 语义是目标 agent 已可交互，`agent.prompt`
//! 只表示提交被接受，不表示模型完成任务。

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::config::HerdrConfig;

/// 查询命令的时限：Workspace 列表必须很快返回，否则按 Herdr 不可达处理。
const QUERY_TIMEOUT: Duration = Duration::from_secs(15);
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

/// 新建 Tab 后由 Herdr 返回的对象标识；两个标识都取自响应，不按名称或序号猜测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedTab {
    pub tab_id: String,
    pub pane_id: String,
}

/// Identity supplied by the official integration for the exact queried agent.
#[derive(Debug)]
pub struct NativeSession {
    pub session_id: String,
    pub cwd: String,
    pub session_file: Option<String>,
}

/// Herdr 控制失败：每个类别都说明失败发生在哪个控制步骤，不把未知说成确定。
#[derive(Debug)]
pub enum HerdrError {
    /// 无法启动 herdr CLI 或无法与运行中的 Herdr 通信。
    Unavailable(String),
    /// 命令在时限内没有返回；不能据此断言操作没有发生。
    Timeout(&'static str),
    /// 配置名称没有匹配到任何 Workspace。
    WorkspaceNotFound { name: String },
    /// 配置名称匹配到多个 Workspace，不能任取一个。
    WorkspaceAmbiguous { name: String, matches: usize },
    /// Herdr 明确返回了失败。
    CommandFailed {
        command: &'static str,
        message: String,
    },
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
            Self::Unavailable(_) => "herdr-unavailable",
            Self::Timeout(_) => "herdr-timeout",
            Self::WorkspaceNotFound { .. } => "workspace-not-found",
            Self::WorkspaceAmbiguous { .. } => "workspace-ambiguous",
            Self::CommandFailed { .. } => "herdr-command-failed",
            Self::UnexpectedResponse { .. } => "herdr-unexpected-response",
        }
    }

    /// 面向使用者的说明。
    pub fn message(&self) -> String {
        match self {
            Self::Unavailable(reason) => {
                format!("无法运行或连接 Herdr：{reason}。本次没有继续创建。")
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
            Self::CommandFailed { command, message } => {
                format!("Herdr 的 {command} 失败：{message}")
            }
            Self::UnexpectedResponse { command, detail } => {
                format!("Herdr 的 {command} 返回了无法理解的响应：{detail}")
            }
        }
    }
}

/// 生产用的 Herdr 控制 adapter。
#[derive(Debug, Clone)]
pub struct HerdrClient {
    executable: PathBuf,
    workspace_name: String,
}

impl HerdrClient {
    pub fn new(config: &HerdrConfig) -> Self {
        Self {
            executable: config.executable.clone(),
            workspace_name: config.workspace_name.clone(),
        }
    }

    /// 目标 Workspace 名称（启动日志与诊断使用）。
    pub fn workspace_name(&self) -> &str {
        &self.workspace_name
    }

    /// 按本次目标名称精确查找唯一 Workspace；焦点与顺序都不参与选择。
    pub async fn workspace(&self, name: &str) -> Result<Workspace, HerdrError> {
        let value = self
            .run(
                "workspace list",
                &["workspace".into(), "list".into()],
                QUERY_TIMEOUT,
            )
            .await?;
        let entries = value
            .pointer("/result/workspaces")
            .and_then(Value::as_array)
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "workspace list",
                detail: "响应缺少 result.workspaces 数组".to_string(),
            })?;

        let mut matches = Vec::new();
        for entry in entries {
            let label = entry.get("label").and_then(Value::as_str);
            let id = entry.get("workspace_id").and_then(Value::as_str);
            if let (Some(label), Some(id)) = (label, id)
                && label == name
            {
                matches.push(Workspace {
                    id: id.to_string(),
                    label: label.to_string(),
                });
            }
        }

        match matches.len() {
            0 => Err(HerdrError::WorkspaceNotFound {
                name: name.to_string(),
            }),
            1 => Ok(matches.remove(0)),
            count => Err(HerdrError::WorkspaceAmbiguous {
                name: name.to_string(),
                matches: count,
            }),
        }
    }

    /// 在指定 Workspace 中新建 Tab，并把 cwd 显式设为 zone 目录。
    ///
    /// 不使用当前焦点 Workspace；`--no-focus` 避免创建动作改变用户在 Herdr 中的焦点。
    pub async fn create_tab(
        &self,
        workspace_id: &str,
        cwd: &str,
        label: &str,
    ) -> Result<CreatedTab, HerdrError> {
        let value = self
            .run(
                "tab create",
                &[
                    "tab".into(),
                    "create".into(),
                    "--workspace".into(),
                    workspace_id.into(),
                    "--cwd".into(),
                    cwd.into(),
                    "--label".into(),
                    label.into(),
                    "--no-focus".into(),
                ],
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
        let value = self
            .run(
                "agent prompt",
                &["agent".into(), "prompt".into(), target.into(), text.into()],
                PROMPT_TIMEOUT,
            )
            .await?;
        value
            .pointer("/result/agent")
            .ok_or_else(|| HerdrError::UnexpectedResponse {
                command: "agent prompt",
                detail: "响应缺少 result.agent".to_string(),
            })?;
        Ok(())
    }

    /// One read-only identity attempt. The caller owns the overall acquisition
    /// deadline; this method never scans directories or invokes agent commands.
    pub async fn native_session(&self, target: &str, kind: &str, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
        if tokio::time::Instant::now() >= deadline { return Err(native_error("native identity acquisition deadline expired")); }
        let value = self.run("agent get", &["agent".into(), "get".into(), target.into()], QUERY_TIMEOUT).await?;
        if tokio::time::Instant::now() >= deadline { return Err(native_error("native identity acquisition deadline expired")); }
        let agent = value.pointer("/result/agent").ok_or_else(|| native_error("missing result.agent"))?;
        let reference = agent.get("agent_session").filter(|value| !value.is_null())
            .ok_or_else(|| native_error("native identity is not available"))?;
        if !matches!(kind, "pi" | "omp")
            || reference.get("agent").and_then(Value::as_str) != Some(kind)
            || reference.get("source").and_then(Value::as_str) != Some(if kind == "omp" { "herdr:omp" } else { "herdr:pi" })
        {
            return Err(native_error("native reference agent/source does not match this creation"));
        }
        let reference_value = nonempty(reference.get("value"))
            .ok_or_else(|| native_error("native reference has no value"))?;
        match reference.get("kind").and_then(Value::as_str) {
            Some("id") => {
                let cwd = nonempty(agent.get("foreground_cwd")).or_else(|| nonempty(agent.get("cwd")))
                    .ok_or_else(|| native_error("native ID has no reported cwd"))?;
                Ok(NativeSession { session_id: reference_value.to_owned(), cwd: cwd.to_owned(), session_file: None })
            }
            Some("path") => {
                let path = PathBuf::from(reference_value);
                if !path.is_absolute() { return Err(native_error("native reference path is not absolute")); }
                let allow_title_metadata = kind == "omp";
                tokio::task::spawn_blocking(move || native_header(path, allow_title_metadata, deadline)).await
                    .map_err(|error| native_error(&format!("native header reader failed: {error}")))?
            }
            _ => Err(native_error("unsupported native reference kind")),
        }
    }

    /// Query the live inventory on every attempt; names, cwd and registration
    /// liveness are never native conversation identities.
    pub(crate) async fn matching_targets(&self, kind: &str, session_id: &str) -> Result<Vec<NavigationTarget>, HerdrError> {
        let value = self.run("agent list", &["agent".into(), "list".into()], QUERY_TIMEOUT).await?;
        let entries = value.pointer("/result/agents").and_then(Value::as_array)
            .ok_or_else(|| native_error("agent list response has no result.agents array"))?;
        let id = if std::path::Path::new(session_id).is_absolute() {
            std::borrow::Cow::Owned(read_native_id(session_id, kind).await?)
        } else { std::borrow::Cow::Borrowed(session_id) };
        let mut targets = Vec::new();
        for entry in entries {
            if entry.get("agent").and_then(Value::as_str) != Some(kind) { continue; }
            let reference = entry.get("agent_session").filter(|reference| !reference.is_null())
                .ok_or_else(|| native_error("matching agent kind has no verifiable native identity"))?;
            if reference.get("agent").and_then(Value::as_str) != Some(kind)
                || reference.get("source").and_then(Value::as_str) != Some(if kind == "omp" { "herdr:omp" } else { "herdr:pi" }) {
                return Err(native_error("native identity agent/source is inconsistent"));
            }
            let value = nonempty(reference.get("value")).ok_or_else(|| native_error("native reference has no value"))?;
            let candidate = match reference.get("kind").and_then(Value::as_str) {
                Some("id") => std::borrow::Cow::Borrowed(value),
                Some("path") => std::borrow::Cow::Owned(read_native_id(value, kind).await?),
                _ => return Err(native_error("unsupported native reference kind")),
            };
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

    /// 执行一次 herdr CLI 调用；参数逐个传递，绝不经过 Shell。
    async fn run(
        &self,
        command_name: &'static str,
        args: &[String],
        timeout: Duration,
    ) -> Result<Value, HerdrError> {
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let child = command.spawn().map_err(|error| {
            HerdrError::Unavailable(format!(
                "无法启动 {}（{}）：{error}",
                self.executable.display(),
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
            let detail = error_message_from_stderr(&stderr)
                .unwrap_or_else(|| stderr.trim().to_string())
                .trim()
                .to_string();
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
            return Err(HerdrError::CommandFailed { command: command_name,
                message: error.get("message").and_then(Value::as_str).unwrap_or("Herdr returned an error response").to_owned() });
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationTarget {
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
}
impl NavigationTarget {
    fn from_agent(agent: &Value) -> Result<Self, HerdrError> {
        let field = |name| nonempty(agent.get(name)).map(str::to_owned)
            .ok_or_else(|| native_error("matched agent has incomplete navigation location"));
        Ok(Self { workspace_id: field("workspace_id")?, tab_id: field("tab_id")?, pane_id: field("pane_id")? })
    }
}

async fn read_native_id(path: &str, kind: &str) -> Result<String, HerdrError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() { return Err(native_error("native reference path is not absolute")); }
    let allow_title = kind == "omp";
    let deadline = tokio::time::Instant::now() + QUERY_TIMEOUT;
    tokio::task::spawn_blocking(move || native_header(path, allow_title, deadline)).await
        .map_err(|error| native_error(&format!("native identity reader failed: {error}")))?
        .map(|identity| identity.session_id)
}

fn native_error(detail: &str) -> HerdrError {
    HerdrError::UnexpectedResponse { command: "agent get native identity", detail: detail.to_owned() }
}

fn nonempty(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|text| !text.trim().is_empty())
}

/// Read only the metadata prefix of this exact file. OMP reserves a title line
/// before the session header; Pi starts with the session header. Neither agent
/// is inferred from the JSON shape, and conversation entries are never imported.
fn native_header(path: PathBuf, allow_title_metadata: bool, deadline: tokio::time::Instant) -> Result<NativeSession, HerdrError> {
    use std::io::Read;
    if tokio::time::Instant::now() >= deadline {
        return Err(native_error("native identity acquisition deadline expired"));
    }
    let mut file = std::fs::File::open(&path)
        .map_err(|error| native_error(&format!("native header unavailable: {error}")))?;
    let mut line = Vec::with_capacity(512);
    let mut remaining = 64 * 1024;
    loop {
        line.clear();
        loop {
            // In-flight kernel I/O is not forcibly cancellable, but a completed
            // operation must not initiate further reads after the total budget.
            if tokio::time::Instant::now() >= deadline {
                return Err(native_error("native identity acquisition deadline expired"));
            }
            if remaining == 0 { return Err(native_error("native metadata prefix exceeds 64 KiB")); }
            remaining -= 1;
            // No read-ahead into conversation content; only identity metadata is needed.
            let mut byte = [0];
            if file.read(&mut byte).map_err(|error| native_error(&format!("native header read failed: {error}")))? == 0 {
                return Err(native_error("native header is not complete yet"));
            }
            if byte[0] == b'\n' { break; }
            line.push(byte[0]);
        }
        if line.iter().all(u8::is_ascii_whitespace) { continue; }
        let metadata: Value = serde_json::from_slice(&line)
            .map_err(|error| native_error(&format!("native metadata is not valid JSON: {error}")))?;
        match metadata.get("type").and_then(Value::as_str) {
            Some("title") if allow_title_metadata => continue,
            Some("session") => {
                let id = nonempty(metadata.get("id")).ok_or_else(|| native_error("native header has no ID"))?;
                let cwd = nonempty(metadata.get("cwd")).ok_or_else(|| native_error("native header has no cwd"))?;
                return Ok(NativeSession { session_id: id.to_owned(), cwd: cwd.to_owned(), session_file: Some(path.to_string_lossy().into_owned()) });
            }
            _ => return Err(native_error("expected native session header, not conversation content")),
        }
    }
}

/// Herdr CLI 的错误 JSON 在 stderr；取出其中的 message，取不到时交给调用方使用原始文本。
fn error_message_from_stderr(stderr: &str) -> Option<String> {
    let value: Value = serde_json::from_str(stderr.trim()).ok()?;
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .pointer("/error/code")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}
