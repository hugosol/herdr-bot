use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
use serde_json::{Value, json};
use herdr_bot::portability_migration::migrate;

struct Fixture { root: PathBuf }
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!("herdr-zone-migration-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(root.join("maps")).unwrap();
        Self { root }
    }
    fn put(&self, name: &str, value: Value) { fs::write(self.root.join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap(); }
    fn read(&self, name: &str) -> Value { serde_json::from_slice(&fs::read(self.root.join(name)).unwrap()).unwrap() }
    fn run(&self) -> anyhow::Result<usize> { migrate(&self.root.join("zones.json"), &self.root.join("maps"), &self.root.join("mapping.json")) }
}
impl Drop for Fixture { fn drop(&mut self) { fs::remove_dir_all(&self.root).unwrap(); } }
fn map(zone: &str, file: &str) -> Value {
    json!({"title":"Migration", "description":"Raw ${prompt} remains", "nodes":{"worker":{
        "title":"Worker", "bot_id":"worker", "input":"Raw input", "zone":zone, "blocked_by":[],
        "input_rule":[{"type":"file", "id":"source", "path":file}, {"type":"node", "id":"upstream", "node_id":"other", "output_id":"result"}]
    }}})
}

#[test]
fn migrates_registry_maps_and_files_but_preserves_history_and_raw_prompts() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/old/project", "id":"team/project", "local_path":"D:/current/project"}]));
    f.put("zones.json", json!({"zones":[{"path":"C:\\old\\project"}]}));
    f.put("maps/map.json", map("C:\\old\\project", "C:\\old\\project\\inputs\\材料.txt"));
    f.put("run.json", json!({"zone":"C:/old/project"}));
    f.put("chats.json", json!({"cwd":"C:/old/project"}));
    let history = fs::read(f.root.join("run.json")).unwrap();
    let chats = fs::read(f.root.join("chats.json")).unwrap();
    let original = fs::read(f.root.join("maps/map.json")).unwrap();
    assert_eq!(f.run().unwrap(), 2);
    assert_eq!(f.read("zones.json"), json!({"zones":[{"id":"team/project","path":"D:/current/project"}]}));
    let migrated = f.read("maps/map.json");
    assert_eq!(migrated["nodes"]["worker"]["zone"], "team/project");
    assert_eq!(migrated["nodes"]["worker"]["input_rule"][0], json!({"type":"file","id":"source","path":"inputs/材料.txt","zone_id":"team/project"}));
    assert_eq!(migrated["nodes"]["worker"]["input_rule"][1]["type"], "node");
    assert_eq!(migrated["description"], "Raw ${prompt} remains");
    assert_eq!(migrated["nodes"]["worker"]["input"], "Raw input");
    assert_eq!(fs::read(f.root.join("maps/map.json.zone-portability-v1.bak")).unwrap(), original);
    assert_eq!(fs::read(f.root.join("run.json")).unwrap(), history);
    assert_eq!(fs::read(f.root.join("chats.json")).unwrap(), chats);
    assert_eq!(f.run().unwrap(), 0);
}

#[test]
fn missing_mapping_does_not_publish_any_source() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/known", "id":"team/known"}]));
    f.put("zones.json", json!({"zones":[{"path":"C:/known"}]}));
    f.put("maps/map.json", map("C:/unknown", "C:/known/input.txt"));
    let registry = fs::read(f.root.join("zones.json")).unwrap();
    let error = f.run().unwrap_err().to_string();
    assert!(format!("{:#}", f.run().unwrap_err()).contains("missing mapping"), "{error}");
    assert_eq!(fs::read(f.root.join("zones.json")).unwrap(), registry);
    assert!(!f.root.join("zones.json.zone-portability-v1.bak").exists());
}

#[test]
fn overlapping_file_roots_require_explicit_disambiguation() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/project","id":"team/root"},{"path":"C:/project/data","id":"team/data"}]));
    f.put("maps/map.json", map("C:/project", "C:/project/data/input.txt"));
    assert!(f.run().unwrap_err().to_string().contains("2 matching mappings"));
    let mut value = f.read("maps/map.json");
    value["nodes"]["worker"]["input_rule"][0]["zone_id"] = json!("team/data");
    f.put("maps/map.json", value);
    assert_eq!(f.run().unwrap(), 1);
    assert_eq!(f.read("maps/map.json")["nodes"]["worker"]["input_rule"][0]["path"], "input.txt");
}

#[test]
fn duplicate_path_ids_and_conflicting_alias_bindings_are_rejected() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/project","id":"one"},{"path":"C:\\project","id":"two"}]));
    assert!(f.run().unwrap_err().to_string().contains("ambiguous mapping"));
    f.put("mapping.json", json!([{"path":"C:/project","id":"one"},{"path":"D:/project","id":"one"}]));
    assert!(f.run().unwrap_err().to_string().contains("conflicting local bindings"));
}

#[test]
fn aliases_with_explicit_shared_binding_and_relative_manager_input_work() {
    let f = Fixture::new();
    f.put("mapping.json", json!([
        {"path":"C:/old","id":"team/project","local_path":"D:/current"},
        {"path":"D:/current","id":"team/project","local_path":"D:/current"}
    ]));
    f.put("zones.json", json!({"zones":["C:/old"]}));
    f.put("maps/map.json", map("C:/old", "inputs/source.txt"));
    assert_eq!(f.run().unwrap(), 2);
    let rule = &f.read("maps/map.json")["nodes"]["worker"]["input_rule"][0];
    assert!(rule.get("zone_id").is_none());
    assert_eq!(rule["path"], "inputs/source.txt");
}

#[test]
fn file_prefix_collision_and_escape_paths_fail_without_changes() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/project","id":"team/project"}]));
    f.put("maps/map.json", map("team/project", "C:/project-other/file.txt"));
    assert!(f.run().unwrap_err().to_string().contains("0 matching mappings"));
    for path in ["../outside.txt", "C:/project/../outside.txt", "/root/../outside.txt", "inputs\\source.txt"] {
        f.put("maps/map.json", map("team/project", path));
        assert!(f.run().is_err(), "accepted {path}");
    }
    assert!(!f.root.join("maps/map.json.zone-portability-v1.bak").exists());
}

#[test]
fn conflicting_backup_prevents_any_publication() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/project","id":"team/project"}]));
    f.put("zones.json", json!({"zones":[{"path":"C:/project"}]}));
    f.put("maps/map.json", map("C:/project", "C:/project/file.txt"));
    fs::write(f.root.join("maps/map.json.zone-portability-v1.bak"), b"unrelated original").unwrap();
    assert!(f.run().unwrap_err().to_string().contains("backup differs"));
    assert!(!f.root.join("zones.json.zone-portability-v1.bak").exists());
    assert!(f.read("zones.json")["zones"][0].get("id").is_none());
}

#[test]
fn public_cli_migrates_explicit_inputs_and_reports_history_unchanged() {
    let f = Fixture::new();
    f.put("mapping.json", json!([{"path":"C:/project","id":"team/project"}]));
    f.put("maps/map.json", map("C:/project", "C:/project/file.txt"));
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_migrate-zone-portability"))
        .arg(f.root.join("zones.json")).arg(f.root.join("maps")).arg(f.root.join("mapping.json"))
        .output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(String::from_utf8_lossy(&result.stdout).contains("Migrated 1 files"));
    assert!(String::from_utf8_lossy(&result.stdout).contains("Historical Runs and Chats were not changed"));
    assert_eq!(f.read("maps/map.json")["nodes"]["worker"]["zone"], "team/project");
    assert!(!f.root.join("zones.json").exists());
}

#[test]
fn public_cli_requires_explicit_mapping_argument() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_migrate-zone-portability"))
        .args(["unused-registry", "unused-maps"]).output().unwrap();
    assert!(!result.status.success());
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(error.contains("<mapping.json>"), "{error}");
    assert!(error.contains("Stop ALL backend processes"), "{error}");
}
