//! Chat 实例身份、Task 快照与原生存储选择的公开接缝（HTTP / Herdr 控制）验收。
//!
//! 期望值来自独立预写的字面样例、真实的临时目录，以及受控 Herdr 夹具自己写下的外部调用记录；
//! 不调用 herdr-bot 的命名函数生成期望，也不把受控记录当作真实 Herdr / agent 已验证。真实 Herdr
//! 下的账号、模型、终端与会话文件仍属于人工 H1 覆盖。

mod common;

use std::fs;
use std::path::Path;
use std::time::Duration;

use common::{
    HerdrFixture, HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp,
    request_json_over_tcp, same_path,
};
use serde_json::{Value, json};

const STAGE_DEADLINE: Duration = Duration::from_secs(15);

/// 与交付配置同一形状；Herdr 可执行文件指向受控副本，目标 Workspace 取显式名称。
fn config_with(herdr: &HerdrFixture, storage_dir: &TempDir) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = storage_dir.path().join("zones.json");
    config.tasks.storage_file = storage_dir.path().join("tasks.json");
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

/// 保存一个 Task 并返回本次保存的完整对象（按预写的显示名匹配，不取列表第一项）。
async fn save_task(server: &TestServer, task: Value) -> Value {
    let saved_name = task["name"]
        .as_str()
        .expect("Task必须有显示名")
        .to_string();
    let response =
        post_json_over_tcp(server.addr(), "/api/tasks", &task.to_string()).await;
    assert_eq!(response.status, 200, "保存Task应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("Task响应是 JSON");
    body["tasks"]
        .as_array()
        .expect("Task响应必须有 tasks")
        .iter()
        .find(|saved| saved["name"] == saved_name.as_str())
        .expect("保存后必须包含刚保存的Task")
        .clone()
}

/// 由另一个独立 HTTP 客户端编辑 Task，返回完整已保存对象。
async fn edit_task(server: &TestServer, id: &str, task: Value) -> HttpResponse {
    request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/tasks/{id}"),
        Some(&task.to_string()),
    )
    .await
}

async fn preview(server: &TestServer, zone: &str, task_id: &str) -> Value {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/chats/preview",
        &json!({ "zone": zone, "taskId": task_id }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "预览应成功：{}", response.body);
    serde_json::from_str(&response.body).expect("预览响应是 JSON")
}

/// 创建请求沿用预览的名称与短标识。
fn create_body(zone: &str, task_id: &str, preview: &Value, input: &str) -> Value {
    json!({
        "zone": zone,
        "taskId": task_id,
        "chatId": preview["chatId"],
        "input": input,
        "expectedName": preview["name"],
    })
}

async fn create(server: &TestServer, body: Value) -> HttpResponse {
    post_json_over_tcp(server.addr(), "/api/chats", &body.to_string()).await
}

async fn creation(server: &TestServer, id: &str) -> Value {
    let response = get_over_tcp(server.addr(), &format!("/api/chats/{id}")).await;
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

/// 预览、创建并等待成功；返回创建结果。
async fn create_succeeded(
    server: &TestServer,
    zone: &str,
    task_id: &str,
    input: &str,
) -> Value {
    let preview = preview(server, zone, task_id).await;
    let response = create(server, create_body(zone, task_id, &preview, input)).await;
    assert_eq!(
        response.status, 202,
        "创建应被接受并返回后端拥有的生命周期：{}",
        response.body
    );
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建回执必须有生命周期标识");
    wait_stage(server, id, "succeeded").await
}

fn control_calls(herdr: &HerdrFixture) -> Vec<Value> {
    // Zone history observes native identity without controlling the Chat.
    herdr.calls().into_iter().filter(|call| call["kind"] != "agent_get").collect()
}

fn calls_of_kind(herdr: &HerdrFixture, kind: &str) -> Vec<Value> {
    herdr
        .calls()
        .into_iter()
        .filter(|call| call["kind"] == kind)
        .collect()
}

fn string_args(call: &Value) -> Vec<String> {
    call["args"]
        .as_array()
        .expect("agent start 记录必须有 args")
        .iter()
        .map(|arg| arg.as_str().expect("启动参数必须是字符串").to_string())
        .collect()
}

fn task(agent: &str, name: &str, chat_name: &str) -> Value {
    json!({
        "name": name,
        "chatName": chat_name,
        "agent": agent,
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    })
}

/// P23/P24，S4（经公开创建 HTTP 驱动受控 Herdr）：原生存储选择留给 agent。
/// 独立期望来自 contract：仅加载扩展，不传目录或强制新建/恢复参数。
/// 具体违规：旧实现分配 bot-sessions 并覆盖 --session-dir；两种 agent 的完整 argv
/// 和真实目录清单都可辨别此违规。真实原生落盘/发现另行隔离运行，不以此冒充。
#[tokio::test(flavor = "multi_thread")]
async fn each_agent_keeps_its_native_storage_selection_without_allocating_directories() {
    let herdr = HerdrFixture::new("task identity sessions");
    let storage = TempDir::new("task identity sessions storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    // 预置同 cwd 的旧对话：这是外部对照，不属于任何一次创建。
    let old_conversation = zone.join("old-conversation.jsonl");
    fs::write(&old_conversation, "{\"type\":\"session\",\"id\":\"old\"}\n").expect("预置旧对话");

    let mut names = Vec::new();
    for (agent, task_name) in [("omp", "会话 OMP"), ("pi", "会话 Pi")] {
        let task = save_task(
            &server,
            task(agent, task_name, "{zone.name} · 新对话"),
        )
        .await;
        let task_id = task["id"].as_str().expect("Task标识");
        let finished = create_succeeded(&server, &zone_path, task_id, "独立任务").await;
        names.push(
            finished["agentName"]
                .as_str()
                .expect("必须记录控制别名")
                .to_string(),
        );
    }

    let starts = calls_of_kind(&herdr, "agent_start");
    assert_eq!(starts.len(), 2, "每个 Chat 都必须独立启动一次");
    let kinds: Vec<&str> = starts
        .iter()
        .map(|start| start["agent_kind"].as_str().expect("agent 类型"))
        .collect();
    assert_eq!(kinds, vec!["omp", "pi"], "OMP 与 Pi 都必须验证原生存储选择");
    assert_ne!(names[0], names[1], "两个 Chat 的控制别名不得相同");

    for start in &starts {
        let args = string_args(start);
        assert_eq!(args[0], "--no-extensions", "不得覆盖 agent 原生存储选择：{args:?}");
        assert!(args[1..].chunks(2).all(|pair| pair.len() == 2 && pair[0] == "--extension"),
            "默认Task只显式选择扩展：{args:?}");
    }
    assert!(!storage.path().join("bot-sessions").exists(), "不得预分配原生目录");
    assert_eq!(fs::read(&old_conversation).unwrap(), b"{\"type\":\"session\",\"id\":\"old\"}\n");
    assert_eq!(fs::read_dir(&zone).unwrap().count(), 1);
}

/// P12 / P24：移除命名规则中的实例标识后连续创建，两个对象显示名完全相同且各自独立。
///
/// 身份取自受控 Herdr 返回的 Tab / Pane 与内部生成的 agent 控制别名，不按显示名定位。
#[tokio::test(flavor = "multi_thread")]
async fn two_creations_in_one_zone_are_distinct_objects_under_the_same_display_name() {
    let herdr = HerdrFixture::new("task identity same name");
    let storage = TempDir::new("task identity same name storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let task = save_task(&server, task("omp", "同名Task", "{zone.name} · 同名")).await;
    let task_id = task["id"].as_str().expect("Task标识");

    // 两次独立预览产生同一个预写字面名称，没有自动后缀。
    let first_preview = preview(&server, &zone_path, task_id).await;
    let second_preview = preview(&server, &zone_path, task_id).await;
    assert_eq!(first_preview["name"], "alpha · 同名");
    assert_eq!(second_preview["name"], "alpha · 同名");

    let first = create_succeeded(&server, &zone_path, task_id, "第一任务").await;
    let second = create_succeeded(&server, &zone_path, task_id, "第二任务").await;

    let tabs = herdr.tabs();
    assert_eq!(tabs.len(), 2, "两次创建必须各自新建 Tab");
    assert_eq!(tabs[0]["label"], "alpha · 同名");
    assert_eq!(tabs[1]["label"], "alpha · 同名");
    assert_eq!(
        tabs[0]["label"], tabs[1]["label"],
        "命名规则去掉实例标识后不得自动追加去重后缀"
    );
    assert_ne!(tabs[0]["tab_id"], tabs[1]["tab_id"], "Tab 对象不得复用");
    assert_ne!(tabs[0]["pane_id"], tabs[1]["pane_id"], "初始 Pane 不得复用");
    assert_eq!(first["tabId"], tabs[0]["tab_id"]);
    assert_eq!(first["paneId"], tabs[0]["pane_id"]);
    assert_eq!(second["tabId"], tabs[1]["tab_id"]);
    assert_eq!(second["paneId"], tabs[1]["pane_id"]);

    let starts = calls_of_kind(&herdr, "agent_start");
    assert_eq!(starts.len(), 2, "每个 Chat 必须独立启动 agent");
    assert_ne!(starts[0]["name"], starts[1]["name"], "控制别名不得互相覆盖");
    assert_ne!(starts[0]["pane"], starts[1]["pane"], "不得复用已有 Pane");
    assert_eq!(first["agentName"], starts[0]["name"]);
    assert_eq!(second["agentName"], starts[1]["name"]);
    for start in &starts {
        let alias = start["name"].as_str().expect("控制别名是字符串");
        assert!(
            !alias.is_empty() && alias.len() <= 32,
            "Herdr 控制别名长度受限：{alias}"
        );
        assert!(
            alias
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase()),
            "Herdr 控制别名必须以小写字母开头：{alias}"
        );
        assert!(
            alias.chars().all(|character| character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'),
            "Herdr 控制别名只允许 [a-z0-9_-]：{alias}"
        );
        assert_ne!(
            Some(alias),
            tabs[0]["label"].as_str(),
            "控制别名不得直接使用显示名"
        );
    }

    let prompts = calls_of_kind(&herdr, "agent_prompt");
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0]["target"], starts[0]["name"]);
    assert_eq!(prompts[0]["text"], "第一任务");
    assert_eq!(prompts[1]["target"], starts[1]["name"]);
    assert_eq!(prompts[1]["text"], "第二任务");
    assert_ne!(
        prompts[0]["target"], prompts[1]["target"],
        "两次投递必须按各自的控制别名定位，不能靠显示名"
    );
}

/// P13：两次同名创建使用不同的预写配置与普通任务，各自目标收到属于自己的启动配置和输入。
#[tokio::test(flavor = "multi_thread")]
async fn same_name_creations_receive_their_own_configuration_and_input() {
    let herdr = HerdrFixture::new("task identity same name configs");
    let storage = TempDir::new("task identity same name configs storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    let omp = save_task(
        &server,
        json!({
            "name": "OMP 配置",
            "chatName": "{zone.name} · 同名",
            "agent": "omp",
            "model": "deepseek-flash",
            "thinking": "high",
            "skill": null,
            "prompt": "OMP 固定指示",
        }),
    )
    .await;
    let pi = save_task(
        &server,
        json!({
            "name": "Pi 配置",
            "chatName": "{zone.name} · 同名",
            "agent": "pi",
            "model": "gpt-6.1-sol",
            "thinking": "medium",
            "skill": null,
            "prompt": "Pi 固定指示",
        }),
    )
    .await;
    let omp_id = omp["id"].as_str().expect("OMP Task标识");
    let pi_id = pi["id"].as_str().expect("Pi Task标识");

    let omp_preview = preview(&server, &zone_path, omp_id).await;
    let pi_preview = preview(&server, &zone_path, pi_id).await;
    assert_eq!(omp_preview["name"], "alpha · 同名");
    assert_eq!(pi_preview["name"], "alpha · 同名");

    let omp_finished = create_succeeded(&server, &zone_path, omp_id, "OMP 本次任务").await;
    let pi_finished = create_succeeded(&server, &zone_path, pi_id, "Pi 本次任务").await;

    let tabs = herdr.tabs();
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0]["label"], "alpha · 同名");
    assert_eq!(tabs[1]["label"], "alpha · 同名");
    assert_ne!(tabs[0]["tab_id"], tabs[1]["tab_id"]);
    assert_ne!(tabs[0]["pane_id"], tabs[1]["pane_id"]);

    let starts = calls_of_kind(&herdr, "agent_start");
    let omp_start = starts
        .iter()
        .find(|start| start["agent_kind"] == "omp")
        .expect("必须启动 OMP");
    let pi_start = starts
        .iter()
        .find(|start| start["agent_kind"] == "pi")
        .expect("必须启动 Pi");
    assert_ne!(omp_start["name"], pi_start["name"], "别名不得互相覆盖");
    assert_eq!(omp_finished["agentName"], omp_start["name"]);
    assert_eq!(pi_finished["agentName"], pi_start["name"]);

    let omp_args = string_args(omp_start);
    assert_eq!(
        &omp_args[..4],
        ["--model", "deepseek/deepseek-flash", "--thinking", "high"],
        "OMP 必须收到自己Task的预写配置"
    );
    assert_eq!(omp_args[4], "--no-extensions");

    let pi_args = string_args(pi_start);
    assert_eq!(
        &pi_args[..4],
        [
            "--model",
            "openai-codex/gpt-6.1-sol",
            "--thinking",
            "medium"
        ],
        "Pi 必须收到自己Task的预写配置"
    );
    assert_eq!(pi_args[4], "--no-extensions");

    let prompts = calls_of_kind(&herdr, "agent_prompt");
    let omp_prompt = prompts
        .iter()
        .find(|prompt| prompt["target"] == omp_start["name"])
        .expect("OMP 必须收到自己的初始指示");
    let pi_prompt = prompts
        .iter()
        .find(|prompt| prompt["target"] == pi_start["name"])
        .expect("Pi 必须收到自己的初始指示");
    assert_eq!(omp_prompt["text"], "OMP 固定指示\n\nOMP 本次任务");
    assert_eq!(pi_prompt["text"], "Pi 固定指示\n\nPi 本次任务");
}

/// P3 / P40：一次已接受的创建被挂起时，另一独立客户端编辑 Task；本次仍按旧字面快照执行，
/// 下一次创建才使用新配置。
#[tokio::test(flavor = "multi_thread")]
async fn an_accepted_creation_keeps_its_task_snapshot_while_another_client_edits_it() {
    let herdr = HerdrFixture::new("task identity snapshot");
    herdr.set_behavior(json!({ "agent_start_delay_ms": 1_200 }));
    let storage = TempDir::new("task identity snapshot storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    let old = save_task(
        &server,
        json!({
            "name": "旧Task",
            "chatName": "{zone.name} · {task.name}",
            "agent": "omp",
            "model": "deepseek-flash",
            "thinking": "high",
            "skill": "teach",
            "prompt": "旧固定指示 第一行\n旧固定指示 第二行",
        }),
    )
    .await;
    let task_id = old["id"].as_str().expect("Task标识");

    let old_preview = preview(&server, &zone_path, task_id).await;
    assert_eq!(old_preview["name"], "alpha · 旧Task");
    let accepted = create(
        &server,
        create_body(
            &zone_path,
            task_id,
            &old_preview,
            "旧本次任务 第一行\n旧本次任务 第二行",
        ),
    )
    .await;
    assert_eq!(accepted.status, 202, "创建应被接受：{}", accepted.body);
    let accepted: Value = serde_json::from_str(&accepted.body).expect("创建回执是 JSON");
    let creation_id = accepted["id"].as_str().expect("创建标识").to_string();

    // 启动已经被受控 Herdr 记录，但仍在延迟中：此时由独立 HTTP 客户端编辑 Task。
    assert!(
        common::wait_until(Duration::from_secs(5), || {
            !calls_of_kind(&herdr, "agent_start").is_empty()
        })
        .await,
        "受控 Herdr 必须已经收到 agent start"
    );
    let edited = edit_task(
        &server,
        task_id,
        json!({
            "name": "新Task",
            "chatName": "{zone.name} · {task.name}",
            "agent": "pi",
            "model": "gpt-6.1-sol",
            "thinking": "medium",
            "skill": "code-review",
            "prompt": "新固定指示",
        }),
    )
    .await;
    assert_eq!(edited.status, 200, "编辑Task应成功：{}", edited.body);

    let finished = wait_stage(&server, &creation_id, "succeeded").await;
    assert_eq!(
        finished["name"], "alpha · 旧Task",
        "本次创建的名称必须保持接受时的快照"
    );
    assert_eq!(herdr.tabs()[0]["label"], "alpha · 旧Task");

    let starts = calls_of_kind(&herdr, "agent_start");
    assert_eq!(starts.len(), 1, "本次创建只启动一次");
    let start = &starts[0];
    assert_eq!(start["agent_kind"], "omp", "必须使用旧快照的 agent");
    let args = string_args(start);
    assert_eq!(
        &args[..4],
        ["--model", "deepseek/deepseek-flash", "--thinking", "high"],
        "必须使用旧快照的 model / thinking"
    );
    let prompt = calls_of_kind(&herdr, "agent_prompt")
        .pop()
        .expect("必须提交旧快照的初始指示");
    assert_eq!(prompt["target"], start["name"]);
    assert_eq!(
        prompt["text"],
        "/skill:teach 旧固定指示 第一行\n旧固定指示 第二行\n\n旧本次任务 第一行\n旧本次任务 第二行",
        "必须按旧快照的 Skill、prompt 与本次 input 提交"
    );

    // 下一次创建才使用编辑后的配置，期望值来自预写的新字面样例。
    let new_preview = preview(&server, &zone_path, task_id).await;
    assert_eq!(new_preview["name"], "alpha · 新Task");
    let next = create_succeeded(&server, &zone_path, task_id, "新任务").await;
    assert_eq!(next["name"], "alpha · 新Task");

    let starts = calls_of_kind(&herdr, "agent_start");
    assert_eq!(starts.len(), 2);
    let next_start = &starts[1];
    assert_eq!(
        next_start["agent_kind"], "pi",
        "下一次必须使用新配置的 agent"
    );
    let next_args = string_args(next_start);
    assert_eq!(
        &next_args[..4],
        [
            "--model",
            "openai-codex/gpt-6.1-sol",
            "--thinking",
            "medium"
        ],
        "下一次必须使用新配置的 model / thinking"
    );
    let prompts = calls_of_kind(&herdr, "agent_prompt");
    assert_eq!(prompts.len(), 2);
    assert_eq!(
        prompts[1]["text"], "/skill:code-review 新固定指示\n\n新任务",
        "下一次必须使用新配置的 Skill 与 prompt"
    );
    assert_eq!(herdr.tabs()[1]["label"], "alpha · 新Task");
}

/// P39：创建后编辑并删除 Task，已有 Chat 的显示名、对象目标与运行不被重写或关闭。
///
/// Task 删除后不可再选的承诺由 01 负责；这里只观察已有 Chat 与外部控制副作用。
#[tokio::test(flavor = "multi_thread")]
async fn editing_and_deleting_a_task_does_not_touch_an_existing_chat() {
    let herdr = HerdrFixture::new("task identity task lifecycle");
    let storage = TempDir::new("task identity task lifecycle storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let task = save_task(
        &server,
        task("omp", "保留Task", "{zone.name} · {chat.id}"),
    )
    .await;
    let task_id = task["id"].as_str().expect("Task标识");

    let finished = create_succeeded(&server, &zone_path, task_id, "已有任务").await;
    let creation_id = finished["id"].as_str().expect("创建标识");
    let calls_before = control_calls(&herdr);
    let tabs_before = herdr.tabs();
    assert_eq!(calls_before.len(), 4, "一次创建只有四项外部控制调用");

    let edited = edit_task(
        &server,
        task_id,
        json!({
            "name": "改名的Task",
            "chatName": "{zone.name} · {task.name}",
            "agent": "pi",
            "model": "gpt-6.1-sol",
            "thinking": "medium",
            "skill": null,
            "prompt": "不应影响已有 Chat",
        }),
    )
    .await;
    assert_eq!(edited.status, 200, "编辑Task应成功：{}", edited.body);

    let deleted = request_json_over_tcp(
        server.addr(),
        "DELETE",
        &format!("/api/tasks/{task_id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status, 200, "删除Task应成功：{}", deleted.body);

    // 给后端任何错误的后台反应留出时间；已有 Chat 不应出现新的控制调用或记录变化。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        control_calls(&herdr),
        calls_before,
        "编辑或删除Task不得产生新的 Herdr 控制调用（包括关闭或改写对象）"
    );
    assert_eq!(
        herdr.tabs(),
        tabs_before,
        "已有 Chat 的显示名、对象目标和 cwd 不得被重写"
    );

    let after = creation(&server, creation_id).await;
    assert_eq!(after["stage"], "succeeded");
    assert_eq!(after["name"], finished["name"]);
    assert_eq!(after["tabId"], finished["tabId"]);
    assert_eq!(after["paneId"], finished["paneId"]);
    assert_eq!(after["agentName"], finished["agentName"]);
    assert_eq!(after["inputSubmitted"], true);
}
