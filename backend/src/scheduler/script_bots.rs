//! Fixed creation business. Named files are the only parameters; definition
//! publication uses the same revision/path checks as the public Bot save.
use super::*;
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
