//! 按当前分支既有 upstream 与 Git 配置的远端同步。
//!
//! 同步语义（S2）：
//! - 远端状态按需检查：`check` 执行一次真实的 `git fetch <upstream.remote>`，成功后记录“最近检查时间”。
//!   状态读取只使用本地远端跟踪引用，绝不隐式 fetch；页面把未检查与已知状态分开显示。
//! - `execute` 先重新核对登记根、当前分支、upstream 与干净工作树，再按 fetch → pull → 按需 push 的顺序
//!   执行。pull 不附加 repository、refspec 或 merge/rebase/ff-only 策略，因此完全遵循仓库已有配置；
//!   没有待推送提交时不执行无意义的 push。不 stash、不隐式 commit、不强推、不修改配置。
//! - 任一步失败立即停止后续步骤，并保留 Git 原始诊断；已经成功的步骤不会被自动撤销，也不宣称事务回滚。
//! - 与 Commit 共用同一份仓库写操作资格：同一实际仓库的 Sync/Sync、Commit/Sync 与路径别名都不会重叠。
//!   连接断开不取消已接受的同步；命令超时且终止未确认时继续持有资格。
//! - 每条命令都使用 `[git] command_timeout_seconds`；需要交互的凭据、编辑器或签名条件只会明确失败或超时，
//!   不打开终端/网页凭据流程。
//! - 只处理登记根自身的工作树，不递归嵌套仓库或子模块内部；fetch/pull/push 的 Git 语义本身也不会改写
//!   嵌套仓库的内容。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::OwnedMutexGuard;

use crate::commit::RepositoryLocks;
use crate::git::{self, GitError, GitStatus, command_failed};
use crate::zones;

/// 当前分支已有的 upstream；`short` 是 Git 的简短写法（如 `upstream/feature`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamRef {
    pub remote: String,
    pub branch: String,
    pub short: String,
}

/// 一个 zone 的远端同步事实。
///
/// `ahead`/`behind` 来自最近已知的本地远端跟踪引用；`remoteCheckedAt` 是本实例最后一次成功 fetch 的
/// 时刻（Unix 毫秒），为 `null` 时页面必须显示“远端尚未检查”，不能把旧引用当成实时状态。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFacts {
    pub upstream: Option<UpstreamRef>,
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
    pub remote_checked_at: Option<u64>,
}

/// 一次成功检查的记录：时间与当时观察到的远端跟踪引用。
///
/// 只凭路径与 upstream 记忆检查时间会在仓库被外部替换后说谎；把观察到的引用一起记下，
/// 当前引用与它不一致时（外部 fetch、仓库替换）必须重新显示“尚未检查”。
struct CheckRecord {
    checked_at: u64,
    tracking_oid: String,
}

/// 本实例成功检查远端的记录；按实际仓库、upstream 与观察到的远端引用区分。
#[derive(Default)]
pub struct SyncState {
    checks: StdMutex<HashMap<String, CheckRecord>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, root: &Path, upstream: &UpstreamRef, tracking_oid: &str) {
        let mut checks = self
            .checks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        checks.insert(
            check_key(root, upstream),
            CheckRecord {
                checked_at: now_millis(),
                tracking_oid: tracking_oid.to_string(),
            },
        );
    }

    fn checked_at(&self, root: &Path, upstream: &UpstreamRef, tracking_oid: &str) -> Option<u64> {
        self.checks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&check_key(root, upstream))
            .filter(|record| record.tracking_oid == tracking_oid)
            .map(|record| record.checked_at)
    }
}

fn check_key(root: &Path, upstream: &UpstreamRef) -> String {
    format!(
        "{}\0{}",
        zones::path_key(&zones::display_path(root)),
        upstream.short
    )
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// 读取一个已核对工作树根的同步事实。
///
/// 只读取本地状态，不访问网络：upstream 来自 `for-each-ref`，ahead/behind 来自 HEAD 与远端跟踪引用。
/// 没有 upstream、远端跟踪引用缺失或仓库尚无首次提交时，未知项保持 `None`，而不是编造 0。
pub(crate) fn facts(
    root: &Path,
    branch: Option<&str>,
    detached: bool,
    timeout: Duration,
    state: &SyncState,
) -> Result<SyncFacts, GitError> {
    if detached {
        return Ok(SyncFacts::default());
    }
    let Some(branch) = branch else {
        return Ok(SyncFacts::default());
    };
    let Some(upstream) = resolve_upstream(root, branch, timeout)? else {
        return Ok(SyncFacts::default());
    };

    let tracking_oid = remote_tracking_oid(root, &upstream, timeout)?;
    let (ahead, behind) = match &tracking_oid {
        Some(_) if git::head_exists(root, timeout)? => {
            let counts = ahead_behind(root, &upstream, timeout)?;
            (Some(counts.0), Some(counts.1))
        }
        _ => (None, None),
    };

    Ok(SyncFacts {
        remote_checked_at: tracking_oid
            .as_deref()
            .and_then(|oid| state.checked_at(root, &upstream, oid)),
        upstream: Some(upstream),
        ahead,
        behind,
    })
}

/// 读取当前分支配置的 upstream；没有配置时返回 `None`。
fn resolve_upstream(
    root: &Path,
    branch: &str,
    timeout: Duration,
) -> Result<Option<UpstreamRef>, GitError> {
    let output = git::run_git(
        root,
        &[
            "for-each-ref",
            "--format=%(upstream:short)",
            &format!("refs/heads/{branch}"),
        ],
        timeout,
    )?;
    if !output.status.success() {
        return Err(command_failed("git for-each-ref", &output));
    }
    let short = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if short.is_empty() {
        return Ok(None);
    }
    // upstream 可能是本地分支（没有远程部分）；本功能的远端同步只处理远端跟踪分支。
    let Some((remote, branch_name)) = short.split_once('/') else {
        return Ok(None);
    };
    if remote.is_empty() || branch_name.is_empty() {
        return Ok(None);
    }
    Ok(Some(UpstreamRef {
        remote: remote.to_string(),
        branch: branch_name.to_string(),
        short,
    }))
}

fn remote_tracking_oid(
    root: &Path,
    upstream: &UpstreamRef,
    timeout: Duration,
) -> Result<Option<String>, GitError> {
    let reference = format!("refs/remotes/{}", upstream.short);
    let output = git::run_git(
        root,
        &["rev-parse", "--verify", "--quiet", &reference],
        timeout,
    )?;
    if output.status.success() {
        let oid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if oid.is_empty() {
            return Err(GitError::Failed(
                "Git 没有给出远端跟踪引用标识，无法确认远端状态。".to_string(),
            ));
        }
        return Ok(Some(oid));
    }
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    Err(command_failed("git rev-parse --verify upstream", &output))
}

fn ahead_behind(
    root: &Path,
    upstream: &UpstreamRef,
    timeout: Duration,
) -> Result<(u64, u64), GitError> {
    let range = format!("HEAD...refs/remotes/{}", upstream.short);
    let output = git::run_git(
        root,
        &["rev-list", "--left-right", "--count", &range],
        timeout,
    )?;
    if !output.status.success() {
        return Err(command_failed("git rev-list --left-right --count", &output));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fields = text.split_whitespace();
    let ahead = fields.next().and_then(|field| field.parse::<u64>().ok());
    let behind = fields.next().and_then(|field| field.parse::<u64>().ok());
    match (ahead, behind) {
        (Some(ahead), Some(behind)) if fields.next().is_none() => Ok((ahead, behind)),
        _ => Err(GitError::Failed(
            "无法解析 Git 给出的 ahead/behind 数量。".to_string(),
        )),
    }
}

/// 一次远端检查的归宿。
pub enum CheckOutcome {
    Checked(GitStatus),
    NotRepository,
    Busy,
    Blocked(String),
    NoUpstream,
    Failed(GitError),
}

/// 主动检查远端：真实 fetch 当前分支 upstream 的远端，成功后记录检查时间。
///
/// 检查不要求工作树干净（它不触碰工作树），但要求登记根是 Git 工作树根且当前分支有远端 upstream。
/// 与 Commit/Sync 共用同一份仓库写操作资格，因此不会与它们的引用写入重叠。
pub fn check(
    registered_root: &str,
    timeout: Duration,
    locks: &RepositoryLocks,
    state: &SyncState,
) -> CheckOutcome {
    let root = match git::worktree_root(registered_root, timeout) {
        Ok(Some(root)) => root,
        Ok(None) => return CheckOutcome::NotRepository,
        Err(error) => return CheckOutcome::Failed(error),
    };
    let Some(permit) = locks.try_qualify(&zones::display_path(&root)) else {
        return CheckOutcome::Busy;
    };

    let outcome = check_locked(registered_root, &root, timeout, state);
    retain_qualification_if_unconfirmed(permit, outcome)
}

fn check_locked(
    registered_root: &str,
    root: &Path,
    timeout: Duration,
    state: &SyncState,
) -> CheckOutcome {
    let (branch, detached) = match git::current_branch(root, timeout) {
        Ok(current) => current,
        Err(error) => return CheckOutcome::Failed(error),
    };
    let Some(branch) = branch else {
        debug_assert!(detached);
        return CheckOutcome::Blocked(
            "当前处于 detached HEAD：无法确定 upstream，远端检查不可用。".to_string(),
        );
    };
    if detached {
        return CheckOutcome::Blocked(
            "当前处于 detached HEAD：无法确定 upstream，远端检查不可用。".to_string(),
        );
    }
    let upstream = match resolve_upstream(root, &branch, timeout) {
        Ok(Some(upstream)) => upstream,
        Ok(None) => return CheckOutcome::NoUpstream,
        Err(error) => return CheckOutcome::Failed(error),
    };

    if let Err(error) = fetch(root, &upstream, timeout) {
        return CheckOutcome::Failed(error);
    }
    if let Err(error) = record_check(root, &upstream, timeout, state) {
        return CheckOutcome::Failed(error);
    }

    match git::status(registered_root, timeout, state) {
        Ok(status) => CheckOutcome::Checked(status),
        Err(error) => CheckOutcome::Failed(error),
    }
}

/// 一次同步执行的归宿。
pub enum SyncOutcome {
    Synced(SyncAccepted),
    NotRepository,
    /// 同一仓库已有写操作在执行；本次请求没有被接受，也不排队补做。
    Busy,
    /// detached HEAD、冲突或未完成流程；需要用户在外部处理。
    Blocked(String),
    /// 工作树有未提交变更；Sync 不 stash、不隐式提交。
    Dirty,
    /// 当前分支没有远端 upstream；本功能不会配置 upstream 或发布分支。
    NoUpstream,
    /// Git 命令失败或超时；保留已完成步骤与 Git 输出。
    Failed {
        step: &'static str,
        error: GitError,
        completed: Vec<&'static str>,
    },
}

/// 一次成功同步的回执。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAccepted {
    pub zone: String,
    pub outcome: &'static str,
    pub branch: String,
    pub upstream: UpstreamRef,
    /// 同步后的已知待推送数量；成功同步后通常为 0。
    pub ahead: u64,
    /// 同步后的已知待拉取数量；成功同步后通常为 0。
    pub behind: u64,
    pub remote_checked_at: u64,
    /// 本次 pull 整合的远端提交数（fetch 后、pull 前的 behind）。
    pub pulled: u64,
    /// 本次 push 推送到远端的提交数；没有待推送时不执行 push，也为 0。
    pub pushed: u64,
    pub steps: Vec<StepReport>,
}

/// 已执行或按需跳过的同步步骤。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepReport {
    pub step: &'static str,
    pub outcome: &'static str,
}

/// 执行一次按既有 upstream 与 Git 配置的同步。
///
/// 调用方负责先确认请求来自已登记的 zone。本函数是阻塞的（同步 Git 进程），应在
/// `spawn_blocking` 中运行；拿不到仓库资格时立即返回 `Busy`，不等待、不排队。
pub fn execute(
    registered_root: &str,
    timeout: Duration,
    locks: &RepositoryLocks,
    state: &SyncState,
) -> SyncOutcome {
    let root = match git::worktree_root(registered_root, timeout) {
        Ok(Some(root)) => root,
        Ok(None) => return SyncOutcome::NotRepository,
        Err(error) => {
            return SyncOutcome::Failed {
                step: "zone",
                error,
                completed: Vec::new(),
            };
        }
    };
    let Some(permit) = locks.try_qualify(&zones::display_path(&root)) else {
        return SyncOutcome::Busy;
    };

    let outcome = execute_locked(registered_root, &root, timeout, state);
    retain_qualification_if_unconfirmed(permit, outcome)
}

fn execute_locked(
    registered_root: &str,
    root: &Path,
    timeout: Duration,
    state: &SyncState,
) -> SyncOutcome {
    // 执行时重新核对当前分支、工作树与阻塞状态；页面展示的内容不能替代这次检查。
    let status = match git::status(registered_root, timeout, state) {
        Ok(status) => status,
        Err(error) => {
            return SyncOutcome::Failed {
                step: "status",
                error,
                completed: Vec::new(),
            };
        }
    };
    if let Some(reason) = status.blocked_reason.clone() {
        return SyncOutcome::Blocked(reason);
    }
    if !status.changes.is_empty() {
        return SyncOutcome::Dirty;
    }
    let Some(branch) = status.branch.clone() else {
        return SyncOutcome::Blocked(
            "当前处于 detached HEAD：Sync 不可用；请在外部工具检出分支后再试。".to_string(),
        );
    };
    let Some(upstream) = status.sync.upstream.clone() else {
        return SyncOutcome::NoUpstream;
    };

    let mut completed: Vec<&'static str> = Vec::new();

    // 1. 同步前必定重新检查远端；检查成功才记录最近检查时间。
    if let Err(error) = fetch(root, &upstream, timeout) {
        return SyncOutcome::Failed {
            step: "fetch",
            error,
            completed,
        };
    }
    if let Err(error) = record_check(root, &upstream, timeout, state) {
        return SyncOutcome::Failed {
            step: "fetch",
            error,
            completed,
        };
    }
    completed.push("fetch");

    let after_fetch = match facts(root, Some(&branch), false, timeout, state) {
        Ok(facts) => facts,
        Err(error) => {
            return SyncOutcome::Failed {
                step: "fetch",
                error,
                completed,
            };
        }
    };
    let pulled = after_fetch.behind.unwrap_or(0);

    // 2. 遵守仓库已有 pull 配置；不附加 strategy、不 stash、不隐式提交。
    if let Err(error) = run_command(root, &["pull"], timeout) {
        return SyncOutcome::Failed {
            step: "pull",
            error,
            completed,
        };
    }
    completed.push("pull");

    let after_pull = match facts(root, Some(&branch), false, timeout, state) {
        Ok(facts) => facts,
        Err(error) => {
            return SyncOutcome::Failed {
                step: "pull",
                error,
                completed,
            };
        }
    };
    let ahead = after_pull.ahead.unwrap_or(0);

    // 3. 只有同步后仍有本地提交才 push；没有待推送提交时不产生无意义副作用。
    let pushed = if ahead > 0 {
        if let Err(error) = run_command(root, &["push"], timeout) {
            return SyncOutcome::Failed {
                step: "push",
                error,
                completed,
            };
        }
        completed.push("push");
        ahead
    } else {
        0
    };

    // push 成功会推进远端跟踪引用；用新的引用重新记录，避免把刚完成的同步显示成“未检查”。
    if let Err(error) = record_check(root, &upstream, timeout, state) {
        return SyncOutcome::Failed {
            step: "push",
            error,
            completed,
        };
    }
    let final_facts = match facts(root, Some(&branch), false, timeout, state) {
        Ok(facts) => facts,
        Err(error) => {
            return SyncOutcome::Failed {
                step: "push",
                error,
                completed,
            };
        }
    };
    let remote_checked_at = final_facts.remote_checked_at.unwrap_or_else(now_millis);

    let mut steps: Vec<StepReport> = completed
        .iter()
        .map(|step| StepReport {
            step,
            outcome: "completed",
        })
        .collect();
    if pushed == 0 {
        steps.push(StepReport {
            step: "push",
            outcome: "skipped",
        });
    }

    SyncOutcome::Synced(SyncAccepted {
        zone: registered_root.to_string(),
        outcome: "synced",
        branch,
        upstream,
        ahead: final_facts.ahead.unwrap_or(0),
        behind: final_facts.behind.unwrap_or(0),
        remote_checked_at,
        pulled,
        pushed,
        steps,
    })
}

fn fetch(root: &Path, upstream: &UpstreamRef, timeout: Duration) -> Result<(), GitError> {
    run_command(root, &["fetch", &upstream.remote], timeout)
}

/// 记录一次成功的远端检查；只有当前远端跟踪引用可读时才记录。
fn record_check(
    root: &Path,
    upstream: &UpstreamRef,
    timeout: Duration,
    state: &SyncState,
) -> Result<(), GitError> {
    let Some(oid) = remote_tracking_oid(root, upstream, timeout)? else {
        return Err(GitError::Failed(
            "远端检查没有留下可确认的跟踪引用，无法确认远端状态。".to_string(),
        ));
    };
    state.record(root, upstream, &oid);
    Ok(())
}

/// 运行一条同步命令；输出只在失败时保留，避免把成功输出伪装成页面事实。
fn run_command(root: &Path, args: &[&str], timeout: Duration) -> Result<(), GitError> {
    let output = git::run_git(root, args, timeout)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failed(&format!("git {}", args.join(" ")), &output))
    }
}

/// 命令超时且终止未确认时继续保持仓库资格，绝不让后续写操作与可能仍在运行的进程重叠。
fn retain_qualification_if_unconfirmed<T: QualificationOutcome>(
    permit: OwnedMutexGuard<()>,
    outcome: T,
) -> T {
    if outcome.termination_unconfirmed() {
        std::mem::forget(permit);
    }
    outcome
}

/// 两种同步结果都能说明“终止是否尚未确认”。
trait QualificationOutcome {
    fn termination_unconfirmed(&self) -> bool;
}

impl QualificationOutcome for CheckOutcome {
    fn termination_unconfirmed(&self) -> bool {
        matches!(self, Self::Failed(error) if error.termination_unconfirmed())
    }
}

impl QualificationOutcome for SyncOutcome {
    fn termination_unconfirmed(&self) -> bool {
        matches!(self, Self::Failed { error, .. } if error.termination_unconfirmed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex as AsyncMutex;

    fn timed_out(confirmed: bool) -> SyncOutcome {
        SyncOutcome::Failed {
            step: "push",
            error: GitError::TimedOut {
                command: "git push".to_string(),
                timeout: Duration::from_secs(1),
                output: String::new(),
                confirmed,
            },
            completed: vec!["fetch", "pull"],
        }
    }

    #[test]
    fn unconfirmed_termination_keeps_the_repository_qualification() {
        let repository = Arc::new(AsyncMutex::new(()));
        let permit = Arc::clone(&repository)
            .try_lock_owned()
            .expect("初始资格必须可用");

        let outcome = retain_qualification_if_unconfirmed(permit, timed_out(false));
        assert!(
            matches!(&outcome, SyncOutcome::Failed { error, .. } if error.termination_unconfirmed())
        );
        assert!(
            repository.try_lock_owned().is_err(),
            "终止未确认时，后续写操作不能取得仓库资格"
        );
    }

    #[test]
    fn confirmed_termination_and_ordinary_results_release_the_repository_qualification() {
        let outcomes = [
            timed_out(true),
            SyncOutcome::Failed {
                step: "fetch",
                error: GitError::Failed("network failed".to_string()),
                completed: Vec::new(),
            },
            SyncOutcome::Blocked("merge in progress".to_string()),
            SyncOutcome::Dirty,
            SyncOutcome::NoUpstream,
            SyncOutcome::Busy,
            SyncOutcome::NotRepository,
        ];
        for outcome in outcomes {
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
