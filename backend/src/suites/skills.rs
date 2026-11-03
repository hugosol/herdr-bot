//! Selected existing SKILL.md directories, not a second resource catalog.
//! Native OMP settings filter the final skill names, never whole capability
//! providers: native MCP and native project instructions remain discoverable.
use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use serde::Deserialize;
use crate::{files::FileAccess, paths::Roots, tasks::TaskError};
use super::{Suite, Suites};

#[derive(Deserialize)]
struct Frontmatter {
    name: Option<String>,
    description: String,
}

pub(super) struct Skill {
    name: String,
    pub(super) directory: PathBuf,
}

impl Suites {
    pub(super) fn select_skills(&self, suite: &Suite, roots: &Roots) -> Result<Vec<Skill>, TaskError> {
        let id = &suite.id;
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{id}」技能装配失败：{error}；没有回退为空环境。"));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        let mut skills = Vec::with_capacity(suite.skills.len());
        for declaration in &suite.skills {
            let directory = roots.resolve(Path::new(declaration), base).map_err(|error| fail(error.to_string()))?;
            skills.push(read_skill(directory).map_err(|error| fail(format!("{declaration}：{error}")))?);
        }
        let mut names = HashSet::with_capacity(skills.len());
        for skill in &skills {
            if !names.insert(skill.name.as_str()) {
                return Err(fail(format!("所选技能名称重复：{}；请明确选择唯一实体。", skill.name)));
            }
        }
        Ok(skills)
    }
}

pub(super) fn assemble(access: &FileAccess, assembly: &Path, suite: &Suite, skills: &[Skill]) -> Result<Vec<String>, String> {
    let links = assembly.join("skills");
    access.create_dir(&links).map_err(|error| error.to_string())?;
    for skill in skills {
        access.link_resource_directory(&skill.directory, &links.join(&skill.name))
            .map_err(|error| format!("链接 {} → {} 失败：{error}", links.join(&skill.name).display(), skill.directory.display()))?;
    }
    let manifest = assembly.join("omp-skills.json");
    let config = serde_json::json!({"skills": {
        "enabled": !skills.is_empty(), "customDirectories": [links],
        "includeSkills": skills.iter().map(|skill| literal_glob(&skill.name)).collect::<Vec<_>>(),
        "ignoredSkills": []
    }});
    access.create(&manifest, &serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let mut args = vec!["--config".into(), manifest.to_string_lossy().into_owned()];
    if !suite.lsp.is_empty() {
        // Native plugin discovery merges below user/project LSP files and
        // keeps bundled defaults, unlike a native settings override.
        let lsp = serde_json::json!({"servers": &suite.lsp});
        access.create(assembly.join("lsp.json"), &serde_json::to_vec_pretty(&lsp).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        args.extend(["--plugin-dir".into(), assembly.to_string_lossy().into_owned()]);
    }
    Ok(args)
}

fn read_skill(directory: PathBuf) -> Result<Skill, String> {
    let raw = fs::read_to_string(directory.join("SKILL.md")).map_err(|error| error.to_string())?;
    let (opening, remainder) = raw.split_once('\n').ok_or("SKILL.md 缺少原生 YAML frontmatter")?;
    if opening.trim_end_matches('\r') != "---" { return Err("SKILL.md 缺少原生 YAML frontmatter".into()); }
    let mut end = 0;
    let closing = remainder.split_inclusive('\n').find_map(|line| {
        if line.trim_end_matches(['\r', '\n']) == "---" { return Some(end); }
        end += line.len();
        None
    }).ok_or("SKILL.md frontmatter 未闭合")?;
    let metadata: Frontmatter = serde_yaml_ng::from_str(&remainder[..closing]).map_err(|error| error.to_string())?;
    if metadata.description.trim().is_empty() { return Err("SKILL.md description 不能为空".into()); }
    let name = metadata.name.unwrap_or_else(|| directory.file_name().unwrap_or_default().to_string_lossy().into_owned());
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\n', '\r']) {
        return Err("SKILL.md name 不能为空或包含路径分隔符".into());
    }
    // This is an availability check of external, read-only resource contents;
    // it does not import/execute scripts or copy the source tree.
    let mut pending = vec![directory.clone()];
    let mut visited = HashSet::new();
    while let Some(path) = pending.pop() {
        let canonical = fs::canonicalize(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        if !visited.insert(canonical) { continue; }
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|error| error.to_string())? {
                pending.push(entry.map_err(|error| error.to_string())?.path());
            }
        } else if metadata.is_file() {
            fs::File::open(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        } else {
            return Err(format!("{} 不是可读取的普通技能资源", path.display()));
        }
    }
    Ok(Skill { name, directory })
}

fn literal_glob(name: &str) -> String {
    let mut pattern = String::with_capacity(name.len());
    for character in name.chars() {
        if "*?[]{}!".contains(character) { pattern.push('\\'); }
        pattern.push(character);
    }
    pattern
}
