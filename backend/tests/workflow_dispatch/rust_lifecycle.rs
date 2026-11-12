//! S1 public HTTP / real process / owned business-file boundary.
//! Seams: manager dispatch/status/pause/resume/retry and independent Run/Bot
//! reads, actual delivered JSON files, and termination/restart of our backend.
//! The recording Herdr is only the external notification/manager transport.
use super::*;
use futures_util::FutureExt;
use std::{collections::BTreeMap, io::{BufRead, BufReader, Write}, net::SocketAddr, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::mpsc, time::Instant};

struct Backend {
    child: Child,
    addr: SocketAddr,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Backend {
    fn start(root: &Path, generation: u32) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-bot"));
        command.args(["--config", "panel.toml"]).current_dir(root)
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(root.join(format!("stderr-{generation}.log"))).unwrap()));
        for key in ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"] {
            command.env(key, root.join("home").join(key));
        }
        for (key, _) in std::env::vars().filter(|(key, _)| key.starts_with("HERDR_")) { command.env_remove(key); }
        let mut child = command.spawn().unwrap();
        let pipe = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let log = root.join(format!("stdout-{generation}.log"));
        let reader = std::thread::spawn(move || {
            let mut log = fs::File::create(log).unwrap();
            for line in BufReader::new(pipe).lines() {
                let line = line.expect("drain owned backend stdout");
                writeln!(log, "{line}").unwrap();
                if let Some(addr) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(addr.trim_end_matches('/').parse().unwrap());
                }
            }
        });
        // Construct the guard before observing readiness: startup failures also
        // terminate and wait for the exact owned process, never just delete files.
        let mut backend = Self { child, addr: ([127,0,0,1],0).into(), reader: Some(reader) };
        backend.addr = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|error| {
            panic!("backend readiness {error}: {}", fs::read_to_string(root.join(format!("stderr-{generation}.log"))).unwrap())
        });
        println!("script lifecycle backend generation={generation} pid={} addr={} root={}", backend.child.id(), backend.addr, root.display());
        backend
    }
    fn stop(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        match self.child.try_wait() {
            Ok(None) => {
                let result = Command::new("taskkill").args(["/PID", &self.child.id().to_string(), "/T", "/F"]).output();
                if !result.as_ref().is_ok_and(|output|output.status.success()) {
                    errors.push(format!("terminate owned backend tree: {result:?}"));
                    if let Err(error) = self.child.kill() { errors.push(format!("terminate backend: {error}")); }
                }
            }
            Ok(Some(_)) => {},
            Err(error) => errors.push(format!("query backend: {error}")),
        }
        match self.child.wait() {
            Ok(status) => println!("script lifecycle backend exited pid={} status={status}", self.child.id()),
            Err(error) => errors.push(format!("wait backend: {error}")),
        }
        if let Some(reader) = self.reader.take() {
            if reader.join().is_err() { errors.push("backend stdout drain failed".into()); }
        }
        errors
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        if self.reader.is_some() {
            let errors = self.stop();
            if !errors.is_empty() {
                eprintln!("backend cleanup errors: {errors:?}");
                if !std::thread::panicking() { panic!("{errors:?}"); }
            }
        }
    }
}

struct Lifecycle {
    backend: Backend,
    // Do not let temporary-directory destructors remove evidence underneath
    // a process whose exit could not be confirmed. Explicitly drop after cleanup.
    root: std::mem::ManuallyDrop<TempDir>,
    herdr: std::mem::ManuallyDrop<HerdrFixture>,
    clock: Instant,
    target: Value,
    run_id: String,
    outputs: std::cell::RefCell<std::collections::BTreeSet<PathBuf>>,
}
impl Lifecycle {
    async fn new() -> Self {
        let root = TempDir::new("Rust lifecycle");
        let herdr = HerdrFixture::new("Rust lifecycle external");
        for name in ["maps", "bots", "node", "home"] { fs::create_dir(root.path().join(name)).unwrap(); }
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let config = json!({
            "listen_addr":"127.0.0.1:0", "data_directory":root.path().join("data"),
            "static_directory":repo.join("backend/static"), "prompts_file":root.path().join("prompts.toml"),
            "mdns":{"enabled":false},
            "operations":{"start":{"script":"unused.bat","working_directory":root.path()},"stop":{"script":"unused.bat","working_directory":root.path()}},
            "herdr":{"workspace_name":"lifecycle","executable":herdr.executable()},
            "tasks":{"storage_file":root.path().join("tasks.json"),"metadata_file":root.path().join("options.toml")},
            "zones":{"registry_file":root.path().join("zones.json")},
            "sessions":{"history_file":root.path().join("history.json")},
            "scheduler":{"map_directory":root.path().join("maps"),"bot_directory":root.path().join("bots"),"run_directory":root.path().join("runs"),"run_instructions":repo.join("docs/workflow-runtime.md")}
        });
        fs::write(root.path().join("panel.toml"), toml::to_string(&config).unwrap()).unwrap();
        fs::write(root.path().join("fixed.ts"), "export default function () {}\n").unwrap();
        fs::write(root.path().join("options.toml"), "skills = []\n[[agents]]\nid = 'pi'\nlabel = 'Pi'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'pi-manager-chat'\nlabel = 'Manager role'\nagent = 'pi'\nkind = 'fixed'\nentrypoint = 'fixed.ts'\n").unwrap();
        herdr.set_workspaces(json!([{"workspace_id":"w-lifecycle","label":root.path().file_name().unwrap().to_str().unwrap()}]));
        herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"script-lifecycle-manager"},"native_cwd":root.path()}));
        let backend = Backend::start(root.path(), 1);
        let mut s = Self { backend, root: std::mem::ManuallyDrop::new(root), herdr: std::mem::ManuallyDrop::new(herdr), clock: Instant::now(), target: Value::Null, run_id: String::new(), outputs: Default::default() };
        // Real static delivery from this worktree, not an HTML placeholder.
        assert_eq!(get_over_tcp(s.backend.addr, "/").await.status, 200);
        for (id, path) in [("manager", s.root.path().to_owned()), ("node", s.root.path().join("node"))] {
            s.post("/api/zones", json!({"id":id,"path":path}), 200).await;
        }
        let mut manager = Value::Null;
        for (name, kind) in [("manager", "manager"), ("target", "normal")] {
            let saved = s.post("/api/tasks", json!({"name":name,"taskType":kind,"agent":"pi","model":"default","chatName":"{chat.id}","prompt":"Only explicitly requested work"}), 200).await;
            let task = saved["tasks"].as_array().unwrap().iter().find(|task|task["name"] == name).unwrap().clone();
            if name == "manager" { manager = task; } else { s.target = task; }
        }
        s.post("/api/scheduler/bots/save", json!({"id":"creator","revision":null,"content":json!({"name":"Rust creator","when":"","plan_guide":"","node_guide":"","rust_script":"create_blank_bot"}).to_string()}), 200).await;
        let mut nodes = serde_json::Map::new();
        for (node, bot) in [("create", "first-result"), ("later", "second-result")] {
            let request = json!({"schema_version":1,"kind":"request","target":{"type":"task","task_id":s.target["id"]},"bot":{"id":bot,"name":bot},"requirements":"Create only an empty reusable Bot","constraints":["Do not run target"],"non_goals":["Business execution"],"resource_limits":[],"authorization":{"create_blank":true,"write_materials":true,"save_guides":false},"confirmation":{"evidence":"User authorized this isolated blank Bot","scope":"create_blank"}});
            fs::write(s.root.path().join(format!("{node}-request.json")), request.to_string()).unwrap();
            nodes.insert(node.into(), json!({"title":node,"bot_id":"creator","zone":"node","input":"This is not script parameters","input_rule":[{"id":"request","type":"file","zone_id":"manager","path":format!("{node}-request.json")}],"output_rule":[{"id":"baseline"},{"id":"handoff"}],"blocked_by":[]}));
        }
        fs::write(s.root.path().join("maps/work.json"), json!({"title":"Lifecycle acceptance","description":"Independent ready scripts","nodes":nodes}).to_string()).unwrap();
        s.get("/api/scheduler/maps/work.json").await;
        let started = s.post("/api/scheduler/maps/work.json/runs", json!({"managerTaskId":manager["id"],"zone":"manager","userInput":"Only explicit dispatch creates work"}), 201).await;
        s.run_id = started["id"].as_str().unwrap().to_owned();
        s.action("prepare_inputs", json!({"fileInputs":[],"confirmed":true,"reason":"Reviewed two exact request file bindings"}), 200).await;
        s
    }
    fn mark(&self, event: &str) { println!("script lifecycle t={}ms {event}", self.clock.elapsed().as_millis()); }
    async fn post(&self, path: &str, body: Value, status: u16) -> Value {
        let response = tokio::time::timeout(Duration::from_secs(20), post_json_over_tcp(self.backend.addr, path, &body.to_string())).await.expect("bounded HTTP response");
        self.mark(&format!("POST {path} request={body} HTTP={} response={}",response.status,response.body));
        assert_eq!(response.status, status, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    async fn get(&self, path: &str) -> Value {
        let response = tokio::time::timeout(Duration::from_secs(5), get_over_tcp(self.backend.addr, path)).await.expect("bounded independent read");
        assert_eq!(response.status, 200, "{}", response.body);
        serde_json::from_str(&response.body).unwrap()
    }
    async fn action(&self, action: &str, fields: Value, status: u16) -> Value {
        let mut body = fields.as_object().unwrap().clone();
        body.insert("sessionId".into(), json!("script-lifecycle-manager"));
        self.post(&format!("/api/workflow/manager/{action}"), Value::Object(body), status).await
    }
    async fn detail(&self) -> Value { self.get(&format!("/api/workflow/runs/{}",self.run_id)).await }
    async fn ended(&self) -> Value {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let read = self.detail().await;
                if read["reports"].as_array().unwrap().len() == 1 {
                    let id = read["reports"][0]["id"].as_str().unwrap();
                    if self.herdr.calls().iter().any(|call|call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text|text.starts_with("Workflow report\n") && text.contains(id))) && self.cli_idle() {
                        self.mark(&format!("original execution result={read}"));
                        return read;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("automatic report and notification CLI exit")
    }
    fn cli_idle(&self) -> bool {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new().read(true).write(true).share_mode(0).open(self.herdr.executable()).is_ok()
    }
    async fn assert_delivery(&self, read: &Value, execution: &Value) {
        let report = &read["reports"][0];
        assert_eq!(read["run"]["nodes"]["create"]["execution"]["id"], execution["id"]);
        assert_eq!(read["run"]["nodes"]["create"]["execution"]["lifecycle"]["status"], "succeeded");
        assert_eq!(read["run"]["nodes"]["create"]["status"], "running");
        assert_eq!(report["executionId"], execution["id"]);
        assert_eq!(report["runId"], self.run_id);
        assert_eq!(report["canComplete"], true);
        let edit = self.post("/api/scheduler/bots/edit", json!({"id":"first-result"}), 200).await;
        let expected = json!({"id":"first-result","name":"first-result","when":"","plan_guide":"","node_guide":"","task_id":self.target["id"]});
        assert_eq!(serde_json::from_str::<Value>(edit["revision"]["original"].as_str().unwrap()).unwrap(), expected);
        assert_eq!(report["outputs"].as_array().unwrap().len(), 2);
        for output in report["outputs"].as_array().unwrap() {
            let path = output["path"].as_str().unwrap();
            self.outputs.borrow_mut().insert(PathBuf::from(path));
            let file: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            if output["id"] == "baseline" {
                assert_eq!(file["bot"], expected);
                assert_eq!(file["revision"], edit["revision"]);
                assert_eq!(file["target"]["configuration"], self.target);
            } else {
                assert_eq!(output["id"], "handoff");
                assert_eq!(file["stage"], "created");
                assert_eq!(file["checks"], json!({"bot_read_back":true,"target_unchanged":true}));
            }
            self.mark(&format!("actual file {path}={file}"));
        }
    }
    fn facts(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn scan(path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() { scan(&entry.path(), out); }
                else { out.insert(entry.path(), fs::read(entry.path()).unwrap()); }
            }
        }
        let mut out = BTreeMap::new();
        for name in ["runs", "bots", "node"] { scan(&self.root.path().join(name), &mut out); }
        // Output locations come from the public report, never an assumed zone
        // or product-private material directory. Include their exact raw bytes.
        for path in self.outputs.borrow().iter() { out.insert(path.clone(), fs::read(path).unwrap()); }
        out
    }
    async fn unchanged_window(&self, before: &Value) {
        let calls = self.herdr.calls();
        let files = self.facts();
        self.mark("begin no-automatic-work observation budget=500ms");
        let end = tokio::time::Instant::now() + Duration::from_millis(500);
        loop {
            assert_eq!(self.detail().await, *before);
            assert_eq!(self.herdr.calls(), calls, "no repeated notification or host work");
            assert_eq!(self.facts(), files, "no business rerun or synthetic lifecycle repair");
            if tokio::time::Instant::now() >= end { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.mark("end no-automatic-work observation");
    }
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        let mut errors = self.backend.stop();
        let end = Instant::now() + Duration::from_secs(5);
        while !self.cli_idle() && Instant::now() < end { std::thread::sleep(Duration::from_millis(10)); }
        if !self.cli_idle() { errors.push("owned external CLI did not exit; retained files".into()); }
        if std::thread::panicking() || !errors.is_empty() {
            for entry in fs::read_dir(self.root.path()).unwrap().flatten() {
                if entry.path().extension().is_some_and(|ext|ext == "log") { eprintln!("{}: {}",entry.path().display(),fs::read_to_string(entry.path()).unwrap_or_default()); }
            }
        }
        if errors.is_empty() {
            for path in [self.root.path(), self.herdr.path()] {
                if let Err(error) = fs::remove_dir_all(path) { errors.push(format!("cleanup {}: {error}",path.display())); }
            }
        }
        if errors.is_empty() {
            // SAFETY: these fields are initialized once, dropped only here, and
            // ManuallyDrop prevents any second destructor call after this Drop.
            unsafe {
                std::mem::ManuallyDrop::drop(&mut self.root);
                std::mem::ManuallyDrop::drop(&mut self.herdr);
            }
        } else {
            eprintln!("retaining owned fixture roots {} and {}",self.root.path().display(),self.herdr.path().display());
        }
        eprintln!("script lifecycle cleanup errors={errors:?}");
        if !std::thread::panicking() { assert!(errors.is_empty(),"{errors:?}"); }
    }
}

// P4/P16/P17 source: ticket requires accepted business to outlive TCP and
// pause, while same-Run query/pause finish BEFORE business is released.
// Plausible violations: hold the Run permit throughout Rust work; cancel on
// caller disconnect/pause; resume starts the other ready script or notifies again.
// The real task-file oplock is reached by the compiled capability, not a mock
// or notification gate. Independent reads and bounded responses distinguish
// these errors without assuming any private lock structure or product timeout.
#[tokio::test]
async fn script_lifecycle_disconnect_and_pause_preserve_accepted_business_without_holding_run() {
    let s = Lifecycle::new().await;
    let gate = super::history_integrity::file_gate::Gate::new(&s.root.path().join("tasks.json"));
    let caller = common::OpenJsonRequest::submit(s.backend.addr, "/api/workflow/manager/dispatch", r#"{"sessionId":"script-lifecycle-manager","nodes":["create"]}"#).await;
    let observed = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), ||gate.reached()).await, "compiled business must reach held external task file");
        s.mark("business gate entered; dropping accepted dispatch TCP connection without reading response");
        caller.abort();
        let during = s.detail().await;
        let execution = during["run"]["nodes"]["create"]["execution"].clone();
        assert_eq!(execution["kind"], "rust_script");
        assert_eq!(execution["lifecycle"]["status"], "running");
        assert_eq!(during["reports"], json!([]));
        assert!(!s.root.path().join("bots/first-result.json").exists());
        let status = tokio::time::timeout(Duration::from_secs(2), s.action("status", json!({}), 200)).await.expect("same Run query must finish before gate release");
        assert_eq!(status["snapshot"]["nodes"]["create"]["execution"], execution);
        let pause = tokio::time::timeout(Duration::from_secs(2), s.action("pause", json!({}), 200)).await.expect("same Run pause must finish before gate release");
        assert_eq!(pause["snapshot"]["status"], "paused");
        assert_eq!(pause["snapshot"]["nodes"]["create"]["execution"], execution);
        let dispatch = s.action("dispatch", json!({"nodes":["later"]}), 200).await;
        assert_eq!(dispatch["results"], json!([{"nodeId":"later","status":"rejected","reasons":["Run 状态为 paused"]}]));
        let retry = s.action("retry", json!({"nodeId":"create","confirmed":true,"reason":"Paused cannot grant new execution"}), 409).await;
        assert_eq!(retry["code"], "run-paused");
        let before_release = s.detail().await;
        assert_eq!(before_release["reports"], json!([]));
        assert_eq!(before_release["run"]["nodes"]["create"]["execution"], execution);
        execution
    }).catch_unwind().await;
    s.mark("releasing business gate, including on observation failure");
    drop(gate);
    let cleanup = std::panic::AssertUnwindSafe(s.ended()).catch_unwind().await;
    if let Err(error) = observed {
        if cleanup.is_err() { eprintln!("cleanup also failed while awaiting accepted script report"); }
        std::panic::resume_unwind(error);
    }
    let execution = observed.unwrap();
    let read = cleanup.unwrap();
    assert_eq!(read["run"]["status"], "paused");
    s.assert_delivery(&read, &execution).await;
    let calls_before_resume = s.herdr.calls();
    let business_files = |facts: BTreeMap<PathBuf, Vec<u8>>| facts.into_iter().filter(|(path, _)|!path.starts_with(s.root.path().join("runs"))).collect::<BTreeMap<_, _>>();
    let files_before_resume = business_files(s.facts());
    let resumed = s.action("resume", json!({}), 200).await;
    assert_eq!(resumed["snapshot"]["status"], "running");
    assert!(resumed["ready"].as_array().unwrap().contains(&json!("later")));
    assert_eq!(resumed["snapshot"]["nodes"], read["run"]["nodes"]);
    let after_resume = s.detail().await;
    assert_eq!(after_resume["run"], resumed["snapshot"], "no automatic execution between resume response and independent read");
    assert_eq!(after_resume["reports"], read["reports"]);
    s.unchanged_window(&after_resume).await;
    assert_eq!(s.herdr.calls(), calls_before_resume, "no notification even before observation baseline");
    assert_eq!(business_files(s.facts()), files_before_resume, "no business file effects across resume");
}

// P18 source: an actual backend process restart drops control, not saved facts.
// Plausible violations: rehydrate manager authority, repair interrupted running
// lifecycle, repeat completed business/notifications, or auto-start pending work.
// Keep one completed real write/report and another accepted script at its business
// file gate, kill + wait the original Child, then use a new backend PID and HTTP
// reads. Byte comparisons include all three Run records and real output files.
#[tokio::test]
async fn script_lifecycle_real_restart_preserves_finished_and_interrupted_facts_without_replay() {
    let mut s = Lifecycle::new().await;
    s.action("dispatch", json!({"nodes":["create"]}), 200).await;
    let finished = s.ended().await;
    s.assert_delivery(&finished, &finished["run"]["nodes"]["create"]["execution"]).await;
    let gate = super::history_integrity::file_gate::Gate::new(&s.root.path().join("tasks.json"));
    let interrupted = std::panic::AssertUnwindSafe(async {
        s.action("dispatch", json!({"nodes":["later"]}), 200).await;
        assert!(common::wait_until(Duration::from_secs(5), ||gate.reached()).await, "second compiled business must actually enter external file gate");
        let before = s.detail().await;
        assert_eq!(before["managed"], true);
        assert_eq!(before["run"]["nodes"]["later"]["execution"]["kind"], "rust_script");
        assert_eq!(before["run"]["nodes"]["later"]["execution"]["lifecycle"]["status"], "running");
        assert_eq!(before["reports"], finished["reports"]);
        assert!(!s.root.path().join("bots/second-result.json").exists());
        s.mark(&format!("before real termination detail={before}"));
        let files = s.facts();
        let calls = s.herdr.calls();
        let old_pid = s.backend.child.id();
        s.mark("terminating owned backend while second business gate remains held");
        let errors = s.backend.stop();
        assert!(errors.is_empty(), "termination cleanup errors={errors:?}");
        assert!(s.backend.child.try_wait().unwrap().is_some(), "original Child must be confirmed exited before restart");
        (before, files, calls, old_pid)
    }).catch_unwind().await;
    s.mark("releasing interrupted business gate on success or failure");
    drop(gate);
    let (before, files, calls, old_pid) = match interrupted {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error),
    };
    s.backend = Backend::start(s.root.path(), 2);
    assert_ne!(s.backend.child.id(), old_pid, "new process, not reconstructed AppState");
    let after = s.detail().await;
    s.mark(&format!("after actual restart detail={after}"));
    assert_eq!(after["managed"], false);
    for field in ["id", "run", "reports", "pendingReports"] { assert_eq!(after[field], before[field], "saved field {field}"); }
    assert_eq!(s.facts(), files, "restart must preserve all saved files verbatim");
    assert_eq!(s.herdr.calls(), calls, "startup does not notify or execute");
    let list = s.get("/api/workflow/runs").await;
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["id"], s.run_id);
    assert_eq!(list["runs"][0]["managed"], false);
    // Valid strict request shapes distinguish lost authority from validation
    // failures, and keep every old manager runtime operation fail-closed.
    for (action, fields) in [
        ("status", json!({})),
        ("prepare_inputs", json!({"fileInputs":[],"confirmed":true,"reason":"Old manager cannot prepare"})),
        ("dispatch", json!({"nodes":["later"]})),
        ("complete", json!({"nodeId":"create","reason":"Saved report does not restore authority"})),
        ("fail", json!({"nodeId":"later","reason":"Cannot repair old running lifecycle"})),
        ("reconcile_dispatch", json!({"nodeId":"later","finding":"uncertain","reason":"No authority after restart"})),
        ("recover", json!({"nodeId":"later","confirmed":true,"reason":"No takeover"})),
        ("retry", json!({"nodeId":"later","confirmed":true,"reason":"No replay"})),
        ("pause", json!({})), ("resume", json!({})),
        ("finish", json!({})), ("collect", json!({})),
    ] {
        let rejected = tokio::time::timeout(Duration::from_secs(2), s.action(action, fields, 403)).await.expect("old manager operation must reject without waiting");
        assert_eq!(rejected["code"], "manager-unassociated", "{action}: {rejected}");
    }
    s.assert_delivery(&after, &finished["run"]["nodes"]["create"]["execution"]).await;
    s.unchanged_window(&after).await;
    assert_eq!(s.facts(), files);
    assert_eq!(s.herdr.calls(), calls);
}
