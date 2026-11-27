use std::{fmt::Write, path::{Path, PathBuf}};

use crate::{chief_delegation::Material, runs::MapInput, scheduler::maps::{OutputRule, ResolvedFile}};

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
    pub input: &'a MapInput,
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
    pub output_rule: &'a [OutputRule],
}

fn write_manager(text: &mut String, context: &ManagerContext<'_>) {
    writeln!(text, "{}\n\nRun ID: {}\nMap: {}\n运行说明: {}",
        roles::MANAGER, context.id, context.map.display(), context.files.instructions.display()).unwrap();
    context.files.write_records(text);
    write!(text, "\n中间资料位置: {}", context.files.materials.display()).unwrap();
}

fn write_named_files(text: &mut String, files: &[ResolvedFile]) {
    text.push_str("[Named file inputs]\n");
    for file in files {
        writeln!(text, "{}: {}", file.id, file.path).unwrap();
    }
    text.push_str("[/Named file inputs]");
}

pub(super) fn manager(context: &ManagerContext<'_>, chief_outputs: Option<&[OutputRule]>) -> String {
    let mut text = String::new();
    write_manager(&mut text, context);
    if let Some(outputs) = chief_outputs {
        text.push_str("\n\n");
        text.push_str(roles::CHIEF_DELEGATION);
        write_chief_output_rule(&mut text, outputs);
    }
    if !context.input.files.is_empty() {
        text.push_str("\n\n");
        write_named_files(&mut text, &context.input.files);
    }
    if !context.input.run_input.is_empty() {
        text.push_str("\n\n");
        text.push_str(&context.input.run_input);
    }
    text
}

pub(super) fn node(context: &NodeContext<'_>, rendered_input: &str) -> serde_json::Result<String> {
    let mut text = String::new();
    if let Some(role) = &context.role {
        writeln!(text, "{}\n\nNode ID: {}\nNode: {}\n运行说明: {}", roles::NODE, role.id, role.title, role.run_files.instructions.display()).unwrap();
        role.run_files.write_records(&mut text);
        write!(text, "\n\n中间资料位置: {}\n\n", role.run_files.materials.display()).unwrap();
    }
    write!(text, "[Run background / runInput]\n{}\n[/Run background / runInput]\n\n", context.background).unwrap();
    write_named_files(&mut text, context.files);
    text.push_str("\n\n");
    if let Some(role) = &context.role {
        let output_rule = serde_json::to_string(&role.output_rule)?;
        write!(text, "[Output rules]\n{output_rule}\n[/Output rules]\n\n[Result approval control]\n本次成果批准模式为 {}。{}\n[/Result approval control]\n\n",
            role.auto_approved, roles::NODE_APPROVAL).unwrap();
    }
    write!(text, "[Business input]\n{rendered_input}").unwrap();
    if let Some(child) = &context.child {
        text.push_str("\n[/Business input]\n\n[Child Run manager context]\n");
        write_manager(&mut text, child);
        write!(text, "\n\n{}\n[/Child Run manager context]", roles::CHILD_MANAGER).unwrap();
    }
    Ok(text)
}

fn write_chief_output_rule(text: &mut String, outputs: &[OutputRule]) {
    if !outputs.is_empty() {
        let requirements = serde_json::to_string(outputs).expect("output rule serialization");
        write!(text, "\n\n[Requested file outputs]\n{requirements}\n[/Requested file outputs]").unwrap();
    }
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
    let mut text = format!("{input}\n\n[Chief delegation]\n{}\n\n{}", roles::CHIEF_DELEGATION, serde_json::to_string_pretty(&facts).expect("context serialization"));
    write_chief_output_rule(&mut text, context.output_rule);
    text
}
