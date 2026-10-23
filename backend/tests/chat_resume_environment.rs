//! Approved seams: public HTTP, isolated on-disk persistence across real server
//! process restarts, and the production Herdr CLI adapter. Controlled Herdr argv
//! proves submitted configuration, not real extension loading (manual P4).
mod common;
use common::{HerdrFixture, TempDir, get_over_tcp, post_json_over_tcp, request_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, io::{BufRead, BufReader}, net::SocketAddr, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::mpsc, time::Duration};

struct Panel { child: Child, addr: SocketAddr }
impl Panel {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .args(["--config", "panel.toml"]).current_dir(root)
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(root.join("server-stderr.log")).unwrap()))
            .spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(address) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(address.trim_end_matches('/').parse::<SocketAddr>().unwrap().port());
                }
            }
        });
        let mut panel = Self { child, addr: ([127,0,0,1],0).into() };
        panel.addr.set_port(rx.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|error| {
            panic!("server did not start: {error}: {}", fs::read_to_string(root.join("server-stderr.log")).unwrap())
        }));
        panel
    }
}
impl Drop for Panel {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); }
}
fn configure(root: &Path, herdr: &HerdrFixture) {
    fs::write(root.join("panel.toml"), format!(r#"listen_addr = '127.0.0.1:0'
[operations.start]
script = 'unused.bat'
working_directory = '.'
[operations.stop]
script = 'unused.bat'
working_directory = '.'
[mdns]
enabled = false
[herdr]
workspace_name = 'bot-playground'
executable = '{}'
[zones]
registry_file = 'zones.json'
[bots]
storage_file = 'bots.json'
metadata_file = 'options.toml'
[sessions]
history_file = 'history.json'
"#, herdr.executable().display())).unwrap();
}
fn catalog(root: &Path, agent: &str, revision: &str, trigger: bool) {
    let mut text = format!("{}[[agents]]\nid = '{agent}'\nlabel = 'Agent'\n[[models]]\nid = 'default'\nlabel = 'Default'\n", if trigger { "" } else { "skills = []\n" });
    if trigger {
        text.push_str(&format!("[[models]]\nid = 'launch-model'\nlabel = 'Launch model'\nprovider_model = 'provider/launch-model'\nthinking = {{ {agent} = ['high'] }}\n[[skills]]\nid = 'launch-skill'\nlabel = 'Launch skill'\nnative_name = 'start-only'\n"));
    }
    for (id, kind) in [("fixed", "fixed"), ("a", "optional"), ("b", "optional"), ("ambient", "optional")] {
        let entry = root.join(format!("{agent}-{id}-{revision}.ts"));
        fs::write(&entry, "// Independent valid extension fixture; never execute at validation.\nexport default function () {}\n").unwrap();
        text.push_str(&format!("[[extensions]]\nid = '{id}'\nlabel = '{id}'\nagent = '{agent}'\nkind = '{kind}'\nentrypoint = '{}'\n",entry.display()));
    }
    fs::write(root.join("options.toml"), text).unwrap();
}
fn bot(agent: &str, selection: &str, name: &str) -> Value {
    json!({"name":name,"chatName":"Same name {chat.id}","agent":agent,"model":"launch-model","thinking":"high","skill":"launch-skill","prompt":"INITIAL ONLY","extensions":[selection]})
}
async fn post(panel: &Panel, path: &str, body: Value) -> Value {
    let response = post_json_over_tcp(panel.addr, path, &body.to_string()).await;
    assert!([200,202].contains(&response.status), "{path}: {} {}", response.status, response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn read(panel: &Panel, path: &str) -> Value {
    let response = get_over_tcp(panel.addr, path).await;
    assert_eq!(response.status,200,"{path}: {}",response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn create(panel: &Panel, zone: &str, bot_id: &str) -> Value {
    let preview = post(panel,"/api/chats/preview",json!({"zone":zone,"botId":bot_id})).await;
    let created = post(panel,"/api/chats",json!({"zone":zone,"botId":bot_id,"chatId":preview["chatId"],"input":"USER INPUT"})).await;
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {
            let done = read(panel,&format!("/api/chats/{}",created["id"].as_str().unwrap())).await;
            assert_ne!(done["stage"],"failed","{done}");
            if done["stage"] == "succeeded" { return done; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap()
}
async fn history(panel: &Panel, count: usize) -> Value {
    tokio::time::timeout(Duration::from_secs(8),async {
        loop {
            let rows = read(panel,"/api/sessions/history").await;
            if rows["total"] == count { return rows; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("confirmed native identity is collected through normal launch")
}
async fn collected(panel: &Panel, id: &str, name: &Value) -> Value {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let rows = read(panel, "/api/sessions/history").await;
            if let Some(row) = rows["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == id && row["sessionName"] == *name) {
                return row.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("normal confirmed creation contributes its visible name")
}
async fn register(panel: &Panel, id: &str, data: Value) {
    post(panel, "/api/sessions/events", json!({"sessionId":id,"kind":"register","data":data})).await;
}
async fn memo(panel: &Panel, id: &str, text: &str) {
    let reply = request_json_over_tcp(panel.addr,"PUT","/api/sessions/history/memo",Some(&json!({"sessionId":id,"memo":text}).to_string())).await;
    assert_eq!(reply.status,200,"{}",reply.body);
}
fn native(root: &Path, herdr: &HerdrFixture, agent: &str, id: &str) -> PathBuf {
    let file = root.join(format!("{id}.jsonl"));
    fs::write(&file,format!("{}\n",json!({"type":"session","version":3,"id":id,"cwd":root}))).unwrap();
    herdr.set_behavior(json!({"native_session":{"source":format!("herdr:{agent}"),"agent":agent,"kind":"path","value":file}}));
    file
}
// P1/P2/P3/P4/P9/P10/P18. Independent expected set: newest fixed + selected B,
// not historical A or ambient; only creation submits trigger actions. A plausible
// violation is restoring fixed-only or resolving stale trigger settings. Normal
// creation, public edit, a real process restart and external argv distinguish it.
#[tokio::test]
async fn source_survives_restart_and_resume_uses_latest_environment_without_trigger_replay() {
    for agent in ["pi","omp"] {
        let root = TempDir::new("source latest environment");
        let herdr = HerdrFixture::new("source latest environment herdr");
        configure(root.path(),&herdr);
        catalog(root.path(),agent,"old",true);
        let file = native(root.path(),&herdr,agent,"native-full-identity-001");
        let panel = Panel::start(root.path());
        let zones = post(&panel,"/api/zones",json!({"path":root.path()})).await;
        let zone = zones["zones"][0]["path"].as_str().unwrap();
        let saved = post(&panel,"/api/bots",bot(agent,"a","Source A")).await;
        let id = saved["bots"][0]["id"].as_str().unwrap();
        create(&panel,zone,id).await;
        history(&panel,1).await;
        let initial = herdr.calls();
        assert_eq!(initial.iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--model","provider/launch-model","--thinking","high","--no-extensions","--extension",root.path().join(format!("{agent}-fixed-old.ts")),"--extension",root.path().join(format!("{agent}-a-old.ts"))]));
        assert_eq!(initial.iter().find(|c| c["kind"] == "agent_prompt").unwrap()["text"],"/skill:start-only INITIAL ONLY\n\nUSER INPUT");
        let edited = request_json_over_tcp(panel.addr,"PUT",&format!("/api/bots/{id}"),Some(&bot(agent,"b","Source A renamed").to_string())).await;
        assert_eq!(edited.status,200,"{}",edited.body);
        drop(panel);
        catalog(root.path(),agent,"current",true);
        let panel = Panel::start(root.path());
        let offset = herdr.calls().len();
        let result = post(&panel,"/api/chats/resume",json!({"sessionId":"native-full-identity-001"})).await;
        assert_eq!(result["outcome"],"restored");
        let calls = herdr.calls();
        let resumed = &calls[offset..];
        assert!(!resumed.iter().any(|c| c["kind"] == "agent_prompt" || c["kind"] == "agent_input"));
        assert_eq!(resumed.iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--no-extensions","--extension",root.path().join(format!("{agent}-fixed-current.ts")),"--extension",root.path().join(format!("{agent}-b-current.ts")),if agent == "omp" {"--resume"} else {"--session"},file]));
    }
}

// P10: save valid trigger choices before removing either the model or the Skill
// from the current catalog. Resolving the whole preset would wrongly block
// restoration. The external start must still receive the selected environment;
// the same stale Bot must remain invalid for ordinary creation.
#[tokio::test]
async fn invalid_model_or_initial_skill_does_not_block_environment_restoration() {
    for agent in ["pi", "omp"] {
        for missing in ["model", "skill"] {
            let root = TempDir::new("invalid trigger restore");
            let herdr = HerdrFixture::new("invalid trigger restore herdr");
            configure(root.path(), &herdr);
            catalog(root.path(), agent, "valid", true);
            let file = native(root.path(), &herdr, agent, "native-trigger-independent");
            let panel = Panel::start(root.path());
            let zones = post(&panel,"/api/zones",json!({"path":root.path()})).await;
            let zone = zones["zones"][0]["path"].as_str().unwrap().to_owned();
            let saved = post(&panel,"/api/bots",bot(agent,"b","Source")).await;
            let id = saved["bots"][0]["id"].as_str().unwrap();
            create(&panel,&zone,id).await;
            history(&panel,1).await;
            drop(panel);
            let options = fs::read_to_string(root.path().join("options.toml")).unwrap();
            // Change only the catalog ID, leaving the saved Bot untouched.
            fs::write(root.path().join("options.toml"), options.replace(
                if missing == "model" { "id = 'launch-model'" } else { "id = 'launch-skill'" },
                "id = 'replacement-not-selected'",
            )).unwrap();
            let panel = Panel::start(root.path());
            let offset = herdr.calls().len();
            let result = post(&panel,"/api/chats/resume",json!({"sessionId":"native-trigger-independent"})).await;
            assert_eq!(result["outcome"],"restored");
            let calls = herdr.calls();
            let calls = &calls[offset..];
            assert!(!calls.iter().any(|c| c["kind"] == "agent_prompt"));
            assert_eq!(calls.iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--no-extensions","--extension",root.path().join(format!("{agent}-fixed-valid.ts")),"--extension",root.path().join(format!("{agent}-b-valid.ts")),if agent == "omp" {"--resume"} else {"--session"},file]));
            let preview = post(&panel,"/api/chats/preview",json!({"zone":zone,"botId":id})).await;
            let before = herdr.calls().len();
            let rejected = post_json_over_tcp(panel.addr,"/api/chats",&json!({"zone":zone,"botId":id,"chatId":preview["chatId"],"input":"must not start"}).to_string()).await;
            assert_eq!(rejected.status,400,"{}",rejected.body);
            assert_eq!(serde_json::from_str::<Value>(&rejected.body).unwrap()["code"],if missing == "model" { "bot-model-invalid" } else { "bot-skill-invalid" });
            assert_eq!(herdr.calls().len(),before);
        }
    }
}

// P2/P3/P9: full identities share cwd/name/prefix but remain independent. Exercise
// real launch intake and register in both orders and under an external identity
// gate plus simultaneous HTTP writes. A later Bot B, forged register provenance
// or a default update must not replace A or erase first time/memo. Only a later
// explicit launch can fill the separate source-less Chat. Resume after actual
// process restarts is the sole observation of provenance.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn intake_orders_and_concurrent_writes_preserve_first_source_time_memo_and_identity_set() {
    for order in ["register-first", "creation-first", "concurrent"] {
        let root = TempDir::new(order);
        let herdr = HerdrFixture::new("source merge herdr");
        configure(root.path(),&herdr);
        catalog(root.path(),"omp","merge",true);
        let target = "native-shared-full-prefix-001";
        let other = "native-shared-full-prefix-002";
        let file = native(root.path(),&herdr,"omp",target);
        let original_native = fs::read(&file).unwrap();
        fs::write(root.path().join("history.json"),json!([{"sessionId":other,"sessionName":"Same name","agentType":"omp","cwd":root.path(),"firstRecordedAt":123,"memo":"independent memo"}]).to_string()).unwrap();
        let panel = Panel::start(root.path());
        let zones = post(&panel,"/api/zones",json!({"path":root.path()})).await;
        let zone = zones["zones"][0]["path"].as_str().unwrap();
        let saved = post(&panel,"/api/bots",bot("omp","a","Same display name")).await;
        let source_a = saved["bots"][0]["id"].as_str().unwrap();
        let saved = post(&panel,"/api/bots",bot("omp","b","Same display name")).await;
        let source_b = saved["bots"][1]["id"].as_str().unwrap();
        let sparse = json!({"cwd":" ","sessionName":" ","agentType":"","sessionFile":"relative.jsonl","sourceBotId":source_b});
        let first_time;
        if order == "register-first" {
            register(&panel,target,json!({"cwd":root.path(),"sessionName":"Same name","sourceBotId":source_b})).await;
            let rows = history(&panel,2).await;
            first_time = rows["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == target).unwrap()["firstRecordedAt"].clone();
            memo(&panel,target,"first memo\n保留").await;
            let done = create(&panel,zone,source_a).await;
            collected(&panel,target,&done["name"]).await;
        } else if order == "creation-first" {
            let done = create(&panel,zone,source_a).await;
            let row = collected(&panel,target,&done["name"]).await;
            first_time = row["firstRecordedAt"].clone();
            memo(&panel,target,"first memo\n保留").await;
            register(&panel,target,sparse.clone()).await;
        } else {
            let release = root.path().join("release-native");
            herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"path","value":file},"native_session_gate":{"wait_for_file":release,"timeout_ms":10000}}));
            let done = create(&panel,zone,source_a).await;
            assert!(common::wait_until(Duration::from_secs(3),|| herdr.calls().iter().any(|call| call["kind"] == "agent_get")).await);
            let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(4));
            let writes = [target,other,"native-shared-full-prefix-003"].map(|id| {
                let barrier = barrier.clone();
                let addr = panel.addr;
                let data = json!({"cwd":root.path(),"sessionName":"Same name","sourceBotId":source_b});
                tokio::spawn(async move {
                    barrier.wait().await;
                    let reply = post_json_over_tcp(addr,"/api/sessions/events",&json!({"sessionId":id,"kind":"register","data":data}).to_string()).await;
                    assert_eq!(reply.status,200,"{}",reply.body);
                })
            });
            barrier.wait().await;
            fs::write(&release,b"release").unwrap();
            for write in writes { write.await.unwrap(); }
            let row = collected(&panel,target,&done["name"]).await;
            first_time = row["firstRecordedAt"].clone();
            memo(&panel,target,"first memo\n保留").await;
        }
        // A duplicate launch through B is not authority to rewrite an existing source.
        let done = create(&panel,zone,source_b).await;
        collected(&panel,target,&done["name"]).await;
        register(&panel,target,json!({"cwd":""})).await;
        for _ in 0..2 { register(&panel,target,sparse.clone()).await; }
        let before = read(&panel,"/api/sessions/history").await;
        let target_row = before["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == target).unwrap();
        assert_eq!(target_row["firstRecordedAt"],first_time);
        assert_eq!(target_row["memo"],"first memo\n保留");
        assert_eq!(target_row["cwd"],json!(root.path()));
        assert_eq!(target_row["agentType"],"omp");
        drop(panel);
        let panel = Panel::start(root.path());
        let after = read(&panel,"/api/sessions/history").await;
        assert_eq!(after,before);
        let mut ids = after["sessions"].as_array().unwrap().iter().map(|row| row["sessionId"].as_str().unwrap()).collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids,if order == "concurrent" { vec![target,other,"native-shared-full-prefix-003"] } else { vec![target,other] });
        let other_row = after["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == other).unwrap();
        assert_eq!(other_row["firstRecordedAt"],123);
        assert_eq!(other_row["memo"],"independent memo");
        let offset = herdr.calls().len();
        assert_eq!(post(&panel,"/api/chats/resume",json!({"sessionId":target})).await["outcome"],"restored");
        let calls = herdr.calls();
        assert_eq!(calls[offset..].iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--no-extensions","--extension",root.path().join("omp-fixed-merge.ts"),"--extension",root.path().join("omp-a-merge.ts"),"--resume",file]));
        assert!(!calls[offset..].iter().any(|call| call["kind"] == "agent_prompt"));
        // Same display name/cwd and even a forged source cannot grant optional B.
        let other_file = native(root.path(),&herdr,"omp",other);
        let offset = herdr.calls().len();
        assert_eq!(post(&panel,"/api/chats/resume",json!({"sessionId":other})).await["outcome"],"restored");
        assert_eq!(herdr.calls()[offset..].iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--no-extensions","--extension",root.path().join("omp-fixed-merge.ts"),"--resume",other]));
        let done = create(&panel,zone,source_a).await;
        collected(&panel,other,&done["name"]).await;
        drop(panel);
        let panel = Panel::start(root.path());
        let offset = herdr.calls().len();
        assert_eq!(post(&panel,"/api/chats/resume",json!({"sessionId":other})).await["outcome"],"restored");
        assert_eq!(herdr.calls()[offset..].iter().find(|c| c["kind"] == "agent_start").unwrap()["args"],json!(["--no-extensions","--extension",root.path().join("omp-fixed-merge.ts"),"--extension",root.path().join("omp-a-merge.ts"),"--resume",other_file]));
        let after = read(&panel,"/api/sessions/history").await;
        let row = after["sessions"].as_array().unwrap().iter().find(|row| row["sessionId"] == other).unwrap();
        assert_eq!(row["firstRecordedAt"],123);
        assert_eq!(row["memo"],"independent memo");
        assert_eq!(fs::read(&file).unwrap(),original_native);
    }
}
