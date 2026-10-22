//! Approved S2 seam: delivered router over HTTP; expected records are independent literals.
mod common;
use std::{path::Path, sync::Arc};
use common::{TestServer, TempDir, get_over_tcp, post_json_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};
use axum::{body::{Body, to_bytes}, extract::connect_info::MockConnectInfo, http::Request};
use tower::ServiceExt;
struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("history never powers off") }
}

// P03: an accepted register must be independently readable as history. A plausible
// violation is updating only the transient registration; GET distinguishes that.
#[tokio::test]
async fn accepted_register_is_readable_as_history_without_optional_metadata() {
    let root = TempDir::new("history-register");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let accepted = post_json_over_tcp(server.addr(), "/api/sessions/events", r#"{"sessionId":"native-full-id-001","kind":"register","data":{"cwd":"C:/known/project"}}"#).await;
    assert_eq!(accepted.status, 200);
    assert_eq!(serde_json::from_str::<Value>(&accepted.body).unwrap()["outcome"], "registered");
    let response = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(response.status, 200, "accepted registration must have a public history: {}", response.body);
    let body: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(body["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(body["sessions"][0]["sessionId"], "native-full-id-001");
    assert_eq!(body["sessions"][0]["cwd"], "C:/known/project");
    assert_eq!(body["sessions"][0]["sessionName"], Value::Null);
    assert_eq!(body["sessions"][0]["agentType"], Value::Null);
    assert!(body["sessions"][0]["firstRecordedAt"].as_u64().unwrap() > 0);
    assert_eq!(body["sessions"][0].as_object().unwrap().len(), 5);
}

// P08 and basic P04: literal full IDs deliberately share prefix/name/cwd. A
// replacement-only update loses known fields; prefix/cwd keys collapse two rows.
#[tokio::test]
async fn full_identity_and_valid_metadata_survive_partial_and_empty_updates() {
    let root = TempDir::new("history-merge");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    for (id, data) in [
        ("native-shared-prefix-001", json!({"cwd":"C:/initial", "sessionName":"Same name", "agentType":"omp", "sessionFile":"C:/native/original.jsonl"})),
        ("native-shared-prefix-002", json!({"cwd":"C:/initial", "sessionName":"Same name"})),
        ("native-shared-prefix-001", json!({"cwd":"C:/updated", "sessionName":"Updated name", "sessionFile":"C:/native/new.jsonl"})),
        ("native-shared-prefix-001", json!({"cwd":" ", "sessionName":"  ", "agentType":"", "sessionFile":"relative.jsonl"})),
        ("native-shared-prefix-002", json!({"cwd":"", "agentType":"pi"})),
    ] {
        let reply = post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({"sessionId":id,"kind":"register","data":data}).to_string()).await;
        assert_eq!(reply.status, 200, "{}", reply.body);
    }
    let reply = get_over_tcp(server.addr(), "/api/sessions/history").await;
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    let records = body["sessions"].as_array().unwrap();
    assert_eq!(records.len(), 2);
    let one = records.iter().find(|r| r["sessionId"] == "native-shared-prefix-001").unwrap();
    assert_eq!(one["sessionName"], "Updated name");
    assert_eq!(one["cwd"], "C:/updated");
    assert_eq!(one["agentType"], "omp");
    let two = records.iter().find(|r| r["sessionId"] == "native-shared-prefix-002").unwrap();
    assert_eq!(two["sessionName"], "Same name");
    assert_eq!(two["cwd"], "C:/initial");
    assert_eq!(two["agentType"], "pi");
}

// P03/P27: protocol and source refusal must leave the exact history set unchanged.
// Only the socket address is substituted; request parsing and source policy stay real.
#[tokio::test]
async fn rejected_sources_missing_required_fields_and_non_register_events_never_collect() {
    let root = TempDir::new("history-boundary");
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let app = herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff));
    let server = TestServer::start_app(app.clone()).await;
    for (event, status) in [
        (json!({"sessionId":"missing-cwd", "kind":"register", "data":{}}), 400),
        (json!({"sessionId":"unknown-heartbeat", "kind":"heartbeat"}), 404),
        (json!({"sessionId":"unknown-state", "kind":"state", "data":{"value":"working"}}), 404),
        (json!({"sessionId":"unknown-unregister", "kind":"unregister"}), 404),
    ] {
        assert_eq!(post_json_over_tcp(server.addr(), "/api/sessions/events", &event.to_string()).await.status, status);
    }
    let lan = app.layer(MockConnectInfo("192.0.2.123:45678".parse::<std::net::SocketAddr>().unwrap()));
    let rejected = lan.clone().oneshot(Request::builder().method("POST").uri("/api/sessions/events").header("content-type", "application/json").body(Body::from(r#"{"sessionId":"rejected-lan","kind":"register","data":{"cwd":"C:/fake"}}"#)).unwrap()).await.unwrap();
    assert_eq!(rejected.status(), 403);
    let rejected: Value = serde_json::from_slice(&to_bytes(rejected.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(rejected["code"], "forbidden");
    let before = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(serde_json::from_str::<Value>(&before.body).unwrap(), json!({"sessions":[]}));
    let accepted = post_json_over_tcp(server.addr(), "/api/sessions/events", r#"{"sessionId":"accepted-local","kind":"register","data":{"cwd":"C:/known"}}"#).await;
    assert_eq!(accepted.status, 200);
    let read = lan.oneshot(Request::builder().uri("/api/sessions/history").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(read.status(), 200);
    let read: Value = serde_json::from_slice(&to_bytes(read.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(read["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(read["sessions"][0]["sessionId"], "accepted-local");
}

// P25/P26: native files on disk are not an import trigger. Accepted explicit
// registration collects the old ID; restart preserves history, never transient intent.
#[tokio::test]
async fn explicit_register_survives_restart_without_restoring_current_or_shutdown_intent() {
    use common::sse::SseConnection;
    let root = TempDir::new("history-restart");
    let native = root.path().join("old-native.jsonl");
    let sidecar = root.path().join("old-native.notes");
    std::fs::write(&native, b"old native body must remain exact\n").unwrap();
    std::fs::write(&sidecar, b"old notes").unwrap();
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let empty = get_over_tcp(server.addr(), "/api/sessions/history").await;
    assert_eq!(serde_json::from_str::<Value>(&empty.body).unwrap(), json!({"sessions":[]}));
    for event in [
        json!({"sessionId":"old-native-exact-id", "kind":"register", "data":{"cwd":"C:/old", "sessionName":"Original", "sessionFile":native}}),
        json!({"sessionId":"old-native-exact-id", "kind":"state", "data":{"value":"working"}}),
    ] { assert_eq!(post_json_over_tcp(server.addr(), "/api/sessions/events", &event.to_string()).await.status, 200); }
    let monitor = post_json_over_tcp(server.addr(), "/api/shutdown/monitor", r#"{"sessionId":"old-native-exact-id","selected":true}"#).await;
    assert_eq!(monitor.status, 200, "{}", monitor.body);
    let mut before = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let snapshot: Value = before.next_event("snapshot").await.json();
    assert_eq!(snapshot["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"], json!(["old-native-exact-id"]));
    assert_eq!(post_json_over_tcp(server.addr(), "/api/sessions/events", r#"{"sessionId":"old-native-exact-id","kind":"register","data":{"cwd":"C:/updated","sessionName":"Updated"}}"#).await.status, 200);
    let mut updated = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    let snapshot: Value = updated.next_event("snapshot").await.json();
    assert_eq!(snapshot["sessions"][0]["state"], "working");
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"], json!(["old-native-exact-id"]));
    let expected: Value = serde_json::from_str(&get_over_tcp(server.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(expected["sessions"][0]["sessionName"], "Updated");
    assert_eq!(common::post_over_tcp(server.addr(), "/api/shutdown/start").await.status, 200);
    drop(before); drop(updated); drop(server);
    let restarted = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let actual: Value = serde_json::from_str(&get_over_tcp(restarted.addr(), "/api/sessions/history").await.body).unwrap();
    assert_eq!(actual, expected);
    let mut feed = SseConnection::open(restarted.addr(), "/api/sessions/events", &[]).await;
    let snapshot: Value = feed.next_event("snapshot").await.json();
    assert_eq!(snapshot["sessions"], json!([]));
    assert_eq!(snapshot["shutdown"]["monitoredSessionIds"], json!([]));
    assert_eq!(snapshot["shutdown"]["phase"], "none");
    assert_eq!(std::fs::read(&native).unwrap(), b"old native body must remain exact\n");
    assert_eq!(std::fs::read(&sidecar).unwrap(), b"old notes");
}
