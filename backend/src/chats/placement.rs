//! Position decisions, separate from creating or navigating a hosted Chat.
//!
//! Business scenarios stay explicit even where their current rules coincide.
//! Directory and Workspace resolution are separate steps: Resume must still
//! validate its native session and environment between them. No step creates a
//! Tab, starts an agent, persists history, or retries a failed host query.

use std::{borrow::Cow, path::Path, time::Duration};

use serde::Deserialize;

use super::herdr::{HerdrClient, HerdrError, Workspace};

/// The existing public Chat request's choice, not a Herdr object identity.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspaceTarget {
    #[default]
    Default,
    Current,
}

impl WorkspaceTarget {
    /// Resolve before launch validation, at the existing request stage. No I/O.
    pub(crate) fn name(self, zone: &str, default: &str) -> Option<String> {
        match self {
            Self::Default => Some(default.to_owned()),
            Self::Current => Path::new(zone).file_name().and_then(|name| name.to_str())
                .filter(|name| !name.is_empty()).map(str::to_owned),
        }
    }
}

/// Root Run preflight historically accepts a lossy directory name, unlike the
/// standalone Chat request. Keep that distinction rather than normalizing it.
pub(crate) fn root_map_workspace_name(zone: &str) -> Option<String> {
    Path::new(zone).file_name().map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
}

/// Inputs are already selected/approved local bindings, not Task or Run objects.
/// Node bindings may be the Run's saved bindings rather than today's zone paths.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Scenario<'a> {
    Chat { cwd: &'a str, workspace_name: &'a str },
    Chief { cwd: &'a str, workspace_name: &'a str },
    ChiefDelegatedTask { cwd: &'a str },
    RootMap { cwd: &'a str, workspace_name: &'a str },
    ChiefDelegatedMap { cwd: &'a str, workspace_name: &'a str },
    Node { cwd: &'a str, manager_workspace_id: Option<&'a str> },
    NestedMap { cwd: &'a str, manager_workspace_id: Option<&'a str> },
    Resume {
        historical_cwd: Option<&'a str>,
        original_workspace_id: Option<&'a str>,
        git_timeout: Duration,
    },
}

/// A requirement for this process, not proof of its eventual actual cwd.
#[derive(Debug)]
pub(crate) enum ExecutionDirectory<'a> {
    Explicit(Cow<'a, str>),
    HerdrSelected,
}

impl ExecutionDirectory<'_> {
    pub(crate) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::Explicit(cwd) => Some(cwd.as_ref()),
            Self::HerdrSelected => None,
        }
    }
}

/// A replacement strategy that has actually been selected, not a warning about
/// a hypothetical fallback. The caller merges these with non-position feedback.
#[derive(Debug)]
pub(crate) struct Degradation {
    pub element: &'static str,
    pub reason: String,
    pub strategy: String,
}

/// Retains the scenario and directory decision across Resume's intervening work.
#[derive(Debug)]
pub(crate) struct DirectoryMapping<'a> {
    scenario: Scenario<'a>,
    pub directory: ExecutionDirectory<'a>,
    pub degradation: Option<Degradation>,
}

#[derive(Debug)]
pub(crate) struct WorkspaceMapping {
    pub workspace: Workspace,
    pub degradation: Option<Degradation>,
}

#[derive(Debug)]
pub(crate) enum PlacementError {
    Herdr(HerdrError),
    ManagerUnassociated,
    ManagerWorkspaceUnavailable,
}

impl From<HerdrError> for PlacementError {
    fn from(error: HerdrError) -> Self { Self::Herdr(error) }
}

impl PlacementError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Herdr(error) => error.code(),
            Self::ManagerUnassociated => "manager-unassociated",
            Self::ManagerWorkspaceUnavailable => "manager-workspace-unavailable",
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::Herdr(error) => error.message(),
            Self::ManagerUnassociated => "管理者没有真实 Chat 关联".into(),
            Self::ManagerWorkspaceUnavailable => "管理者实际 Workspace 不可用；不回退默认 Workspace".into(),
        }
    }
}

impl<'a> Scenario<'a> {
    /// Launch variants only borrow their approved cwd; they perform no I/O or
    /// validation here. In particular, Run root validation stays after its
    /// Workspace lookup. Only Resume performs the existing directory fallback.
    pub(crate) async fn directory(self, herdr: &HerdrClient) -> Result<DirectoryMapping<'a>, HerdrError> {
        let (directory, degradation) = match self {
            Self::Chat { cwd, .. }
            | Self::Chief { cwd, .. }
            | Self::ChiefDelegatedTask { cwd }
            | Self::RootMap { cwd, .. }
            | Self::ChiefDelegatedMap { cwd, .. }
            | Self::Node { cwd, .. }
            | Self::NestedMap { cwd, .. } => (ExecutionDirectory::Explicit(Cow::Borrowed(cwd)), None),
            Self::Resume { historical_cwd, git_timeout, .. } => {
                recovery_directory(herdr, historical_cwd, git_timeout).await?
            }
        };
        Ok(DirectoryMapping { scenario: self, directory, degradation })
    }
}

impl DirectoryMapping<'_> {
    pub(crate) async fn workspace(&self, herdr: &HerdrClient) -> Result<WorkspaceMapping, PlacementError> {
        let workspace = match self.scenario {
            Scenario::Chat { workspace_name, .. }
            | Scenario::Chief { workspace_name, .. }
            | Scenario::RootMap { workspace_name, .. }
            | Scenario::ChiefDelegatedMap { workspace_name, .. } => herdr.workspace(workspace_name).await?,
            Scenario::ChiefDelegatedTask { .. } => herdr.workspace(herdr.workspace_name()).await?,
            Scenario::Node { manager_workspace_id, .. }
            | Scenario::NestedMap { manager_workspace_id, .. } => {
                let id = manager_workspace_id.ok_or(PlacementError::ManagerUnassociated)?;
                herdr.workspace_by_id(id).await?.ok_or(PlacementError::ManagerWorkspaceUnavailable)?
            }
            Scenario::Resume { original_workspace_id, .. } => {
                return recovery_workspace(herdr, original_workspace_id, &self.directory).await.map_err(Into::into);
            }
        };
        Ok(WorkspaceMapping { workspace, degradation: None })
    }
}

async fn recovery_directory<'a>(
    herdr: &HerdrClient,
    historical: Option<&'a str>,
    timeout: Duration,
) -> Result<(ExecutionDirectory<'a>, Option<Degradation>), HerdrError> {
    if let Some(cwd) = historical.filter(|cwd| Path::new(cwd).is_absolute()) {
        if Path::new(cwd).is_dir() {
            return Ok((ExecutionDirectory::Explicit(Cow::Borrowed(cwd)), None));
        }
        if let Some(root) = herdr.main_worktree_for(cwd, timeout).await? {
            let degradation = Degradation {
                element: "cwd",
                reason: format!("历史工作目录 {cwd} 不可用。"),
                strategy: format!("已依据准确工作树归属回退到主工作树 {root}；不修改历史会话元数据。"),
            };
            return Ok((ExecutionDirectory::Explicit(Cow::Owned(root)), Some(degradation)));
        }
    }
    Ok((ExecutionDirectory::HerdrSelected, Some(Degradation {
        element: "cwd",
        reason: format!("历史工作目录 {} 缺失或不可用，无法确认可用的主工作树。", historical.unwrap_or("（未记录）")),
        strategy: "在默认 Workspace 恢复，不指定 --cwd，由 Herdr 选择实际目录；不修改历史会话元数据。".into(),
    })))
}

async fn recovery_workspace(
    herdr: &HerdrClient,
    original_id: Option<&str>,
    directory: &ExecutionDirectory<'_>,
) -> Result<WorkspaceMapping, HerdrError> {
    let reason = match directory {
        ExecutionDirectory::HerdrSelected => "历史目录不可用且无可靠主工作树，本次使用默认 Workspace。".into(),
        ExecutionDirectory::Explicit(_) => match original_id {
            Some(id) => {
                if let Some(workspace) = herdr.workspace_by_id(id).await? {
                    return Ok(WorkspaceMapping { workspace, degradation: None });
                }
                format!("原 Workspace 已删除（ID {id}，成功查询确认）。")
            }
            None => "缺少原 Workspace 信息。".into(),
        },
    };
    let workspace = herdr.workspace(herdr.workspace_name()).await?;
    let degradation = Degradation {
        element: "workspace",
        reason,
        strategy: format!("已回退到配置的唯一默认 Workspace {}（{}）；保留历史原位置。", workspace.label, workspace.id),
    };
    Ok(WorkspaceMapping { workspace, degradation: Some(degradation) })
}
