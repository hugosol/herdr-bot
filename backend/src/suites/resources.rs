use std::{collections::BTreeMap, io, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{files::{FileAccess, PublishMode}, paths::Roots, tasks::TaskError};

use super::{Suite, SuiteError, SuiteErrorKind, Suites, safe_id};

#[derive(Clone, Copy)]
pub(crate) enum ResourceKind { Mcp, Lsp, Dap }

impl ResourceKind {
    pub(crate) fn parse(kind: &str) -> Result<Self, SuiteError> {
        match kind {
            "mcp" => Ok(Self::Mcp),
            "lsp" => Ok(Self::Lsp),
            "dap" => Ok(Self::Dap),
            _ => Err(SuiteError::invalid("资源类型必须是 mcp、lsp 或 dap。")),
        }
    }

    fn label(self) -> &'static str {
        match self { Self::Mcp => "MCP", Self::Lsp => "LSP", Self::Dap => "DAP" }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceDefinition {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) config: Value,
}

impl ResourceDefinition {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err("资源定义必须是 JSON 对象。".into());
        }
        serde_json::from_slice(bytes).map_err(|error| error.to_string())
    }

    fn validate(&self, kind: ResourceKind) -> Result<(), String> {
        if !self.config.is_object() {
            return Err("config 必须是原生配置对象".into());
        }
        if let ResourceKind::Dap = kind { super::dap::validate(self)?; }
        Ok(())
    }
}

#[derive(Serialize)]
pub(super) struct ResourceOption {
    pub(super) id: String,
    #[serde(flatten)]
    pub(super) definition: ResourceDefinition,
}

#[derive(Serialize)]
enum NativeDocument<'a> {
    #[serde(rename = "mcpServers")]
    Mcp(BTreeMap<&'a str, Value>),
    #[serde(rename = "servers")]
    Lsp(BTreeMap<&'a str, Value>),
    #[serde(rename = "adapters")]
    Dap(BTreeMap<&'a str, Value>),
}

impl Suite {
    pub(super) fn resource_selections(&self) -> [(ResourceKind, &[String]); 3] {
        let (lsp, dap) = match self {
            Self::Omp { lsp, dap, .. } => (lsp.as_slice(), dap.as_slice()),
            Self::Pi { .. } => (&[][..], &[][..]),
        };
        [(ResourceKind::Mcp, self.mcp()), (ResourceKind::Lsp, lsp), (ResourceKind::Dap, dap)]
    }
}

impl Suites {
    pub(super) fn native_document(&self, suite: &str, kind: ResourceKind, selected: &[String], roots: &Roots) -> Result<Option<Vec<u8>>, TaskError> {
        if selected.is_empty() { return Ok(None); }
        let configs = self.native_configs(suite, kind, selected, roots)?;
        let document = match kind {
            ResourceKind::Mcp => NativeDocument::Mcp(configs),
            ResourceKind::Lsp => NativeDocument::Lsp(configs),
            ResourceKind::Dap => NativeDocument::Dap(configs),
        };
        serde_json::to_vec_pretty(&document).map(Some).map_err(|error| {
            TaskError::InvalidSuite(format!("Suite「{suite}」{} 装配失败：{error}；没有回退为空环境。", kind.label()))
        })
    }

    fn resource_directory(&self, kind: ResourceKind) -> &Path {
        match kind {
            ResourceKind::Mcp => &self.mcp_directory,
            ResourceKind::Lsp => &self.lsp_directory,
            ResourceKind::Dap => &self.dap_directory,
        }
    }

    pub(super) fn read_resources(&self, access: &FileAccess, kind: ResourceKind) -> Result<Vec<ResourceOption>, SuiteError> {
        let directory = self.resource_directory(kind);
        let fail = |error: &dyn std::fmt::Display| {
            SuiteError::storage(format!("集中 {} 目录 {}：{error}", kind.label(), directory.display()))
        };
        let entries = match access.read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(fail(&error)),
        };
        let mut options = Vec::new();
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json")
                || !entry.file_type().map_err(|error| fail(&error))?.is_file()
            {
                continue;
            }
            let id = path.file_stem().and_then(|name| name.to_str()).filter(|id| safe_id(id))
                .ok_or_else(|| fail(&format!("资源文件 ID 无效：{}", path.display())))?;
            let raw = access.read(&path).map_err(|error| fail(&error))?;
            let definition = ResourceDefinition::parse(&raw).and_then(|definition| {
                definition.validate(kind)?;
                Ok(definition)
            }).map_err(|error| fail(&format!("资源「{id}」：{error}")))?;
            options.push(ResourceOption { id: id.to_owned(), definition });
        }
        options.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        Ok(options)
    }

    pub(crate) fn mcp_ids(&self) -> Result<Vec<String>, SuiteError> {
        self.files.with(&[self.mcp_directory.clone()], |access| {
            self.read_resources(access, ResourceKind::Mcp).map(|options| options.into_iter().map(|option| option.id).collect())
        }).map_err(SuiteError::storage)?
    }

    pub(super) fn validate_references(&self, access: &FileAccess, suite: &Suite) -> Result<(), SuiteError> {
        for (kind, selected) in suite.resource_selections() {
            if selected.is_empty() { continue; }
            let catalog = self.read_resources(access, kind)?;
            validate_reference_ids(suite.id(), kind, selected, &catalog)?;
        }
        Ok(())
    }

    pub(super) fn native_configs<'a>(&self, suite: &str, kind: ResourceKind, selected: &'a [String], roots: &Roots) -> Result<BTreeMap<&'a str, Value>, TaskError> {
        if selected.is_empty() { return Ok(BTreeMap::new()); }
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{suite}」{} 装配失败：{error}；没有回退为空环境。", kind.label()));
        let directory = self.resource_directory(kind);
        self.files.with(&[directory.to_path_buf()], |access| {
            let mut catalog = self.read_resources(access, kind).map_err(|error| fail(error.message))?;
            let mut configs = BTreeMap::new();
            for id in selected {
                let index = catalog.binary_search_by(|option| option.id.cmp(id))
                    .map_err(|_| fail(missing_reference(suite, kind, id).message))?;
                let mut config = std::mem::take(&mut catalog[index].definition.config);
                // Rebase executable paths (not args, marker names or native payloads).
                use super::mcp_paths::{resolve_command, resolve_command_and_cwd};
                match kind {
                    ResourceKind::Dap => resolve_command(&mut config, roots, directory),
                    ResourceKind::Mcp | ResourceKind::Lsp => resolve_command_and_cwd(&mut config, roots, directory),
                }.map_err(|error| fail(format!("资源「{id}」：{error}")))?;
                configs.insert(id.as_str(), config);
            }
            Ok(configs)
        }).map_err(|error| fail(error.to_string()))?
    }

    pub(super) fn replace_resource(&self, access: &FileAccess, kind: ResourceKind, id: &str, definition: &ResourceDefinition) -> Result<(), SuiteError> {
        definition.validate(kind).map_err(SuiteError::invalid)?;
        let path = self.resource_directory(kind).join(format!("{id}.json"));
        let bytes = serde_json::to_vec_pretty(definition).map_err(SuiteError::storage)?;
        access.publish_checked(&path, &bytes, PublishMode::Replace, |access| {
            match access.symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => Ok(()),
                Ok(_) => Err(resource_not_found()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Err(resource_not_found()),
                Err(error) => Err(SuiteError::storage(error)),
            }
        }, SuiteError::storage)
    }
}

pub(super) fn validate_reference_ids(suite: &str, kind: ResourceKind, selected: &[String], options: &[ResourceOption]) -> Result<(), SuiteError> {
    for id in selected {
        if options.binary_search_by(|option| option.id.cmp(id)).is_err() {
            return Err(missing_reference(suite, kind, id));
        }
    }
    Ok(())
}

fn missing_reference(suite: &str, kind: ResourceKind, id: &str) -> SuiteError {
    SuiteError::invalid(format!("Suite「{suite}」的集中 {} 引用失效：{id}；请显式修正引用。", kind.label()))
}

pub(super) fn resource_not_found() -> SuiteError {
    SuiteError { kind: SuiteErrorKind::NotFound, code: "suite-resource-not-found", message: "目标 Suite 资源不存在。".into() }
}
