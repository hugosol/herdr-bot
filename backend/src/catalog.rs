//! Hand-maintained option metadata. Each operation loads one complete, validated snapshot.
//! No generated defaults, watcher, or stale-cache fallback.
use crate::tasks::{Agent, Task, TaskError, TaskType};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

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
pub struct AgentOption {
    pub id: Agent,
    pub label: String,
}
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
pub struct SkillOption {
    pub id: String,
    pub label: String,
    pub native_name: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionOption {
    pub id: String,
    pub label: String,
    pub agent: Agent,
    pub kind: ExtensionKind,
    #[serde(default)]
    pub task_type: Option<TaskType>,
    #[serde(default)]
    pub entrypoint: PathBuf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionKind {
    Fixed,
    Optional,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOptions {
    agents: Vec<String>,
    models: Vec<String>,
    thinking: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    skills: Vec<String>,
    labels: BTreeMap<String, BTreeMap<String, String>>,
    extensions: Vec<ExtensionOption>,
}

impl Catalog {
    pub fn load(path: &Path) -> Result<Self, TaskError> {
        let result = (|| -> Result<Self, String> {
            let raw = fs::read_to_string(path).map_err(|e| format!("读取失败：{e}"))?;
            let catalog: Self = toml::from_str(&raw).map_err(|e| format!("TOML：{e}"))?;
            catalog.validate()?;
            Ok(catalog)
        })();
        result.map_err(|reason| {
            TaskError::Metadata(format!("选项元数据 {}：{reason}", path.display()))
        })
    }

    fn validate(&self) -> Result<(), String> {
        fn identifiers<'a>(
            items: impl Iterator<Item = (&'a str, &'a str)>,
            kind: &str,
        ) -> Result<(), String> {
            let mut seen = BTreeSet::new();
            for (id, label) in items {
                if id.trim().is_empty() || id.trim() != id || label.trim().is_empty() {
                    return Err(format!("{kind} 标识或显示名称为空/含首尾空白：{id:?}"));
                }
                if !seen.insert(id) {
                    return Err(format!("{kind} 重复标识：{id}"));
                }
            }
            Ok(())
        }
        identifiers(
            self.agents.iter().map(|v| (v.id.label(), v.label.as_str())),
            "agent",
        )?;
        identifiers(
            self.models
                .iter()
                .map(|v| (v.id.as_str(), v.label.as_str())),
            "model",
        )?;
        identifiers(
            self.skills
                .iter()
                .map(|v| (v.id.as_str(), v.label.as_str())),
            "skill",
        )?;
        identifiers(
            self.extensions
                .iter()
                .map(|v| (v.id.as_str(), v.label.as_str())),
            "extension",
        )?;
        if self.agents.is_empty() {
            return Err("agents 不能为空".into());
        }
        if !self.models.iter().any(|v| v.id == "default") {
            return Err("models 缺少 default".into());
        }
        for model in &self.models {
            if model.id == "default" {
                if model.provider_model.is_some() || !model.thinking.is_empty() {
                    return Err("default 模型不能指定 provider_model 或 thinking 覆盖".into());
                }
                continue;
            }
            let mapping = model.provider_model.as_deref().unwrap_or("");
            if !mapping.split_once('/').is_some_and(|(provider, name)| {
                !provider.trim().is_empty() && !name.trim().is_empty()
            }) || mapping.trim() != mapping
            {
                return Err(format!("model {} 需要完整 provider/model 映射", model.id));
            }
            if model.thinking.is_empty() {
                return Err(format!("model {} 缺少 agent 适用关系", model.id));
            }
            for (agent, levels) in &model.thinking {
                if !self.agents.iter().any(|v| v.id.label() == agent) {
                    return Err(format!("model {} 引用未知 agent {agent}", model.id));
                }
                let mut seen = BTreeSet::new();
                for level in levels {
                    if level.trim().is_empty()
                        || level.trim() != level
                        || level == "default"
                        || !seen.insert(level)
                    {
                        return Err(format!(
                            "model {} / {agent} thinking 非法或重复：{level:?}",
                            model.id
                        ));
                    }
                }
            }
        }
        for skill in &self.skills {
            if skill.native_name.trim().is_empty()
                || skill.native_name.contains(char::is_whitespace)
            {
                return Err(format!("skill {} 的 native_name 无效", skill.id));
            }
        }
        for extension in &self.extensions {
            if extension.task_type.is_some_and(|role| role == TaskType::Normal || extension.agent != Agent::Omp || extension.kind != ExtensionKind::Fixed) {
                return Err(format!("角色扩展 {} 必须是 omp manager/node 固定扩展", extension.id));
            }
            if !self.agents.iter().any(|v| v.id == extension.agent) {
                return Err(format!(
                    "extension {} 引用未知 agent {}",
                    extension.id,
                    extension.agent.label()
                ));
            }
            // A missing entrypoint is an item-local availability failure. Strict
            // launch rejects it below; Resume can skip it without losing peers.
        }
        for agent in &self.agents {
            if !self
                .extensions
                .iter()
                .any(|v| v.agent == agent.id && v.kind == ExtensionKind::Fixed)
            {
                return Err(format!("agent {} 缺少固定扩展", agent.id.label()));
            }
        }
        Ok(())
    }

    pub fn supported_thinking(&self, agent: Agent, model: &str) -> &[String] {
        self.models
            .iter()
            .find(|v| v.id == model)
            .and_then(|v| v.thinking.get(agent.label()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn validate_selection(
        &self,
        agent: Agent,
        model: &str,
        thinking: &str,
        skill: Option<&str>,
    ) -> Result<(), TaskError> {
        if !self.agents.iter().any(|v| v.id == agent) {
            return Err(TaskError::UnknownAgent(agent.label().into()));
        }
        let selected = self
            .models
            .iter()
            .find(|v| v.id == model)
            .filter(|v| v.id == "default" || v.thinking.contains_key(agent.label()))
            .ok_or_else(|| TaskError::UnknownModel(model.into()))?;
        if selected.id == "default" && thinking != "default" {
            return Err(TaskError::ThinkingRequiresDefaultModel);
        }
        if thinking != "default" {
            if !self.models.iter().any(|v| {
                v.thinking
                    .values()
                    .any(|levels| levels.iter().any(|level| level == thinking))
            }) {
                return Err(TaskError::UnknownThinking(thinking.into()));
            }
            if !self
                .supported_thinking(agent, model)
                .iter()
                .any(|v| v == thinking)
            {
                return Err(TaskError::ThinkingUnsupported {
                    agent,
                    model: model.into(),
                    thinking: thinking.into(),
                    supported: self.supported_thinking(agent, model).to_vec(),
                });
            }
        }
        if let Some(skill) = skill {
            if !self.skills.iter().any(|v| v.id == skill) {
                return Err(TaskError::UnknownSkill(skill.into()));
            }
        }
        Ok(())
    }

    pub fn validate_extensions(
        &self,
        agent: Agent,
        selections: &[String],
    ) -> Result<(), TaskError> {
        for (index, id) in selections.iter().enumerate() {
            if selections[..index].contains(id)
                || !self.extensions.iter().any(|extension| {
                    extension.id == *id
                        && extension.agent == agent
                        && extension.kind == ExtensionKind::Optional
                })
            {
                return Err(TaskError::InvalidExtension(id.clone()));
            }
        }
        Ok(())
    }

    pub fn options(&self) -> TaskOptions {
        let mut thinking = BTreeMap::new();
        for agent in &self.agents {
            let mut models = BTreeMap::new();
            for model in &self.models {
                if model.id == "default" || model.thinking.contains_key(agent.id.label()) {
                    models.insert(
                        model.id.clone(),
                        self.supported_thinking(agent.id, &model.id).to_vec(),
                    );
                }
            }
            thinking.insert(agent.id.label().into(), models);
        }
        let labels = BTreeMap::from([
            (
                "agents".into(),
                self.agents
                    .iter()
                    .map(|v| (v.id.label().into(), v.label.clone()))
                    .collect(),
            ),
            (
                "models".into(),
                self.models
                    .iter()
                    .map(|v| (v.id.clone(), v.label.clone()))
                    .collect(),
            ),
            (
                "skills".into(),
                self.skills
                    .iter()
                    .map(|v| (v.id.clone(), v.label.clone()))
                    .collect(),
            ),
        ]);
        TaskOptions {
            agents: self.agents.iter().map(|v| v.id.label().into()).collect(),
            models: self.models.iter().map(|v| v.id.clone()).collect(),
            skills: self.skills.iter().map(|v| v.id.clone()).collect(),
            thinking,
            labels,
            extensions: self.extensions.clone(),
        }
    }

    /// Resolve stable IDs once, before any external work; the caller owns this snapshot.
    pub fn resolve(&self, task: &Task) -> Result<ResolvedPreset, TaskError> {
        self.validate_selection(task.agent, &task.trigger.model, &task.trigger.thinking, task.trigger.skill.as_deref())?;
        Ok(ResolvedPreset {
            env: self.resolve_env(task.agent, task.task_type, &task.env.extensions)?,
            trigger: ResolvedTriggerConfig {
                provider_model: self.models.iter()
                    .find(|v| v.id == task.trigger.model)
                    .and_then(|v| v.provider_model.clone()),
                thinking: (task.trigger.thinking != "default").then(|| task.trigger.thinking.clone()),
                native_skill: task.trigger.skill.as_ref()
                    .and_then(|id| self.skills.iter().find(|v| &v.id == id))
                    .map(|v| v.native_name.clone()),
            },
        })
    }

    /// Resolve only the current session environment. Trigger choices may have
    /// become invalid since the Task was saved and are deliberately not consulted.
    pub fn resolve_env(&self, agent: Agent, task_type: TaskType, selections: &[String]) -> Result<ResolvedEnvConfig, TaskError> {
        task_type.validate_agent(agent)?;
        self.validate_extensions(agent, selections)?;
        if task_type != TaskType::Normal && !self.extensions.iter().any(|extension| extension.task_type == Some(task_type) && extension.agent == agent && extension.kind == ExtensionKind::Fixed) {
            return Err(TaskError::Metadata("当前目录缺少对应 Task 类型的固定角色扩展。".into()));
        }
        Ok(ResolvedEnvConfig {
            extensions: self.extensions.iter().filter(|extension| {
                extension.applies_to(agent, task_type) && (extension.kind == ExtensionKind::Fixed
                    || selections.contains(&extension.id))
            }).cloned().collect(),
        })
    }
}

/// A creation snapshot keeps launch-only actions separate from session resources.
/// The persisted Task and HTTP representation do not change format.
pub struct ResolvedPreset {
    pub trigger: ResolvedTriggerConfig,
    pub env: ResolvedEnvConfig,
}

pub struct ResolvedTriggerConfig {
    pub provider_model: Option<String>,
    pub thinking: Option<String>,
    pub native_skill: Option<String>,
}

pub struct ResolvedEnvConfig {
    pub extensions: Vec<ExtensionOption>,
}

impl ResolvedEnvConfig {
    /// Inspect this creation's extension files or directory roots for readability.
    /// Directory discovery and package interpretation remain owned by the target agent.
    /// Never import, execute, or probe extension services.
    /// A successful check is not a promise that files cannot change before the agent loads them.
    pub fn check_entrypoints(&self) -> Result<(), String> {
        for extension in &self.extensions {
            extension.check_entrypoint()?;
        }
        Ok(())
    }
}

impl ExtensionOption {
    pub fn applies_to(&self, agent: Agent, task_type: TaskType) -> bool {
        self.agent == agent && self.task_type.is_none_or(|role| role == task_type)
    }

    /// Check availability only; the agent remains responsible for loading.
    pub fn check_entrypoint(&self) -> Result<(), String> {
        use std::io::Read;
        let extension = self;
        let check = || -> Result<(), String> {
                if extension.entrypoint.as_os_str().is_empty() {
                    return Err("入口映射缺失".into());
                }
                if fs::metadata(&extension.entrypoint).is_ok_and(|metadata| metadata.is_dir()) {
                    let mut entries = fs::read_dir(&extension.entrypoint)
                        .map_err(|error| format!("入口目录无法读取：{error}"))?;
                    entries
                        .next()
                        .transpose()
                        .map_err(|error| format!("入口目录无法读取：{error}"))?;
                    return Ok(());
                }
                let mut file = fs::File::open(&extension.entrypoint).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::NotFound {
                        format!("入口不存在：{error}")
                    } else {
                        format!("入口无法读取：{error}")
                    }
                })?;
                let metadata = file
                    .metadata()
                    .map_err(|error| format!("无法检查文件类型：{error}"))?;
                if !metadata.is_file() {
                    return Err("入口不是文件".into());
                }
                file.read(&mut [0u8; 1])
                    .map_err(|error| format!("入口无法读取：{error}"))?;
                Ok(())
            };
            check().map_err(|reason| {
                format!(
                    "扩展「{}」（{}）入口 {}：{reason}。",
                    extension.label,
                    extension.id,
                    extension.entrypoint.display()
                )
            })
    }
}
