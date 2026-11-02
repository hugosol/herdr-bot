//! S1/S3: independent legacy records plus actual backend process restart.
use super::*;
use std::{io::{BufRead, BufReader}, net::SocketAddr, process::{Child, Command, Stdio}, sync::mpsc};

struct Panel {
    child: Child,
    addr: SocketAddr,
    reader: Option<std::thread::JoinHandle<()>>,
}
impl Panel {
    fn start(s: &Scenario, generation: u8) -> Self {
        let stderr = s.data.path().join(format!("restart-{generation}-stderr.log"));
        let stdout_log = s.data.path().join(format!("restart-{generation}-stdout.log"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .args(["--config", "report-panel.toml"]).current_dir(s.data.path())
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap())).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            use std::io::Write;
            let mut log = fs::File::create(stdout_log).unwrap();
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                writeln!(log, "{line}").unwrap();
                if let Some(address) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(address.trim_end_matches('/').parse::<SocketAddr>().unwrap());
                }
            }
        });
        let mut panel = Self { child, addr: ([127,0,0,1],0).into(), reader: Some(reader) };
        panel.addr = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|error| panic!("backend startup: {error}: {}", fs::read_to_string(stderr).unwrap()));
        panel
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        self.child.wait().expect("reap owned backend process");
        self.reader.take().unwrap().join().expect("drain backend stdout");
    }
}

// P25/P26: independently authored old fields must read as absent, not true.
// A restarted backend that reclaims saved associations would accept the old
// manager/node calls below. New report fields must survive real process exits.
#[tokio::test]
async fn historical_and_new_reports_survive_actual_restart_without_reclaiming_runs() {
    let s = Scenario::new().await;
    let started = s.start("restart-report-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"restart-report-node"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("restart-report-manager", &["A"]).await;
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"restart-report-node","body":"Saved before actual restart","canComplete":false,"outputs":[{"id":"result","path":"C:\\restart\\result.txt"}]}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let new_report = serde_json::from_str::<Value>(&response.body).unwrap()["report"].clone();
    let legacy_manager = json!({"sessionId":"legacy-manager","workspaceId":"old-workspace","tabId":"old-manager-tab","paneId":"old-manager-pane","agentName":"old-manager-agent"});
    let legacy_node = json!({"sessionId":"legacy-node","workspaceId":"old-workspace","tabId":"old-node-tab","paneId":"old-node-pane","agentName":"old-node-agent"});
    let legacy_run = json!({"id":"legacy-reports","map":"retired.json","createdAt":100,"managerTaskId":"old-manager-task","zone":"C:/legacy/manager","workspaceName":"old-workspace","userInput":"Historical input","manager":legacy_manager,"status":"running","finished":null,"nodes":{"A":{"status":"running","execution":{"id":"old-execution","taskId":"old-node-task","agent":"omp","zone":"C:/legacy/node","chat":legacy_node},"failure":null,"completion":null,"previousExecutions":[]}}});
    let legacy_report = json!({"id":"old-report","type":"report","at":120,"runId":"legacy-reports","nodeId":"A","executionId":"old-execution","from":legacy_node,"to":legacy_manager,"body":"Historical report without completion declaration","outputs":[]});
    let legacy_root = s.config.scheduler.run_directory.join("legacy-reports");
    fs::create_dir(&legacy_root).unwrap();
    fs::write(legacy_root.join("run.json"), legacy_run.to_string()).unwrap();
    fs::write(legacy_root.join("history.jsonl"), b"").unwrap();
    fs::write(legacy_root.join("messages.jsonl"), format!("{legacy_report}\n")).unwrap();
    fs::write(s.data.path().join("report-panel.toml"), format!(r#"listen_addr = '127.0.0.1:0'
data_directory = 'restart-data'
static_directory = 'unused-static'
[operations.start]
script = 'unused.bat'
working_directory = '.'
[operations.stop]
script = 'unused.bat'
working_directory = '.'
[mdns]
enabled = false
[herdr]
workspace_name = 'report-restart'
executable = '{}'
[zones]
registry_file = '{}'
[tasks]
storage_file = '{}'
metadata_file = '{}'
[sessions]
history_file = 'restart-session-history.json'
[scheduler]
map_directory = '{}'
bot_directory = '{}'
run_directory = '{}'
run_instructions = '{}'
"#, s.herdr.executable().display(), s.config.zones.registry_file.display(), s.config.tasks.storage_file.display(), s.config.tasks.metadata_file.display(), s.config.scheduler.map_directory.display(), s.config.scheduler.bot_directory.display(), s.config.scheduler.run_directory.display(), s.config.scheduler.run_instructions.display())).unwrap();
    let new_root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let before = [&legacy_root, &new_root].map(|root| ["run.json","history.jsonl","messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()));
    let calls = s.herdr.calls();
    let mut observations = Vec::new();
    for generation in [1, 2] {
        let panel = Panel::start(&s, generation);
        let mut reads = Vec::new();
        for (id, expected) in [("legacy-reports", legacy_report.clone()), (started["id"].as_str().unwrap(), new_report.clone())] {
            let response = get_over_tcp(panel.addr, &format!("/api/workflow/runs/{id}")).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let detail: Value = serde_json::from_str(&response.body).unwrap();
            assert_eq!(detail["managed"], false);
            assert_eq!(detail["reports"], json!([expected]));
            assert_eq!(detail["run"]["nodes"]["A"]["status"], "running");
            assert_eq!(detail["pendingReports"]["A"], expected["id"]);
            reads.push(detail);
        }
        assert!(reads[0]["reports"][0].get("canComplete").is_none());
        assert_eq!(reads[0]["reports"][0]["outputs"], json!([]));
        for identity in ["legacy-manager", "restart-report-manager"] {
            for action in ["status", "collect", "resume", "dispatch"] {
                let mut body = json!({"sessionId":identity});
                if action == "dispatch" { body["nodes"] = json!(["A"]); }
                let denied = post_json_over_tcp(panel.addr, &format!("/api/workflow/manager/{action}"), &body.to_string()).await;
                assert_eq!(denied.status, 403, "{}", denied.body);
            }
        }
        for identity in ["legacy-node", "restart-report-node"] {
            let denied = post_json_over_tcp(panel.addr, "/api/workflow/node/report", &json!({"sessionId":identity,"body":"No restart write qualification","canComplete":true,"outputs":[{"id":"result","path":"C:\\must-not-register.txt"}]}).to_string()).await;
            assert_eq!(denied.status, 403, "{}", denied.body);
        }
        observations.push(reads);
        drop(panel); // waits for the actual OS process to exit before restarting
    }
    assert_eq!(observations[0], observations[1]);
    assert_eq!([&legacy_root, &new_root].map(|root| ["run.json","history.jsonl","messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap())), before);
    assert_eq!(s.herdr.calls(), calls);
    println!("P25 actual process restart legacy and new report reads={}", json!(observations));
}
