//! P38, approved S2/S3/S4: HTTP deletion/dispatch, real records and owned host.
use super::*;

// An actual Windows filesystem boundary: a level-1 oplock pauses a competing
// open until the owning handle is closed. No product hooks or lock introspection.
mod file_gate {
    use std::{ffi::c_void, fs, os::windows::{fs::OpenOptionsExt, io::AsRawHandle}};
    #[repr(C)]
    struct Overlapped { internal: usize, internal_high: usize, offset: u32, offset_high: u32, event: *mut c_void }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
        fn DeviceIoControl(file: *mut c_void, code: u32, input: *const c_void, input_size: u32, output: *mut c_void, output_size: u32, bytes: *mut u32, overlapped: *mut Overlapped) -> i32;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
        fn CancelIoEx(file: *mut c_void, overlapped: *mut Overlapped) -> i32;
        fn GetOverlappedResult(file: *mut c_void, overlapped: *mut Overlapped, bytes: *mut u32, wait: i32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    pub struct Gate { file: Option<fs::File>, overlapped: Box<Overlapped> }
    impl Gate {
        pub fn new(path: &std::path::Path) -> Self {
            let file = fs::OpenOptions::new().read(true).write(true).share_mode(7).custom_flags(0x40000000).open(path).unwrap();
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            assert!(!event.is_null(), "{}", std::io::Error::last_os_error());
            let mut gate = Self { file: Some(file), overlapped: Box::new(Overlapped { internal: 0, internal_high: 0, offset: 0, offset_high: 0, event }) };
            let result = unsafe { DeviceIoControl(gate.file.as_ref().unwrap().as_raw_handle(), 0x90000, std::ptr::null(), 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut *gate.overlapped) };
            assert_eq!(result, 0);
            assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(997), "oplock must be pending");
            gate
        }
        pub fn reached(&self) -> bool { unsafe { WaitForSingleObject(self.overlapped.event, 0) == 0 } }
    }
    impl Drop for Gate {
        fn drop(&mut self) {
            if let Some(file) = self.file.take() {
                let mut bytes = 0;
                unsafe {
                    CancelIoEx(file.as_raw_handle(), &mut *self.overlapped);
                    GetOverlappedResult(file.as_raw_handle(), &mut *self.overlapped, &mut bytes, 1);
                }
                drop(file); // Acknowledge the break; release the competing open.
            }
            unsafe { CloseHandle(self.overlapped.event); }
        }
    }
}

fn nested(s: &Scenario) {
    fs::write(s.config.scheduler.bot_directory.join("nested.json"), json!({"id":"nested","name":"Misleading task name","when":"","plan_guide":"","node_guide":"","map_file":"inner.json"}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("inner.json"), json!({"title":"Same display name","description":"","nodes":{"A":{"title":"A","bot_id":"worker","input":"INNER","zone":"manager","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(s.config.scheduler.map_directory.join("work.json"), json!({"title":"Same display name","description":"","nodes":{"A":{"title":"A","bot_id":"nested","manager_task_id":s.manager_task,"input":"CHILD","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
}

// P38: deletion wins before a queued child creation. Late ordinary-node
// saves and the already accepted child request must not resurrect the parent.
#[tokio::test]
async fn history_deleted_parent_cannot_be_revived_by_queued_child_creation() {
    use futures_util::FutureExt;
    let s = Scenario::new().await;
    nested(&s);
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["B"] = map["nodes"]["A"].clone();
    map["nodes"]["A"] = json!({"title":"ordinary","bot_id":"worker","input":"ORDINARY","zone":"node","blocked_by":[]});
    fs::write(&map_path, map.to_string()).unwrap();
    let started = s.start("history-delete-before-child").await;
    let id = started["id"].as_str().unwrap();
    let release = s.data.path().join("release-ordinary");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-ordinary"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let first = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"history-delete-before-child","nodes":["A"]}).to_string()).await;
    let mut queued = None;
    let observed = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2).await);
        queued = Some(common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"history-delete-before-child","nodes":["B"]}).to_string()).await);
        let calls = s.herdr.calls();
        let response = delete(&s, "work.json").await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert!(!s.config.scheduler.run_directory.join(id).exists());
        assert_eq!(s.herdr.calls(), calls);
        calls
    }).catch_unwind().await;
    let released = fs::write(&release, "release owned ordinary node");
    let first = tokio::time::timeout(Duration::from_secs(10), first.response()).await;
    let queued = match queued { Some(request) => Some(tokio::time::timeout(Duration::from_secs(10), request.response()).await), None => None };
    assert!(released.is_ok() && first.is_ok() && queued.as_ref().is_none_or(Result::is_ok), "original={:?}; release={released:?}; first cleanup={:?}; queued cleanup={:?}", observed.as_ref().err(), first.as_ref().err(), queued.as_ref().and_then(|result| result.as_ref().err()));
    let calls = match observed { Ok(calls) => calls, Err(error) => std::panic::resume_unwind(error) };
    assert_eq!(queued.unwrap().unwrap().status, 500);
    let list = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(serde_json::from_str::<Value>(&list.body).unwrap()["runs"], json!([]));
    assert_eq!(fs::read_dir(&s.config.scheduler.run_directory).unwrap().count(), 0);
    assert_eq!(s.herdr.calls(), calls, "no child startup, retry or external rollback");
}

// Stages are controlled beyond the production CLI, not by private locks.
// Catch observations so every failure still releases and awaits owned work.
#[tokio::test]
async fn history_deletion_during_child_identity_and_submission_preserves_both_links() {
    use futures_util::FutureExt;
    for gate in ["native_session_gate", "prompt_gate"] {
        let s = Scenario::new().await;
        nested(&s);
        let parent = s.start("history-gated-parent").await;
        let parent_id = parent["id"].as_str().unwrap();
        let release = s.data.path().join("release-child");
        let mut behavior = json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-gated-child"},"native_cwd":s.data.path().join("node-zone")});
        behavior[gate] = json!({"wait_for_file":release,"timeout_ms":30000});
        s.herdr.set_behavior(behavior);
        let pending = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"history-gated-parent","nodes":["A"]}).to_string()).await;
        let observed = std::panic::AssertUnwindSafe(async {
            let kind = if gate == "native_session_gate" { "agent_get" } else { "agent_prompt" };
            assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == kind).count() == 2).await, "child must reach {gate}");
            let parent_run: Value = serde_json::from_slice(&records(&s, parent_id)[0]).unwrap();
            let child_id = parent_run["nodes"]["A"]["execution"]["childRunId"].as_str().unwrap();
            let child: Value = serde_json::from_slice(&records(&s, child_id)[0]).unwrap();
            assert_eq!(child["parent"], json!({"runId":parent_id,"nodeId":"A","executionId":parent_run["nodes"]["A"]["execution"]["id"]}));
            fs::remove_file(s.config.scheduler.bot_directory.join("nested.json")).unwrap();
            let before = [records(&s, parent_id), records(&s, child_id)];
            let calls = s.herdr.calls();
            for map in ["work.json", "inner.json"] {
                let response = tokio::time::timeout(Duration::from_secs(5), delete(&s, map)).await.expect("history coordination must not span host gate");
                assert_eq!(response.status, 500, "{}", response.body);
                assert!(response.body.contains(parent_id) && response.body.contains(child_id), "{}", response.body);
                assert_eq!([records(&s, parent_id), records(&s, child_id)], before);
            }
            assert_eq!(s.herdr.calls(), calls);
            child_id.to_owned()
        }).catch_unwind().await;
        let release_result = fs::write(&release, "release owned child");
        let drained = tokio::time::timeout(Duration::from_secs(10), pending.response()).await;
        assert!(release_result.is_ok() && drained.is_ok(), "original={:?}; gate cleanup={release_result:?}; request cleanup={:?}", observed.as_ref().err(), drained.as_ref().err());
        let child_id = match observed { Ok(id) => id, Err(error) => std::panic::resume_unwind(error) };
        let response = drained.unwrap();
        assert_eq!(response.status, 200, "{}", response.body);
        let parent: Value = serde_json::from_slice(&records(&s, parent_id)[0]).unwrap();
        let child: Value = serde_json::from_slice(&records(&s, &child_id)[0]).unwrap();
        assert_eq!(parent["nodes"]["A"]["execution"]["childRunId"], child_id);
        assert_eq!(child["parent"]["runId"], parent_id);
        assert_eq!(child["manager"], parent["nodes"]["A"]["execution"]["chat"]);
        assert!(!child["manager"].is_null());
    }
}

// P38: freeze the real deletion scan at a later unrelated record after its
// candidate enumeration. A queued nested dispatch is then allowed to proceed.
// Without scan/publication coordination it can create a child outside the
// selected snapshot, after which deletion would leave that child dangling.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn history_scan_and_child_publication_cannot_both_leave_a_dangling_reference() {
    use futures_util::FutureExt;
    let s = Scenario::new().await;
    nested(&s);
    let map_path = s.config.scheduler.map_directory.join("work.json");
    let mut map: Value = serde_json::from_slice(&fs::read(&map_path).unwrap()).unwrap();
    map["nodes"]["B"] = map["nodes"]["A"].clone();
    map["nodes"]["A"] = json!({"title":"ordinary","bot_id":"worker","input":"ORDINARY","zone":"node","blocked_by":[]});
    fs::write(&map_path, map.to_string()).unwrap();
    let started = s.start("history-scan-parent").await;
    let id = started["id"].as_str().unwrap();
    let unrelated = s.config.scheduler.run_directory.join("zz-scan-gate");
    fs::create_dir(&unrelated).unwrap();
    let mut record = started["run"].clone();
    record["id"] = json!("zz-scan-gate");
    record["map"] = json!("unrelated.json");
    record["manager"] = Value::Null;
    fs::write(unrelated.join("run.json"), record.to_string()).unwrap();
    fs::write(unrelated.join("history.jsonl"), b"").unwrap();
    fs::write(unrelated.join("messages.jsonl"), b"").unwrap();
    let unrelated_before = fs::read(unrelated.join("run.json")).unwrap();
    let release = s.data.path().join("release-scan-ordinary");
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-scan-ordinary"},"native_cwd":s.data.path().join("node-zone"),"prompt_gate":{"wait_for_file":release,"timeout_ms":30000}}));
    let first = common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"history-scan-parent","nodes":["A"]}).to_string()).await;
    let mut queued = None;
    let mut deletion = None;
    let mut gate = None;
    let observed = std::panic::AssertUnwindSafe(async {
        assert!(common::wait_until(Duration::from_secs(5), || s.herdr.calls().iter().filter(|call| call["kind"] == "agent_prompt").count() == 2).await);
        queued = Some(common::OpenJsonRequest::submit(s.server.addr(), "/api/workflow/manager/dispatch", &json!({"sessionId":"history-scan-parent","nodes":["B"]}).to_string()).await);
        gate = Some(file_gate::Gate::new(&unrelated.join("run.json")));
        deletion = Some(common::OpenJsonRequest::submit(s.server.addr(), "/api/scheduler/maps/work.json/delete", r#"{"deleteHistory":true}"#).await);
        assert!(common::wait_until(Duration::from_secs(5), || gate.as_ref().unwrap().reached()).await, "real deletion read must reach the filesystem gate");
        fs::write(&release, "release ordinary input").unwrap();
        // B was accepted before the scan; allow it to attempt publication while
        // deletion is still held. This is an observation window, not ordering.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }).catch_unwind().await;
    let released = fs::write(&release, "release owned ordinary input");
    drop(gate);
    let first = tokio::time::timeout(Duration::from_secs(10), first.response()).await;
    let queued = match queued { Some(request) => Some(tokio::time::timeout(Duration::from_secs(10), request.response()).await), None => None };
    let deletion = match deletion { Some(request) => Some(tokio::time::timeout(Duration::from_secs(10), request.response()).await), None => None };
    assert!(released.is_ok() && first.is_ok() && queued.as_ref().is_none_or(Result::is_ok) && deletion.as_ref().is_none_or(Result::is_ok), "original={:?}; release={released:?}; first={:?}; queued={:?}; deletion={:?}", observed.as_ref().err(), first.as_ref().err(), queued.as_ref().and_then(|result| result.as_ref().err()), deletion.as_ref().and_then(|result| result.as_ref().err()));
    if let Err(error) = observed { std::panic::resume_unwind(error); }
    let deletion = deletion.unwrap().unwrap();
    assert_eq!(deletion.status, 200, "{}", deletion.body);
    assert_eq!(queued.unwrap().unwrap().status, 500);
    assert!(!s.config.scheduler.run_directory.join(id).exists());
    let entries = fs::read_dir(&s.config.scheduler.run_directory).unwrap().map(|entry| entry.unwrap().file_name()).collect::<Vec<_>>();
    assert_eq!(entries, vec![std::ffi::OsString::from("zz-scan-gate")]);
    assert_eq!(fs::read(unrelated.join("run.json")).unwrap(), unrelated_before);
    assert_eq!(s.herdr.calls().iter().filter(|call| call["kind"] == "agent_start").count(), 2, "no child host start or rollback");
}

async fn delete(s: &Scenario, map: &str) -> common::HttpResponse {
    post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{map}/delete"), r#"{"deleteHistory":true}"#).await
}

fn records(s: &Scenario, id: &str) -> [Vec<u8>; 3] {
    ["run.json", "history.jsonl", "messages.jsonl"].map(|name| fs::read(s.config.scheduler.run_directory.join(id).join(name)).unwrap())
}

// Independent oracle: P38 prohibits BOTH directions, even if both records
// would be selected. A scanner only following current nodes or Bot definitions
// would allow one side to disappear. Remove the definition reference first so
// the request reaches the history boundary, not the separate Bot guard.
#[tokio::test]
async fn history_rejects_parent_and_child_deletion_without_unlinking_or_host_effects() {
    let s = Scenario::new().await;
    nested(&s);
    let parent = s.start("history-parent").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-child"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatched = s.dispatch("history-parent", &["A"]).await;
    let parent_id = parent["id"].as_str().unwrap();
    let execution = &dispatched["snapshot"]["nodes"]["A"]["execution"];
    let child_id = execution["childRunId"].as_str().unwrap();
    fs::remove_file(s.config.scheduler.bot_directory.join("nested.json")).unwrap();
    let before = [records(&s, parent_id), records(&s, child_id)];
    let calls = s.herdr.calls();
    for map in ["work.json", "inner.json"] {
        let rejected = delete(&s, map).await;
        assert_eq!(rejected.status, 500, "{map}: {}", rejected.body);
        let error: Value = serde_json::from_str(&rejected.body).unwrap();
        assert_eq!(error["code"], "map-history-delete-failed");
        for locator in [parent_id, child_id, "A", execution["id"].as_str().unwrap()] {
            assert!(error["message"].as_str().unwrap().contains(locator), "missing {locator}: {error}");
        }
        assert!(s.config.scheduler.map_directory.join(map).is_file());
        assert_eq!([records(&s, parent_id), records(&s, child_id)], before);
        for id in [parent_id, child_id] {
            assert_eq!(get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await.status, 200);
        }
    }
    assert_eq!(s.herdr.calls(), calls);
}

// P38: an unreadable unrelated record might contain an incoming reference.
// A real Windows sharing denial must fail closed, not become an empty catalog.
#[tokio::test]
async fn history_unreadable_association_preserves_all_records_and_map() {
    use std::os::windows::fs::OpenOptionsExt;
    let s = Scenario::new().await;
    let started = s.start("history-unreadable").await;
    let id = started["id"].as_str().unwrap();
    let before = records(&s, id);
    let unknown = s.config.scheduler.run_directory.join("unknown-history");
    fs::create_dir(&unknown).unwrap();
    let mut record = started["run"].clone();
    record["id"] = json!("unknown-history");
    record["map"] = json!("unrelated.json");
    let path = unknown.join("run.json");
    fs::write(&path, record.to_string()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let held = fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
    assert!(fs::read(&path).is_err(), "fixture must actually deny reads");
    let rejected = delete(&s, "work.json").await;
    assert_eq!(rejected.status, 500, "{}", rejected.body);
    assert!(rejected.body.contains("run.json"), "{}", rejected.body);
    assert!(s.config.scheduler.map_directory.join("work.json").is_file());
    assert_eq!(records(&s, id), before);
    drop(held);
    assert_eq!(fs::read(path).unwrap(), bytes);
}

// P38 also protects incomplete/retired facts after restart. The missing
// counterpart must not turn an incoming reference into 'unassociated history'.
#[tokio::test]
async fn history_scan_protects_one_sided_and_retired_execution_links_after_restart() {
    for child_side in [true, false] {
        let mut s = Scenario::new().await;
        nested(&s);
        let parent = s.start("history-retained-parent").await;
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"history-retained-child"},"native_cwd":s.data.path().join("node-zone")}));
        let dispatched = s.dispatch("history-retained-parent", &["A"]).await;
        let parent_id = parent["id"].as_str().unwrap();
        let mut parent_run = dispatched["snapshot"].clone();
        let execution = parent_run["nodes"]["A"]["execution"].clone();
        let child_id = execution["childRunId"].as_str().unwrap();
        let parent_path = s.config.scheduler.run_directory.join(parent_id).join("run.json");
        let child_path = s.config.scheduler.run_directory.join(child_id).join("run.json");
        if child_side {
            parent_run["nodes"]["A"]["execution"] = Value::Null;
        } else {
            parent_run["nodes"]["A"]["previousExecutions"] = json!([{"execution":execution,"failure":null}]);
            parent_run["nodes"]["A"]["execution"] = Value::Null;
            let mut child: Value = serde_json::from_slice(&fs::read(&child_path).unwrap()).unwrap();
            child.as_object_mut().unwrap().remove("parent");
            fs::write(&child_path, child.to_string()).unwrap();
        }
        fs::write(&parent_path, parent_run.to_string()).unwrap();
        fs::remove_file(s.config.scheduler.bot_directory.join("nested.json")).unwrap();
        s.server = TestServer::start_app(herdr_bot::app(&s.config)).await;
        let before = [records(&s, parent_id), records(&s, child_id)];
        for map in ["work.json", "inner.json"] {
            let rejected = delete(&s, map).await;
            assert_eq!(rejected.status, 500, "{}", rejected.body);
            assert!(rejected.body.contains(parent_id) && rejected.body.contains(child_id), "{}", rejected.body);
            assert_eq!([records(&s, parent_id), records(&s, child_id)], before);
        }
    }
}
