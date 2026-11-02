//! Explicit offline file-handoff cutover. Never invoked by normal readers or startup.
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};
use anyhow::{Context, bail, ensure};
use serde::{Deserialize, de::{self, MapAccess, SeqAccess, Visitor}};
use serde_json::{Value, Map, json};
use crate::files::{FileAccess, FileStore, PublishMode};

type Outputs = BTreeMap<(String, String), BTreeSet<String>>;
struct Change { path: PathBuf, original: Vec<u8>, replacement: Vec<u8> }
struct RunData { run: Value, history: Vec<Value>, reports: Vec<Value>, paths: [PathBuf; 3] }

/// Run with every backend process stopped. Returns the number of replaced files.
/// Every source is validated before backups/publication; originals are retained in
/// sibling `.file-handoff-v1.bak` files. Publication is atomic per file, not per tree.
pub fn migrate(map_directory: &Path, run_directory: &Path) -> anyhow::Result<usize> {
    let maps = std::path::absolute(map_directory)?;
    let runs = std::path::absolute(run_directory)?;
    ensure!(!maps.starts_with(&runs) && !runs.starts_with(&maps), "Map and Run directories must not overlap");
    FileStore.with(&[maps.clone(), runs.clone()], |files| migrate_locked(files, &maps, &runs))?
}

fn regular(files: &FileAccess, path: &Path, directory: bool) -> anyhow::Result<()> {
    let metadata = files.symlink_metadata(path)?;
    ensure!(!crate::zones::browse::is_link_like(&metadata), "links/reparse points are forbidden: {}", path.display());
    ensure!(if directory { metadata.is_dir() } else { metadata.is_file() }, "unexpected file type: {}", path.display());
    Ok(())
}
fn inventory(files: &FileAccess, root: &Path) -> anyhow::Result<()> {
    regular(files, root, true)?;
    for entry in files.read_dir(root)? {
        let path = entry.path();
        let metadata = files.symlink_metadata(&path)?;
        ensure!(!crate::zones::browse::is_link_like(&metadata), "links/reparse points are forbidden: {}", path.display());
        if metadata.is_dir() { inventory(files, &path)?; }
        else { regular(files, &path, false)?; }
    }
    Ok(())
}
fn load(files: &FileAccess, path: &Path) -> anyhow::Result<Value> {
    regular(files, path, false)?;
    parse(&files.read(path)?).with_context(|| format!("invalid JSON: {}", path.display()))
}
fn lines(files: &FileAccess, path: &Path) -> anyhow::Result<Vec<Value>> {
    regular(files, path, false)?;
    let bytes = files.read(path)?;
    ensure!(bytes.is_empty() || bytes.last() == Some(&b'\n'), "incomplete JSONL: {}", path.display());
    bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()).map(|line| {
        let value = parse(line)?;
        ensure!(value.is_object(), "JSONL record must be an object: {}", path.display());
        Ok(value)
    }).collect()
}
fn encoded_lines(files: &FileAccess, path: &Path, values: &[Value]) -> anyhow::Result<Vec<u8>> {
    let original = files.read(path)?;
    let mut bytes = Vec::new();
    let mut records = values.iter();
    for line in original.split_inclusive(|b| *b == b'\n') {
        let content = &line[..line.len() - 1];
        if content.is_empty() { bytes.extend_from_slice(line); continue; }
        let value = records.next().context("JSONL record count changed")?;
        if parse(content)? == *value { bytes.extend_from_slice(line); }
        else { serde_json::to_writer(&mut bytes, value)?; bytes.push(b'\n'); }
    }
    ensure!(records.next().is_none(), "JSONL record count changed");
    Ok(bytes)
}
fn record_change(files: &FileAccess, changes: &mut Vec<Change>, path: &Path, bytes: Vec<u8>) -> anyhow::Result<()> {
    let original = files.read(path)?;
    // Compare JSON semantically so an already-cut-over file is not rewritten.
    let equal = if path.extension().is_some_and(|e| e == "jsonl") {
        lines(files, path)? == bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(parse).collect::<anyhow::Result<Vec<_>>>()?
    } else { parse(&original)? == parse(&bytes)? };
    if !equal { changes.push(Change { path: path.to_owned(), original, replacement: bytes }); }
    Ok(())
}
fn backup(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned(); name.push(".file-handoff-v1.bak"); PathBuf::from(name)
}
fn migrate_locked(files: &FileAccess, maps: &Path, runs: &Path) -> anyhow::Result<usize> {
    inventory(files, maps)?; inventory(files, runs)?;
    let mut definitions = BTreeMap::new();
    for entry in files.read_dir(maps)? {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            let name = entry.file_name().into_string().map_err(|_| anyhow::anyhow!("non-UTF8 Map filename"))?;
            definitions.insert(name, (path.clone(), load(files, &path)?));
        }
    }
    let mut data = Vec::new();
    for entry in files.read_dir(runs)? {
        if !entry.file_type()?.is_dir() { continue; }
        let root = entry.path();
        let paths = [root.join("run.json"), root.join("history.jsonl"), root.join("messages.jsonl")];
        let run = load(files, &paths[0])?;
        ensure!(text(&run, "id")? == entry.file_name().to_str().context("non-UTF8 Run directory")?, "Run ID does not match directory");
        data.push(RunData { run, history: lines(files, &paths[1])?, reports: lines(files, &paths[2])?, paths });
    }
    let mut map_outputs: Outputs = BTreeMap::new();
    let mut changes = Vec::new();
    for data in &mut data {
        let run_id = text(&data.run, "id")?.to_owned();
        let map = text(&data.run, "map")?.to_owned();
        let mut identities = BTreeMap::new();
        collect_run_executions(&data.run, &mut identities)?;
        for event in &data.history { collect_history_executions(event, &mut identities)?; }
        let mut report_ids = BTreeSet::new();
        for report in &mut data.reports {
            transform_report(report)?;
            crate::runs::validate_report_source(&serde_json::to_string(report)?).map_err(anyhow::Error::msg)?;
            ensure!(text(report, "runId")? == run_id, "report belongs to another Run");
            ensure!(report_ids.insert(text(report, "id")?.to_owned()), "duplicate report ID");
            let node = text(report, "nodeId")?.to_owned();
            let execution = text(report, "executionId")?.to_owned();
            ensure!(identities.get(&execution) == Some(&node), "report execution is unknown or belongs to another node: {execution}");
            let ids = file_ids(&report["outputs"], true)?;
            map_outputs.entry((map.clone(), node)).or_default().extend(ids);
        }
        transform_run(&mut data.run)?;
        serde_json::from_value::<crate::runs::Run>(data.run.clone()).context("invalid transformed Run")?;
        for event in &mut data.history { transform_history(event)?; }
        record_change(files, &mut changes, &data.paths[0], serde_json::to_vec_pretty(&data.run)?)?;
        record_change(files, &mut changes, &data.paths[1], encoded_lines(files, &data.paths[1], &data.history)?)?;
        record_change(files, &mut changes, &data.paths[2], encoded_lines(files, &data.paths[2], &data.reports)?)?;
    }
    for (name, (path, map)) in &mut definitions {
        transform_map(map, name, &map_outputs)?;
        let bytes = serde_json::to_vec_pretty(map)?;
        crate::scheduler::validate_map_source(std::str::from_utf8(&bytes)?).map_err(anyhow::Error::msg)?;
        record_change(files, &mut changes, path, bytes)?;
    }
    // No disk mutation before all transformations, schemas, and backup conflicts pass.
    for change in &changes {
        ensure!(!files.try_exists(backup(&change.path))?, "backup already exists; inspect/restore the previous migration before retrying: {}", backup(&change.path).display());
        ensure!(files.read(&change.path)? == change.original, "source changed during migration: {}", change.path.display());
    }
    for change in &changes {
        files.publish_checked(&backup(&change.path), &change.original, PublishMode::Create,
            |_| Ok::<_, anyhow::Error>(()), anyhow::Error::from)?;
    }
    for change in &changes {
        files.publish_checked(&change.path, &change.replacement, PublishMode::Replace, |access| {
            ensure!(access.read(&change.path)? == change.original, "source changed: {}", change.path.display());
            Ok::<_, anyhow::Error>(())
        }, anyhow::Error::from)?;
    }
    Ok(changes.len())
}
fn object(value: &mut Value) -> anyhow::Result<&mut Map<String, Value>> { value.as_object_mut().context("expected JSON object") }
fn text<'a>(value: &'a Value, field: &str) -> anyhow::Result<&'a str> {
    value.get(field).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).with_context(|| format!("missing/invalid {field}"))
}
fn rename(value: &mut Value, old: &str, new: &str) -> anyhow::Result<()> {
    let object = object(value)?;
    if let Some(old_value) = object.remove(old) {
        ensure!(!object.contains_key(new), "ambiguous mixed fields {old}/{new}");
        object.insert(new.into(), old_value);
    }
    Ok(())
}
fn file_ids(value: &Value, paths: bool) -> anyhow::Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for item in value.as_array().context("expected file array")? {
        let id = text(item, "id")?;
        ensure!(ids.insert(id.to_owned()), "duplicate file ID: {id}");
        if paths { text(item, "path")?; }
    }
    Ok(ids)
}
fn transform_report(report: &mut Value) -> anyhow::Result<()> {
    rename(report, "source", "from")?; rename(report, "target", "to")?;
    let fields = object(report)?;
    match fields.remove("outputs") {
        None => { fields.insert("outputs".into(), json!([])); }
        Some(Value::Object(outputs)) => {
            let mut files = Vec::new();
            for (id, path) in outputs {
                ensure!(!id.trim().is_empty() && path.as_str().is_some_and(|p| !p.trim().is_empty()), "invalid legacy report output");
                files.push(json!({"id":id,"path":path}));
            }
            fields.insert("outputs".into(), Value::Array(files));
        }
        Some(value @ Value::Array(_)) => { fields.insert("outputs".into(), value); }
        Some(_) => bail!("invalid report outputs"),
    }
    file_ids(&report["outputs"], true)?;
    Ok(())
}
fn transform_inputs(value: Value) -> anyhow::Result<Value> {
    let mut result = Vec::new();
    for (id, mut rule) in value.as_object().context("legacy inputs must be an object")?.clone() {
        rename(&mut rule, "source", "type")?;
        rename(&mut rule, "output", "output_id")?;
        ensure!(!object(&mut rule)?.contains_key("id"), "legacy input already contains id");
        object(&mut rule)?.insert("id".into(), Value::String(id));
        serde_json::from_value::<crate::scheduler::InputRule>(rule.clone()).context("invalid legacy input rule")?;
        result.push(rule);
    }
    Ok(Value::Array(result))
}
fn transform_map(map: &mut Value, name: &str, historical: &Outputs) -> anyhow::Result<()> {
    let nodes = map.get_mut("nodes").and_then(Value::as_object_mut).context("Map nodes must be an object")?;
    for node in nodes.values_mut() {
        let fields = object(node)?;
        if let Some(inputs) = fields.remove("inputs") {
            ensure!(!fields.contains_key("input_rule"), "mixed Map inputs/input_rule");
            fields.insert("input_rule".into(), transform_inputs(inputs)?);
        }
        fields.entry("input_rule").or_insert_with(|| json!([]));
    }
    let mut derived: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in nodes.values() {
        for rule in node["input_rule"].as_array().context("input_rule must be an array")? {
            if rule["type"] == "node" {
                derived.entry(text(rule, "node_id")?.into()).or_default().insert(text(rule, "output_id")?.into());
            }
        }
    }
    // Historical reports inform only legacy nodes without declarations. A current
    // Map may legitimately rename outputs while older frozen executions retain
    // their original IDs; those facts must not rewrite or invalidate this Map.
    for ((map_name, node), ids) in historical {
        if map_name == name && nodes.get(node).is_some_and(|node| node.get("output_rule").is_none()) {
            derived.entry(node.clone()).or_default().extend(ids.clone());
        }
    }
    for (id, node) in nodes {
        let expected = derived.remove(id).unwrap_or_default();
        let fields = object(node)?;
        if let Some(existing) = fields.get("output_rule") {
            let declared = file_ids(existing, false)?;
            ensure!(expected.is_subset(&declared), "existing output_rule conflicts with current cross-node outputs for {id}");
        } else { fields.insert("output_rule".into(), Value::Array(expected.into_iter().map(|id| json!({"id":id})).collect())); }
    }
    ensure!(derived.is_empty(), "input references unknown Map node");
    Ok(())
}
fn register_execution(value: &Value, node: &str, identities: &mut BTreeMap<String, String>) -> anyhow::Result<()> {
    if value.is_null() { return Ok(()); }
    let id = text(value, "id")?;
    if let Some(previous) = identities.insert(id.into(), node.into()) { ensure!(previous == node, "execution ID associated with multiple nodes: {id}"); }
    Ok(())
}
fn collect_run_executions(run: &Value, identities: &mut BTreeMap<String, String>) -> anyhow::Result<()> {
    for (node, state) in run["nodes"].as_object().context("Run nodes must be an object")? {
        register_execution(&state["execution"], node, identities)?;
        for fact in state["previousExecutions"].as_array().context("previousExecutions must be an array")? { register_execution(&fact["execution"], node, identities)?; }
    }
    Ok(())
}
fn collect_history_executions(event: &Value, identities: &mut BTreeMap<String, String>) -> anyhow::Result<()> {
    for key in ["execution", "currentExecution"] {
        if let Some(execution) = event.get(key).filter(|v| !v.is_null()) { register_execution(execution, text(event, "nodeId")?, identities)?; }
    }
    if let Some(snapshot) = event.get("snapshot") { collect_run_executions(snapshot, identities)?; }
    Ok(())
}
fn transform_execution(execution: &mut Value) -> anyhow::Result<()> {
    if execution.is_null() { return Ok(()); }
    text(execution, "id")?;
    let fields = object(execution)?;
    // Neither a mutable Map nor partial reports prove a frozen declaration or
    // actual resolved inputs. Preserve known arrays; explicitly mark unknowns.
    fields.entry("inputRule").or_insert(Value::Null);
    fields.entry("outputRule").or_insert(Value::Null);
    fields.entry("inputs").or_insert(Value::Null);
    for (field, paths) in [("outputRule", false), ("inputs", true)] {
        if !execution[field].is_null() { file_ids(&execution[field], paths)?; }
    }
    if let Some(rules) = execution["inputRule"].as_array() {
        let mut ids = BTreeSet::new();
        for rule in rules { ensure!(ids.insert(text(rule, "id")?), "duplicate frozen input rule ID"); }
    }
    // History events predate unrelated execution metadata such as agent. Validate
    // only handoff fields here; current Run records are validated separately.
    if !execution["inputRule"].is_null() {
        serde_json::from_value::<Vec<crate::scheduler::InputRule>>(execution["inputRule"].clone()).context("invalid frozen inputRule")?;
    }
    if !execution["outputRule"].is_null() {
        serde_json::from_value::<Vec<crate::scheduler::OutputRule>>(execution["outputRule"].clone()).context("invalid frozen outputRule")?;
    }
    if !execution["inputs"].is_null() {
        serde_json::from_value::<Vec<crate::scheduler::ResolvedFile>>(execution["inputs"].clone()).context("invalid resolved inputs")?;
    }
    Ok(())
}
fn transform_run(run: &mut Value) -> anyhow::Result<()> {
    for state in run["nodes"].as_object_mut().context("Run nodes must be an object")?.values_mut() {
        transform_execution(&mut state["execution"])?;
        for fact in state["previousExecutions"].as_array_mut().context("previousExecutions must be an array")? { transform_execution(&mut fact["execution"])?; }
    }
    Ok(())
}
fn transform_history(event: &mut Value) -> anyhow::Result<()> {
    for key in ["execution", "currentExecution"] {
        if event.get(key).is_some_and(|v| !v.is_null()) {
            text(event, "nodeId")?;
            transform_execution(&mut event[key])?;
        }
    }
    if let Some(snapshot) = event.get_mut("snapshot") {
        transform_run(snapshot)?;
        serde_json::from_value::<crate::runs::Run>(snapshot.clone()).context("invalid history Run snapshot")?;
    }
    Ok(())
}

// serde_json::Value normally discards duplicate object keys. Offline repair must
// reject those rather than silently choosing one historical fact.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("JSON without duplicate object keys") }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut map = Map::new();
                while let Some((key, Unique(value))) = a.next_entry::<String, Unique>()? {
                    if map.insert(key.clone(), value).is_some() { return Err(de::Error::custom(format!("duplicate JSON key: {key}"))); }
                }
                Ok(Unique(Value::Object(map)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new(); while let Some(Unique(value)) = a.next_element()? { values.push(value); }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> { Ok(Unique(v.into())) }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> { Ok(Unique(v.into())) }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> { Ok(Unique(v.into())) }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> { serde_json::Number::from_f64(v).map(|v| Unique(Value::Number(v))).ok_or_else(|| E::custom("invalid number")) }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> { Ok(Unique(v.into())) }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> { Ok(Unique(v.into())) }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> { Ok(Unique(Value::Null)) }
        }
        d.deserialize_any(V)
    }
}
fn parse(bytes: &[u8]) -> anyhow::Result<Value> { Ok(serde_json::from_slice::<Unique>(bytes)?.0) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture { root: PathBuf, maps: PathBuf, runs: PathBuf }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("herdr-handoff-migration-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            let maps = root.join("maps"); let runs = root.join("runs");
            std::fs::create_dir_all(&maps).unwrap(); std::fs::create_dir_all(runs.join("run-1")).unwrap();
            Self { root, maps, runs }
        }
        fn seed(&self) {
            let node = json!({"title":"Producer","bot_id":"bot","input":"work","zone":"zone","blocked_by":[]});
            let mut consumer = node.clone();
            consumer["inputs"] = json!({"artifact":{"source":"node","node_id":"producer","output":"result"}});
            std::fs::write(self.maps.join("map.json"), serde_json::to_vec(&json!({"title":"Map","description":"test","nodes":{"producer":node,"consumer":consumer}})).unwrap()).unwrap();
            let execution = json!({"id":"execution-1","taskId":"task","agent":"pi","zone":"zone","chat":null});
            let previous = json!({"id":"execution-0","taskId":"old-task","agent":"pi","zone":"zone","chat":null});
            let run = json!({"id":"run-1","map":"map.json","createdAt":1,"managerTaskId":"manager","zone":"zone","workspaceName":"workspace","userInput":"request","manager":null,"status":"paused","finished":null,"nodes":{"producer":{"status":"failed","execution":execution,"failure":null,"completion":null,"previousExecutions":[{"execution":previous,"failure":null}]}}});
            std::fs::write(self.runs.join("run-1/run.json"), serde_json::to_vec(&run).unwrap()).unwrap();
            let event = json!({"nodeId":"producer","action":"retry","before":"failed","after":"running","execution":previous,"currentExecution":execution,"custom":{"retained":true}});
            std::fs::write(self.runs.join("run-1/history.jsonl"), format!("{{ \"action\" : \"unrelated\", \"payload\" : [1,2] }}\n\n{event}\n")).unwrap();
            let chat = json!({"sessionId":"s","workspaceId":"w","tabId":"t","paneId":"p","agentName":"pi"});
            let report = json!({"id":"report-1","type":"report","at":2,"runId":"run-1","nodeId":"producer","executionId":"execution-0","source":chat,"target":chat,"body":"done","outputs":{"historical":"C:/result.txt"}});
            std::fs::write(self.runs.join("run-1/messages.jsonl"), format!("{report}\n")).unwrap();
        }
        fn read(&self, relative: &str) -> Value { serde_json::from_slice(&std::fs::read(self.root.join(relative)).unwrap()).unwrap() }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.root); } }

    // Approved public seam: the offline migration and its durable files. Literal
    // expectations distinguish report-derived Map IDs from invented execution rules.
    #[test]
    fn migrates_maps_reports_and_all_execution_snapshots_without_changing_state() {
        let f = Fixture::new(); f.seed();
        let original = std::fs::read(f.runs.join("run-1/run.json")).unwrap();
        assert_eq!(migrate(&f.maps, &f.runs).unwrap(), 4);
        let map = f.read("maps/map.json");
        assert_eq!(map["nodes"]["producer"]["output_rule"], json!([{"id":"historical"},{"id":"result"}]));
        assert_eq!(map["nodes"]["consumer"]["output_rule"], json!([]));
        assert_eq!(map["nodes"]["consumer"]["input_rule"], json!([{"id":"artifact","type":"node","node_id":"producer","output_id":"result"}]));
        assert!(map["nodes"]["consumer"].get("inputs").is_none());
        let run = f.read("runs/run-1/run.json");
        assert_eq!(run["status"], "paused"); assert_eq!(run["nodes"]["producer"]["status"], "failed");
        for execution in [&run["nodes"]["producer"]["execution"], &run["nodes"]["producer"]["previousExecutions"][0]["execution"]] {
            for field in ["inputRule", "outputRule", "inputs"] { assert_eq!(execution.get(field), Some(&Value::Null)); }
        }
        let history = std::fs::read_to_string(f.runs.join("run-1/history.jsonl")).unwrap();
        assert!(history.starts_with("{ \"action\" : \"unrelated\", \"payload\" : [1,2] }\n\n"));
        let event: Value = serde_json::from_str(history.lines().last().unwrap()).unwrap();
        assert_eq!(event["execution"]["outputRule"], Value::Null); assert_eq!(event["currentExecution"]["inputs"], Value::Null);
        assert_eq!(event["custom"], json!({"retained":true}));
        let report: Value = serde_json::from_str(std::fs::read_to_string(f.runs.join("run-1/messages.jsonl")).unwrap().trim()).unwrap();
        assert!(report.get("source").is_none()); assert!(report.get("target").is_none());
        assert!(report["from"].is_object()); assert!(report["to"].is_object());
        assert_eq!(report["outputs"], json!([{"id":"historical","path":"C:/result.txt"}]));
        assert_eq!(std::fs::read(backup(&f.runs.join("run-1/run.json"))).unwrap(), original);
        assert_eq!(migrate(&f.maps, &f.runs).unwrap(), 0);
    }

    #[test]
    fn preserves_older_history_events_without_agent_metadata() {
        let f = Fixture::new(); f.seed();
        let path = f.runs.join("run-1/history.jsonl");
        let mut event = json!({"nodeId":"producer","action":"dispatch","execution":{"id":"execution-1","taskId":"task","zone":"zone","chat":null}});
        let original = event.clone();
        let bytes = format!("{event}\n");
        std::fs::write(&path, &bytes).unwrap();
        migrate(&f.maps, &f.runs).unwrap();
        event = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        let mut expected = original;
        for field in ["inputRule", "outputRule", "inputs"] { expected["execution"][field] = Value::Null; }
        assert_eq!(event, expected);
        assert!(event["execution"].get("agent").is_none());
        assert_eq!(std::fs::read(backup(&path)).unwrap(), bytes.as_bytes());
    }

    #[test]
    fn prevalidates_every_map_before_writing_any_source_or_backup() {
        let f = Fixture::new(); f.seed();
        let path = f.maps.join("map.json"); let original = std::fs::read(&path).unwrap();
        std::fs::write(f.maps.join("broken.json"), b"{\"title\":\"bad\"}").unwrap();
        assert!(migrate(&f.maps, &f.runs).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!backup(&path).exists()); assert!(!backup(&f.runs.join("run-1/run.json")).exists());
    }

    #[test]
    fn rejects_ambiguous_fields_duplicate_keys_and_truncated_history_without_writes() {
        for invalid in [
            "{\"source\":{},\"from\":{}}\n",
            "{\"id\":\"one\",\"id\":\"two\"}\n",
            "{\"id\":\"truncated\"}",
        ] {
            let f = Fixture::new(); f.seed();
            let path = f.runs.join("run-1/messages.jsonl"); std::fs::write(&path, invalid).unwrap();
            assert!(migrate(&f.maps, &f.runs).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
            assert!(!backup(&f.maps.join("map.json")).exists());
        }
    }

    #[test]
    fn preserves_known_frozen_execution_arrays_instead_of_latest_map_rules() {
        let f = Fixture::new(); f.seed();
        let mut run = f.read("runs/run-1/run.json");
        let execution = &mut run["nodes"]["producer"]["execution"];
        execution["inputRule"] = json!([]);
        execution["outputRule"] = json!([{"id":"frozen-not-current-map"}]);
        execution["inputs"] = json!([{"id":"actual-file","path":"C:/actual.txt"}]);
        let expected = execution.clone();
        std::fs::write(f.runs.join("run-1/run.json"), serde_json::to_vec(&run).unwrap()).unwrap();
        migrate(&f.maps, &f.runs).unwrap();
        assert_eq!(f.read("runs/run-1/run.json")["nodes"]["producer"]["execution"], expected);
    }

    #[test]
    fn migrated_map_can_rename_outputs_without_reimporting_historical_ids() {
        let f = Fixture::new(); f.seed();
        migrate(&f.maps, &f.runs).unwrap();
        let mut map = f.read("maps/map.json");
        map["nodes"]["producer"]["output_rule"] = json!([{"id":"renamed"}]);
        map["nodes"]["consumer"]["input_rule"][0]["output_id"] = json!("renamed");
        let path = f.maps.join("map.json");
        let current = serde_json::to_vec(&map).unwrap();
        std::fs::write(&path, &current).unwrap();
        let reports = std::fs::read(f.runs.join("run-1/messages.jsonl")).unwrap();
        let original_backup = std::fs::read(backup(&path)).unwrap();
        assert_eq!(migrate(&f.maps, &f.runs).unwrap(), 0);
        assert_eq!(std::fs::read(&path).unwrap(), current);
        assert_eq!(std::fs::read(f.runs.join("run-1/messages.jsonl")).unwrap(), reports);
        assert_eq!(std::fs::read(backup(&path)).unwrap(), original_backup);
    }

    #[test]
    fn rejects_conflicting_backup_before_any_publication() {
        let f = Fixture::new(); f.seed();
        let path = f.maps.join("map.json"); let original = std::fs::read(&path).unwrap();
        std::fs::write(backup(&path), b"prior original").unwrap();
        assert!(migrate(&f.maps, &f.runs).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read(backup(&path)).unwrap(), b"prior original");
        assert!(!backup(&f.runs.join("run-1/run.json")).exists());
    }
}

