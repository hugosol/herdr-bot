use super::{Duration, fs, json};
// Included by the delegation S2 target; this host owns no product behavior.
pub(super) struct TestServer {
    child: std::process::Child,
    addr: std::net::SocketAddr,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl TestServer {
    pub(super) async fn start(config: &herdr_bot::Config) -> Self {
        use std::{
            io::{BufRead, BufReader, Write},
            process::{Command, Stdio},
            sync::mpsc,
        };
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let index = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = config.tasks.storage_file.parent().unwrap();
        let config_file = root.join(format!("process-{index}.toml"));
        let stderr = root.join(format!("stderr-{index}.log"));
        let stdout = root.join(format!("stdout-{index}.log"));
        let input = json!({
            "listen_addr":"127.0.0.1:0", "static_directory":config.static_directory,
            "data_directory":config.data_directory,"prompts_file":config.prompts_file,
            "mdns":{"enabled":false},
            "operations":{
                "start":{"script":config.operations.start.script,"working_directory":root},
                "stop":{"script":config.operations.stop.script,"working_directory":root}
            },
            "herdr":{"workspace_name":config.herdr.workspace_name,"executable":config.herdr.executable},
            "zones":{"registry_file":config.zones.registry_file},
            "tasks":{"storage_file":config.tasks.storage_file,"metadata_file":config.tasks.metadata_file},
            "sessions":{"history_file":config.sessions.history_file},
            "scheduler":{"bot_directory":config.scheduler.bot_directory,"map_directory":config.scheduler.map_directory,
                "run_directory":config.scheduler.run_directory,"run_instructions":root.join("run-guide.md")},
            "suites":{"definition_file":config.suites.definition_file,"mcp_file":config.suites.mcp_file,
                "generated_directory":config.suites.generated_directory}
        });
        fs::write(&config_file, toml::to_string(&input).unwrap()).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .args(["--config"])
            .arg(config_file)
            .current_dir(root)
            .env("HOME", root)
            .env("USERPROFILE", root)
            .env("APPDATA", root.join("appdata"))
            .env("LOCALAPPDATA", root.join("localappdata"))
            .env("XDG_CONFIG_HOME", root.join("xdg-config"))
            .env("XDG_DATA_HOME", root.join("xdg-data"))
            .env("XDG_STATE_HOME", root.join("xdg-state"))
            .env("XDG_CACHE_HOME", root.join("xdg-cache"))
            .env_remove("HERDR_FIXTURE_DIR")
            .env_remove("HERDR_BOT_FIXTURE_RECORD_DIR")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
            .spawn()
            .unwrap();
        let mut server = Self {
            child,
            addr: ([127, 0, 0, 1], 0).into(),
            reader: None,
        };
        let pipe = server.child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        server.reader = Some(std::thread::spawn(move || {
            let mut log = fs::File::create(stdout).unwrap();
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                writeln!(log, "{line}").unwrap();
                if let Some(addr) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(addr.trim_end_matches('/').parse().unwrap());
                }
            }
        }));
        server.addr = rx
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|error| {
                panic!(
                    "backend startup: {error}; {}",
                    fs::read_to_string(stderr).unwrap()
                )
            });
        eprintln!(
            "delegation S2 backend pid={} addr={} logs={}",
            server.child.id(),
            server.addr,
            root.display()
        );
        server
    }
    pub(super) fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }
}
impl Drop for TestServer {
    fn drop(&mut self) {
        let mut errors = Vec::new();
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            #[cfg(windows)]
            {
                let result = std::process::Command::new("taskkill")
                    .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                    .output();
                if !result.as_ref().is_ok_and(|output| output.status.success()) {
                    errors.push(format!("owned process tree termination: {result:?}"));
                    if let Err(error) = self.child.kill() {
                        errors.push(format!("backend kill: {error}"));
                    }
                }
            }
            #[cfg(not(windows))]
            if let Err(error) = self.child.kill() {
                errors.push(format!("backend kill: {error}"));
            }
        }
        if let Err(error) = self.child.wait() {
            errors.push(format!("backend wait: {error}"));
        }
        if let Some(reader) = self.reader.take() {
            if reader.join().is_err() {
                errors.push("stdout reader failed".into());
            }
            eprintln!(
                "delegation S2 backend exited pid={} cleanup={errors:?}",
                self.child.id()
            );
        }
        if std::thread::panicking() {
            eprintln!("preserving original test failure; cleanup={errors:?}");
        } else {
            assert!(errors.is_empty(), "{errors:?}");
        }
    }
}
