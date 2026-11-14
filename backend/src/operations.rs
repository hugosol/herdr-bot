//! 固定脚本操作的执行：启动配置的执行目标并等待它结束。
//!
//! 记录执行目标能否启动及如何结束，并据此给出固定操作结果。接受后的独立任务持有执行资格，
//! 不随 HTTP 连接断开取消；不推断后台 Python 进程的生命周期——脚本另开的长期进程不属于本次调用。

use std::fmt;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use tokio::process::Command;

/// 由服务端配置绑定的一个固定操作。
#[derive(Debug, Clone)]
pub struct ScriptOperation {
    script: PathBuf,
    working_directory: PathBuf,
}

impl ScriptOperation {
    pub fn new(script: PathBuf, working_directory: PathBuf) -> Self {
        Self {
            script,
            working_directory,
        }
    }

    pub fn script(&self) -> &Path {
        &self.script
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    /// 启动执行目标并等待它结束，返回本次调用的原始观测结果。
    ///
    /// “目标能否启动”必须先于调用判断：目标不存在、目标不可执行、工作目录不存在时，脚本进程
    /// 连创建都做不到，而这类情形**无法**从退出码上看出来（`cmd.exe` 找不到批处理时自己以退出码
    /// 1 结束，与批处理自身 `exit /b 1` 无法区分）。
    pub async fn invoke(&self) -> Invocation {
        if !self.working_directory.is_dir() {
            return Invocation::NotStarted(NotStartedReason::WorkingDirectoryMissing);
        }
        if !self.script.is_file() {
            return Invocation::NotStarted(NotStartedReason::ScriptMissing);
        }

        let mut command = self.launch_command();
        command
            .current_dir(&self.working_directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if console_available() {
            // 面板连着控制台（用户手动启动的常态）：让脚本附着并继承它，等价于用户手动运行脚本，
            // 脚本里需要控制台输入的命令（例如现有脚本末尾的 `timeout /t N /nobreak`）才会按设计结束。
            // 子进程附着在面板已有的控制台上，因此不会额外弹出窗口。
            command.stdin(Stdio::inherit());
        } else {
            // 面板没有控制台：保持非交互、不弹窗；脚本中依赖控制台输入的命令会以非交互方式失败，
            // 操作结果如实显示为该退出码。
            command.stdin(Stdio::null());
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            // 工作目录与目标文件都在，Windows 仍可能拒绝把它创建成进程
            // （例如目标不是可执行映像）。
            Err(error) => {
                return Invocation::NotStarted(NotStartedReason::ProcessCreationFailed(error));
            }
        };

        match child.wait().await {
            Ok(status) => Invocation::Ended(status),
            Err(error) => Invocation::EndUnobserved(error),
        }
    }

    /// 用哪个命令启动执行目标。
    ///
    /// 批处理由 `cmd.exe` 解释执行（`.bat` 不能直接作为可执行映像启动），工作目录固定为绑定目录，
    /// 避免脚本内的相对路径改变语义。其他目标直接作为可执行映像启动：这样“无法作为映像加载”
    /// 由 Windows 在创建进程时报告，而不是被 `cmd.exe` 折进一个与脚本自身失败无法区分的退出码。
    fn launch_command(&self) -> Command {
        if is_batch(&self.script) {
            // 始终经 cmd.exe 调用批处理。
            let mut command = Command::new("cmd.exe");
            command.arg("/C").arg(&self.script);
            command
        } else {
            Command::new(&self.script)
        }
    }
}

/// 目标是否是批处理：`.bat` / `.cmd` 由 `cmd.exe` 解释执行。
fn is_batch(script: &Path) -> bool {
    script
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("bat") || extension.eq_ignore_ascii_case("cmd")
        })
}

/// 面板进程是否连着控制台（标准输入是控制台）。
///
/// 现有启动/停止脚本末尾都有 `timeout /t N /nobreak`：该命令需要真实控制台作为标准输入，输入被重定向时
/// 会立即失败（“Input redirection is not supported”）并让脚本以退出码 1 结束——即便脚本主体
/// （例如 `start` 打开窗口启动 Python 程序）已经生效。面板由用户手动启动时本就有控制台；此时把控制台
/// 继承给子进程，脚本才会像用户手动运行那样结束。面板没有控制台时保持非交互，结果如实反映该环境下的退出码。
pub fn console_available() -> bool {
    std::io::stdin().is_terminal()
}

/// 一次调用中执行目标进程的观测结果：只有事实，不含成功或失败的判断。
#[derive(Debug)]
pub enum Invocation {
    /// 执行目标没能启动：本次调用没有创建任何脚本进程。
    NotStarted(NotStartedReason),
    /// 脚本进程已创建并已结束。
    Ended(ExitStatus),
    /// 脚本进程已创建，但它的结束结果无法观测（等待失败）。
    EndUnobserved(io::Error),
}

/// 执行目标没能启动的原因；只用于诊断输出，不进入 HTTP 结果类别。
#[derive(Debug)]
pub enum NotStartedReason {
    /// 配置的工作目录不存在或不是目录。
    WorkingDirectoryMissing,
    /// 配置的目标既不存在，也不是文件。
    ScriptMissing,
    /// 目标存在，但 Windows 拒绝把它创建成进程（例如不是可执行映像）。
    ProcessCreationFailed(io::Error),
}

impl fmt::Display for NotStartedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkingDirectoryMissing => write!(formatter, "配置的工作目录不存在"),
            Self::ScriptMissing => write!(formatter, "配置的目标文件不存在"),
            Self::ProcessCreationFailed(error) => write!(formatter, "创建进程失败：{error}"),
        }
    }
}

/// `CREATE_NO_WINDOW`：以无控制台窗口方式启动子进程。
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use std::sync::Arc;
use serde::Serialize;
use tokio::sync::{Mutex, oneshot};
/// 一次已结束的固定操作调用的结果：本次调用已结束的事实、脚本进程的原始观测结果，
/// 以及由它得出的结果类别。
///
/// 结果类别只看脚本进程本身能否启动及其退出结果，不看脚本打印了什么，也不推断后台 Python 进程的
/// 状态：脚本结束不等于后台服务已就绪。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationResponse {
    /// 本次调用的固定操作名：`start` 或 `stop`。
    operation: &'static str,
    /// 本次调用是否已经结束（HTTP 响应返回时必然为 true）。
    ended: bool,
    /// 脚本进程是否成功创建。
    started: bool,
    /// 脚本进程退出码；未创建或非正常退出结束时为 null。
    exit_code: Option<i32>,
    /// 结果类别：脚本成功退出为 `completed`，无法启动或失败退出为 `failed`。
    outcome: OperationOutcome,
    /// 失败的子类别；仅当 `outcome` 为 `failed` 时出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<FailureKind>,
}

/// 一次调用的结果类别。
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum OperationOutcome {
    /// 脚本以成功退出码结束：本次脚本执行完成。
    Completed,
    /// 脚本无法启动，或运行后没有以成功退出码结束。
    Failed,
}

/// 失败结果的子类别；“无法启动”与“运行后失败退出”因此可互相区分。
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum FailureKind {
    /// 脚本无法启动：目标不存在、目标不可执行或工作目录不存在。
    NotStarted,
    /// 脚本进程已创建，但没有以成功退出码结束（非零退出码、没有退出码，或结束结果无法观测）。
    FailedExit,
}

/// 把一次调用的原始观测结果转成结果类别，并写一条诊断输出。
fn operation_response(
    name: &'static str,
    operation: &ScriptOperation,
    invocation: Invocation,
) -> OperationResponse {
    match invocation {
        // 脚本进程已创建并结束：只有成功退出码算“执行完成”，其余都是“执行失败”。
        Invocation::Ended(status) => {
            eprintln!(
                "[herdr-bot] {name} 调用已结束：{}，{status}",
                operation.script().display()
            );
            let exit_code = status.code();
            let succeeded = exit_code == Some(0);
            OperationResponse {
                operation: name,
                ended: true,
                started: true,
                exit_code,
                outcome: if succeeded {
                    OperationOutcome::Completed
                } else {
                    OperationOutcome::Failed
                },
                failure: (!succeeded).then_some(FailureKind::FailedExit),
            }
        }
        // 进程已创建，但结束结果无法观测：不能当作成功退出。
        Invocation::EndUnobserved(error) => {
            eprintln!(
                "[herdr-bot] {name} 调用已结束：无法观测脚本进程的结束结果（{}）：{error}",
                operation.script().display()
            );
            OperationResponse {
                operation: name,
                ended: true,
                started: true,
                exit_code: None,
                outcome: OperationOutcome::Failed,
                failure: Some(FailureKind::FailedExit),
            }
        }
        // 目标没能启动：没有脚本进程，属于“执行失败”里的“无法启动”。
        Invocation::NotStarted(reason) => {
            eprintln!(
                "[herdr-bot] {name} 调用已结束：脚本无法启动（{}，工作目录 {}）：{reason}",
                operation.script().display(),
                operation.working_directory().display()
            );
            OperationResponse {
                operation: name,
                ended: true,
                started: false,
                exit_code: None,
                outcome: OperationOutcome::Failed,
                failure: Some(FailureKind::NotStarted),
            }
        }
    }
}


pub(crate) enum ExecutionError { Busy, ResultUnavailable }

pub(crate) async fn execute(name: &'static str, operation: &ScriptOperation, execution: &Arc<Mutex<()>>) -> Result<OperationResponse, ExecutionError> {
    let permit = Arc::clone(execution).try_lock_owned().map_err(|_| ExecutionError::Busy)?;
    let operation = operation.clone();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let _permit = permit;
        let result = operation_response(name, &operation, operation.invoke().await);
        let _ = sender.send(result);
    });
    receiver.await.map_err(|_| ExecutionError::ResultUnavailable)
}
