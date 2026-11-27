use std::borrow::Cow;

use crate::scheduler::maps::OutputRule;

use super::{Field, PromptDictionary, Snapshot, context::{self, ChiefDelegationContext, ManagerContext, NodeContext}, roles};

impl PromptDictionary {
    pub(crate) fn task(&self, fixed: &str, skill: Option<&str>, input: &str) -> anyhow::Result<String> {
        self.assemble(fixed, skill, "", |_| Ok(Cow::Borrowed(input)))
    }

    pub(crate) fn chief(&self, fixed: &str, skill: Option<&str>, input: &str) -> anyhow::Result<String> {
        self.assemble(fixed, skill, roles::CHIEF, |_| Ok(Cow::Borrowed(input)))
    }

    pub(crate) fn manager(&self, fixed: &str, skill: Option<&str>, facts: &ManagerContext<'_>, chief_outputs: Option<&[OutputRule]>) -> anyhow::Result<String> {
        self.assemble(fixed, skill, "", |_| Ok(Cow::Owned(context::manager(facts, chief_outputs))))
    }

    pub(crate) fn node(&self, fixed: &str, skill: Option<&str>, facts: &NodeContext<'_>) -> anyhow::Result<String> {
        self.assemble(fixed, skill, "", |snapshot| {
            let input = snapshot.render(Field::NodeInput, facts.input)?;
            Ok(Cow::Owned(context::node(facts, &input)?))
        })
    }

    pub(crate) fn chief_delegated_task(&self, fixed: &str, skill: Option<&str>, facts: &ChiefDelegationContext<'_>, input: &str) -> anyhow::Result<String> {
        self.assemble(fixed, skill, "", |_| Ok(Cow::Owned(context::chief_delegated_task(facts, input))))
    }

    fn assemble<'a>(
        &self,
        task_prompt: &str,
        skill: Option<&str>,
        role: &str,
        input: impl FnOnce(&Snapshot) -> anyhow::Result<Cow<'a, str>>,
    ) -> anyhow::Result<String> {
        let snapshot = self.snapshot()?;
        let fixed = snapshot.render(Field::TaskPrompt, task_prompt)?;
        let input = input(&snapshot)?;
        let instructions = snapshot.instructions(task_prompt);
        Ok(join(skill, [fixed.as_ref(), role, &input, &instructions]))
    }
}

/// A native Skill is the single leading `/skill:<name>` invocation. The
/// remaining sections are already rendered or literal and are never rescanned.
fn join(skill: Option<&str>, sections: [&str; 4]) -> String {
    let sections = sections.into_iter().filter(|part| !part.is_empty());
    let body_len = sections.clone().map(|part| part.len() + 2).sum::<usize>().saturating_sub(2);
    let skill_len = skill.map_or(0, |skill| "/skill:".len() + skill.len() + usize::from(body_len != 0));
    let mut text = String::with_capacity(body_len + skill_len);
    if let Some(skill) = skill {
        text.push_str("/skill:");
        text.push_str(skill);
        if body_len != 0 {
            text.push(' ');
        }
    }
    let mut sections = sections;
    if let Some(first) = sections.next() {
        text.push_str(first);
        for part in sections {
            text.push_str("\n\n");
            text.push_str(part);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::join;

    #[test]
    fn plain_input_is_submitted_once_without_extra_prefix() {
        assert_eq!(join(None, ["", "", "检查登录\n第二行", ""]), "检查登录\n第二行");
    }

    #[test]
    fn skill_uses_one_native_invocation_prefix() {
        assert_eq!(join(Some("code-review"), ["固定指示", "", "本次任务", ""]),
            "/skill:code-review 固定指示\n\n本次任务");
    }
}
