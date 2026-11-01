//! Approved S1/S3: public HTTP, owned persistent records and external host calls.
use super::*;

async fn action(s: &Scenario, name: &str, body: Value, status: u16) -> Value {
    let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{name}"), &body.to_string()).await;
    assert_eq!(response.status, status, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

async fn report(s: &Scenario, identity: &str, ready: bool) -> Value {
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":identity,"body":"Completion declaration; no file output required","canComplete":ready}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str::<Value>(&response.body).unwrap()["report"].clone()
}

// P14/P15/P17/P20 source: ticket requires the latest current-execution strict
// true, never an absent/older true declaration. Plausible violation: complete
// ignores reports or searches backwards for any true. Keep HTTP, report save,
// operation coordination and notifications real; compare bytes and host calls.
#[tokio::test]
async fn latest_current_report_alone_allows_first_completion_without_file_outputs() {
    let s = Scenario::new().await;
    let started = s.start("gate-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"gate-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("gate-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let body = json!({"sessionId":"gate-manager","nodeId":"A","reason":"Manager accepts handoff"});
    for ready in [None, Some(false), Some(true), Some(false)] {
        let latest = match ready { Some(value) => Some(report(&s, "gate-A", value).await), None => None };
        let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
        let calls = s.herdr.calls();
        // The intermediate true is checked as a report-only fact; completing
        // now would prevent exercising the subsequent true -> false boundary.
        if ready == Some(true) {
            let value = action(&s, "status", json!({"sessionId":"gate-manager"}), 200).await;
            assert_eq!(value["snapshot"]["nodes"]["A"]["status"], "running");
            assert_eq!(value["snapshot"]["nodes"]["C"]["status"], "pending");
        } else {
            let denied = action(&s, "complete", body.clone(), 409).await;
            assert_eq!(denied["code"], "report-not-completable");
            assert_eq!(denied["changed"], false);
            let message = denied["message"].as_str().unwrap();
            if let Some(report) = latest {
                assert!(message.contains(report["id"].as_str().unwrap()), "{denied}");
                assert!(message.contains("false"), "{denied}");
            } else {
                assert!(message.contains("没有"), "{denied}");
                assert_eq!(denied["reports"], json!([]));
            }
        }
        assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
        assert_eq!(s.herdr.calls(), calls);
    }
    let checked = action(&s, "collect", json!({"sessionId":"gate-manager"}), 200).await;
    assert_eq!(checked["reason"], "notification");
    let waiting = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/collect", r#"{"sessionId":"gate-manager"}"#);
    tokio::pin!(waiting);
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut waiting).await.is_err());
    action(&s, "complete", body.clone(), 409).await;
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut waiting).await.is_err(), "rejection must not create a notification");
    let latest = report(&s, "gate-A", true).await;
    let notified = tokio::time::timeout(Duration::from_secs(5), waiting).await.unwrap();
    let notified: Value = serde_json::from_str(&notified.body).unwrap();
    assert_eq!(notified["pendingReports"]["A"], latest["id"]);
    assert_eq!(notified["snapshot"]["nodes"]["A"]["status"], "running");
    let calls = s.herdr.calls();
    let completed = action(&s, "complete", body, 200).await;
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["ready"], json!(["B"]));
    assert_eq!(s.herdr.calls(), calls, "report/complete must not launch downstream");
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), completed["snapshot"]);
    assert_eq!(common::read_json_lines(&root.join("messages.jsonl")).last(), Some(&latest));
    assert_eq!(common::read_json_lines(&root.join("history.jsonl")).last().unwrap()["action"], "complete");
    println!("P14/P15/P17/P20 HTTP smoke run={} latest={} completed={completed}", started["id"], latest["id"]);
}

// P14/P15 source: historical absence is not true, and old executions never
// authorize current work. A plausible violation selects any node report or
// defaults a missing field true. Construct the historical wire record only;
// all subsequent retry, reporting and completion still use public HTTP.
#[tokio::test]
async fn historical_missing_declaration_and_old_execution_true_never_authorize_current_work() {
    let s = Scenario::new().await;
    let started = s.start("history-gate-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-gate-old"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("history-gate-manager", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let mut historical = report(&s, "history-gate-old", true).await;
    historical.as_object_mut().unwrap().remove("canComplete");
    fs::write(root.join("messages.jsonl"), format!("{historical}\n")).unwrap();
    let body = json!({"sessionId":"history-gate-manager","nodeId":"A","reason":"Accept applicable declaration only"});
    let denied = action(&s, "complete", body.clone(), 409).await;
    assert!(denied["message"].as_str().unwrap().contains("缺失"));
    assert!(denied["message"].as_str().unwrap().contains(historical["id"].as_str().unwrap()));
    assert!(denied["reports"][0].get("canComplete").is_none());
    report(&s, "history-gate-old", true).await;
    action(&s, "fail", json!({"sessionId":"history-gate-manager","nodeId":"A","reason":"Replace old execution"}), 200).await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-gate-new"},"native_cwd":s.data.path().join("node-zone")}));
    let retry = action(&s, "retry", json!({"sessionId":"history-gate-manager","nodeId":"A","confirmed":true,"reason":"Independent replacement authorized"}), 200).await;
    let denied = action(&s, "complete", body.clone(), 409).await;
    assert!(denied["message"].as_str().unwrap().contains("没有"));
    let current = report(&s, "history-gate-new", false).await;
    report(&s, "history-gate-old", true).await;
    let before = ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let calls = s.herdr.calls();
    let denied = action(&s, "complete", body.clone(), 409).await;
    assert_eq!(denied["snapshot"], retry["snapshot"]);
    assert!(denied["message"].as_str().unwrap().contains(current["id"].as_str().unwrap()));
    assert_eq!(["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert_eq!(s.herdr.calls(), calls);
    report(&s, "history-gate-new", true).await;
    assert_eq!(action(&s, "complete", body, 200).await["changed"], true);
}

struct Release(std::path::PathBuf);
impl Drop for Release {
    fn drop(&mut self) { let _ = fs::write(&self.0, "release"); }
}

// P16/P18: observe real commits while external delivery/close remains gated.
// A pre-lock or stale report decision could accept the saved false; a gate
// applied to duplicate completion could reject the later false or close twice.
// Neither order is inferred from task scheduling, timestamp equality or sleep.
#[tokio::test]
async fn committed_report_and_completion_order_survives_overlapping_external_effects() {
    let s = Scenario::new().await;
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["A"]["auto_close"] = json!(true);
    fs::write(&map_path, map.to_string()).unwrap();
    let started = s.start("ordered-manager").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"ordered-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("ordered-manager", &["A"]).await;
    report(&s, "ordered-A", true).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let delivery_release = s.data.path().join("ordered-delivery-release");
    let _release_delivery = Release(delivery_release.clone());
    s.herdr.set_behavior(json!({"prompt_gate":{"wait_for_file":delivery_release,"timeout_ms":30000}}));
    let pending_report = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/node/report", r#"{"sessionId":"ordered-A","body":"New blocker","canComplete":false}"#).await;
    assert!(common::wait_until(Duration::from_secs(10), || common::read_json_lines(&root.join("messages.jsonl")).len() == 2).await);
    let saved_false = common::read_json_lines(&root.join("messages.jsonl"))[1].clone();
    let before = ["run.json", "history.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let complete_body = json!({"sessionId":"ordered-manager","nodeId":"A","reason":"Accept once"});
    let denied = action(&s, "complete", complete_body.clone(), 409).await;
    assert!(denied["message"].as_str().unwrap().contains(saved_false["id"].as_str().unwrap()));
    assert_eq!(["run.json", "history.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), before);
    assert!(!delivery_release.exists());
    assert!(!s.herdr.calls().iter().any(|call| call["kind"] == "tab_close"));
    fs::write(&delivery_release, "release").unwrap();
    assert_eq!(pending_report.response().await.status, 200);
    s.herdr.set_behavior(json!({}));
    report(&s, "ordered-A", true).await;
    let close_release = s.data.path().join("ordered-close-release");
    let _release_close = Release(close_release.clone());
    s.herdr.set_behavior(json!({"tab_close_gate":{"wait_for_file":close_release,"timeout_ms":30000}}));
    let pending_complete = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/complete", &complete_body.to_string()).await;
    assert!(common::wait_until(Duration::from_secs(10), || s.herdr.calls().iter().any(|call| call["kind"] == "tab_close")
        && common::read_json_lines(&root.join("history.jsonl")).last().is_some_and(|event| event["action"] == "complete")).await);
    let committed = ["run.json", "history.jsonl"].map(|name| fs::read(root.join(name)).unwrap());
    let committed_run: Value = serde_json::from_slice(&committed[0]).unwrap();
    assert_eq!(committed_run["nodes"]["A"]["status"], "completed");
    let later = report(&s, "ordered-A", false).await;
    let completed = pending_complete.response().await;
    assert_eq!(completed.status, 200, "{}", completed.body);
    let repeated = action(&s, "complete", json!({"sessionId":"ordered-manager","nodeId":"A","reason":"Do not overwrite first fact"}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], committed_run);
    assert_eq!(repeated["reports"].as_array().unwrap().last(), Some(&later));
    assert_eq!(["run.json", "history.jsonl"].map(|name| fs::read(root.join(name)).unwrap()), committed);
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "tab_close").count(), 1);
    assert!(!close_release.exists());
    assert!(common::read_json_lines(&s.herdr.path().join("closed.jsonl")).is_empty());
    fs::write(&close_release, "release").unwrap();
    assert!(common::wait_until(Duration::from_secs(10), || common::read_json_lines(&s.herdr.path().join("closed.jsonl")).len() == 1).await);
    println!("P16/P18 commit order: false={} -> reject; completed={} -> false={} -> unchanged; history={:?}", saved_false["id"], committed_run["nodes"]["A"]["completion"]["id"], later["id"], common::read_json_lines(&root.join("history.jsonl")));
}
