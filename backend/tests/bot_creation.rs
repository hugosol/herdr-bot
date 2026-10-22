//! 从 zone 创建 Bot 的公开接缝（S2 HTTP / S3 Herdr 控制）验收。
//!
//! 期望值来自独立预写的字面样例、真实的临时目录与受控 Herdr 夹具自己写下的调用记录；不调用
//! herdr-bot 的命名函数生成页面侧期望，也不把受控记录当作真实 Herdr / agent 已验证。真实 Herdr
//! 下的账号、模型与终端行为仍属于人工 H1 覆盖。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::sse::SseConnection;
use common::{
    HerdrFixture, HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp,
    request_json_over_tcp, same_path, wait_until,
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
    config.bot_templates.storage_file = storage_dir.path().join("bot-templates.json");
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

/// 保存一个模板并返回本次保存的完整对象（按请求中的显示名定位，不假设列表里只有一个）。
///
/// 接口按保存顺序返回完整列表；同一用例连续保存同名模板时，本次保存的在最后。
async fn save_template(server: &TestServer, template: Value) -> Value {
    let name = template["name"]
        .as_str()
        .expect("测试模板必须有显示名")
        .to_string();
    let response =
        post_json_over_tcp(server.addr(), "/api/bot-templates", &template.to_string()).await;
    assert_eq!(response.status, 200, "保存模板应成功：{}", response.body);
    let body: Value = serde_json::from_str(&response.body).expect("模板响应是 JSON");
    body["templates"]
        .as_array()
        .expect("模板响应必须有 templates")
        .iter()
        .rev()
        .find(|saved| saved["name"] == name)
        .unwrap_or_else(|| panic!("保存响应必须包含刚保存的模板 {name}"))
        .clone()
}

/// 经公开读取接口取回一个已保存模板，作为“实际保存的是什么”的独立观察。
async fn read_template(server: &TestServer, id: &str) -> Value {
    let response = get_over_tcp(server.addr(), &format!("/api/bot-templates/{id}")).await;
    assert_eq!(
        response.status, 200,
        "读取已保存模板应成功：{}",
        response.body
    );
    let body: Value = serde_json::from_str(&response.body).expect("模板读取响应是 JSON");
    body["template"].clone()
}

async fn preview(server: &TestServer, zone: &str, template_id: &str) -> HttpResponse {
    post_json_over_tcp(
        server.addr(),
        "/api/bots/preview",
        &json!({ "zone": zone, "templateId": template_id }).to_string(),
    )
    .await
}

async fn create(server: &TestServer, body: Value) -> HttpResponse {
    post_json_over_tcp(server.addr(), "/api/bots", &body.to_string()).await
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
    wait_stage_within(server, id, expected, STAGE_DEADLINE).await
}

/// 与 [`wait_stage`] 相同，但允许超时用例给出超过普通阶段时限的等待窗口。
async fn wait_stage_within(
    server: &TestServer,
    id: &str,
    expected: &str,
    window: Duration,
) -> Value {
    let deadline = tokio::time::Instant::now() + window;
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

fn calls_of_kind(herdr: &HerdrFixture, kind: &str) -> Vec<Value> {
    herdr
        .calls()
        .into_iter()
        .filter(|call| call["kind"] == kind)
        .collect()
}

/// 外部调用记录里的步骤顺序；`unsupported` 表示 herdr-bot 尝试了夹具不认识的命令。
fn call_kinds(herdr: &HerdrFixture) -> Vec<String> {
    herdr
        .calls()
        .into_iter()
        .filter_map(|call| call["kind"].as_str().map(str::to_string))
        .collect()
}

/// 准备一个已登记的真实 zone 与一个已保存的模板，供故障矩阵用例复用。
async fn prepare_zone_and_template(server: &TestServer, storage: &TempDir) -> (String, String) {
    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(server, &zone).await;
    let template = save_template(server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识").to_string();
    (zone_path, template_id)
}

/// 经公开预览与创建入口接受一次创建，返回它的生命周期标识。
async fn accept_creation(
    server: &TestServer,
    zone: &str,
    template_id: &str,
    input: &str,
) -> String {
    let preview_response = preview(server, zone, template_id).await;
    assert_eq!(
        preview_response.status, 200,
        "预览应成功：{}",
        preview_response.body
    );
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let response = create(server, create_body(zone, template_id, &preview, input)).await;
    assert_eq!(response.status, 202, "创建应被接受：{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    accepted["id"].as_str().expect("创建标识").to_string()
}

fn zone_leaf(path: &str) -> String {
    Path::new(path)
        .file_name()
        .expect("zone 目录必须有末级名称")
        .to_string_lossy()
        .into_owned()
}

/// 预写的模板字面样例：默认 model / thinking、无 Skill、固定 prompt 为空。
fn default_template() -> Value {
    json!({
        "name": "审查",
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    })
}

/// 创建请求沿用预览的名称与短标识。
fn create_body(zone: &str, template_id: &str, preview: &Value, input: &str) -> Value {
    json!({
        "zone": zone,
        "templateId": template_id,
        "botId": preview["botId"],
        "input": input,
        "expectedName": preview["name"],
    })
}

/// 预写的多行固定 prompt：内部含 slash 文本，用来确认它不会被再次解析成初始化步骤。
const PREWRITTEN_PROMPT: &str =
    "固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面";

/// 预写的多行用户 input。
const PREWRITTEN_INPUT: &str = "本次任务第一行\n第二行带引号 \"quoted\" 与 & 符号";

/// 无 Skill 时的完整正文：prompt 与 input 以空行分段。
const PREWRITTEN_PLAIN_BODY: &str = "固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面\n\n本次任务第一行\n第二行带引号 \"quoted\" 与 & 符号";

/// `/skill:teach` 的完整提交正文，用于与外部记录逐字比较。
const PREWRITTEN_TEACH_BODY: &str = "/skill:teach 固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面\n\n本次任务第一行\n第二行带引号 \"quoted\" 与 & 符号";

/// `/skill:code-review` 的完整提交正文，用于与外部记录逐字比较。
const PREWRITTEN_CODE_REVIEW_BODY: &str = "/skill:code-review 固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面\n\n本次任务第一行\n第二行带引号 \"quoted\" 与 & 符号";

/// 一次初始输入验收使用的独立环境：受控 Herdr、隔离存储、已登记的真实 zone 与真实 HTTP 服务。
struct InputFixture {
    herdr: HerdrFixture,
    server: TestServer,
    /// 保持隔离存储存活到用例结束；声明在服务之后，确保先停止服务再删除目录。
    _storage: TempDir,
    zone: String,
}

async fn input_fixture(label: &str) -> InputFixture {
    let herdr = HerdrFixture::new(label);
    let storage = TempDir::new(&format!("{label} storage"));
    let server = TestServer::start(&config_with(&herdr, &storage)).await;
    let directory = storage.path().join("alpha");
    fs::create_dir_all(&directory).expect("创建 zone");
    let zone = register(&server, &directory).await;
    InputFixture {
        herdr,
        server,
        _storage: storage,
        zone,
    }
}

/// 预写的模板配置：只改变 Skill 与固定 prompt，其余字段与交付默认值一致。
fn input_template(skill: Option<&str>, prompt: &str) -> Value {
    json!({
        "name": "审查",
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": skill,
        "prompt": prompt,
    })
}

/// 经公开预览与创建入口驱动一次创建，等待到达指定终态。
async fn create_with(
    inputs: &InputFixture,
    template: Value,
    input: &str,
    expected_stage: &str,
) -> Value {
    let saved = save_template(&inputs.server, template).await;
    let template_id = saved["id"].as_str().expect("模板标识");
    let preview_response = preview(&inputs.server, &inputs.zone, template_id).await;
    assert_eq!(
        preview_response.status, 200,
        "预览应成功：{}",
        preview_response.body
    );
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let response = create(
        &inputs.server,
        create_body(&inputs.zone, template_id, &preview, input),
    )
    .await;
    assert_eq!(response.status, 202, "创建应被接受：{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");
    wait_stage(&inputs.server, id, expected_stage).await
}

fn initialize_git(directory: &Path) {
    let status = Command::new("git")
        .arg("init")
        .arg(directory)
        .status()
        .expect("运行 git init");
    assert!(status.success(), "git init 必须成功");
}

#[tokio::test(flavor = "multi_thread")]
async fn creates_from_normal_and_git_zones_in_the_configured_workspace_with_zone_cwd() {
    let herdr = HerdrFixture::new("bot creation zones");
    let storage = TempDir::new("bot creation storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let normal = storage.path().join("普通 目录 alpha");
    let git = storage.path().join("git worktree beta");
    fs::create_dir_all(&normal).expect("创建普通目录");
    fs::create_dir_all(&git).expect("创建 Git 工作树目录");
    initialize_git(&git);

    let normal_path = register(&server, &normal).await;
    let git_path = register(&server, &git).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");

    // 两个 zone 各自创建一次；每次从公开预览取得本次名称与短标识。
    for (zone, input) in [
        (normal_path.as_str(), "普通任务：检查登录"),
        (git_path.as_str(), "Git 任务：检查差异"),
    ] {
        let preview = preview(&server, zone, template_id).await;
        assert_eq!(preview.status, 200, "预览应成功：{}", preview.body);
        let preview: Value = serde_json::from_str(&preview.body).expect("预览响应是 JSON");
        let bot_id = preview["botId"].as_str().expect("预览必须有 botId");
        assert!(
            bots_like_bot_id(bot_id),
            "短实例标识必须是六个小写十六进制字符：{bot_id}"
        );
        let expected_name = format!("{} · 审查 · {bot_id}", zone_leaf(zone));
        assert_eq!(
            preview["name"], expected_name,
            "预览名称必须是预写字面样式的实例"
        );

        let response = create(&server, create_body(zone, template_id, &preview, input)).await;
        assert_eq!(
            response.status, 202,
            "创建应被接受并返回后端拥有的生命周期：{}",
            response.body
        );
        let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
        assert_eq!(accepted["stage"], "starting");
        assert_eq!(accepted["name"], expected_name);
        assert_eq!(accepted["zone"], zone);
        let id = accepted["id"].as_str().expect("创建回执必须有生命周期标识");

        let finished = wait_stage(&server, id, "succeeded").await;
        assert_eq!(finished["name"], expected_name);
        assert_eq!(finished["zone"], zone);
        assert_eq!(finished["inputSubmitted"], true);

        // 外部记录：受控 Herdr 自己写下的 Tab 与调用，不由 herdr-bot 的响应生成。
        let last_tab = herdr
            .tabs()
            .into_iter()
            .last()
            .expect("受控 Herdr 必须记录新建 Tab");
        assert_eq!(
            last_tab["workspace"], "w-target",
            "必须使用配置名称的目标 Workspace"
        );
        assert_eq!(last_tab["cwd"], zone, "cwd 必须是所选 zone 的目录");
        assert_eq!(
            last_tab["label"], expected_name,
            "Tab 显示名必须等于预览名称"
        );
        assert_eq!(finished["tabId"], last_tab["tab_id"]);
        assert_eq!(finished["paneId"], last_tab["pane_id"]);
        assert_ne!(
            finished["tabId"], bot_id,
            "bot.id 是短实例标识，不是 Herdr Tab ID"
        );
        assert!(
            expected_name.contains(bot_id),
            "名称必须包含本次短实例标识：{expected_name}"
        );

        let start = calls_of_kind(&herdr, "agent_start")
            .into_iter()
            .last()
            .expect("必须启动 agent");
        assert_eq!(start["agent_kind"], "omp");
        assert_eq!(start["pane"], last_tab["pane_id"]);
        let args = start["args"].as_array().expect("agent start 的参数是数组");
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--model" || arg == "--thinking"),
            "默认 model / thinking 不传启动覆盖参数：{args:?}"
        );

        assert_eq!(args[0], "--session-dir", "必须显式选择独立的新会话");
        assert!(
            Path::new(args[1].as_str().expect("会话目录是字符串")).is_absolute(),
            "会话目录必须是绝对路径：{args:?}"
        );

        let prompt = calls_of_kind(&herdr, "agent_prompt")
            .into_iter()
            .last()
            .expect("必须提交初始指示");
        assert_eq!(prompt["target"], start["name"]);
        assert_eq!(prompt["text"], input, "本次 input 必须按字面提交");
    }

    // 当前焦点指向另一个 Workspace；两次创建都只使用配置目标。
    for tab in herdr.tabs() {
        assert_eq!(tab["workspace"], "w-target");
    }
    assert!(
        herdr
            .calls()
            .iter()
            .all(|call| call["kind"] != "workspace_list"
                || call["argv"] == json!(["workspace", "list"])),
        "Workspace 查询必须是只读列表"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn per_create_rule_uses_preview_context_without_changing_the_template() {
    let herdr = HerdrFixture::new("bot creation rule override");
    let storage = TempDir::new("bot creation rule override storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;
    // Replacement values deliberately contain recognized placeholders: they stay literal.
    let zone_dir = storage.path().join("alpha-{template.name}-{bot.id}");
    fs::create_dir_all(&zone_dir).expect("创建 zone");
    let zone = register(&server, &zone_dir).await;
    let mut template = default_template();
    template["name"] = json!("审查-{agent}-{bot.id}");
    let saved = save_template(&server, template).await;
    let template_id = saved["id"].as_str().expect("模板标识");
    let response = preview(&server, &zone, template_id).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let preview: Value = serde_json::from_str(&response.body).expect("预览 JSON");
    let bot_id = preview["botId"].as_str().expect("短标识");
    assert_eq!(preview["zoneName"], "alpha-{template.name}-{bot.id}");
    assert_eq!(preview["templateName"], "审查-{agent}-{bot.id}");
    assert_eq!(preview["botNameRule"], saved["botName"]);
    assert_eq!(preview["agent"], "omp");
    assert_eq!(
        preview["name"],
        format!("alpha-{{template.name}}-{{bot.id}} · 审查-{{agent}}-{{bot.id}} · {bot_id}")
    );
    let expected = format!(
        "自定 / {bot_id} / alpha-{{template.name}}-{{bot.id}} / 审查-{{agent}}-{{bot.id}} / omp / {{unknown}} / {bot_id}"
    );
    let mut body = create_body(&zone, template_id, &preview, "检查自定名称");
    body["botNameRule"] =
        json!("自定 / {bot.id} / {zone.name} / {template.name} / {agent} / {unknown} / {bot.id}");
    body["expectedName"] = json!(expected);
    let response = create(&server, body).await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建 JSON");
    assert_eq!(accepted["name"], expected);
    let finished = wait_stage(
        &server,
        accepted["id"].as_str().expect("创建标识"),
        "succeeded",
    )
    .await;
    assert_eq!(finished["name"], expected);
    assert_eq!(herdr.tabs().last().expect("实际 Tab")["label"], expected);
    assert_eq!(read_template(&server, template_id).await, saved);

    // Omission still selects the stored template rule for a later creation.
    let response = self::preview(&server, &zone, template_id).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let next_preview: Value = serde_json::from_str(&response.body).expect("预览 JSON");
    assert_eq!(next_preview["botNameRule"], saved["botName"]);
    let response = create(
        &server,
        create_body(&zone, template_id, &next_preview, "默认规则"),
    )
    .await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建 JSON");
    let finished = wait_stage(
        &server,
        accepted["id"].as_str().expect("创建标识"),
        "succeeded",
    )
    .await;
    assert_eq!(finished["name"], next_preview["name"]);
    assert_eq!(
        herdr.tabs().last().expect("默认规则 Tab")["label"],
        next_preview["name"]
    );
    assert_eq!(read_template(&server, template_id).await, saved);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_invalid_or_mismatched_creation_rules_before_herdr_side_effects() {
    let herdr = HerdrFixture::new("bot creation invalid rule");
    let storage = TempDir::new("bot creation invalid rule storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;
    let (zone, template_id) = prepare_zone_and_template(&server, &storage).await;
    let response = preview(&server, &zone, &template_id).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let preview: Value = serde_json::from_str(&response.body).expect("预览 JSON");
    for (rule, status, code) in [
        ("", 400, "bot-name-rule-required"),
        (" \t ", 400, "bot-name-rule-required"),
        ("first\nsecond", 400, "bot-name-rule-not-single-line"),
        ("first\rsecond", 400, "bot-name-rule-not-single-line"),
        ("changed {bot.id}", 409, "name-preview-stale"),
    ] {
        let mut body = create_body(&zone, &template_id, &preview, "不可提交");
        body["botNameRule"] = json!(rule);
        let response = create(&server, body).await;
        assert_eq!(response.status, status, "{rule:?}: {}", response.body);
        let rejected: Value = serde_json::from_str(&response.body).expect("拒绝 JSON");
        assert_eq!(rejected["code"], code, "{rejected}");
        assert!(
            herdr.calls().is_empty(),
            "拒绝前不能调用 Herdr：{:?}",
            herdr.calls()
        );
        assert!(herdr.tabs().is_empty(), "拒绝不能创建 Tab");
    }
    // A nonblank rule can still render blank for a registered filesystem root.
    let root = Path::new(&zone).ancestors().last().expect("文件系统根目录");
    let root_zone = register(&server, root).await;
    let response = self::preview(&server, &root_zone, &template_id).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let root_preview: Value = serde_json::from_str(&response.body).expect("根目录预览 JSON");
    assert_eq!(root_preview["zoneName"], "");
    let mut body = create_body(&root_zone, &template_id, &root_preview, "不可提交");
    body["botNameRule"] = json!("{zone.name}");
    body["expectedName"] = json!("");
    let response = create(&server, body).await;
    assert_eq!(response.status, 400, "{}", response.body);
    let rejected: Value = serde_json::from_str(&response.body).expect("拒绝 JSON");
    assert_eq!(rejected["code"], "bot-name-required");
    assert!(herdr.calls().is_empty());
    assert!(herdr.tabs().is_empty());
    let saved = read_template(&server, &template_id).await;
    assert_eq!(saved["botName"], default_template()["botName"]);
}

fn bots_like_bot_id(value: &str) -> bool {
    value.len() == 6
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// 四组显式 agent / model / thinking 的预写样例：选择值与外部启动参数分开写死。
///
/// 期望值来自已确认的产品映射和目标源码能力事实，不由 herdr-bot 的映射函数生成：
/// - `openai-codex/gpt-6.1-sol`：OMP `thinking.efforts` 为 low/medium/high/xhigh/max；
///   Pi 1.3.0 的 map 为 off:null、minimal..max，`getSupportedThinkingLevels` 因此为
///   minimal/low/medium/high/xhigh/max（不接受 off）。
/// - `deepseek/deepseek-flash`：OMP 为 low/high/max；Pi 的 map 把 minimal/medium 置为 null，
///   因而支持 off/low/high/max。
/// Pi 的合法级别不含 `auto`；四组样例使用的级别都落在各自 agent / model 的能力集合内。
const EXPLICIT_LAUNCH_CASES: [(&str, &str, &str, [&str; 4]); 4] = [
    (
        "omp",
        "gpt-6.1-sol",
        "high",
        ["--model", "openai-codex/gpt-6.1-sol", "--thinking", "high"],
    ),
    (
        "omp",
        "deepseek-flash",
        "max",
        ["--model", "deepseek/deepseek-flash", "--thinking", "max"],
    ),
    (
        "pi",
        "gpt-6.1-sol",
        "xhigh",
        ["--model", "openai-codex/gpt-6.1-sol", "--thinking", "xhigh"],
    ),
    (
        "pi",
        "deepseek-flash",
        "low",
        ["--model", "deepseek/deepseek-flash", "--thinking", "low"],
    ),
];

/// 从公开入口保存模板、独立读回保存值，再创建一次并核对受控 Herdr 写下的外部启动记录。
async fn assert_launch_from_public_entry(
    server: &TestServer,
    herdr: &HerdrFixture,
    zone: &str,
    agent: &str,
    model: &str,
    thinking: &str,
    expected_args: Value,
    input: &str,
) -> Value {
    let template = json!({
        "name": format!("启动配置 {agent} {model} {thinking}"),
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": agent,
        "model": model,
        "thinking": thinking,
        "skill": null,
        "prompt": "",
    });
    let saved = save_template(server, template.clone()).await;
    let template_id = saved["id"].as_str().expect("模板标识").to_string();

    // 公开保存、独立读回与外部启动参数必须一致：先确认后端实际保存的就是这组值。
    let stored = read_template(server, &template_id).await;
    for field in ["name", "agent", "model", "thinking"] {
        assert_eq!(
            stored[field], template[field],
            "已保存模板的 {field} 必须与公开保存值一致"
        );
    }

    let calls_before = herdr.calls().len();
    let preview_response = preview(server, zone, &template_id).await;
    assert_eq!(
        preview_response.status, 200,
        "预览应成功：{}",
        preview_response.body
    );
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");

    let response = create(server, create_body(zone, &template_id, &preview, input)).await;
    assert_eq!(response.status, 202, "创建应被接受：{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识").to_string();

    let finished = wait_stage(server, &id, "succeeded").await;
    assert_eq!(finished["inputSubmitted"], true);

    let calls = herdr.calls();
    let created_calls = calls
        .get(calls_before..)
        .expect("创建前后的外部调用记录必须连续");
    let tail: Vec<&str> = created_calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        tail,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt"
        ],
        "一次创建只有一次启动与一次提交；启动参数在提交之前确定，没有启动后切换"
    );

    let start = created_calls
        .iter()
        .find(|call| call["kind"] == "agent_start")
        .expect("必须启动 agent");
    assert_eq!(
        start["agent_kind"], agent,
        "外部启动目标类型必须对应所选 agent"
    );
    let expected = expected_args.as_array().expect("预写样例是数组");
    let args = start["args"].as_array().expect("启动参数是数组");

    assert_eq!(
        &args[..expected.len()],
        expected.as_slice(),
        "启动参数中的模型与 thinking 必须与预写样例一致"
    );
    assert_eq!(
        args[expected.len()],
        "--session-dir",
        "模型 / thinking 之后必须显式选择独立的新会话目录：{}",
        start["args"]
    );
    assert!(
        Path::new(args[expected.len() + 1].as_str().expect("会话目录是字符串")).is_absolute(),
        "会话目录必须是绝对路径：{}",
        start["args"]
    );
    if agent == "pi" {
        assert!(
            !args.iter().any(|arg| arg == "auto"),
            "Pi 不接收 auto：{}",
            start["args"]
        );
    }

    let prompt = created_calls
        .iter()
        .find(|call| call["kind"] == "agent_prompt")
        .expect("必须提交初始指示");
    assert_eq!(
        prompt["target"], start["name"],
        "初始指示必须提交给本次启动的 agent"
    );
    assert_eq!(prompt["text"], input, "初始指示按字面提交一次");
    finished
}

/// P14 / P16 / P17：公开入口保存的 omp / pi 与两个显式模型，在受控 Herdr 的外部启动记录中
/// 使用完整 provider / model 与适用的 thinking 原生参数；Pi 的级别不含 auto。
#[tokio::test(flavor = "multi_thread")]
async fn explicit_agents_and_models_reach_herdr_as_native_launch_arguments() {
    let herdr = HerdrFixture::new("bot launch explicit");
    let storage = TempDir::new("bot launch explicit storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    for (index, (agent, model, thinking, expected_args)) in EXPLICIT_LAUNCH_CASES.iter().enumerate()
    {
        let input = format!("显式组合 {index}：{agent}/{model}/{thinking}");
        assert_launch_from_public_entry(
            &server,
            &herdr,
            &zone_path,
            agent,
            model,
            thinking,
            json!(expected_args),
            &input,
        )
        .await;
    }
}

/// P14 / P18：默认 model 的启动不传 model 与 thinking 覆盖；显式模型搭配默认 thinking 时
/// 只传 model。两个 agent 都从公开创建入口观察实际外部参数。
#[tokio::test(flavor = "multi_thread")]
async fn defaulted_model_and_thinking_are_absent_from_native_launch_arguments() {
    let herdr = HerdrFixture::new("bot launch defaults");
    let storage = TempDir::new("bot launch defaults storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    let cases = [
        ("omp", "default", "default", json!([])),
        ("pi", "default", "default", json!([])),
        (
            "omp",
            "gpt-6.1-sol",
            "default",
            json!(["--model", "openai-codex/gpt-6.1-sol"]),
        ),
        (
            "pi",
            "deepseek-flash",
            "default",
            json!(["--model", "deepseek/deepseek-flash"]),
        ),
    ];

    for (index, (agent, model, thinking, expected_args)) in cases.into_iter().enumerate() {
        let input = format!("默认组合 {index}：{agent}/{model}/{thinking}");
        assert_launch_from_public_entry(
            &server,
            &herdr,
            &zone_path,
            agent,
            model,
            thinking,
            expected_args,
            &input,
        )
        .await;
    }
}

/// P27 / P32 / P33 / P34：启动被延迟时不提交；就绪后才进入提交阶段；提交确认后立即成功。
#[tokio::test(flavor = "multi_thread")]
async fn stage_progress_waits_for_readiness_and_does_not_wait_for_the_model_turn() {
    let herdr = HerdrFixture::new("bot creation stages");
    herdr.set_behavior(json!({
        "agent_start_delay_ms": 1_200,
        "prompt_delay_ms": 1_200,
    }));
    let storage = TempDir::new("bot creation stage storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");

    let response = create(
        &server,
        create_body(&zone_path, template_id, &preview, "等待就绪后提交"),
    )
    .await;
    assert_eq!(response.status, 202, "创建应被接受：{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识").to_string();
    assert_eq!(accepted["stage"], "starting", "接受后应处于正在启动");

    // 启动调用已经被受控 Herdr 记录，但它在延迟中：此时不得有任何初始输入。
    assert!(
        wait_until(Duration::from_secs(5), || {
            !calls_of_kind(&herdr, "agent_start").is_empty()
        })
        .await,
        "受控 Herdr 必须已经收到 agent start"
    );
    let during_start = creation(&server, &id).await;
    assert_eq!(
        during_start["stage"], "starting",
        "就绪之前页面只能看到正在启动"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        calls_of_kind(&herdr, "agent_prompt").is_empty(),
        "目标就绪之前不得提交初始指示"
    );

    // 就绪之后才进入提交阶段；提交响应被延迟时，页面稳定看到提交阶段。
    let submitting = wait_stage(&server, &id, "submitting").await;
    assert_eq!(submitting["stage"], "submitting");
    assert_eq!(submitting["inputSubmitted"], false, "提交尚未确认");
    tokio::time::sleep(Duration::from_millis(250)).await;
    let still_submitting = creation(&server, &id).await;
    assert_eq!(
        still_submitting["stage"], "submitting",
        "提交响应被延迟时不能提前显示成功"
    );

    let succeeded = wait_stage(&server, &id, "succeeded").await;
    assert_eq!(succeeded["inputSubmitted"], true);
    assert_eq!(
        calls_of_kind(&herdr, "agent_prompt").len(),
        1,
        "只提交一次，不自动重发"
    );
    // P34：受控 agent 在提交后仍为 working；成功不等待模型轮次结束。
    let prompt = calls_of_kind(&herdr, "agent_prompt")
        .pop()
        .expect("必须有提交记录");
    assert_eq!(prompt["text"], "等待就绪后提交");

    // 时序：启动记录必须先于提交记录。
    let calls = herdr.calls();
    let kinds: Vec<&str> = calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt"
        ],
        "一次创建只有一次 Workspace 查询、一次 Tab、一次启动、一次提交"
    );
}

/// P33：提交被明确拒绝时不假报成功，并保留已创建的 Tab / agent 定位信息。
#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_submission_fails_creation_and_keeps_the_created_objects() {
    let herdr = HerdrFixture::new("bot creation prompt failure");
    herdr.set_behavior(json!({ "fail_prompt": true }));
    let storage = TempDir::new("bot creation prompt failure storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let response = create(
        &server,
        create_body(&zone_path, template_id, &preview, "会被拒绝的指示"),
    )
    .await;
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");

    let failed = wait_stage(&server, id, "failed").await;
    assert_eq!(failed["stage"], "failed");
    assert_eq!(failed["failure"]["stage"], "submit");
    assert_eq!(failed["failure"]["code"], "herdr-command-failed");
    assert_eq!(failed["inputSubmitted"], false);
    assert!(
        failed["tabId"].as_str().is_some(),
        "已创建的 Tab 标识必须保留"
    );
    assert!(
        failed["paneId"].as_str().is_some(),
        "已创建的 Pane 标识必须保留"
    );
    assert!(
        failed["agentName"].as_str().is_some(),
        "已启动的 agent 控制别名必须保留"
    );
    let message = failed["failure"]["message"]
        .as_str()
        .expect("失败必须有可读说明");
    assert!(message.contains("agent 已就绪"), "{message}");
    assert!(message.contains("被明确拒绝"), "{message}");
    assert!(message.contains("保留"), "{message}");
    assert!(
        !message.contains("未确认"),
        "明确拒绝不能表述成结果未确认：{message}"
    );

    // 外部独立记录：agent 已就绪、投递已被夹具收到并拒绝；之后没有重发或清理动作。
    let created_tab = herdr.tabs().pop().expect("受控 Herdr 必须记录已创建的 Tab");
    assert_eq!(failed["tabId"], created_tab["tab_id"]);
    assert_eq!(failed["paneId"], created_tab["pane_id"]);
    let expected_kinds = vec![
        "workspace_list".to_string(),
        "tab_create".to_string(),
        "agent_start".to_string(),
        "agent_prompt".to_string(),
    ];
    assert_eq!(call_kinds(&herdr), expected_kinds);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        call_kinds(&herdr),
        expected_kinds,
        "失败后不得自动重发、重建、关闭或删除现场"
    );
    assert_eq!(herdr.tabs().len(), 1, "已创建的 Tab 仍保留");
}

/// P36 / P37 / P38：投递超时无法确认结果时，只能表述为未确认；现场保留且不重发。
#[tokio::test(flavor = "multi_thread")]
async fn a_timed_out_submission_is_unconfirmed_and_keeps_the_site_without_retrying() {
    let herdr = HerdrFixture::new("bot creation prompt timeout");
    // 夹具先记录这次投递，再睡过 adapter 的提交时限：外部副作用已经发生，结果却未知。
    herdr.set_behavior(json!({ "prompt_delay_ms": 31_000 }));
    let storage = TempDir::new("bot creation prompt timeout storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;
    let (zone_path, template_id) = prepare_zone_and_template(&server, &storage).await;
    let id = accept_creation(&server, &zone_path, &template_id, "超时的指示").await;

    let failed = wait_stage_within(&server, &id, "failed", Duration::from_secs(50)).await;
    assert_eq!(failed["stage"], "failed", "不能显示创建成功");
    assert_eq!(failed["failure"]["stage"], "submit");
    assert_eq!(failed["failure"]["code"], "herdr-timeout");
    assert_eq!(failed["inputSubmitted"], false, "超时不能被说成已确认提交");
    assert!(failed["tabId"].as_str().is_some());
    assert!(failed["paneId"].as_str().is_some());
    assert!(failed["agentName"].as_str().is_some());

    let message = failed["failure"]["message"]
        .as_str()
        .expect("失败必须有可读说明");
    assert!(message.contains("投递结果未确认"), "{message}");
    assert!(message.contains("不能据此断定未发送"), "{message}");
    assert!(
        !message.contains("没有发送"),
        "超时不能表述成确定未发送：{message}"
    );

    // 外部独立记录：投递确实到达过夹具；之后没有重发、重建、关闭或删除。
    let expected_kinds = vec![
        "workspace_list".to_string(),
        "tab_create".to_string(),
        "agent_start".to_string(),
        "agent_prompt".to_string(),
    ];
    assert_eq!(call_kinds(&herdr), expected_kinds);
    assert_eq!(herdr.tabs().len(), 1, "已创建的 Tab 必须保留");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        call_kinds(&herdr),
        expected_kinds,
        "超时后不得自动重发、重建、关闭或删除现场"
    );
    assert_eq!(herdr.tabs().len(), 1, "超时后不得关闭或删除现场");
}

/// P27 / P33：agent 启动失败后绝不提交初始指示；已创建的 Tab / Pane 仍可定位。
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_agent_start_never_submits_input() {
    let herdr = HerdrFixture::new("bot creation start failure");
    herdr.set_behavior(json!({ "fail_agent_start": true }));
    let storage = TempDir::new("bot creation start failure storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let response = create(
        &server,
        create_body(&zone_path, template_id, &preview, "不应被提交"),
    )
    .await;
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");

    let failed = wait_stage(&server, id, "failed").await;
    assert_eq!(failed["failure"]["stage"], "start");
    assert_eq!(failed["failure"]["code"], "herdr-command-failed");
    assert_eq!(failed["inputSubmitted"], false);
    assert!(failed["tabId"].as_str().is_some());
    assert!(failed["paneId"].as_str().is_some());
    assert!(
        failed["agentName"].as_str().is_some(),
        "已尝试启动的 agent 控制别名必须保留"
    );
    let message = failed["failure"]["message"]
        .as_str()
        .expect("失败必须有可读说明");
    assert!(message.contains("Tab 与 Pane 已创建"), "{message}");
    assert!(message.contains("没有提交初始指示"), "{message}");

    // 外部独立记录：Tab 已创建并保留，启动被拒绝后没有任何提交或清理动作。
    assert_eq!(
        call_kinds(&herdr),
        vec![
            "workspace_list".to_string(),
            "tab_create".to_string(),
            "agent_start".to_string(),
        ]
    );
    assert_eq!(herdr.tabs().len(), 1, "已创建的 Tab 必须保留");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        call_kinds(&herdr),
        vec![
            "workspace_list".to_string(),
            "tab_create".to_string(),
            "agent_start".to_string(),
        ],
        "启动失败后不得提交、重试、重建、关闭或删除现场"
    );
}

/// 当前模式只按 zone 根目录名匹配，不回退到默认或焦点 Workspace。
#[tokio::test(flavor = "multi_thread")]
async fn current_workspace_uses_exact_zone_basename_without_fallback() {
    for (label, entries, expected_error) in [
        (
            "unique",
            json!([{"workspace_id":"w-zone","label":"alpha"}]),
            None,
        ),
        ("missing", json!([]), Some("workspace-not-found")),
        (
            "case",
            json!([{"workspace_id":"w-case","label":"Alpha"}]),
            Some("workspace-not-found"),
        ),
        (
            "duplicate",
            json!([
                {"workspace_id":"w-one","label":"alpha"},
                {"workspace_id":"w-two","label":"alpha"}
            ]),
            Some("workspace-ambiguous"),
        ),
    ] {
        let fixture = input_fixture(&format!("current workspace {label}")).await;
        let mut workspaces = entries.as_array().unwrap().clone();
        workspaces
            .push(json!({"workspace_id":"w-default","label":"bot-playground","focused":true}));
        fixture.herdr.set_workspaces(json!(workspaces));
        let template = save_template(&fixture.server, default_template()).await;
        let template_id = template["id"].as_str().unwrap();
        let response = preview(&fixture.server, &fixture.zone, template_id).await;
        let preview: Value = serde_json::from_str(&response.body).unwrap();
        let mut body = create_body(&fixture.zone, template_id, &preview, "测试输入");
        body["workspaceTarget"] = json!("current");
        let response = create(&fixture.server, body).await;
        assert_eq!(response.status, 202, "{}", response.body);
        let accepted: Value = serde_json::from_str(&response.body).unwrap();
        let result = wait_stage(
            &fixture.server,
            accepted["id"].as_str().unwrap(),
            if expected_error.is_some() {
                "failed"
            } else {
                "succeeded"
            },
        )
        .await;
        if let Some(code) = expected_error {
            assert_eq!(result["failure"]["code"], code);
            assert!(
                result["failure"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("alpha")
            );
            assert_eq!(call_kinds(&fixture.herdr), vec!["workspace_list"]);
            assert!(fixture.herdr.tabs().is_empty());
        } else {
            let tabs = fixture.herdr.tabs();
            assert_eq!(tabs.len(), 1);
            assert_eq!(tabs[0]["workspace"], "w-zone");
            assert_eq!(tabs[0]["cwd"], fixture.zone);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn workspace_retry_reuses_preview_but_preserves_independent_creation_records() {
    let fixture = input_fixture("workspace retry identity").await;
    fixture.herdr.set_workspaces(json!([
        {"workspace_id":"w-default","label":"bot-playground"}
    ]));
    let template = save_template(&fixture.server, default_template()).await;
    let template_id = template["id"].as_str().unwrap();
    let response = preview(&fixture.server, &fixture.zone, template_id).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let preview: Value = serde_json::from_str(&response.body).unwrap();
    let mut body = create_body(&fixture.zone, template_id, &preview, "保留重试输入");
    body["workspaceTarget"] = json!("current");
    let response = create(&fixture.server, body.clone()).await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).unwrap();
    let failed_id = accepted["id"].as_str().unwrap();
    assert_ne!(accepted["id"], preview["botId"], "创建标识不是显示名短标识");
    let failed = wait_stage(&fixture.server, failed_id, "failed").await;
    assert_eq!(failed["failure"]["stage"], "workspace");
    assert_eq!(failed["failure"]["code"], "workspace-not-found");
    assert_eq!(call_kinds(&fixture.herdr), vec!["workspace_list"]);
    assert!(fixture.herdr.tabs().is_empty());

    // 同一目标也可主动重试；不要求用户先切换 Workspace，且旧失败不能被覆盖。
    let response = create(&fixture.server, body.clone()).await;
    assert_eq!(response.status, 202, "{}", response.body);
    let repeated: Value = serde_json::from_str(&response.body).unwrap();
    let repeated_id = repeated["id"].as_str().unwrap();
    assert_ne!(repeated["id"], accepted["id"]);
    wait_stage(&fixture.server, repeated_id, "failed").await;
    assert_eq!(creation(&fixture.server, failed_id).await, failed);
    assert_eq!(
        call_kinds(&fixture.herdr),
        vec!["workspace_list", "workspace_list"]
    );

    // 只改 Workspace；名称、botId、模板与 input 沿用同一次预览，不产生会话目录冲突。
    body["workspaceTarget"] = json!("default");
    let response = create(&fixture.server, body).await;
    assert_eq!(response.status, 202, "{}", response.body);
    let retry: Value = serde_json::from_str(&response.body).unwrap();
    let retry_id = retry["id"].as_str().unwrap();
    assert_ne!(retry["id"], accepted["id"]);
    assert_ne!(retry["id"], repeated["id"]);
    let succeeded = wait_stage(&fixture.server, retry_id, "succeeded").await;
    assert_eq!(succeeded["name"], failed["name"]);
    assert_eq!(succeeded["name"], preview["name"]);
    assert_eq!(succeeded["templateId"], failed["templateId"]);
    assert_eq!(creation(&fixture.server, failed_id).await, failed);
    assert_eq!(
        creation(&fixture.server, repeated_id).await["stage"],
        "failed"
    );
    assert_eq!(fixture.herdr.tabs().len(), 1);
    assert_eq!(fixture.herdr.tabs()[0]["workspace"], "w-default");
    let prompts = calls_of_kind(&fixture.herdr, "agent_prompt");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0]["text"], "保留重试输入");
    let start = calls_of_kind(&fixture.herdr, "agent_start");
    assert_eq!(start.len(), 1);
    let session_dir = start[0]["args"][1].as_str().expect("独立会话目录");
    assert_eq!(
        Path::new(session_dir)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        retry_id
    );
    assert_eq!(
        call_kinds(&fixture.herdr),
        vec![
            "workspace_list",
            "workspace_list",
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt",
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn current_workspace_rejects_root_without_a_directory_name() {
    let fixture = input_fixture("current workspace root").await;
    let root = Path::new(&fixture.zone).ancestors().last().unwrap();
    let zone = register(&fixture.server, root).await;
    let template = save_template(&fixture.server, default_template()).await;
    let template_id = template["id"].as_str().unwrap();
    let response = preview(&fixture.server, &zone, template_id).await;
    let preview: Value = serde_json::from_str(&response.body).unwrap();
    let mut body = create_body(&zone, template_id, &preview, "不应提交");
    body["workspaceTarget"] = json!("current");
    let response = create(&fixture.server, body).await;
    assert_eq!(response.status, 400, "{}", response.body);
    let error: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(error["code"], "workspace-name-unavailable");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("请选择默认 Workspace")
    );
    assert!(fixture.herdr.calls().is_empty());
}

/// P41 / P46（基础失败）：找不到目标 Workspace 或存在同名歧义时不新建 Tab，也不伪造标识。
#[tokio::test(flavor = "multi_thread")]
async fn workspace_mismatches_fail_without_creating_a_tab() {
    for (label, workspaces, expected_code) in [
        (
            "missing",
            json!([{"workspace_id":"w-other","label":"other","focused":true}]),
            "workspace-not-found",
        ),
        (
            "ambiguous",
            json!([
                {"workspace_id":"w-one","label":"bot-playground","focused":true},
                {"workspace_id":"w-two","label":"bot-playground","focused":false}
            ]),
            "workspace-ambiguous",
        ),
    ] {
        let herdr = HerdrFixture::new(&format!("bot creation workspace {label}"));
        herdr.set_workspaces(workspaces);
        let storage = TempDir::new(&format!("bot creation workspace {label} storage"));
        let server = TestServer::start(&config_with(&herdr, &storage)).await;

        let zone = storage.path().join("alpha");
        fs::create_dir_all(&zone).expect("创建 zone");
        let zone_path = register(&server, &zone).await;
        let template = save_template(&server, default_template()).await;
        let template_id = template["id"].as_str().expect("模板标识");
        let preview_response = preview(&server, &zone_path, template_id).await;
        let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
        let response = create(
            &server,
            create_body(&zone_path, template_id, &preview, "不应被提交"),
        )
        .await;
        let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
        let id = accepted["id"].as_str().expect("创建标识");

        let failed = wait_stage(&server, id, "failed").await;
        assert_eq!(failed["failure"]["stage"], "workspace");
        assert_eq!(failed["failure"]["code"], expected_code);
        assert_eq!(failed["inputSubmitted"], false);
        assert!(
            failed.get("tabId").is_none(),
            "没有新建 Tab 时不得伪造 Tab 标识：{failed}"
        );
        assert!(
            failed.get("paneId").is_none(),
            "没有新建 Pane 时不得伪造 Pane 标识：{failed}"
        );
        assert!(
            failed.get("agentName").is_none(),
            "没有启动 agent 时不得伪造控制别名：{failed}"
        );
        let message = failed["failure"]["message"]
            .as_str()
            .expect("失败必须有可读说明");
        assert!(message.contains("没有新建 Tab"), "{message}");

        // 外部独立记录：只有一次只读列表查询；没有新建 Workspace、没有启动 Herdr、
        // 没有回退到焦点或其他名称相近的 Workspace，也没有重试。
        assert_eq!(
            call_kinds(&herdr),
            vec!["workspace_list".to_string()],
            "{label}：Workspace 不唯一时必须停止，不能任取一个或改换目标"
        );
        assert!(herdr.tabs().is_empty());
        assert!(calls_of_kind(&herdr, "tab_create").is_empty());
        assert!(calls_of_kind(&herdr, "agent_prompt").is_empty());
    }
}

/// P42：Herdr 不可达时经公开创建入口失败；没有 Tab、agent 或 Workspace 副作用。
#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_herdr_fails_before_any_tab_or_agent_control() {
    let herdr = HerdrFixture::new("bot creation unreachable herdr");
    let storage = TempDir::new("bot creation unreachable storage");
    let mut config = config_with(&herdr, &storage);
    // 可执行文件不存在就是生产 adapter 的“无法启动或连接 Herdr”路径；不是命令被拒绝。
    config.herdr.executable = herdr.path().join("missing-herdr.exe");
    let server = TestServer::start(&config).await;
    let (zone_path, template_id) = prepare_zone_and_template(&server, &storage).await;
    let id = accept_creation(&server, &zone_path, &template_id, "不应被提交").await;

    let failed = wait_stage(&server, &id, "failed").await;
    assert_eq!(failed["failure"]["stage"], "workspace");
    assert_eq!(failed["failure"]["code"], "herdr-unavailable");
    assert_eq!(failed["inputSubmitted"], false);
    assert!(failed.get("tabId").is_none());
    assert!(failed.get("paneId").is_none());
    assert!(failed.get("agentName").is_none());
    let message = failed["failure"]["message"]
        .as_str()
        .expect("失败必须有可读说明");
    assert!(message.contains("没有继续创建"), "{message}");

    // 外部独立记录：不可达时没有产生任何控制调用，也没有创建 Tab。
    assert!(
        herdr.calls().is_empty(),
        "不可达时不得产生外部控制调用：{:?}",
        herdr.calls()
    );
    assert!(herdr.tabs().is_empty(), "不可达时不得创建 Tab");
}

/// P37 / P42：Workspace 查询与 Tab 创建错误后不继续后续阶段，也不自动重试。
#[tokio::test(flavor = "multi_thread")]
async fn workspace_query_and_tab_failures_stop_the_creation_chain() {
    for (label, behavior, failure_stage, expected_kinds, expected_message) in [
        (
            "workspace",
            json!({ "fail_workspace_list": true }),
            "workspace",
            vec!["workspace_list".to_string()],
            "没有新建 Tab",
        ),
        (
            "tab",
            json!({ "fail_tab_create": true }),
            "tab",
            vec!["workspace_list".to_string(), "tab_create".to_string()],
            "没有继续启动 agent",
        ),
    ] {
        let herdr = HerdrFixture::new(&format!("bot creation {label} failure chain"));
        herdr.set_behavior(behavior);
        let storage = TempDir::new(&format!("bot creation {label} failure chain storage"));
        let server = TestServer::start(&config_with(&herdr, &storage)).await;
        let (zone_path, template_id) = prepare_zone_and_template(&server, &storage).await;
        let id = accept_creation(&server, &zone_path, &template_id, "不应被提交").await;

        let failed = wait_stage(&server, &id, "failed").await;
        assert_eq!(failed["failure"]["stage"], failure_stage);
        assert_eq!(failed["failure"]["code"], "herdr-command-failed");
        assert_eq!(failed["inputSubmitted"], false);
        assert!(
            failed.get("tabId").is_none(),
            "{label}：未取得 Tab 标识时不得伪造"
        );
        assert!(
            failed.get("paneId").is_none(),
            "{label}：未取得 Pane 标识时不得伪造"
        );
        let message = failed["failure"]["message"]
            .as_str()
            .expect("失败必须有可读说明");
        assert!(message.contains(expected_message), "{message}");

        // 外部独立记录：执行到失败步骤为止，之后没有启动、提交、重试或清理。
        assert_eq!(call_kinds(&herdr), expected_kinds, "{label}");
        assert!(herdr.tabs().is_empty(), "{label}：不得留下未记录的 Tab");
        assert!(calls_of_kind(&herdr, "agent_start").is_empty());
        assert!(calls_of_kind(&herdr, "agent_prompt").is_empty());
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            call_kinds(&herdr),
            expected_kinds,
            "{label}：失败后不得继续或自动重试"
        );
    }
}

/// P46：公开请求不能覆盖 Workspace；额外字段或查询参数在产生任何控制调用之前被拒绝。
#[tokio::test(flavor = "multi_thread")]
async fn a_public_request_cannot_override_the_configured_workspace() {
    let herdr = HerdrFixture::new("bot creation override");
    let storage = TempDir::new("bot creation override storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");

    let mut with_workspace = create_body(&zone_path, template_id, &preview, "普通任务");
    with_workspace["workspace"] = json!("w-other");
    let rejected = create(&server, with_workspace).await;
    assert_eq!(rejected.status, 400, "额外 Workspace 字段必须被拒绝");
    assert!(
        herdr.calls().is_empty(),
        "被拒绝的请求不得产生任何 Herdr 调用：{:?}",
        herdr.calls()
    );

    let rejected = post_json_over_tcp(
        server.addr(),
        "/api/bots?workspace=w-other",
        &create_body(&zone_path, template_id, &preview, "普通任务").to_string(),
    )
    .await;
    assert_eq!(rejected.status, 400, "查询参数必须被拒绝");
    assert!(herdr.calls().is_empty());

    // 正常请求仍只使用服务端配置的目标。
    let accepted = create(
        &server,
        create_body(&zone_path, template_id, &preview, "普通任务"),
    )
    .await;
    assert_eq!(accepted.status, 202, "正常请求应被接受：{}", accepted.body);
    let body: Value = serde_json::from_str(&accepted.body).expect("创建回执是 JSON");
    let id = body["id"].as_str().expect("创建标识");
    wait_stage(&server, id, "succeeded").await;
    let tabs = herdr.tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0]["workspace"], "w-target");
}

/// P43：创建 Bot 不自动登记 connect-bot 会话，也不修改关机监听或安排。
#[tokio::test(flavor = "multi_thread")]
async fn creating_a_bot_does_not_register_sessions_or_change_shutdown_state() {
    let herdr = HerdrFixture::new("bot creation isolation");
    let storage = TempDir::new("bot creation isolation storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let response = create(
        &server,
        create_body(&zone_path, template_id, &preview, "独立创建"),
    )
    .await;
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");
    wait_stage(&server, id, "succeeded").await;

    // 独立读取公开观察流的快照：没有新增会话，也没有监听或倒计时。
    let mut connection = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    assert_eq!(connection.status, 200);
    let frame = connection
        .next_frame_within(Duration::from_secs(5))
        .await
        .expect("观察流必须在时限内给出首帧");
    frame.named("snapshot");
    let snapshot = frame.json();
    assert_eq!(
        snapshot["sessions"],
        json!([]),
        "创建 Bot 不应产生 connect-bot 登记"
    );
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"], json!([]));
    assert_eq!(snapshot["shutdown"]["phase"], "none");
    assert!(snapshot["shutdown"].get("arrangementId").is_none());
    assert!(snapshot["shutdown"].get("deadline").is_none());
}

/// P49（名称侧）：引号、中文、空格与 Shell 特殊字符的名称经生产调用包装层原样到达外部，
/// 包装层没有额外执行 Shell 代码。正文侧的同类覆盖由 04 负责。
#[tokio::test(flavor = "multi_thread")]
async fn a_name_with_shell_special_characters_reaches_herdr_verbatim() {
    let herdr = HerdrFixture::new("bot creation shell name");
    let storage = TempDir::new("bot creation shell storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;

    let marker: PathBuf = storage.path().join("shell-marker.txt");
    let template_name = format!(
        "审查 \"quoted\" & echo injected > \"{}\" & 中文 $(echo pwned) %PATH% | ; 空格",
        marker.display()
    );
    let template = save_template(
        &server,
        json!({
            "name": template_name,
            "botName": "{zone.name} · {template.name} · {bot.id}",
            "agent": "pi",
            "model": "default",
            "thinking": "default",
            "skill": null,
            "prompt": "",
        }),
    )
    .await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    assert_eq!(preview_response.status, 200, "{}", preview_response.body);
    let preview: Value = serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");
    let bot_id = preview["botId"].as_str().expect("预览必须有 botId");
    let expected = format!("alpha · {template_name} · {bot_id}");
    assert_eq!(preview["name"], expected);

    let response = create(
        &server,
        create_body(&zone_path, template_id, &preview, "普通任务"),
    )
    .await;
    assert_eq!(response.status, 202, "{}", response.body);
    let accepted: Value = serde_json::from_str(&response.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");
    wait_stage(&server, id, "succeeded").await;

    let tab = herdr.tabs().pop().expect("受控 Herdr 必须记录新建 Tab");
    assert_eq!(
        tab["label"], expected,
        "名称必须以单个 argv 原样到达 Herdr，引号、中文、空格与 Shell 元字符都不被解释"
    );
    assert!(
        !marker.exists(),
        "包装层不得把名称交给 Shell 执行（{} 不应被创建）",
        marker.display()
    );

    let calls = herdr.calls();
    let kinds: Vec<&str> = calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt"
        ],
        "除既定控制步骤外不得有额外执行"
    );
}

/// 未登记的目录不能作为 Bot 的 cwd。
#[tokio::test(flavor = "multi_thread")]
async fn an_unregistered_zone_is_rejected_before_any_herdr_call() {
    let herdr = HerdrFixture::new("bot creation unregistered");
    let storage = TempDir::new("bot creation unregistered storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("not-registered");
    fs::create_dir_all(&zone).expect("创建目录");
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let rejected = preview(&server, &zone.display().to_string(), template_id).await;
    assert_eq!(rejected.status, 400);
    let body: Value = serde_json::from_str(&rejected.body).expect("拒绝是 JSON");
    assert_eq!(body["code"], "zone-not-registered");
    assert!(herdr.calls().is_empty());

    let missing = preview(&server, "C:\\definitely\\missing", template_id).await;
    assert_eq!(missing.status, 400);
    assert!(herdr.calls().is_empty());
}

/// P10 / P11：预览之后模板被编辑时，创建不会悄悄改成一个没预览过的名称。
#[tokio::test(flavor = "multi_thread")]
async fn a_stale_preview_is_rejected_instead_of_silently_renaming() {
    let herdr = HerdrFixture::new("bot creation stale preview");
    let storage = TempDir::new("bot creation stale preview storage");
    let server = TestServer::start(&config_with(&herdr, &storage)).await;

    let zone = storage.path().join("alpha");
    fs::create_dir_all(&zone).expect("创建 zone");
    let zone_path = register(&server, &zone).await;
    let template = save_template(&server, default_template()).await;
    let template_id = template["id"].as_str().expect("模板标识");
    let preview_response = preview(&server, &zone_path, template_id).await;
    let stale_preview: Value =
        serde_json::from_str(&preview_response.body).expect("预览响应是 JSON");

    // 另一个客户端在预览之后编辑模板；本次创建仍携带旧预览的名称。
    let edited = json!({
        "name": "发布检查",
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    });
    let response = request_json_over_tcp(
        server.addr(),
        "PUT",
        &format!("/api/bot-templates/{template_id}"),
        Some(&edited.to_string()),
    )
    .await;
    assert_eq!(response.status, 200, "编辑模板应成功：{}", response.body);

    let stale = create(
        &server,
        create_body(&zone_path, template_id, &stale_preview, "普通任务"),
    )
    .await;
    assert_eq!(stale.status, 409, "过期预览必须被拒绝：{}", stale.body);
    let body: Value = serde_json::from_str(&stale.body).expect("拒绝是 JSON");
    assert_eq!(body["code"], "name-preview-stale");
    assert!(
        herdr.calls().is_empty(),
        "过期预览被拒绝时不得产生任何 Herdr 调用"
    );

    // 重新预览后，名称来自编辑后的模板，创建正常继续。
    let refreshed_response = preview(&server, &zone_path, template_id).await;
    let refreshed: Value = serde_json::from_str(&refreshed_response.body).expect("预览响应是 JSON");
    assert!(
        refreshed["name"]
            .as_str()
            .is_some_and(|name| name.contains("alpha · 发布检查 · ")),
        "重新预览必须反映新模板：{refreshed}"
    );
    let accepted = create(
        &server,
        create_body(&zone_path, template_id, &refreshed, "普通任务"),
    )
    .await;
    assert_eq!(accepted.status, 202, "{}", accepted.body);
    let accepted: Value = serde_json::from_str(&accepted.body).expect("创建回执是 JSON");
    let id = accepted["id"].as_str().expect("创建标识");
    wait_stage(&server, id, "succeeded").await;
    assert_eq!(herdr.tabs()[0]["label"], refreshed["name"]);
}

/// P28 / P29：两项固定 Skill 都生成真正的 `/skill:` 调用；名称后用普通空格接完整正文，
/// 固定 prompt 与本次 input 以空行分段。外部记录证明每次创建只提交一次，没有先单独开启
/// Skill 轮次，prompt 中的 slash 文本也不会被再次解析成额外初始化步骤。
#[tokio::test(flavor = "multi_thread")]
async fn both_skills_submit_one_native_skill_invocation_with_the_full_body() {
    let inputs = input_fixture("bot initial skill invocation").await;

    for (skill, prefix, extension_prefix, expected) in [
        ("teach", "/skill:teach ", "/teach ", PREWRITTEN_TEACH_BODY),
        (
            "code-review",
            "/skill:code-review ",
            "/code-review ",
            PREWRITTEN_CODE_REVIEW_BODY,
        ),
    ] {
        let finished = create_with(
            &inputs,
            input_template(Some(skill), PREWRITTEN_PROMPT),
            PREWRITTEN_INPUT,
            "succeeded",
        )
        .await;
        assert_eq!(
            finished["inputSubmitted"], true,
            "{skill} 提交确认后应成功：{finished}"
        );

        let prompts = calls_of_kind(&inputs.herdr, "agent_prompt");
        let prompt = prompts.last().expect("受控 Herdr 必须收到初始指示");
        assert_eq!(
            prompt["text"], expected,
            "{skill} 的完整正文必须与预写字面样例逐字一致"
        );

        // 正文只作为 agent prompt 的单个文本参数，不携带 system-role 消息或其他包装。
        let argv = prompt["argv"].as_array().expect("调用记录必须包含 argv");
        assert_eq!(argv.len(), 4, "不得把正文拆成多条参数或附加角色字段");
        assert_eq!(argv[0], "agent");
        assert_eq!(argv[1], "prompt");
        assert_eq!(argv[2], prompt["target"]);
        assert_eq!(argv[3], expected);
        assert!(
            prompt.get("role").is_none(),
            "初始指示不是 system-role 消息：{prompt}"
        );

        let text = prompt["text"].as_str().expect("正文是字符串");
        assert!(
            text.starts_with(prefix),
            "{skill} 必须以独立字面的原生 Skill 前缀开头：{text}"
        );
        assert!(
            !text.starts_with(extension_prefix),
            "{skill} 不得调用同名扩展命令或提示词模板：{text}"
        );
    }

    // 两次创建各自只有一次查询、建 Tab、启动与一次提交；没有额外的 Skill 轮次。
    let calls = inputs.herdr.calls();
    let kinds: Vec<&str> = calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt",
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt",
        ],
        "每次创建只提交一次，不先单独发起 Skill 轮次"
    );
}

/// P22 / P30：无 Skill 时正文只由固定 prompt 与本次 input 组成，不补 Skill 前缀；
/// 空 prompt、空 input 与两者都多行的样例分别按预写字面值保真。
#[tokio::test(flavor = "multi_thread")]
async fn without_a_skill_the_body_keeps_prompt_and_input_without_a_prefix() {
    let inputs = input_fixture("bot initial plain body").await;

    for (prompt, input, expected) in [
        (PREWRITTEN_PROMPT, "", PREWRITTEN_PROMPT),
        ("", PREWRITTEN_INPUT, PREWRITTEN_INPUT),
        (PREWRITTEN_PROMPT, PREWRITTEN_INPUT, PREWRITTEN_PLAIN_BODY),
    ] {
        let finished = create_with(&inputs, input_template(None, prompt), input, "succeeded").await;
        assert_eq!(
            finished["inputSubmitted"], true,
            "正文提交确认后应成功：{finished}"
        );

        let prompt_call = calls_of_kind(&inputs.herdr, "agent_prompt")
            .pop()
            .expect("受控 Herdr 必须收到初始指示");
        let text = prompt_call["text"].as_str().expect("正文是字符串");
        assert_eq!(
            text, expected,
            "无 Skill 时正文必须与预写字面样例一致（prompt={prompt:?}, input={input:?}）"
        );
        assert!(
            !text.starts_with("/skill:"),
            "没有 Skill 时不得补加调用前缀：{text}"
        );
    }
}

/// P31：Skill-only 仍从公开入口进入既定提交，正文是单独的 Skill 调用；不补造 prompt 或 input。
#[tokio::test(flavor = "multi_thread")]
async fn a_skill_only_template_submits_the_bare_skill_call() {
    let inputs = input_fixture("bot initial skill only").await;
    let finished = create_with(&inputs, input_template(Some("teach"), ""), "", "succeeded").await;
    assert_eq!(finished["inputSubmitted"], true);

    let prompts = calls_of_kind(&inputs.herdr, "agent_prompt");
    assert_eq!(prompts.len(), 1, "Skill-only 也只提交一次");
    assert_eq!(prompts[0]["text"], "/skill:teach");
}

/// P22 / P29：有 Skill 时，空的 prompt 或 input 只是缺少该分段；命令与剩余正文仍按约定合成。
#[tokio::test(flavor = "multi_thread")]
async fn a_skill_with_an_empty_section_composes_the_remaining_body() {
    let inputs = input_fixture("bot initial skill partial body").await;

    for (skill, prompt, input, expected) in [
        (
            "teach",
            PREWRITTEN_PROMPT,
            "",
            "/skill:teach 固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面",
        ),
        (
            "code-review",
            "",
            PREWRITTEN_INPUT,
            "/skill:code-review 本次任务第一行\n第二行带引号 \"quoted\" 与 & 符号",
        ),
    ] {
        let finished = create_with(
            &inputs,
            input_template(Some(skill), prompt),
            input,
            "succeeded",
        )
        .await;
        assert_eq!(
            finished["inputSubmitted"], true,
            "{skill} 提交确认后应成功：{finished}"
        );

        let prompt_call = calls_of_kind(&inputs.herdr, "agent_prompt")
            .pop()
            .expect("受控 Herdr 必须收到初始指示");
        assert_eq!(
            prompt_call["text"], expected,
            "{skill} 缺少一个分段时仍必须与预写字面样例一致"
        );
    }
}

/// P31：Skill、prompt、input 全空仍被公开入口接受并实际提交；受控 Herdr 像真实 Herdr 一样
/// 拒绝空文本，创建按普通失败流程保留现场。不能前置组合校验、补造内容、跳过提交或假报成功。
#[tokio::test(flavor = "multi_thread")]
async fn an_all_empty_template_attempts_submission_and_reports_the_herdr_rejection() {
    let inputs = input_fixture("bot initial all empty").await;
    let finished = create_with(&inputs, input_template(None, ""), "", "failed").await;

    assert_eq!(finished["failure"]["stage"], "submit");
    assert_eq!(finished["failure"]["code"], "herdr-command-failed");
    assert_eq!(finished["inputSubmitted"], false);
    assert!(
        finished["tabId"].as_str().is_some(),
        "已创建的 Tab 标识必须保留：{finished}"
    );
    assert!(
        finished["paneId"].as_str().is_some(),
        "已创建的 Pane 标识必须保留：{finished}"
    );
    let message = finished["failure"]["message"]
        .as_str()
        .expect("失败说明是字符串");
    assert!(
        message.contains("agent prompt must not be empty"),
        "必须呈现 Herdr 的拒绝：{message}"
    );
    assert!(
        message.contains("初始指示的提交被明确拒绝")
            && message.contains("Tab、Pane 与 agent 已保留"),
        "必须区分现场已存在而提交失败：{message}"
    );

    let prompts = calls_of_kind(&inputs.herdr, "agent_prompt");
    assert_eq!(prompts.len(), 1, "全空也必须实际提交一次，不能跳过或重发");
    assert_eq!(prompts[0]["text"], "", "不得补造内容");
    assert_eq!(
        prompts[0]["argv"]
            .as_array()
            .expect("调用记录包含 argv")
            .len(),
        4,
        "空正文仍作为单个文本参数提交"
    );

    let calls = inputs.herdr.calls();
    let kinds: Vec<&str> = calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt"
        ],
        "被拒绝后不得自动重发或继续其他步骤"
    );
}

/// P49（正文侧）：引号、换行与 Shell 特殊字符经生产调用包装层原样到达外部。
/// 包装层逐参数启动进程、不经过 Shell，因此正文不会被解释成命令，也不会产生额外执行。
#[tokio::test(flavor = "multi_thread")]
async fn a_body_with_shell_special_characters_reaches_herdr_verbatim() {
    let inputs = input_fixture("bot initial shell body").await;
    // 无空格路径，保证这里是一条真正的 Shell 重定向；如果包装层交给 Shell，它会被创建。
    let marker =
        std::env::temp_dir().join(format!("herdr-bot-body-shell-{}.txt", std::process::id()));
    let _ = fs::remove_file(&marker);
    let prompt = format!(
        "固定 \"quoted\" %PATH% & echo injected > {} & $(echo pwned) `echo backtick` | ; ^\n第二行",
        marker.display()
    );
    let input = "本次 'single' ! %CD% > < & 结束\n最后一行";
    let expected = format!("{prompt}\n\n{input}");

    let finished = create_with(&inputs, input_template(None, &prompt), input, "succeeded").await;
    assert_eq!(finished["inputSubmitted"], true);

    let prompts = calls_of_kind(&inputs.herdr, "agent_prompt");
    assert_eq!(prompts.len(), 1);
    let recorded = prompts[0]["text"].as_str().expect("正文是字符串");
    assert_eq!(
        recorded, expected,
        "正文必须以单个参数原样到达 Herdr，引号、换行与 Shell 元字符都不被解释"
    );
    let argv = prompts[0]["argv"].as_array().expect("调用记录包含 argv");
    assert_eq!(argv.len(), 4, "不得把正文拆成多条参数");
    assert_eq!(argv[3], expected);
    let executed = marker.exists();
    let _ = fs::remove_file(&marker);
    assert!(
        !executed,
        "包装层不得把正文交给 Shell 执行（{} 不应被创建）",
        marker.display()
    );

    let calls = inputs.herdr.calls();
    let kinds: Vec<&str> = calls
        .iter()
        .filter_map(|call| call["kind"].as_str())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "workspace_list",
            "tab_create",
            "agent_start",
            "agent_prompt"
        ],
        "除既定控制步骤外不得有额外执行"
    );
}
