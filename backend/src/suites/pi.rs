//! Launch-local Pi configuration, consumed by the fixed public pi-suite factory.
//! This never redirects the native agent directory or writes project/user state.
use std::{collections::BTreeMap, path::Path};
use crate::{paths::Roots, tasks::TaskError};
use super::{Suite, Suites};

impl Suites {
    pub(crate) fn prepare_pi(&self, suite: Option<&Suite>, roots: &Roots) -> Result<Vec<String>, TaskError> {
        // --no-extensions disables builtins too. Keep native MCP even with no
        // Suite or an empty MCP selection; do not create a second MCP manager.
        let mut args = vec!["--extension".into(), "builtin:mcp".into(),
            "--extension".into(), "builtin:tool-search".into()];
        let Some(suite) = suite else {
            args.extend(["--extension".into(), "builtin:codemode".into()]);
            return Ok(args);
        };
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{}」Pi 装配失败：{error}；没有回退为空环境。", suite.id));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        let mode = &suite.pi.as_ref().ok_or_else(|| fail("必须明确设置 codemode 为 on 或 only".into()))?.codemode;
        // No implicit template discovery and no initialization expansion. Native
        // Pi owns template names, arguments and expansion when explicitly called.
        args.push("--no-prompt-templates".into());
        for declaration in &suite.prompts {
            let path = roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))?;
            super::extensions::check_available(&path).map_err(|error| fail(error))?;
            args.extend(["--prompt-template".into(), path.to_string_lossy().into_owned()]);
        }
        let catalog = self.mcp_catalog().map_err(|error| fail(error.message))?;
        let mut servers = BTreeMap::new();
        for id in &suite.mcp {
            let mut config = catalog.get(id).ok_or_else(|| fail(format!("集中 MCP 引用失效：{id}")))?.clone();
            self.resolve_mcp_paths(&mut config, roots).map_err(|error| fail(format!("MCP「{id}」：{error}")))?;
            servers.insert(id, config);
        }
        let config = serde_json::json!({"version": 1, "codemode": mode, "mcpServers": servers});
        let assembly = self.generated_directory.join(&suite.id).join("pi").join(crate::chats::new_creation_id());
        self.files.with(std::slice::from_ref(&self.generated_directory), |access| -> Result<(), TaskError> {
            access.create_dir_all(assembly.parent().unwrap()).map_err(|error| fail(error.to_string()))?;
            access.create_dir(&assembly).map_err(|error| fail(error.to_string()))?;
            let result = serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())
                .and_then(|bytes| access.create(assembly.join("pi-suite.json"), &bytes).map_err(|error| error.to_string()));
            result.map_err(|error| match access.remove_dir_all(&assembly) {
                Ok(()) => fail(error),
                Err(cleanup) => fail(format!("{error}；清理本次产物另失败：{cleanup}")),
            })
        }).map_err(|error| fail(error.to_string()))??;
        // Herdr's public agent start forwards arguments, not per-agent env.
        // The toolkit factory reads this public extension flag before MCP setup.
        args.extend(["--pi-suite-config".into(), assembly.join("pi-suite.json").to_string_lossy().into_owned()]);
        // LSP belongs only to OMP; it must not prevent other Pi resources loading.
        Ok(args)
    }
}
