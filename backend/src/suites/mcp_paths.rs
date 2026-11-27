//! Rebase only native fields that denote paths. Bare commands remain PATH
//! lookups; argv, environment and URLs retain their native literal semantics.
use std::path::Path;
use crate::paths::Roots;

pub(super) fn resolve_command_and_cwd(config: &mut serde_json::Value, roots: &Roots, base: &Path) -> Result<(), String> {
    resolve_command(config, roots, base)?;
    if let Some(value) = config.get("cwd").and_then(serde_json::Value::as_str) {
        config["cwd"] = resolve_path(value, roots, base)?;
    }
    Ok(())
}

pub(super) fn resolve_command(config: &mut serde_json::Value, roots: &Roots, base: &Path) -> Result<(), String> {
    if let Some(value) = config.get("command").and_then(serde_json::Value::as_str)
        && ["${roots.", "./", "../", ".\\", "..\\"].iter().any(|prefix| value.starts_with(prefix))
    {
        config["command"] = resolve_path(value, roots, base)?;
    }
    Ok(())
}

fn resolve_path(value: &str, roots: &Roots, base: &Path) -> Result<serde_json::Value, String> {
    let path = roots.resolve(Path::new(value), base).map_err(|error| error.to_string())?;
    Ok(serde_json::Value::String(path.to_string_lossy().into_owned()))
}
