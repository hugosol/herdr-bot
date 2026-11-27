//! Public Map dispatch and actual Bun files: no built-in capability registration.
use super::*;

#[tokio::test]
async fn typescript_collect_observes_live_output_without_consuming_or_owning_execution() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const MANAGER: &str = "typescript-output-manager";
    const LOGS: &str = "/api/workflow/manager/script-output";
    const COLLECT: &str = "/api/workflow/manager/collect";
    let s = Scenario::new().await;
    bind(&s, r#"import {join} from 'node:path';
export default async function(context) {
    const gate = async (name) => {
        const deadline = Date.now() + 15000;
        while (!(await Bun.file(join(context.zone, name)).exists()) && !(await Bun.file(join(context.zone, 'release-all')).exists())) {
            if (Date.now() >= deadline) throw Error('fixture gate expired: ' + name);
            await Bun.sleep(10);
        }
    };
    const stdout = data => new Promise((resolve, reject) => process.stdout.write(data, error => error ? reject(error) : resolve()));
    await stdout('gone\r\none\r\ntwo\nthree\nfour\n\x1b[31mpartial\x1b[0m\x07 ');
    await stdout(Buffer.from([0xe7, 0x95]));
    await gate('next');
    await stdout(Buffer.from([0x8c]));
    await stdout(' end\r\n' + 'X'.repeat(2048) + '\nstdout-inflight');
    const host = await context.host('parse_json', {text:'{"privateProtocol":"not a log"}'});
    if (host.privateProtocol !== 'not a log') throw Error('host call did not complete');
    console.log('console visible');
    process.stderr.write('stderr-inflight');
    await gate('finish');
    await new Promise((resolve, reject) => process.stderr.write(' continued\r\n', error => error ? reject(error) : resolve()));
    return {status:'success'};
}"#, json!([])).await;
    let zone = s.data.path().join("node-zone");
    struct Release(std::path::PathBuf);
    impl Drop for Release { fn drop(&mut self) { let _ = fs::write(&self.0, "release owned Bun"); } }
    let _release = Release(zone.join("release-all"));
    let run = s.start(MANAGER).await;
    let other = s.start("different-output-manager").await;
    let dispatch = s.dispatch(MANAGER, &["custom"]).await;
    let id = &dispatch["snapshot"]["nodes"]["custom"]["execution"]["id"];
    let identity = json!({"sessionId": MANAGER}).to_string();
    let snapshot = async |await_line: &str| {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let response = post_json_over_tcp(s.server.addr(), LOGS, &identity).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let output: Value = serde_json::from_str(&response.body).unwrap();
            if output["nodes"][0]["lines"].as_array().is_some_and(|lines| lines.iter().any(|line| line == await_line)) {
                break output;
            }
            assert!(tokio::time::Instant::now() < deadline, "did not observe {await_line:?}: {output}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let first = snapshot("partial ").await;
    assert_eq!(first["runId"], run["id"]);
    let node = &first["nodes"][0];
    assert_eq!(node["nodeId"], "custom");
    assert_eq!(node["nodeName"], "Run user code");
    assert_eq!(node["executionId"], *id);
    assert_eq!(node["status"], "running");
    assert_eq!(node["lines"], json!(["one", "two", "three", "four", "partial "]));
    assert!(node["lastOutputAt"].as_u64().unwrap() <= first["observedAt"].as_u64().unwrap());
    let denied = post_json_over_tcp(s.server.addr(), LOGS, r#"{"sessionId":"unassociated"}"#).await;
    assert_eq!(denied.status, 403, "{}", denied.body);
    let selector = post_json_over_tcp(s.server.addr(), LOGS, &json!({"sessionId":MANAGER,"runId":other["id"]}).to_string()).await;
    assert_eq!(selector.status, 400, "{}", selector.body);
    let other_output = post_json_over_tcp(s.server.addr(), LOGS, r#"{"sessionId":"different-output-manager"}"#).await;
    assert_eq!(other_output.status, 200, "{}", other_output.body);
    let other_output: Value = serde_json::from_str(&other_output.body).unwrap();
    assert_eq!(other_output["runId"], other["id"]);
    assert_eq!(other_output["nodes"], json!([]));

    // Wait for the real collect response headers before dropping its TCP caller.
    let mut caller = tokio::net::TcpStream::connect(s.server.addr()).await.unwrap();
    caller.write_all(format!("POST {COLLECT} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{identity}", s.server.addr(), identity.len()).as_bytes()).await.unwrap();
    let mut headers = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !headers.ends_with(b"\r\n\r\n") { headers.push(caller.read_u8().await.unwrap()); }
    }).await.unwrap();
    assert!(headers.starts_with(b"HTTP/1.1 200"));
    fs::write(zone.join("next"), "release next chunk").unwrap();
    let live = snapshot("stderr-inflight").await;
    let lines = live["nodes"][0]["lines"].as_array().unwrap();
    assert_eq!(live["nodes"][0]["status"], "running");
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], "partial 界 end");
    let long = lines[1].as_str().unwrap();
    assert!(long.starts_with("XXXX") && long.len() <= 512, "unbounded or missing line: {long}");
    assert_eq!(lines[2..], [json!("stdout-inflight"), json!("console visible"), json!("stderr-inflight")]);
    drop(caller);
    fs::write(zone.join("finish"), "release first execution").unwrap();
    let done = ended(&s, &run).await;
    assert_eq!(done["run"]["nodes"]["custom"]["execution"]["lifecycle"]["status"], "succeeded");
    let retained = snapshot("stderr-inflight continued").await;
    assert_eq!(retained["nodes"][0]["status"], "succeeded");
    assert_eq!(retained["nodes"][0]["lines"][2], "stdout-inflight");
    let collected = post_json_over_tcp(s.server.addr(), COLLECT, &identity).await;
    assert_eq!(collected.status, 200, "{}", collected.body);
    let collected: Value = serde_json::from_str(&collected.body).unwrap();
    assert_eq!(collected["reason"], "notification");
    assert!(!collected.to_string().contains("stderr-inflight"));

    let path = s.config.scheduler.bot_directory.parent().unwrap().join("scripts/user/custom.ts");
    fs::write(path, r#"export default async ({zone}) => {
        process.stdout.write('retry only');
        const deadline = Date.now() + 15000;
        while (!(await Bun.file(zone + '/retry-finish').exists()) && !(await Bun.file(zone + '/release-all').exists())) {
            if (Date.now() >= deadline) throw Error('fixture retry gate expired');
            await Bun.sleep(10);
        }
        return {status:'success'};
    };"#).unwrap();
    let fail = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/fail", &json!({"sessionId":MANAGER,"nodeId":"custom","reason":"Inspect and repeat"}).to_string()).await;
    assert_eq!(fail.status, 200, "{}", fail.body);
    let retry = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/retry", &json!({"sessionId":MANAGER,"nodeId":"custom","reason":"Old effects inspected","confirmed":true}).to_string()).await;
    assert_eq!(retry.status, 200, "{}", retry.body);
    let current = snapshot("retry only").await;
    assert_eq!(current["nodes"].as_array().unwrap().len(), 1);
    assert_ne!(current["nodes"][0]["executionId"], *id);
    assert_eq!(current["nodes"][0]["status"], "running");
    assert_eq!(current["nodes"][0]["lines"], json!(["retry only"]));
    fs::write(zone.join("retry-finish"), "release retry").unwrap();
    ended_count(&s, &run, 2).await;
    let complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":MANAGER,"nodeId":"custom","reason":"Read actual result"}).to_string()).await;
    assert_eq!(complete.status, 200, "{}", complete.body);
    let finish = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &identity).await;
    assert_eq!(finish.status, 200, "{}", finish.body);
    let gone = post_json_over_tcp(s.server.addr(), LOGS, &identity).await;
    assert_eq!(gone.status, 200, "{}", gone.body);
    assert_eq!(serde_json::from_str::<Value>(&gone.body).unwrap()["nodes"], json!([]));
}

// P15: keep an approved execution and its real output, then hold a new Bun
// execution before success. Inheriting either approval or old outputs would
// incorrectly allow complete while this execution is still at the file gate.
#[tokio::test]
async fn typescript_retry_starts_unapproved_until_this_execution_succeeds() {
    let s = Scenario::new().await;
    bind(&s, r#"import {join} from 'node:path';
export default async function(context) {
    await Bun.write(join(context.zone, 'entered-'+context.executionId), 'entered');
    while (!(await Bun.file(join(context.zone, 'release')).exists())) await Bun.sleep(10);
    const path = join(context.zone, 'result-'+context.executionId+'.txt');
    await Bun.write(path, context.executionId);
    return {status:'success',outputs:[{id:'result',path}]};
}"#, json!([{"id":"result"}])).await;
    let release = s.data.path().join("node-zone/release");
    fs::write(&release, "first execution may finish").unwrap();
    let run = s.start("typescript-retry-manager").await;
    s.dispatch("typescript-retry-manager", &["custom"]).await;
    let first = ended(&s, &run).await;
    let old = &first["run"]["nodes"]["custom"]["execution"];
    assert_eq!(old["approved"], true);
    let old_path = first["reports"][0]["outputs"][0]["path"].as_str().unwrap();
    let old_bytes = fs::read(old_path).unwrap();
    fs::remove_file(&release).unwrap();
    struct Release(std::path::PathBuf);
    impl Drop for Release { fn drop(&mut self) { let _ = fs::write(&self.0, "release owned Bun"); } }
    let _release = Release(release.clone());
    let failed = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/fail", &json!({"sessionId":"typescript-retry-manager","nodeId":"custom","reason":"Inspect and repeat independent work"}).to_string()).await;
    assert_eq!(failed.status, 200, "{}", failed.body);
    let retried = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/retry", &json!({"sessionId":"typescript-retry-manager","nodeId":"custom","reason":"Old effects inspected","confirmed":true}).to_string()).await;
    assert_eq!(retried.status, 200, "{}", retried.body);
    let retry: Value = serde_json::from_str(&retried.body).unwrap();
    let current = &retry["snapshot"]["nodes"]["custom"]["execution"];
    assert_ne!(current["id"], old["id"]);
    assert_eq!(current["approved"], false);
    assert!(current["approvalSource"].is_null());
    assert_eq!(retry["snapshot"]["nodes"]["custom"]["previousExecutions"][0]["execution"], *old);
    let entered = s.data.path().join("node-zone").join(format!("entered-{}", current["id"].as_str().unwrap()));
    assert!(common::wait_until(Duration::from_secs(5), || entered.is_file()).await, "real Bun must reach the new execution gate");
    let status = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
    let status: Value = serde_json::from_str(&status.body).unwrap();
    assert_eq!(status["run"]["nodes"]["custom"]["execution"]["approved"], false);
    assert_eq!(status["reports"], first["reports"]);
    let denied = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"typescript-retry-manager","nodeId":"custom","reason":"Old success must not authorize this execution"}).to_string()).await;
    // Always release owned Bun before assertions can unwind the fixture.
    fs::write(&release, "new execution may finish").unwrap();
    let done = ended_count(&s, &run, 2).await;
    assert_eq!(denied.status, 409, "{}", denied.body);
    let current_done = &done["run"]["nodes"]["custom"]["execution"];
    assert_eq!(current_done["id"], current["id"]);
    assert_eq!(current_done["approved"], true);
    assert_eq!(current_done["approvalSource"], "auto");
    assert_eq!(done["reports"][0], first["reports"][0]);
    assert_eq!(done["reports"][1]["executionId"], current["id"]);
    assert_ne!(done["reports"][1]["outputs"][0]["path"], old_path);
    assert_eq!(fs::read_to_string(done["reports"][1]["outputs"][0]["path"].as_str().unwrap()).unwrap(), current["id"].as_str().unwrap());
    assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    println!("P15 run={} old={} new={} reports={}", run["id"], old, current_done, done["reports"]);
}

// P25: public save must reject false, not normalize it. A direct-file false
// configuration must also fail preflight before any external launch.
#[tokio::test]
async fn typescript_definition_rejects_manual_approval_and_round_trips_auto() {
    let s = Scenario::new().await;
    bind(&s, "export default async () => ({status:'success'});", json!([])).await;
    let route = "/api/scheduler/maps/work.json/nodes";
    let edited = post_json_over_tcp(s.server.addr(), &format!("{route}/edit"), &json!({"id":"custom"}).to_string()).await;
    assert_eq!(edited.status, 200, "{}", edited.body);
    let draft: Value = serde_json::from_str(&edited.body).unwrap();
    let original = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    let mut node = draft["content"].clone();
    node["auto_approved"] = json!(false);
    let rejected = post_json_over_tcp(s.server.addr(), &format!("{route}/save"), &json!({"id":"custom","revision":draft["revision"],"content":node.to_string(),"create":false}).to_string()).await;
    // A valid boolean that conflicts with the script target is a 409,
    // unlike malformed node JSON. Either way the original must stay intact.
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    assert_eq!(serde_json::from_str::<Value>(&rejected.body).unwrap()["code"], "script-policy-invalid");
    assert_eq!(get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await.body, original.body);
    node["auto_approved"] = json!(true);
    let saved = post_json_over_tcp(s.server.addr(), &format!("{route}/save"), &json!({"id":"custom","revision":draft["revision"],"content":node.to_string(),"create":false}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    let mut map: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(map["nodes"]["custom"]["auto_approved"], true);
    map["nodes"]["custom"]["auto_approved"] = json!(false);
    fs::write(s.config.scheduler.map_directory.join("work.json"), map.to_string()).unwrap();
    let calls = s.herdr.calls();
    for suffix in ["preflight", "runs"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/work.json/{suffix}"), &json!({"managerTaskId":s.manager_task,"zone":s.zone,"userInput":""}).to_string()).await;
        assert_eq!(response.status, 400, "{}", response.body);
        assert!(serde_json::from_str::<Value>(&response.body).unwrap()["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "script-policy-invalid"));
    }
    assert_eq!(s.herdr.calls(), calls);
}

async fn bind(s: &Scenario, source: &str, outputs: Value) {
    let directory = s.config.scheduler.bot_directory.parent().unwrap().join("scripts").join("user");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("helper.ts"), "export const suffix: string = ' / imported';\n").unwrap();
    fs::write(directory.join("custom.ts"), source).unwrap();
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"user-script","revision":null,"content":json!({"name":"User script","when":"","description":"","input_rule":"","output_rule":"","script":"user/custom.ts"}).to_string()}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"User TS","description":"","nodes":{
        "custom":{"title":"Run user code","bot_id":"user-script","input":"literal input 第二行","zone":"node","blocked_by":[],"output_rule":outputs}
    }}).to_string()).unwrap();
}

async fn ended(s: &Scenario, run: &Value) -> Value { ended_count(s, run, 1).await }

async fn ended_count(s: &Scenario, run: &Value, count: usize) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let detail: Value = serde_json::from_str(&response.body).unwrap();
        if detail["reports"].as_array().unwrap().len() == count {
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.contains(detail["reports"][count - 1]["id"].as_str().unwrap())))
                    && fs::OpenOptions::new().read(true).write(true).share_mode(0).open(s.herdr.executable()).is_ok()
            }).await, "notification process must exit before fixture cleanup");
            return detail;
        }
        assert!(tokio::time::Instant::now() < deadline, "Bun result did not arrive: {detail}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn typescript_dynamic_zone_uses_selected_root_without_chat_or_binding_mutation() {
    let s = Scenario::new().await;
    bind(&s, r#"import {join, resolve} from 'node:path';
export default async function(context) {
    if (process.cwd() !== resolve(context.zone)) throw new Error('wrong dynamic cwd');
    const path = join(context.zone, 'dynamic.txt');
    await Bun.write(path, 'dynamic execution');
    return {status:'success', outputs:[{id:'result',path}]};
}"#, json!([{"id":"result"}])).await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    map["nodes"]["custom"]["zone"] = json!({"type":"dynamic"});
    fs::write(path, map.to_string()).unwrap();
    let run = s.start("typescript-dynamic-manager").await;
    let root = s.data.path().join("dynamic-script-root");
    fs::create_dir(&root).unwrap();
    let registration = post_json_over_tcp(s.server.addr(), "/api/zones", &json!({"id":"script-root","path":root}).to_string()).await;
    assert_eq!(registration.status, 200, "{}", registration.body);
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"typescript-dynamic-manager","nodes":["custom"],"zoneIds":{"custom":"script-root"}}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let result = ended(&s, &run).await;
    let execution = &result["run"]["nodes"]["custom"]["execution"];
    assert_eq!(execution["zoneId"], "script-root");
    assert_eq!(execution["zone"], json!(root));
    assert_eq!(execution["lifecycle"]["status"], "succeeded");
    assert!(execution.get("chat").is_none());
    assert!(result["run"]["zoneBindings"].get("script-root").is_none());
    assert_eq!(fs::read_to_string(root.join("dynamic.txt")).unwrap(), "dynamic execution");
    assert!(!s.data.path().join("node-zone/dynamic.txt").exists());
}

#[tokio::test]
async fn typescript_user_file_imports_writes_and_hands_off_without_rebuilding_backend() {
    let s = Scenario::new().await;
    bind(&s, r#"import { join, resolve } from 'node:path';
import { suffix } from './helper.ts';
export default async function(context: {zone: string, input: string}) {
    if (process.cwd() !== resolve(context.zone)) throw new Error('wrong execution working directory');
    if (JSON.stringify(context).includes('MANAGER-USER-SECRET') || ['userInput', 'mapInput', 'runInput'].some(key => Object.hasOwn(context, key))) throw new Error('Run background must not enter the script context');
    console.log('ordinary script log, not a result');
    process.stdout.write('another ordinary log\n');
    const path = join(context.zone, 'custom output.txt');
    await Bun.write(path, context.input + suffix);
    return {status:'success',outputs:[{id:'result',path}]};
}"#, json!([{"id":"result"}])).await;
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["consume"] = json!({"title":"Read result","bot_id":"worker","input":"Read the delivered file","zone":"node","blocked_by":["custom"],"input_rule":[{"id":"result","type":"node","node_id":"custom","output_id":"result"}],"output_rule":[]});
    fs::write(&map_path, map.to_string()).unwrap();
    let run = s.start("typescript-manager").await;
    let calls = s.herdr.calls();
    s.dispatch("typescript-manager", &["custom"]).await;
    let result = ended(&s, &run).await;
    assert!(result["run"]["finished"].is_null());
    assert_eq!(result["run"]["nodes"]["consume"]["status"], "pending");
    assert_eq!(s.herdr.calls()[calls.len()..].len(), 1);
    assert!(s.herdr.calls()[calls.len()..].iter().all(|call| call["kind"] == "agent_prompt"), "only the result report notifies manager");
    let node = &result["run"]["nodes"]["custom"];
    assert_eq!(node["execution"]["kind"], "typescript");
    assert_eq!(node["execution"]["script"], "user/custom.ts");
    assert!(node["execution"].get("chat").is_none());
    assert_eq!(node["execution"]["lifecycle"]["status"], "succeeded");
    assert_eq!(node["status"], "running");
    // P26/P27: success is the independent criterion, not dispatch or report
    // presence. A host that only reports success must fail this assertion.
    assert_eq!(node["execution"]["autoApproved"], true);
    assert_eq!(node["execution"]["approved"], true);
    assert_eq!(node["execution"]["approvalSource"], "auto");
    for key in ["taskId", "agent", "chat"] { assert!(node["execution"].get(key).is_none(), "{key}"); }
    let report = &result["reports"][0];
    assert_eq!(report["canComplete"], true);
    assert_eq!(report["source"], json!({"kind":"typescript","script":"user/custom.ts"}));
    assert_eq!(fs::read_to_string(report["outputs"][0]["path"].as_str().unwrap()).unwrap(), "literal input 第二行 / imported");
    let completed = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"typescript-manager","nodeId":"custom","reason":"Read actual delivered file"}).to_string()).await;
    assert_eq!(completed.status, 200, "{}", completed.body);
    let completed: Value = serde_json::from_str(&completed.body).unwrap();
    assert_eq!(completed["snapshot"]["nodes"]["custom"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["consume"]["status"], "pending");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"script-consumer"},"native_cwd":s.data.path().join("node-zone")}));
    let downstream = s.dispatch("typescript-manager", &["consume"]).await;
    assert_eq!(downstream["snapshot"]["nodes"]["consume"]["status"], "running");
    assert_eq!(downstream["snapshot"]["nodes"]["consume"]["execution"]["inputs"], report["outputs"]);
    println!("P26/P27 run={} execution={} report={} explicitComplete={}", run["id"], node["execution"], report, completed["snapshot"]["nodes"]["custom"]["status"]);
}

#[tokio::test]
async fn typescript_status_and_actual_declared_files_control_success() {
    let cases = [
        ("minimal", "return {status:'success'}", false, true),
        ("failure", "return {status:'failure'}", false, false),
        ("missing-status", "return {}", false, false),
        ("wrong-status", "return {status:true}", false, false),
        ("no-result", "process.exit(0)", false, false),
        ("nonzero", "process.exit(7)", false, false),
        ("success-nonzero", "const exit=process.exit; process.exit=()=>exit(7); return {status:'success'}", false, false),
        ("null-result", "return null", false, false),
        ("invalid-outputs", "return {status:'success',outputs:{}}", false, false),
        ("unknown-result-field", "return {status:'success',invented:true}", false, false),
        ("invalid-file-shape", "return {status:'success',outputs:[{id:'result'}]}", true, false),
        ("unreadable", "return {status:'success',outputs:[{id:'result',path:context.zone+'/locked.txt'}]}", true, false),
        ("exception", "console.error('failure stderr'); throw new Error('user failure')", false, false),
        ("missing-binding", "return {status:'success'}", true, false),
        ("missing-file", "return {status:'success',outputs:[{id:'result',path:context.zone+'/absent.txt'}]}", true, false),
        ("directory", "return {status:'success',outputs:[{id:'result',path:context.zone}]}", true, false),
        ("relative", "return {status:'success',outputs:[{id:'result',path:'actual.txt'}]}", true, false),
        ("undeclared", "return {status:'success',outputs:[{id:'other',path}]}", true, false),
        ("duplicate", "return {status:'success',outputs:[{id:'result',path},{id:'result',path}]}", true, false),
        ("partial", "return {status:'failure',outputs:[{id:'result',path}]}", true, false),
    ];
    for (name, body, needs_output, success) in cases {
        let s = Scenario::new().await;
        let source = format!("import {{join}} from 'node:path'; export default async function(context) {{ const path=join(context.zone,'actual.txt'); await Bun.write(path,'real partial effect'); {body}; }}");
        bind(&s, &source, if needs_output { json!([{"id":"result"}]) } else { json!([]) }).await;
        let run = s.start("typescript-manager").await;
        // Real Windows sharing denial leaves validation itself in the path.
        use std::os::windows::fs::OpenOptionsExt;
        let locked = (name == "unreadable").then(|| fs::OpenOptions::new().create(true).write(true).share_mode(0).open(s.data.path().join("node-zone/locked.txt")).unwrap());
        s.dispatch("typescript-manager", &["custom"]).await;
        let result = ended(&s, &run).await;
        drop(locked);
        let node = &result["run"]["nodes"]["custom"];
        assert_eq!(node["execution"]["approved"], success, "{name}: {result}");
        assert_eq!(node["execution"]["approvalSource"], if success { json!("auto") } else { Value::Null }, "{name}: {result}");
        assert_eq!(node["status"], "running", "{name}: {result}");
        assert!(node["execution"].get("chat").is_none());
        assert_eq!(node["execution"]["lifecycle"]["status"], if success { "succeeded" } else { "failed" }, "{name}: {result}");
        assert_eq!(result["reports"][0]["canComplete"], success, "{name}: {result}");
        assert_eq!(fs::read_to_string(s.data.path().join("node-zone/actual.txt")).unwrap(), "real partial effect", "failure must not roll back: {name}");
        println!("P26 case={name} run={} execution={} report={}", run["id"], node["execution"], result["reports"][0]);
        if name == "exception" {
            let failure = &node["execution"]["lifecycle"]["failure"];
            assert_eq!(failure["code"], "script-process-failed");
            assert!(failure["logs"].as_str().unwrap().contains("failure stderr\n"));
            let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/script-output", r#"{"sessionId":"typescript-manager"}"#).await;
            assert_eq!(response.status, 200, "{}", response.body);
            let snapshot: Value = serde_json::from_str(&response.body).unwrap();
            assert_eq!(snapshot["nodes"][0]["status"], "failed");
            assert!(snapshot["nodes"][0]["lastOutputAt"].is_u64());
        }
        if name == "partial" {
            let outputs = result["reports"][0]["outputs"].as_array().unwrap();
            assert_eq!(outputs[0]["id"], "result");
            assert_eq!(fs::read_to_string(outputs[0]["path"].as_str().unwrap()).unwrap(), "real partial effect");
        }
    }
}
