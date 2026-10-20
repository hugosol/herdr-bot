//! 新建 workspace：在配置的父目录下创建一个直接子目录、初始化 Git，再交给登记列表。
//!
//! 创建与 Git 初始化是两个已经产生外部效果的步骤，不能假装成一个可回滚事务：任何后续失败都保留
//! 已经创建的目录，并在错误里说明失败阶段与完整路径。只有登记持久化成功后，调用方才返回成功。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::browse;
use crate::git;

/// 创建 workspace 时的明确失败类别。
#[derive(Debug)]
pub enum CreateWorkspaceError {
    ParentNotAbsolute(PathBuf),
    ParentNotFound(PathBuf),
    ParentNotDirectory(PathBuf),
    ParentInaccessible { path: PathBuf, error: io::Error },
    NameRequired,
    InvalidName(String),
    AlreadyExists { path: PathBuf },
    CreateFailed { path: PathBuf, error: io::Error },
    GitInitFailed { path: PathBuf, reason: String },
    FinalizeFailed { path: PathBuf, error: io::Error },
}

impl CreateWorkspaceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ParentNotAbsolute(_) => "create-parent-not-absolute",
            Self::ParentNotFound(_) => "create-parent-not-found",
            Self::ParentNotDirectory(_) => "create-parent-not-a-directory",
            Self::ParentInaccessible { .. } => "create-parent-inaccessible",
            Self::NameRequired => "name-required",
            Self::InvalidName(_) => "invalid-name",
            Self::AlreadyExists { .. } => "name-already-exists",
            Self::CreateFailed { .. } => "create-directory-failed",
            Self::GitInitFailed { .. } => "git-init-failed",
            Self::FinalizeFailed { .. } => "create-finalize-failed",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::ParentNotAbsolute(path) => format!(
                "配置的 workspace 创建父目录不是绝对路径：{}。",
                path.display()
            ),
            Self::ParentNotFound(path) => format!(
                "配置的 workspace 创建父目录不存在：{}。不会自动创建父目录。",
                path.display()
            ),
            Self::ParentNotDirectory(path) => {
                format!("配置的 workspace 创建父目录不是目录：{}。", path.display())
            }
            Self::ParentInaccessible { path, error } => format!(
                "无法访问配置的 workspace 创建父目录：{}；{error}。",
                path.display()
            ),
            Self::NameRequired => "请填写新 workspace 的名称。".to_string(),
            Self::InvalidName(reason) => format!("新 workspace 名称不可接受：{reason}"),
            Self::AlreadyExists { path } => {
                format!("目标目录已存在，未覆盖、未复用：{}。", path.display())
            }
            Self::CreateFailed { path, error } => format!(
                "创建 workspace 目录失败，目录未创建或未完成：{}；{error}。",
                path.display()
            ),
            Self::GitInitFailed { path, reason } => format!(
                "git init 失败，已创建的目录保留：{}；{reason}",
                path.display()
            ),
            Self::FinalizeFailed { path, error } => format!(
                "目录与 Git 初始化已完成，但无法确认目标路径，目录保留：{}；{error}。",
                path.display()
            ),
        }
    }
}

/// 在配置父目录下创建直接子目录并执行 `git init`。
///
/// 只创建一格名称：名称里的路径分隔符、`..`、Windows 设备名和会被文件系统静默改写的尾随点／空格
/// 都在写盘前拒绝。目标已存在时绝不覆盖；Git 初始化失败时也不删除已创建目录。
pub fn create_and_init(
    parent: &Path,
    name: &str,
    git_timeout: Duration,
) -> Result<PathBuf, CreateWorkspaceError> {
    validate_parent(parent)?;
    let name = validate_name(name)?;
    let parent =
        fs::canonicalize(parent).map_err(|error| CreateWorkspaceError::ParentInaccessible {
            path: parent.to_path_buf(),
            error,
        })?;
    let target = parent.join(&name);

    match fs::create_dir(&target) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(CreateWorkspaceError::AlreadyExists { path: target });
        }
        Err(error) => {
            return Err(CreateWorkspaceError::CreateFailed {
                path: target,
                error,
            });
        }
    }

    let initialized = git::run_git(&target, &["init"], git_timeout).map_err(|error| {
        CreateWorkspaceError::GitInitFailed {
            path: target.clone(),
            reason: error.message(),
        }
    })?;
    if !initialized.status.success() {
        let reason = git::command_failed("git init", &initialized).message();
        return Err(CreateWorkspaceError::GitInitFailed {
            path: target,
            reason,
        });
    }

    fs::canonicalize(&target).map_err(|error| CreateWorkspaceError::FinalizeFailed {
        path: target,
        error,
    })
}

fn validate_parent(parent: &Path) -> Result<(), CreateWorkspaceError> {
    if parent.as_os_str().is_empty() || !parent.is_absolute() {
        return Err(CreateWorkspaceError::ParentNotAbsolute(
            parent.to_path_buf(),
        ));
    }
    let metadata = fs::metadata(parent).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => CreateWorkspaceError::ParentNotFound(parent.to_path_buf()),
        _ => CreateWorkspaceError::ParentInaccessible {
            path: parent.to_path_buf(),
            error,
        },
    })?;
    if !metadata.is_dir() {
        return Err(CreateWorkspaceError::ParentNotDirectory(
            parent.to_path_buf(),
        ));
    }
    Ok(())
}

fn validate_name(input: &str) -> Result<String, CreateWorkspaceError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CreateWorkspaceError::NameRequired);
    }
    if trimmed.contains(['/', '\\']) {
        return Err(CreateWorkspaceError::InvalidName(
            "名称只能是配置父目录下的一个直接子目录，不能包含路径分隔符。".to_string(),
        ));
    }
    let normalized = browse::normalize_relative(trimmed)
        .map_err(|error| CreateWorkspaceError::InvalidName(error.message()))?;
    if normalized.is_empty() || normalized.contains('/') {
        return Err(CreateWorkspaceError::InvalidName(
            "名称必须是一个非空的直接子目录。".to_string(),
        ));
    }
    Ok(normalized)
}
