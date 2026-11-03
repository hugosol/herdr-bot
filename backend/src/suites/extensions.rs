//! Suite entries remain existing native OMP extensions, never copied factories
//! or a replacement module loader. Declaration paths belong to suites.json.
use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use crate::{files::FileAccess, paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites};

impl Suites {
    pub(super) fn select_extensions(&self, suite: &Suite, agent: Agent, roots: &Roots) -> Result<Vec<(PathBuf, PathBuf)>, TaskError> {
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{}」扩展装配失败：{error}；没有跳过失败资源或回退为空环境。", suite.id));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        let declarations = suite.extensions.get(agent.label()).map(Vec::as_slice).unwrap_or_default();
        if declarations.is_empty() { return Ok(Vec::new()); }
        let mut entries = Vec::with_capacity(declarations.len());
        let mut seen = HashSet::with_capacity(declarations.len());
        for declaration in declarations {
            let entry = roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))?;
            let canonical = fs::canonicalize(&entry).map_err(|error| fail(format!("{declaration}：{error}")))?;
            if !seen.insert(canonical) { continue; }
            let package = package_root(&entry).map_err(|error| fail(format!("{declaration}：{error}")))?;
            check_available(&package).map_err(|error| fail(format!("{declaration}：{error}")))?;
            if agent == Agent::Pi && entry.is_dir() {
                // Pi's explicit -e accepts a file, not a collection directory.
                // Require the package author's precise native entry list.
                let manifest_path = entry.join("package.json");
                let manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)
                    .map_err(|error| fail(format!("{}：{error}", manifest_path.display())))?)
                    .map_err(|error| fail(format!("{}：{error}", manifest_path.display())))?;
                let declared = manifest.pointer("/pi/extensions").and_then(serde_json::Value::as_array)
                    .filter(|entries| !entries.is_empty())
                    .ok_or_else(|| fail(format!("{} 必须声明非空 pi.extensions 入口清单；裸目录不表示已加载扩展", manifest_path.display())))?;
                for relative in declared {
                    let relative = relative.as_str().filter(|value| !value.is_empty())
                        .ok_or_else(|| fail("pi.extensions 必须是具体相对文件入口清单".into()))?;
                    let relative = Path::new(relative);
                    if relative.is_absolute() || relative.components().any(|part| !matches!(part, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
                        return Err(fail(format!("Pi 包入口必须位于包内：{}", relative.display())));
                    }
                    let file = entry.join(relative);
                    if !file.is_file() { return Err(fail(format!("Pi 包入口不是可读取文件：{}", file.display()))); }
                    entries.push((file, package.clone()));
                }
            } else {
                entries.push((entry, package));
            }
        }
        Ok(entries)
    }
}

pub(super) fn assemble(access: &FileAccess, assembly: &Path, agent: Agent, entries: &[(PathBuf, PathBuf)]) -> Result<Vec<String>, String> {
    if entries.is_empty() { return Ok(Vec::new()); }
    let directory = assembly.join("extensions");
    access.create_dir(&directory).map_err(|error| error.to_string())?;
    let mut args = Vec::with_capacity(entries.len() * 2);
    let mut manifest = Vec::with_capacity(entries.len());
    for (index, (entry, package)) in entries.iter().enumerate() {
        let link = directory.join(format!("package-{index}"));
        access.link_resource_directory(package, &link)
            .map_err(|error| format!("链接 {} → {} 失败：{error}", link.display(), package.display()))?;
        let relative = entry.strip_prefix(package).map_err(|error| error.to_string())?;
        let assembled = link.join(relative);
        args.extend(["--extension".into(), assembled.to_string_lossy().into_owned()]);
        manifest.push(serde_json::json!({"source":entry, "package":package, "entrypoint":assembled}));
    }
    access.create(directory.join(format!("{}-extensions.json", agent.label())),
        &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(args)
}

// The closest existing package boundary keeps sibling modules, assets and
// node_modules together. A standalone entry retains its containing directory.
fn package_root(entry: &Path) -> Result<std::path::PathBuf, String> {
    let directory = if fs::metadata(entry).map_err(|error| error.to_string())?.is_dir() {
        entry
    } else { entry.parent().ok_or("扩展入口没有父目录")? };
    for ancestor in directory.ancestors() {
        let manifest = ancestor.join("package.json");
        match fs::metadata(&manifest) {
            Ok(metadata) if metadata.is_file() => return Ok(ancestor.to_path_buf()),
            Ok(_) => return Err(format!("{} 不是普通包清单", manifest.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(format!("{}：{error}", manifest.display())),
        }
    }
    Ok(directory.to_path_buf())
}

// External resources are read-only, outside the system's persistence boundary.
// Check bundled modules/assets too; no source import, execution or copying.
pub(super) fn check_available(entry: &Path) -> Result<(), String> {
    let mut pending = vec![entry.to_path_buf()];
    let mut visited = HashSet::new();
    while let Some(path) = pending.pop() {
        let canonical = fs::canonicalize(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        if !visited.insert(canonical) { continue; }
        let metadata = fs::metadata(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        if metadata.is_dir() {
            for child in fs::read_dir(&path).map_err(|error| format!("{}：{error}", path.display()))? {
                pending.push(child.map_err(|error| error.to_string())?.path());
            }
        } else if metadata.is_file() {
            fs::File::open(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        } else {
            return Err(format!("{} 不是可读取的普通扩展资源", path.display()));
        }
    }
    Ok(())
}
