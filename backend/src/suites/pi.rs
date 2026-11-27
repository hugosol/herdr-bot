//! Launch-local Pi configuration, consumed by the fixed public pi-suite factory.
//! This never redirects the native agent directory or writes project/user state.
use std::path::Path;
use crate::{paths::Roots, tasks::TaskError};
use super::{Codemode, ResourceKind, Suites};

fn builtin_args(additional: usize) -> Vec<String> {
    // --no-extensions disables builtins too; keep native MCP and tool search.
    let base = ["--extension", "builtin:mcp", "--extension", "builtin:tool-search"];
    let mut args = Vec::with_capacity(base.len() + additional);
    args.extend(base.map(str::to_owned));
    args
}

pub(super) fn default_args() -> Vec<String> {
    let mut args = builtin_args(2);
    args.extend(["--extension".into(), "builtin:codemode".into()]);
    args
}

impl Suites {
    pub(super) fn select_pi(&self, id: &str, prompts: &[String], mode: Codemode, mcp: &[String], roots: &Roots) -> Result<(Vec<String>, Vec<u8>), TaskError> {
        let mut args = builtin_args(1 + 2 * prompts.len());
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{id}」Pi 装配失败：{error}；没有回退为空环境。"));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        // No implicit template discovery and no initialization expansion. Native
        // Pi owns template names, arguments and expansion when explicitly called.
        args.push("--no-prompt-templates".into());
        for declaration in prompts {
            let path = roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))?;
            super::extensions::check_available(&path).map_err(|error| fail(error))?;
            args.extend(["--prompt-template".into(), path.to_string_lossy().into_owned()]);
        }
        let servers = self.native_configs(id, ResourceKind::Mcp, mcp, roots)?;
        let config = serde_json::json!({"version": 1, "codemode": mode, "mcpServers": servers});
        let bytes = serde_json::to_vec_pretty(&config).map_err(|error| fail(error.to_string()))?;
        Ok((args, bytes))
    }
}
