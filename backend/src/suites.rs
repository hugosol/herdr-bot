//! Suite definitions are re-read from one user-maintained JSON authority on every
//! operation. Catalog operations never read generated output; private resource
//! adapters prepare native launch resources separately from definition storage.
mod skills;
mod mcp;
mod extensions;
mod assembly;
mod pi;
mod mcp_paths;
mod dap;

use std::{collections::{BTreeMap, HashSet}, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AppState, files::{FileAccess, FileStore}, tasks::Agent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codemode { On, Only }

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OmpSkillSource {
    Collection { path: String },
    Skill { path: String },
}

impl<'de> Deserialize<'de> for OmpSkillSource {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SourceObject;
        impl<'de> serde::de::Visitor<'de> for SourceObject {
            type Value = OmpSkillSource;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an OMP skill source object with kind and path")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Fields<'a> {
                    #[serde(borrow)]
                    kind: std::borrow::Cow<'a, str>,
                    path: String,
                }
                let Fields { kind, path } = Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                match kind.as_ref() {
                    "collection" => Ok(OmpSkillSource::Collection { path }),
                    "skill" => Ok(OmpSkillSource::Skill { path }),
                    kind => Err(serde::de::Error::unknown_variant(kind, &["collection", "skill"])),
                }
            }
        }
        // Derived internally tagged enums also accept positional arrays and numeric tags.
        deserializer.deserialize_map(SourceObject)
    }
}

impl OmpSkillSource {
    fn path(&self) -> &str {
        match self { Self::Collection { path } | Self::Skill { path } => path }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "harness", rename_all = "lowercase", deny_unknown_fields)]
pub enum Suite {
    Omp {
        id: String,
        name: String,
        #[serde(default)]
        skills: Vec<OmpSkillSource>,
        #[serde(default)]
        extensions: Vec<String>,
        #[serde(default)]
        mcp: Vec<String>,
        #[serde(default)]
        lsp: BTreeMap<String, Value>,
        #[serde(default)]
        dap: Vec<String>,
    },
    Pi {
        id: String,
        name: String,
        #[serde(default)]
        skills: Vec<String>,
        #[serde(default)]
        extensions: Vec<String>,
        #[serde(default)]
        mcp: Vec<String>,
        #[serde(default)]
        prompts: Vec<String>,
        codemode: Codemode,
    },
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Document { suites: Vec<Suite> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct McpDocument {
    #[serde(rename = "mcpServers")]
    servers: BTreeMap<String, Value>,
}

pub struct Suites {
    pub definition_file: PathBuf,
    pub mcp_file: PathBuf,
    dap_file: PathBuf,
    pub generated_directory: PathBuf,
    files: FileStore,
}

#[derive(Debug)]
pub enum SuiteErrorKind { Invalid, Storage, NotFound, Conflict }

#[derive(Debug)]
pub struct SuiteError {
    pub kind: SuiteErrorKind,
    pub code: &'static str,
    pub message: String,
}
impl SuiteError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self { kind: SuiteErrorKind::Invalid, code: "invalid-suite", message: message.into() }
    }
    fn storage(error: impl std::fmt::Display) -> Self {
        Self { kind: SuiteErrorKind::Storage, code: "suite-storage-failed", message: format!("Suite 权威文件操作失败：{error}；没有回退到其他副本。") }
    }
    fn not_found() -> Self {
        Self { kind: SuiteErrorKind::NotFound, code: "suite-not-found", message: "目标 Suite 不存在，可能已被删除。".into() }
    }
}

impl Suite {
    fn id(&self) -> &str {
        match self { Self::Omp { id, .. } | Self::Pi { id, .. } => id }
    }

    fn name(&self) -> &str {
        match self { Self::Omp { name, .. } | Self::Pi { name, .. } => name }
    }

    fn harness(&self) -> Agent {
        match self { Self::Omp { .. } => Agent::Omp, Self::Pi { .. } => Agent::Pi }
    }

    fn require_harness(&self, agent: Agent) -> Result<(), crate::tasks::TaskError> {
        if self.harness() != agent {
            return Err(crate::tasks::TaskError::InvalidSuite(format!(
                "Suite「{}」不适用于 {}；没有改选或回退为空环境。", self.id(), agent.label())));
        }
        Ok(())
    }

    fn extensions(&self) -> &[String] {
        match self { Self::Omp { extensions, .. } | Self::Pi { extensions, .. } => extensions }
    }

    fn mcp(&self) -> &[String] {
        match self { Self::Omp { mcp, .. } | Self::Pi { mcp, .. } => mcp }
    }

    fn validate(&self) -> Result<(), SuiteError> {
        let (id_suffix, name_suffix) = match self.harness() {
            Agent::Omp => ("-omp", " · OMP"),
            Agent::Pi => ("-pi", " · Pi"),
        };
        let id = self.id();
        if !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !id.strip_suffix(id_suffix).is_some_and(|base| !base.is_empty())
        {
            return Err(SuiteError::invalid(format!("Suite ID 只能包含英文字母、数字、连字符和下划线，且必须以 {id_suffix} 结尾。")));
        }
        if !self.name().strip_suffix(name_suffix).is_some_and(|base| !base.trim().is_empty()) {
            return Err(SuiteError::invalid(format!("Suite 名称必须非空且以{name_suffix} 结尾。")));
        }
        let empty_skill_path = match self {
            Self::Omp { skills, .. } => skills.iter().any(|source| source.path().trim().is_empty()),
            Self::Pi { skills, .. } => skills.iter().any(|path| path.trim().is_empty()),
        };
        if empty_skill_path {
            return Err(SuiteError::invalid("技能路径不能包含空值。"));
        }
        validate_entries("extensions", self.extensions())?;
        validate_entries("mcp", self.mcp())?;
        match self {
            Self::Pi { prompts, .. } => validate_entries("prompts", prompts)?,
            Self::Omp { lsp, dap, .. } => {
                validate_entries("dap", dap)?;
                if lsp.iter().any(|(id, config)| id.trim().is_empty() || !config.is_object()) {
                    return Err(SuiteError::invalid("LSP 声明必须是服务名到原生配置对象的 JSON 对象。"));
                }
            }
        }
        Ok(())
    }
}

fn validate_entries(name: &str, entries: &[String]) -> Result<(), SuiteError> {
    let mut seen = HashSet::new();
    if entries.iter().any(|value| value.trim().is_empty() || !seen.insert(value)) {
        return Err(SuiteError::invalid(format!("{name} 声明不能包含空值或重复项。")));
    }
    Ok(())
}

impl Suites {
    pub fn new(definition_file: PathBuf, mcp_file: PathBuf, dap_file: PathBuf, generated_directory: PathBuf) -> Self {
        Self { definition_file, mcp_file, dap_file, generated_directory, files: FileStore::default() }
    }

    pub(crate) fn selection_scope(&self) -> [PathBuf; 3] {
        [self.definition_file.clone(), self.mcp_file.clone(), self.dap_file.clone()]
    }
    fn read(&self, access: &FileAccess) -> Result<Document, SuiteError> {
        let document: Document = match access.read_to_string(&self.definition_file) {
            Ok(raw) => serde_json::from_str(&raw).map_err(SuiteError::storage)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Document::default(),
            Err(error) => return Err(SuiteError::storage(error)),
        };
        let mut seen = HashSet::new();
        for suite in &document.suites {
            suite.validate().map_err(|error| SuiteError::storage(error.message))?;
            if !seen.insert(suite.id()) { return Err(SuiteError::storage("Suite ID 重复")); }
        }
        Ok(document)
    }
    pub fn list(&self) -> Result<Vec<Suite>, SuiteError> {
        self.files.with(std::slice::from_ref(&self.definition_file), |access| self.read(access).map(|document| document.suites))
            .map_err(SuiteError::storage)?
    }
    pub fn get(&self, id: &str) -> Result<Suite, SuiteError> {
        self.list()?.into_iter().find(|suite| suite.id() == id).ok_or_else(SuiteError::not_found)
    }
    /// Read-only central native MCP definitions; never copy connection settings into Suites.
    pub fn mcp_catalog(&self) -> Result<BTreeMap<String, Value>, SuiteError> {
        self.files.with(std::slice::from_ref(&self.mcp_file), |access| self.read_mcp(access)).map_err(SuiteError::storage)?
    }
    fn read_mcp(&self, access: &FileAccess) -> Result<BTreeMap<String, Value>, SuiteError> {
        let raw = match access.read_to_string(&self.mcp_file) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(SuiteError::storage(error)),
        };
        let document: McpDocument = serde_json::from_str(&raw).map_err(SuiteError::storage)?;
        if document.servers.iter().any(|(id, config)| id.trim().is_empty() || !config.is_object()) {
            return Err(SuiteError::storage("集中 MCP 目录必须包含非空 ID 和原生配置对象。"));
        }
        Ok(document.servers)
    }
    pub(crate) fn resolve_selection(&self, access: &FileAccess, id: Option<&str>, agent: Agent) -> Result<Option<Suite>, crate::tasks::TaskError> {
        use crate::tasks::TaskError;
        let Some(id) = id else { return Ok(None) };
        let document = self.read(access).map_err(|error| TaskError::InvalidSuite(error.message))?;
        let suite = document.suites.into_iter().find(|suite| suite.id() == id)
            .ok_or_else(|| TaskError::InvalidSuite(format!("Suite「{id}」不存在；请明确修正引用，没有改选或回退为空环境。")))?;
        suite.require_harness(agent)?;
        let catalog = self.read_mcp(access).map_err(|error| TaskError::InvalidSuite(error.message))?;
        let missing: Vec<_> = suite.mcp().iter().filter(|id| !catalog.contains_key(*id)).map(String::as_str).collect();
        if !missing.is_empty() {
            return Err(TaskError::InvalidSuite(format!("Suite「{id}」的集中 MCP 引用失效：{}；没有删除引用或回退为空环境。", missing.join("、"))));
        }
        if let Suite::Omp { dap, .. } = &suite {
            self.validate_dap_references(access, id, dap).map_err(|error| TaskError::InvalidSuite(error.message))?;
        }
        Ok(Some(suite))
    }
    fn change(&self, scopes: &[PathBuf], action: impl FnOnce(&FileAccess, &mut Vec<Suite>) -> Result<(), SuiteError>) -> Result<Vec<Suite>, SuiteError> {
        self.files.with(scopes, |access| {
            access.update_json(&self.definition_file, |access| self.read(access),
                |document| action(access, &mut document.suites),
                |document| {
                    let bytes = serde_json::to_vec_pretty(document).map_err(SuiteError::storage)?;
                    if let Some(parent) = self.definition_file.parent().filter(|path| !path.as_os_str().is_empty()) {
                        access.create_dir_all(parent).map_err(SuiteError::storage)?;
                    }
                    Ok(bytes)
                },
                SuiteError::storage).map(|(document, ())| document.suites)
        }).map_err(SuiteError::storage)?
    }
    pub fn save(&self, id: Option<&str>, suite: Suite) -> Result<Vec<Suite>, SuiteError> {
        if id.is_some_and(|id| id != suite.id()) { return Err(SuiteError::invalid("编辑不能更改 Suite ID。")); }
        self.change(&self.selection_scope(), |access, suites| {
            let current = if let Some(id) = id {
                let current = suites.iter_mut().find(|suite| suite.id() == id).ok_or_else(SuiteError::not_found)?;
                if current.harness() != suite.harness() {
                    return Err(SuiteError::invalid("编辑不能更改 Suite 宿主；请为另一宿主新建 Suite。"));
                }
                Some(current)
            } else {
                None
            };
            suite.validate()?;
            let catalog = self.read_mcp(access)?;
            for reference in suite.mcp() {
                if !catalog.contains_key(reference) {
                    return Err(SuiteError::invalid(format!("Suite「{}」的集中 MCP 引用失效：{reference}；请显式修正引用。", suite.id())));
                }
            }
            if let Suite::Omp { dap, .. } = &suite {
                self.validate_dap_references(access, suite.id(), dap)?;
            }
            if let Some(current) = current {
                *current = suite;
            } else {
                if suites.iter().any(|current| current.id() == suite.id()) {
                    return Err(SuiteError { kind: SuiteErrorKind::Conflict, code: "suite-exists", message: "Suite ID 已存在。".into() });
                }
                suites.push(suite);
            }
            Ok(())
        })
    }
    pub fn delete(&self, id: &str, tasks: &crate::tasks::Tasks) -> Result<Vec<Suite>, SuiteError> {
        self.change(&[self.definition_file.clone(), tasks.storage_file().to_path_buf()], |access, suites| {
            let index = suites.iter().position(|suite| suite.id() == id).ok_or_else(SuiteError::not_found)?;
            let tasks = tasks.read(access).map_err(|error| SuiteError::storage(error.message()))?;
            let references: Vec<_> = tasks.iter().filter(|task| task.env.suite.as_deref() == Some(id))
                .map(|task| format!("{}（{}）", task.name, task.id)).collect();
            if !references.is_empty() {
                return Err(SuiteError { kind: SuiteErrorKind::Conflict, code: "suite-in-use", message: format!("Suite「{id}」仍被 Task 引用，不能删除：{}。请先显式修改这些 Task 的 Suite 选择。", references.join("、")) });
            }
            suites.remove(index);
            Ok(())
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuiteList {
    suites: Vec<Suite>,
    mcp: Vec<String>,
    dap: Vec<dap::CatalogEntry>,
    fixed_extensions: Vec<crate::tasks::catalog::FixedExtensionOption>,
}

/// Obtain all fallible response catalogs before a mutation can publish bytes.
struct SuiteCatalogs {
    mcp: Vec<String>,
    dap: Vec<dap::CatalogEntry>,
    fixed_extensions: Vec<crate::tasks::catalog::FixedExtensionOption>,
}
impl SuiteCatalogs {
    fn load(state: &AppState) -> Result<Self, SuiteError> {
        let mcp = state.suites.mcp_catalog()?.into_keys().collect();
        let dap = state.suites.dap_catalog()?;
        let extensions = state.tasks.suite_extensions().map_err(|error| SuiteError {
            kind: SuiteErrorKind::Storage,
            code: error.code(),
            message: error.message(),
        })?;
        Ok(Self { mcp, dap, fixed_extensions: extensions })
    }
    fn listing(self, suites: Vec<Suite>) -> SuiteList {
        SuiteList { suites, mcp: self.mcp, dap: self.dap, fixed_extensions: self.fixed_extensions }
    }
}
pub(crate) fn listing(state: &AppState) -> Result<SuiteList, SuiteError> {
    let catalogs = SuiteCatalogs::load(state)?;
    Ok(catalogs.listing(state.suites.list()?))
}

pub(crate) fn save(state: &AppState, id: Option<&str>, suite: Suite) -> Result<SuiteList, SuiteError> {
    let catalogs = SuiteCatalogs::load(state)?;
    Ok(catalogs.listing(state.suites.save(id, suite)?))
}

pub(crate) fn remove(state: &AppState, id: &str) -> Result<SuiteList, SuiteError> {
    let catalogs = SuiteCatalogs::load(state)?;
    Ok(catalogs.listing(state.suites.delete(id, &state.tasks)?))
}
