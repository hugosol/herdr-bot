//! Host-owned definition and material operations for trusted scripts.
//! Business rules live in meta-data/scripts; file coordination stays in Rust.
use super::{Bot, BotSave, bot_target, save_bot_definition};
use crate::files::{FileStore, PublishMode};
use crate::scheduler::{edit_error, prepare_directory, with_definitions};
use crate::scheduler::maps::read_map;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn failure(code: &str, message: impl ToString) -> Value {
    json!({"code": code, "message": message.to_string()})
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Value> {
    serde_json::from_value(value).map_err(|error| failure("invalid-format", error))
}

pub(crate) async fn call(state: &crate::AppState, run: &crate::runs::Run, execution: &crate::runs::NodeExecution, operation: &str, args: Value) -> Result<Value, Value> {
    let state = state.clone();
    let zone = run.zone.clone();
    let directory = Path::new(&zone).join(".manager-zone").join(".workflow").join(&run.id).join(&execution.id);
    let operation = operation.to_owned();
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || runtime.block_on(invoke(&state, &zone, &directory, &operation, args)))
        .await.map_err(|error| failure("execution-failed", error))?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRequest { path: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextRequest { text: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity { id: String }
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum Target { Task { task_id: String }, Map { map_file: String } }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    id: String,
    content: Value,
    revision: Option<crate::scheduler::DefinitionRevision>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Material { id: String, document: Value }

async fn invoke(state: &crate::AppState, zone: &str, directory: &Path, operation: &str, args: Value) -> Result<Value, Value> {
    match operation {
        "read_json" | "read_text" => {
            let request: FileRequest = decode(args)?;
            let bytes = FileStore::default().read(&request.path).map_err(|error| failure("invalid-format", error))?;
            if operation == "read_json" { parse(&bytes) }
            else { String::from_utf8(bytes).map(Value::String).map_err(|error| failure("invalid-format", error)) }
        }
        "parse_json" => {
            let request: TextRequest = decode(args)?;
            parse(request.text.as_bytes())
        }
        "target" => match decode::<Target>(args)? {
            Target::Task { task_id } => {
                let task = state.tasks.get(&task_id).await.map_err(|error| failure("target-conflict", error.message()))?;
                if task.task_type != crate::tasks::TaskType::Normal { return Err(failure("target-conflict", "目标必须是普通 Task。")); }
                Ok(json!({"name":task.name,"default_id":format!("bot-{task_id}"),"configuration":task}))
            }
            Target::Map { map_file } => {
                if map_file.is_empty() || !map_file.ends_with(".json") || map_file.contains(['/', '\\', ':', '\0']) {
                    return Err(failure("target-conflict", "目标必须是 Map 文件名。"));
                }
                with_definitions(&[state.map_directory.clone()], "map-read-failed", |access| {
                    let map = read_map(access, &state.map_directory, &map_file)?;
                    Ok(json!({"name":map.title,"default_id":format!("bot-map-{}",map_file.strip_suffix(".json").unwrap()),"configuration":map}))
                }).map_err(|error| failure(error.code, error.message))
            }
        },
        "bot_read" => {
            let request: Identity = decode(args)?;
            let revision = with_definitions(&[state.bot_directory.clone()], "bot-read-failed", |access| {
                bot_target(access, &state.bot_directory, &request.id)?.ok_or_else(|| edit_error("identity-conflict", "Bot 缺失。"))
            }).map_err(|error| failure(error.code, error.message))?;
            let bot = parse(revision.original.as_bytes())?;
            let checked: Bot = decode(bot.clone())?;
            checked.validate_target().map_err(|error| failure(error.code, error.message))?;
            Ok(json!({"bot":bot,"revision":revision}))
        }
        "bot_save" => {
            let request: Save = decode(args)?;
            let mut content = request.content;
            let object = content.as_object_mut().ok_or_else(|| failure("invalid-format", "Bot content 必须是对象。"))?;
            if object.contains_key("id") { return Err(failure("invalid-format", "content 不包含顶层 id。")); }
            object.insert("id".into(), Value::String(request.id.clone()));
            let bot: Bot = decode(content.clone())?;
            bot.validate_target().map_err(|error| failure(error.code, error.message))?;
            if crate::runs::definition_in_use(state, bot.task_id.as_deref(), bot.map_file.as_deref()).await.map_err(|error| failure("execution-failed", error))? {
                return Err(failure("definition-in-use", "目标定义正被活动 Run 使用，禁止创建或修改该绑定。"));
            }
            content.as_object_mut().unwrap().remove("id");
            let saved = save_bot_definition(state, BotSave { id: request.id, content: content.to_string(), revision: request.revision }, true)
                .map_err(|error| failure(error.code, error.message))?;
            serde_json::to_value(saved).map_err(|error| failure("write-failed", error))
        }
        "material" => {
            let request: Material = decode(args)?;
            if request.id.is_empty() || request.id == "." || request.id == ".." || request.id.contains(['/', '\\', ':', '\0']) {
                return Err(failure("material-write-failed", "材料 ID 必须是单个文件名。"));
            }
            let path = directory.join(format!("{}.json", request.id));
            let result = (|| -> Result<Value, Value> {
            let bytes = serde_json::to_vec_pretty(&request.document).map_err(|error| failure("material-write-failed", error))?;
            with_definitions(&[PathBuf::from(zone)], "material-write-failed", |access| {
                crate::zones::browse::checked_root(zone).map_err(|error| edit_error("material-write-failed", error.message()))?;
                prepare_directory(access, directory)?;
                access.publish_checked(&path, &bytes, PublishMode::Create,
                    |access| prepare_directory(access, directory).map(|_| ()),
                    |error| edit_error("material-write-failed", error.to_string()))
            }).map_err(|error| failure(error.code, error.message))?;
            let read = FileStore::default().read(&path).map_err(|error| failure("material-read-back-failed", error))?;
            if read != bytes { return Err(failure("material-read-back-failed", "材料完整读回不一致。")); }
            Ok(json!({"id":request.id,"path":crate::zones::display_path(&path)}))
            })();
            result.map_err(|mut error| {
                error["path"] = json!(crate::zones::display_path(&path));
                error
            })
        }
        _ => Err(failure("unknown-operation", format!("未知宿主操作：{operation}"))),
    }
}

fn parse(bytes: &[u8]) -> Result<Value, Value> {
    strict_json(bytes).map_err(|error| failure(if error.contains("duplicate") { "ambiguous-request" } else { "invalid-format" }, error))
}

// JSON.parse silently discards duplicate keys. File protocols reject ambiguity.
fn strict_json(bytes: &[u8]) -> Result<Value, String> {
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
