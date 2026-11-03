//! Suite entries remain existing native OMP extensions, never copied factories
//! or a replacement module loader. Declaration paths belong to suites.json.
use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use crate::{files::FileAccess, paths::Roots, tasks::TaskError};
use super::{Suite, Suites};

impl Suites {
    pub(super) fn select_extensions(&self, suite: &Suite, roots: &Roots) -> Result<Vec<(PathBuf, PathBuf)>, TaskError> {
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{}」扩展装配失败：{error}；没有跳过失败资源或回退为空环境。", suite.id));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        if suite.extensions.is_empty() { return Ok(Vec::new()); }
        let mut entries = Vec::with_capacity(suite.extensions.len());
        let mut seen = HashSet::with_capacity(suite.extensions.len());
        for declaration in &suite.extensions {
            let entry = roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))?;
            let canonical = fs::canonicalize(&entry).map_err(|error| fail(format!("{declaration}：{error}")))?;
            if !seen.insert(canonical) { continue; }
            let package = package_root(&entry).map_err(|error| fail(format!("{declaration}：{error}")))?;
            check_available(&package).map_err(|error| fail(format!("{declaration}：{error}")))?;
            entries.push((entry, package));
        }
        Ok(entries)
    }
}

pub(super) fn assemble(access: &FileAccess, assembly: &Path, entries: &[(PathBuf, PathBuf)]) -> Result<Vec<String>, String> {
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
    access.create(directory.join("omp-extensions.json"),
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
fn check_available(entry: &Path) -> Result<(), String> {
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
