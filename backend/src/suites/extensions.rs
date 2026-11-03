//! Suite entries remain existing native OMP extensions, never copied factories
//! or a replacement module loader. Declaration paths belong to suites.json.
use std::{collections::HashSet, fs, path::Path};
use crate::{paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites};

impl Suites {
    pub(crate) fn prepare_extensions(&self, suite: Option<&Suite>, agent: Agent, roots: &Roots) -> Result<Vec<String>, TaskError> {
        let Some(suite) = suite.filter(|_| agent == Agent::Omp) else { return Ok(Vec::new()) };
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
            entries.push((entry, package));
        }
        // Publish no launch arguments until every package link and the entry
        // manifest exist. Native OMP resolves each entry (including package
        // manifests); we do not parse/rewrite JS, TS or dependency imports.
        let assembly = self.generated_directory.join(&suite.id).join("extensions").join(crate::chats::new_creation_id());
        let mut scopes = vec![self.generated_directory.clone()];
        scopes.extend(entries.iter().map(|(_, package)| package.clone()));
        self.files.with(&scopes, |access| -> Result<Vec<String>, TaskError> {
            access.create_dir_all(assembly.parent().unwrap())
                .map_err(|error| fail(format!("{}：{error}", assembly.parent().unwrap().display())))?;
            access.create_dir(&assembly).map_err(|error| fail(error.to_string()))?;
            let result = (|| -> Result<Vec<String>, String> {
                let mut args = Vec::with_capacity(entries.len() * 2);
                let mut manifest = Vec::with_capacity(entries.len());
                for (index, (entry, package)) in entries.iter().enumerate() {
                    let link = assembly.join(format!("package-{index}"));
                    access.link_resource_directory(package, &link)
                        .map_err(|error| format!("链接 {} → {} 失败：{error}", link.display(), package.display()))?;
                    let relative = entry.strip_prefix(package).map_err(|error| error.to_string())?;
                    let assembled = link.join(relative);
                    args.extend(["--extension".into(), assembled.to_string_lossy().into_owned()]);
                    manifest.push(serde_json::json!({"source":entry, "package":package, "entrypoint":assembled}));
                }
                access.create(assembly.join("omp-extensions.json"),
                    &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
                Ok(args)
            })();
            result.map_err(|error| match access.remove_dir_all(&assembly) {
                Ok(()) => fail(error),
                Err(cleanup) => fail(format!("{error}；清理本次产物另失败：{cleanup}")),
            })
        }).map_err(|error| fail(error.to_string()))?
    }
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
