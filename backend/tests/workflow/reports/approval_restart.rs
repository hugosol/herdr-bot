//! P14: public HTTP and two actual owned backend processes, never edited Runs.
use super::*;
use std::{io::{BufRead, BufReader, Write}, net::SocketAddr, process::{Child, Command, Stdio}, sync::mpsc};

struct Panel {
    child: Child,
    addr: SocketAddr,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Panel {
    fn start(s: &Scenario, generation: u8) -> Self {
        let root = s.data.path();
        let stderr = root.join(format!("approval-{generation}-stderr.log"));
        let stdout = root.join(format!("approval-{generation}-stdout.log"));
        let child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .args(["--config", "approval-panel.toml"]).current_dir(root)
            .env("HOME", root).env("USERPROFILE", root)
            .env("APPDATA", root.join("appdata")).env("LOCALAPPDATA", root.join("localappdata"))
            .env("XDG_CONFIG_HOME", root.join("xdg-config")).env("XDG_DATA_HOME", root.join("xdg-data"))
            .env("XDG_STATE_HOME", root.join("xdg-state")).env("XDG_CACHE_HOME", root.join("xdg-cache"))
            .env_remove("HERDR_FIXTURE_DIR").env_remove("HERDR_BOT_FIXTURE_RECORD_DIR")
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap())).spawn().unwrap();
        let mut panel = Self { child, addr: ([127,0,0,1],0).into(), reader: None };
        let pipe = panel.child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        panel.reader = Some(std::thread::spawn(move || {
            let mut log = fs::File::create(stdout).unwrap();
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                writeln!(log, "{line}").unwrap();
                if let Some(address) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(address.trim_end_matches('/').parse().unwrap());
                }
            }
        }));
        panel.addr = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|error| panic!("backend startup: {error}; {}", fs::read_to_string(stderr).unwrap()));
        panel
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        let kill = self.child.kill();
        let exited = self.child.wait();
        let drained = self.reader.take().map(|reader| reader.join());
        // Record cleanup independently so an original assertion is not replaced.
        if kill.is_err() || exited.is_err() || drained.as_ref().is_some_and(|result| result.is_err()) {
            eprintln!("approval backend cleanup pid={} kill={kill:?} wait={exited:?} stdout={drained:?}", self.child.id());
            if !std::thread::panicking() { panic!("owned approval backend cleanup failed"); }
        }
    }
}
async fn request(panel: &Panel, route: &str, body: Value, expected: u16) -> Value {
    let response = post_json_over_tcp(panel.addr, route, &body.to_string()).await;
    assert_eq!(response.status, expected, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

// A cached flag or automatic startup reclaim would fail the full persisted
// execution readback / denied old-authority checks after a confirmed OS exit.
#[tokio::test]
async fn approved_execution_survives_actual_backend_exit_without_reclaim_or_input_replay() {
    let s = Scenario::without_output_contract().await;
    let root = s.data.path();
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    map["nodes"]["A"]["auto_approved"] = json!(false);
    fs::write(&path, map.to_string()).unwrap();
    let config = json!({
        "listen_addr":"127.0.0.1:0", "static_directory":"unused-static", "data_directory":root.join("panel-data"),
        "prompts_file":s.config.prompts_file,"mdns":{"enabled":false},
        "operations":{"start":{"script":"unused.bat","working_directory":root},"stop":{"script":"unused.bat","working_directory":root}},
        "herdr":{"workspace_name":s.config.herdr.workspace_name,"executable":s.herdr.executable()},
        "zones":{"registry_file":s.config.zones.registry_file},
        "tasks":{"storage_file":s.config.tasks.storage_file,"metadata_file":s.config.tasks.metadata_file},
        "sessions":{"history_file":root.join("panel-session-history.json")},
        "scheduler":{"map_directory":s.config.scheduler.map_directory,"bot_directory":s.config.scheduler.bot_directory,"run_directory":s.config.scheduler.run_directory,"run_instructions":s.config.scheduler.run_instructions}
    });
    fs::write(root.join("approval-panel.toml"), toml::to_string(&config).unwrap()).unwrap();
    let first = Panel::start(&s, 1);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"persist-manager"},"native_cwd":root}));
    let started = request(&first, "/api/scheduler/maps/work.json/runs", json!({"managerTaskId":s.manager_task,"zone":"manager","userInput":"One independent business input, never replay"}), 201).await;
    request(&first, "/api/workflow/manager/prepare_inputs", json!({"sessionId":"persist-manager","fileInputs":[],"confirmed":true,"reason":"No required materials"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"persist-node"},"native_cwd":root.join("node-zone")}));
    let dispatched = request(&first, "/api/workflow/manager/dispatch", json!({"sessionId":"persist-manager","nodes":["A"]}), 200).await;
    let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
    request(&first, "/api/workflow/node/approve", json!({"sessionId":"persist-node","executionId":execution["id"],"source":"manual"}), 200).await;
    let before = request(&first, "/api/workflow/manager/status", json!({"sessionId":"persist-manager"}), 200).await;
    assert_eq!(before["snapshot"]["nodes"]["A"]["execution"]["approved"], true);
    assert_eq!(before["snapshot"]["nodes"]["A"]["execution"]["approvalSource"], "manual");
    let calls = s.herdr.calls();
    let old_pid = first.child.id();
    drop(first); // kill + wait + stdout join before a new backend is started.
    let second = Panel::start(&s, 2);
    assert_ne!(second.child.id(), old_pid);
    map["nodes"]["A"]["auto_approved"] = json!(true);
    fs::write(path, map.to_string()).unwrap();
    let response = get_over_tcp(second.addr, &format!("/api/workflow/runs/{}", started["id"].as_str().unwrap())).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let after: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(after["run"], before["snapshot"]);
    assert_eq!(after["run"]["nodes"]["A"]["execution"]["autoApproved"], false);
    assert_eq!(after["reports"], before["reports"]);
    assert_eq!(after["managed"], false);
    request(&second, "/api/workflow/manager/status", json!({"sessionId":"persist-manager"}), 403).await;
    request(&second, "/api/workflow/manager/resume", json!({"sessionId":"persist-manager"}), 403).await;
    request(&second, "/api/workflow/node/context", json!({"sessionId":"persist-node"}), 403).await;
    request(&second, "/api/workflow/node/approve", json!({"sessionId":"persist-node","executionId":execution["id"],"source":"manual"}), 403).await;
    assert_eq!(s.herdr.calls(), calls, "restart must neither launch nor replay input");
    println!("P14 oldPid={old_pid} confirmedExited=true newPid={} readback={after} calls={calls:?}", second.child.id());
    drop(second);
}
