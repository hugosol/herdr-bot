//! Hand-maintained option metadata. Each operation loads one complete, validated snapshot.
//! No generated defaults, watcher, or stale-cache fallback.
use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use crate::templates::{Agent, BotTemplate, TemplateError};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub agents: Vec<AgentOption>,
    pub models: Vec<ModelOption>,
    pub skills: Vec<SkillOption>,
    pub extensions: Vec<ExtensionOption>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentOption { pub id: Agent, pub label: String }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub provider_model: Option<String>,
    #[serde(default)]
    pub thinking: BTreeMap<String, Vec<String>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillOption { pub id: String, pub label: String, pub native_name: String }
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionOption {
    pub id: String,
    pub label: String,
    pub agent: Agent,
    pub kind: ExtensionKind,
    pub entrypoint: PathBuf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionKind { Fixed, Optional }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateOptions {
    agents: Vec<String>,
    models: Vec<String>,
    thinking: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    skills: Vec<String>,
    labels: BTreeMap<String, BTreeMap<String, String>>,
    extensions: Vec<ExtensionOption>,
}

impl Catalog {
    pub fn load(path: &Path) -> Result<Self, TemplateError> {
        let result = (|| -> Result<Self, String> {
            let raw = fs::read_to_string(path).map_err(|e| format!("读取失败：{e}"))?;
            let catalog: Self = toml::from_str(&raw).map_err(|e| format!("TOML：{e}"))?;
            catalog.validate()?;
            Ok(catalog)
        })();
        result.map_err(|reason| TemplateError::Metadata(format!("选项元数据 {}：{reason}", path.display())))
    }

    fn validate(&self) -> Result<(), String> {
        fn identifiers<'a>(items: impl Iterator<Item=(&'a str, &'a str)>, kind: &str) -> Result<(), String> {
            let mut seen = BTreeSet::new();
            for (id, label) in items {
                if id.trim().is_empty() || id.trim() != id || label.trim().is_empty() {
                    return Err(format!("{kind} 标识或显示名称为空/含首尾空白：{id:?}"));
                }
                if !seen.insert(id) { return Err(format!("{kind} 重复标识：{id}")); }
            }
            Ok(())
        }
        identifiers(self.agents.iter().map(|v| (v.id.label(), v.label.as_str())), "agent")?;
        identifiers(self.models.iter().map(|v| (v.id.as_str(), v.label.as_str())), "model")?;
        identifiers(self.skills.iter().map(|v| (v.id.as_str(), v.label.as_str())), "skill")?;
        identifiers(self.extensions.iter().map(|v| (v.id.as_str(), v.label.as_str())), "extension")?;
        if self.agents.is_empty() { return Err("agents 不能为空".into()); }
        if !self.models.iter().any(|v| v.id == "default") { return Err("models 缺少 default".into()); }
        for model in &self.models {
            if model.id == "default" {
                if model.provider_model.is_some() || !model.thinking.is_empty() {
                    return Err("default 模型不能指定 provider_model 或 thinking 覆盖".into());
                }
                continue;
            }
            let mapping = model.provider_model.as_deref().unwrap_or("");
            if !mapping.split_once('/').is_some_and(|(provider, name)| !provider.trim().is_empty() && !name.trim().is_empty()) || mapping.trim() != mapping {
                return Err(format!("model {} 需要完整 provider/model 映射", model.id));
            }
            if model.thinking.is_empty() { return Err(format!("model {} 缺少 agent 适用关系", model.id)); }
            for (agent, levels) in &model.thinking {
                if !self.agents.iter().any(|v| v.id.label() == agent) {
                    return Err(format!("model {} 引用未知 agent {agent}", model.id));
                }
                let mut seen = BTreeSet::new();
                for level in levels {
                    if level.trim().is_empty() || level.trim() != level || level == "default" || !seen.insert(level) {
                        return Err(format!("model {} / {agent} thinking 非法或重复：{level:?}", model.id));
                    }
                }
            }
        }
        for skill in &self.skills {
            if skill.native_name.trim().is_empty() || skill.native_name.contains(char::is_whitespace) {
                return Err(format!("skill {} 的 native_name 无效", skill.id));
            }
        }
        for extension in &self.extensions {
            if !self.agents.iter().any(|v| v.id == extension.agent) {
                return Err(format!("extension {} 引用未知 agent {}", extension.id, extension.agent.label()));
            }
            if extension.entrypoint.as_os_str().is_empty() {
                return Err(format!("extension {} 缺少入口", extension.id));
            }
        }
        for agent in &self.agents {
            if !self.extensions.iter().any(|v| v.agent == agent.id && v.kind == ExtensionKind::Fixed) {
                return Err(format!("agent {} 缺少固定扩展", agent.id.label()));
            }
        }
        Ok(())
    }

    pub fn supported_thinking(&self, agent: Agent, model: &str) -> &[String] {
        self.models.iter().find(|v| v.id == model)
            .and_then(|v| v.thinking.get(agent.label())).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn validate_selection(&self, agent: Agent, model: &str, thinking: &str, skill: Option<&str>) -> Result<(), TemplateError> {
        if !self.agents.iter().any(|v| v.id == agent) { return Err(TemplateError::UnknownAgent(agent.label().into())); }
        let selected = self.models.iter().find(|v| v.id == model)
            .filter(|v| v.id == "default" || v.thinking.contains_key(agent.label()))
            .ok_or_else(|| TemplateError::UnknownModel(model.into()))?;
        if selected.id == "default" && thinking != "default" { return Err(TemplateError::ThinkingRequiresDefaultModel); }
        if thinking != "default" {
            if !self.models.iter().any(|v| v.thinking.values().any(|levels| levels.iter().any(|level| level == thinking))) {
                return Err(TemplateError::UnknownThinking(thinking.into()));
            }
            if !self.supported_thinking(agent, model).iter().any(|v| v == thinking) {
                return Err(TemplateError::ThinkingUnsupported { agent, model: model.into(), thinking: thinking.into(), supported: self.supported_thinking(agent, model).to_vec() });
            }
        }
        if let Some(skill) = skill {
            if !self.skills.iter().any(|v| v.id == skill) { return Err(TemplateError::UnknownSkill(skill.into())); }
        }
        Ok(())
    }

    pub fn validate_extensions(&self, agent: Agent, selections: &[String]) -> Result<(), TemplateError> {
        for (index, id) in selections.iter().enumerate() {
            if selections[..index].contains(id) || !self.extensions.iter().any(|extension| {
                extension.id == *id && extension.agent == agent && extension.kind == ExtensionKind::Optional
            }) {
                return Err(TemplateError::InvalidExtension(id.clone()));
            }
        }
        Ok(())
    }

    pub fn validate_template(&self, template: &BotTemplate) -> Result<(), TemplateError> {
        self.validate_selection(template.agent, &template.model, &template.thinking, template.skill.as_deref())?;
        self.validate_extensions(template.agent, &template.extensions)
    }

    pub fn options(&self) -> TemplateOptions {
        let mut thinking = BTreeMap::new();
        for agent in &self.agents {
            let mut models = BTreeMap::new();
            for model in &self.models {
                if model.id == "default" || model.thinking.contains_key(agent.id.label()) {
                    models.insert(model.id.clone(), self.supported_thinking(agent.id, &model.id).to_vec());
                }
            }
            thinking.insert(agent.id.label().into(), models);
        }
        let labels = BTreeMap::from([
            ("agents".into(), self.agents.iter().map(|v| (v.id.label().into(), v.label.clone())).collect()),
            ("models".into(), self.models.iter().map(|v| (v.id.clone(), v.label.clone())).collect()),
            ("skills".into(), self.skills.iter().map(|v| (v.id.clone(), v.label.clone())).collect()),
        ]);
        TemplateOptions {
            agents: self.agents.iter().map(|v| v.id.label().into()).collect(),
            models: self.models.iter().map(|v| v.id.clone()).collect(),
            skills: self.skills.iter().map(|v| v.id.clone()).collect(),
            thinking, labels, extensions: self.extensions.clone(),
        }
    }

    /// Resolve stable IDs once, before any external work; the caller owns this snapshot.
    pub fn resolve(&self, template: &BotTemplate) -> Result<ResolvedPreset, TemplateError> {
        self.validate_template(template)?;
        Ok(ResolvedPreset {
            provider_model: self.models.iter().find(|v| v.id == template.model).and_then(|v| v.provider_model.clone()),
            thinking: (template.thinking != "default").then(|| template.thinking.clone()),
            native_skill: template.skill.as_ref().and_then(|id| self.skills.iter().find(|v| &v.id == id)).map(|v| v.native_name.clone()),
        })
    }
}

pub struct ResolvedPreset {
    pub provider_model: Option<String>,
    pub thinking: Option<String>,
    pub native_skill: Option<String>,
}
