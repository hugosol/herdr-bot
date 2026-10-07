//! 关机动作：让**整台** Windows 电脑按系统自己的流程关机（契约 P7、P8；ADR 0003）。
//!
//! “关机”与“停止”是两个概念：后者只执行配置绑定的关闭批处理，不代表程序已被优雅关闭；
//! 关机是系统级动作，整台电脑会停止运行，且**不**先执行“停止”脚本、也**不**占用执行资格。
//!
//! 动作本身放在一个**可替换的接缝**（[`PowerOff`]）后面：交付运行时用 [`SystemPowerOff`]（真实的
//! Win32 调用），自动化测试注入记录型实现（见 `tests/shutdown.rs`），因此自动化永远不会真的关机；
//! 真实断电只做一次人工验收。
//!
//! 生产实现先启用进程的 `SeShutdownPrivilege`（该权限默认存在但禁用），再调用
//! `ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)`：Windows 先向各应用发
//! `WM_QUERYENDSESSION`/`WM_ENDSESSION` 让它们保存并退出，对超过系统等待仍不响应的进程才强制结束，
//! 然后断电——因此“等应用关闭再断电”是有界的，不会永远卡在“某个应用阻止关机”。调用是同步的：
//! 返回 FALSE 就如实失败，**不**改走 `EWX_FORCE` 之类的强制路径，也不重试。
//!
//! 诊断输出（S3 诊断日志 seam）用固定类别词 `invoked` / `failed` 加稳定的 `key=value` 字段，
//! 行首为 `[omp-bot] 关机状态：`，与 mDNS 状态一行的风格一致，因此关机结果可以被自动化断言。
//!
//! 平台门控：Win32 绑定（`windows-sys`）只在 Windows 目标平台引入（见 `Cargo.toml` 的
//! `[target.'cfg(windows)'.dependencies]`），不引入任何额外运行时。本 crate 只构建于 Windows——
//! `operations.rs` 已经直接使用 `std::os::windows`，因此这里的 FFI 不做 `cfg` 分支。

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, GetLastError, HANDLE, LUID};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Shutdown::{
    EWX_FORCEIFHUNG, EWX_SHUTDOWN, ExitWindowsEx, SHTDN_REASON_FLAG_PLANNED,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 关机动作**失败**的原始原因：只写诊断输出，不进入 HTTP 结果类别。
///
/// HTTP 结果只区分“关机已发出（200）”与“动作失败（500 `shutdown-failed`）”；失败发生在哪一步、
/// 系统的原始错误码是什么，只出现在诊断输出里。
#[derive(Debug)]
pub enum PowerOffFailure {
    /// 启用 `SeShutdownPrivilege` 失败：进程拿不到关机权限，动作没有执行。
    Privilege(String),
    /// 系统拒绝了这次关机（`ExitWindowsEx` 返回 FALSE）：动作**没有**改走强制路径。
    Rejected(String),
    /// 动作没能送回结果（执行它的任务没有正常结束）：结果无法确认，按失败处理。
    Internal(String),
}

impl PowerOffFailure {
    /// 失败步骤的机器可读标识：出现在诊断输出里，因此可以按类别断言。
    pub fn step(&self) -> &'static str {
        match self {
            PowerOffFailure::Privilege(_) => "privilege",
            PowerOffFailure::Rejected(_) => "exit-windows-ex",
            PowerOffFailure::Internal(_) => "internal",
        }
    }

    /// 系统的原始错误文本；只用于诊断输出。
    pub fn message(&self) -> &str {
        match self {
            PowerOffFailure::Privilege(message)
            | PowerOffFailure::Rejected(message)
            | PowerOffFailure::Internal(message) => message,
        }
    }
}

/// 关机动作接缝：发起一次本机优雅关机。
///
/// `Ok(())` 表示关机**已发出**（HTTP 200）；`Err` 表示动作失败（HTTP 500 `shutdown-failed`），
/// 且原始错误只写诊断输出。
pub trait PowerOff: Send + Sync + 'static {
    /// 发起关机，并返回“是否已被系统接受”这一个事实。
    ///
    /// 调用是同步的：返回时结果已经确定（要么关机已发出，要么失败）。
    fn power_off(&self) -> Result<(), PowerOffFailure>;
}

/// 一次关机动作的结果事实：类别固定，其余只进诊断输出。
pub enum ShutdownOutcome {
    /// 动作已被调用、系统已接受：关机已发出。
    Invoked,
    /// 动作失败：关机**没有**发出。
    Failed(PowerOffFailure),
}

/// 诊断输出的固定前缀；类别词紧随其后，自动化据此断言关机结果类别。
const DIAGNOSTIC_PREFIX: &str = "[omp-bot] 关机状态：";

/// 写一条关机诊断输出（S3 诊断日志 seam）。
pub fn report(outcome: &ShutdownOutcome) {
    eprintln!("{}", diagnostic(outcome));
}

/// 关机诊断行：类别词 `invoked` / `failed` 固定；失败时附失败步骤与系统原始错误。
fn diagnostic(outcome: &ShutdownOutcome) -> String {
    match outcome {
        ShutdownOutcome::Invoked => format!("{DIAGNOSTIC_PREFIX}invoked"),
        ShutdownOutcome::Failed(failure) => format!(
            "{DIAGNOSTIC_PREFIX}failed step={} error={}",
            failure.step(),
            failure.message()
        ),
    }
}

/// 生产关机动作：真实的 Win32 调用。
pub struct SystemPowerOff;

impl PowerOff for SystemPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> {
        enable_shutdown_privilege()?;

        // SAFETY: 两个参数都是常量，调用只向系统发起一次关机，不读写本进程的内存。
        let issued = unsafe {
            ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG, SHTDN_REASON_FLAG_PLANNED)
        };
        if issued == 0 {
            return Err(PowerOffFailure::Rejected(format!(
                "ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG) 返回 FALSE（GetLastError={}）",
                last_error()
            )));
        }
        Ok(())
    }
}

/// 启用本进程的 `SeShutdownPrivilege`：`ExitWindowsEx` 要求调用进程持有该权限。
///
/// 权限通常已经存在但处于禁用状态（原型验证：`whoami /priv` 中 `SeShutdownPrivilege` 为“已禁用”），
/// 因此每次动作前都启用一次；启用是幂等的。
fn enable_shutdown_privilege() -> Result<(), PowerOffFailure> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `GetCurrentProcess` 只返回当前进程的伪句柄（不需要、也不能关闭）；
    // `OpenProcessToken` 成功后写入一个由本函数负责关闭的真实句柄。
    let opened = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    };
    if opened == 0 {
        return Err(PowerOffFailure::Privilege(format!(
            "OpenProcessToken 失败（GetLastError={}）",
            last_error()
        )));
    }

    let result = enable_privilege_on(token);
    // SAFETY: `token` 是上面成功打开的进程令牌句柄，且只在这里关闭一次。
    unsafe { CloseHandle(token) };
    result
}

/// 在已打开的进程令牌上启用 `SeShutdownPrivilege`（保持 [`enable_shutdown_privilege`] 的句柄管理单一）。
fn enable_privilege_on(token: HANDLE) -> Result<(), PowerOffFailure> {
    let name = wide("SeShutdownPrivilege");
    let mut luid = LUID::default();
    // SAFETY: 名字是以 0 结尾的宽字符缓冲，`luid` 是本函数栈上的可写 LUID。
    let found = unsafe { LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) };
    if found == 0 {
        return Err(PowerOffFailure::Privilege(format!(
            "LookupPrivilegeValueW(\"SeShutdownPrivilege\") 失败（GetLastError={}）",
            last_error()
        )));
    }

    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    // SAFETY: 结构体按 Win32 布局构造、只有一项权限；`token` 由调用者持有并在此后关闭。
    let adjusted = unsafe {
        AdjustTokenPrivileges(
            token,
            0,
            &privileges,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    // `AdjustTokenPrivileges` 即使**一项都没启用成功**也可能返回 TRUE：必须再看 `GetLastError`。
    let error = last_error();
    if adjusted == 0 || error != ERROR_SUCCESS {
        return Err(PowerOffFailure::Privilege(format!(
            "AdjustTokenPrivileges 没能启用 SeShutdownPrivilege（GetLastError={error}）"
        )));
    }
    Ok(())
}

/// 最近一次 Win32 错误的错误码。
fn last_error() -> u32 {
    // SAFETY: 只读取调用线程的“最近错误码”，没有参数也没有副作用。
    unsafe { GetLastError() }
}

/// 把字符串转成 Win32 需要的、以 0 结尾的宽字符缓冲。
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 诊断类别必须机器可断言、互相区分，并带上失败步骤与系统原始错误。
    #[test]
    fn diagnostics_carry_a_distinct_category_and_the_original_error() {
        let invoked = diagnostic(&ShutdownOutcome::Invoked);
        assert!(
            invoked.contains("关机状态：invoked"),
            "成功必须留下 invoked 类别：{invoked}"
        );
        assert!(
            !invoked.contains("failed"),
            "成功不得同时报成失败类别：{invoked}"
        );

        let privilege = diagnostic(&ShutdownOutcome::Failed(PowerOffFailure::Privilege(
            "GetLastError=5".to_string(),
        )));
        assert!(
            privilege.contains("关机状态：failed step=privilege error=GetLastError=5"),
            "提权失败必须带失败步骤与原始错误：{privilege}"
        );

        let refused = diagnostic(&ShutdownOutcome::Failed(PowerOffFailure::Rejected(
            "GetLastError=1115".to_string(),
        )));
        assert!(
            refused.contains("关机状态：failed step=exit-windows-ex error=GetLastError=1115"),
            "系统拒绝必须带失败步骤与原始错误：{refused}"
        );

        let internal = diagnostic(&ShutdownOutcome::Failed(PowerOffFailure::Internal(
            "join error".to_string(),
        )));
        assert!(
            internal.contains("关机状态：failed step=internal error=join error"),
            "没能送回结果的执行必须留下自己的失败步骤：{internal}"
        );

        assert_ne!(
            privilege, refused,
            "两种失败步骤的类别必须互相区分（它们是不同的动作失败原因）"
        );
        assert_ne!(
            refused, internal,
            "系统拒绝与内部故障必须互相区分"
        );
        for line in [&privilege, &refused, &internal] {
            assert!(
                !line.contains("invoked"),
                "失败不得报成成功类别：{line}"
            );
        }
    }

    /// 提权与调用路径证据（验收 9 的非关机部分）：真实启用一次 `SeShutdownPrivilege`。
    ///
    /// 这**不是**关机：只调用 `OpenProcessToken` + `LookupPrivilegeValueW` + `AdjustTokenPrivileges`
    /// （原型已验证该权限存在但默认禁用、可成功启用）。`ExitWindowsEx` 只出现在
    /// [`SystemPowerOff::power_off`] 里，而自动化从不调用它——测试全部注入记录型替身，因此没有任何
    /// 真实关机路径被执行。
    #[test]
    fn enabling_the_shutdown_privilege_succeeds_without_any_shutdown() {
        enable_shutdown_privilege().expect("SeShutdownPrivilege 必须能在本进程成功启用");
    }
}
