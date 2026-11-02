//! Explicit offline zone cutover. Normal startup never guesses IDs or rewrites history.
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use anyhow::{Context, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::files::{FileAccess, FileStore};

/// An explicit association supplied by the operator, not inferred from directory names.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneMapping {
    pub path: String,
    pub id: String,
    /// Optional explicitly supplied current-host binding, also shared by aliases.
    #[serde(default)]
    pub local_path: Option<String>,
}

struct Change { path: PathBuf, original: Vec<u8>, replacement: Vec<u8> }

/// Stop all backend processes before invoking. Validates all inputs before writing.
/// Originals remain in sibling `.zone-portability-v1.bak` files. Publication is
/// atomic per file, not across files. Run and Chat records are never opened.
pub fn migrate(registry: &Path, maps: &Path, mapping_file: &Path) -> anyhow::Result<usize> {
    let registry = std::path::absolute(registry)?;
    let maps = std::path::absolute(maps)?;
    let mapping_file = std::path::absolute(mapping_file)?;
    ensure!(!registry.starts_with(&maps) && !mapping_file.starts_with(&maps), "registry and mapping file must be outside the Map directory");
    let registry_scope = registry.parent().context("registry has no parent directory")?.to_owned();
    FileStore.with(&[registry_scope, maps.clone(), mapping_file.clone()], |files| {
        let mappings: Vec<ZoneMapping> = serde_json::from_slice(&files.read(&mapping_file)?)
            .context("mapping must be a JSON array of {\"path\":\"old absolute directory\",\"id\":\"stable ID\"}")?;
        let mut by_path = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        for mapping in &mappings {
            ensure!(!mapping.id.trim().is_empty(), "mapping ID must be nonempty: {:?}", mapping.id);
            let path = absolute(&mapping.path).with_context(|| format!("mapping path must be absolute without traversal: {}", mapping.path))?;
            if let Some(previous) = by_path.insert(path, mapping.id.clone()) {
                ensure!(previous == mapping.id, "ambiguous mapping for {}; provide exactly one ID per old path", mapping.path);
            }
            let binding = mapping.local_path.as_deref().unwrap_or(&mapping.path);
            let normalized = absolute(binding).with_context(|| format!("local_path must be absolute without traversal: {binding}"))?;
            if let Some((previous, _)) = by_id.get(&mapping.id) {
                ensure!(previous == &normalized, "conflicting local bindings for ID {}; supply the same explicit local_path for every old alias", mapping.id);
            } else { by_id.insert(mapping.id.clone(), (normalized, binding.to_owned())); }
        }
        let mut changes = Vec::new();
        if files.try_exists(&registry)? {
            let mut document = load(files, &registry)?;
            let zones = document.get_mut("zones").and_then(Value::as_array_mut).context("registry must contain a zones array")?;
            let mut ids = BTreeMap::new();
            for zone in zones {
                let path = zone.as_str().or_else(|| zone.get("path").and_then(Value::as_str)).context("registry zone must be a path string or an object containing path")?.to_owned();
                absolute(&path).with_context(|| format!("registry zone is not absolute: {path}"))?;
                let id = if let Some(id) = zone.get("id").and_then(Value::as_str) {
                    ensure!(!id.trim().is_empty(), "existing registry ID must be nonempty");
                    id.to_owned()
                } else { lookup(&by_path, &path)?.to_owned() };
                let binding = by_id.get(&id).map(|(_, binding)| binding.as_str()).unwrap_or(&path);
                ensure!(ids.insert(id.clone(), binding.to_owned()).is_none(), "duplicate registry ID {id}; reconcile duplicate legacy entries explicitly");
                *zone = json!({"id":id,"path":binding});
            }
            record(files, &mut changes, &registry, document)?;
        }
        regular(files, &maps, true)?;
        let mut entries = files.read_dir(&maps)?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("json")) { continue; }
            let mut map = load(files, &path)?;
            let nodes = map.get_mut("nodes").and_then(Value::as_object_mut).with_context(|| format!("Map {} must contain nodes object", path.display()))?;
            for (node_id, node) in nodes {
                let context = format!("{} node {node_id}", path.display());
                let zone = node.get_mut("zone").context(format!("{context}: missing zone"))?;
                let old = zone.as_str().context(format!("{context}: zone must be a string"))?;
                if !by_id.contains_key(old) && (old.starts_with('/') || old.starts_with('\\') || old.as_bytes().get(1) == Some(&b':')) {
                    *zone = Value::String(lookup(&by_path, old).with_context(|| context.clone())?.to_owned());
                }
                if let Some(rules) = node.get_mut("input_rule") {
                    for rule in rules.as_array_mut().context(format!("{context}: input_rule must be an array"))? {
                        if rule.get("type").and_then(Value::as_str) != Some("file") { continue; }
                        let source = rule.get("path").and_then(Value::as_str).context(format!("{context}: File input needs path"))?;
                        if let Some(source) = absolute(source) {
                            let explicit = rule.get("zone_id").map(|zone| zone.as_str().filter(|zone| !zone.trim().is_empty()).context(format!("{context}: explicit File zone_id must be a nonempty string"))).transpose()?;
                            let candidates: Vec<_> = by_path.iter().filter_map(|(root, id)| {
                                if explicit.is_some_and(|explicit| explicit != id) { return None; }
                                let relative = if root == "/" { source.strip_prefix('/') } else { source.strip_prefix(root.as_str()).and_then(|suffix| suffix.strip_prefix('/')) };
                                relative.filter(|suffix| !suffix.is_empty()).map(|suffix| (id, suffix))
                            }).collect();
                            ensure!(candidates.len() == 1, "{context}: absolute File input {source:?} has {} matching mappings; supply exactly one enclosing path->ID mapping, or set zone_id explicitly to disambiguate overlapping roots", candidates.len());
                            let (id, relative) = candidates[0];
                            rule["path"] = Value::String(relative.to_owned());
                            rule["zone_id"] = Value::String(id.clone());
                        } else {
                            ensure!(!source.is_empty() && !source.contains('\\') && !source.starts_with('/') && !source.contains(':') && source.split('/').all(|part| !part.is_empty() && part != "." && part != ".."), "{context}: File path {source:?} is neither an absolute migratable path nor a portable relative '/' path");
                        }
                    }
                }
            }
            record(files, &mut changes, &path, map)?;
        }
        // Check every backup before publishing anything, including a resumed cutover.
        for change in &changes {
            let backup = backup(&change.path);
            if files.try_exists(&backup)? {
                regular(files, &backup, false)?;
                ensure!(files.read(&backup)? == change.original, "backup differs from current source: {}; restore or reconcile it before retrying", backup.display());
            }
        }
        for change in &changes {
            let backup = backup(&change.path);
            if !files.try_exists(&backup)? { files.create(&backup, &change.original)?; }
        }
        for change in &changes {
            files.replace(&change.path, &change.replacement).with_context(|| format!("publication failed at {}; some earlier files may already be migrated; retain backups and rerun after resolving the error", change.path.display()))?;
        }
        Ok(changes.len())
    })?
}

fn regular(files: &FileAccess, path: &Path, directory: bool) -> anyhow::Result<()> {
    let metadata = files.symlink_metadata(path)?;
    ensure!(!crate::zones::browse::is_link_like(&metadata), "links/reparse points are forbidden: {}", path.display());
    ensure!(if directory { metadata.is_dir() } else { metadata.is_file() }, "unexpected file type: {}", path.display());
    Ok(())
}
fn load(files: &FileAccess, path: &Path) -> anyhow::Result<Value> {
    regular(files, path, false)?;
    serde_json::from_slice(&files.read(path)?).with_context(|| format!("invalid JSON: {}", path.display()))
}
fn record(files: &FileAccess, changes: &mut Vec<Change>, path: &Path, value: Value) -> anyhow::Result<()> {
    let original = files.read(path)?;
    if serde_json::from_slice::<Value>(&original)? != value {
        let mut replacement = serde_json::to_vec_pretty(&value)?; replacement.push(b'\n');
        changes.push(Change { path: path.to_owned(), original, replacement });
    }
    Ok(())
}
fn backup(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned(); name.push(".zone-portability-v1.bak"); PathBuf::from(name)
}
fn lookup<'a>(mappings: &'a BTreeMap<String, String>, path: &str) -> anyhow::Result<&'a str> {
    let normalized = absolute(path).context("expected an absolute path without traversal")?;
    match mappings.get(&normalized) {
        Some(id) => Ok(id),
        None => bail!("missing mapping for {path:?}; add {{\"path\":{path:?},\"id\":\"your/stable-id\"}} to the mapping JSON and rerun; no ID is inferred"),
    }
}
// Lexical only: migration must also work after the old host/directories are gone.
// Preserve case: differently cased paths require explicit operator associations.
fn absolute(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let absolute = path.starts_with('/') || (path.len() >= 3 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/');
    if !absolute || path.split('/').any(|part| part == "." || part == "..") { return None; }
    let path = path.trim_end_matches('/');
    Some(if path.is_empty() { "/".to_owned() } else { path.to_owned() })
}
