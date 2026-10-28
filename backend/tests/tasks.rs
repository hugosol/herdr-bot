//! Task 的公开 HTTP interface（S2）。
//!
//! 这些用例只经真实 HTTP 请求观察行为：Task 的保存、读取、编辑与删除。期望值来自独立预写的
//! 字面样例与目标 agent 的固定能力事实，不调用实现的计算路径生成。

mod common;

use std::path::Path;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

/// 把 Task 存储文件放到隔离的临时目录里；除此之外与交付配置同一形状。
fn config_with_tasks(storage: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.tasks.storage_file = storage.to_path_buf();
    config
}

/// 响应里的 Task 列表（按出现顺序）。
fn tasks_of(response: &HttpResponse) -> Vec<Value> {
    assert_eq!(response.status, 200, "Task读取应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("Task响应必须是 JSON");
    body["tasks"]
        .as_array()
        .expect("tasks 必须是数组")
        .clone()
}

// P5.01 / S2: the contract requires default normal and omp-only roles.
// A server accepting manager+pi is a plausible bypass of page-only validation;
// observe actual HTTP rejection and the unchanged persisted Task list.
#[tokio::test]
async fn task_types_are_enforced_through_direct_http() {
    let dir = TempDir::new("task types HTTP");
    let server = TestServer::start(&config_with_tasks(&dir.path().join("tasks.json"))).await;
    let mut input = json!({"name":"manager in name is not a role", "chatName":"{task.name}",
        "agent":"pi", "model":"default", "thinking":"", "skill":null, "prompt":"node"});
    let created = post_json_over_tcp(server.addr(), "/api/tasks", &input.to_string()).await;
    let initial = tasks_of(&created);
    assert_eq!(initial[0]["taskType"], "normal");
    let id = initial[0]["id"].as_str().unwrap();
    for role in ["manager", "node"] {
        input["taskType"] = json!(role);
        let rejected = common::request_json_over_tcp(server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&input.to_string())).await;
        assert_eq!(rejected.status, 400, "{}", rejected.body);
        assert!(rejected.body.contains("invalid-task-type"));
        assert_eq!(tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await), initial);
    }
    input["agent"] = json!("omp");
    for role in ["manager", "node", "normal"] {
        input["taskType"] = json!(role);
        let changed = tasks_of(&common::request_json_over_tcp(server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&input.to_string())).await);
        assert_eq!(changed[0]["taskType"], role);
        assert_eq!(changed[0]["id"], id);
        for extensions in [json!(["omp-manager-chat"]), json!(["omp-node-chat"]), json!(["omp-manager-chat", "omp-node-chat"])] {
            input["extensions"] = extensions;
            let rejected = common::request_json_over_tcp(server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&input.to_string())).await;
            assert_eq!(rejected.status, 400, "{}", rejected.body);
            assert!(rejected.body.contains("task-extension-invalid"));
        }
        input["extensions"] = json!([]);
        let mut cancellation = input.clone();
        cancellation["fixedExtensions"] = json!([]);
        let rejected = common::request_json_over_tcp(server.addr(), "PUT", &format!("/api/tasks/{id}"), Some(&cancellation.to_string())).await;
        assert_eq!(rejected.status, 400, "{}", rejected.body);
        assert_eq!(tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await), changed);
    }
    input["taskType"] = json!("administrator");
    let rejected = post_json_over_tcp(server.addr(), "/api/tasks", &input.to_string()).await;
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(rejected.body.contains("invalid-task-type"));
}

#[tokio::test]
async fn saves_a_complete_literal_task_and_independent_clients_read_it() {
    let dir = TempDir::new("task task literal");
    let storage = dir.path().join("tasks.json");
    let server = TestServer::start(&config_with_tasks(&storage)).await;

    // 存储文件尚不存在：空列表不是读取错误。
    assert_eq!(
        tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await),
        Vec::<Value>::new()
    );

    // 独立预写的完整字面样例：每个字段都覆盖，prompt 含多行与引号。
    let literal = json!({
        "name": "发布检查",
        "chatName": "{zone.name} · {task.name} · {chat.id}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "max",
        "skill": "code-review",
        "prompt": "第一行：检查登录\n第二行：保留 \"引号\" 与反斜杠 \\\n第三行：最后一行",
    });

    let created =
        post_json_over_tcp(server.addr(), "/api/tasks", &literal.to_string()).await;
    assert_eq!(created.status, 200, "保存Task应成功：{}", created.body);
    let created_body: Value = serde_json::from_str(&created.body).expect("保存响应必须是 JSON");
    let saved = created_body["tasks"]
        .as_array()
        .expect("保存响应必须有 tasks")
        .first()
        .expect("保存后必须出现一个Task")
        .clone();

    assert!(
        saved["id"].as_str().is_some_and(|id| !id.is_empty()),
        "Task必须有非空标识：{saved}"
    );
    for field in [
        "name", "chatName", "agent", "model", "thinking", "skill", "prompt",
    ] {
        assert_eq!(
            saved[field], literal[field],
            "新增Task必须完整保存字段 {field}：{saved}"
        );
    }

    // 另一条 TCP 连接（独立客户端）读取同一份已保存值。
    let independent = tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await);
    assert_eq!(
        independent,
        vec![saved.clone()],
        "独立客户端必须读到同一份Task"
    );
}

#[tokio::test]
async fn edits_in_place_and_deletes_only_the_target_task() {
    let dir = TempDir::new("task task lifecycle");
    let storage = dir.path().join("tasks.json");
    let server = TestServer::start(&config_with_tasks(&storage)).await;

    let old = json!({
        "name": "旧Task",
        "chatName": "{zone.name} · {task.name}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "旧指示",
    });
    let created = post_json_over_tcp(server.addr(), "/api/tasks", &old.to_string()).await;
    assert_eq!(created.status, 200, "新增Task应成功：{}", created.body);
    let created_body: Value = serde_json::from_str(&created.body).expect("保存响应必须是 JSON");
    let id = created_body["tasks"][0]["id"]
        .as_str()
        .expect("Task必须有标识")
        .to_string();

    // 编辑为一份独立预写的新字面样例；标识保持不变。
    let new = json!({
        "name": "新Task",
        "chatName": "{task.name} · {agent} · {chat.id}",
        "agent": "omp",
        "model": "gpt-6.1-sol",
        "thinking": "xhigh",
        "skill": "teach",
        "prompt": "第一段\n第二段",
    });
    let updated = common::request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/tasks/{id}"),
        Some(&new.to_string()),
    )
    .await;
    assert_eq!(updated.status, 200, "编辑Task应成功：{}", updated.body);
    let updated_tasks = tasks_of(&updated);
    assert_eq!(
        updated_tasks.len(),
        1,
        "编辑不新增Task：{updated_tasks:?}"
    );
    assert_eq!(updated_tasks[0]["id"], json!(id));
    for field in [
        "name", "chatName", "agent", "model", "thinking", "skill", "prompt",
    ] {
        assert_eq!(
            updated_tasks[0][field], new[field],
            "编辑必须保存新字面值 {field}：{updated_tasks:?}"
        );
    }

    // 独立客户端按标识读取并看到编辑后的同一份值。
    let selected = get_over_tcp(server.addr(), &format!("/api/tasks/{id}")).await;
    assert_eq!(selected.status, 200, "按标识读取应成功：{}", selected.body);
    let selected_body: Value = serde_json::from_str(&selected.body).expect("Task响应必须是 JSON");
    assert_eq!(selected_body["task"], updated_tasks[0]);

    // 再建一个 Task，删除目标后另一个必须原样保留。
    let keep = json!({
        "name": "保留Task",
        "chatName": "{zone.name}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "low",
        "skill": "code-review",
        "prompt": "",
    });
    let kept = post_json_over_tcp(server.addr(), "/api/tasks", &keep.to_string()).await;
    assert_eq!(kept.status, 200, "第二个Task应能保存：{}", kept.body);
    let kept_tasks = tasks_of(&kept);
    let kept = kept_tasks
        .iter()
        .find(|task| task["name"] == json!("保留Task"))
        .expect("第二个Task必须存在")
        .clone();

    let deleted = common::request_json_over_tcp(
        server.addr(),
        "DELETE",
        &format!("/api/tasks/{id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status, 200, "删除Task应成功：{}", deleted.body);
    assert_eq!(tasks_of(&deleted), vec![kept.clone()]);

    // 独立客户端再读取与尝试按标识选择：目标 Task 不可用。
    assert_eq!(
        tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await),
        vec![kept],
        "删除后独立客户端不得再读到目标Task"
    );
    let unavailable = get_over_tcp(server.addr(), &format!("/api/tasks/{id}")).await;
    assert_eq!(
        unavailable.status, 404,
        "已删除的Task不能按标识选择：{}",
        unavailable.body
    );
    let unavailable_body: Value =
        serde_json::from_str(&unavailable.body).expect("拒绝响应必须是 JSON");
    assert_eq!(unavailable_body["code"], json!("task-not-found"));

    // 再次删除与编辑都不存在，且不改变保留的 Task。
    for response in [
        common::request_json_over_tcp(
            server.addr(),
            "DELETE",
            &format!("/api/tasks/{id}"),
            None,
        )
        .await,
        common::request_json_over_tcp(
            server.addr(),
            "PUT",
            &format!("/api/tasks/{id}"),
            Some(&new.to_string()),
        )
        .await,
    ] {
        assert_eq!(response.status, 404, "目标Task已删除：{}", response.body);
    }
}

/// 一份可覆盖任意字段的请求样例；默认字段均合法。
fn sample(
    name: &str,
    chat_name: &str,
    agent: &str,
    model: &str,
    thinking: &str,
    skill: Value,
    prompt: &str,
) -> Value {
    json!({
        "name": name,
        "chatName": chat_name,
        "agent": agent,
        "model": model,
        "thinking": thinking,
        "skill": skill,
        "prompt": prompt,
    })
}

fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(
        response.status, status,
        "拒绝状态码不符合预期（期望 {code}）：{}",
        response.body
    );
    let body: Value = serde_json::from_str(&response.body).expect("拒绝响应必须是 JSON");
    assert_eq!(body["code"], json!(code), "拒绝类别不符合预期：{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "拒绝必须给出可读说明：{body}"
    );
}

#[tokio::test]
async fn rejects_invalid_fixed_fields_and_capability_violations() {
    let dir = TempDir::new("task task rejections");
    let storage = dir.path().join("tasks.json");
    let server = TestServer::start(&config_with_tasks(&storage)).await;

    let cases = [
        (
            sample(
                "",
                "{zone.name}",
                "omp",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "task-name-required",
        ),
        (
            sample(
                "   ",
                "{zone.name}",
                "omp",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "task-name-required",
        ),
        (
            sample("Task", "", "omp", "default", "default", Value::Null, ""),
            400,
            "task-chat-name-required",
        ),
        (
            sample(
                "Task",
                "{zone.name}\n{chat.id}",
                "omp",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "task-chat-name-not-single-line",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "claude",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "task-agent-invalid",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "gpt-5",
                "default",
                Value::Null,
                "",
            ),
            400,
            "task-model-invalid",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "auto",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-invalid",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "minimal",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-unsupported",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "pi",
                "gpt-6.1-sol",
                "off",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-unsupported",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "deepseek-flash",
                "medium",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-unsupported",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "pi",
                "deepseek-flash",
                "medium",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-unsupported",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "default",
                "high",
                Value::Null,
                "",
            ),
            400,
            "task-thinking-requires-default-model",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "low",
                json!("review"),
                "",
            ),
            400,
            "task-skill-invalid",
        ),
        (
            sample(
                "Task",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "low",
                json!("teach two"),
                "",
            ),
            400,
            "task-skill-invalid",
        ),
    ];

    for (case, status, code) in cases {
        let response =
            post_json_over_tcp(server.addr(), "/api/tasks", &case.to_string()).await;
        assert_rejected(&response, status, code);
    }

    // agent 缺失是必需的固定选择，不是默认值。
    let missing_agent = json!({
        "name": "缺少 agent",
        "chatName": "{zone.name}",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    });
    assert_rejected(
        &post_json_over_tcp(
            server.addr(),
            "/api/tasks",
            &missing_agent.to_string(),
        )
        .await,
        400,
        "task-agent-required",
    );

    // 所有拒绝都没有改变 Task 列表（仍是空列表）。
    assert_eq!(
        tasks_of(&get_over_tcp(server.addr(), "/api/tasks").await),
        Vec::<Value>::new()
    );
}

#[tokio::test]
async fn accepts_every_supported_combination_and_empty_or_multiline_prompt() {
    let dir = TempDir::new("task task accepted combinations");
    let storage = dir.path().join("tasks.json");
    let server = TestServer::start(&config_with_tasks(&storage)).await;

    // 独立预写的固定能力事实：每个 agent / model 只接受这些显式级别。
    let combinations: [(&str, &str, &[&str]); 4] = [
        (
            "omp",
            "gpt-6.1-sol",
            &["low", "medium", "high", "xhigh", "max"],
        ),
        ("omp", "deepseek-flash", &["low", "high", "max"]),
        (
            "pi",
            "gpt-6.1-sol",
            &["minimal", "low", "medium", "high", "xhigh", "max"],
        ),
        ("pi", "deepseek-flash", &["off", "low", "high", "max"]),
    ];

    let mut index = 0;
    for (agent, model, levels) in combinations {
        for thinking in levels {
            index += 1;
            let name = format!("组合 {index}");
            let payload = sample(
                &name,
                "{zone.name} · {task.name} · {chat.id}",
                agent,
                model,
                thinking,
                Value::Null,
                "",
            );
            let response =
                post_json_over_tcp(server.addr(), "/api/tasks", &payload.to_string()).await;
            assert_eq!(
                response.status, 200,
                "合法组合 {agent}/{model}/{thinking} 应保存成功：{}",
                response.body
            );
            let tasks = tasks_of(&response);
            let saved = tasks
                .iter()
                .find(|task| task["name"] == json!(name))
                .expect("新增Task必须出现在列表中");
            assert_eq!(saved["agent"], json!(agent));
            assert_eq!(saved["model"], json!(model));
            assert_eq!(saved["thinking"], json!(thinking));
            assert_eq!(saved["prompt"], json!(""));
            assert_eq!(saved["skill"], Value::Null);
        }
    }

    // 多行 prompt 与两个固定 Skill 都按字面保存；不新增任何非空组合校验。
    // 公开调用也接受界面显示值 `/teach`、`/code-review`，并规范化为 Skill 名称。
    let literal_prompt = "第一行\n第二行：保留 \"引号\" 与反斜杠 \\\n\n第四行";
    for (submitted, stored) in [
        (Value::Null, Value::Null),
        (json!("teach"), json!("teach")),
        (json!("/code-review"), json!("code-review")),
    ] {
        let name = format!("prompt 样例 {submitted}");
        let payload = sample(
            &name,
            "{zone.name}",
            "pi",
            "gpt-6.1-sol",
            "default",
            submitted,
            literal_prompt,
        );
        let response =
            post_json_over_tcp(server.addr(), "/api/tasks", &payload.to_string()).await;
        assert_eq!(
            response.status, 200,
            "多行 prompt 应保存成功：{}",
            response.body
        );
        let tasks = tasks_of(&response);
        let saved = tasks
            .iter()
            .find(|task| task["name"] == json!(name))
            .expect("新增Task必须出现在列表中");
        assert_eq!(saved["prompt"], json!(literal_prompt));
        assert_eq!(saved["skill"], stored);
    }
}

/// 受测的真实交付进程：启动后从 stdout 取得它实际监听的端口。
struct Panel {
    child: std::process::Child,
    port: u16,
}

impl Panel {
    fn start(config_path: &Path) -> Self {
        use std::process::{Command, Stdio};

        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-bot"))
            .current_dir(common::repo_fixtures_dir().parent().unwrap())
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动 herdr-bot 进程");

        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        for source in [
            Box::new(child.stdout.take().expect("stdout")) as Box<dyn std::io::Read + Send>,
            Box::new(child.stderr.take().expect("stderr")) as Box<dyn std::io::Read + Send>,
        ] {
            let log = std::sync::Arc::clone(&log);
            std::thread::spawn(move || {
                let mut source = source;
                let mut buffer = [0_u8; 4096];
                loop {
                    match source.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => log
                            .lock()
                            .expect("读取日志")
                            .extend_from_slice(&buffer[..read]),
                    }
                }
            });
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let port = loop {
            let text = String::from_utf8_lossy(&log.lock().expect("读取日志")).into_owned();
            if let Some(port) = text.lines().find_map(|line| {
                let rest = line.split("http://").nth(1)?;
                rest.trim_end_matches('/')
                    .rsplit(':')
                    .next()?
                    .parse::<u16>()
                    .ok()
            }) {
                break port;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "herdr-bot 没有在 30 秒内打印监听端口：{text}"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        };

        Self { child, port }
    }

    fn addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], self.port))
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 写一份隔离真实进程的配置：回环临时端口、关闭 mDNS、Task 存储指向临时文件。
fn write_process_config(dir: &TempDir, storage: &Path) -> std::path::PathBuf {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let config_path = dir.path().join("herdr-bot.toml");
    std::fs::write(
        &config_path,
        format!(
            "listen_addr = \"127.0.0.1:0\"\n\n\
             [operations.start]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [operations.stop]\nscript = '{script}'\nworking_directory = '{operations}'\n\n\
             [mdns]\nenabled = false\n\n\
             [herdr]\nworkspace_name = 'bot-playground'\n\n\
             [tasks]\nstorage_file = '{storage}'\n",
            script = script.display(),
            operations = operations.display(),
            storage = storage.display(),
        ),
    )
    .expect("写临时配置");
    config_path
}

/// 改一个文件的只读属性；用来构造「读取仍成功、写入失败」的真实存储条件。
fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = std::fs::metadata(path).expect("读取文件权限").permissions();
    permissions.set_readonly(readonly);
    std::fs::set_permissions(path, permissions).expect("设置文件权限");
}

/// 预先写定的 Task 在真实进程停止并重新启动后仍能通过公开入口读取。
#[tokio::test]
async fn predetermined_tasks_survive_a_real_process_restart() {
    let dir = TempDir::new("task task process restart");
    let storage = dir.path().join("tasks.json");
    let config_path = write_process_config(&dir, &storage);

    let old = json!({
        "name": "重启前Task",
        "chatName": "{zone.name} · {task.name} · {chat.id}",
        "agent": "omp",
        "model": "gpt-6.1-sol",
        "thinking": "high",
        "skill": "teach",
        "prompt": "重启前的多行指示\n第二行",
    });

    let saved = {
        let first = Panel::start(&config_path);
        let response =
            post_json_over_tcp(first.addr(), "/api/tasks", &old.to_string()).await;
        assert_eq!(response.status, 200, "重启前保存应成功：{}", response.body);
        tasks_of(&response)
            .into_iter()
            .next()
            .expect("重启前必须保存了一个Task")
    };
    assert_eq!(saved["name"], old["name"]);

    // 第一个进程已停止；同一个配置文件启动的第二个真实进程从磁盘读取。
    let restarted = Panel::start(&config_path);
    let restored = tasks_of(&get_over_tcp(restarted.addr(), "/api/tasks").await);
    assert_eq!(
        restored,
        vec![saved.clone()],
        "重启后必须读到同一份已保存值"
    );

    // 按标识读取（独立选择路径）同样恢复字面值。
    let id = saved["id"].as_str().expect("Task必须有标识");
    let selected = get_over_tcp(restarted.addr(), &format!("/api/tasks/{id}")).await;
    assert_eq!(
        selected.status, 200,
        "重启后按标识读取应成功：{}",
        selected.body
    );
    let selected_body: Value = serde_json::from_str(&selected.body).expect("Task响应必须是 JSON");
    assert_eq!(selected_body["task"], saved);
}

/// 写入失败是明确错误：不报告成功，也不破坏此前已保存的 Task；重启后仍是旧值。
#[tokio::test]
async fn write_failure_is_reported_and_keeps_previously_saved_tasks() {
    let dir = TempDir::new("task task write failure");
    let storage = dir.path().join("tasks.json");
    let config_path = write_process_config(&dir, &storage);
    let panel = Panel::start(&config_path);

    let first = json!({
        "name": "已保存Task",
        "chatName": "{zone.name}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "high",
        "skill": null,
        "prompt": "已保存",
    });
    let response = post_json_over_tcp(panel.addr(), "/api/tasks", &first.to_string()).await;
    assert_eq!(response.status, 200, "第一次保存应成功：{}", response.body);
    let before = tasks_of(&get_over_tcp(panel.addr(), "/api/tasks").await);

    set_readonly(&storage, true);
    let second = json!({
        "name": "写入失败Task",
        "chatName": "{zone.name}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    });
    let response =
        post_json_over_tcp(panel.addr(), "/api/tasks", &second.to_string()).await;
    let after_failure = tasks_of(&get_over_tcp(panel.addr(), "/api/tasks").await);
    set_readonly(&storage, false);

    assert_rejected(&response, 500, "tasks-write-failed");
    assert_eq!(after_failure, before, "写入失败不得改变已保存的Task");
    assert!(
        !dir.path().join("tasks.json.tmp").exists(),
        "失败的临时写入必须被清理"
    );
    drop(panel);

    // 重启后读取到的仍是写入失败之前的 Task。
    let restarted = Panel::start(&config_path);
    assert_eq!(
        tasks_of(&get_over_tcp(restarted.addr(), "/api/tasks").await),
        before,
        "重启后仍是写入失败之前的已保存值"
    );
}

/// 损坏的存储文件是明确错误：不伪装成空列表，也不被保存或重启静默覆盖。
#[tokio::test]
async fn corrupt_storage_is_reported_and_never_overwritten() {
    let dir = TempDir::new("task task corrupt storage");
    let storage = dir.path().join("tasks.json");
    for corrupt in [b"{ this is not a task store".as_slice(), br#"{"bots":[]}"#.as_slice()] {
        std::fs::write(&storage, corrupt).expect("写入损坏内容");
        let config_path = write_process_config(&dir, &storage);
        let panel = Panel::start(&config_path);

        assert_rejected(
            &get_over_tcp(panel.addr(), "/api/tasks").await,
            500,
            "tasks-corrupt",
        );

        let payload = json!({
            "name": "候选Task",
            "chatName": "{zone.name}",
            "agent": "omp",
            "model": "default",
            "thinking": "default",
            "skill": null,
            "prompt": "",
        });
        let response =
            post_json_over_tcp(panel.addr(), "/api/tasks", &payload.to_string()).await;
        assert_rejected(&response, 500, "tasks-corrupt");
        assert_eq!(
            std::fs::read(&storage).expect("存储文件仍应存在"),
            corrupt,
            "保存失败不得覆盖损坏的原文件"
        );
        drop(panel);

        let restarted = Panel::start(&config_path);
        assert_rejected(
            &get_over_tcp(restarted.addr(), "/api/tasks").await,
            500,
            "tasks-corrupt",
        );
        assert_eq!(
            std::fs::read(&storage).expect("存储文件仍应存在"),
            corrupt,
            "重启不得修复或覆盖损坏的原文件"
        );
    }
}
