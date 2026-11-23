//! Native skill paths, validated before any launch output is published.
//! OMP consumes scan directories and selected names; Pi consumes individual skill roots.
//! Neither disables native MCP or project instructions.
use std::{borrow::Cow, collections::{HashMap, HashSet, hash_map::Entry}, fs, path::{Path, PathBuf}};
use serde::Deserialize;
use crate::{files::FileAccess, paths::Roots, tasks::{Agent, TaskError}};
use super::{OmpSkillSource, Suite, Suites};

#[derive(Deserialize)]
struct Frontmatter {
    name: Option<String>,
    #[serde(default)]
    description: String,
}

struct Skill {
    name: String,
}

pub(super) struct Selection {
    directories: Vec<PathBuf>,
    names: Vec<String>,
}

impl Suites {
    pub(super) fn select_skills(&self, suite: &Suite, roots: &Roots) -> Result<Selection, TaskError> {
        let id = suite.id();
        let agent = suite.harness();
        let fail = |error: String| TaskError::InvalidSuite(format!("Suite「{id}」技能装配失败：{error}；没有回退为空环境。"));
        let base = self.definition_file.parent().unwrap_or(Path::new("."));
        let resolve = |path: &str| -> Result<PathBuf, TaskError> {
            let resolved = roots.resolve(Path::new(path), base).map_err(|error| fail(error.to_string()))?;
            fs::canonicalize(&resolved).map_err(|error| fail(format!("{}：{error}", resolved.display())))
        };
        let mut seen_skills = HashSet::new();
        let mut names = HashMap::<String, PathBuf>::new();
        let mut select = |candidate: &Path| -> Result<bool, TaskError> {
            let canonical = fs::canonicalize(candidate.join("SKILL.md"))
                .map_err(|error| fail(format!("{}：{error}", candidate.display())))?;
            if !seen_skills.insert(canonical.clone()) { return Ok(false); }
            let Some(skill) = read_skill(candidate, agent)
                .map_err(|error| fail(format!("{}：{error}", canonical.display())))? else { return Ok(false) };
            match names.entry(skill.name) {
                Entry::Vacant(entry) => { entry.insert(canonical); }
                Entry::Occupied(entry) => return Err(fail(format!(
                    "所选技能名称重复：{}；{}；{}；请明确选择唯一实体。", entry.key(), entry.get().display(), canonical.display()))),
            }
            Ok(true)
        };
        let mut seen_directories = HashSet::new();
        let directories = match suite {
            Suite::Omp { skills, .. } => {
                let mut directories = Vec::with_capacity(skills.len());
                let mut scanned_collections = HashSet::new();
                for source in skills {
                    let root = resolve(source.path())?;
                    let directory = match source {
                        OmpSkillSource::Collection { .. } => {
                            if !scanned_collections.insert(root.clone()) { continue; }
                            for candidate in collection_skills(&root).map_err(&fail)? {
                                select(&candidate)?;
                            }
                            Cow::Owned(root)
                        }
                        OmpSkillSource::Skill { .. } => {
                            select(&root)?;
                            let parent = root.parent().ok_or_else(|| fail(format!("{} 没有技能父目录。", root.display())))?;
                            Cow::Borrowed(parent)
                        }
                    };
                    if !seen_directories.contains(directory.as_ref()) {
                        let directory = directory.into_owned();
                        seen_directories.insert(directory.clone());
                        directories.push(directory);
                    }
                }
                directories
            }
            Suite::Pi { skills, .. } => {
                let mut directories = Vec::with_capacity(skills.len());
                for path in skills {
                    let directory = resolve(path)?;
                    if seen_directories.insert(directory.clone()) && select(&directory)? {
                        directories.push(directory);
                    }
                }
                directories
            }
        };
        let mut names = names.into_keys().collect::<Vec<_>>();
        names.sort();
        Ok(Selection { directories, names })
    }
}

fn collection_skills(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| format!("{}：{error}", directory.display()))? {
        let entry = entry.map_err(|error| format!("{}：{error}", directory.display()))?;
        if entry.file_name().to_string_lossy().starts_with('.') { continue; }
        let path = entry.path();
        let metadata = fs::metadata(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        if !metadata.is_dir() { continue; }
        match fs::metadata(path.join("SKILL.md")) {
            Ok(_) => candidates.push(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(format!("{}：{error}", path.display())),
        }
    }
    candidates.sort_by_cached_key(|path| {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        (name.to_lowercase(), name)
    });
    Ok(candidates)
}

pub(super) fn assemble(access: &FileAccess, assembly: &Path, suite: &Suite, skills: &Selection) -> Result<Vec<String>, String> {
    let Suite::Omp { .. } = suite else {
        let mut args = vec!["--no-skills".into()];
        for directory in &skills.directories {
            args.extend(["--skill".into(), directory.to_string_lossy().into_owned()]);
        }
        return Ok(args);
    };
    let manifest = assembly.join("omp-skills.json");
    let config = serde_json::json!({"skills": {
        "enabled": !skills.names.is_empty(),
        "enableCodexUser": false, "enableClaudeUser": false,
        "enableClaudeProject": false, "enablePiUser": false,
        "enablePiProject": false, "enableAgentsUser": false,
        "enableAgentsProject": false,
        "customDirectories": skills.directories,
        // Native includeSkills is a glob filter over final (collision-resolved)
        // names, not a discovery sandbox. Escape authored names as literals so
        // a selected wildcard cannot admit unrelated default/managed skills.
        "includeSkills": skills.names.iter().map(|name| literal_skill_pattern(name)).collect::<Vec<_>>(),
        "ignoredSkills": []
    }});
    access.create(&manifest, &serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(vec!["--config".into(), manifest.to_string_lossy().into_owned()])
}

fn literal_skill_pattern(name: &str) -> String {
    let mut pattern = String::with_capacity(name.len());
    for character in name.chars() {
        if matches!(character, '*' | '?' | '[' | ']' | '{' | '}' | '!' | '(' | ')') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern
}

fn read_skill(directory: &Path, agent: Agent) -> Result<Option<Skill>, String> {
    let raw = fs::read_to_string(directory.join("SKILL.md")).map_err(|error| error.to_string())?;
    if agent == Agent::Omp {
        let metadata = omp_frontmatter(&raw);
        if metadata.get("enabled").and_then(serde_yaml_ng::Value::as_bool) == Some(false)
            || !metadata.get("description").is_some_and(yaml_truthy) { return Ok(None); }
        let name = metadata.get("name").and_then(serde_yaml_ng::Value::as_str).map(str::trim)
            .filter(|name| !name.is_empty()).map(str::to_owned)
            .unwrap_or_else(|| directory.file_name().unwrap_or_default().to_string_lossy().into_owned());
        if name.contains(['/', '\\']) { return Ok(None); }
        check_readable_resources(directory)?;
        return Ok(Some(Skill { name }));
    }
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
    check_readable_resources(directory)?;
    Ok(Some(Skill { name }))
}

// Mirror OMP's permissive native frontmatter reader: comments and CRLF are
// normalized, ambiguous scalars repaired, then malformed YAML falls back to
// independent key/value lines. Invalid metadata must not invent a skill name.
fn omp_frontmatter(raw: &str) -> serde_yaml_ng::Value {
    use serde_yaml_ng::{Mapping, Value};
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut source = normalized.as_str();
    let mut after_comments = source;
    while let Some(comment) = after_comments.trim_start().strip_prefix("<!--") {
        let Some((_, rest)) = comment.split_once("-->") else { break };
        after_comments = rest;
    }
    if after_comments.len() != source.len() && after_comments.trim_start().starts_with("---") { source = after_comments.trim_start(); }
    if !source.starts_with("---") { return Value::Null; }
    let Some(end) = source.get(3..).and_then(|tail| tail.find("\n---")).map(|offset| offset + 3) else { return Value::Null };
    let mut metadata = source.get(4..end).unwrap_or_default().to_owned();
    while let Some(start) = metadata.find("<!--") {
        let Some(end) = metadata[start + 4..].find("-->").map(|offset| start + 4 + offset + 3) else { break };
        metadata.replace_range(start..end, "");
    }
    metadata = metadata.replace('\t', "  ");
    if let Ok(value) = serde_yaml_ng::from_str::<Value>(&metadata) { return value; }
    let repaired = metadata.lines().map(|line| {
        let Some((key, value)) = line.split_once(':') else { return line.to_owned() };
        let trimmed = value.trim();
        if !value.starts_with(char::is_whitespace) || !trimmed.contains(": ")
            || trimmed.starts_with(['\"', '\'', '[', '{', '|', '>', '!', '&', '*', '#']) { return line.to_owned(); }
        format!("{key}: {}", serde_json::to_string(trimmed).expect("string serialization"))
    }).collect::<Vec<_>>().join("\n");
    if let Ok(value) = serde_yaml_ng::from_str::<Value>(&repaired) { return value; }
    let mut values = Mapping::new();
    for line in metadata.lines() {
        let Some((key, raw)) = line.split_once(':') else { continue };
        if key.is_empty() || !key.chars().all(|character| character.is_alphanumeric() || matches!(character, '_' | '-')) { continue; }
        let raw = raw.trim();
        let value = match serde_yaml_ng::from_str::<Value>(raw) {
            Ok(value) if !value.is_null() && !value.is_mapping() => value,
            _ => Value::String(raw.to_owned()),
        };
        values.insert(Value::String(key.to_owned()), value);
    }
    Value::Mapping(values)
}

fn yaml_truthy(value: &serde_yaml_ng::Value) -> bool {
    use serde_yaml_ng::Value;
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0 && !value.is_nan()),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn check_readable_resources(directory: &Path) -> Result<(), String> {
    // This is an availability check of external, read-only resource contents;
    // it does not import/execute scripts or copy the source tree.
    let mut pending = vec![directory.to_path_buf()];
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
    Ok(())
}
