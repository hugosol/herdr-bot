//! Public Map dispatch and actual Bun files: no built-in capability registration.
use super::*;

async fn bind(s: &Scenario, source: &str, outputs: Value) {
    let directory = s.config.scheduler.bot_directory.parent().unwrap().join("scripts").join("user");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("helper.ts"), "export const suffix: string = ' / imported';\n").unwrap();
    fs::write(directory.join("custom.ts"), source).unwrap();
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/bots/save", &json!({"id":"user-script","revision":null,"content":json!({"name":"User script","when":"","plan_guide":"","node_guide":"","script":"user/custom.ts"}).to_string()}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"User TS","description":"","nodes":{
        "custom":{"title":"Run user code","bot_id":"user-script","input":"literal input 第二行","zone":"node","blocked_by":[],"output_rule":outputs}
    }}).to_string()).unwrap();
}

async fn ended(s: &Scenario, run: &Value) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let response = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{}", run["id"].as_str().unwrap())).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let detail: Value = serde_json::from_str(&response.body).unwrap();
        if !detail["reports"].as_array().unwrap().is_empty() {
            assert!(common::wait_until(Duration::from_secs(5), || {
                use std::os::windows::fs::OpenOptionsExt;
                s.herdr.calls().iter().any(|call| call["kind"] == "agent_prompt" && call["text"].as_str().is_some_and(|text| text.starts_with("Workflow report\n")))
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
    console.log('ordinary script log, not a result');
    process.stdout.write('another ordinary log\n');
    const path = join(context.zone, 'custom output.txt');
    await Bun.write(path, context.input + suffix);
    return {status:'success',outputs:[{id:'result',path}]};
}"#, json!([{"id":"result"}])).await;
    let run = s.start("typescript-manager").await;
    s.dispatch("typescript-manager", &["custom"]).await;
    let result = ended(&s, &run).await;
    let node = &result["run"]["nodes"]["custom"];
    assert_eq!(node["execution"]["kind"], "typescript");
    assert_eq!(node["execution"]["script"], "user/custom.ts");
    assert!(node["execution"].get("chat").is_none());
    assert_eq!(node["execution"]["lifecycle"]["status"], "succeeded");
    assert_eq!(node["status"], "running");
    let report = &result["reports"][0];
    assert_eq!(report["canComplete"], true);
    assert_eq!(report["source"], json!({"kind":"typescript","script":"user/custom.ts"}));
    assert_eq!(fs::read_to_string(report["outputs"][0]["path"].as_str().unwrap()).unwrap(), "literal input 第二行 / imported");
    let completed = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"typescript-manager","nodeId":"custom","reason":"Read actual delivered file"}).to_string()).await;
    assert_eq!(completed.status, 200, "{}", completed.body);
}

#[tokio::test]
async fn typescript_status_and_actual_declared_files_control_success() {
    let cases = [
        ("minimal", "return {status:'success'}", false, true),
        ("failure", "return {status:'failure'}", false, false),
        ("missing-status", "return {}", false, false),
        ("wrong-status", "return {status:true}", false, false),
        ("no-result", "process.exit(0)", false, false),
        ("exception", "throw new Error('user failure')", false, false),
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
        s.dispatch("typescript-manager", &["custom"]).await;
        let result = ended(&s, &run).await;
        let node = &result["run"]["nodes"]["custom"];
        assert_eq!(node["execution"]["lifecycle"]["status"], if success { "succeeded" } else { "failed" }, "{name}: {result}");
        assert_eq!(result["reports"][0]["canComplete"], success, "{name}: {result}");
        assert_eq!(fs::read_to_string(s.data.path().join("node-zone/actual.txt")).unwrap(), "real partial effect", "failure must not roll back: {name}");
        if name == "partial" {
            let outputs = result["reports"][0]["outputs"].as_array().unwrap();
            assert_eq!(outputs[0]["id"], "result");
            assert_eq!(fs::read_to_string(outputs[0]["path"].as_str().unwrap()).unwrap(), "real partial effect");
        }
    }
}
