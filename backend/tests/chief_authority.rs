//! S2: real backend processes and production CLI adapter; controlled Herdr only.
//! P33 oracle: definite start rejection frees the slot; timeout after external
//! start keeps an uncertain reservation, including after killing the backend.
mod common;
use common::{HerdrFixture, TempDir, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};
use std::{fs, io::{BufRead, BufReader}, net::SocketAddr, path::Path, process::{Child, Command, Stdio}, sync::mpsc, time::Duration};

struct Panel { child: Child, addr: SocketAddr, root:std::path::PathBuf, log_reader:Option<std::thread::JoinHandle<()>> }
impl Panel {
    fn start(root: &Path) -> Self {
        static NEXT_LOG:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let log_id=NEXT_LOG.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let stderr=root.join(format!("stderr-{log_id}.log"));
        let stdout_path=root.join(format!("stdout-{log_id}.log"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-bot"));
        command.args(["--config", "panel.toml"]).current_dir(root)
            .env("HOME",root).env("USERPROFILE",root).env("APPDATA",root.join("appdata"))
            .env("LOCALAPPDATA",root.join("localappdata")).env("XDG_CONFIG_HOME",root.join("xdg-config"))
            .env("XDG_DATA_HOME",root.join("xdg-data")).env("XDG_STATE_HOME",root.join("xdg-state"))
            .stdin(Stdio::null()).stdout(Stdio::piped())
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
        let child = command.spawn().unwrap();
        let mut panel = Self { child, addr: ([127,0,0,1],0).into(),root:root.into(),log_reader:None };
        let stdout = panel.child.stdout.take().unwrap();
        let (tx,rx) = mpsc::channel();
        panel.log_reader=Some(std::thread::spawn(move || {
            use std::io::Write;
            let mut log=fs::File::create(stdout_path).unwrap();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                writeln!(log,"{line}").unwrap();
                if let Some(addr) = line.strip_prefix("[herdr-bot] 监听：http://") {
                    let _ = tx.send(addr.trim_end_matches('/').parse::<SocketAddr>().unwrap());
                }
            }
        }));
        panel.addr = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|error| panic!("{error}: {}",fs::read_to_string(&stderr).unwrap()));
        eprintln!("S2 backend started pid={} address={} logs={}",panel.child.id(),panel.addr,root.display());
        panel
    }
    fn stop(&mut self) {
        let mut cleanup=Vec::new();
        if self.child.try_wait().unwrap().is_none() {
            #[cfg(windows)] {
                match Command::new("taskkill").args(["/PID",&self.child.id().to_string(),"/T","/F"]).output() {
                    Ok(output) if output.status.success()=>{},
                    output=>{
                        cleanup.push(format!("owned tree termination: {output:?}"));
                        if let Err(error)=self.child.kill() { cleanup.push(format!("backend kill: {error}")); }
                    }
                }
            }
            #[cfg(not(windows))]
            if let Err(error)=self.child.kill() { cleanup.push(format!("backend kill: {error}")); }
        }
        if let Err(error)=self.child.wait() { cleanup.push(format!("backend wait: {error}")); }
        if let Some(reader)=self.log_reader.take() {
            if reader.join().is_err() { cleanup.push("stdout drain failed".into()); }
            eprintln!("S2 backend exited pid={} cleanup={cleanup:?}",self.child.id());
        }
        if std::thread::panicking() || !cleanup.is_empty() {
            for entry in fs::read_dir(&self.root).unwrap().filter_map(Result::ok) {
                if entry.path().extension().is_some_and(|extension|extension=="log") {
                    eprintln!("{}: {}",entry.path().display(),fs::read_to_string(entry.path()).unwrap_or_default());
                }
            }
            eprintln!("cleanup errors: {cleanup:?}");
        }
        if !std::thread::panicking() { assert!(cleanup.is_empty(),"{cleanup:?}"); }
    }
}
impl Drop for Panel { fn drop(&mut self) { self.stop(); } }
fn configure(root: &Path, herdr: &HerdrFixture) {
    fs::write(root.join("panel.toml"),format!(r#"listen_addr = '127.0.0.1:0'
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
[tasks]
storage_file = 'tasks.json'
metadata_file = 'options.toml'
[sessions]
history_file = 'history.json'
"#,herdr.executable().display())).unwrap();
    fs::write(root.join("fixed.ts"),"export default function () {}\n").unwrap();
    fs::write(root.join("options.toml"),"skills = []\n[[agents]]\nid = 'pi'\nlabel = 'Pi'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'fixed'\nlabel = 'Fixed'\nagent = 'pi'\nkind = 'fixed'\nentrypoint = 'fixed.ts'\n").unwrap();
}
fn value(response: common::HttpResponse) -> Value {
    assert!([200,202].contains(&response.status),"{} {}",response.status,response.body);
    serde_json::from_str(&response.body).unwrap()
}
async fn post(panel: &Panel, path: &str, body: Value) -> Value {
    value(post_json_over_tcp(panel.addr,path,&body.to_string()).await)
}
async fn current(panel: &Panel) -> Value { value(get_over_tcp(panel.addr,"/api/chief").await)["current"].clone() }
async fn setup(panel: &Panel, root: &Path, name: &str) -> (String,String) {
    let zone = root.join(name); fs::create_dir(&zone).unwrap();
    post(panel,"/api/zones",json!({"id":name,"path":zone})).await;
    let task = post(panel,"/api/tasks",json!({"name":name,"taskType":"chief","chatName":"chief {chat.id}","agent":"pi","model":"default","prompt":"initial"})).await;
    let id = task["tasks"].as_array().unwrap().iter().find(|task|task["name"]==name).unwrap()["id"].as_str().unwrap().to_owned();
    (name.into(),id)
}
async fn launch(panel: &Panel, zone: &str, task: &str) -> common::HttpResponse {
    let preview = post(panel,"/api/chats/preview",json!({"zone":zone,"taskId":task})).await;
    post_json_over_tcp(panel.addr,"/api/chats",&json!({"zone":zone,"taskId":task,"chatId":preview["chatId"],"input":"hello"}).to_string()).await
}
async fn finished(panel: &Panel, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(20),async { loop {
        let result = value(get_over_tcp(panel.addr,&format!("/api/chats/{id}")).await);
        if ["failed","succeeded"].contains(&result["stage"].as_str().unwrap()) { return result; }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }}).await.unwrap()
}

// P32 oracle: backend restart clears registrations, not trusted Chat provenance.
// A plausible violation reserves only per Task or only after external navigation.
// Hold that navigation at the external CLI while different live/recovering
// callers compete, then inspect public authority and authorization results.
#[tokio::test(flavor="multi_thread",worker_threads=4)]
async fn p32_real_restart_serializes_live_registration_and_resume_before_external_completion() {
    let root = TempDir::new("chief P32"); let herdr = HerdrFixture::new("chief P32 CLI");
    configure(root.path(),&herdr); let mut panel = Panel::start(root.path());
    let (zone_a,task_a) = setup(&panel,root.path(),"alpha").await;
    let (zone_b,task_b) = setup(&panel,root.path(),"beta").await;
    for (id,zone,task) in [("native-a",&zone_a,&task_a),("native-b",&zone_b,&task_b)] {
        let native = root.path().join(format!("{id}.jsonl"));
        fs::write(&native,format!("{}\n",json!({"type":"session","version":3,"id":id,"cwd":root.path().join(zone)}))).unwrap();
        herdr.set_behavior(json!({"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":native}}));
        let created = value(launch(&panel,zone,task).await);
        assert_eq!(finished(&panel,created["id"].as_str().unwrap()).await["stage"],"succeeded");
        assert_eq!(current(&panel).await["sessionId"],id,"successful chief launch binds the actual native identity");
        post(&panel,"/api/sessions/events",json!({"sessionId":id,"kind":"register","data":{"cwd":root.path().join(zone),"agentType":"pi"}})).await;
        assert_eq!(current(&panel).await["phase"],"registered");
        if id == "native-a" {
            post(&panel,"/api/sessions/events",json!({"sessionId":id,"kind":"unregister"})).await;
            assert_eq!(current(&panel).await,Value::Null);
        }
    }
    panel.stop();
    let mut panel = Panel::start(root.path());
    assert_eq!(current(&panel).await,Value::Null,"registrations are transient across restart");
    let gate = root.path().join("release-navigation");
    let agents = json!([{"agent":"pi","name":"live-a","workspace_id":"w-target","tab_id":"a","pane_id":"a:0","agent_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-a"}},
        {"agent":"pi","name":"live-b","workspace_id":"w-target","tab_id":"b","pane_id":"b:0","agent_session":{"source":"herdr:pi","agent":"pi","kind":"id","value":"native-b"}}]);
    herdr.set_behavior(json!({"agents":agents,"agent_list_gate":{"wait_for_file":gate,"timeout_ms":10000}}));
    let before = herdr.calls().len();
    let addr = panel.addr;
    let resume = tokio::spawn(async move { post_json_over_tcp(addr,"/api/chats/resume",&json!({"sessionId":"native-a"}).to_string()).await });
    let entered = common::wait_until(Duration::from_secs(5),||herdr.calls()[before..].iter().any(|call|call["kind"]=="agent_list")).await;
    // Always release the owned CLI before asserting, even when the product fails.
    let competing = if entered {
        let state = current(&panel).await;
        let register_body = json!({"sessionId":"native-b","kind":"register","data":{"cwd":root.path(),"agentType":"pi"}}).to_string();
        let restore_body = json!({"sessionId":"native-b"}).to_string();
        let register = post_json_over_tcp(addr,"/api/sessions/events",&register_body);
        let restore = post_json_over_tcp(addr,"/api/chats/resume",&restore_body);
        let (register,restore) = tokio::join!(register,restore);
        Some((state,register,restore))
    } else { None };
    fs::write(&gate,"release").unwrap();
    let resumed = tokio::time::timeout(Duration::from_secs(15),resume).await.unwrap().unwrap();
    assert!(entered,"resume must reach external navigation");
    let (state,register,restore) = competing.unwrap();
    assert_eq!(state["sessionId"],"native-a");
    assert_eq!(register.status,409,"{}",register.body);
    assert_eq!(restore.status,409,"{}",restore.body);
    assert_eq!(value(resumed)["outcome"],"located");
    post(&panel,"/api/sessions/events",json!({"sessionId":"native-a","kind":"register","data":{"cwd":root.path(),"agentType":"pi"}})).await;
    assert_eq!(current(&panel).await["sessionId"],"native-a");
    let accepted = post(&panel,"/api/chief/qualify",json!({"sessionId":"native-a"})).await;
    assert_eq!(accepted["current"]["sessionId"],"native-a");
    let denied = post_json_over_tcp(addr,"/api/chief/qualify",&json!({"sessionId":"native-b"}).to_string()).await;
    assert_eq!(denied.status,409,"{}",denied.body);
    panel.stop();
    let mut panel = Panel::start(root.path());
    let old = post_json_over_tcp(panel.addr,"/api/sessions/events",&json!({"sessionId":"native-b","kind":"register","data":{"cwd":root.path(),"agentType":"pi"}}).to_string()).await;
    assert_eq!(old.status,409,"backend restart must not restore a superseded identity: {}",old.body);
    post(&panel,"/api/sessions/events",json!({"sessionId":"native-a","kind":"register","data":{"cwd":root.path(),"agentType":"pi"}})).await;
    assert_eq!(current(&panel).await["sessionId"],"native-a");
    panel.stop();
    // Development lifecycle regression through the same S2 interface. Manual
    // time uses the actual configured 10s online / 30s retention policy; this
    // is not the cross-browser P30 acceptance owned by ticket 07.
    let config=herdr_bot::Config::load(&root.path().join("panel.toml")).unwrap();
    let clock=std::sync::Arc::new(herdr_bot::clock::ManualClock::new());
    let sessions=std::sync::Arc::new(herdr_bot::sessions::SessionsState::with_policy_and_clock(config.sessions.policy(),clock.clone()));
    struct NeverPowerOff;
    impl herdr_bot::PowerOff for NeverPowerOff {
        fn power_off(&self)->Result<(),herdr_bot::PowerOffFailure> { panic!("chief checks must not shut down"); }
    }
    let server=common::TestServer::start_app(herdr_bot::app_with_sessions(&config,std::sync::Arc::new(NeverPowerOff),sessions.clone())).await;
    value(post_json_over_tcp(server.addr(),"/api/sessions/events",&json!({"sessionId":"native-a","kind":"register","data":{"cwd":root.path(),"agentType":"pi"}}).to_string()).await);
    clock.advance(Duration::from_secs(11)); sessions.scan_expired().await;
    let retained=value(get_over_tcp(server.addr(),"/api/chief").await);
    assert_eq!(retained["current"]["sessionId"],"native-a","offline membership retains authority");
    let occupied=post_json_over_tcp(server.addr(),"/api/chats/resume",&json!({"sessionId":"native-b"}).to_string()).await;
    assert_eq!(occupied.status,409,"{}",occupied.body);
    clock.advance(Duration::from_secs(30)); sessions.scan_expired().await;
    assert_eq!(value(get_over_tcp(server.addr(),"/api/chief").await)["current"],Value::Null);
    let stale=post_json_over_tcp(server.addr(),"/api/chief/qualify",&json!({"sessionId":"native-a"}).to_string()).await;
    assert_eq!(stale.status,409,"removed membership revokes subsequent consumer authorization");
    let prompts_before=herdr.calls().iter().filter(|call|call["kind"]=="agent_prompt").count();
    herdr.set_behavior(json!({"agents":[],"native_session":{"source":"herdr:pi","agent":"pi","kind":"path","value":root.path().join("native-b.jsonl")}}));
    let restored=value(post_json_over_tcp(server.addr(),"/api/chats/resume",&json!({"sessionId":"native-b"}).to_string()).await);
    assert_eq!(restored["outcome"],"restored");
    let awaiting=value(get_over_tcp(server.addr(),"/api/chief").await);
    assert_eq!(awaiting["current"]["phase"],"awaiting-registration");
    let not_registered=post_json_over_tcp(server.addr(),"/api/chief/qualify",&json!({"sessionId":"native-b"}).to_string()).await;
    assert_eq!(not_registered.status,409);
    value(post_json_over_tcp(server.addr(),"/api/sessions/events",&json!({"sessionId":"native-b","kind":"register","data":{"cwd":root.path().join("beta"),"agentType":"pi"}}).to_string()).await);
    assert_eq!(value(post_json_over_tcp(server.addr(),"/api/chief/qualify",&json!({"sessionId":"native-b"}).to_string()).await)["current"]["sessionId"],"native-b");
    assert_eq!(herdr.calls().iter().filter(|call|call["kind"]=="agent_prompt").count(),prompts_before,"restoration does not replay initial input");
}

#[tokio::test]
async fn p33_definite_start_failure_frees_slot_but_uncertain_external_start_survives_restart() {
    let root = TempDir::new("chief P33"); let herdr = HerdrFixture::new("chief P33 CLI");
    configure(root.path(),&herdr); let mut panel = Panel::start(root.path());
    let (zone,task) = setup(&panel,root.path(),"first").await;
    herdr.set_behavior(json!({"fail_agent_start":true}));
    let created = value(launch(&panel,&zone,&task).await);
    let failed = finished(&panel,created["id"].as_str().unwrap()).await;
    assert_eq!(failed["failure"]["code"],"herdr-command-failed");
    assert_eq!(current(&panel).await,Value::Null,"definite rejection must release");
    herdr.set_behavior(json!({"timeout_agent_start":true}));
    let retried = value(launch(&panel,&zone,&task).await);
    let failed = finished(&panel,retried["id"].as_str().unwrap()).await;
    assert_eq!(failed["failure"]["code"],"herdr-timeout");
    let uncertain = current(&panel).await;
    assert_eq!(uncertain["phase"],"uncertain");
    let starts = herdr.calls().iter().filter(|call|call["kind"]=="agent_start").count();
    panel.stop(); let panel = Panel::start(root.path());
    assert_eq!(current(&panel).await,uncertain);
    let rejected = launch(&panel,&zone,&task).await;
    assert_eq!(rejected.status,409,"{}",rejected.body);
    assert_eq!(serde_json::from_str::<Value>(&rejected.body).unwrap()["code"],"chief-occupied");
    assert_eq!(herdr.calls().iter().filter(|call|call["kind"]=="agent_start").count(),starts,"no second external start");
    let stale = post_json_over_tcp(panel.addr,"/api/chief/release",&json!({"claimId":"stale-claim"}).to_string()).await;
    assert_eq!(stale.status,409,"stale browser must not revoke a replacement: {}",stale.body);
    let released = post(&panel,"/api/chief/release",json!({"claimId":uncertain["claimId"]})).await;
    assert_eq!(released["released"],uncertain,"explicit revocation must retain and report the uncertain fact");
    assert_eq!(current(&panel).await,Value::Null);
    herdr.set_behavior(json!({"fail_agent_start":true}));
    let next = value(launch(&panel,&zone,&task).await);
    assert_eq!(finished(&panel,next["id"].as_str().unwrap()).await["failure"]["code"],"herdr-command-failed");
}
