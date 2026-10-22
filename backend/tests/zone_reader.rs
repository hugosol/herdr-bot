//! HTML 阅读模式使用的 zone 原始资源路由。
//!
//! HTML 本身与相对资源都从一个按 zone 根约束的路径读取；期望值来自独立创建的真实文件，
//! 边界反例通过公开 HTTP 请求实施。

mod common;

use std::fs;
use std::path::Path;

use common::{HttpResponse, TempDir, TestServer, get_over_tcp, post_json_over_tcp};
use serde_json::{Value, json};

fn config_with_registry(registry: &Path) -> herdr_bot::Config {
    let fixtures = common::repo_fixtures_dir();
    let script = fixtures.join("operations").join("success.bat");
    let operations = fixtures.join("operations");
    let mut config = common::config(&script, &operations);
    config.zones.registry_file = registry.to_path_buf();
    config
}

fn hex(text: &str) -> String {
    text.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn path_segments(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            let mut encoded = String::new();
            for byte in segment.as_bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        encoded.push(*byte as char)
                    }
                    _ => encoded.push_str(&format!("%{byte:02X}")),
                }
            }
            encoded
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn reader_url(zone: &Path, path: &str) -> String {
    format!(
        "/api/zones/reader/{}/{}",
        hex(&zone.to_string_lossy()),
        path_segments(path)
    )
}

fn header<'a>(response: &'a HttpResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
}

fn assert_rejected(response: &HttpResponse, status: u16, code: &str) {
    assert_eq!(
        response.status, status,
        "拒绝状态码不符合预期：{}",
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

async fn register(server: &TestServer, zone: &Path) {
    let response = post_json_over_tcp(
        server.addr(),
        "/api/zones",
        &json!({ "path": zone }).to_string(),
    )
    .await;
    assert_eq!(response.status, 200, "登记应成功：{}", response.body);
}

#[tokio::test]
async fn serves_html_and_relative_resources_with_isolated_content_type_and_csp() {
    let dir = TempDir::new("zone reader resources");
    let zone = dir.path().join("course");
    fs::create_dir_all(zone.join("lessons")).expect("创建 lessons");
    fs::create_dir_all(zone.join("assets")).expect("创建 assets");
    let html = r#"<!doctype html><html><head><link rel="stylesheet" href="../assets/style.css"></head><body><h1>lesson</h1><img src="../assets/pixel.png"></body></html>"#;
    fs::write(zone.join("lessons/page.html"), html).expect("写入 HTML");
    fs::write(
        zone.join("assets/style.css"),
        "body { font-family: sans-serif; }",
    )
    .expect("写入 CSS");
    fs::write(zone.join("assets/pixel.png"), b"\x89PNG\r\n\x1a\n").expect("写入图片");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &zone).await;

    let response = get_over_tcp(server.addr(), &reader_url(&zone, "lessons/page.html")).await;
    assert_eq!(response.status, 200, "HTML 应能读取：{}", response.body);
    assert_eq!(response.body, html);
    assert_eq!(
        header(&response, "content-type"),
        Some("text/html; charset=utf-8")
    );
    let csp = header(&response, "content-security-policy").expect("HTML 必须带 CSP");
    assert!(csp.contains("default-src 'none'"));
    assert!(csp.contains("script-src 'none'"));
    assert!(csp.contains("frame-ancestors 'self'"));
    assert_eq!(header(&response, "x-content-type-options"), Some("nosniff"));

    let response = get_over_tcp(server.addr(), &reader_url(&zone, "assets/style.css")).await;
    assert_eq!(response.status, 200, "CSS 应能读取：{}", response.body);
    assert_eq!(
        response.body, "body { font-family: sans-serif; }",
        "CSS 响应必须是原始内容"
    );
    assert_eq!(
        header(&response, "content-type"),
        Some("text/css; charset=utf-8")
    );

    let response = get_over_tcp(server.addr(), &reader_url(&zone, "assets/pixel.png")).await;
    assert_eq!(response.status, 200, "图片应能读取：{}", response.body);
    assert_eq!(header(&response, "content-type"), Some("image/png"));
}

#[tokio::test]
async fn reader_resource_route_rejects_unregistered_zones_and_path_escapes() {
    let dir = TempDir::new("zone reader boundary");
    let zone = dir.path().join("course");
    let outside = dir.path().join("outside.txt");
    fs::create_dir_all(zone.join("lessons")).expect("创建 zone");
    fs::write(zone.join("lessons/page.html"), "<h1>inside</h1>").expect("写入 HTML");
    fs::write(&outside, "outside").expect("写入根外文件");
    let registry = dir.path().join("zones.json");
    let server = TestServer::start(&config_with_registry(&registry)).await;
    register(&server, &zone).await;

    let escape = format!(
        "/api/zones/reader/{}/%2e%2e/outside.txt",
        hex(&zone.to_string_lossy())
    );
    assert_rejected(
        &get_over_tcp(server.addr(), &escape).await,
        403,
        "path-outside-zone",
    );

    let unregistered = dir.path().join("other");
    fs::create_dir_all(&unregistered).expect("创建未登记目录");
    assert_rejected(
        &get_over_tcp(server.addr(), &reader_url(&unregistered, "page.html")).await,
        404,
        "zone-not-registered",
    );

    assert_rejected(
        &get_over_tcp(server.addr(), &format!("/api/zones/reader/zz/page.html")).await,
        400,
        "invalid-zone-token",
    );

    assert_rejected(
        &get_over_tcp(server.addr(), &reader_url(&zone, "lessons")).await,
        400,
        "not-a-file",
    );
}
