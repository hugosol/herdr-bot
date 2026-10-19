//! workspace 文件浏览：只读地列目录与读取单个文件内容。
//!
//! 边界（P14）是这里的核心：所有公开读取都先确认 workspace 登记仍然有效，再从登记根出发逐级检查
//! 每个路径分量。绝对路径、`..` 遍历、盘符/UNC／备用数据流写法、符号链接、Windows 目录联接和任何
//! 其他 reparse point 都不会被跟随；根目录本身被替换成链接时也拒绝读取。根外链接会在目录列表中
//! 显示目标与不可进入原因，但不会读取目标内容。
//!
//! 只读（P13、P51）：列目录与读文件只使用只读文件系统调用，不写文件、不接触 Git 索引。目录消失、
//! 根目录被替换、路径不存在或读取失败都返回明确错误，不伪装成空目录或空内容。
//!
//! 预览（P43、P44）：文本按内容识别，不依赖后缀；超过配置上限的文件只返回大小与上限说明，绝不读取
//! 完整内容；二进制文件返回不可文本预览的原因，但仍保留文件元数据。

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::workspaces::{self, RegisteredWorkspace};

/// 一个目录项的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    Directory,
    File,
    Link,
}

/// 目录里的一个条目；链接条目额外携带目标与不可进入原因。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEntry {
    pub name: String,
    /// 相对于 workspace 根的路径，统一使用 `/` 分隔。
    pub path: String,
    pub kind: EntryKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_reason: Option<String>,
}

/// 一次目录读取的结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryView {
    /// 已登记的 workspace 根路径（服务器上的路径）。
    pub workspace: String,
    /// 被读取目录相对根的路径；根目录是空字符串。
    pub path: String,
    pub entries: Vec<BrowseEntry>,
}

/// 单个文件的内容预览。
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Preview {
    /// 按内容识别为文本，附带完整内容（不超过配置上限）。
    Text { content: String },
    /// 不是可识别文本；不提供内容。
    Binary { reason: String },
    /// 超过预览上限；没有加载完整内容。
    TooLarge {
        reason: String,
        size: u64,
        limit: u64,
    },
}

/// 一次文件读取的结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub workspace: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub preview: Preview,
}

/// 公开浏览请求的明确失败类别。
#[derive(Debug)]
pub enum BrowseError {
    /// 请求的路径不在 workspace 登记列表中。
    WorkspaceNotRegistered,
    /// 登记根已消失、不可访问或不再是原来的目录（例如被替换为链接）。
    WorkspaceUnavailable(String),
    /// 绝对路径、`..` 遍历或其他越出登记根的写法。
    PathOutside,
    /// 路径不存在（包括读取前被外部删除）。
    PathNotFound,
    /// 请求的中间分量不是目录。
    NotADirectory,
    /// 清单读取的目标不是目录，或文件读取的目标是目录。
    NotAFile,
    /// 路径上出现链接／目录联接／其他 reparse point，拒绝跟随。
    Link {
        target: Option<String>,
        reason: String,
    },
    /// 路径写法本身不可接受（空分量、设备名等）。
    InvalidPath(String),
    /// 实际读取失败；不伪装成空内容。
    ReadFailed(io::Error),
}

impl BrowseError {
    /// 机器可读的失败类别码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceNotRegistered => "workspace-not-registered",
            Self::WorkspaceUnavailable(_) => "workspace-unavailable",
            Self::PathOutside => "path-outside-workspace",
            Self::PathNotFound => "path-not-found",
            Self::NotADirectory => "not-a-directory",
            Self::NotAFile => "not-a-file",
            Self::Link { .. } => "path-is-link",
            Self::InvalidPath(_) => "invalid-path",
            Self::ReadFailed(_) => "read-failed",
        }
    }

    /// 面向使用者的说明；链接错误会带上目标与拒绝跟随的原因。
    pub fn message(&self) -> String {
        match self {
            Self::WorkspaceNotRegistered => "该目录没有登记为 workspace，拒绝读取。".to_string(),
            Self::WorkspaceUnavailable(reason) => reason.clone(),
            Self::PathOutside => "路径越出了 workspace 根目录，拒绝读取。".to_string(),
            Self::PathNotFound => "路径在 workspace 中不存在或已被删除。".to_string(),
            Self::NotADirectory => "路径中的某一级不是目录。".to_string(),
            Self::NotAFile => "请求的路径不是可读取的文件。".to_string(),
            Self::Link { target, reason } => {
                let target = target
                    .as_deref()
                    .map(|target| format!("目标：{target}。"))
                    .unwrap_or_else(|| "目标无法读取。".to_string());
                format!("路径是链接或目录联接，未跟随。{target}{reason}")
            }
            Self::InvalidPath(reason) => format!("路径写法不可接受：{reason}"),
            Self::ReadFailed(error) => format!("读取 workspace 内容失败：{error}"),
        }
    }
}

/// 在登记列表中查找请求的 workspace，返回登记时的原始路径。
///
/// 查找本身不读取文件系统；根的真实性在每次读取时重新核对。
pub fn registered_root<'a>(
    workspaces: &'a [RegisteredWorkspace],
    requested: &str,
) -> Result<&'a str, BrowseError> {
    workspaces
        .iter()
        .find(|entry| workspaces::same_path(&entry.path, requested))
        .map(|entry| entry.path.as_str())
        .ok_or(BrowseError::WorkspaceNotRegistered)
}

/// 读取一个目录的直接子项（不递归），完整包含隐藏文件、忽略规则文件与被忽略的文件。
pub fn list_directory(registered_root: &str, relative: &str) -> Result<DirectoryView, BrowseError> {
    let root = checked_root(registered_root)?;
    let components = relative_components(relative)?;
    let directory = resolve_existing(&root, &components)?;
    let metadata = fs::metadata(&directory).map_err(map_metadata_error)?;
    if !metadata.is_dir() {
        return Err(BrowseError::NotAFile);
    }

    let read = fs::read_dir(&directory).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => BrowseError::PathNotFound,
        _ => BrowseError::ReadFailed(error),
    })?;

    let parent = components.join("/");
    let mut entries = Vec::new();
    for item in read {
        let item = item.map_err(BrowseError::ReadFailed)?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        let absolute = item.path();
        let metadata = fs::symlink_metadata(&absolute).map_err(BrowseError::ReadFailed)?;
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };

        if is_link_like(&metadata) {
            let (link_target, link_reason) = describe_link(&root, &absolute);
            entries.push(BrowseEntry {
                name,
                path,
                kind: EntryKind::Link,
                size: None,
                link_target,
                link_reason: Some(link_reason),
            });
        } else if metadata.is_dir() {
            entries.push(BrowseEntry {
                name,
                path,
                kind: EntryKind::Directory,
                size: None,
                link_target: None,
                link_reason: None,
            });
        } else {
            entries.push(BrowseEntry {
                name,
                path,
                kind: EntryKind::File,
                size: Some(metadata.len()),
                link_target: None,
                link_reason: None,
            });
        }
    }

    entries.sort_by(|left, right| {
        entry_rank(left.kind)
            .cmp(&entry_rank(right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(DirectoryView {
        workspace: registered_root.to_string(),
        path: parent,
        entries,
    })
}

/// 读取单个文件；大小上限在读取内容之前应用，超限时不加载完整内容。
pub fn read_file(
    registered_root: &str,
    relative: &str,
    preview_limit: u64,
) -> Result<FileView, BrowseError> {
    let root = checked_root(registered_root)?;
    let components = relative_components(relative)?;
    if components.is_empty() {
        return Err(BrowseError::NotAFile);
    }
    let path = resolve_existing(&root, &components)?;
    let metadata = fs::metadata(&path).map_err(map_metadata_error)?;
    if metadata.is_dir() {
        return Err(BrowseError::NotAFile);
    }

    let path_text = components.join("/");
    let name = components
        .last()
        .cloned()
        .unwrap_or_else(|| path_text.clone());
    let size = metadata.len();

    if size > preview_limit {
        return Ok(FileView {
            workspace: registered_root.to_string(),
            path: path_text,
            name,
            size,
            preview: Preview::TooLarge {
                reason: format!(
                    "文件大小 {size} 字节，超过预览上限 {preview_limit} 字节；未加载完整内容。"
                ),
                size,
                limit: preview_limit,
            },
        });
    }

    let file = fs::File::open(&path).map_err(BrowseError::ReadFailed)?;
    let mut bytes = Vec::new();
    file.take(preview_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(BrowseError::ReadFailed)?;

    if bytes.len() as u64 > preview_limit {
        // 文件在核对大小之后变大：仍然不把多出来的内容当成可预览文本。
        let grown = bytes.len() as u64;
        return Ok(FileView {
            workspace: registered_root.to_string(),
            path: path_text,
            name,
            size: grown,
            preview: Preview::TooLarge {
                reason: format!(
                    "文件大小 {grown} 字节，超过预览上限 {preview_limit} 字节；未加载完整内容。"
                ),
                size: grown,
                limit: preview_limit,
            },
        });
    }

    let actual_size = bytes.len() as u64;
    let preview = match classify_text(&bytes) {
        Ok(content) => Preview::Text { content },
        Err(reason) => Preview::Binary {
            reason: reason.to_string(),
        },
    };

    Ok(FileView {
        workspace: registered_root.to_string(),
        path: path_text,
        name,
        size: actual_size,
        preview,
    })
}

/// 核对登记根当前仍是原来的真实目录，并且没有被替换成链接或目录联接。
fn checked_root(registered_root: &str) -> Result<PathBuf, BrowseError> {
    let root = PathBuf::from(registered_root);
    if !root.is_absolute() {
        return Err(BrowseError::WorkspaceUnavailable(
            "workspace 登记路径不是绝对路径，拒绝读取。".to_string(),
        ));
    }

    let metadata = fs::symlink_metadata(&root).map_err(|error| {
        BrowseError::WorkspaceUnavailable(format!("workspace 目录已不存在或无法访问：{error}"))
    })?;
    if is_link_like(&metadata) {
        return Err(BrowseError::WorkspaceUnavailable(
            "workspace 根目录现在是链接或目录联接，拒绝跟随并读取。".to_string(),
        ));
    }
    if !metadata.is_dir() {
        return Err(BrowseError::WorkspaceUnavailable(
            "workspace 根目录已不再是目录，拒绝读取。".to_string(),
        ));
    }

    let canonical = fs::canonicalize(&root).map_err(|error| {
        BrowseError::WorkspaceUnavailable(format!("workspace 根目录无法解析：{error}"))
    })?;
    if !workspaces::same_path(&workspaces::display_path(&canonical), registered_root) {
        return Err(BrowseError::WorkspaceUnavailable(
            "workspace 根目录已被替换或重定向到其他位置，拒绝读取。".to_string(),
        ));
    }
    Ok(canonical)
}

/// 校验相对路径写法，返回逐级分量；任何可能逃出根的写法都在这里拒绝。
fn relative_components(relative: &str) -> Result<Vec<String>, BrowseError> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.contains('\0') {
        return Err(BrowseError::InvalidPath("路径包含空字符。".to_string()));
    }

    // Windows 上 `/` 与 `\` 都是分隔符；先统一再逐分量检查。
    let normalized = trimmed.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() || path.has_root() || has_windows_prefix(path) {
        return Err(BrowseError::PathOutside);
    }

    let parts: Vec<&str> = normalized.split('/').collect();
    let mut components = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            // 只容忍末尾分隔符；中间的空分量会让路径语义含糊。
            if index + 1 == parts.len() {
                continue;
            }
            return Err(BrowseError::InvalidPath(
                "路径包含空的目录分量。".to_string(),
            ));
        }
        if *part == "." || *part == ".." {
            return Err(BrowseError::PathOutside);
        }
        // Windows 会静默去掉路径分量末尾的空格与点；`.. `、`...` 之类的写法因此可能变成
        // 遍历。这里直接拒绝任何会被这种规范化改写的分量，避免根边界依赖文件系统的隐式别名。
        let normalized_part = part.trim_end_matches([' ', '.']);
        if normalized_part.is_empty() || normalized_part == "." || normalized_part == ".." {
            return Err(BrowseError::PathOutside);
        }
        if normalized_part.len() != part.len() {
            return Err(BrowseError::InvalidPath(format!(
                "路径分量 `{part}` 的末尾空格或点会被 Windows 改写，拒绝读取。"
            )));
        }
        if part.contains(':') {
            // 盘符、UNC 的卷语法与 NTFS 备用数据流都不属于 workspace 内的普通相对路径。
            return Err(BrowseError::PathOutside);
        }
        if is_windows_device_name(part) {
            return Err(BrowseError::InvalidPath(format!(
                "路径分量 `{part}` 是 Windows 设备名，拒绝读取。"
            )));
        }
        components.push((*part).to_string());
    }
    Ok(components)
}

/// `Path` 的 Windows 前缀（盘符或 UNC）即使不以 `is_absolute` 形式出现也不能接受。
fn has_windows_prefix(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(_)) | Some(Component::RootDir)
    )
}

/// Windows 设备名（CON、NUL、COM1…）不能被当作普通文件打开。
fn is_windows_device_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(
        base.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// 从登记根出发逐级解析一个已经存在的相对路径；链接在任何一级都拒绝跟随。
fn resolve_existing(root: &Path, components: &[String]) -> Result<PathBuf, BrowseError> {
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => BrowseError::PathNotFound,
            io::ErrorKind::NotADirectory => BrowseError::NotADirectory,
            _ => BrowseError::ReadFailed(error),
        })?;
        if is_link_like(&metadata) {
            let (target, reason) = describe_link(root, &current);
            return Err(BrowseError::Link { target, reason });
        }
        let is_last = index + 1 == components.len();
        if !is_last && !metadata.is_dir() {
            return Err(BrowseError::NotADirectory);
        }
    }

    // 最后一级可能是文件：只 canonicalize 它的父目录。这样既完成了根边界核对，又不必为了
    // 边界检查打开文件；超限文件的“不加载完整内容”因此仍是可外部观察的行为。
    let canonical = if components.is_empty() {
        fs::canonicalize(&current).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => BrowseError::PathNotFound,
            _ => BrowseError::ReadFailed(error),
        })?
    } else {
        let parent = current.parent().unwrap_or(root);
        let canonical_parent = fs::canonicalize(parent).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => BrowseError::PathNotFound,
            _ => BrowseError::ReadFailed(error),
        })?;
        canonical_parent.join(components.last().expect("非空路径必须有最后一级"))
    };
    if !is_within(&canonical, root) {
        return Err(BrowseError::PathOutside);
    }
    Ok(canonical)
}

/// `path` 是否就是 `root` 或在 `root` 之下（按路径分量比较，避免 `C:\root2` 冒充 `C:\root`）。
fn is_within(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    for root_component in root.components() {
        match path_components.next() {
            Some(component) if component_eq(component, root_component) => {}
            _ => return false,
        }
    }
    true
}

fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
    // Windows 路径大小写不敏感（包括非 ASCII 字母）；与登记路径比较时使用同一规则。
    left.as_os_str().to_string_lossy().to_lowercase()
        == right.as_os_str().to_string_lossy().to_lowercase()
}

/// 判断一个目录项是否是链接、目录联接或其他 reparse point。
///
/// Windows 上仅靠 `FileType::is_symlink` 不够：目录联接（junction）与其他 reparse point 也必须
/// 拒绝。这里直接检查 `FILE_ATTRIBUTE_REPARSE_POINT`（0x400），因此不会跟随任何重解析点。
fn is_link_like(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// 链接条目的目标与拒绝跟随原因；目标不可读取时也明确说明。
fn describe_link(root: &Path, link: &Path) -> (Option<String>, String) {
    let target = fs::read_link(link).ok().map(|target| {
        let absolute = if target.is_absolute() {
            target
        } else {
            link.parent()
                .map(|parent| parent.join(&target))
                .unwrap_or(target)
        };
        workspaces::display_path(&absolute)
    });

    let reason = match target.as_deref() {
        Some(target) => {
            let resolved = fs::canonicalize(target).ok();
            match resolved {
                Some(resolved) if !is_within(&resolved, root) => {
                    "链接指向 workspace 之外；本服务不跟随，未读取目标内容。".to_string()
                }
                Some(_) => "链接位于 workspace 内；本服务仍不跟随任何链接或目录联接。".to_string(),
                None => "链接目标不可访问；本服务不跟随，未读取目标内容。".to_string(),
            }
        }
        None => "无法读取链接目标；本服务不跟随任何链接或目录联接。".to_string(),
    };
    (target, reason)
}

/// 把元数据读取错误归一化：不存在的目标明确报不存在，其余报读取失败。
fn map_metadata_error(error: io::Error) -> BrowseError {
    match error.kind() {
        io::ErrorKind::NotFound => BrowseError::PathNotFound,
        io::ErrorKind::NotADirectory => BrowseError::NotADirectory,
        _ => BrowseError::ReadFailed(error),
    }
}

/// 按内容识别文本：只接受完整有效的 UTF-8，且拒绝 NUL 字节与大量控制字符。
fn classify_text(bytes: &[u8]) -> Result<String, &'static str> {
    // 不做 BOM 剥离：返回的文本必须与文件字节一一对应，BOM 只是开头的合法 UTF-8 字符。
    if bytes.contains(&0) {
        return Err("文件包含 NUL 字节，不是可文本预览的文本。");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "文件不是有效的 UTF-8 文本。")?;
    let mut control = 0usize;
    let mut characters = 0usize;
    for character in text.chars() {
        characters += 1;
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t' | '\u{000C}') {
            control += 1;
        }
    }
    if characters > 0 && control * 20 > characters {
        return Err("文件包含过多控制字符，不是可文本预览的文本。");
    }
    Ok(text.to_string())
}

fn entry_rank(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Link => 2,
    }
}
