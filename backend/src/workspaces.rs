//! workspace 登记列表：由后端以文件持久保存，电脑与手机共享同一份列表。
//!
//! 登记列表是**服务器**状态，不是浏览器的收藏夹：读取、登记与取消登记都经后端完成，取消登记只移除
//! 登记项，不删除目录或文件。登记项只是一个服务器上的绝对目录路径；它可以是普通目录，也可以是一个
//! Git 工作树的根目录——本模块不初始化 Git，也不扫描、聚合任何嵌套仓库。
//!
//! 持久化语义（P50）：
//! - 只有登记文件成功写入后才返回成功；写入先写同目录的临时文件并 `sync_all`，再原子替换登记文件。
//! - 登记文件不存在表示「还没有任何登记」，不是读取错误。
//! - 读取失败、权限不足或内容损坏都是**明确错误**，不会伪装成空列表；这时登记与取消登记都会失败，
//!   绝不覆盖原文件。
//! - 同一实例内的读取与写入共用一把锁，两个调用者不会看到半写状态。

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// 登记文件在配置缺省时的相对路径（相对进程工作目录，与既有配置路径语义一致）。
pub const DEFAULT_REGISTRY_FILE: &str = "workspaces.json";

/// 登记文件里的一个 workspace：服务器上的一个绝对目录路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredWorkspace {
    pub path: String,
}

/// 登记文件的完整内容。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    workspaces: Vec<RegisteredWorkspace>,
}

/// 一次 workspace 登记请求的明确失败类别。
#[derive(Debug)]
pub enum WorkspaceError {
    /// 没有填写路径。
    Required,
    /// 路径不是绝对路径：workspace 指服务器上的目录，不能相对进程工作目录猜测。
    NotAbsolute,
    /// 路径在服务器上不存在。
    NotFound,
    /// 路径存在但不是目录。
    NotADirectory,
    /// 路径存在但无法访问（权限、坏的链接目标等）。
    Inaccessible(io::Error),
    /// 同一目录已经登记。
    AlreadyRegistered,
    /// 要取消的目录不在登记列表中。
    NotRegistered,
    /// 登记文件存在但无法读取。
    Unreadable(io::Error),
    /// 登记文件内容不是可理解的登记列表。
    Corrupt(String),
    /// 登记文件无法写入；原文件保持不变。
    WriteFailed(io::Error),
}

impl WorkspaceError {
    /// 机器可读的失败类别码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Required => "path-required",
            Self::NotAbsolute => "path-not-absolute",
            Self::NotFound => "path-not-found",
            Self::NotADirectory => "not-a-directory",
            Self::Inaccessible(_) => "path-inaccessible",
            Self::AlreadyRegistered => "already-registered",
            Self::NotRegistered => "not-registered",
            Self::Unreadable(_) => "registry-unreadable",
            Self::Corrupt(_) => "registry-corrupt",
            Self::WriteFailed(_) => "registry-write-failed",
        }
    }

    /// 面向使用者的说明；存储类错误明确说明原登记文件未被修改。
    pub fn message(&self) -> String {
        match self {
            Self::Required => "请填写服务器上的目录路径。".to_string(),
            Self::NotAbsolute => "workspace 路径必须是服务器上的绝对路径。".to_string(),
            Self::NotFound => "该路径在服务器上不存在。".to_string(),
            Self::NotADirectory => "该路径不是目录。".to_string(),
            Self::Inaccessible(error) => format!("无法访问该路径：{error}"),
            Self::AlreadyRegistered => "该目录已经登记。".to_string(),
            Self::NotRegistered => "该目录没有登记。".to_string(),
            Self::Unreadable(error) => {
                format!("读取 workspace 登记列表失败：{error}。原登记文件未被修改。")
            }
            Self::Corrupt(reason) => {
                format!("workspace 登记文件内容已损坏：{reason}。未覆盖原文件。")
            }
            Self::WriteFailed(error) => {
                format!("保存 workspace 登记列表失败：{error}。原登记文件未被修改。")
            }
        }
    }
}

/// 持久保存在一个 JSON 文件里的 workspace 登记列表。
pub struct Workspaces {
    registry_file: PathBuf,
    /// 读取与读改写写共用：同一实例内任意时刻只有一个调用者在操作登记文件。
    lock: Mutex<()>,
}

impl Workspaces {
    pub fn new(registry_file: PathBuf) -> Self {
        Self {
            registry_file,
            lock: Mutex::new(()),
        }
    }

    /// 登记文件位置（启动日志与测试观察用）。
    pub fn registry_file(&self) -> &Path {
        &self.registry_file
    }

    /// 读取当前登记列表；文件尚不存在时是空列表（首次启动）。
    pub async fn list(&self) -> Result<Vec<RegisteredWorkspace>, WorkspaceError> {
        let _guard = self.lock.lock().await;
        self.read()
    }

    /// 登记一个真实存在的服务器目录；成功返回登记后的完整列表。
    pub async fn register(&self, input: &str) -> Result<Vec<RegisteredWorkspace>, WorkspaceError> {
        let candidate = canonical_candidate(input)?;

        let _guard = self.lock.lock().await;
        let mut workspaces = self.read()?;
        if workspaces
            .iter()
            .any(|entry| same_path(&entry.path, &candidate))
        {
            return Err(WorkspaceError::AlreadyRegistered);
        }
        workspaces.push(RegisteredWorkspace { path: candidate });
        self.save(&workspaces)?;
        Ok(workspaces)
    }

    /// 取消一个登记；不检查目录是否仍然存在，也不删除任何目录或文件。
    pub async fn unregister(
        &self,
        input: &str,
    ) -> Result<Vec<RegisteredWorkspace>, WorkspaceError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(WorkspaceError::Required);
        }

        let _guard = self.lock.lock().await;
        let mut workspaces = self.read()?;
        let before = workspaces.len();
        workspaces.retain(|entry| !same_path(&entry.path, trimmed));
        if workspaces.len() == before {
            return Err(WorkspaceError::NotRegistered);
        }
        self.save(&workspaces)?;
        Ok(workspaces)
    }

    /// 读取并校验登记文件；文件不存在是空列表，其余失败都是明确错误。
    fn read(&self) -> Result<Vec<RegisteredWorkspace>, WorkspaceError> {
        match fs::read_to_string(&self.registry_file) {
            Ok(raw) => {
                let document: RegistryDocument = serde_json::from_str(&raw)
                    .map_err(|error| WorkspaceError::Corrupt(error.to_string()))?;
                validate(&document.workspaces)?;
                Ok(document.workspaces)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(WorkspaceError::Unreadable(error)),
        }
    }

    /// 先把新内容写进同目录的临时文件并落盘，再原子替换登记文件。
    ///
    /// 任一步失败都删除临时文件并返回错误；原登记文件在替换成功之前保持原样。
    fn save(&self, workspaces: &[RegisteredWorkspace]) -> Result<(), WorkspaceError> {
        let document = RegistryDocument {
            workspaces: workspaces.to_vec(),
        };
        let content = serde_json::to_string_pretty(&document).map_err(|error| {
            WorkspaceError::WriteFailed(io::Error::new(io::ErrorKind::InvalidData, error))
        })?;

        let temporary = self.temporary_path();
        let written = (|| -> io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(WorkspaceError::WriteFailed(error));
        }
        if let Err(error) = fs::rename(&temporary, &self.registry_file) {
            let _ = fs::remove_file(&temporary);
            return Err(WorkspaceError::WriteFailed(error));
        }
        Ok(())
    }

    fn temporary_path(&self) -> PathBuf {
        let mut name = self
            .registry_file
            .file_name()
            .map(|name| name.to_os_string())
            .unwrap_or_else(|| OsString::from(DEFAULT_REGISTRY_FILE));
        name.push(".tmp");
        self.registry_file.with_file_name(name)
    }
}

/// 登记文件里的每一项都必须有非空路径，且不能重复。
fn validate(workspaces: &[RegisteredWorkspace]) -> Result<(), WorkspaceError> {
    let mut seen = Vec::with_capacity(workspaces.len());
    for entry in workspaces {
        if entry.path.trim().is_empty() {
            return Err(WorkspaceError::Corrupt("登记项缺少路径".to_string()));
        }
        let key = path_key(&entry.path);
        if seen.contains(&key) {
            return Err(WorkspaceError::Corrupt("登记项包含重复路径".to_string()));
        }
        seen.push(key);
    }
    Ok(())
}

/// 校验并规范化一个待登记路径：必须是服务器上的绝对、真实存在的目录。
fn canonical_candidate(input: &str) -> Result<String, WorkspaceError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(WorkspaceError::Required);
    }

    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err(WorkspaceError::NotAbsolute);
    }

    let metadata = fs::metadata(path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => WorkspaceError::NotFound,
        _ => WorkspaceError::Inaccessible(error),
    })?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::NotADirectory);
    }

    let canonical = fs::canonicalize(path).map_err(WorkspaceError::Inaccessible)?;
    Ok(display_path(&canonical))
}

/// Windows 的 `canonicalize` 返回 `\\?\C:\…` 形式；存回用户可读的普通绝对路径。
fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    }
}

/// 路径身份：Windows 上大小写不敏感，结尾分隔符不影响同一性。
fn path_key(path: &str) -> String {
    let trimmed = path.trim();
    let trimmed = if trimmed.len() > 3 {
        trimmed.trim_end_matches(['\\', '/'])
    } else {
        trimmed
    };
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

fn same_path(left: &str, right: &str) -> bool {
    path_key(left) == path_key(right)
}
