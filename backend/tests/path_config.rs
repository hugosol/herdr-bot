use herdr_bot::config::Config;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("herdr-path-config-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn config(extra: &str) -> String {
    format!(r#"listen_addr = '127.0.0.1:0'
{extra}
[operations.start]
script = '${{roots.toolkit}}/start.cmd'
working_directory = 'work'
[operations.stop]
script = 'stop.cmd'
working_directory = '${{roots.checkout}}'
[herdr]
workspace_name = 'isolated'
"#)
}

#[test]
fn config_load_resolves_each_source_and_keeps_a_root_snapshot() {
    let fixture = Fixture::new();
    let roots_path = fixture.write("machine/paths.toml", "[roots]\ncheckout = '../source'\ntoolkit = '${roots.checkout}/tools'\n");
    let file = fixture.write("deployment/config.toml", &config("paths_file = '../machine/paths.toml'\nstatic_directory = '${roots.checkout}/public'\ndata_directory = 'state'"));
    let loaded = Config::load(&file).unwrap();
    assert_eq!(loaded.operations.start.script, fixture.0.join("source/tools/start.cmd"));
    assert_eq!(loaded.operations.start.working_directory, fixture.0.join("deployment/work"));
    assert_eq!(loaded.operations.stop.script, fixture.0.join("deployment/stop.cmd"));
    assert_eq!(loaded.static_directory, fixture.0.join("source/public"));
    assert_eq!(loaded.data_directory, fixture.0.join("deployment/state"));
    assert_eq!(loaded.paths_file.as_deref(), Some(roots_path.as_path()));
    assert_eq!(loaded.herdr.executable, Path::new("herdr"));
    for path in [&loaded.zones.registry_file, &loaded.tasks.storage_file, &loaded.tasks.metadata_file,
        &loaded.sessions.history_file, &loaded.scheduler.map_directory, &loaded.scheduler.bot_directory,
        &loaded.scheduler.run_directory, &loaded.scheduler.run_instructions] {
        assert!(path.starts_with(&loaded.base_directory), "{}", path.display());
    }
    std::fs::write(&roots_path, "[roots]\ncheckout = '../replacement'\ntoolkit = '${roots.checkout}/new-tools'\n").unwrap();
    assert_eq!(loaded.roots.resolve(Path::new("${roots.toolkit}/later.cmd"), &loaded.base_directory).unwrap(), fixture.0.join("source/tools/later.cmd"));
    assert_eq!(Config::load(&file).unwrap().operations.start.script, fixture.0.join("replacement/new-tools/start.cmd"));
}

#[test]
fn config_load_reports_invalid_roots_instead_of_falling_back() {
    for (roots, diagnostic) in [
        ("[roots]\ntoolkit = '${roots.checkout}'\ncheckout = '${roots.toolkit}'\n", "循环"),
        ("[roots]\ntoolkit = '${roots.missing}'\n", "missing"),
        ("[roots]\ntoolkit = ''\n", "不能为空"),
        ("[roots]\ntoolkit = 'before/${roots.checkout}'\ncheckout = '.'\n", "前缀"),
        ("[roots]\ntoolkit = '${roots.checkout}/x/${roots.checkout}'\ncheckout = '.'\n", "前缀"),
        ("[roots]\ntoolkit = '${roots.}'\n", "名称"),
    ] {
        let fixture = Fixture::new();
        fixture.write("paths.toml", roots);
        let file = fixture.write("config.toml", &config("paths_file = 'paths.toml'"));
        let error = format!("{:#}", Config::load(&file).unwrap_err());
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn config_load_rejects_missing_root_files_and_unconfigured_references() {
    let fixture = Fixture::new();
    let file = fixture.write("config.toml", &config("paths_file = 'missing.toml'"));
    let error = format!("{:#}", Config::load(&file).unwrap_err());
    assert!(error.contains("missing.toml"), "{error}");
    fixture.write("config.toml", &config(""));
    let error = format!("{:#}", Config::load(&file).unwrap_err());
    assert!(error.contains("toolkit"), "{error}");
}

#[cfg(windows)]
#[test]
fn config_load_rejects_drive_relative_paths_but_keeps_absolute_windows_paths() {
    let fixture = Fixture::new();
    let plain = config("")
        .replace("${roots.toolkit}/start.cmd", "start.cmd")
        .replace("${roots.checkout}", "work");
    for raw in [
        plain.replace("script = 'start.cmd'", "script = 'C:relative.cmd'"),
        plain.replace("workspace_name = 'isolated'", "workspace_name = 'isolated'\nexecutable = 'C:herdr.exe'"),
        config("paths_file = 'C:paths.toml'"),
    ] {
        let file = fixture.write("config.toml", &raw);
        let error = format!("{:#}", Config::load(&file).unwrap_err());
        assert!(error.contains("驱动器相对路径"), "{error}");
    }
    fixture.write("paths.toml", "[roots]\ncheckout = 'C:relative'\ntoolkit = '${roots.checkout}/tools'\n");
    let file = fixture.write("config.toml", &config("paths_file = 'paths.toml'"));
    let error = format!("{:#}", Config::load(&file).unwrap_err());
    assert!(error.contains("驱动器相对路径"), "{error}");

    for absolute in ["C:/portable-test/start.cmd", r"\\server\share\portable-test\start.cmd"] {
        fixture.write("config.toml", &plain.replace("script = 'start.cmd'", &format!("script = '{absolute}'")));
        let loaded = Config::load(&file).unwrap();
        assert_eq!(loaded.operations.start.script, Path::new(absolute));
        assert_eq!(loaded.herdr.executable, Path::new("herdr"));
    }
}

#[test]
fn isolated_config_needs_no_roots_file_and_resolves_file_not_command_names() {
    let fixture = Fixture::new();
    let raw = config("[zones]\ncreate_parent_directory = 'new-zones'")
        .replace("${roots.toolkit}/start.cmd", "start.cmd")
        .replace("${roots.checkout}", "work")
        .replace("workspace_name = 'isolated'", "workspace_name = 'isolated'\nexecutable = './bin/herdr'");
    let file = fixture.write("config.toml", &raw);
    let loaded = Config::load(&file).unwrap();
    assert_eq!(loaded.operations.start.script, fixture.0.join("start.cmd"));
    assert_eq!(loaded.herdr.executable, fixture.0.join("bin/herdr"));
    assert_eq!(loaded.zones.create_parent_directory, Some(fixture.0.join("new-zones")));
    let invalid = raw.replace("script = 'start.cmd'", "script = 'prefix/${roots.toolkit}'");
    fixture.write("config.toml", &invalid);
    assert!(Config::load(&file).is_err());
}
