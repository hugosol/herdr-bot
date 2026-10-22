//! 浏览器侧模板管理与创建意图和会话事件写入的来源边界（P45）。
//!
//! Bot 模板管理与“从 zone 创建 Bot”不是 connect-bot 会话登记：它们不读取 TCP 来源，局域网设备
//! 可直接使用，本版也没有账号、token 或权限系统。会话事件写入仍只接受本机来源（ADR 0004）。
//!
//! 本文件在同一个公开 HTTP 路由上，用同一个受控来源分别观察两种能力：来源是 RFC 5737 的文档
//! 地址（本机不可能拥有），代表局域网设备。模板新增/读取、名称预览与创建请求被接受，而同一来源
//! 的会话事件写入仍得到 `403 forbidden`。受控来源经 axum 的 `MockConnectInfo` 注入，与
//! `session_write_boundary.rs` 使用同一条接缝；创建目标仍走受控 Herdr 夹具，夹具记录不冒充真实
//! Herdr / agent 验收。

mod common;

use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use herdr_bot::PowerOff;
use herdr_bot::sessions::SessionsState;
use serde_json::{Value, json};
use tower::ServiceExt;

/// 代表局域网设备的来源地址：RFC 5737 的 TEST-NET-3 文档地址，本机不可能拥有它。
const LAN_SOURCE: &str = "203.0.113.7:40000";

/// 来源边界用例绝不接触关机路径；这个动作只会在路由接错时立刻把用例打失败。
struct NeverPowerOff;

impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), herdr_bot::PowerOffFailure> {
        panic!("来源边界用例不应触发关机");
    }
}

/// 把应用装上受控的 TCP 对端地址：这批请求不来自真 socket，来源就是这里指定的地址。
fn from_source(app: &Router, source: &str) -> Router {
    app.clone().layer(MockConnectInfo(
        source.parse::<SocketAddr>().expect("受控来源地址"),
    ))
}

/// 不经真 socket 提交一次公开 HTTP 请求，返回状态码与响应体。
async fn send(app: Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, String) {
    let request = match body {
        Some(payload) => Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string())),
        None => Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty()),
    }
    .expect("构造请求");
    let response = app.oneshot(request).await.expect("请求应用");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("读取响应体");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn body_of(status: StatusCode, body: &str) -> Value {
    serde_json::from_str(body)
        .unwrap_or_else(|error| panic!("响应体必须是 JSON（HTTP {status}）：{error}：{body}"))
}

#[tokio::test]
async fn lan_source_manages_templates_and_creates_bots_but_cannot_write_session_events() {
    let herdr = common::HerdrFixture::new("bot source boundary");
    let storage = common::TempDir::new("bot source boundary storage");
    let zone_root = common::TempDir::new("bot source boundary zone");
    let workspace = zone_root.path().join("alpha");
    fs::create_dir_all(&workspace).expect("创建 zone 目录");

    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = storage.path().join("zones.json");
    config.bot_templates.storage_file = storage.path().join("bot-templates.json");
    config.herdr = herdr_bot::HerdrConfig {
        workspace_name: "bot-playground".to_string(),
        executable: herdr.executable().to_path_buf(),
    };

    let sessions = Arc::new(SessionsState::new());
    let app = herdr_bot::app_with_sessions(&config, Arc::new(NeverPowerOff), sessions);
    let lan = from_source(&app, LAN_SOURCE);

    // 模板读取与新增对局域网来源开放：这是与 connect-bot 会话写入不同的产品能力。
    let (status, body) = send(lan.clone(), "GET", "/api/bot-templates", None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "局域网来源应能读取模板与固定选项：{status} {body}"
    );

    let zone_body = json!({ "path": workspace }).to_string();
    let (status, body) = send(lan.clone(), "POST", "/api/zones", Some(&zone_body)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "局域网来源应能登记服务器目录：{status} {body}"
    );
    let registered = body_of(status, &body);
    let zone = registered["zones"]
        .as_array()
        .and_then(|zones| zones.first())
        .and_then(|zone| zone["path"].as_str())
        .expect("登记响应必须包含刚登记的目录")
        .to_string();

    let template = json!({
        "name": "审查",
        "botName": "{zone.name} · {template.name} · {bot.id}",
        "agent": "omp",
        "model": "default",
        "thinking": "default",
        "skill": null,
        "prompt": "",
    })
    .to_string();
    let (status, body) = send(lan.clone(), "POST", "/api/bot-templates", Some(&template)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "局域网来源应能保存模板：{status} {body}"
    );
    let saved = body_of(status, &body);
    let template_id = saved["templates"]
        .as_array()
        .and_then(|templates| templates.first())
        .and_then(|template| template["id"].as_str())
        .expect("保存响应应包含模板标识")
        .to_string();

    let (status, body) = send(
        lan.clone(),
        "POST",
        "/api/bots/preview",
        Some(&json!({ "zone": zone, "templateId": template_id }).to_string()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "局域网来源应能预览 Bot 名称：{status} {body}"
    );
    let preview = body_of(status, &body);
    let bot_id = preview["botId"].as_str().expect("预览应给出 Bot 标识");
    let expected_name = preview["name"].as_str().expect("预览应给出名称");

    let create = json!({
        "zone": zone,
        "templateId": template_id,
        "botId": bot_id,
        "expectedName": expected_name,
        "input": "局域网来源创建",
    })
    .to_string();
    let (status, body) = send(lan.clone(), "POST", "/api/bots", Some(&create)).await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "创建意图应独立于会话写入边界被接受：{status} {body}"
    );
    let accepted = body_of(status, &body);
    let creation_id = accepted["id"]
        .as_str()
        .expect("接受响应应包含创建标识")
        .to_string();

    // 同一来源继续读取这次创建，直到后端确认成功；证明被接受的创建真的执行，而不只是返回 202。
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut stage = String::new();
    while tokio::time::Instant::now() < deadline {
        let path = format!("/api/bots/{creation_id}");
        let (status, body) = send(lan.clone(), "GET", &path, None).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "局域网来源应能读取自己创建的进展：{status} {body}"
        );
        stage = body_of(status, &body)["stage"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if stage == "succeeded" || stage == "failed" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        stage, "succeeded",
        "局域网来源接受的创建应完成，而不是停在不可观察的阶段"
    );

    // 同一个局域网来源写会话事件仍然被拒绝：没有为手机创建放宽既有边界。
    let event = json!({
        "sessionId": "lan-source-1",
        "kind": "register",
        "data": { "cwd": "C:\\work" },
    })
    .to_string();
    let (status, body) = send(lan, "POST", "/api/sessions/events", Some(&event)).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "会话事件写入只接受本机来源：{status} {body}"
    );
    assert_eq!(body_of(status, &body)["code"], "forbidden");
}
