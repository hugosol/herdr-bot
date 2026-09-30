//! 固定脚本操作的执行：启动配置的执行目标并等待它结束。
//!
//! 这里只负责两个原始事实：执行目标**能否启动**，以及启动后**如何结束**。它不判断脚本成功或
//! 失败，也不做后台 Python 进程的生命周期推断——脚本另开的长期进程不属于本次调用。

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
