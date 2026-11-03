//! Suite definitions are re-read from one user-maintained JSON authority on every
//! operation. Generated host resources are not read or written by this catalog.
use std::{collections::{BTreeMap, HashSet}, io, path::PathBuf};

use axum::{Json, extract::{Path, Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AppState, files::{FileAccess, FileStore}, tasks::Agent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codemode { On, Only }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PiConfig { pub codemode: Codemode }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub id: String,
    pub name: String,
    pub agents: Vec<Agent>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub prompts: Vec<String>,
    #[serde(default)]
    pub lsp: BTreeMap<String, Value>,
    #[serde(default)]
    pub mcp: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi: Option<PiConfig>,
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
    files: FileStore,
}

#[derive(Debug)]
pub struct SuiteError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl SuiteError {
    fn invalid(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, code: "invalid-suite", message: message.into() }
    }
    fn storage(error: impl std::fmt::Display) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, code: "suite-storage-failed", message: format!("Suite 权威文件操作失败：{error}；没有回退到其他副本。") }
    }
    fn not_found() -> Self {
        Self { status: StatusCode::NOT_FOUND, code: "suite-not-found", message: "目标 Suite 不存在，可能已被删除。".into() }
    }
}
impl IntoResponse for SuiteError {
    fn into_response(self) -> Response {
        crate::rejected_owned(self.status, self.code, self.message)
    }
}

impl Suite {
    fn validate(&self) -> Result<(), SuiteError> {
        if self.id.is_empty() || !self.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            return Err(SuiteError::invalid("Suite ID 只能包含英文字母、数字、连字符和下划线，且不能为空。"));
        }
        if self.name.trim().is_empty() { return Err(SuiteError::invalid("Suite 名称不能为空。")); }
        if self.agents.is_empty() || self.agents.iter().enumerate().any(|(i, agent)| self.agents[..i].contains(agent)) {
            return Err(SuiteError::invalid("请选择至少一个宿主，不能重复。"));
        }
        if self.agents.contains(&Agent::Pi) != self.pi.is_some() {
            return Err(SuiteError::invalid("Pi 适用 Suite 必须明确设置 codemode 为 on 或 only；OMP-only Suite 不接受 Pi 配置。"));
        }
        for (name, values) in [("skills", &self.skills), ("extensions", &self.extensions), ("prompts", &self.prompts), ("mcp", &self.mcp)] {
            let mut seen = HashSet::new();
            if values.iter().any(|value| value.trim().is_empty() || !seen.insert(value)) {
                return Err(SuiteError::invalid(format!("{name} 声明不能包含空值或重复项。")));
            }
        }
        if self.lsp.iter().any(|(id, config)| id.trim().is_empty() || !config.is_object()) {
            return Err(SuiteError::invalid("LSP 声明必须是服务名到原生配置对象的 JSON 对象。"));
        }
        Ok(())
    }
}

impl Suites {
    pub fn new(definition_file: PathBuf, mcp_file: PathBuf) -> Self {
        Self { definition_file, mcp_file, files: FileStore::default() }
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
            if !seen.insert(&suite.id) { return Err(SuiteError::storage("Suite ID 重复")); }
        }
        Ok(document)
    }
    pub fn list(&self) -> Result<Vec<Suite>, SuiteError> {
        self.files.with(std::slice::from_ref(&self.definition_file), |access| self.read(access).map(|document| document.suites))
            .map_err(SuiteError::storage)?
    }
    pub fn get(&self, id: &str) -> Result<Suite, SuiteError> {
        self.list()?.into_iter().find(|suite| suite.id == id).ok_or_else(SuiteError::not_found)
    }
    /// Read-only central native MCP definitions; never copy connection settings into Suites.
    pub fn mcp_catalog(&self) -> Result<BTreeMap<String, Value>, SuiteError> {
        let raw = match self.files.read_to_string(&self.mcp_file) {
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
    fn change(&self, action: impl FnOnce(&mut Vec<Suite>) -> Result<(), SuiteError>) -> Result<Vec<Suite>, SuiteError> {
        self.files.with(std::slice::from_ref(&self.definition_file), |access| {
            access.update_json(&self.definition_file, |access| self.read(access),
                |document| action(&mut document.suites),
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
        suite.validate()?;
        if id.is_some_and(|id| id != suite.id) { return Err(SuiteError::invalid("编辑不能更改 Suite ID。")); }
        self.change(|suites| {
            if let Some(id) = id {
                *suites.iter_mut().find(|suite| suite.id == id).ok_or_else(SuiteError::not_found)? = suite;
            } else {
                if suites.iter().any(|current| current.id == suite.id) {
                    return Err(SuiteError { status: StatusCode::CONFLICT, code: "suite-exists", message: "Suite ID 已存在。".into() });
                }
                suites.push(suite);
            }
            Ok(())
        })
    }
    pub fn delete(&self, id: &str) -> Result<Vec<Suite>, SuiteError> {
        self.change(|suites| {
            let index = suites.iter().position(|suite| suite.id == id).ok_or_else(SuiteError::not_found)?;
            suites.remove(index);
            Ok(())
        })
    }
}

#[derive(Serialize)]
struct SuiteList { suites: Vec<Suite>, mcp: Vec<String> }
fn listing(suites: &Suites, definitions: Vec<Suite>) -> Result<Json<SuiteList>, SuiteError> {
    Ok(Json(SuiteList { suites: definitions, mcp: suites.mcp_catalog()?.into_keys().collect() }))
}
pub async fn list(State(state): State<AppState>) -> Result<impl IntoResponse, SuiteError> {
    listing(&state.suites, state.suites.list()?)
}
pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Result<impl IntoResponse, SuiteError> {
    Ok(Json(serde_json::json!({ "suite": state.suites.get(&id)? })))
}
async fn input(request: Request) -> Result<Suite, SuiteError> {
    if request.uri().query().is_some() { return Err(SuiteError::invalid("Suite 命令不接受查询参数。")); }
    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024).await.map_err(|error| SuiteError::invalid(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| SuiteError::invalid(error.to_string()))
}
pub async fn create(State(state): State<AppState>, request: Request) -> Result<impl IntoResponse, SuiteError> {
    let suite = input(request).await?;
    let mcp = state.suites.mcp_catalog()?.into_keys().collect();
    let suites = state.suites.save(None, suite)?;
    Ok(Json(SuiteList { suites, mcp }))
}
pub async fn update(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Result<impl IntoResponse, SuiteError> {
    let suite = input(request).await?;
    let mcp = state.suites.mcp_catalog()?.into_keys().collect();
    let suites = state.suites.save(Some(&id), suite)?;
    Ok(Json(SuiteList { suites, mcp }))
}
pub async fn delete(State(state): State<AppState>, Path(id): Path<String>, request: Request) -> Result<impl IntoResponse, SuiteError> {
    if request.uri().query().is_some() { return Err(SuiteError::invalid("删除 Suite 不接受查询参数。")); }
    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024).await.map_err(|error| SuiteError::invalid(error.to_string()))?;
    if !bytes.is_empty() { return Err(SuiteError::invalid("删除 Suite 不接受请求体。")); }
    let mcp = state.suites.mcp_catalog()?.into_keys().collect();
    let suites = state.suites.delete(&id)?;
    Ok(Json(SuiteList { suites, mcp }))
}
