//! Git workspace 状态：当前分支与待提交变更。
//!
//! 本模块只做**只读查询**：检测登记根自身是否是一个 Git 工作树根，读取当前分支，并用 Git 自己的
//! 结果列出待提交变更。它不暂存、不提交、不修改配置，也不会为了展示而改变索引；`git status` 只以
//! `--no-optional-locks` 运行，避免可选的索引缓存刷新。
//!
//! 待提交集合遵循 Git 原生规则：已跟踪文件的修改、删除（含已暂存与未暂存），以及未被忽略的未跟踪
//! 文件。被忽略的未跟踪文件不出现在这里；已经跟踪、后来才匹配忽略规则的文件仍然出现。重命名采用
//! Git 当前比较结果：未暂存的文件系统改名保持「删除 + 新增」，暂存后由 Git 决定是否识别为一项
//! 重命名——本模块不自行配对，也不通过暂存来制造可显示的结果。
//!
//! 失败语义：登记目录消失、Git 不可用或 Git 命令失败都是明确错误，不会被当成「没有变更」的干净
//! 状态；只有登记根确实不是工作树根时，才返回 `repository: false` 的普通目录结果。冲突只来自 Git
//! 的未合并文件条目，普通命令失败与单纯 ahead/behind 不会被标记为冲突。

use std::collections::HashSet;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::browse;
use crate::sync::{self, SyncFacts, SyncState};
use crate::workspaces::{self, RegisteredWorkspace};

/// 超时后等待子进程确认结束的宽限；超过它仍未确认终止时，调用方必须保留仓库资格。
const TERMINATION_CONFIRMATION: Duration = Duration::from_secs(5);

/// 等待子进程输出读取线程的宽限；派生进程可能仍持有管道。
const OUTPUT_DRAIN: Duration = Duration::from_secs(2);

/// 一个待提交变更的类别；只用于页面着色，不是 Git 的 A/M/D 字母。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Conflicted,
}

/// 待提交变更的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChange {
    /// 当前路径（重命名时是新路径），相对 workspace 根，使用 `/` 分隔。
    pub path: String,
    /// Git 已识别的重命名旧路径；其它类别没有这个字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub kind: ChangeKind,
}

/// 一个 workspace 的 Git 状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub workspace: String,
    /// 登记根自身是否是一个 Git 工作树根。
    pub repository: bool,
    /// 当前分支名；detached HEAD 时为 `null`。
    pub branch: Option<String>,
    pub detached: bool,
    pub changes: Vec<GitChange>,
    /// Git 是否报告了未解决的冲突文件。
    pub conflict: bool,
    /// 待提交变更项数：一项变更计一次；目录与 diff 行数不参与。
    pub pending_count: usize,
    /// 本次展示的审查版本标识；提交请求必须带回同一个标识，才能证明提交针对的是这份审查。
    ///
    /// 标识由分支、HEAD、索引、待提交路径与完整内容共同决定，不受预览上限影响。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_token: Option<String>,
    /// 页面与直接请求都必须阻止普通 Commit 的仓库级原因；没有阻塞时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    /// 当前分支既有 upstream 与最近已知的 ahead/behind；状态读取本身不访问网络。
    pub sync: SyncFacts,
}

/// 登记列表中一个 workspace 的角标概览。
///
/// `pending_count` 只在能够确认是 Git 仓库时出现：普通目录没有 Git 待提交角标，
/// 读取失败则给出 `error`，两者都不会被伪装成「0 项待提交」。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub path: String,
    pub repository: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_count: Option<usize>,
    pub conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 读取整个登记列表的概览；单个 workspace 的失败只影响它自己的条目，不会伪装成空列表。
pub fn overview(
    registered: &[RegisteredWorkspace],
    timeout: Duration,
    sync_state: &SyncState,
) -> Vec<WorkspaceSummary> {
    registered
        .iter()
        .map(
            |workspace| match status(&workspace.path, timeout, sync_state) {
                Ok(status) => WorkspaceSummary {
                    path: workspace.path.clone(),
                    repository: status.repository,
                    branch: status.branch,
                    pending_count: status.repository.then_some(status.pending_count),
                    conflict: status.conflict,
                    error: None,
                },
                Err(error) => WorkspaceSummary {
                    path: workspace.path.clone(),
                    repository: false,
                    branch: None,
                    pending_count: None,
                    conflict: false,
                    error: Some(error.message()),
                },
            },
        )
        .collect()
}

/// Git 状态查询的明确失败类别。
#[derive(Debug)]
pub enum GitError {
    /// 登记根已消失、不可访问或不再是原来的目录。
    WorkspaceUnavailable(String),
    /// 找不到 `git` 可执行文件。
    GitUnavailable(String),
    /// Git 命令失败；保留 Git 返回的诊断，不伪装成干净状态。
    Failed(String),
    /// Git 命令超过本功能配置的执行时限；`confirmed` 表示是否已确认命令及其派生进程结束。
    ///
    /// 这是本功能取消的命令，不是 Git 自己给出的退出结果；`output` 保留取消前已有的输出。
    TimedOut {
        command: String,
        timeout: Duration,
        output: String,
        confirmed: bool,
    },
}

impl GitError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceUnavailable(_) => "workspace-unavailable",
            Self::GitUnavailable(_) => "git-unavailable",
            Self::Failed(_) => "git-command-failed",
            Self::TimedOut { .. } => "git-timeout",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::WorkspaceUnavailable(reason) => reason.clone(),
            Self::GitUnavailable(reason) => reason.clone(),
            Self::Failed(reason) => reason.clone(),
            Self::TimedOut {
                command,
                timeout,
                output,
                confirmed,
            } => {
                let termination = if *confirmed {
                    "已停止本次执行。".to_string()
                } else {
                    "终止尚未确认；在该仓库上的后续写操作不会被允许重叠执行。".to_string()
                };
                let existing = if output.trim().is_empty() {
                    "取消前没有收到 Git 输出。".to_string()
                } else {
                    format!("取消前已有的输出：{}", output.trim())
                };
                format!(
                    "{command} 超过本功能配置的 {} 秒执行上限，{termination}{existing}",
                    timeout.as_secs()
                )
            }
        }
    }

    /// 命令超时且尚未确认其派生进程已停止；调用方必须保持仓库资格。
    pub fn termination_unconfirmed(&self) -> bool {
        matches!(
            self,
            Self::TimedOut {
                confirmed: false,
                ..
            }
        )
    }
}

/// 读取一个已登记 workspace 的 Git 状态。
///
/// 先重新核对登记根（边界与只读语义与浏览一致），再判断它是否是一个工作树根；普通目录返回
/// `repository: false`，不会运行 `git status`，也就不会把父仓库的变更误当成该 workspace 的变更。
pub fn status(
    registered_root: &str,
    timeout: Duration,
    sync_state: &SyncState,
) -> Result<GitStatus, GitError> {
    let Some(root) = worktree_root(registered_root, timeout)? else {
        return Ok(plain_workspace(registered_root));
    };

    let (branch, detached) = current_branch(&root, timeout)?;
    let status = run_git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=no",
            "--ignore-submodules=dirty",
        ],
        timeout,
    )?;
    if !status.status.success() {
        return Err(command_failed("git status", &status));
    }
    let changes = parse_status(&status.stdout)?;
    let conflict = changes
        .iter()
        .any(|change| change.kind == ChangeKind::Conflicted);
    let review_token = review_token(
        &root,
        branch.as_deref(),
        detached,
        &changes,
        &status.stdout,
        timeout,
    )?;
    let blocked_reason = blocked_reason(&root, &changes, detached, timeout)?;
    let sync = sync::facts(&root, branch.as_deref(), detached, timeout, sync_state)?;

    Ok(GitStatus {
        workspace: registered_root.to_string(),
        repository: true,
        branch,
        detached,
        pending_count: changes.len(),
        conflict,
        changes,
        review_token: Some(review_token),
        blocked_reason,
        sync,
    })
}

fn plain_workspace(registered_root: &str) -> GitStatus {
    GitStatus {
        workspace: registered_root.to_string(),
        repository: false,
        branch: None,
        detached: false,
        changes: Vec::new(),
        conflict: false,
        pending_count: 0,
        review_token: None,
        blocked_reason: None,
        sync: SyncFacts::default(),
    }
}

/// 登记根自身是否是一个 Git 工作树根；是则返回核对过边界的根，否则返回 `None`。
///
/// 与状态查询共用同一份判断：登记根是父仓库的子目录、或自身没有 Git 元数据时都按普通目录
/// 处理，不会聚合父仓库或其它仓库的 Git 事实。根被替换、消失或无法访问是明确错误。
pub(crate) fn worktree_root(
    registered_root: &str,
    timeout: Duration,
) -> Result<Option<PathBuf>, GitError> {
    let root = browse::checked_root(registered_root)
        .map_err(|error| GitError::WorkspaceUnavailable(error.message()))?;

    let git_entry = root.join(".git");
    match fs::symlink_metadata(&git_entry) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(GitError::Failed(format!(
                "无法访问 workspace 的 Git 元数据：{error}"
            )));
        }
    }

    let top_level = run_git(&root, &["rev-parse", "--show-toplevel"], timeout)?;
    if !top_level.status.success() {
        return Err(command_failed("git rev-parse --show-toplevel", &top_level));
    }
    let top_level = String::from_utf8_lossy(&top_level.stdout)
        .trim()
        .to_string();
    if top_level.is_empty() || !workspaces::same_path(&top_level, &workspaces::display_path(&root))
    {
        // 登记根不是仓库的工作树根：不把父仓库或其它仓库的变更聚合进来。
        return Ok(None);
    }
    Ok(Some(root))
}

/// 读取当前工作树根下被 Git 忽略的未跟踪路径。
///
/// 使用 Git 自己的 `ls-files --others --ignored --exclude-standard --directory -z`：
/// 因此仓库与嵌套 `.gitignore`、`.git/info/exclude` 和全局 excludes 都由 Git 解释，调用方不解析规则。
/// `--directory` 会直接给出可整棵跳过的 ignored 目录；包含 tracked 文件的目录不会被误报为可跳过。
/// 普通目录返回 `None`，不运行任何 Git 命令。
pub(crate) fn ignored_paths(
    registered_root: &str,
    timeout: Duration,
) -> Result<Option<HashSet<String>>, GitError> {
    let Some(root) = worktree_root(registered_root, timeout)? else {
        return Ok(None);
    };
    let output = run_git(
        &root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
        timeout,
    )?;
    if !output.status.success() {
        return Err(command_failed(
            "git ls-files --others --ignored --exclude-standard --directory -z",
            &output,
        ));
    }

    let mut paths = HashSet::new();
    for raw in output.stdout.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let value = String::from_utf8_lossy(raw);
        let path = value.trim_end_matches('/').replace('\\', "/");
        if !path.is_empty() {
            paths.insert(path);
        }
    }
    Ok(Some(paths))
}

/// 当前分支名；detached HEAD 用 `detached: true` 表示，而不是编造一个分支名。
pub(crate) fn current_branch(
    root: &Path,
    timeout: Duration,
) -> Result<(Option<String>, bool), GitError> {
    let output = run_git(
        root,
        &["symbolic-ref", "--short", "--quiet", "HEAD"],
        timeout,
    )?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() {
            return Err(GitError::Failed(
                "Git 没有给出当前分支名，无法确认操作目标。".to_string(),
            ));
        }
        return Ok((Some(name), false));
    }
    // `--quiet` 让 detached HEAD 以退出码 1 结束且不打印提示；其它失败必须明确报出。
    if output.status.code() == Some(1) {
        return Ok((None, true));
    }
    Err(command_failed("git symbolic-ref", &output))
}

/// 变更审查 diff 的一侧。
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DiffSide {
    /// 这一侧没有该路径：新增文件的旧侧、删除文件的新侧，或尚未首次提交的 HEAD。
    Absent,
    /// 完整文本内容；只有不超过预览上限时才会返回。
    Text { content: String, size: u64 },
    /// 不是可识别的文本；不返回内容。
    Binary { reason: String, size: u64 },
    /// 超过预览上限；没有加载完整内容。
    TooLarge {
        reason: String,
        size: u64,
        limit: u64,
    },
}

/// 一次变更审查的完整事实：旧侧来自当前分支最新本地提交，新侧来自最终本地文件。
///
/// 合并观察已暂存与未暂存内容，不使用索引作为旧侧，也不比较远端。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDiff {
    pub workspace: String,
    /// 变更的新路径；重命名时是新路径，删除时是原路径。
    pub path: String,
    /// Git 已识别重命名的旧路径；其它类别没有这个字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub old: DiffSide,
    pub new: DiffSide,
    /// 两侧是否都可用于渲染完整 diff；任一侧二进制或超限时为 `false`。
    pub complete: bool,
}

/// diff 读取的明确失败：路径边界沿用浏览错误类别，Git 失败保留 Git 诊断。
#[derive(Debug)]
pub enum DiffError {
    Browse(browse::BrowseError),
    Git(GitError),
}

impl DiffError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Browse(error) => error.code(),
            Self::Git(error) => error.code(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Browse(error) => error.message(),
            Self::Git(error) => error.message(),
        }
    }
}

impl From<browse::BrowseError> for DiffError {
    fn from(error: browse::BrowseError) -> Self {
        Self::Browse(error)
    }
}

impl From<GitError> for DiffError {
    fn from(error: GitError) -> Self {
        Self::Git(error)
    }
}

/// 读取一个变更的完整审查 diff：HEAD 内容与最终本地内容。
///
/// 边界（P14）与浏览入口相同：先校验新路径与旧路径的写法，再核对登记根；新侧只通过
/// `browse::read_file` 读取，链接、目录联接、根外路径和根替换都不会被跟随。旧侧只从
/// 已经核对过工作树根的 Git 对象库读取，不使用索引内容。
///
/// 只读（P13）：`git ls-tree` / `git cat-file` 不写索引、不暂存、不改文件。
/// 限额（P44）：新旧两侧各自在读取完整内容之前应用同一个预览上限。
pub fn change_diff(
    registered_root: &str,
    path: &str,
    old_path: Option<&str>,
    preview_limit: u64,
    timeout: Duration,
) -> Result<ChangeDiff, DiffError> {
    // 路径写法先于任何读取被校验；旧路径同样不能借历史内容绕过根边界。
    let new_relative = browse::normalize_relative(path)?;
    if new_relative.is_empty() {
        return Err(browse::BrowseError::NotAFile.into());
    }
    let old_relative = match old_path {
        Some(old) => {
            let normalized = browse::normalize_relative(old)?;
            if normalized.is_empty() {
                return Err(browse::BrowseError::NotAFile.into());
            }
            normalized
        }
        None => new_relative.clone(),
    };

    let Some(root) = worktree_root(registered_root, timeout)? else {
        return Err(DiffError::Git(GitError::Failed(
            "该 workspace 不是 Git 工作树根，没有可审查的本地提交差异。".to_string(),
        )));
    };

    let old = head_side(&root, &old_relative, preview_limit, timeout)?;
    let new = worktree_side(registered_root, &new_relative, preview_limit)?;

    if matches!((&old, &new), (DiffSide::Absent, DiffSide::Absent)) {
        // 两侧都没有内容：变更已经过期或被外部还原，不能伪造一份空 diff。
        return Err(browse::BrowseError::PathNotFound.into());
    }

    let complete = matches!(old, DiffSide::Absent | DiffSide::Text { .. })
        && matches!(new, DiffSide::Absent | DiffSide::Text { .. });

    Ok(ChangeDiff {
        workspace: registered_root.to_string(),
        path: new_relative,
        old_path: old_path.map(|_| old_relative),
        old,
        new,
        complete,
    })
}

/// HEAD 一侧的内容；仓库尚无首次提交时按空树处理，返回明确的 `Absent`。
fn head_side(
    root: &Path,
    path: &str,
    preview_limit: u64,
    timeout: Duration,
) -> Result<DiffSide, DiffError> {
    if !head_exists(root, timeout)? {
        return Ok(DiffSide::Absent);
    }

    let listing = run_git(root, &["ls-tree", "-z", "HEAD", "--", path], timeout)?;
    if !listing.status.success() {
        return Err(command_failed("git ls-tree HEAD", &listing).into());
    }
    let Some((kind, oid)) = ls_tree_entry(&listing.stdout, path)? else {
        return Ok(DiffSide::Absent);
    };
    if kind != "blob" {
        // 子模块引用（commit）或目录（tree）没有可比较的普通文件内容。
        return Ok(DiffSide::Binary {
            reason: format!("该路径在 Git 中不是普通文件内容（{kind}），无法进行文本 diff。"),
            size: 0,
        });
    }

    let size = blob_size(root, &oid, timeout)?;
    if size > preview_limit {
        return Ok(DiffSide::TooLarge {
            reason: format!(
                "旧侧内容大小 {size} 字节，超过预览上限 {preview_limit} 字节；未加载完整内容。"
            ),
            size,
            limit: preview_limit,
        });
    }

    let blob = run_git(root, &["cat-file", "blob", &oid], timeout)?;
    if !blob.status.success() {
        return Err(command_failed("git cat-file blob", &blob).into());
    }
    let bytes = blob.stdout;
    let actual_size = bytes.len() as u64;
    if actual_size > preview_limit {
        return Ok(DiffSide::TooLarge {
            reason: format!(
                "旧侧内容大小 {actual_size} 字节，超过预览上限 {preview_limit} 字节；未加载完整内容。"
            ),
            size: actual_size,
            limit: preview_limit,
        });
    }
    match browse::classify_text(&bytes) {
        Ok(content) => Ok(DiffSide::Text {
            content,
            size: actual_size,
        }),
        Err(reason) => Ok(DiffSide::Binary {
            reason: reason.to_string(),
            size: actual_size,
        }),
    }
}

/// 最终本地文件一侧的内容；路径不存在表示该侧被删除，而不是读取失败。
fn worktree_side(
    registered_root: &str,
    path: &str,
    preview_limit: u64,
) -> Result<DiffSide, DiffError> {
    match browse::read_file(registered_root, path, preview_limit) {
        Ok(view) => Ok(match view.preview {
            browse::Preview::Text { content } => DiffSide::Text {
                content,
                size: view.size,
            },
            browse::Preview::Binary { reason } => DiffSide::Binary {
                reason,
                size: view.size,
            },
            browse::Preview::TooLarge {
                reason,
                size,
                limit,
            } => DiffSide::TooLarge {
                reason,
                size,
                limit,
            },
        }),
        Err(browse::BrowseError::PathNotFound) => Ok(DiffSide::Absent),
        Err(error) => Err(DiffError::Browse(error)),
    }
}

/// 当前分支是否已有首次提交；`rev-parse --verify --quiet` 在无 HEAD 时以 1 结束。
pub(crate) fn head_exists(root: &Path, timeout: Duration) -> Result<bool, GitError> {
    let output = run_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"], timeout)?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(command_failed("git rev-parse --verify HEAD", &output))
}

/// 解析 `git ls-tree -z` 中精确匹配的路径；没有该路径时返回 `None`。
fn ls_tree_entry(output: &[u8], expected: &str) -> Result<Option<(String, String)>, GitError> {
    for record in output.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(GitError::Failed(
                "无法解析 Git 的树对象记录：缺少路径分隔符。".to_string(),
            ));
        };
        let path = String::from_utf8_lossy(&record[tab + 1..]);
        if path != expected {
            continue;
        }
        let header = String::from_utf8_lossy(&record[..tab]);
        let mut parts = header.split(' ');
        let (Some(_mode), Some(kind), Some(oid)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(GitError::Failed(
                "无法解析 Git 的树对象记录：头部形状不符合 ls-tree。".to_string(),
            ));
        };
        if oid.is_empty() {
            return Err(GitError::Failed(
                "无法解析 Git 的树对象记录：缺少对象标识。".to_string(),
            ));
        }
        return Ok(Some((kind.to_string(), oid.to_string())));
    }
    Ok(None)
}

fn blob_size(root: &Path, oid: &str, timeout: Duration) -> Result<u64, GitError> {
    let output = run_git(root, &["cat-file", "-s", oid], timeout)?;
    if !output.status.success() {
        return Err(command_failed("git cat-file -s", &output));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|_| GitError::Failed("无法解析 Git 对象大小：返回内容不是整数。".to_string()))
}

/// 当前 HEAD 提交标识；仓库尚无首次提交时为 `None`。
pub(crate) fn head_oid(root: &Path, timeout: Duration) -> Result<Option<String>, GitError> {
    let verify = run_git(root, &["rev-parse", "--verify", "--quiet", "HEAD"], timeout)?;
    if !verify.status.success() {
        if verify.status.code() == Some(1) {
            return Ok(None);
        }
        return Err(command_failed("git rev-parse --verify HEAD", &verify));
    }
    let oid = String::from_utf8_lossy(&verify.stdout).trim().to_string();
    if oid.is_empty() {
        return Err(GitError::Failed(
            "Git 没有给出 HEAD 提交标识，无法确认审查版本。".to_string(),
        ));
    }
    Ok(Some(oid))
}

/// 一个审查版本的标识。
///
/// 它覆盖分支之外的全部提交依据：HEAD、完整索引、待提交路径集合，以及每个待提交路径的最终内容
/// （重命名同时包含旧路径）。内容哈希直接读取文件，不经过预览上限，因此超限与二进制变化同样会使
/// 旧标识失效；嵌套仓库与子模块只记录其 gitlink 提交，不递归其内部文件。
fn review_token(
    root: &Path,
    branch: Option<&str>,
    detached: bool,
    changes: &[GitChange],
    status_output: &[u8],
    timeout: Duration,
) -> Result<String, GitError> {
    let mut hasher = DefaultHasher::new();
    branch.hash(&mut hasher);
    detached.hash(&mut hasher);
    status_output.hash(&mut hasher);
    head_oid(root, timeout)?.hash(&mut hasher);

    let index = run_git(root, &["ls-files", "--stage", "-z"], timeout)?;
    if !index.status.success() {
        return Err(command_failed("git ls-files --stage", &index));
    }
    index.stdout.hash(&mut hasher);

    for change in changes {
        change.path.hash(&mut hasher);
        change.old_path.hash(&mut hasher);
        change.kind.hash(&mut hasher);
        hash_pending_content(root, &change.path, &mut hasher, timeout)?;
    }

    Ok(format!("{:016x}", hasher.finish()))
}

/// 把待提交路径的最终内容写入审查哈希；不跟随链接，也不递归嵌套仓库内部。
fn hash_pending_content(
    root: &Path,
    relative: &str,
    hasher: &mut DefaultHasher,
    timeout: Duration,
) -> Result<(), GitError> {
    let relative = relative.trim_end_matches('/');
    hasher.write(b"path\0");
    let full = root.join(relative);
    let metadata = match fs::symlink_metadata(&full) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            hasher.write(b"absent");
            return Ok(());
        }
        Err(error) => {
            return Err(GitError::Failed(format!(
                "无法读取待提交路径 {relative}：{error}"
            )));
        }
    };

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&full).map_err(|error| {
            GitError::Failed(format!("无法读取链接 {relative} 的目标：{error}"))
        })?;
        hasher.write(b"link");
        target.as_os_str().hash(hasher);
        return Ok(());
    }

    if metadata.is_dir() {
        if fs::symlink_metadata(full.join(".git")).is_ok() {
            hasher.write(b"gitlink");
            let head = run_git(
                &full,
                &["rev-parse", "--verify", "--quiet", "HEAD"],
                timeout,
            )?;
            if head.status.success() {
                hasher.write(&head.stdout);
            } else if head.status.code() == Some(1) {
                hasher.write(b"unborn");
            } else {
                return Err(command_failed(
                    "git rev-parse --verify HEAD（嵌套仓库）",
                    &head,
                ));
            }
            return Ok(());
        }
        // `git status -uall` 不会把普通目录作为待提交项；这里只记录空目录事实，不递归。
        hasher.write(b"directory");
        return Ok(());
    }

    hasher.write(b"file");
    let mut file = fs::File::open(&full)
        .map_err(|error| GitError::Failed(format!("无法读取待提交文件 {relative}：{error}")))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            GitError::Failed(format!("读取待提交文件 {relative} 失败：{error}"))
        })?;
        if read == 0 {
            break;
        }
        hasher.write(&buffer[..read]);
    }
    Ok(())
}

/// 普通 Commit 的仓库级阻塞原因：detached HEAD、未解决冲突、未完成的 Git 流程。
///
/// 普通命令失败与单纯分支分叉不进入这里。冲突解决但流程尚未结束时仍由流程标记阻塞。
pub(crate) fn blocked_reason(
    root: &Path,
    changes: &[GitChange],
    detached: bool,
    timeout: Duration,
) -> Result<Option<String>, GitError> {
    let mut reasons = Vec::new();
    if detached {
        reasons.push(
            "当前处于 detached HEAD：普通 Commit 不可用；请在外部工具检出分支后再试。".to_string(),
        );
    }
    if changes
        .iter()
        .any(|change| change.kind == ChangeKind::Conflicted)
    {
        reasons.push("存在未解决的冲突文件：请在外部工具解决冲突后再试。".to_string());
    }

    let git_dir = run_git(root, &["rev-parse", "--absolute-git-dir"], timeout)?;
    if !git_dir.status.success() {
        return Err(command_failed("git rev-parse --absolute-git-dir", &git_dir));
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&git_dir.stdout).trim().to_string());
    for (marker, label) in [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase/am"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("sequencer", "多步提交"),
    ] {
        match fs::symlink_metadata(git_dir.join(marker)) {
            Ok(_) => reasons.push(format!(
                "存在未完成的 {label} 流程：请在外部工具完成或撤销后再试。"
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(GitError::Failed(format!(
                    "无法确认 Git 流程状态（{marker}）：{error}"
                )));
            }
        }
    }

    Ok((!reasons.is_empty()).then(|| reasons.join("；")))
}

/// 运行一条 Git 命令，并保证它不会无限期占用仓库。
///
/// 输出由独立线程持续读取，避免子进程写满管道后阻塞。达到时限时结束整棵进程树：
/// 先 `taskkill /T /F`，再等待直接子进程确认退出。只有当直接子进程结束、且输出线程都已结束时，
/// 才把这次终止视为已确认；否则返回 `confirmed: false`，调用方必须保持仓库资格。
pub(crate) fn run_git(root: &Path, args: &[&str], timeout: Duration) -> Result<Output, GitError> {
    let command = format!("git {}", args.join(" "));
    let mut child = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GitError::GitUnavailable(
                    "找不到 git 命令，无法读取 workspace 的 Git 状态。".to_string(),
                )
            } else {
                GitError::Failed(format!("无法运行 git：{error}"))
            }
        })?;

    let stdout = child.stdout.take().expect("stdout 已设为管道");
    let stderr = child.stderr.take().expect("stderr 已设为管道");
    let (stdout_sender, stdout_receiver) = mpsc::channel();
    let (stderr_sender, stderr_receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = stdout_sender.send(read_all(stdout));
    });
    thread::spawn(move || {
        let _ = stderr_sender.send(read_all(stderr));
    });

    let deadline = Instant::now() + timeout;
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_receiver
                    .recv_timeout(OUTPUT_DRAIN)
                    .unwrap_or_default();
                let stderr = stderr_receiver
                    .recv_timeout(OUTPUT_DRAIN)
                    .unwrap_or_default();
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    break true;
                }
                let remaining = deadline.saturating_duration_since(now);
                thread::sleep(remaining.min(Duration::from_millis(20)));
            }
            Err(error) => {
                return Err(GitError::Failed(format!("无法等待 git 结束：{error}")));
            }
        }
    };
    debug_assert!(timed_out);

    let mut confirmed = terminate_tree(child.id());
    let confirmation_deadline = Instant::now() + TERMINATION_CONFIRMATION;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < confirmation_deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                confirmed = false;
                break;
            }
            Err(_) => {
                confirmed = false;
                break;
            }
        }
    }

    let stdout = stdout_receiver
        .recv_timeout(OUTPUT_DRAIN)
        .unwrap_or_default();
    let stderr = stderr_receiver
        .recv_timeout(OUTPUT_DRAIN)
        .unwrap_or_default();
    if stdout_receiver.try_recv().is_ok() || stderr_receiver.try_recv().is_ok() {
        confirmed = false;
    }
    let output = output_text(&stdout, &stderr);
    Err(GitError::TimedOut {
        command,
        timeout,
        output,
        confirmed,
    })
}

/// 结束 Git 进程及其派生进程；返回是否成功发出了整棵进程树的终止请求。
fn terminate_tree(pid: u32) -> bool {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn read_all(mut reader: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = reader.read_to_end(&mut bytes);
    bytes
}

pub(crate) fn command_failed(command: &str, output: &Output) -> GitError {
    let detail = output_text(&output.stdout, &output.stderr);
    let detail = if detail.is_empty() {
        format!("退出码 {:?}", output.status.code())
    } else {
        detail
    };
    GitError::Failed(format!("{command} 失败：{detail}"))
}

/// 把 Git 的输出按纯文本合并；stdout 与 stderr 都保留，便于页面显示已发生的事实。
pub(crate) fn output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (true, false) => stderr,
        (false, true) => stdout,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

/// 解析 `git status --porcelain=v1 -z`。
///
/// 每条记录以 NUL 结束；已识别重命名（以及复制）紧跟一条只含旧路径的 NUL 字段。
fn parse_status(output: &[u8]) -> Result<Vec<GitChange>, GitError> {
    let mut changes: Vec<GitChange> = Vec::new();
    // 重命名/复制的记录后面紧跟一条旧路径字段；重命名保留旧路径，复制只消费它。
    let mut second_path_for: Option<(usize, bool)> = None;

    for field in output.split(|byte| *byte == 0) {
        if field.is_empty() {
            continue;
        }
        if let Some((index, is_rename)) = second_path_for.take() {
            if is_rename {
                changes[index].old_path = Some(String::from_utf8_lossy(field).into_owned());
            }
            continue;
        }
        if field.len() < 4 || field[2] != b' ' {
            return Err(GitError::Failed(
                "无法解析 Git 的待提交状态：记录形状不符合 porcelain v1。".to_string(),
            ));
        }
        let x = field[0] as char;
        let y = field[1] as char;
        let path = String::from_utf8_lossy(&field[3..]).into_owned();
        if path.is_empty() {
            return Err(GitError::Failed(
                "无法解析 Git 的待提交状态：变更缺少路径。".to_string(),
            ));
        }
        if x == '!' && y == '!' {
            // 被忽略的未跟踪文件不属于待提交集合；即使显式请求了 ignored 也不计入。
            continue;
        }

        let kind = change_kind(x, y);
        let index = changes.len();
        changes.push(GitChange {
            path,
            old_path: None,
            kind,
        });
        if x == 'R' || x == 'C' {
            second_path_for = Some((index, x == 'R'));
        }
    }

    if second_path_for.is_some() {
        return Err(GitError::Failed(
            "无法解析 Git 的待提交状态：重命名记录缺少旧路径。".to_string(),
        ));
    }
    Ok(changes)
}

fn change_kind(x: char, y: char) -> ChangeKind {
    let unmerged = matches!(
        (x, y),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    );
    if unmerged {
        return ChangeKind::Conflicted;
    }
    if x == '?' && y == '?' {
        return ChangeKind::Added;
    }
    if x == 'D' || y == 'D' {
        return ChangeKind::Deleted;
    }
    if x == 'R' {
        return ChangeKind::Renamed;
    }
    if x == 'A' || x == 'C' {
        return ChangeKind::Added;
    }
    ChangeKind::Modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_changes_renames_and_conflicts() {
        let parsed = parse_status(
            b" M tracked.txt\0R  new.txt\0old.txt\0?? added.txt\0 D gone.txt\0UU conflict.txt\0",
        )
        .expect("解析成功");
        assert_eq!(
            parsed,
            vec![
                GitChange {
                    path: "tracked.txt".to_string(),
                    old_path: None,
                    kind: ChangeKind::Modified,
                },
                GitChange {
                    path: "new.txt".to_string(),
                    old_path: Some("old.txt".to_string()),
                    kind: ChangeKind::Renamed,
                },
                GitChange {
                    path: "added.txt".to_string(),
                    old_path: None,
                    kind: ChangeKind::Added,
                },
                GitChange {
                    path: "gone.txt".to_string(),
                    old_path: None,
                    kind: ChangeKind::Deleted,
                },
                GitChange {
                    path: "conflict.txt".to_string(),
                    old_path: None,
                    kind: ChangeKind::Conflicted,
                },
            ]
        );
    }
}
