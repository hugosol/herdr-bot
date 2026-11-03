//! Rebase only native fields that denote paths. Bare commands remain PATH
//! lookups; argv, environment and URLs retain their native literal semantics.
use std::path::Path;
use crate::paths::Roots;
use super::Suites;

impl Suites {
    pub(super) fn resolve_mcp_paths(&self, config: &mut serde_json::Value, roots: &Roots) -> Result<(), String> {
        let base = self.mcp_file.parent().unwrap_or(Path::new("."));
        for key in ["command", "cwd"] {
            let Some(value) = config.get(key).and_then(serde_json::Value::as_str) else { continue };
            if key == "cwd" || value.starts_with("${roots.") || value.starts_with("./")
                || value.starts_with("../") || value.starts_with(".\\") || value.starts_with("..\\") {
                let path = roots.resolve(Path::new(value), base).map_err(|error| error.to_string())?;
                config[key] = serde_json::Value::String(path.to_string_lossy().into_owned());
            }
        }
        Ok(())
    }
}
