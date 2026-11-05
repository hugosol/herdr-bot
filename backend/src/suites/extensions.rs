//! Native hosts load extension modules and assets at their original locations.
//! Only the declared entry is checked; Herdr is not a package loader.
use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use crate::{paths::Roots, tasks::{Agent, TaskError}};
use super::{Suite, Suites};

impl Suites {
    pub(super) fn select_extensions(&self, suite: &Suite, agent: Agent, roots: &Roots, shared: &[PathBuf]) -> Result<Vec<PathBuf>, TaskError> {
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{}」扩展入口无效：{error}；没有跳过失败资源或回退为空环境。", suite.id));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        let declarations = suite.extensions.get(agent.label()).map(Vec::as_slice).unwrap_or_default();
        let mut entries = Vec::with_capacity(shared.len() + declarations.len());
        let mut seen = HashSet::with_capacity(shared.len() + declarations.len());
        let candidates = shared.iter().cloned().map(Ok).chain(declarations.iter().map(|declaration|
            roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))));
        for entry in candidates {
            let entry = entry?;
            if entry.as_os_str().is_empty() { return Err(fail("缺少具体扩展入口".into())); }
            let canonical = fs::canonicalize(&entry).map_err(|error| fail(format!("{}：{error}", entry.display())))?;
            if !seen.insert(canonical) { continue; }
            let metadata = fs::metadata(&entry).map_err(|error| fail(error.to_string()))?;
            if metadata.is_file() {
                fs::File::open(&entry).map_err(|error| fail(format!("{}：{error}", entry.display())))?;
            } else if agent == Agent::Omp && metadata.is_dir() {
                // Native OMP supports directories. Do not inspect package contents.
                fs::read_dir(&entry).map_err(|error| fail(format!("{}：{error}", entry.display())))?;
            } else {
                return Err(fail(format!("{} 必须是可读取{}", entry.display(), if agent == Agent::Pi { "的具体文件入口（不展开 package.json）" } else { "文件或目录" })));
            }
            entries.push(entry);
        }
        Ok(entries)
    }
}

// Prompt templates retain their independent recursive availability check.
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
            return Err(format!("{} 不是可读取的普通提示模板资源", path.display()));
        }
    }
    Ok(())
}
