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

use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

use serde::Serialize;

use crate::browse;
use crate::workspaces::{self, RegisteredWorkspace};

/// 一个待提交变更的类别；只用于页面着色，不是 Git 的 A/M/D 字母。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
pub fn overview(registered: &[RegisteredWorkspace]) -> Vec<WorkspaceSummary> {
    registered
        .iter()
        .map(|workspace| match status(&workspace.path) {
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
        })
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
}

impl GitError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceUnavailable(_) => "workspace-unavailable",
            Self::GitUnavailable(_) => "git-unavailable",
            Self::Failed(_) => "git-command-failed",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::WorkspaceUnavailable(reason) => reason.clone(),
            Self::GitUnavailable(reason) => reason.clone(),
            Self::Failed(reason) => reason.clone(),
        }
    }
}

/// 读取一个已登记 workspace 的 Git 状态。
///
/// 先重新核对登记根（边界与只读语义与浏览一致），再判断它是否是一个工作树根；普通目录返回
/// `repository: false`，不会运行 `git status`，也就不会把父仓库的变更误当成该 workspace 的变更。
pub fn status(registered_root: &str) -> Result<GitStatus, GitError> {
    let root = browse::checked_root(registered_root)
        .map_err(|error| GitError::WorkspaceUnavailable(error.message()))?;

    let git_entry = root.join(".git");
    match fs::symlink_metadata(&git_entry) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(plain_workspace(registered_root));
        }
        Err(error) => {
            return Err(GitError::Failed(format!(
                "无法访问 workspace 的 Git 元数据：{error}"
            )));
        }
    }

    let top_level = run_git(&root, &["rev-parse", "--show-toplevel"])?;
    if !top_level.status.success() {
        return Err(command_failed("git rev-parse --show-toplevel", &top_level));
    }
    let top_level = String::from_utf8_lossy(&top_level.stdout)
        .trim()
        .to_string();
    if top_level.is_empty() || !workspaces::same_path(&top_level, &workspaces::display_path(&root))
    {
        // 登记根不是仓库的工作树根：不把父仓库或其它仓库的变更聚合进来。
        return Ok(plain_workspace(registered_root));
    }

    let (branch, detached) = current_branch(&root)?;
    let status = run_git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=no",
        ],
    )?;
    if !status.status.success() {
        return Err(command_failed("git status", &status));
    }
    let changes = parse_status(&status.stdout)?;
    let conflict = changes
        .iter()
        .any(|change| change.kind == ChangeKind::Conflicted);

    Ok(GitStatus {
        workspace: registered_root.to_string(),
        repository: true,
        branch,
        detached,
        pending_count: changes.len(),
        conflict,
        changes,
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
    }
}

/// 当前分支名；detached HEAD 用 `detached: true` 表示，而不是编造一个分支名。
fn current_branch(root: &Path) -> Result<(Option<String>, bool), GitError> {
    let output = run_git(root, &["symbolic-ref", "--short", "--quiet", "HEAD"])?;
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

fn run_git(root: &Path, args: &[&str]) -> Result<Output, GitError> {
    Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GitError::GitUnavailable(
                    "找不到 git 命令，无法读取 workspace 的 Git 状态。".to_string(),
                )
            } else {
                GitError::Failed(format!("无法运行 git：{error}"))
            }
        })
}

fn command_failed(command: &str, output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("退出码 {:?}", output.status.code())
    } else {
        stderr
    };
    GitError::Failed(format!("{command} 失败：{detail}"))
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
