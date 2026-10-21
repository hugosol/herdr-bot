//! Bot 模板：由后端以文件持久保存，电脑与手机共享同一份。
//!
//! 模板是创建 Bot 时复用的预设：显示名、Bot 自动命名规则、agent、模型、思考级别、可选 Skill
//! 与固定 prompt。模板只在创建时被读取，编辑或删除不改变已经创建的 Bot。
//!
//! 持久化语义与 zone 登记相同：只有存储文件成功写入后才返回成功；写入先写同目录的临时文件并
//! `sync_all`，再原子替换存储文件。文件不存在表示还没有任何模板，不是读取错误；读取失败或内容
//! 损坏都是明确错误，不会伪装成空列表，也不会覆盖原文件。同一实例内的读取与写入共用一把锁。
//!
//! agent / model / thinking 的选项来自目标 agent 源码中的固定能力事实：OMP 的模型元数据给出
//! `thinking.efforts`，Pi 使用 `getSupportedThinkingLevels`。全局 CLI 支持的级别（例如 OMP 的
//! `auto`）不因此成为某个模型的可用级别；两个 agent 的能力表也互不复制。

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// 模板存储文件在配置缺省时的相对路径（相对进程工作目录，与既有配置路径语义一致）。
pub const DEFAULT_STORAGE_FILE: &str = "bot-templates.json";

/// 模板可选的 agent 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Omp,
    Pi,
}

impl Agent {
    /// 按界面顺序排列的全部固定选项。
    pub const ALL: [Agent; 2] = [Agent::Omp, Agent::Pi];

    /// 序列化后的固定值（存入存储文件、HTTP 响应也使用同一个值）。
    pub fn label(self) -> &'static str {
        match self {
            Agent::Omp => "omp",
            Agent::Pi => "pi",
        }
    }
}

/// 模板可选的模型：沿用 agent 默认值，或两个已确认的显式模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Model {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "gpt-6.1-sol")]
    Gpt61Sol,
    #[serde(rename = "deepseek-flash")]
    DeepseekFlash,
}

impl Model {
    /// 按界面顺序排列的全部固定选项。
    pub const ALL: [Model; 3] = [Model::Default, Model::Gpt61Sol, Model::DeepseekFlash];

    /// 序列化后的固定值。
    pub fn label(self) -> &'static str {
        match self {
            Model::Default => "default",
            Model::Gpt61Sol => "gpt-6.1-sol",
            Model::DeepseekFlash => "deepseek-flash",
        }
    }

    /// 显式模型映射到约定的完整 provider / model 标识；沿用默认值时不传模型覆盖。
    pub fn provider_model(self) -> Option<&'static str> {
        match self {
            Model::Default => None,
            Model::Gpt61Sol => Some("openai-codex/gpt-6.1-sol"),
            Model::DeepseekFlash => Some("deepseek/deepseek-flash"),
        }
    }
}

/// 模板可选的思考级别；`Default` 表示沿用 agent 默认值，不传 `--thinking`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Thinking {
    Default,
    Off,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Thinking {
    /// 序列化后的固定值。
    pub fn label(self) -> &'static str {
        match self {
            Thinking::Default => "default",
            Thinking::Off => "off",
            Thinking::Minimal => "minimal",
            Thinking::Low => "low",
            Thinking::Medium => "medium",
            Thinking::High => "high",
            Thinking::Xhigh => "xhigh",
            Thinking::Max => "max",
        }
    }
}

/// 模板可选的 Skill（单选，可为无）；这两个值表示名为 teach / code-review 的 Skill。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Skill {
    Teach,
    CodeReview,
}

impl Skill {
    /// 按界面顺序排列的全部固定选项。
    pub const ALL: [Skill; 2] = [Skill::Teach, Skill::CodeReview];

    /// 序列化后的固定值（界面显示为 `/teach`、`/code-review`）。
    pub fn label(self) -> &'static str {
        match self {
            Skill::Teach => "teach",
            Skill::CodeReview => "code-review",
        }
    }
}

/// 一个已保存的模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BotTemplate {
    /// 稳定标识；编辑以它为定位，不依赖可改动的显示名。
    pub id: String,
    /// 模板显示名，不是新 Bot 的固定名字。
    pub name: String,
    /// Bot 自动命名规则：单行固定文字与占位符。
    pub bot_name: String,
    pub agent: Agent,
    pub model: Model,
    pub thinking: Thinking,
    /// 不选 Skill 时为 `None`。
    pub skill: Option<Skill>,
    /// 可多行、可为空的固定指示。
    pub prompt: String,
}

/// 一次新增或编辑请求的原始字段；字符串先按字段逐个校验，失败时给出明确类别。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateInput {
    name: Option<String>,
    bot_name: Option<String>,
    agent: Option<String>,
    model: Option<String>,
    thinking: Option<String>,
    skill: Option<String>,
    prompt: Option<String>,
}

/// 校验后的模板字段（不含标识）。
struct TemplateFields {
    name: String,
    bot_name: String,
    agent: Agent,
    model: Model,
    thinking: Thinking,
    skill: Option<Skill>,
    prompt: String,
}

impl TemplateInput {
    fn validate(self) -> Result<TemplateFields, TemplateError> {
        let name = self.name.unwrap_or_default();
        if name.trim().is_empty() {
            return Err(TemplateError::NameRequired);
        }

        let bot_name = self.bot_name.unwrap_or_default();
        if bot_name.trim().is_empty() {
            return Err(TemplateError::BotNameRequired);
        }
        if bot_name.contains(|c| c == '\n' || c == '\r') {
            return Err(TemplateError::BotNameNotSingleLine);
        }

        let agent = self
            .agent
            .as_deref()
            .ok_or(TemplateError::AgentRequired)?
            .trim();
        let agent = Agent::ALL
            .into_iter()
            .find(|candidate| candidate.label() == agent)
            .ok_or_else(|| TemplateError::UnknownAgent(agent.to_string()))?;

        let model = match self.model.as_deref().map(str::trim) {
            None | Some("") => Model::Default,
            Some(value) => Model::ALL
                .into_iter()
                .find(|candidate| candidate.label() == value)
                .ok_or_else(|| TemplateError::UnknownModel(value.to_string()))?,
        };

        let thinking = match self.thinking.as_deref().map(str::trim) {
            None | Some("") => Thinking::Default,
            Some(value) => THINKING_OPTIONS
                .into_iter()
                .find(|candidate| candidate.label() == value)
                .ok_or_else(|| TemplateError::UnknownThinking(value.to_string()))?,
        };

        if model == Model::Default && thinking != Thinking::Default {
            return Err(TemplateError::ThinkingRequiresDefaultModel);
        }
        if thinking != Thinking::Default && !supported_thinking(agent, model).contains(&thinking) {
            return Err(TemplateError::ThinkingUnsupported {
                agent,
                model,
                thinking,
                supported: supported_thinking(agent, model),
            });
        }

        let skill = match self.skill.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(value) => {
                // 界面显示为 `/teach`；公开调用同时接受该显示值与规范的 Skill 名称。
                let name = value.strip_prefix('/').unwrap_or(value);
                Some(
                    Skill::ALL
                        .into_iter()
                        .find(|candidate| candidate.label() == name)
                        .ok_or_else(|| TemplateError::UnknownSkill(value.to_string()))?,
                )
            }
        };

        Ok(TemplateFields {
            name,
            bot_name,
            agent,
            model,
            thinking,
            skill,
            prompt: self.prompt.unwrap_or_default(),
        })
    }
}

/// 全部已知的思考级别（用于解析请求中的固定值）。
const THINKING_OPTIONS: [Thinking; 8] = [
    Thinking::Default,
    Thinking::Off,
    Thinking::Minimal,
    Thinking::Low,
    Thinking::Medium,
    Thinking::High,
    Thinking::Xhigh,
    Thinking::Max,
];

/// 指定 agent 与模型支持的显式思考级别（不含“沿用默认值”）。
///
/// 这些是目标源码中的固定能力事实：
/// - OMP `packages/catalog/src/models.json` 的 `thinking.efforts`：
///   `openai-codex/gpt-6.1-sol` = low/medium/high/xhigh/max，`deepseek/deepseek-flash` = low/high/max。
/// - Pi `getSupportedThinkingLevels`：`openai-codex/gpt-6.1-sol` 的 map 为 off=null、
///   minimal=low、low=low、medium=medium、high=high、xhigh=xhigh、max=max，因此支持
///   minimal/low/medium/high/xhigh/max（不接受 off）；`deepseek-flash` 的 map 为
///   minimal=null、low=low、medium=null、high=high、max=max，因此支持 off/low/high/max。
///
/// 模型沿用默认值时没有可校验的具体模型，只保留默认值。
pub fn supported_thinking(agent: Agent, model: Model) -> &'static [Thinking] {
    use Thinking::*;
    match (agent, model) {
        (_, Model::Default) => &[],
        (Agent::Omp, Model::Gpt61Sol) => &[Low, Medium, High, Xhigh, Max],
        (Agent::Omp, Model::DeepseekFlash) => &[Low, High, Max],
        (Agent::Pi, Model::Gpt61Sol) => &[Minimal, Low, Medium, High, Xhigh, Max],
        (Agent::Pi, Model::DeepseekFlash) => &[Off, Low, High, Max],
    }
}

/// 公开的固定选项；页面与直接调用者读取同一份，公开保存也按同一份能力事实校验。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateOptions {
    agents: Vec<&'static str>,
    models: Vec<&'static str>,
    /// `thinking[agent][model]` 的显式级别；`default` 模型对应空数组。
    thinking: BTreeMap<&'static str, BTreeMap<&'static str, Vec<&'static str>>>,
    skills: Vec<&'static str>,
}

impl TemplateOptions {
    pub fn current() -> Self {
        let mut thinking = BTreeMap::new();
        for agent in Agent::ALL {
            let mut by_model = BTreeMap::new();
            for model in Model::ALL {
                by_model.insert(
                    model.label(),
                    supported_thinking(agent, model)
                        .iter()
                        .map(|level| level.label())
                        .collect(),
                );
            }
            thinking.insert(agent.label(), by_model);
        }

        Self {
            agents: Agent::ALL.iter().map(|agent| agent.label()).collect(),
            models: Model::ALL.iter().map(|model| model.label()).collect(),
            thinking,
            skills: Skill::ALL.iter().map(|skill| skill.label()).collect(),
        }
    }
}

/// 一次模板操作的明确失败类别。
#[derive(Debug)]
pub enum TemplateError {
    NameRequired,
    BotNameRequired,
    BotNameNotSingleLine,
    AgentRequired,
    UnknownAgent(String),
    UnknownModel(String),
    UnknownThinking(String),
    ThinkingRequiresDefaultModel,
    ThinkingUnsupported {
        agent: Agent,
        model: Model,
        thinking: Thinking,
        supported: &'static [Thinking],
    },
    UnknownSkill(String),
    NotFound,
    Unreadable(io::Error),
    Corrupt(String),
    WriteFailed(io::Error),
}

impl TemplateError {
    /// 机器可读的稳定类别。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NameRequired => "template-name-required",
            Self::BotNameRequired => "template-bot-name-required",
            Self::BotNameNotSingleLine => "template-bot-name-not-single-line",
            Self::AgentRequired => "template-agent-required",
            Self::UnknownAgent(_) => "template-agent-invalid",
            Self::UnknownModel(_) => "template-model-invalid",
            Self::UnknownThinking(_) => "template-thinking-invalid",
            Self::ThinkingRequiresDefaultModel => "template-thinking-requires-default-model",
            Self::ThinkingUnsupported { .. } => "template-thinking-unsupported",
            Self::UnknownSkill(_) => "template-skill-invalid",
            Self::NotFound => "template-not-found",
            Self::Unreadable(_) => "templates-unreadable",
            Self::Corrupt(_) => "templates-corrupt",
            Self::WriteFailed(_) => "templates-write-failed",
        }
    }

    /// 面向使用者的说明。
    pub fn message(&self) -> String {
        match self {
            Self::NameRequired => "模板显示名不能为空。".to_string(),
            Self::BotNameRequired => "bot-name 命名规则不能为空。".to_string(),
            Self::BotNameNotSingleLine => {
                "bot-name 必须是单行规则：固定文字与占位符不能包含换行。".to_string()
            }
            Self::AgentRequired => "请选择 agent：omp 或 pi。".to_string(),
            Self::UnknownAgent(value) => {
                format!("agent 只能是 omp 或 pi，收到：{value}。")
            }
            Self::UnknownModel(value) => {
                format!("model 只能沿用默认值、gpt-6.1-sol 或 deepseek-flash，收到：{value}。")
            }
            Self::UnknownThinking(value) => {
                format!(
                    "thinking 不是已定义的思考级别，收到：{value}；请从页面提供的固定选项中选择。"
                )
            }
            Self::ThinkingRequiresDefaultModel => {
                "model 沿用默认值时，thinking 也必须沿用默认值。".to_string()
            }
            Self::ThinkingUnsupported {
                agent,
                model,
                thinking,
                supported,
            } => {
                let options = supported
                    .iter()
                    .map(|level| level.label())
                    .collect::<Vec<_>>()
                    .join("、");
                format!(
                    "thinking「{}」不适用于 {} / {}；该组合可用：{}。",
                    thinking.label(),
                    agent.label(),
                    model.label(),
                    if options.is_empty() {
                        "沿用默认值"
                    } else {
                        options.as_str()
                    }
                )
            }
            Self::UnknownSkill(value) => {
                format!("Skill 只能不选、teach 或 code-review，收到：{value}。")
            }
            Self::NotFound => "目标模板不存在：可能已被删除。".to_string(),
            Self::Unreadable(error) => {
                format!("读取 Bot 模板存储文件失败：{error}；没有把它当成空列表。")
            }
            Self::Corrupt(detail) => {
                format!("Bot 模板存储文件内容无效：{detail}；请先修复该文件。")
            }
            Self::WriteFailed(error) => {
                format!("保存 Bot 模板失败：{error}；此前已保存的模板保持不变。")
            }
        }
    }
}

/// 持久保存在一个 JSON 文件里的 Bot 模板列表。
pub struct Templates {
    storage_file: PathBuf,
    /// 读取与读改写写共用：同一实例内任意时刻只有一个调用者在操作存储文件。
    lock: Mutex<()>,
}

impl Templates {
    pub fn new(storage_file: PathBuf) -> Self {
        Self {
            storage_file,
            lock: Mutex::new(()),
        }
    }

    /// 存储文件位置（启动日志与测试观察用）。
    pub fn storage_file(&self) -> &Path {
        &self.storage_file
    }

    /// 读取当前模板列表；文件尚不存在时是空列表。
    pub async fn list(&self) -> Result<Vec<BotTemplate>, TemplateError> {
        let _guard = self.lock.lock().await;
        self.read()
    }

    /// 按标识读取一个模板；不存在是明确错误。
    pub async fn get(&self, id: &str) -> Result<BotTemplate, TemplateError> {
        let _guard = self.lock.lock().await;
        self.read()?
            .into_iter()
            .find(|template| template.id == id)
            .ok_or(TemplateError::NotFound)
    }

    /// 新增一个模板；成功返回保存后的完整列表。
    pub async fn create(&self, input: TemplateInput) -> Result<Vec<BotTemplate>, TemplateError> {
        let fields = input.validate()?;

        let _guard = self.lock.lock().await;
        let mut templates = self.read()?;
        let id = next_id(&templates);
        templates.push(BotTemplate {
            id,
            name: fields.name,
            bot_name: fields.bot_name,
            agent: fields.agent,
            model: fields.model,
            thinking: fields.thinking,
            skill: fields.skill,
            prompt: fields.prompt,
        });
        self.save(&templates)?;
        Ok(templates)
    }

    /// 编辑一个模板；标识保持不变，只有存储成功后才返回新列表。
    pub async fn update(
        &self,
        id: &str,
        input: TemplateInput,
    ) -> Result<Vec<BotTemplate>, TemplateError> {
        let fields = input.validate()?;

        let _guard = self.lock.lock().await;
        let mut templates = self.read()?;
        let target = templates
            .iter_mut()
            .find(|template| template.id == id)
            .ok_or(TemplateError::NotFound)?;
        target.name = fields.name;
        target.bot_name = fields.bot_name;
        target.agent = fields.agent;
        target.model = fields.model;
        target.thinking = fields.thinking;
        target.skill = fields.skill;
        target.prompt = fields.prompt;
        self.save(&templates)?;
        Ok(templates)
    }

    /// 删除一个模板；不改变任何已经创建的 Bot。
    pub async fn delete(&self, id: &str) -> Result<Vec<BotTemplate>, TemplateError> {
        let _guard = self.lock.lock().await;
        let mut templates = self.read()?;
        let before = templates.len();
        templates.retain(|template| template.id != id);
        if templates.len() == before {
            return Err(TemplateError::NotFound);
        }
        self.save(&templates)?;
        Ok(templates)
    }

    /// 读取并校验存储文件；文件不存在是空列表，其余失败都是明确错误。
    fn read(&self) -> Result<Vec<BotTemplate>, TemplateError> {
        match fs::read_to_string(&self.storage_file) {
            Ok(raw) => {
                let document: TemplatesDocument = serde_json::from_str(&raw)
                    .map_err(|error| TemplateError::Corrupt(error.to_string()))?;
                validate_document(&document.templates)?;
                Ok(document.templates)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(TemplateError::Unreadable(error)),
        }
    }

    /// 先把新内容写进同目录的临时文件并落盘，再原子替换存储文件。
    ///
    /// 任一步失败都删除临时文件并返回错误；原存储文件在替换成功之前保持原样。
    fn save(&self, templates: &[BotTemplate]) -> Result<(), TemplateError> {
        let document = TemplatesDocument {
            templates: templates.to_vec(),
        };
        let content = serde_json::to_string_pretty(&document).map_err(|error| {
            TemplateError::WriteFailed(io::Error::new(io::ErrorKind::InvalidData, error))
        })?;

        let temporary = self.temporary_path();
        let written = (|| -> io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(TemplateError::WriteFailed(error));
        }
        if let Err(error) = fs::rename(&temporary, &self.storage_file) {
            let _ = fs::remove_file(&temporary);
            return Err(TemplateError::WriteFailed(error));
        }
        Ok(())
    }

    fn temporary_path(&self) -> PathBuf {
        let mut name = self
            .storage_file
            .file_name()
            .map(|name| name.to_os_string())
            .unwrap_or_else(|| OsString::from(DEFAULT_STORAGE_FILE));
        name.push(".tmp");
        self.storage_file.with_file_name(name)
    }
}

/// 存储文件的完整内容。
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplatesDocument {
    templates: Vec<BotTemplate>,
}

/// 存储文件里的每一项都必须满足与公开保存相同的固定规则，且标识不能重复。
fn validate_document(templates: &[BotTemplate]) -> Result<(), TemplateError> {
    let mut seen = Vec::with_capacity(templates.len());
    for template in templates {
        if template.id.trim().is_empty() {
            return Err(TemplateError::Corrupt("模板缺少标识".to_string()));
        }
        if seen.contains(&template.id) {
            return Err(TemplateError::Corrupt("模板标识重复".to_string()));
        }
        seen.push(template.id.clone());

        if template.name.trim().is_empty() {
            return Err(TemplateError::Corrupt(format!(
                "模板 {} 缺少显示名",
                template.id
            )));
        }
        if template.bot_name.trim().is_empty()
            || template.bot_name.contains(|c| c == '\n' || c == '\r')
        {
            return Err(TemplateError::Corrupt(format!(
                "模板 {} 的 bot-name 不是合法单行规则",
                template.id
            )));
        }
        if template.model == Model::Default && template.thinking != Thinking::Default {
            return Err(TemplateError::Corrupt(format!(
                "模板 {} 在默认模型下指定了 thinking",
                template.id
            )));
        }
        if template.thinking != Thinking::Default
            && !supported_thinking(template.agent, template.model).contains(&template.thinking)
        {
            return Err(TemplateError::Corrupt(format!(
                "模板 {} 的 thinking 不适用于所选 agent / model",
                template.id
            )));
        }
    }
    Ok(())
}

/// 生成一个不与现有模板重复的标识。
fn next_id(templates: &[BotTemplate]) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let candidate = format!(
            "tpl-{nanos:x}-{:x}",
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        if !templates.iter().any(|template| template.id == candidate) {
            return candidate;
        }
    }
}
