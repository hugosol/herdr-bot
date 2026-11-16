//! Chat identities remain required for old executions; Rust work has no Chat.
use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExecutionTarget {
    Script(ScriptExecution),
    Chat(ChatExecution),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatExecution {
    pub task_id: String,
    pub agent: crate::tasks::Agent,
    pub chat: Option<ChatReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_run_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptKind { RustScript }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptExecution {
    pub kind: ScriptKind,
    pub rust_script: String,
    pub lifecycle: Lifecycle,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Lifecycle {
    Running { #[serde(rename = "startedAt")] started_at: u64 },
    Succeeded { #[serde(rename = "startedAt")] started_at: u64, #[serde(rename = "endedAt")] ended_at: u64 },
    Failed { #[serde(rename = "startedAt")] started_at: u64, #[serde(rename = "endedAt")] ended_at: u64, failure: serde_json::Value },
}
impl NodeExecution {
    pub(super) fn chat(&self) -> Option<&ChatReference> { match &self.target { ExecutionTarget::Chat(chat) => chat.chat.as_ref(), _ => None } }
    pub(super) fn chat_mut(&mut self) -> &mut Option<ChatReference> { match &mut self.target { ExecutionTarget::Chat(chat) => &mut chat.chat, _ => unreachable!("script cannot acquire a Chat") } }
    pub(super) fn child_run_id(&self) -> Option<&String> { match &self.target { ExecutionTarget::Chat(chat) => chat.child_run_id.as_ref(), _ => None } }
    pub(super) fn child_run_id_mut(&mut self) -> &mut Option<String> { match &mut self.target { ExecutionTarget::Chat(chat) => &mut chat.child_run_id, _ => unreachable!("script cannot acquire a child Run") } }
    pub(super) fn task_id(&self) -> Option<&str> { match &self.target { ExecutionTarget::Chat(chat) => Some(&chat.task_id), _ => None } }
    pub(super) fn agent(&self) -> Option<crate::tasks::Agent> { match &self.target { ExecutionTarget::Chat(chat) => Some(chat.agent), _ => None } }
    pub(super) fn script(&self) -> Option<&ScriptExecution> { match &self.target { ExecutionTarget::Script(script) => Some(script), _ => None } }
}
