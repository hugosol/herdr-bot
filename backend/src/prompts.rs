//! Machine-local prompt values. Read one fresh snapshot per outgoing message;
//! render only explicitly typed definition fields, never user input or reports.
use std::{collections::BTreeMap, io, path::PathBuf};

use anyhow::Context;
use crate::files::FileStore;

#[derive(Debug, Clone, Copy)]
pub enum Field {
    TaskPrompt,
    BotWhen,
    BotPlanGuide,
    BotNodeGuide,
    MapDescription,
    NodeInput,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::TaskPrompt => "Task.prompt",
            Self::BotWhen => "Bot.when",
            Self::BotPlanGuide => "Bot.plan_guide",
            Self::BotNodeGuide => "Bot.node_guide",
            Self::MapDescription => "Map.description",
            Self::NodeInput => "node.input",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PromptDictionary {
    path: PathBuf,
}

pub struct Snapshot {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

impl PromptDictionary {
    pub fn new(path: PathBuf) -> Self {
        Self { path: std::path::absolute(path).expect("resolve prompt dictionary path") }
    }

    /// An absent optional local dictionary is empty; other I/O and syntax errors
    /// remain errors even when the current message contains no placeholders.
    pub fn snapshot(&self) -> anyhow::Result<Snapshot> {
        let raw = match FileStore::default().read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error).with_context(|| format!("读取提示词词典失败：{}", self.path.display())),
        };
        let values = toml::from_str(&raw)
            .with_context(|| format!("解析提示词词典失败（仅支持平铺字符串值）：{}", self.path.display()))?;
        Ok(Snapshot { path: self.path.clone(), values })
    }
}

impl Snapshot {
    /// Scan the source once. Substituted values and escaped placeholders are
    /// literal text and are never scanned again. Unclosed syntax stays literal.
    pub fn render(&self, field: Field, source: &str) -> anyhow::Result<String> {
        let mut rendered = String::with_capacity(source.len());
        let mut rest = source;
        while let Some(start) = rest.find('$') {
            rendered.push_str(&rest[..start]);
            rest = &rest[start..];
            let escaped = rest.starts_with("$${");
            let prefix = if escaped { 3 } else if rest.starts_with("${") { 2 } else {
                rendered.push('$');
                rest = &rest[1..];
                continue;
            };
            let Some(end) = rest[prefix..].find('}').map(|end| prefix + end) else {
                rendered.push_str(rest);
                return Ok(rendered);
            };
            if escaped {
                rendered.push_str(&rest[1..=end]);
            } else {
                let key = &rest[prefix..end];
                let value = self.values.get(key).with_context(|| format!(
                    "提示词字段 {} 缺少词典键 {:?}（{}）；请配置该键，不猜测值。",
                    field.name(), key, self.path.display()))?;
                rendered.push_str(value);
            }
            rest = &rest[end + 1..];
        }
        rendered.push_str(rest);
        Ok(rendered)
    }

    pub fn instructions(&self) -> String {
        format!("[Prompt dictionary instructions]\n本机提示词词典：{}\n词典是平铺字符串 TOML。直接读取 Task.prompt、Bot.when/plan_guide/node_guide、Map.description 或 node.input 原始定义时，对实际使用的字段按同一次最新词典快照解析 ${{key}}；$${{key}} 表示字面 ${{key}}。只扫描原文一次，替换值不递归，不执行表达式或命令。后端已合成的正文已完成替换，不再插值。用户 Chat 输入、Run userInput、报告、输出与任意文件内容都不插值。保存定义保留原始占位符，允许暂缺键；词典文件缺失视为空词典，无模板字段仍可使用；实际使用字段缺少键时停止并报告字段和键，不猜测路径或地址。Bot/Map 文件直接读取保持原文，不代表所有指南都应自动执行。\n[/Prompt dictionary instructions]", self.path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        let path = std::env::temp_dir().join(format!("herdr-prompt-values-{}.toml", crate::chats::new_creation_id()));
        FileStore::default().with(&[path.clone()], |access| access.write(&path,
            "repo = 'D:/工作/${not-expanded}'\napi = 'http://127.0.0.1:4321'\n".as_bytes())).unwrap().unwrap();
        let snapshot = PromptDictionary::new(path.clone()).snapshot().unwrap();
        FileStore::default().with(&[path.clone()], |access| access.remove_file(&path)).unwrap().unwrap();
        snapshot
    }

    #[test]
    fn renders_once_and_preserves_escaped_keys_and_literal_syntax() {
        assert_eq!(snapshot().render(Field::TaskPrompt, "${repo} $${missing} ${api} $x ${open").unwrap(),
            "D:/工作/${not-expanded} ${missing} http://127.0.0.1:4321 $x ${open");
    }

    #[test]
    fn every_known_field_reports_its_missing_key() {
        for field in [Field::TaskPrompt, Field::BotWhen, Field::BotPlanGuide, Field::BotNodeGuide, Field::MapDescription, Field::NodeInput] {
            let message = snapshot().render(field, "${unknown}").unwrap_err().to_string();
            assert!(message.contains(field.name()));
            assert!(message.contains("unknown"));
        }
    }

    #[test]
    fn snapshots_reload_without_changing_existing_message_values() {
        let path = std::env::temp_dir().join(format!("herdr-prompt-dictionary-{}.toml", crate::chats::new_creation_id()));
        let dictionary = PromptDictionary::new(path.clone());
        assert_eq!(dictionary.snapshot().unwrap().render(Field::TaskPrompt, "plain").unwrap(), "plain");
        let files = FileStore::default();
        files.with(&[path.clone()], |access| access.write(&path, b"repo = 'first'")).unwrap().unwrap();
        let first = dictionary.snapshot().unwrap();
        files.with(&[path.clone()], |access| access.write(&path, b"repo = 'second'")).unwrap().unwrap();
        assert_eq!(first.render(Field::TaskPrompt, "${repo}").unwrap(), "first");
        assert_eq!(dictionary.snapshot().unwrap().render(Field::TaskPrompt, "${repo}").unwrap(), "second");
        files.with(&[path.clone()], |access| access.write(&path, b"repo = 42")).unwrap().unwrap();
        assert!(dictionary.snapshot().is_err());
        files.with(&[path.clone()], |access| access.remove_file(&path)).unwrap().unwrap();
    }
}
