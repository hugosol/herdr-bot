//! Fixed blank-creation and exact three-guide merge businesses. Named files
//! are the only parameters; both use the public Bot save's revision/path checks.
use super::{Bot, BotSave, bot_target, save_bot_definition};
use crate::scheduler::{DefinitionRevision, edit_error, prepare_directory, with_definitions};
use crate::scheduler::maps::{ResolvedFile, map_error, read_map};
use crate::files::{FileStore, PublishMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use serde_json::{Value, json};

pub(crate) struct Outcome {
    pub failure: Option<Value>,
    pub outputs: Vec<ResolvedFile>,
}
struct Work {
    write: &'static str,
    read_back: &'static str,
    materials: Vec<Value>,
    outputs: Vec<ResolvedFile>,
}
impl Work {
    fn failure(&self, stage: &str, code: &str, message: impl ToString) -> Value {
        json!({"stage":stage,"code":code,"message":message.to_string(),"effects":{"write":self.write,"readBack":self.read_back,"materials":self.materials}})
    }
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum CreateTarget { Task { task_id: String }, Map { map_file: String } }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    #[serde(default, deserialize_with = "present")]
    id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    group: Option<String>,
}
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<T>, D::Error> { T::deserialize(deserializer).map(Some) }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authorization { create_blank: bool, write_materials: bool, save_guides: bool }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirmation { evidence: String, scope: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRequest {
    schema_version: u32,
    kind: String,
    target: CreateTarget,
    #[serde(default, deserialize_with = "present")]
    bot: Option<Identity>,
    requirements: String,
    constraints: Vec<String>,
    non_goals: Vec<String>,
    resource_limits: Vec<String>,
    authorization: Authorization,
    confirmation: Confirmation,
}
impl CreateRequest {
    fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 || self.kind != "request" || self.confirmation.scope != "create_blank" { return Err("invalid-format"); }
        if self.requirements.is_empty() || self.confirmation.evidence.is_empty()
            || self.constraints.iter().chain(&self.non_goals).chain(&self.resource_limits).any(String::is_empty)
            || self.bot.as_ref().is_some_and(|bot| bot.id.as_ref().is_some_and(String::is_empty) || bot.name.as_ref().is_some_and(String::is_empty) || !crate::tasks::valid_group(bot.group.as_deref())) { return Err("invalid-format"); }
        if !self.authorization.create_blank || !self.authorization.write_materials { return Err("authorization-required"); }
        // save_guides is deliberately carried, not interpreted as permission to
        // save any guide or run the target in this capability.
        let _ = self.authorization.save_guides;
        match &self.target {
            CreateTarget::Task { task_id } if task_id.trim().is_empty() => Err("target-conflict"),
            CreateTarget::Map { map_file } if map_file.is_empty() || map_file.contains(['/', '\\', ':', '\0']) || !map_file.ends_with(".json") => Err("target-conflict"),
            _ => Ok(()),
        }
    }
}
async fn target(state: &crate::AppState, request: &CreateTarget) -> Result<(Value, String, String), String> {
    match request {
        CreateTarget::Task { task_id } => {
            let task = state.tasks.get(task_id).await.map_err(|error| error.message())?;
            if task.task_type != crate::tasks::TaskType::Normal { return Err("创建仅接受真实普通 Task。".into()); }
            let name = task.name.clone();
            Ok((serde_json::to_value(task).map_err(|error| error.to_string())?, name, format!("bot-{task_id}")))
        }
        CreateTarget::Map { map_file } => {
            with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
                let map = read_map(access, &state.map_directory, map_file)?;
                let name = map.title.clone();
                Ok((serde_json::to_value(map).map_err(|error| map_error(error.to_string()))?, name, format!("bot-map-{}", map_file.strip_suffix(".json").unwrap())))
            }).map_err(|error| error.message)
        }
    }
}

pub(crate) async fn create_blank(state: &crate::AppState, run: &crate::runs::Run, execution: &crate::runs::NodeExecution) -> Outcome {
    let mut work = Work { write: "not_attempted", read_back: "not_attempted", materials: Vec::new(), outputs: Vec::new() };
    let result = create(state, run, execution, &mut work).await;
    Outcome { failure: result.err(), outputs: work.outputs }
}
async fn create(state: &crate::AppState, run: &crate::runs::Run, execution: &crate::runs::NodeExecution, work: &mut Work) -> Result<(), Value> {
    let inputs = execution.inputs.as_ref().ok_or_else(|| work.failure("validate", "missing-field", "缺少冻结 inputs。"))?;
    if inputs.iter().any(|file| !matches!(file.id.as_str(), "request" | "materials")) {
        return Err(work.failure("validate", "ambiguous-request", "创建仅消费 request 和可选 materials 命名绑定。"));
    }
    let input = inputs.iter().find(|file| file.id == "request").ok_or_else(|| work.failure("validate", "missing-field", "缺少 request 命名输入。"))?;
    let raw = FileStore::default().read(&input.path).map_err(|error| work.failure("parse", "invalid-format", error))?;
    let value = strict_json(&raw).map_err(|error| work.failure("parse", if error.contains("duplicate") { "ambiguous-request" } else { "invalid-format" }, error))?;
    if !value.is_object() { return Err(work.failure("parse", "invalid-format", "request 必须是单个 JSON 对象。")); }
    let request = CreateRequest::deserialize(&value).map_err(|error| work.failure("validate", if error.to_string().contains("missing field") { "missing-field" } else { "invalid-format" }, error))?;
    request.validate().map_err(|code| work.failure("validate", code, "创建 request 格式、目标或明确授权无效。"))?;
    let rules = execution.output_rule.as_ref().ok_or_else(|| work.failure("validate", "missing-field", "缺少冻结输出声明。"))?;
    let mut ids = std::collections::BTreeSet::new();
    if rules.iter().any(|rule| !ids.insert(rule.id.as_str())) || ["baseline", "handoff"].iter().any(|id| !ids.contains(id)) {
        return Err(work.failure("validate", "ambiguous-request", "输出必须明确且唯一声明 baseline 和 handoff。"));
    }
    let (configuration, name, default_id) = target(state, &request.target).await.map_err(|error| work.failure("read_current", "target-conflict", error))?;
    let (task_id, map_file) = match &request.target { CreateTarget::Task { task_id } => (Some(task_id.as_str()), None), CreateTarget::Map { map_file } => (None, Some(map_file.as_str())) };
    if crate::runs::definition_in_use(state, task_id, map_file).await.map_err(|error| work.failure("read_current", "execution-failed", error))? {
        return Err(work.failure("save", "definition-in-use", "目标定义正被活动 Run 使用，禁止创建或修改该绑定。"));
    }
    let identity = request.bot.as_ref();
    let id = identity.and_then(|bot| bot.id.as_deref()).unwrap_or(&default_id).to_owned();
    let bot = Bot {
        id: id.clone(), name: identity.and_then(|bot| bot.name.clone()).unwrap_or(name), group: identity.and_then(|bot| bot.group.clone()),
        when: String::new(), plan_guide: String::new(), node_guide: String::new(), task_id: task_id.map(str::to_owned), map_file: map_file.map(str::to_owned), rust_script: None,
    };
    let mut content = serde_json::to_value(&bot).map_err(|error| work.failure("validate", "invalid-format", error))?;
    content.as_object_mut().unwrap().remove("id");
    work.write = "failed";
    save_bot_definition(state, BotSave { id: id.clone(), content: content.to_string(), revision: None }, true).map_err(|error| {
        let code = match error.code { "bot-conflict" => "identity-conflict", "target-conflict" => "target-conflict", "ambiguous-request" => "ambiguous-request", _ => "write-failed" };
        work.failure("save", code, error.message)
    })?;
    work.write = "succeeded";
    work.read_back = "failed";
    let revision = with_definitions(&[state.bot_directory.clone()], "bot-read-failed", |access| {
        bot_target(access, &state.bot_directory, &id)?.ok_or_else(|| edit_error("bot-read-failed", "创建后 Bot 缺失。"))
    }).map_err(|error| work.failure("read_back", "read-back-failed", error.message))?;
    let read_bot: Value = strict_json(revision.original.as_bytes()).map_err(|error| work.failure("read_back", "read-back-failed", error))?;
    let expected = serde_json::to_value(&bot).map_err(|error| work.failure("read_back", "read-back-failed", error))?;
    if read_bot != expected { return Err(work.failure("read_back", "read-back-failed", "创建后完整 Bot 与预期不一致。")); }
    let (after_target, _, _) = target(state, &request.target).await.map_err(|error| work.failure("read_back", "read-back-failed", error))?;
    if after_target != configuration { return Err(work.failure("read_back", "target-conflict", "创建期间目标配置发生变化；不回滚已创建 Bot。")); }
    work.read_back = "succeeded";
    let locator = serde_json::to_value(&request.target).map_err(|error| work.failure("materials", "material-write-failed", error))?;
    let mut snapshot = locator.clone();
    snapshot["configuration"] = configuration;
    let baseline = json!({"schema_version":1,"kind":"baseline","bot":read_bot,"revision":revision,"target":snapshot,"request":value});
    let sources: Vec<_> = inputs.iter().map(|input| json!({"binding_id":input.id,"direction":"input","role":input.id})).collect();
    let handoff = json!({"schema_version":1,"kind":"handoff","stage":"created","bot_id":id,"target":locator,"sources":sources,"confirmation_evidence":[request.confirmation.evidence],"checks":{"bot_read_back":true,"target_unchanged":true},"unverified":["仅创建定义；未执行目标业务，也未保存三指南。"]});
    for (id, document) in [("baseline", baseline), ("handoff", handoff)] {
        material(run, execution, work, id, &document)?;
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetSnapshot {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "present")]
    task_id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    map_file: Option<String>,
    configuration: Value,
}
impl TargetSnapshot {
    fn locator(&self) -> Result<CreateTarget, &'static str> {
        match (self.kind.as_str(), &self.task_id, &self.map_file) {
            ("task", Some(id), None) if !id.trim().is_empty() => Ok(CreateTarget::Task { task_id: id.clone() }),
            ("map", None, Some(file)) if !file.is_empty() && file.ends_with(".json") && !file.contains(['/', '\\', ':', '\0']) => Ok(CreateTarget::Map { map_file: file.clone() }),
            _ => Err("target-conflict"),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    schema_version: u32, kind: String, bot: Bot, revision: DefinitionRevision,
    target: TargetSnapshot, request: CreateRequest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source { binding_id: String, direction: String, role: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextConfirmation {
    evidence: String, text_sha256: String, full_text: bool,
    save_authorized: bool, semantics_preserved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GuideHandoff {
    schema_version: u32, kind: String, stage: String, bot_id: String,
    target: CreateTarget, field: String, sources: Vec<Source>,
    confirmation: TextConfirmation, constraints: Vec<String>, unverified: Vec<String>,
}
fn document<T: for<'de> Deserialize<'de>>(inputs: &[ResolvedFile], id: &str, work: &Work) -> Result<T, Value> {
    let input = inputs.iter().find(|file| file.id == id).ok_or_else(|| work.failure("validate", "missing-field", format!("缺少 {id} 命名输入。")))?;
    let bytes = FileStore::default().read(&input.path).map_err(|error| work.failure("parse", "invalid-format", error))?;
    let value = strict_json(&bytes).map_err(|error| work.failure("parse", if error.contains("duplicate") { "ambiguous-request" } else { "invalid-format" }, error))?;
    if !value.is_object() { return Err(work.failure("parse", "invalid-format", "材料必须是单个 JSON 对象。")); }
    T::deserialize(value).map_err(|error| work.failure("validate", if error.to_string().contains("missing field") { "missing-field" } else { "invalid-format" }, error))
}

pub(crate) async fn save_guides(state: &crate::AppState, run: &crate::runs::Run, execution: &crate::runs::NodeExecution) -> Outcome {
    let mut work = Work { write: "not_attempted", read_back: "not_attempted", materials: Vec::new(), outputs: Vec::new() };
    let result = merge_save(state, run, execution, &mut work).await;
    Outcome { failure: result.err(), outputs: work.outputs }
}
async fn merge_save(state: &crate::AppState, run: &crate::runs::Run, execution: &crate::runs::NodeExecution, work: &mut Work) -> Result<(), Value> {
    use sha2::{Digest, Sha256};
    const IDS: [&str; 7] = ["baseline", "when_text", "when_handoff", "plan_guide_text", "plan_guide_handoff", "node_guide_text", "node_guide_handoff"];
    let inputs = execution.inputs.as_deref().ok_or_else(|| work.failure("validate", "missing-field", "缺少冻结 inputs。"))?;
    let mut ids = std::collections::BTreeSet::new();
    if inputs.iter().any(|file| !IDS.contains(&file.id.as_str()) || !ids.insert(file.id.as_str())) {
        return Err(work.failure("validate", "ambiguous-request", "保存只消费唯一 baseline 和三组 text/handoff。"));
    }
    if IDS.iter().any(|id| !ids.contains(id)) { return Err(work.failure("validate", "missing-field", "缺少 baseline 或三组 text/handoff。")); }
    let rules = execution.output_rule.as_deref().ok_or_else(|| work.failure("validate", "missing-field", "缺少冻结输出声明。"))?;
    let mut outputs = std::collections::BTreeSet::new();
    if rules.iter().any(|rule| !outputs.insert(rule.id.as_str())) || ["saved", "handoff"].iter().any(|id| !outputs.contains(id)) {
        return Err(work.failure("validate", "ambiguous-request", "输出必须唯一声明 saved 和 handoff。"));
    }
    let baseline: Baseline = document(inputs, "baseline", work)?;
    baseline.request.validate().map_err(|code| work.failure("validate", code, "baseline.request 无效。"))?;
    if baseline.schema_version != 1 || baseline.kind != "baseline" || baseline.revision.file.is_empty() || baseline.revision.original.is_empty()
        || !baseline.bot.when.is_empty() || !baseline.bot.plan_guide.is_empty() || !baseline.bot.node_guide.is_empty() {
        return Err(work.failure("validate", "invalid-format", "创建 baseline 必须是三空指南的完整 v1 对象和真实 revision。"));
    }
    let locator = baseline.target.locator().map_err(|code| work.failure("validate", code, "baseline 目标不唯一。"))?;
    let locator_value = serde_json::to_value(&locator).map_err(|error| work.failure("validate", "invalid-format", error))?;
    let bot_value = serde_json::to_value(&baseline.bot).map_err(|error| work.failure("validate", "invalid-format", error))?;
    let original = strict_json(baseline.revision.original.as_bytes()).map_err(|error| work.failure("validate", "invalid-format", error))?;
    if original.get("id") != Some(&json!(baseline.bot.id)) || baseline.request.bot.as_ref().and_then(|bot| bot.id.as_ref()).is_some_and(|id| id != &baseline.bot.id) {
        return Err(work.failure("validate", "identity-conflict", "baseline Bot、request 与 revision 身份不一致。"));
    }
    if original != bot_value { return Err(work.failure("validate", "revision-conflict", "baseline 完整 Bot 与 revision.original 不一致。")); }
    if baseline.bot.rust_script.is_some() || baseline.bot.task_id != baseline.target.task_id || baseline.bot.map_file != baseline.target.map_file
        || serde_json::to_value(&baseline.request.target).unwrap() != locator_value {
        return Err(work.failure("validate", "target-conflict", "baseline/request/唯一 Bot 绑定不一致。"));
    }
    let mut candidate = bot_value;
    let mut evidence = Vec::with_capacity(3);
    let mut unverified = Vec::new();
    for (field, text_id, handoff_id) in [("when", "when_text", "when_handoff"), ("plan_guide", "plan_guide_text", "plan_guide_handoff"), ("node_guide", "node_guide_text", "node_guide_handoff")] {
        let input = inputs.iter().find(|file| file.id == text_id).unwrap();
        let bytes = FileStore::default().read(&input.path).map_err(|error| work.failure("parse", "invalid-format", error))?;
        let text = String::from_utf8(bytes).map_err(|error| work.failure("parse", "invalid-format", error))?;
        let handoff: GuideHandoff = document(inputs, handoff_id, work)?;
        if handoff.schema_version != 1 || handoff.kind != "handoff" || handoff.stage != "guide" { return Err(work.failure("validate", "invalid-format", "指南 handoff 格式无效。")); }
        if handoff.bot_id != baseline.bot.id { return Err(work.failure("validate", "identity-conflict", "指南不属于共同 Bot。")); }
        if handoff.field != field || serde_json::to_value(&handoff.target).unwrap() != locator_value { return Err(work.failure("validate", "target-conflict", "指南字段或目标不对应；不交换材料。")); }
        let mut sources = std::collections::BTreeSet::new();
        if handoff.sources.is_empty() || handoff.sources.iter().any(|source| source.binding_id.is_empty() || !matches!(source.direction.as_str(), "input" | "output") || !matches!(source.role.as_str(), "request" | "materials" | "baseline" | "text" | "handoff") || !sources.insert((source.direction.as_str(), source.binding_id.as_str())))
            || !handoff.sources.iter().any(|source| source.direction == "input" && source.role == "baseline") || !handoff.sources.iter().any(|source| source.direction == "output" && source.role == "text")
            || handoff.constraints.iter().chain(&handoff.unverified).any(String::is_empty) {
            return Err(work.failure("validate", "invalid-format", "指南来源或限制格式无效；不沿来源索引寻址。"));
        }
        let confirmation = handoff.confirmation;
        let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
        if confirmation.evidence.is_empty() || !confirmation.full_text || !confirmation.semantics_preserved || confirmation.text_sha256 != digest
            || (!baseline.request.authorization.save_guides && !confirmation.save_authorized) {
            return Err(work.failure("validate", "authorization-required", "确认必须覆盖本字段最终原样 bytes、保义及保存授权。"));
        }
        candidate[field] = json!(text);
        evidence.push(confirmation.evidence);
        unverified.extend(handoff.unverified);
    }
    let current = with_definitions(&[state.bot_directory.clone()], "bot-read-failed", |access| {
        bot_target(access, &state.bot_directory, &baseline.bot.id)?.ok_or_else(|| edit_error("identity-conflict", "Bot 缺失。"))
    }).map_err(|error| work.failure("read_current", "identity-conflict", error.message))?;
    if current != baseline.revision { return Err(work.failure("read_current", "revision-conflict", "真实 revision 已变化；不按相同正文认定成功。")); }
    let (configuration, _, _) = target(state, &locator).await.map_err(|error| work.failure("read_current", "target-conflict", error))?;
    if configuration != baseline.target.configuration { return Err(work.failure("read_current", "target-conflict", "完整目标配置与 baseline 不一致。")); }
    if crate::runs::definition_in_use(state, baseline.bot.task_id.as_deref(), baseline.bot.map_file.as_deref()).await.map_err(|error| work.failure("save", "execution-failed", error))? {
        return Err(work.failure("save", "definition-in-use", "目标定义正被活动 Run 使用，禁止保存。"));
    }
    // The public save takes content without id. Temporarily remove only that
    // scalar rather than duplicating all three potentially large guide bodies.
    let identity = candidate.as_object_mut().unwrap().remove("id").unwrap();
    let content = candidate.to_string();
    candidate["id"] = identity;
    work.write = "failed";
    save_bot_definition(state, BotSave { id: baseline.bot.id.clone(), content, revision: Some(baseline.revision) }, true).map_err(|error| {
        work.failure("save", match error.code { "bot-conflict" => "revision-conflict", "target-conflict" => "target-conflict", "ambiguous-request" => "ambiguous-request", _ => "write-failed" }, error.message)
    })?;
    work.write = "succeeded";
    work.read_back = "failed";
    let revision = with_definitions(&[state.bot_directory.clone()], "bot-read-failed", |access| {
        bot_target(access, &state.bot_directory, &baseline.bot.id)?.ok_or_else(|| edit_error("bot-read-failed", "保存后 Bot 缺失。"))
    }).map_err(|error| work.failure("read_back", "read-back-failed", error.message))?;
    let bot = strict_json(revision.original.as_bytes()).map_err(|error| work.failure("read_back", "read-back-failed", error))?;
    if bot != candidate { return Err(work.failure("read_back", "read-back-failed", "写后完整 Bot 与三字段候选不一致。")); }
    let (after_target, _, _) = target(state, &locator).await.map_err(|error| work.failure("read_back", "read-back-failed", error))?;
    if after_target != configuration { return Err(work.failure("read_back", "target-conflict", "保存期间目标变化；保留实际效果，不回滚。")); }
    work.read_back = "succeeded";
    let mut snapshot = locator_value.clone();
    snapshot["configuration"] = after_target;
    let saved = json!({"schema_version":1,"kind":"saved","bot":bot,"revision":revision,"target":snapshot});
    let sources: Vec<_> = inputs.iter().map(|input| json!({"binding_id":input.id,"direction":"input","role":if input.id == "baseline" { "baseline" } else if input.id.ends_with("_text") { "text" } else { "handoff" }})).collect();
    let handoff = json!({"schema_version":1,"kind":"handoff","stage":"saved","bot_id":baseline.bot.id,"target":locator_value,"sources":sources,"confirmation_evidence":evidence,"checks":{"bot_read_back":true,"target_unchanged":true},"unverified":unverified});
    for (id, document) in [("saved", saved), ("handoff", handoff)] { material(run, execution, work, id, &document)?; }
    Ok(())
}

fn material(run: &crate::runs::Run, execution: &crate::runs::NodeExecution, work: &mut Work, id: &str, document: &Value) -> Result<(), Value> {
    let directory = Path::new(&run.zone).join(".manager-zone").join(".workflow").join(&run.id).join(&execution.id);
    let path = directory.join(format!("{id}.json"));
    let address = crate::zones::display_path(&path);
    work.materials.push(json!({"id":id,"path":address,"status":"failed"}));
    let bytes = serde_json::to_vec_pretty(document).map_err(|error| work.failure("materials", "material-write-failed", error))?;
    with_definitions(&[PathBuf::from(&run.zone)], "material-write-failed", |access| {
        crate::zones::browse::checked_root(&run.zone).map_err(|error| edit_error("material-write-failed", error.message()))?;
        prepare_directory(access, &directory)?;
        access.publish_checked(&path, &bytes, PublishMode::Create,
            |access| prepare_directory(access, &directory).map(|_| ()),
            |error| edit_error("material-write-failed", error.to_string()))
    }).map_err(|error| work.failure("materials", "material-write-failed", error.message))?;
    let read = FileStore::default().read(&path).map_err(|error| work.failure("materials", "material-read-back-failed", error))?;
    if read != bytes { return Err(work.failure("materials", "material-read-back-failed", "材料完整读回不一致。")); }
    work.materials.last_mut().unwrap()["status"] = json!("succeeded");
    work.outputs.push(ResolvedFile { id: id.into(), path: address });
    Ok(())
}

// serde_json::Value normally silently overwrites duplicate keys. All structured
// business materials use this recursive parser before typed field validation.
pub(crate) fn strict_json(bytes: &[u8]) -> Result<Value, String> {
    struct Strict(Value);
    impl<'de> Deserialize<'de> for Strict {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Strict;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result { formatter.write_str("JSON without duplicate keys") }
                fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Strict, M::Error> {
                    let mut object = serde_json::Map::new();
                    while let Some((key, Strict(value))) = map.next_entry::<String, Strict>()? {
                        if object.insert(key.clone(), value).is_some() { return Err(serde::de::Error::custom(format!("duplicate key: {key}"))); }
                    }
                    Ok(Strict(Value::Object(object)))
                }
                fn visit_seq<S: serde::de::SeqAccess<'de>>(self, mut seq: S) -> Result<Strict, S::Error> {
                    let mut values = Vec::new();
                    while let Some(Strict(value)) = seq.next_element()? { values.push(value); }
                    Ok(Strict(Value::Array(values)))
                }
                fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Strict, E> { Ok(Strict(Value::Bool(value))) }
                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Strict, E> { Ok(Strict(Value::String(value.into()))) }
                fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Strict, E> { Ok(Strict(value.into())) }
                fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Strict, E> { Ok(Strict(value.into())) }
                fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Strict, E> { serde_json::Number::from_f64(value).map(|number| Strict(Value::Number(number))).ok_or_else(|| E::custom("invalid JSON number")) }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Strict, E> { Ok(Strict(Value::Null)) }
            }
            deserializer.deserialize_any(Visitor)
        }
    }
    serde_json::from_slice::<Strict>(bytes).map(|Strict(value)| value).map_err(|error| error.to_string())
}
