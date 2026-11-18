use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS, RtlNtStatusToDosError};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

// Stable Command exposes the child process handle, not its initial thread handle.
// Use the native process-handle API rather than globally enumerating threads on every spawn.
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtResumeProcess(process: HANDLE) -> NTSTATUS;
}

use super::{GitError, spawn_error};

const TERMINATION_CONFIRMATION: Duration = Duration::from_secs(5);

pub(super) struct GitJob(OwnedHandle);

impl GitJob {
    pub(super) fn spawn(command: &mut Command) -> Result<(Child, Self), GitError> {
        // SAFETY: Null arguments request a private, non-inheritable job handle.
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(GitError::Failed(format!(
                "无法创建 Git 进程 Job：{}",
                io::Error::last_os_error()
            )));
        }
        // SAFETY: CreateJobObjectW returned a new handle owned by this caller.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(handle) });
        let mut child = command
            .creation_flags(CREATE_SUSPENDED)
            .spawn()
            .map_err(spawn_error)?;
        let process = child.as_raw_handle();
        // SAFETY: Both handles are live; the child has not started executing yet.
        if unsafe { AssignProcessToJobObject(job.0.as_raw_handle(), process) } == 0 {
            let error = io::Error::last_os_error();
            // No Git code has run yet, so only the directly owned child needs cleanup.
            let _ = child.kill();
            let confirmed = confirm_termination(|| child.try_wait().map(|status| status.is_some()));
            return Err(startup_failure(
                format!("无法将 Git 加入 Job：{error}"),
                confirmed,
            ));
        }
        // SAFETY: This is the live handle for the suspended child we just assigned.
        let status = unsafe { NtResumeProcess(process) };
        if status < 0 {
            // SAFETY: Converts a native status code by value; no pointers are involved.
            let error = io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(status) } as i32);
            return Err(startup_failure(
                format!("无法恢复 Git 进程：{error}"),
                job.terminate_and_confirm(),
            ));
        }
        Ok((child, job))
    }

    pub(super) fn terminate_and_confirm(&self) -> bool {
        // SAFETY: The live job handle identifies only this command and its descendants.
        // Termination request status is not evidence that the processes have exited.
        unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) };
        confirm_termination(|| self.is_empty())
    }

    fn is_empty(&self) -> io::Result<bool> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: The output buffer matches the requested information class and size.
        let ok = unsafe {
            QueryInformationJobObject(
                self.0.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info.ActiveProcesses == 0)
    }
}

fn startup_failure(reason: String, confirmed: bool) -> GitError {
    if confirmed {
        GitError::Failed(reason)
    } else {
        GitError::TerminationUnconfirmed(reason)
    }
}

fn confirm_termination(mut stopped: impl FnMut() -> io::Result<bool>) -> bool {
    let deadline = Instant::now() + TERMINATION_CONFIRMATION;
    loop {
        match stopped() {
            Ok(true) => return true,
            Ok(false) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(false) | Err(_) => return false,
        }
    }
}
