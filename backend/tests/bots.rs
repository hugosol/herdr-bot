//! Bot 的公开 HTTP interface（S2）。
//!
//! 这些用例只经真实 HTTP 请求观察行为：Bot 的保存、读取、编辑与删除。期望值来自独立预写的
//! 字面样例与目标 agent 的固定能力事实，不调用实现的计算路径生成。

mod common;

use std::path::Path;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

/// 把 Bot 存储文件放到隔离的临时目录里；除此之外与交付配置同一形状。
fn config_with_bots(storage: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.bots.storage_file = storage.to_path_buf();
    config
}

/// 响应里的 Bot 列表（按出现顺序）。
fn bots_of(response: &HttpResponse) -> Vec<Value> {
    assert_eq!(response.status, 200, "Bot读取应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("Bot响应必须是 JSON");
    body["bots"]
        .as_array()
        .expect("bots 必须是数组")
        .clone()
}

#[tokio::test]
async fn saves_a_complete_literal_bot_and_independent_clients_read_it() {
    let dir = TempDir::new("bot bot literal");
    let storage = dir.path().join("bots.json");
    let server = TestServer::start(&config_with_bots(&storage)).await;

    // 存储文件尚不存在：空列表不是读取错误。
    assert_eq!(
        bots_of(&get_over_tcp(server.addr(), "/api/bots").await),
        Vec::<Value>::new()
    );

    // 独立预写的完整字面样例：每个字段都覆盖，prompt 含多行与引号。
    let literal = json!({
        "name": "发布检查",
        "chatName": "{zone.name} · {bot.name} · {chat.id}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "max",
        "skill": "code-review",
        "prompt": "第一行：检查登录\n第二行：保留 \"引号\" 与反斜杠 \\\n第三行：最后一行",
    });

    let created =
        post_json_over_tcp(server.addr(), "/api/bots", &literal.to_string()).await;
    assert_eq!(created.status, 200, "保存Bot应成功：{}", created.body);
    let created_body: Value = serde_json::from_str(&created.body).expect("保存响应必须是 JSON");
    let saved = created_body["bots"]
        .as_array()
        .expect("保存响应必须有 bots")
        .first()
        .expect("保存后必须出现一个Bot")
        .clone();

    assert!(
        saved["id"].as_str().is_some_and(|id| !id.is_empty()),
        "Bot必须有非空标识：{saved}"
    );
    for field in [
        "name", "chatName", "agent", "model", "thinking", "skill", "prompt",
    ] {
        assert_eq!(
            saved[field], literal[field],
            "新增Bot必须完整保存字段 {field}：{saved}"
        );
    }

    // 另一条 TCP 连接（独立客户端）读取同一份已保存值。
    let independent = bots_of(&get_over_tcp(server.addr(), "/api/bots").await);
    assert_eq!(
        independent,
        vec![saved.clone()],
        "独立客户端必须读到同一份Bot"
    );
}

#[tokio::test]
async fn edits_in_place_and_deletes_only_the_target_bot() {
    let dir = TempDir::new("bot bot lifecycle");
    let storage = dir.path().join("bots.json");
    let server = TestServer::start(&config_with_bots(&storage)).await;

    let old = json!({
        "name": "旧Bot",
        "chatName": "{zone.name} · {bot.name}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "旧指示",
    });
    let created = post_json_over_tcp(server.addr(), "/api/bots", &old.to_string()).await;
    assert_eq!(created.status, 200, "新增Bot应成功：{}", created.body);
    let created_body: Value = serde_json::from_str(&created.body).expect("保存响应必须是 JSON");
    let id = created_body["bots"][0]["id"]
        .as_str()
        .expect("Bot必须有标识")
        .to_string();

    // 编辑为一份独立预写的新字面样例；标识保持不变。
    let new = json!({
        "name": "新Bot",
        "chatName": "{bot.name} · {agent} · {chat.id}",
        "agent": "omp",
        "model": "gpt-6.1-sol",
        "thinking": "xhigh",
        "skill": "teach",
        "prompt": "第一段\n第二段",
    });
    let updated = common::request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/bots/{id}"),
        Some(&new.to_string()),
    )
    .await;
    assert_eq!(updated.status, 200, "编辑Bot应成功：{}", updated.body);
    let updated_bots = bots_of(&updated);
    assert_eq!(
        updated_bots.len(),
        1,
        "编辑不新增Bot：{updated_bots:?}"
    );
    assert_eq!(updated_bots[0]["id"], json!(id));
    for field in [
        "name", "chatName", "agent", "model", "thinking", "skill", "prompt",
    ] {
        assert_eq!(
            updated_bots[0][field], new[field],
            "编辑必须保存新字面值 {field}：{updated_bots:?}"
        );
    }

    // 独立客户端按标识读取并看到编辑后的同一份值。
    let selected = get_over_tcp(server.addr(), &format!("/api/bots/{id}")).await;
    assert_eq!(selected.status, 200, "按标识读取应成功：{}", selected.body);
    let selected_body: Value = serde_json::from_str(&selected.body).expect("Bot响应必须是 JSON");
    assert_eq!(selected_body["bot"], updated_bots[0]);

    // 再建一个 Bot，删除目标后另一个必须原样保留。
    let keep = json!({
        "name": "保留Bot",
        "chatName": "{zone.name}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "low",
        "skill": "code-review",
        "prompt": "",
    });
    let kept = post_json_over_tcp(server.addr(), "/api/bots", &keep.to_string()).await;
    assert_eq!(kept.status, 200, "第二个Bot应能保存：{}", kept.body);
    let kept_bots = bots_of(&kept);
    let kept = kept_bots
        .iter()
        .find(|bot| bot["name"] == json!("保留Bot"))
        .expect("第二个Bot必须存在")
        .clone();

    let deleted = common::request_json_over_tcp(
        server.addr(),
        "DELETE",
        &format!("/api/bots/{id}"),
        None,
    )
    .await;
    assert_eq!(deleted.status, 200, "删除Bot应成功：{}", deleted.body);
    assert_eq!(bots_of(&deleted), vec![kept.clone()]);

    // 独立客户端再读取与尝试按标识选择：目标 Bot 不可用。
    assert_eq!(
        bots_of(&get_over_tcp(server.addr(), "/api/bots").await),
        vec![kept],
        "删除后独立客户端不得再读到目标Bot"
    );
    let unavailable = get_over_tcp(server.addr(), &format!("/api/bots/{id}")).await;
    assert_eq!(
        unavailable.status, 404,
        "已删除的Bot不能按标识选择：{}",
        unavailable.body
    );
    let unavailable_body: Value =
        serde_json::from_str(&unavailable.body).expect("拒绝响应必须是 JSON");
    assert_eq!(unavailable_body["code"], json!("bot-not-found"));

    // 再次删除与编辑都不存在，且不改变保留的 Bot。
    for response in [
        common::request_json_over_tcp(
            server.addr(),
            "DELETE",
            &format!("/api/bots/{id}"),
            None,
        )
        .await,
        common::request_json_over_tcp(
            server.addr(),
            "PUT",
            &format!("/api/bots/{id}"),
            Some(&new.to_string()),
        )
        .await,
    ] {
        assert_eq!(response.status, 404, "目标Bot已删除：{}", response.body);
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
    let dir = TempDir::new("bot bot rejections");
    let storage = dir.path().join("bots.json");
    let server = TestServer::start(&config_with_bots(&storage)).await;

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
            "bot-name-required",
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
            "bot-name-required",
        ),
        (
            sample("Bot", "", "omp", "default", "default", Value::Null, ""),
            400,
            "bot-chat-name-required",
        ),
        (
            sample(
                "Bot",
                "{zone.name}\n{chat.id}",
                "omp",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "bot-chat-name-not-single-line",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "claude",
                "default",
                "default",
                Value::Null,
                "",
            ),
            400,
            "bot-agent-invalid",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "gpt-5",
                "default",
                Value::Null,
                "",
            ),
            400,
            "bot-model-invalid",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "auto",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-invalid",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "minimal",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-unsupported",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "pi",
                "gpt-6.1-sol",
                "off",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-unsupported",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "deepseek-flash",
                "medium",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-unsupported",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "pi",
                "deepseek-flash",
                "medium",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-unsupported",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "default",
                "high",
                Value::Null,
                "",
            ),
            400,
            "bot-thinking-requires-default-model",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "low",
                json!("review"),
                "",
            ),
            400,
            "bot-skill-invalid",
        ),
        (
            sample(
                "Bot",
                "{zone.name}",
                "omp",
                "gpt-6.1-sol",
                "low",
                json!("teach two"),
                "",
            ),
            400,
            "bot-skill-invalid",
        ),
    ];

    for (case, status, code) in cases {
        let response =
            post_json_over_tcp(server.addr(), "/api/bots", &case.to_string()).await;
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
            "/api/bots",
            &missing_agent.to_string(),
        )
        .await,
        400,
        "bot-agent-required",
    );

    // 所有拒绝都没有改变 Bot 列表（仍是空列表）。
    assert_eq!(
        bots_of(&get_over_tcp(server.addr(), "/api/bots").await),
        Vec::<Value>::new()
    );
}

#[tokio::test]
async fn accepts_every_supported_combination_and_empty_or_multiline_prompt() {
    let dir = TempDir::new("bot bot accepted combinations");
    let storage = dir.path().join("bots.json");
    let server = TestServer::start(&config_with_bots(&storage)).await;

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
                "{zone.name} · {bot.name} · {chat.id}",
                agent,
                model,
                thinking,
                Value::Null,
                "",
            );
            let response =
                post_json_over_tcp(server.addr(), "/api/bots", &payload.to_string()).await;
            assert_eq!(
                response.status, 200,
                "合法组合 {agent}/{model}/{thinking} 应保存成功：{}",
                response.body
            );
            let bots = bots_of(&response);
            let saved = bots
                .iter()
                .find(|bot| bot["name"] == json!(name))
                .expect("新增Bot必须出现在列表中");
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
            post_json_over_tcp(server.addr(), "/api/bots", &payload.to_string()).await;
        assert_eq!(
            response.status, 200,
            "多行 prompt 应保存成功：{}",
            response.body
        );
        let bots = bots_of(&response);
        let saved = bots
            .iter()
            .find(|bot| bot["name"] == json!(name))
            .expect("新增Bot必须出现在列表中");
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

/// 写一份隔离真实进程的配置：回环临时端口、关闭 mDNS、Bot 存储指向临时文件。
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
             [bots]\nstorage_file = '{storage}'\n",
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

/// 预先写定的 Bot 在真实进程停止并重新启动后仍能通过公开入口读取。
#[tokio::test]
async fn predetermined_bots_survive_a_real_process_restart() {
    let dir = TempDir::new("bot bot process restart");
    let storage = dir.path().join("bots.json");
    let config_path = write_process_config(&dir, &storage);

    let old = json!({
        "name": "重启前Bot",
        "chatName": "{zone.name} · {bot.name} · {chat.id}",
        "agent": "omp",
        "model": "gpt-6.1-sol",
        "thinking": "high",
        "skill": "teach",
        "prompt": "重启前的多行指示\n第二行",
    });

    let saved = {
        let first = Panel::start(&config_path);
        let response =
            post_json_over_tcp(first.addr(), "/api/bots", &old.to_string()).await;
        assert_eq!(response.status, 200, "重启前保存应成功：{}", response.body);
        bots_of(&response)
            .into_iter()
            .next()
            .expect("重启前必须保存了一个Bot")
    };
    assert_eq!(saved["name"], old["name"]);

    // 第一个进程已停止；同一个配置文件启动的第二个真实进程从磁盘读取。
    let restarted = Panel::start(&config_path);
    let restored = bots_of(&get_over_tcp(restarted.addr(), "/api/bots").await);
    assert_eq!(
        restored,
        vec![saved.clone()],
        "重启后必须读到同一份已保存值"
    );

    // 按标识读取（独立选择路径）同样恢复字面值。
    let id = saved["id"].as_str().expect("Bot必须有标识");
    let selected = get_over_tcp(restarted.addr(), &format!("/api/bots/{id}")).await;
    assert_eq!(
        selected.status, 200,
        "重启后按标识读取应成功：{}",
        selected.body
    );
    let selected_body: Value = serde_json::from_str(&selected.body).expect("Bot响应必须是 JSON");
    assert_eq!(selected_body["bot"], saved);
}

/// 写入失败是明确错误：不报告成功，也不破坏此前已保存的 Bot；重启后仍是旧值。
#[tokio::test]
async fn write_failure_is_reported_and_keeps_previously_saved_bots() {
    let dir = TempDir::new("bot bot write failure");
    let storage = dir.path().join("bots.json");
    let config_path = write_process_config(&dir, &storage);
    let panel = Panel::start(&config_path);

    let first = json!({
        "name": "已保存Bot",
        "chatName": "{zone.name}",
        "agent": "pi",
        "model": "deepseek-flash",
        "thinking": "high",
        "skill": null,
        "prompt": "已保存",
    });
    let response = post_json_over_tcp(panel.addr(), "/api/bots", &first.to_string()).await;
    assert_eq!(response.status, 200, "第一次保存应成功：{}", response.body);
    let before = bots_of(&get_over_tcp(panel.addr(), "/api/bots").await);

    set_readonly(&storage, true);
    let second = json!({
        "name": "写入失败Bot",
        "chatName": "{zone.name}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    });
    let response =
        post_json_over_tcp(panel.addr(), "/api/bots", &second.to_string()).await;
    let after_failure = bots_of(&get_over_tcp(panel.addr(), "/api/bots").await);
    set_readonly(&storage, false);

    assert_rejected(&response, 500, "bots-write-failed");
    assert_eq!(after_failure, before, "写入失败不得改变已保存的Bot");
    assert!(
        !dir.path().join("bots.json.tmp").exists(),
        "失败的临时写入必须被清理"
    );
    drop(panel);

    // 重启后读取到的仍是写入失败之前的 Bot。
    let restarted = Panel::start(&config_path);
    assert_eq!(
        bots_of(&get_over_tcp(restarted.addr(), "/api/bots").await),
        before,
        "重启后仍是写入失败之前的已保存值"
    );
}

/// 损坏的存储文件是明确错误：不伪装成空列表，也不被保存或重启静默覆盖。
#[tokio::test]
async fn corrupt_storage_is_reported_and_never_overwritten() {
    let dir = TempDir::new("bot bot corrupt storage");
    let storage = dir.path().join("bots.json");
    let corrupt = b"{ this is not a bot store";
    std::fs::write(&storage, corrupt).expect("写入损坏内容");
    let config_path = write_process_config(&dir, &storage);
    let panel = Panel::start(&config_path);

    assert_rejected(
        &get_over_tcp(panel.addr(), "/api/bots").await,
        500,
        "bots-corrupt",
    );

    let payload = json!({
        "name": "候选Bot",
        "chatName": "{zone.name}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    });
    let response =
        post_json_over_tcp(panel.addr(), "/api/bots", &payload.to_string()).await;
    assert_rejected(&response, 500, "bots-corrupt");
    assert_eq!(
        std::fs::read(&storage).expect("存储文件仍应存在"),
        corrupt,
        "保存失败不得覆盖损坏的原文件"
    );
    drop(panel);

    let restarted = Panel::start(&config_path);
    assert_rejected(
        &get_over_tcp(restarted.addr(), "/api/bots").await,
        500,
        "bots-corrupt",
    );
    assert_eq!(
        std::fs::read(&storage).expect("存储文件仍应存在"),
        corrupt,
        "重启不得修复或覆盖损坏的原文件"
    );
}
