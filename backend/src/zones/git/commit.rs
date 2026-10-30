//! 经审查复核的整仓提交：暂存全部符合 Git 规则的最终变更，再创建一次本地提交。
//!
//! 写入语义：
//! - 确认后的第一步是用当前事实重新计算审查标识，并与页面展示的那份比较；分支、HEAD、索引、
//!   待提交集合或完整内容任何一处变化都会拒绝当次提交。
//! - 复核通过后 `git add --all` 暂存整仓最终变化（已跟踪变更 + 未被忽略的未跟踪文件），已有部分
//!   暂存不会缩小范围，被忽略的未跟踪文件不会被强加。
//! - 暂存后 `git commit` 只创建本地提交，不 push、不改配置、不打开编辑器；任何一步失败都立即停止，
//!   且不撤销已经成功的步骤。
//! - 同一实际仓库的写操作由仓库身份（规范化后的工作树根）串行化；路径别名不能绕过它。发起请求的
//!   连接断开不影响已接受的执行，资格保留到执行确认结束。命令超时且终止未确认时，资格继续保持，
//!   不允许后续写操作重叠。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tokio::sync::Mutex as AsyncMutex;

use crate::zones::git::{self, GitError, GitStatus, command_failed};
use crate::zones;

/// 一个仓库的写操作资格。不同的实际仓库互不影响；同一仓库的不同路径别名共用同一把锁。
#[derive(Default)]
pub struct RepositoryLocks {
    repositories: StdMutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl RepositoryLocks {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取得一个仓库身份对应的锁；仓库身份使用规范化绝对路径，不区分大小写与分隔符写法。
    fn for_repository(&self, root: &str) -> Arc<AsyncMutex<()>> {
        let key = zones::path_key(root);
        let mut repositories = self
            .repositories
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            repositories
                .entry(key)
                .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
        )
    }

    /// 立即尝试取得一个实际仓库的写操作资格；拿不到时返回 `None`，不等待、不排队。
    ///
    /// Commit 与 Sync 都经这里取得同一份资格，因此同一仓库的混合并发不会重叠。
    pub(crate) fn try_qualify(&self, root: &str) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        self.for_repository(root).try_lock_owned().ok()
    }
}

/// 一次提交请求的归宿；页面根据它区分“已提交”“审查失效”“被阻塞”和“执行失败/超时”。
#[derive(Debug)]
pub enum CommitOutcome {
    Committed {
        commit: String,
        branch: Option<String>,
        message: String,
    },
    /// 仓库不是 Git 工作树根；普通目录没有可执行的 Commit。
    NotRepository,
    /// 同一仓库已有写操作在执行；本次请求没有被接受，也不会排队补做。
    Busy,
    /// 仓库级阻塞（detached HEAD、冲突、未完成流程）。
    Blocked(String),
    /// 页面展示的审查版本已经与当前事实不一致；本次提交没有发生。
    ReviewInvalid(String),
    /// 复核通过但没有可提交的变更。
    NothingToCommit,
    /// Git 命令失败或超时；保留已成功的步骤与 Git 输出。
    Failed(GitError),
}

/// 执行一次整仓提交。调用方负责先确认 message 非空。
///
/// 本函数是阻塞的（同步 Git 进程），应放在 `spawn_blocking` 中运行。拿不到资格时立即返回
/// `Busy`，不等待、不排队。
pub fn execute(
    registered_root: &str,
    message: &str,
    review_token: &str,
    timeout: Duration,
    locks: &RepositoryLocks,
    sync_state: &crate::zones::git::sync::SyncState,
) -> CommitOutcome {
    sync_state.cache.invalidate(registered_root);
    let root = match git::worktree_root(registered_root, timeout) {
        Ok(Some(root)) => root,
        Ok(None) => return CommitOutcome::NotRepository,
        Err(error) => return CommitOutcome::Failed(error),
    };
    let Some(permit) = locks.try_qualify(&zones::display_path(&root)) else {
        return CommitOutcome::Busy;
    };

    if sync_state.remote_checking(registered_root) {
        return CommitOutcome::Busy;
    }
    sync_state.cache.invalidate(registered_root);
    let outcome = execute_locked(
        &root,
        registered_root,
        message,
        review_token,
        timeout,
        sync_state,
    );
    sync_state.cache.invalidate(registered_root);
    if matches!(&outcome, CommitOutcome::Committed { .. }) {
        sync_state.begin_remote_check(registered_root);
    }
    retain_qualification_if_unconfirmed(permit, outcome)
}

/// 命令超时且终止未确认时继续保持仓库资格，绝不让后续写操作与可能仍在运行的进程重叠。
///
/// 这是保守选择：无法确认进程已经停止时，本实例宁可让该仓库保持忙碌，也不允许第二个写操作。
fn retain_qualification_if_unconfirmed(
    permit: tokio::sync::OwnedMutexGuard<()>,
    outcome: CommitOutcome,
) -> CommitOutcome {
    if matches!(&outcome, CommitOutcome::Failed(error) if error.termination_unconfirmed()) {
        std::mem::forget(permit);
    }
    outcome
}

fn execute_locked(
    root: &PathBuf,
    registered_root: &str,
    message: &str,
    review_token: &str,
    timeout: Duration,
    sync_state: &crate::zones::git::sync::SyncState,
) -> CommitOutcome {
    let status: GitStatus = match git::status(registered_root, timeout, sync_state) {
        Ok(status) => status,
        Err(error) => return CommitOutcome::Failed(error),
    };
    if let Some(reason) = status.blocked_reason {
        return CommitOutcome::Blocked(reason);
    }
    match status.review_token.as_deref() {
        Some(current) if current == review_token => {}
        Some(_) => {
            return CommitOutcome::ReviewInvalid(
                "审查版本已失效：分支、HEAD、索引、待提交集合或内容已变化；本次没有提交，请重新审查。"
                    .to_string(),
            );
        }
        None => {
            return CommitOutcome::Failed(GitError::Failed(
                "无法取得当前审查版本，本次没有提交。".to_string(),
            ));
        }
    }
    if status.changes.is_empty() {
        return CommitOutcome::NothingToCommit;
    }

    let staged = match git::run_git(root, &["add", "--all"], timeout) {
        Ok(output) => output,
        Err(error) => return CommitOutcome::Failed(error),
    };
    if !staged.status.success() {
        return CommitOutcome::Failed(command_failed("git add --all", &staged));
    }

    let committed = match git::run_git(root, &["commit", "-m", message], timeout) {
        Ok(output) => output,
        Err(error) => return CommitOutcome::Failed(error),
    };
    if !committed.status.success() {
        return CommitOutcome::Failed(command_failed("git commit", &committed));
    }

    match git::head_oid(root, timeout) {
        Ok(Some(commit)) => CommitOutcome::Committed {
            commit,
            branch: status.branch,
            message: message.to_string(),
        },
        Ok(None) => CommitOutcome::Failed(GitError::Failed(
            "Git 报告提交成功，但没有取得新的 HEAD 提交标识，无法确认提交结果。".to_string(),
        )),
        Err(error) => CommitOutcome::Failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed_out(confirmed: bool) -> CommitOutcome {
        CommitOutcome::Failed(GitError::TimedOut {
            command: "git commit".to_string(),
            timeout: Duration::from_secs(1),
            output: String::new(),
            confirmed,
        })
    }

    #[tokio::test]
    async fn unconfirmed_termination_keeps_the_repository_qualification() {
        let repository = Arc::new(AsyncMutex::new(()));
        let permit = Arc::clone(&repository)
            .try_lock_owned()
            .expect("初始资格必须可用");

        let outcome = retain_qualification_if_unconfirmed(permit, timed_out(false));
        assert!(
            matches!(&outcome, CommitOutcome::Failed(error) if error.termination_unconfirmed())
        );
        assert!(
            repository.try_lock_owned().is_err(),
            "终止未确认时，后续写操作不能取得仓库资格"
        );
    }

    #[tokio::test]
    async fn confirmed_or_ordinary_failures_release_the_repository_qualification() {
        for outcome in [
            timed_out(true),
            CommitOutcome::Failed(GitError::Failed("hook rejected".to_string())),
            CommitOutcome::Blocked("merge in progress".to_string()),
            CommitOutcome::ReviewInvalid("stale".to_string()),
            CommitOutcome::NothingToCommit,
        ] {
            let repository = Arc::new(AsyncMutex::new(()));
            let permit = Arc::clone(&repository)
                .try_lock_owned()
                .expect("初始资格必须可用");
            let _ = retain_qualification_if_unconfirmed(permit, outcome);
            assert!(
                repository.try_lock_owned().is_ok(),
                "已确认终止或普通结果必须释放仓库资格"
            );
        }
    }
}
