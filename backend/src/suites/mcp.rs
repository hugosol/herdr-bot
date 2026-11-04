//! Native OMP resource-only package: discovery and the complete MCP runtime
//! belong to OMP. Do not read, merge, or rewrite user/project MCP definitions.
use std::{collections::BTreeMap, path::Path};
use serde_json::{Value, json};
use crate::{paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites};

impl Suites {
    pub(super) fn select_mcp(&self, suite: &Suite, agent: Agent, roots: &Roots) -> Result<Option<Vec<u8>>, TaskError> {
        if agent != Agent::Omp || suite.mcp.is_empty() { return Ok(None); }
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{}」MCP 装配失败：{error}；没有回退为空环境。", suite.id));
        let scopes = [self.mcp_file.clone()];
        self.files.with(&scopes, |access| {
            let catalog = self.read_mcp(access).map_err(|error| fail(error.message))?;
            let mut selected = BTreeMap::new();
            let base = self.mcp_file.parent().unwrap_or(Path::new("."));
            for id in &suite.mcp {
                let mut config = catalog.get(id).cloned().ok_or_else(|| fail(format!("集中 MCP 引用失效：{id}")))?;
                // Preserve declaration-relative path semantics after publishing
                // the native config elsewhere; leave all native options intact.
                for field in ["command", "cwd"] {
                    if let Some(value) = config.get(field).and_then(Value::as_str) {
                        let path_like = value.starts_with("./") || value.starts_with("../")
                            || value.starts_with(".\\") || value.starts_with("..\\");
                        if field == "cwd" || path_like || value.starts_with("${roots.") {
                            let resolved = roots.resolve(Path::new(value), base).map_err(|error| fail(error.to_string()))?;
                            config[field] = Value::String(resolved.to_string_lossy().into_owned());
                        }
                    }
                }
                selected.insert(id, config);
            }
            let bytes = serde_json::to_vec_pretty(&json!({"mcpServers":selected})).map_err(|error| fail(error.to_string()))?;
            Ok(Some(bytes))
        }).map_err(|error| fail(error.to_string()))?
    }
}
