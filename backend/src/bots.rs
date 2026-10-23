//! Bot：由后端以文件持久保存，电脑与手机共享同一份。
//!
//! Bot 是创建 Chat 时复用的预设：显示名、Chat 自动命名规则、agent、模型、思考级别、可选 Skill
//! 与固定 prompt。Bot 只在创建时被读取，编辑或删除不改变已经创建的 Chat。
//!
//! 持久化语义与 zone 登记相同：只有存储文件成功写入后才返回成功；写入先写同目录的临时文件并
//! `sync_all`，再原子替换存储文件。文件不存在表示还没有任何 Bot，不是读取错误；读取失败或内容
//! 损坏都是明确错误，不会伪装成空列表，也不会覆盖原文件。同一实例内的读取与写入共用一把锁。
//!
//! agent / model / thinking / Skill 的可用选项由独立 TOML 目录维护；每次保存按当前元数据校验。
//! 持久文件只保留稳定标识，不保存映射副本；读取不因单个选择失效而丢弃其他 Bot。

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Bot 存储文件在配置缺省时的相对路径（相对进程工作目录，与既有配置路径语义一致）。
pub const DEFAULT_STORAGE_FILE: &str = ".data/bots.json";

/// Bot 可选的 agent 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Omp,
    Pi,
}

impl Agent {
    /// 序列化后的固定值（存入存储文件、HTTP 响应也使用同一个值）。
    pub fn label(self) -> &'static str {
        match self {
            Agent::Omp => "omp",
            Agent::Pi => "pi",
        }
    }
}

/// 一个已保存的 Bot。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bot {
    /// 稳定标识；编辑以它为定位，不依赖可改动的显示名。
    pub id: String,
    /// Bot 显示名，不是新 Chat 的固定名字。
    pub name: String,
    /// Chat 自动命名规则：单行固定文字与占位符。
    pub chat_name: String,
    pub agent: Agent,
    // Flattening preserves the existing HTTP and persisted Bot format; these
    // are distinct execution inputs, not a new disk format or legacy aliases.
    #[serde(flatten)]
    pub trigger: TriggerConfig,
    #[serde(flatten)]
    pub env: EnvConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub model: String,
    pub thinking: String,
    /// 不选 Skill 时为 `None`。
    pub skill: Option<String>,
    /// 可多行、可为空的固定初始指示，只在发起时提交。
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvConfig {
    /// Stable IDs of optional extensions; fixed extensions belong to the catalog.
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// 一次新增或编辑请求的原始字段；字符串先按字段逐个校验，失败时给出明确类别。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BotInput {
    name: Option<String>,
    chat_name: Option<String>,
    agent: Option<String>,
    model: Option<String>,
    thinking: Option<String>,
    skill: Option<String>,
    prompt: Option<String>,
    #[serde(default)]
    extensions: Vec<String>,
}

/// 校验后的 Bot 字段（不含标识）。
struct BotFields {
    name: String,
    chat_name: String,
    agent: Agent,
    model: String,
    thinking: String,
    skill: Option<String>,
    prompt: String,
    extensions: Vec<String>,
}

impl BotInput {
    fn validate(self, catalog: &crate::catalog::Catalog) -> Result<BotFields, BotError> {
        let name = self.name.unwrap_or_default();
        if name.trim().is_empty() {
            return Err(BotError::NameRequired);
        }

        let chat_name = self.chat_name.unwrap_or_default();
        if chat_name.trim().is_empty() {
            return Err(BotError::ChatNameRequired);
        }
        if chat_name.contains(|c| c == '\n' || c == '\r') {
            return Err(BotError::ChatNameNotSingleLine);
        }

        let agent = self
            .agent
            .as_deref()
            .ok_or(BotError::AgentRequired)?
            .trim();
        let agent = catalog
            .agents
            .iter()
            .find(|candidate| candidate.id.label() == agent)
            .map(|candidate| candidate.id)
            .ok_or_else(|| BotError::UnknownAgent(agent.to_string()))?;

        let model = self
            .model
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("default")
            .to_owned();
        let thinking = self
            .thinking
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("default")
            .to_owned();
        let skill = self
            .skill
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.strip_prefix('/').unwrap_or(v).to_owned());
        catalog.validate_selection(agent, &model, &thinking, skill.as_deref())?;
        catalog.validate_extensions(agent, &self.extensions)?;

        Ok(BotFields {
            name,
            chat_name,
            agent,
            model,
            thinking,
            skill,
            prompt: self.prompt.unwrap_or_default(),
            extensions: self.extensions,
        })
    }
}

/// 一次 Bot 操作的明确失败类别。
#[derive(Debug)]
pub enum BotError {
    NameRequired,
    ChatNameRequired,
    ChatNameNotSingleLine,
    AgentRequired,
    UnknownAgent(String),
    UnknownModel(String),
    UnknownThinking(String),
    ThinkingRequiresDefaultModel,
    ThinkingUnsupported {
        agent: Agent,
        model: String,
        thinking: String,
        supported: Vec<String>,
    },
    UnknownSkill(String),
    InvalidExtension(String),
    Metadata(String),
    NotFound,
    Unreadable(io::Error),
    Corrupt(String),
    WriteFailed(io::Error),
}

impl BotError {
    /// 机器可读的稳定类别。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NameRequired => "bot-name-required",
            Self::ChatNameRequired => "bot-chat-name-required",
            Self::ChatNameNotSingleLine => "bot-chat-name-not-single-line",
            Self::AgentRequired => "bot-agent-required",
            Self::UnknownAgent(_) => "bot-agent-invalid",
            Self::UnknownModel(_) => "bot-model-invalid",
            Self::UnknownThinking(_) => "bot-thinking-invalid",
            Self::ThinkingRequiresDefaultModel => "bot-thinking-requires-default-model",
            Self::ThinkingUnsupported { .. } => "bot-thinking-unsupported",
            Self::UnknownSkill(_) => "bot-skill-invalid",
            Self::InvalidExtension(_) => "bot-extension-invalid",
            Self::Metadata(_) => "metadata-invalid",
            Self::NotFound => "bot-not-found",
            Self::Unreadable(_) => "bots-unreadable",
            Self::Corrupt(_) => "bots-corrupt",
            Self::WriteFailed(_) => "bots-write-failed",
        }
    }

    /// 面向使用者的说明。
    pub fn message(&self) -> String {
        match self {
            Self::NameRequired => "Bot显示名不能为空。".to_string(),
            Self::ChatNameRequired => "chat-name 命名规则不能为空。".to_string(),
            Self::ChatNameNotSingleLine => {
                "chat-name 必须是单行规则：固定文字与占位符不能包含换行。".to_string()
            }
            Self::AgentRequired => "请选择当前选项目录中的 agent。".to_string(),
            Self::UnknownAgent(value) => {
                format!("agent 不在当前选项目录中，收到：{value}。")
            }
            Self::UnknownModel(value) => {
                format!("model 不在当前 Agent 的可用选项目录中，收到：{value}。")
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
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join("、");
                format!(
                    "thinking「{}」不适用于 {} / {}；该组合可用：{}。",
                    thinking,
                    agent.label(),
                    model,
                    if options.is_empty() {
                        "沿用默认值"
                    } else {
                        options.as_str()
                    }
                )
            }
            Self::UnknownSkill(value) => {
                format!("Skill 不在当前选项目录中，收到：{value}；可不选 Skill。")
            }
            Self::InvalidExtension(value) => {
                format!(
                    "扩展「{value}」不是当前 Agent 的可选扩展，或已重复选择；请按当前选项目录修正。"
                )
            }
            Self::Metadata(detail) => detail.clone(),
            Self::NotFound => "目标Bot不存在：可能已被删除。".to_string(),
            Self::Unreadable(error) => {
                format!("读取 Bot存储文件失败：{error}；没有把它当成空列表。")
            }
            Self::Corrupt(detail) => {
                format!("Bot存储文件内容无效：{detail}；请先修复该文件。")
            }
            Self::WriteFailed(error) => {
                format!("保存 Bot失败：{error}；此前已保存的Bot保持不变。")
            }
        }
    }
}

/// 持久保存在一个 JSON 文件里的 Bot 列表。
pub struct Bots {
    storage_file: PathBuf,
    metadata_file: PathBuf,
    /// 读取与读改写写共用：同一实例内任意时刻只有一个调用者在操作存储文件。
    lock: Mutex<()>,
}

impl Bots {
    pub fn new(storage_file: PathBuf, metadata_file: PathBuf) -> Self {
        Self {
            storage_file,
            metadata_file,
            lock: Mutex::new(()),
        }
    }

    pub fn catalog(&self) -> Result<crate::catalog::Catalog, BotError> {
        crate::catalog::Catalog::load(&self.metadata_file)
    }

    /// 存储文件位置（启动日志与测试观察用）。
    pub fn storage_file(&self) -> &Path {
        &self.storage_file
    }

    /// 读取当前 Bot 列表；文件尚不存在时是空列表。
    pub async fn list(&self) -> Result<Vec<Bot>, BotError> {
        let _guard = self.lock.lock().await;
        self.read()
    }

    /// 按标识读取一个 Bot；不存在是明确错误。
    pub async fn get(&self, id: &str) -> Result<Bot, BotError> {
        let _guard = self.lock.lock().await;
        self.read()?
            .into_iter()
            .find(|bot| bot.id == id)
            .ok_or(BotError::NotFound)
    }

    /// 新增一个 Bot；成功返回保存后的完整列表。
    pub async fn create(&self, input: BotInput) -> Result<Vec<Bot>, BotError> {
        let fields = input.validate(&self.catalog()?)?;

        let _guard = self.lock.lock().await;
        let mut bots = self.read()?;
        let id = next_id(&bots);
        bots.push(Bot {
            id,
            name: fields.name,
            chat_name: fields.chat_name,
            agent: fields.agent,
            trigger: TriggerConfig {
                model: fields.model,
                thinking: fields.thinking,
                skill: fields.skill,
                prompt: fields.prompt,
            },
            env: EnvConfig { extensions: fields.extensions },
        });
        self.save(&bots)?;
        Ok(bots)
    }

    /// 编辑一个 Bot；标识保持不变，只有存储成功后才返回新列表。
    pub async fn update(
        &self,
        id: &str,
        input: BotInput,
    ) -> Result<Vec<Bot>, BotError> {
        let fields = input.validate(&self.catalog()?)?;

        let _guard = self.lock.lock().await;
        let mut bots = self.read()?;
        let target = bots
            .iter_mut()
            .find(|bot| bot.id == id)
            .ok_or(BotError::NotFound)?;
        target.name = fields.name;
        target.chat_name = fields.chat_name;
        target.agent = fields.agent;
        target.trigger = TriggerConfig {
            model: fields.model,
            thinking: fields.thinking,
            skill: fields.skill,
            prompt: fields.prompt,
        };
        target.env = EnvConfig { extensions: fields.extensions };
        self.save(&bots)?;
        Ok(bots)
    }

    /// 删除一个 Bot；不改变任何已经创建的 Chat。
    pub async fn delete(&self, id: &str) -> Result<Vec<Bot>, BotError> {
        let _guard = self.lock.lock().await;
        let mut bots = self.read()?;
        let before = bots.len();
        bots.retain(|bot| bot.id != id);
        if bots.len() == before {
            return Err(BotError::NotFound);
        }
        self.save(&bots)?;
        Ok(bots)
    }

    /// 读取并校验存储文件；文件不存在是空列表，其余失败都是明确错误。
    fn read(&self) -> Result<Vec<Bot>, BotError> {
        match fs::read_to_string(&self.storage_file) {
            Ok(raw) => {
                let document: BotsDocument = serde_json::from_str(&raw)
                    .map_err(|error| BotError::Corrupt(error.to_string()))?;
                validate_document(&document.bots)?;
                Ok(document.bots)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(BotError::Unreadable(error)),
        }
    }

    /// 先把新内容写进同目录的临时文件并落盘，再原子替换存储文件。
    ///
    /// 任一步失败都删除临时文件并返回错误；原存储文件在替换成功之前保持原样。
    fn save(&self, bots: &[Bot]) -> Result<(), BotError> {
        let document = BotsDocument {
            bots: bots.to_vec(),
        };
        let content = serde_json::to_string_pretty(&document).map_err(|error| {
            BotError::WriteFailed(io::Error::new(io::ErrorKind::InvalidData, error))
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
            return Err(BotError::WriteFailed(error));
        }
        if let Err(error) = fs::rename(&temporary, &self.storage_file) {
            let _ = fs::remove_file(&temporary);
            return Err(BotError::WriteFailed(error));
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
struct BotsDocument {
    bots: Vec<Bot>,
}

/// 校验持久文件结构和 Bot 身份；选项适用性只在保存/创建时按当前目录校验。
fn validate_document(bots: &[Bot]) -> Result<(), BotError> {
    let mut seen = Vec::with_capacity(bots.len());
    for bot in bots {
        if bot.id.trim().is_empty() {
            return Err(BotError::Corrupt("Bot缺少标识".to_string()));
        }
        if seen.contains(&bot.id) {
            return Err(BotError::Corrupt("Bot标识重复".to_string()));
        }
        seen.push(bot.id.clone());

        if bot.name.trim().is_empty() {
            return Err(BotError::Corrupt(format!(
                "Bot {} 缺少显示名",
                bot.id
            )));
        }
        if bot.chat_name.trim().is_empty()
            || bot.chat_name.contains(|c| c == '\n' || c == '\r')
        {
            return Err(BotError::Corrupt(format!(
                "Bot {} 的 chat-name 不是合法单行规则",
                bot.id
            )));
        }
    }
    Ok(())
}

/// 生成一个不与现有 Bot 重复的标识。
fn next_id(bots: &[Bot]) -> String {
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
        if !bots.iter().any(|bot| bot.id == candidate) {
            return candidate;
        }
    }
}
