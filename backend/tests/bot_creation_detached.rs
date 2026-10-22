//! 浏览器断开与多客户端并发的公开接缝（S2 HTTP / S3 Herdr 控制）验收。
//!
//! 这里只使用公开 HTTP 入口与受控 Herdr 夹具自己写下的外部记录：创建被接受之后，观察连接的
//! 断开不是取消；两个独立客户端同时提交时各自得到独立对象、模板快照与输入。期望值来自预写的
//! 字面样例与夹具记录，不由 herdr-bot 的命名或合成函数生成。受控屏障只让某一次创建停在
//! 启动或提交之前，用来证明另一个创建不会被它排队等待，而不是用“先后各成功一次”冒充并发。

mod common;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{
    FixtureTree, HerdrFixture, OpenJsonRequest, TempDir, TestServer, get_over_tcp,
    post_json_over_tcp, post_over_tcp, request_json_over_tcp, same_path, wait_until,
};
use serde_json::{Value, json};

const STAGE_DEADLINE: Duration = Duration::from_secs(20);

/// 与交付配置同一形状；Herdr 可执行文件指向受控副本，目标 Workspace 取显式名称。
fn config_with(herdr: &HerdrFixture, storage: &TempDir) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    config_with_operations(
        herdr,
        storage,
        &fixtures.join("operations").join("success.bat"),
        &fixtures.join("operations"),
    )
}

/// 同上，但固定操作绑定到调用者给出的真实脚本与工作目录（需要验证互不影响的用例使用）。
fn config_with_operations(
    herdr: &HerdrFixture,
    storage: &TempDir,
    script: &Path,
    operations: &Path,
) -> herdr_bot::Config {
    let mut config = common::config(script, operations);
    config.zones.registry_file = storage.path().join("zones.json");
    config.bot_templates.storage_file = storage.path().join("bot-templates.json");
    config.herdr = herdr_bot::HerdrConfig {
        workspace_name: "bot-playground".to_string(),
        executable: herdr.executable().to_path_buf(),
    };
    config
}

/// 一个已登记的真实目录；返回登记列表里的路径。
async fn register(server: &TestServer, path: &Path) -> String {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": path }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记 zone 应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("登记响应是 JSON");
    body["zones"]
        .as_array()
        .expect("登记响应必须有 zones")
        .iter()
        .find_map(|zone| {
            let registered = zone["path"].as_str()?;
            same_path(Path::new(registered), path).then(|| registered.to_string())
        })
        .expect("登记响应必须包含刚登记的目录")
}

/// 保存一个模板并返回刚保存的对象（列表按创建顺序追加，新模板在末尾）。
async fn save_template(server: &TestServer, template: Value) -> Value {
    let expected_name = template["name"]
        .as_str()
        .expect("模板样例必须有 name")
        .to_string();
    let response =
        post_json_over_tcp(server.addr(), "/api/bot-templates", &template.to_string()).await;
    assert_eq!(response.status, 200, "保存模板应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("模板响应是 JSON");
    let saved = body["templates"]
        .as_array()
        .expect("模板响应必须有 templates")
        .last()
        .expect("保存后必须有一个模板")
        .clone();
    assert_eq!(saved["name"], expected_name, "刚保存的模板必须在列表末尾");
    saved
}

async fn update_template(server: &TestServer, id: &str, template: Value) -> Value {
    let response = request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/bot-templates/{id}"),
        Some(&template.to_string()),
    )
    .await;
    assert_eq!(response.status, 200, "编辑模板应成功：{}", response.body);
    serde_json::from_str(&response.body).expect("模板响应是 JSON")
}

async fn preview(server: &TestServer, zone: &str, template_id: &str) -> Value {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/bots/preview",
        &json!({ "zone": zone, "templateId": template_id }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "预览应成功：{}", response.body);
    serde_json::from_str(&response.body).expect("预览响应是 JSON")
}

fn create_body(zone: &str, template_id: &str, preview: &Value, input: &str) -> String {
    json!({
        "zone": zone,
        "templateId": template_id,
        "botId": preview["botId"],
        "input": input,
        "expectedName": preview["name"],
    })
    .to_string()
}

/// 启动参数以预写的 model / thinking 覆盖开头，后接本次独立的会话目录。
///
/// `--session-dir` 的具体路径与新鲜度由实例身份覆盖单独断言；这里核对覆盖值与绝对路径，
/// 避免把动态创建标识写进期望值。
fn assert_launch_args(actual: &Value, expected: &[&str], context: &str) {
    let args = actual.as_array().expect("启动参数是数组");

    let expected = expected.iter().map(|arg| json!(arg)).collect::<Vec<_>>();
    assert_eq!(
        &args[..expected.len()],
        expected.as_slice(),
        "{context}：模型与 thinking 必须与预写样例一致：{actual}"
    );
    assert_eq!(
        args[expected.len()],
        "--session-dir",
        "{context}：模型 / thinking 之后必须显式选择独立的新会话目录：{actual}"
    );
    assert!(
        Path::new(args[expected.len() + 1].as_str().expect("会话目录是字符串")).is_absolute(),
        "{context}：会话目录必须是绝对路径：{actual}"
    );
}

async fn create(server: &TestServer, body: &str) -> common::HttpResponse {
    post_json_over_tcp(server.addr(), "/api/bots", body).await
}

async fn creation(server: &TestServer, id: &str) -> Value {
    let response = get_over_tcp(server.addr(), &format!("/api/bots/{id}")).await;
    assert_eq!(
        response.status, 200,
        "创建状态读取应成功：{}",
        response.body
    );
    serde_json::from_str(&response.body).expect("创建状态是 JSON")
}

/// 轮询公开状态直到指定阶段；超时让用例失败并打印最近一次响应。
async fn wait_stage(server: &TestServer, id: &str, expected: &str) -> Value {
    let deadline = tokio::time::Instant::now() + STAGE_DEADLINE;
    loop {
        let view = creation(server, id).await;
        if view["stage"] == expected {
            return view;
        }
        if view["stage"] == "failed" && expected != "failed" {
            panic!("创建在到达 {expected} 之前失败：{view}");
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("创建没有在时限内到达阶段 {expected}，最近状态：{view}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 经公开预览与创建入口接受一次创建，返回它的生命周期标识与预览。
async fn accept_creation(
    server: &TestServer,
    zone: &str,
    template_id: &str,
    input: &str,
) -> (String, Value) {
    let preview = preview(server, zone, template_id).await;
    let response = create(server, &create_body(zone, template_id, &preview, input)).await;
    assert_eq!(response.status, 202, "创建应被接受：{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    assert_eq!(accepted["stage"], "starting");
    (
        accepted["id"].as_str().expect("创建标识").to_string(),
        accepted,
    )
}

fn calls_of_kind(herdr: &HerdrFixture, kind: &str) -> Vec<Value> {
    herdr
        .calls()
        .into_iter()
        .filter(|call| call["kind"] == kind)
        .collect()
}

fn call_kinds(herdr: &HerdrFixture) -> Vec<String> {
    herdr
        .calls()
        .into_iter()
        .filter_map(|call| call["kind"].as_str().map(str::to_string))
        .collect()
}

/// 预写的模板字面样例：默认 model / thinking、无 Skill、固定 prompt 为空。
fn default_template(name: &str) -> Value {
    json!({
        "name": name,
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    })
}

fn zone_leaf(path: &str) -> String {
    Path::new(path)
        .file_name()
        .expect("zone 目录必须有末级名称")
        .to_string_lossy()
        .into_owned()
}

fn release_path(herdr: &HerdrFixture, name: &str) -> PathBuf {
    herdr.path().join(name)
}

fn release(path: &Path) {
    fs::write(path, b"release").expect("写释放文件");
}

/// 一条已经发出的创建状态观察连接；用例可以在不读取响应的情况下断开它。
struct OpenObservation {
    stream: tokio::net::TcpStream,
}

impl OpenObservation {
    async fn open(addr: SocketAddr, path: &str) -> Self {
        use tokio::io::AsyncWriteExt;

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("连接测试服务器");
        let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("发送观察请求");
        stream.flush().await.expect("发送观察请求");
        Self { stream }
    }

    /// 主动断开，不读取这次观察的任何响应，也不发送任何“取消”请求。
    fn abort(self) {
        drop(self.stream);
    }
}

/// 从外部记录中找出这一次创建自己的 Tab、启动与提交。
fn external_objects<'a>(herdr: &'a HerdrFixture, view: &Value) -> (Value, Value, Value) {
    let agent_name = view["agentName"]
        .as_str()
        .expect("成功的创建必须有 agent 别名");
    let tabs = herdr.tabs();
    let tab = tabs
        .iter()
        .find(|tab| tab["tab_id"] == view["tabId"])
        .cloned()
        .expect("外部记录必须包含这次创建的 Tab");
    let starts = calls_of_kind(herdr, "agent_start");
    let start = starts
        .iter()
        .find(|call| call["name"] == agent_name)
        .cloned()
        .expect("外部记录必须包含这次创建的 agent 启动");
    let prompts = calls_of_kind(herdr, "agent_prompt");
    let prompt = prompts
        .iter()
        .find(|call| call["target"] == agent_name)
        .cloned()
        .expect("外部记录必须包含这次创建的初始指示提交");
    (tab, start, prompt)
}

/// P50：接受创建后，在尚未就绪与尚未投递确认两个受控时点断开观察连接；同一次创建仍完成，
/// 且只有一次初始化提交，没有关闭、重建或重发。
#[tokio::test(flavor = "multi_thread")]
async fn accepted_creation_continues_after_observation_connections_drop() {
    let herdr = HerdrFixture::new("bot detach observations");
    herdr.set_behavior(json!({
        "agent_start_delay_ms": 1_200,
        "prompt_delay_ms": 1_200,
    }));
    let storage = TempDir::new("bot detach observations storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template("审查")).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let (id, accepted) = accept_creation(&server, &zone_path, template_id, "断线后的任务").await;

    // 尚未就绪：创建仍处于正在启动，此时断开一条观察连接。
    let starting = creation(&server, &id).await;
    assert_eq!(starting["stage"], "starting");
    OpenObservation::open(server.addr(), &format!("/api/bots/{id}"))
        .await
        .abort();
    assert!(
        wait_until(Duration::from_secs(10), || {
            !calls_of_kind(&herdr, "agent_start").is_empty()
        })
        .await,
        "受控 Herdr 必须已经收到 agent start"
    );
    let still_starting = creation(&server, &id).await;
    assert_eq!(
        still_starting["stage"], "starting",
        "观察连接断开不能取消已接受的创建"
    );
    assert!(
        calls_of_kind(&herdr, "agent_prompt").is_empty(),
        "就绪之前不得提交初始指示"
    );

    // 尚未投递确认：提交调用已经到达夹具但响应被延迟；再断开一条观察连接。
    let submitting = wait_stage(&server, &id, "submitting").await;
    assert_eq!(submitting["inputSubmitted"], false);
    assert!(
        wait_until(Duration::from_secs(10), || {
            !calls_of_kind(&herdr, "agent_prompt").is_empty()
        })
        .await,
        "受控 Herdr 必须已经收到初始指示提交"
    );
    assert_eq!(creation(&server, &id).await["inputSubmitted"], false);
    assert_eq!(
        calls_of_kind(&herdr, "agent_prompt").len(),
        1,
        "提交必须已经到达受控 Herdr"
    );
    OpenObservation::open(server.addr(), &format!("/api/bots/{id}"))
        .await
        .abort();

    // 独立客户端继续读取同一次创建：断开观察不改变创建责任。
    let finished = wait_stage(&server, &id, "succeeded").await;
    assert_eq!(finished["id"], accepted["id"]);
    assert_eq!(finished["name"], accepted["name"]);
    assert_eq!(finished["zone"], zone_path);
    assert_eq!(finished["inputSubmitted"], true);

    // 独立外部记录：一次 Tab、一次启动、一次提交；没有关闭、重建、重发。
    let kinds = call_kinds(&herdr);
    assert_eq!(
        kinds,
        vec![
            "workspace_list".to_string(),
            "tab_create".to_string(),
            "agent_start".to_string(),
            "agent_prompt".to_string(),
        ],
        "断开观察连接后不得产生额外的创建、提交或清理动作"
    );
    let (tab, start, prompt) = external_objects(&herdr, &finished);
    assert_eq!(tab["cwd"], zone_path);
    assert_eq!(tab["label"], accepted["name"]);
    assert_eq!(start["pane"], tab["pane_id"]);
    assert_eq!(prompt["text"], "断线后的任务");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(call_kinds(&herdr), kinds, "连接断开不能伪装成重试或重建");
    assert_eq!(herdr.tabs().len(), 1, "已创建的 Tab 必须保留");
}

/// P50：提交请求的连接在结果返回前断开，后端已接受的创建仍继续完成。
#[tokio::test(flavor = "multi_thread")]
async fn accepted_creation_continues_when_the_submitting_client_disconnects() {
    let herdr = HerdrFixture::new("bot detach submit connection");
    herdr.set_behavior(json!({ "agent_start_delay_ms": 1_500 }));
    let storage = TempDir::new("bot detach submit connection storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template("审查")).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview = preview(&server, &zone_path, template_id).await;
    let body = create_body(&zone_path, template_id, &preview, "提交连接断开的任务");

    // 只发送创建请求，不读取回执；受控 Herdr 收到 agent start 说明后端已经接手。
    let client = OpenJsonRequest::submit(server.addr(), "/api/bots", &body).await;
    assert!(
        wait_until(Duration::from_secs(10), || {
            !calls_of_kind(&herdr, "agent_start").is_empty()
        })
        .await,
        "后端必须在断线前已经开始执行"
    );
    client.abort();

    // 独立外部记录：同一次创建继续到唯一一次提交完成。
    assert!(
        wait_until(Duration::from_secs(15), || {
            calls_of_kind(&herdr, "agent_prompt").len() == 1
        })
        .await,
        "断线不能取消已接受的创建：{:?}",
        call_kinds(&herdr)
    );
    let tab = herdr.tabs().pop().expect("受控 Herdr 必须记录 Tab");
    assert_eq!(tab["label"], preview["name"]);
    assert_eq!(tab["cwd"], zone_path);
    let prompt = calls_of_kind(&herdr, "agent_prompt")
        .pop()
        .expect("必须有提交记录");
    assert_eq!(prompt["text"], "提交连接断开的任务");
    assert_eq!(
        call_kinds(&herdr),
        vec![
            "workspace_list".to_string(),
            "tab_create".to_string(),
            "agent_start".to_string(),
            "agent_prompt".to_string(),
        ],
        "提交连接断开不得引起重发、重建或清理"
    );
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(calls_of_kind(&herdr, "agent_prompt").len(), 1);
    assert_eq!(herdr.tabs().len(), 1);
}

/// P51：两个独立客户端同时对同一 zone、再同时对不同 zone 提交；每次创建各有对象、输入与目标。
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_clients_get_independent_objects_and_inputs() {
    let herdr = HerdrFixture::new("bot concurrent clients");
    let storage = TempDir::new("bot concurrent clients storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let alpha = storage.path().join("alpha");
    let beta = storage.path().join("beta");
    fs::create_dir_all(&alpha).expect("创建 alpha");
    fs::create_dir_all(&beta).expect("创建 beta");
    let alpha_path = register(&server, &alpha).await;
    let beta_path = register(&server, &beta).await;
    let template = save_template(&server, default_template("并行")).await;
    let template_id = template["id"].as_str().expect("模板标识").to_string();

    // 同一 zone：两个独立客户端同时提交，各自先用独立的预览与字面 input。
    let first_preview = preview(&server, &alpha_path, &template_id).await;
    let second_preview = preview(&server, &alpha_path, &template_id).await;
    let first_body = create_body(&alpha_path, &template_id, &first_preview, "同 zone 任务甲");
    let second_body = create_body(&alpha_path, &template_id, &second_preview, "同 zone 任务乙");
    let (first_response, second_response) =
        tokio::join!(create(&server, &first_body), create(&server, &second_body));
    assert_eq!(first_response.status, 202, "{}", first_response.body);
    assert_eq!(second_response.status, 202, "{}", second_response.body);
    let first: Value = serde_json::from_str(&first_response.body).expect("创建回执是 JSON");
    let second: Value = serde_json::from_str(&second_response.body).expect("创建回执是 JSON");
    assert_ne!(
        first["id"], second["id"],
        "每次创建必须有独立的生命周期标识"
    );
    assert_ne!(
        first["name"], second["name"],
        "独立创建必须使用各自的短实例标识"
    );

    let first = wait_stage(&server, first["id"].as_str().unwrap(), "succeeded").await;
    let second = wait_stage(&server, second["id"].as_str().unwrap(), "succeeded").await;
    assert_ne!(first["tabId"], second["tabId"], "不得复用同一个 Tab");
    assert_ne!(first["paneId"], second["paneId"], "不得复用同一个 Pane");
    assert_ne!(first["agentName"], second["agentName"], "不得复用控制别名");
    for view in [&first, &second] {
        let (tab, start, prompt) = external_objects(&herdr, view);
        assert_eq!(tab["cwd"], alpha_path, "两个创建都必须使用所选 zone 的目录");
        assert_eq!(tab["label"], view["name"]);
        assert_eq!(start["pane"], tab["pane_id"]);
        assert_eq!(prompt["target"], start["name"]);
    }
    let first_prompt = external_objects(&herdr, &first).2;
    let second_prompt = external_objects(&herdr, &second).2;
    assert_eq!(first_prompt["text"], "同 zone 任务甲");
    assert_eq!(second_prompt["text"], "同 zone 任务乙");
    assert_eq!(herdr.tabs().len(), 2, "同一 zone 的两次创建各有独立 Tab");

    // 不同 zone：同时提交后，每次创建使用自己 zone 的目录与字面任务。
    let alpha_preview = preview(&server, &alpha_path, &template_id).await;
    let beta_preview = preview(&server, &beta_path, &template_id).await;
    let alpha_body = create_body(&alpha_path, &template_id, &alpha_preview, "跨 zone 任务甲");
    let beta_body = create_body(&beta_path, &template_id, &beta_preview, "跨 zone 任务乙");
    let (alpha_response, beta_response) =
        tokio::join!(create(&server, &alpha_body), create(&server, &beta_body));
    assert_eq!(alpha_response.status, 202, "{}", alpha_response.body);
    assert_eq!(beta_response.status, 202, "{}", beta_response.body);
    let alpha_creation: Value =
        serde_json::from_str(&alpha_response.body).expect("创建回执是 JSON");
    let beta_creation: Value = serde_json::from_str(&beta_response.body).expect("创建回执是 JSON");
    let alpha_creation =
        wait_stage(&server, alpha_creation["id"].as_str().unwrap(), "succeeded").await;
    let beta_creation =
        wait_stage(&server, beta_creation["id"].as_str().unwrap(), "succeeded").await;

    let (alpha_tab, _, alpha_prompt) = external_objects(&herdr, &alpha_creation);
    let (beta_tab, _, beta_prompt) = external_objects(&herdr, &beta_creation);
    assert_eq!(
        alpha_tab["cwd"], alpha_path,
        "输入与目标必须对应同一个 zone"
    );
    assert_eq!(beta_tab["cwd"], beta_path, "输入与目标必须对应同一个 zone");
    assert_eq!(alpha_prompt["text"], "跨 zone 任务甲");
    assert_eq!(beta_prompt["text"], "跨 zone 任务乙");
    assert_ne!(alpha_tab["tab_id"], beta_tab["tab_id"]);
    assert_eq!(herdr.tabs().len(), 4, "两次阶段共创建四个独立 Tab");
    assert_eq!(calls_of_kind(&herdr, "agent_prompt").len(), 4);
    assert_eq!(
        calls_of_kind(&herdr, "unsupported").len(),
        0,
        "并发创建不得尝试合并、关闭或重建"
    );
}

/// P51：一个创建停在提交屏障时，另一个已接受的创建继续完成，不排队等待。
#[tokio::test(flavor = "multi_thread")]
async fn a_creation_blocked_at_submission_does_not_queue_another() {
    let herdr = HerdrFixture::new("bot concurrent submit barrier");
    let blocked_release = release_path(&herdr, "release-blocked-prompt");
    herdr.set_behavior(json!({
        "prompt_gate": {
            "match": "被阻塞的创建",
            "wait_for_file": blocked_release.to_string_lossy(),
            "timeout_ms": 20_000,
        },
    }));
    let storage = TempDir::new("bot concurrent submit barrier storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template("并发")).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let (blocked_id, _) = accept_creation(&server, &zone_path, template_id, "被阻塞的创建").await;

    // 受控夹具已经记录这次提交，但屏障让它的响应停在“正在提交”。
    assert!(
        wait_until(Duration::from_secs(10), || {
            calls_of_kind(&herdr, "agent_prompt").len() == 1
        })
        .await,
        "被阻塞的创建必须已经发起提交"
    );
    let blocked = wait_stage(&server, &blocked_id, "submitting").await;
    assert_eq!(blocked["inputSubmitted"], false);

    // 第二个创建在第一个仍受阻时被接受并完整完成。
    let (free_id, _) = accept_creation(&server, &zone_path, template_id, "率先完成的创建").await;
    let free = wait_stage(&server, &free_id, "succeeded").await;
    let blocked_during = creation(&server, &blocked_id).await;
    assert_eq!(
        blocked_during["stage"], "submitting",
        "第一个创建在屏障释放前不能完成"
    );
    assert_eq!(blocked_during["inputSubmitted"], false);
    assert_eq!(free["inputSubmitted"], true);

    // 外部记录：第二个创建的完整链已经在第一个提交仍未确认时发生。
    let free_prompt = external_objects(&herdr, &free).2;
    assert_eq!(free_prompt["text"], "率先完成的创建");
    assert_eq!(
        calls_of_kind(&herdr, "agent_prompt").len(),
        2,
        "每个已接受创建各提交一次"
    );

    release(&blocked_release);
    let blocked = wait_stage(&server, &blocked_id, "succeeded").await;
    let blocked_prompt = external_objects(&herdr, &blocked).2;
    assert_eq!(blocked_prompt["text"], "被阻塞的创建");
    assert_eq!(herdr.tabs().len(), 2);
    assert_eq!(calls_of_kind(&herdr, "agent_prompt").len(), 2);
    assert_eq!(calls_of_kind(&herdr, "unsupported").len(), 0);
}

/// P51：一个创建停在启动屏障时，另一个创建不被它阻塞。
#[tokio::test(flavor = "multi_thread")]
async fn a_creation_blocked_before_readiness_does_not_queue_another() {
    let herdr = HerdrFixture::new("bot concurrent start barrier");
    let blocked_release = release_path(&herdr, "release-blocked-start");
    herdr.set_behavior(json!({
        "agent_start_gate": {
            "match": "deepseek/deepseek-flash",
            "wait_for_file": blocked_release.to_string_lossy(),
            "timeout_ms": 20_000,
        },
    }));
    let storage = TempDir::new("bot concurrent start barrier storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let blocked_template = save_template(
        &server,
        json!({
            "name": "启动受阻",
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "omp",
            "model": "deepseek-flash",
            "thinking": "high",
            "skill": null,
            "prompt": "",
        }),
    )
    .await;
    let free_template = save_template(&server, default_template("起跑顺利")).await;
    let blocked_template_id = blocked_template["id"].as_str().expect("模板标识");
    let free_template_id = free_template["id"].as_str().expect("模板标识");

    let (blocked_id, _) =
        accept_creation(&server, &zone_path, blocked_template_id, "启动被阻塞的任务").await;
    assert!(
        wait_until(Duration::from_secs(10), || {
            calls_of_kind(&herdr, "agent_start").len() == 1
        })
        .await,
        "被阻塞的创建必须已经发起启动"
    );
    let blocked = creation(&server, &blocked_id).await;
    assert_eq!(blocked["stage"], "starting");

    // 第二个创建在第一个仍未就绪时完成全部阶段。
    let (free_id, _) = accept_creation(&server, &zone_path, free_template_id, "先完成的任务").await;
    let free = wait_stage(&server, &free_id, "succeeded").await;
    assert_eq!(
        creation(&server, &blocked_id).await["stage"],
        "starting",
        "第一个创建在启动屏障释放前不能进入后续阶段"
    );
    let free_prompt = external_objects(&herdr, &free).2;
    assert_eq!(free_prompt["text"], "先完成的任务");
    assert!(
        calls_of_kind(&herdr, "agent_prompt")
            .iter()
            .all(|prompt| prompt["text"] != "启动被阻塞的任务"),
        "受阻的创建不得提前提交"
    );

    release(&blocked_release);
    let blocked = wait_stage(&server, &blocked_id, "succeeded").await;
    let (_, blocked_start, blocked_prompt) = external_objects(&herdr, &blocked);
    assert_launch_args(
        &blocked_start["args"],
        &["--model", "deepseek/deepseek-flash", "--thinking", "high"],
        "启动受阻的创建",
    );
    assert_eq!(blocked_prompt["text"], "启动被阻塞的任务");
    assert_eq!(herdr.tabs().len(), 2);
    assert_eq!(calls_of_kind(&herdr, "agent_prompt").len(), 2);
}

/// P51：并发请求各自采用接受时的模板快照；两个独立客户端在创建途中分别编辑模板，
/// 已经接受的两次创建仍按旧字面配置与输入执行。
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creations_keep_their_own_accepted_template_snapshots() {
    let herdr = HerdrFixture::new("bot concurrent snapshots");
    let start_release = release_path(&herdr, "release-snapshot-start");
    let prompt_release = release_path(&herdr, "release-snapshot-prompt");
    herdr.set_behavior(json!({
        "agent_start_gate": {
            "match": "deepseek/deepseek-flash",
            "wait_for_file": start_release.to_string_lossy(),
            "timeout_ms": 20_000,
        },
        "prompt_gate": {
            "match": "快照 B 的输入",
            "wait_for_file": prompt_release.to_string_lossy(),
            "timeout_ms": 20_000,
        },
    }));
    let storage = TempDir::new("bot concurrent snapshots storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template_a = save_template(
        &server,
        json!({
            "name": "并发旧版 A",
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "omp",
            "model": "deepseek-flash",
            "thinking": "high",
            "skill": "teach",
            "prompt": "快照 A 的固定指示",
        }),
    )
    .await;
    let template_b = save_template(
        &server,
        json!({
            "name": "并发旧版 B",
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "pi",
            "model": "gpt-6.1-sol",
            "thinking": "low",
            "skill": "code-review",
            "prompt": "快照 B 的固定指示",
        }),
    )
    .await;
    let template_a_id = template_a["id"].as_str().expect("模板标识").to_string();
    let template_b_id = template_b["id"].as_str().expect("模板标识").to_string();

    // 两个独立客户端同时提交，各自使用不同的已保存版本与字面 input。
    let preview_a = preview(&server, &zone_path, &template_a_id).await;
    let preview_b = preview(&server, &zone_path, &template_b_id).await;
    let body_a = create_body(&zone_path, &template_a_id, &preview_a, "快照 A 的输入");
    let body_b = create_body(&zone_path, &template_b_id, &preview_b, "快照 B 的输入");
    let (response_a, response_b) = tokio::join!(create(&server, &body_a), create(&server, &body_b));
    assert_eq!(response_a.status, 202, "{}", response_a.body);
    assert_eq!(response_b.status, 202, "{}", response_b.body);
    let creation_a: Value = serde_json::from_str(&response_a.body).expect("创建回执是 JSON");
    let creation_b: Value = serde_json::from_str(&response_b.body).expect("创建回执是 JSON");
    let id_a = creation_a["id"].as_str().expect("创建标识").to_string();
    let id_b = creation_b["id"].as_str().expect("创建标识").to_string();

    // 两个创建分别停在启动与提交屏障：它们都已接受，但都还没有完成。
    assert!(
        wait_until(Duration::from_secs(10), || {
            calls_of_kind(&herdr, "agent_start").len() >= 2
        })
        .await,
        "受控 Herdr 必须已经收到两次启动"
    );
    assert!(
        wait_until(Duration::from_secs(10), || {
            calls_of_kind(&herdr, "agent_prompt").len() == 1
        })
        .await,
        "第二个创建必须已经发起提交"
    );
    assert_eq!(creation(&server, &id_a).await["stage"], "starting");
    assert_eq!(creation(&server, &id_b).await["stage"], "submitting");

    // 另一客户端在创建途中把两个模板都改成完全不同的新版本。
    update_template(
        &server,
        &template_a_id,
        json!({
            "name": "并发新版 A",
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "pi",
            "model": "default",
            "thinking": "default",
            "skill": null,
            "prompt": "新版 A 的固定指示",
        }),
    )
    .await;
    update_template(
        &server,
        &template_b_id,
        json!({
            "name": "并发新版 B",
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "omp",
            "model": "default",
            "thinking": "default",
            "skill": null,
            "prompt": "新版 B 的固定指示",
        }),
    )
    .await;

    release(&start_release);
    release(&prompt_release);
    let finished_a = wait_stage(&server, &id_a, "succeeded").await;
    let finished_b = wait_stage(&server, &id_b, "succeeded").await;
    assert_eq!(finished_a["templateId"], template_a_id);
    assert_eq!(finished_b["templateId"], template_b_id);
    assert_eq!(finished_a["name"], preview_a["name"]);
    assert_eq!(finished_b["name"], preview_b["name"]);

    // 外部记录：两次创建分别使用接受时的旧快照，而不是编辑后的新版本。
    let (tab_a, start_a, prompt_a) = external_objects(&herdr, &finished_a);
    let (tab_b, start_b, prompt_b) = external_objects(&herdr, &finished_b);
    assert_eq!(tab_a["label"], preview_a["name"]);
    assert_eq!(tab_b["label"], preview_b["name"]);
    assert_eq!(start_a["agent_kind"], "omp");
    assert_eq!(start_b["agent_kind"], "pi");
    assert_launch_args(
        &start_a["args"],
        &["--model", "deepseek/deepseek-flash", "--thinking", "high"],
        "创建 A 必须使用接受时的旧模板启动参数",
    );
    assert_launch_args(
        &start_b["args"],
        &["--model", "openai-codex/gpt-6.1-sol", "--thinking", "low"],
        "创建 B 必须使用接受时的旧模板启动参数",
    );
    assert_eq!(
        prompt_a["text"],
        "/skill:teach 快照 A 的固定指示\n\n快照 A 的输入"
    );
    assert_eq!(
        prompt_b["text"],
        "/skill:code-review 快照 B 的固定指示\n\n快照 B 的输入"
    );
    assert_eq!(herdr.tabs().len(), 2);
    assert_eq!(calls_of_kind(&herdr, "unsupported").len(), 0);
}

/// P36/P37 的预写元数据：保持稳定标识，替换完整 provider/model、Skill 和两类扩展映射。
/// 文件在真实磁盘改写；期望参数在断言处独立写明，不调用生产解析或参数合成函数。
fn write_snapshot_catalog(storage: &TempDir, revision: &str) -> PathBuf {
    let (model, native_skill, fixed, tools, connect) = match revision {
        "old" => (
            "old-provider/old-model",
            "old-native",
            "old-fixed.ts",
            "old-tools.ts",
            "old-connect.ts",
        ),
        "new" => (
            "new-provider/new-model",
            "new-native",
            "new-fixed.ts",
            "new-tools.ts",
            "new-connect.ts",
        ),
        _ => panic!("未知样例版本"),
    };
    let metadata = storage.path().join("snapshot-options.toml");
    fs::write(
        &metadata,
        format!(
            r#"
[[agents]]
id = 'omp'
label = 'OMP'
[[models]]
id = 'default'
label = '默认'
[[models]]
id = 'mapped-model'
label = '映射模型'
provider_model = '{model}'
thinking = {{ omp = ['low', 'high'] }}
[[skills]]
id = 'mapped-skill'
label = '映射 Skill'
native_name = '{native_skill}'
[[extensions]]
id = 'fixed'
label = '固定扩展'
agent = 'omp'
kind = 'fixed'
entrypoint = '{}'
[[extensions]]
id = 'tools'
label = '工具扩展'
agent = 'omp'
kind = 'optional'
entrypoint = '{}'
[[extensions]]
id = 'connect'
label = '接入扩展'
agent = 'omp'
kind = 'optional'
entrypoint = '{}'
"#,
            storage.path().join(fixed).display(),
            storage.path().join(tools).display(),
            storage.path().join(connect).display()
        ),
    )
    .expect("写入真实元数据");
    for name in [fixed, tools, connect] {
        fs::write(
            storage.path().join(name),
            "// controlled extension entrypoint\n",
        )
        .expect("写扩展入口");
    }
    metadata
}

fn snapshot_template(revision: &str) -> Value {
    match revision {
        "old" => json!({
            "name": "旧快照", "botName": "old-{template.name}-{bot.id}", "agent": "omp",
            "model": "mapped-model", "thinking": "high", "skill": "mapped-skill",
            "prompt": "旧固定指示\n第二行", "extensions": ["tools"],
        }),
        "new" => json!({
            "name": "新快照", "botName": "new-{template.name}-{bot.id}", "agent": "omp",
            "model": "mapped-model", "thinking": "low", "skill": "mapped-skill",
            "prompt": "新固定指示\n第二行", "extensions": ["connect"],
        }),
        _ => panic!("未知模板版本"),
    }
}

/// 检查完整外部启动记录，只有每次独立的会话目录是动态值。
fn assert_snapshot_launch(start: &Value, storage: &TempDir, expected: &[&str], entries: &[&str]) {
    assert_eq!(start["agent_kind"], "omp");
    let args = start["args"].as_array().expect("原生启动参数");
    assert_eq!(
        &args[..4],
        expected
            .iter()
            .map(|value| json!(value))
            .collect::<Vec<_>>()
    );
    assert_eq!(args[4], "--session-dir");
    let session = Path::new(args[5].as_str().expect("会话位置"));
    assert!(session.is_dir(), "会话目录必须实际存在");
    assert!(same_path(
        session.parent().unwrap(),
        &storage.path().join("bot-sessions")
    ));
    let mut expected_tail = vec![json!("--no-extensions")];
    for entry in entries {
        expected_tail.push(json!("--extension"));
        expected_tail.push(json!(storage.path().join(entry)));
    }
    assert_eq!(
        &args[6..],
        expected_tail.as_slice(),
        "只能使用接受时的固定及所选扩展"
    );
}

/// S2 / P36：旧创建停在 Tab 返回之前，另一客户端改模板和真实元数据后新建。
/// 违反样例：阶段推进重新解析映射、只复制模板未复制扩展，或全局队列使新创建等待旧创建。
/// 观察：未释放旧屏障时新创建成功；旧创建在模板已删除、元数据已移除后仍按旧字面配置完成。
#[tokio::test(flavor = "multi_thread")]
async fn p36_accepted_creation_keeps_full_mapping_while_new_creation_finishes_independently() {
    let herdr = HerdrFixture::new("full creation snapshot");
    let tab_release = release_path(&herdr, "release-old-tab");
    let start_release = release_path(&herdr, "release-old-start");
    let prompt_release = release_path(&herdr, "release-old-prompt");
    herdr.set_behavior(json!({
        "tab_create_gate": { "match": "old-旧快照-", "wait_for_file": tab_release, "timeout_ms": 25_000 },
        "agent_start_gate": { "match": "old-provider/old-model", "wait_for_file": start_release, "timeout_ms": 25_000 },
        "prompt_gate": { "match": "旧本次输入", "wait_for_file": prompt_release, "timeout_ms": 25_000 },
    }));
    let storage = TempDir::new("full creation snapshot storage");
    let metadata = write_snapshot_catalog(&storage, "old");
    let mut config = config_with(&herdr, &storage);
    config.bot_templates.metadata_file = metadata.clone();
    let server = TestServer::start(&config).await;
    let zone = storage.path().join("alpha");
    fs::create_dir(&zone).unwrap();
    let zone = register(&server, &zone).await;
    let saved = save_template(&server, snapshot_template("old")).await;
    let template_id = saved["id"].as_str().unwrap();
    let (old_id, old_accepted) =
        accept_creation(&server, &zone, template_id, "旧本次输入\n保留换行").await;
    assert!(
        wait_until(Duration::from_secs(10), || calls_of_kind(
            &herdr,
            "tab_create"
        )
        .len()
            == 1)
        .await
    );
    assert_eq!(creation(&server, &old_id).await["stage"], "starting");
    assert!(herdr.tabs().is_empty(), "旧 Tab 尚未创建，启动参数还未发出");
    assert!(calls_of_kind(&herdr, "agent_start").is_empty());

    write_snapshot_catalog(&storage, "new");
    update_template(&server, template_id, snapshot_template("new")).await;
    let (new_id, _) = accept_creation(&server, &zone, template_id, "新本次输入\n保留换行").await;
    let new_finished = wait_stage(&server, &new_id, "succeeded").await;
    assert!(!tab_release.exists(), "新创建必须在旧创建释放之前完成");
    let old_blocked = creation(&server, &old_id).await;
    assert_eq!(old_blocked["stage"], "starting");
    assert_eq!(old_blocked["inputSubmitted"], false);
    assert!(old_blocked.get("tabId").is_none());
    let (new_tab, new_start, new_prompt) = external_objects(&herdr, &new_finished);
    assert!(
        new_tab["label"]
            .as_str()
            .unwrap()
            .starts_with("new-新快照-")
    );
    assert_eq!(new_tab["cwd"], zone);
    assert_snapshot_launch(
        &new_start,
        &storage,
        &["--model", "new-provider/new-model", "--thinking", "low"],
        &["new-fixed.ts", "new-connect.ts"],
    );
    assert_eq!(
        new_prompt["text"],
        "/skill:new-native 新固定指示\n第二行\n\n新本次输入\n保留换行"
    );

    // 已接受的旧创建不再依赖可变来源；删除模板及元数据也不能触发重读/重配。
    let deleted = request_json_over_tcp(
        server.addr(),
        "DELETE",
        &format!("/api/bot-templates/{template_id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    fs::remove_file(&metadata).unwrap();
    release(&tab_release);
    assert!(
        wait_until(Duration::from_secs(10), || calls_of_kind(
            &herdr,
            "agent_start"
        )
        .len()
            == 2)
        .await
    );
    assert_eq!(creation(&server, &old_id).await["stage"], "starting");
    assert_eq!(
        calls_of_kind(&herdr, "agent_prompt").len(),
        1,
        "旧 agent 就绪前不能提交"
    );
    release(&start_release);
    assert!(
        wait_until(Duration::from_secs(10), || calls_of_kind(
            &herdr,
            "agent_prompt"
        )
        .len()
            == 2)
        .await
    );
    let submitting = creation(&server, &old_id).await;
    assert_eq!(submitting["stage"], "submitting");
    assert_eq!(
        submitting["inputSubmitted"], false,
        "外部提交尚未确认时不能报告成功"
    );
    release(&prompt_release);
    let old_finished = wait_stage(&server, &old_id, "succeeded").await;
    assert_eq!(old_finished["inputSubmitted"], true);
    assert_eq!(old_finished["name"], old_accepted["name"]);
    assert_ne!(old_finished["tabId"], new_finished["tabId"]);
    assert_ne!(old_finished["paneId"], new_finished["paneId"]);
    assert_ne!(old_finished["agentName"], new_finished["agentName"]);
    let (old_tab, old_start, old_prompt) = external_objects(&herdr, &old_finished);
    assert!(
        old_tab["label"]
            .as_str()
            .unwrap()
            .starts_with("old-旧快照-")
    );
    assert_eq!(old_tab["cwd"], zone);
    assert_snapshot_launch(
        &old_start,
        &storage,
        &["--model", "old-provider/old-model", "--thinking", "high"],
        &["old-fixed.ts", "old-tools.ts"],
    );
    assert_ne!(
        old_start["args"][5], new_start["args"][5],
        "并发创建不能复用会话目录"
    );
    assert_eq!(
        old_prompt["text"],
        "/skill:old-native 旧固定指示\n第二行\n\n旧本次输入\n保留换行"
    );
    assert_eq!(herdr.tabs().len(), 2);
    assert_eq!(calls_of_kind(&herdr, "workspace_list").len(), 2);
    assert_eq!(calls_of_kind(&herdr, "tab_create").len(), 2);
    assert_eq!(calls_of_kind(&herdr, "agent_start").len(), 2);
    assert_eq!(calls_of_kind(&herdr, "agent_prompt").len(), 2);
    assert_eq!(herdr.calls().len(), 8, "没有额外重试、改配、关闭或重建调用");
    assert_eq!(creation(&server, &new_id).await, new_finished);
}

/// S2 / P37：先实际创建，再依次编辑、改元数据、删除来源；已有 Bot 的外部对象和调用不变。
/// 违反样例：模板编辑向旧 agent 下发改配/重启，或后续创建误用旧映射/允许已删除来源。
#[tokio::test(flavor = "multi_thread")]
async fn p37_template_and_metadata_changes_leave_existing_bots_untouched() {
    let herdr = HerdrFixture::new("existing bot isolation");
    let storage = TempDir::new("existing bot isolation storage");
    let metadata = write_snapshot_catalog(&storage, "old");
    let mut config = config_with(&herdr, &storage);
    config.bot_templates.metadata_file = metadata;
    let server = TestServer::start(&config).await;
    let zone = storage.path().join("alpha");
    fs::create_dir(&zone).unwrap();
    let zone = register(&server, &zone).await;
    let saved = save_template(&server, snapshot_template("old")).await;
    let template_id = saved["id"].as_str().unwrap();
    let (id, _) = accept_creation(&server, &zone, template_id, "已有 Bot 的输入").await;
    let existing = wait_stage(&server, &id, "succeeded").await;
    let original_calls = herdr.calls();
    let original_tabs = herdr.tabs();
    let (_, original_start, original_prompt) = external_objects(&herdr, &existing);
    assert_snapshot_launch(
        &original_start,
        &storage,
        &["--model", "old-provider/old-model", "--thinking", "high"],
        &["old-fixed.ts", "old-tools.ts"],
    );
    assert_eq!(
        original_prompt["text"],
        "/skill:old-native 旧固定指示\n第二行\n\n已有 Bot 的输入"
    );

    update_template(&server, template_id, snapshot_template("new")).await;
    assert_eq!(herdr.calls(), original_calls, "编辑模板不触碰外部 Bot");
    assert_eq!(herdr.tabs(), original_tabs);
    write_snapshot_catalog(&storage, "new");
    // 经公开读取触发当前元数据加载，而非仅写一个无人读取的文件。
    let read = get_over_tcp(server.addr(), &format!("/api/bot-templates/{template_id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(herdr.calls(), original_calls, "元数据更新不触碰外部 Bot");
    assert_eq!(herdr.tabs(), original_tabs);
    assert_eq!(creation(&server, &id).await, existing);

    // 同一仍有效的来源立即使用新配置，不用重启服务或重新保存映射。
    let (new_id, _) = accept_creation(&server, &zone, template_id, "后续 Bot 的输入").await;
    let new_bot = wait_stage(&server, &new_id, "succeeded").await;
    let (_, new_start, new_prompt) = external_objects(&herdr, &new_bot);
    assert_snapshot_launch(
        &new_start,
        &storage,
        &["--model", "new-provider/new-model", "--thinking", "low"],
        &["new-fixed.ts", "new-connect.ts"],
    );
    assert_eq!(
        new_prompt["text"],
        "/skill:new-native 新固定指示\n第二行\n\n后续 Bot 的输入"
    );
    assert_eq!(
        herdr.calls().len(),
        original_calls.len() + 4,
        "只增加另一次创建的四次外部调用"
    );
    assert_eq!(
        external_objects(&herdr, &existing),
        (original_tabs[0].clone(), original_start, original_prompt)
    );
    let before_delete_calls = herdr.calls();
    let before_delete_tabs = herdr.tabs();
    let stale_preview = preview(&server, &zone, template_id).await;
    let deleted = request_json_over_tcp(
        server.addr(),
        "DELETE",
        &format!("/api/bot-templates/{template_id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    write_snapshot_catalog(&storage, "old");
    assert_eq!(
        get_over_tcp(server.addr(), &format!("/api/bot-templates/{template_id}"))
            .await
            .status,
        404
    );
    let rejected = create(
        &server,
        &create_body(&zone, template_id, &stale_preview, "不能发送的输入"),
    )
    .await;
    assert_eq!(rejected.status, 404, "{}", rejected.body);
    let rejected: Value = serde_json::from_str(&rejected.body).unwrap();
    assert_eq!(rejected["code"], "template-not-found");
    // 不只看页面列表：跨过一段调度窗口，独立外部日志与对象完全相同，两条已完成创建也不变。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        herdr.calls(),
        before_delete_calls,
        "删除来源不重发、改配、关闭或重建已有 Bot"
    );
    assert_eq!(herdr.tabs(), before_delete_tabs);
    assert_eq!(creation(&server, &id).await, existing);
    assert_eq!(creation(&server, &new_id).await, new_bot);
}

/// P51：创建 Bot 不共享固定操作的执行资格；创建停在提交阶段时，固定操作仍被接受并执行。
#[tokio::test(flavor = "multi_thread")]
async fn bot_creation_does_not_take_the_fixed_operation_execution_slot() {
    let herdr = HerdrFixture::new("bot concurrent fixed operations");
    let blocked_release = release_path(&herdr, "release-fixed-operation");
    herdr.set_behavior(json!({
        "prompt_gate": {
            "match": "与固定操作并行",
            "wait_for_file": blocked_release.to_string_lossy(),
            "timeout_ms": 20_000,
        },
    }));
    let fixtures = FixtureTree::new("bot concurrent fixed operations fixtures");
    let storage = TempDir::new("bot concurrent fixed operations storage");
    let server = TestServer::start(&config_with_operations(
        &herdr,
        &storage,
        &fixtures.script("success.bat"),
        &fixtures.operations(),
    ))
    .await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template("并行")).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let (creation_id, _) =
        accept_creation(&server, &zone_path, template_id, "与固定操作并行").await;
    assert!(
        wait_until(Duration::from_secs(10), || {
            calls_of_kind(&herdr, "agent_prompt").len() == 1
        })
        .await
    );
    assert_eq!(creation(&server, &creation_id).await["stage"], "submitting");

    // 创建仍停在提交阶段时，固定操作得到正常接受与执行，而不是忙碌拒绝。
    let operation = post_over_tcp(server.addr(), "/api/operations/start").await;
    assert_eq!(
        operation.status, 200,
        "固定操作不应被创建占用执行资格：{}",
        operation.body
    );
    let recorded = fixtures.records();
    assert_eq!(
        recorded
            .iter()
            .filter(|record| record.stage == "invoke")
            .count(),
        1,
        "固定操作必须真实执行一次：{recorded:#?}"
    );

    release(&blocked_release);
    let finished = wait_stage(&server, &creation_id, "succeeded").await;
    assert_eq!(finished["inputSubmitted"], true);
    assert_eq!(
        external_objects(&herdr, &finished).2["text"],
        "与固定操作并行"
    );
    assert_eq!(herdr.tabs().len(), 1);
}

/// 并发创建的名称仍然遵守各自 zone 的末级目录名，而不是被另一请求覆盖。
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_zone_creations_render_their_own_zone_names() {
    let herdr = HerdrFixture::new("bot concurrent zone names");
    let storage = TempDir::new("bot concurrent zone names storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let one = storage.path().join("one");
    let two = storage.path().join("two");
    fs::create_dir_all(&one).expect("创建 one");
    fs::create_dir_all(&two).expect("创建 two");
    let one_path = register(&server, &one).await;
    let two_path = register(&server, &two).await;
    let template = save_template(&server, default_template("命名")).await;
    let template_id = template["id"].as_str().expect("模板标识");

    let one_preview = preview(&server, &one_path, &template_id).await;
    let two_preview = preview(&server, &two_path, &template_id).await;
    let one_body = create_body(&one_path, &template_id, &one_preview, "命名任务一");
    let two_body = create_body(&two_path, &template_id, &two_preview, "命名任务二");
    let (one_response, two_response) =
        tokio::join!(create(&server, &one_body), create(&server, &two_body));
    let one_creation: Value = serde_json::from_str(&one_response.body).expect("创建回执是 JSON");
    let two_creation: Value = serde_json::from_str(&two_response.body).expect("创建回执是 JSON");
    let one_id = one_creation["id"].as_str().expect("创建标识");
    let two_id = two_creation["id"].as_str().expect("创建标识");
    let one_finished = wait_stage(&server, one_id, "succeeded").await;
    let two_finished = wait_stage(&server, two_id, "succeeded").await;

    let one_bot_id = one_preview["botId"].as_str().expect("预览短标识");
    let two_bot_id = two_preview["botId"].as_str().expect("预览短标识");
    assert_eq!(
        one_finished["name"],
        format!("one · 命名 · {one_bot_id}"),
        "名称必须使用本次 zone 的末级目录名"
    );
    assert_eq!(
        two_finished["name"],
        format!("two · 命名 · {two_bot_id}"),
        "名称必须使用本次 zone 的末级目录名"
    );
    assert_eq!(zone_leaf(&one_path), "one");
    assert_eq!(zone_leaf(&two_path), "two");
    for (view, expected_cwd) in [(&one_finished, &one_path), (&two_finished, &two_path)] {
        let (tab, _, prompt) = external_objects(&herdr, view);
        assert_eq!(tab["label"], view["name"]);
        assert_eq!(&tab["cwd"], expected_cwd);
        assert_eq!(
            prompt["text"],
            if expected_cwd == &one_path {
                "命名任务一"
            } else {
                "命名任务二"
            }
        );
    }
}
