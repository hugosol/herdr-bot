//! P11/P12 at approved S2/S3: HTTP results, independent durable facts and
//! commands recorded beyond the production Herdr CLI transport.
use super::*;

// P11: the saved transition, not the in-memory candidate, grants close eligibility.
// A handle denies real Windows replacement while leaving reads operational. A
// close-before-save implementation would still issue a command and fail this test.
#[tokio::test]
async fn current_fact_save_failure_never_completes_or_closes() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    let started = s.start("commit-failed-manager").await;
    node_host(&s, "commit-failed-A");
    s.dispatch("commit-failed-manager", &["A"]).await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let path = root.join("run.json");
    let before = fs::read(&path).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    // FILE_SHARE_READ | FILE_SHARE_WRITE, deliberately not FILE_SHARE_DELETE.
    let held = fs::OpenOptions::new().read(true).share_mode(3).open(&path).unwrap();
    let failed = manager_action(&s, "complete", json!({"sessionId":"commit-failed-manager","nodeId":"A","reason":"Not durable; must not close"}), 500).await;
    assert_eq!(failed["code"], "complete-failed");
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    let read: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(read["run"]["nodes"]["A"]["status"], "running");
    assert!(read["run"]["nodes"]["A"]["completion"].is_null());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    drop(held);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(s.herdr.calls(), calls, "failed persistence must not invoke the host");
    assert!(closes(&s).is_empty());
    println!("P11 currentFactSaveFailure={failed} independentRead={read} observationWindowMs=200 calls={calls:?}");
}

// P11: only history writes fail; the durable current fact still authorizes one
// close. Waiting for history success, rolling back, or treating the history
// error as new close eligibility on a duplicate must violate these observations.
#[tokio::test]
async fn history_failure_keeps_completion_and_close_without_duplicate_eligibility() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    let started = s.start("commit-history-manager").await;
    node_host(&s, "commit-history-A");
    let dispatched = s.dispatch("commit-history-manager", &["A"]).await;
    let tab = &dispatched["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"];
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let history_path = root.join("history.jsonl");
    let before = fs::read(&history_path).unwrap();
    // Permit independent reads, but deny actual history writes and replacement.
    let held = fs::OpenOptions::new().read(true).share_mode(1).open(&history_path).unwrap();
    let completed = manager_action(&s, "complete", json!({"sessionId":"commit-history-manager","nodeId":"A","reason":"Verified result; history unavailable"}), 200).await;
    assert_eq!(completed["changed"], true);
    assert_eq!(completed["code"], "history-append-failed");
    assert_eq!(completed["stateCommitted"], true);
    assert!(completed["historyError"].as_str().unwrap().contains("history.jsonl"));
    assert_eq!(completed["message"], "当前状态已提交，但执行历史追加失败；状态未回滚。请先查询现场，不要盲目重试。");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["completion"]["reason"], "Verified result; history unavailable");
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["run"], completed["snapshot"]);
    let saved = fs::read(root.join("run.json")).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&saved).unwrap(), completed["snapshot"]);
    assert_eq!(fs::read(&history_path).unwrap(), before);
    wait_closed(&s, 1).await;
    assert_eq!(closes(&s).iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab", "close", tab])]);
    drop(held);
    let repeated = manager_action(&s, "complete", json!({"sessionId":"commit-history-manager","nodeId":"A","reason":"Duplicate must not repair history or close again"}), 200).await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], completed["snapshot"]);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), saved);
    assert_eq!(fs::read(&history_path).unwrap(), before, "no synthetic replacement completion history");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(closes(&s).len(), 1, "history failure must not grant a duplicate close");
    println!("P11 historyFailure={completed} repeated={repeated} observationWindowMs=200 closes={:?}", closes(&s));
}

struct ReleaseOnDrop(std::path::PathBuf);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) { let _ = fs::write(&self.0, "release"); }
}

// P12: a successful complete owns the first fact and one close attempt even
// while that command remains in flight. Reissuing close from Unchanged, or
// replacing the first completion ID/reason/history, violates literal counts.
#[tokio::test]
async fn sequential_duplicates_preserve_first_fact_and_single_in_flight_close() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    let started = s.start("commit-sequential-manager").await;
    node_host(&s, "commit-sequential-A");
    let dispatched = s.dispatch("commit-sequential-manager", &["A"]).await;
    let tab = &dispatched["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"];
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let release = s.data.path().join("sequential-close-release");
    let _release_on_drop = ReleaseOnDrop(release.clone());
    s.herdr.set_behavior(json!({"tab_close_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let completed = tokio::time::timeout(Duration::from_secs(10), manager_action(&s, "complete", json!({"sessionId":"commit-sequential-manager","nodeId":"A","reason":"First accepted handoff"}), 200)).await.expect("completion must not await close");
    assert_eq!(completed["changed"], true);
    assert!(common::wait_until(Duration::from_secs(10), || closes(&s).len() == 1).await);
    let fact = completed["snapshot"]["nodes"]["A"]["completion"].clone();
    assert!(fact["id"].is_string());
    assert_eq!(fact["reason"], "First accepted handoff");
    let saved = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    for reason in ["Second request", "Third request"] {
        let repeated = tokio::time::timeout(Duration::from_secs(10), manager_action(&s, "complete", json!({"sessionId":"commit-sequential-manager","nodeId":"A","reason":reason}), 200)).await.expect("duplicate must not await the in-flight close");
        assert_eq!(repeated["changed"], false);
        assert_eq!(repeated["snapshot"]["nodes"]["A"]["completion"], fact);
    }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), saved);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let events = common::read_json_lines(&root.join("history.jsonl"));
    let completions: Vec<_> = events.iter().filter(|event| event["action"] == "complete").collect();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0]["id"], fact["id"]);
    assert_eq!(completions[0]["reason"], "First accepted handoff");
    assert!(!release.exists());
    assert!(common::read_json_lines(&s.herdr.path().join("closed.jsonl")).is_empty());
    fs::write(&release, "release").unwrap();
    wait_closed(&s, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(closes(&s).iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab", "close", tab])]);
    println!("P12 sequential firstFact={fact} history={completions:?} observationWindowMs=200 closes={:?}", closes(&s));
}

// P12: two separate TCP requests compete while a real B dispatch holds this
// Run's business boundary. Both must be outstanding before releasing B; this
// is not sequential calls disguised with join. A stale pre-lock decision or
// missing serialization would return early or commit/close more than once.
#[tokio::test]
async fn concurrent_duplicates_commit_one_fact_and_attempt_one_close() {
    let s = Scenario::new().await;
    set_auto_close(&s, "A", true).await;
    let started = s.start("commit-concurrent-manager").await;
    node_host(&s, "commit-concurrent-A");
    let dispatched = s.dispatch("commit-concurrent-manager", &["A"]).await;
    let tab = &dispatched["snapshot"]["nodes"]["A"]["execution"]["chat"]["tabId"];
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    let dispatch_release = s.data.path().join("concurrent-dispatch-release");
    let close_release = s.data.path().join("concurrent-close-release");
    let _dispatch_release_on_drop = ReleaseOnDrop(dispatch_release.clone());
    let _close_release_on_drop = ReleaseOnDrop(close_release.clone());
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"commit-concurrent-B"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":dispatch_release,"timeout_ms":30000},"tab_close_gate":{"wait_for_file":close_release,"timeout_ms":30000}}));
    let prompts = s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count();
    let addr = s.server.addr();
    let dispatch = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/dispatch", &json!({"sessionId":"commit-concurrent-manager","nodes":["B"]}).to_string()).await;
    assert!(common::wait_until(Duration::from_secs(10), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == prompts + 1).await, "B must reach the real external input gate");
    let before = manager_action(&s, "status", json!({"sessionId":"commit-concurrent-manager"}), 200).await;
    assert_eq!(before["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(before["snapshot"]["nodes"]["B"]["status"], "pending");
    let first = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/complete", &json!({"sessionId":"commit-concurrent-manager","nodeId":"A","reason":"Racing handoff one"}).to_string()).await;
    let second = common::OpenJsonRequest::submit(addr, "/api/workflow/manager/complete", &json!({"sessionId":"commit-concurrent-manager","nodeId":"A","reason":"Racing handoff two"}).to_string()).await;
    let first = first.response();
    let second = second.response();
    tokio::pin!(first, second);
    let pending = tokio::join!(tokio::time::timeout(Duration::from_millis(200), &mut first), tokio::time::timeout(Duration::from_millis(200), &mut second));
    assert!(pending.0.is_err() && pending.1.is_err(), "both duplicate requests must be outstanding at the held Run boundary");
    assert!(closes(&s).is_empty());
    fs::write(&dispatch_release, "release").unwrap();
    let dispatch = tokio::time::timeout(Duration::from_secs(10), dispatch.response()).await.unwrap();
    assert_eq!(dispatch.status, 200, "{}", dispatch.body);
    assert_eq!(serde_json::from_str::<Value>(&dispatch.body).unwrap()["results"][0]["status"], "running");
    let responses = tokio::time::timeout(Duration::from_secs(10), async { tokio::join!(&mut first, &mut second) }).await.expect("both complete requests must finish without releasing close");
    let mut results = Vec::new();
    for response in [responses.0, responses.1] {
        assert_eq!(response.status, 200, "{}", response.body);
        results.push(serde_json::from_str::<Value>(&response.body).unwrap());
    }
    assert_eq!(results.iter().filter(|value| value["changed"] == true).count(), 1);
    assert_eq!(results.iter().filter(|value| value["changed"] == false).count(), 1);
    assert_eq!(results[0]["snapshot"], results[1]["snapshot"]);
    let winner = results.iter().find(|value| value["changed"] == true).unwrap();
    let fact = &winner["snapshot"]["nodes"]["A"]["completion"];
    assert!(fact["id"].is_string());
    assert!([json!("Racing handoff one"), json!("Racing handoff two")].contains(&fact["reason"]));
    let read = get_over_tcp(addr, &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["run"], winner["snapshot"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(root.join("run.json")).unwrap()).unwrap(), winner["snapshot"]);
    let events = common::read_json_lines(&root.join("history.jsonl"));
    let completions: Vec<_> = events.iter().filter(|event| event["action"] == "complete").collect();
    assert_eq!(completions.len(), 1);
    for key in ["id", "at", "actor", "reason"] {
        assert_eq!(completions[0][key], fact[key]);
    }
    assert!(common::wait_until(Duration::from_secs(10), || !closes(&s).is_empty()).await);
    assert!(!close_release.exists());
    assert!(common::read_json_lines(&s.herdr.path().join("closed.jsonl")).is_empty());
    fs::write(&close_release, "release").unwrap();
    wait_closed(&s, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(closes(&s).iter().map(|call| call["argv"].clone()).collect::<Vec<_>>(), vec![json!(["tab", "close", tab])]);
    println!("P12 concurrent outstandingRequests=2 observationWindowMs=200 first={} second={} history={completions:?} closes={:?}", results[0], results[1], closes(&s));
}
