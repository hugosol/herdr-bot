use std::{fmt::Write, path::{Path, PathBuf}};

use crate::{chief_delegation::Material, scheduler::maps::{OutputRule, ResolvedFile}};

use super::roles;

pub(crate) struct RunFiles<'a> {
    pub instructions: &'a Path,
    pub records: [PathBuf; 3],
    pub materials: PathBuf,
}

impl RunFiles<'_> {
    fn write_records(&self, text: &mut String) {
        let [state, history, reports] = &self.records;
        write!(text, "当前状态: {}\n执行历史: {}\n报告记录: {}",
            state.display(), history.display(), reports.display()).unwrap();
    }
}

pub(crate) struct ManagerContext<'a> {
    pub id: &'a str,
    pub map: PathBuf,
    pub files: RunFiles<'a>,
}

pub(crate) struct NodeContext<'a> {
    pub background: &'a str,
    pub input: &'a str,
    pub files: &'a [ResolvedFile],
    /// No node report role for a Chat handed off to the user at an end node.
    pub role: Option<NodeRole<'a>>,
    pub child: Option<ManagerContext<'a>>,
}

pub(crate) struct NodeRole<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub run_files: RunFiles<'a>,
    pub output_rule: Option<&'a [OutputRule]>,
    pub auto_approved: bool,
}

pub(crate) struct ChiefDelegationContext<'a> {
    pub id: &'a str,
    pub bot_id: &'a str,
    pub chief_session_id: &'a str,
    pub zone_id: &'a str,
    pub cwd: &'a str,
    pub materials: &'a [Material],
}

fn write_manager_facts(text: &mut String, context: &ManagerContext<'_>, child: bool) {
    writeln!(text, "Run ID: {}\nMap: {}", context.id, context.map.display()).unwrap();
    if !child {
        writeln!(text, "运行说明: {}", context.files.instructions.display()).unwrap();
    }
    context.files.write_records(text);
    write!(text, "\n中间资料位置: {}", context.files.materials.display()).unwrap();
}

fn write_manager_contract(text: &mut String, child: bool) {
    write!(text, "\n\n[End node contract] {} [/End node contract]", roles::manager_end_node_contract(child)).unwrap();
}

pub(super) fn manager(context: &ManagerContext<'_>, input: &str) -> String {
    let mut text = String::new();
    write_manager_facts(&mut text, context, false);
    write_manager_contract(&mut text, false);
    if !input.is_empty() {
        text.push_str("\n\n");
        text.push_str(input);
    }
    text
}

pub(super) fn node(context: &NodeContext<'_>, rendered_input: &str) -> serde_json::Result<String> {
    let mut text = String::new();
    if let Some(role) = &context.role {
        writeln!(text, "Node ID: {}\nNode: {}\n运行说明: {}", role.id, role.title, role.run_files.instructions.display()).unwrap();
        role.run_files.write_records(&mut text);
        write!(text, "\n\n中间资料位置: {}\n\n", role.run_files.materials.display()).unwrap();
    }
    write!(text, "[Run background / userInput]\n{}\n[/Run background / userInput]\n\n[Named file inputs]\n", context.background).unwrap();
    for file in context.files {
        writeln!(text, "{}: {}", file.id, file.path).unwrap();
    }
    text.push_str("[/Named file inputs]\n\n");
    if let Some(role) = &context.role {
        let output_rule = serde_json::to_string(&role.output_rule)?;
        write!(text, "[Output rules]\n{output_rule}\n[/Output rules]\n\n[Result approval control]\nauto_approved: {}\n{}\n[/Result approval control]\n\n",
            role.auto_approved, roles::NODE_APPROVAL).unwrap();
    }
    write!(text, "[Business input]\n{rendered_input}").unwrap();
    if let Some(child) = &context.child {
        text.push_str("\n\n[Child Run manager context]\n");
        write_manager_facts(&mut text, child, true);
        text.push_str("\n[/Child Run manager context]");
        write_manager_contract(&mut text, true);
    }
    Ok(text)
}

pub(super) fn chief_delegated_task(context: &ChiefDelegationContext<'_>, input: &str) -> String {
    let facts = serde_json::json!({
        "delegationId": context.id,
        "botId": context.bot_id,
        "sourceChiefSessionId": context.chief_session_id,
        "zoneId": context.zone_id,
        "cwd": context.cwd,
        "materials": context.materials,
    });
    format!("{input}\n\n[Chief delegation]\n{}", serde_json::to_string_pretty(&facts).expect("context serialization"))
}
