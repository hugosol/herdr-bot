//! Startup-only machine roots and source-relative path resolution.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::files::FileStore;

/// Fully resolved machine roots. Clones retain the startup snapshot, never reread the file.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(transparent)]
pub struct Roots(BTreeMap<String, PathBuf>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootsFile {
    roots: BTreeMap<String, PathBuf>,
}

impl Roots {
    pub fn load(path: &Path) -> Result<Self> {
        let path = absolute(path, &std::env::current_dir()?)?;
        let raw = FileStore::default().read_to_string(&path)
            .with_context(|| format!("读取路径根配置失败：{}", path.display()))?;
        let source: RootsFile = toml::from_str(&raw)
            .with_context(|| format!("解析路径根配置失败：{}", path.display()))?;
        let base = path.parent().context("路径根配置没有父目录")?;
        let mut resolved = BTreeMap::new();
        let mut visiting = BTreeSet::new();
        for name in source.roots.keys() {
            resolve_root(name, &source.roots, &mut resolved, &mut visiting, base)
                .with_context(|| format!("路径根配置 {}", path.display()))?;
        }
        Ok(Self(resolved))
    }

    pub fn resolve(&self, value: &Path, base: &Path) -> Result<PathBuf> {
        match expression(value)? {
            Some((name, suffix)) => {
                let root = self.0.get(name).with_context(|| format!("未知路径根：{name}"))?;
                absolute(Path::new(suffix), root)
            }
            None => absolute(value, base),
        }
    }

    /// A bare executable name remains a PATH lookup; every path is resolved normally.
    pub fn command(&self, value: &Path, base: &Path) -> Result<PathBuf> {
        let expression = expression(value)?;
        let text = value.to_str().context("命令路径必须是 UTF-8")?;
        if expression.is_none() && !text.contains(['/', '\\'])
            && matches!(value.components().next(), Some(Component::Normal(_)))
            && !value.is_absolute()
        {
            return Ok(value.to_path_buf());
        }
        self.resolve(value, base)
    }
}

fn resolve_root(
    name: &str,
    source: &BTreeMap<String, PathBuf>,
    resolved: &mut BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<String>,
    base: &Path,
) -> Result<PathBuf> {
    if let Some(path) = resolved.get(name) { return Ok(path.clone()); }
    let value = source.get(name).with_context(|| format!("未知路径根：{name}"))?;
    if name.is_empty() { bail!("路径根名称不能为空"); }
    if !visiting.insert(name.to_owned()) { bail!("路径根循环引用：{name}"); }
    let path = match expression(value).with_context(|| format!("路径根 {name}"))? {
        Some((dependency, suffix)) => {
            let root = resolve_root(dependency, source, resolved, visiting, base)?;
            absolute(Path::new(suffix), &root)?
        }
        None => absolute(value, base)?,
    };
    visiting.remove(name);
    resolved.insert(name.to_owned(), path.clone());
    Ok(path)
}

/// Expressions occupy only the path prefix, followed by an optional `/relative` suffix.
fn expression(value: &Path) -> Result<Option<(&str, &str)>> {
    let text = value.to_str().context("路径必须是 UTF-8")?;
    if text.trim().is_empty() { bail!("路径不能为空或纯空白"); }
    if let Some(rest) = text.strip_prefix("${roots.") {
        let (name, tail) = rest.split_once('}').context("非法路径根表达式：缺少 }")?;
        if name.is_empty() || name.contains(['$', '{', '}']) { bail!("非法路径根表达式：根名称不能为空或包含表达式"); }
        let suffix = if tail.is_empty() { "" } else {
            tail.strip_prefix('/').context("路径根表达式后只能附加 /relative 路径")?
        };
        if suffix.contains("${") || Path::new(suffix).has_root()
            || matches!(Path::new(suffix).components().next(), Some(Component::Prefix(_)))
        { bail!("非法路径根表达式：只能在路径前缀引用一个根"); }
        return Ok(Some((name, suffix)));
    }
    if text.contains("${") { bail!("非法路径根表达式：只能在路径前缀使用 ${{roots.name}}"); }
    Ok(None)
}

fn absolute(value: &Path, base: &Path) -> Result<PathBuf> {
    if !value.is_absolute()
        && matches!(value.components().next(), Some(Component::Prefix(_)))
    {
        bail!("不支持驱动器相对路径（必须使用绝对路径或普通相对路径）：{}", value.display());
    }
    let joined = if value.is_absolute() { value.to_path_buf() } else { base.join(value) };
    let absolute = std::path::absolute(joined)?;
    // Lexical normalization does not require the destination to exist or follow links.
    let mut result = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => { result.pop(); }
            part => result.push(part.as_os_str()),
        }
    }
    Ok(result)
}
