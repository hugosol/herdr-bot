//! Approved seams: S2 public HTTP; disk fixtures supply independent server times,
//! never serve as an assertion side channel. S1 is covered by historyPagination.spec.ts.
mod common;

use std::path::Path;
use std::sync::Arc;
use common::{TempDir, TestServer, get_over_tcp};
use herdr_bot::{PowerOff, PowerOffFailure};
use serde_json::{Value, json};

struct NeverPowerOff;
impl PowerOff for NeverPowerOff {
    fn power_off(&self) -> Result<(), PowerOffFailure> { panic!("history never powers off") }
}

fn seed(root: &TempDir, count: usize, tied: bool) -> herdr_bot::Config {
    let mut config = common::config(Path::new("unused.bat"), root.path());
    config.sessions.history_file = root.path().join("history.json");
    let records: Vec<_> = (1..=count).map(|number| json!({
        "sessionId": format!("native-{number:03}"), "sessionName": format!("Conversation {number}"),
        "agentType": "omp", "cwd": root.path().to_str().unwrap(),
        "firstRecordedAt": if tied { 946684800000_u64 } else { 946684800000 + number as u64 * 1000 },
        "sessionFile": null
    })).collect();
    std::fs::write(&config.sessions.history_file, serde_json::to_vec(&records).unwrap()).unwrap();
    config
}

async fn page(server: &TestServer, number: usize) -> Value {
    let response = get_over_tcp(server.addr(), &format!("/api/sessions/history?page={number}")).await;
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

fn ids(body: &Value) -> Vec<String> {
    body["sessions"].as_array().unwrap().iter().map(|row| row["sessionId"].as_str().unwrap().to_owned()).collect()
}

// P13: literal boundary 51 requires 50 newest rows then the oldest. An unbounded
// list, off-by-one offset or ignored page query cannot satisfy these observations.
#[tokio::test]
async fn fifty_one_records_have_exact_first_and_last_pages() {
    let root = TempDir::new("history-pages-51");
    let config = seed(&root, 51, false);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let first = page(&server, 1).await;
    assert_eq!(first["pageSize"], 50);
    assert_eq!(first["page"], 1);
    assert_eq!(first["total"], 51);
    assert_eq!(first["totalPages"], 2);
    assert_eq!(ids(&first), (2..=51).rev().map(|n| format!("native-{n:03}")).collect::<Vec<_>>());
    let last = page(&server, 2).await;
    assert_eq!(last["page"], 2);
    assert_eq!(ids(&last), ["native-001"]);
    assert_eq!(page(&server, 1).await, first);
    assert_eq!(page(&server, 999).await, last);
}

// P13: empty, exactly full and multiple pages have exact independent ID ranges.
// Page metadata also defines first/last navigation without an inaccessible tail.
#[tokio::test]
async fn empty_full_and_multiple_pages_expose_every_record() {
    for count in [0, 50, 123] {
        let root = TempDir::new("history-page-boundaries");
        let config = seed(&root, count, false);
        let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
        let expected_ranges = match count { 0 => vec![(0, 0)], 50 => vec![(1, 50)], _ => vec![(74, 123), (24, 73), (1, 23)] };
        for (offset, (low, high)) in expected_ranges.iter().enumerate() {
            let result = page(&server, offset + 1).await;
            let expected: Vec<String> = if *high == 0 { vec![] } else { (*low..=*high).rev().map(|n| format!("native-{n:03}")).collect() };
            assert_eq!(ids(&result), expected);
            assert_eq!(result["total"], count);
            assert_eq!(result["page"], offset + 1);
            assert_eq!(result["totalPages"], expected_ranges.len());
            assert!(ids(&result).len() <= 50);
        }
        assert_eq!(page(&server, 0).await, page(&server, 1).await);
        assert_eq!(page(&server, 999).await, page(&server, expected_ranges.len()).await);
    }
}

// P12: all rows have the same independently supplied timestamp. Do not freeze
// the tie-break choice: freeze each observed page, then require the exact union.
#[tokio::test]
async fn equal_first_times_have_stable_pages_and_an_exact_complete_set() {
    let root = TempDir::new("history-tied-pages");
    let config = seed(&root, 123, true);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    let pages = [page(&server, 1).await, page(&server, 2).await, page(&server, 3).await];
    assert_eq!(ids(&pages[0]).len(), 50);
    assert_eq!(ids(&pages[1]).len(), 50);
    assert_eq!(ids(&pages[2]).len(), 23);
    let mut all: Vec<String> = pages.iter().flat_map(ids).collect();
    all.sort();
    assert_eq!(all, (1..=123).map(|n| format!("native-{n:03}")).collect::<Vec<_>>());
    for _ in 0..3 {
        for number in [3, 1, 2] { assert_eq!(page(&server, number).await, pages[number - 1]); }
    }
}

// P12: seeded server times put 003 > 002 > 001, independent of arrival order.
// Refreshing the old registration must not promote it or change its first time.
#[tokio::test]
async fn heartbeat_repeat_register_and_metadata_update_do_not_promote_old_history() {
    let root = TempDir::new("history-fixed-order");
    let config = seed(&root, 3, false);
    let server = TestServer::start_app(herdr_bot::app_with_power_off(&config, Arc::new(NeverPowerOff))).await;
    for (kind, data) in [
        ("register", json!({"cwd":"C:/old", "sessionName":"Initial"})),
        ("heartbeat", json!({})),
        ("register", json!({"cwd":"C:/updated", "sessionName":"Updated metadata", "agentType":"pi"})),
    ] {
        let response = common::post_json_over_tcp(server.addr(), "/api/sessions/events", &json!({"sessionId":"native-001", "kind":kind, "data":data}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let result = page(&server, 1).await;
        assert_eq!(ids(&result), ["native-003", "native-002", "native-001"]);
        assert_eq!(result["sessions"][2]["firstRecordedAt"], 946684801000_u64);
    }
    let result = page(&server, 1).await;
    assert_eq!(result["sessions"][2]["sessionName"], "Updated metadata");
    assert_eq!(result["sessions"][2]["cwd"], "C:/updated");
}

async fn all_history(server: &TestServer) -> Vec<Value> {
    let first = page(server, 1).await;
    let total_pages = first["totalPages"].as_u64().unwrap() as usize;
    let mut records = first["sessions"].as_array().unwrap().clone();
    for number in 2..=total_pages { records.extend(page(server, number).await["sessions"].as_array().unwrap().clone()); }
    records
}

// P07: all 123 old rows are registered through HTTP, then unregister, zone
// removal and controlled expiry run through real handlers/maintenance. Observing
// SSE proves expiry actually happened; public paginated reads prove no deletion.
#[tokio::test]
async fn unregister_zone_removal_expiry_age_and_growth_never_evict_history() {
    use common::sse::SseConnection;
    use herdr_bot::clock::ManualClock;
    use herdr_bot::sessions::{SessionPolicy, SessionsState};
    use std::time::Duration;
    let root = TempDir::new("history-independent-retention");
    let mut config = seed(&root, 123, false);
    config.zones.registry_file = root.path().join("zones.json");
    let clock = Arc::new(ManualClock::new());
    let sessions = Arc::new(SessionsState::with_policy_and_clock(SessionPolicy::default(), clock.clone()));
    let (app, state) = herdr_bot::app_with_state(&config, Arc::new(NeverPowerOff), sessions.clone());
    let server = TestServer::start_app(app).await;
    let post = |body: Value| {
        let server = &server;
        async move { common::post_json_over_tcp(server.addr(), "/api/sessions/events", &body.to_string()).await }
    };
    for number in 1..=123 {
        assert_eq!(post(json!({"sessionId":format!("native-{number:03}"),"kind":"register","data":{"cwd":root.path()}})).await.status, 200);
    }
    let before = all_history(&server).await;
    assert_eq!(before.len(), 123);
    assert_eq!(before[0]["sessionId"], "native-123");
    assert_eq!(before[122]["sessionId"], "native-001");
    assert_eq!(post(json!({"sessionId":"native-123","kind":"unregister","data":{}})).await.status, 200);
    assert_eq!(all_history(&server).await, before);
    for route in ["/api/zones", "/api/zones/remove"] {
        let result = common::post_json_over_tcp(server.addr(), route, &json!({"path":root.path()}).to_string()).await;
        assert_eq!(result.status, 200, "{}", result.body);
        assert_eq!(all_history(&server).await, before);
    }
    let mut observation = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    assert_eq!(observation.next_frame().await.json()["sessions"].as_array().unwrap().len(), 122);
    drop(observation);
    clock.advance(Duration::from_secs(41));
    sessions.scan_expired().await;
    state.shutdown.sessions_changed().await;
    let mut observation = SseConnection::open(server.addr(), "/api/sessions/events", &[]).await;
    assert_eq!(observation.next_frame().await.json()["sessions"], json!([]));
    assert_eq!(all_history(&server).await, before);
    clock.advance(Duration::from_secs(86400 * 365 * 20));
    sessions.scan_expired().await;
    assert_eq!(post(json!({"sessionId":"new-after-growth","kind":"register","data":{"cwd":"C:/new"}})).await.status, 200);
    let after = all_history(&server).await;
    assert_eq!(after.len(), 124);
    assert_eq!(after[0]["sessionId"], "new-after-growth");
    assert_eq!(&after[1..], before.as_slice());
}


