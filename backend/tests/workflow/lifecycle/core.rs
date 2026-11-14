// Approved S2/S3/S4 seams. P18.03/P20.03/P36.01/P68.04: pause only
// changes dispatch eligibility. A plausible violation stops A or blocks its
// report/complete along with B's dispatch. Keep real HTTP, persistence and host
// adapter calls in path; observe active A, literal rejection, first history and
// no external calls during the bounded 150ms pause window.
#[tokio::test]
async fn pause_blocks_new_dispatch_but_preserves_active_chat_handoff_and_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-pause").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-pause-A"},"native_cwd":s.data.path().join("node-zone")}));
    assert_eq!(s.dispatch("manager-pause", &["A"]).await["results"][0]["status"], "running");
    for event in [json!({"sessionId":"node-pause-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":"node-pause-A","kind":"state","data":{"value":"working"}})] {
        assert_eq!(post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await.status, 200);
    }
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &json!({"sessionId":"manager-pause"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let paused: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(paused["changed"], true);
    assert_eq!(paused["snapshot"]["status"], "paused");
    assert_eq!(paused["snapshot"]["nodes"]["A"]["status"], "running");
    assert_eq!(paused["ready"], json!([]));
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["action"], "pause");
    assert_eq!(events[2]["before"], "running");
    assert_eq!(events[2]["after"], "paused");
    assert_eq!(events[2]["actor"], started["run"]["manager"]);
    assert!(events[2]["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(events[2]["at"].as_u64().is_some());
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &json!({"sessionId":"manager-pause"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], paused["snapshot"]);
    let rejected = s.dispatch("manager-pause", &["B"]).await;
    assert_eq!(rejected["results"], json!([{"nodeId":"B","status":"rejected","reasons":["Run 状态为 paused"]}]));
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls, "pause must not stop or launch Chats");
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-pause-A","body":"A ready; cleanup continues while paused","canComplete":true}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let report: Value = serde_json::from_str(&report.body).unwrap();
    assert_eq!(report["saved"], true);
    assert_eq!(report["notification"]["status"], "submitted");
    let calls_after_report = s.herdr.calls();
    assert_eq!(calls_after_report.len(), calls.len() + 1, "only the requested report notification is sent");
    let complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-pause","nodeId":"A","reason":"Accepted while Chat remains working"}).to_string()).await;
    assert_eq!(complete.status, 200, "{}", complete.body);
    let complete: Value = serde_json::from_str(&complete.body).unwrap();
    assert_eq!(complete["snapshot"]["status"], "paused");
    assert_eq!(complete["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(complete["pendingReports"], json!({}));
    assert_eq!(complete["ready"], json!([]));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls_after_report);
    println!("P18.03/P20.03/P36.01/P68.04 observationWindowMs=150 pause={paused} rejected={rejected} report={report} complete={complete} callsBeforeReport={calls:?} callsAfterReport={calls_after_report:?} history={events:?}");
}

// P20.03/P37.01/P68.04 at S2/S3/S4. Contract source: resume changes
// paused -> running only. A plausible violation starts ready B automatically,
// or repeated resume overwrites the first transition. After real pause/resume,
// observe literal pending/ready facts, byte-identical repeated facts/history,
// and unchanged external calls for 150ms before explicitly dispatching B.
#[tokio::test]
async fn resume_restores_only_eligibility_until_explicit_dispatch_and_preserves_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-resume").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let body = json!({"sessionId":"manager-resume"}).to_string();
    let paused = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &body).await;
    assert_eq!(paused.status, 200, "{}", paused.body);
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", &body).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let resumed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(resumed["changed"], true);
    assert_eq!(resumed["snapshot"]["status"], "running");
    assert_eq!(resumed["ready"], json!(["A", "B"]));
    assert_eq!(resumed["snapshot"]["nodes"], started["run"]["nodes"]);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["action"], "resume");
    assert_eq!(events[2]["before"], "paused");
    assert_eq!(events[2]["after"], "running");
    assert_eq!(events[2]["actor"], started["run"]["manager"]);
    assert!(events[2]["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(events[2]["at"].as_u64().is_some());
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/resume", &body).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], resumed["snapshot"]);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &body).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], resumed[field]); }
    assert_eq!(s.herdr.calls(), calls, "resume must not start ready work");
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-resume-B"},"native_cwd":s.data.path().join("node-zone")}));
    let explicit = s.dispatch("manager-resume", &["B"]).await;
    assert_eq!(explicit["results"][0]["status"], "running");
    assert_eq!(explicit["snapshot"]["nodes"]["A"]["status"], "pending");
    let after = s.herdr.calls();
    assert_eq!(after.iter().filter(|call| call["kind"] == "agent_start").count(), 2, "only manager and explicit B start");
    println!("P20.03/P37.01/P68.04 observationWindowMs=150 resume={resumed} query={query} explicit={explicit} callsBeforeDispatch={calls:?} callsAfterDispatch={after:?} history={events:?}");
}

// P35.02 at S2/S3/S4: the approved finish condition is all nodes completed,
// even while paused and a Chat is working. A plausible violation requires
// resume/Chat exit, or permits terminal pause/resume. Reach this state through
// real dispatch/complete/pause (not a hand-built snapshot), then compare the
// first finish facts/history and all host calls after terminal requests.
#[tokio::test]
async fn paused_run_finishes_with_active_chat_and_terminal_eligibility_actions_are_rejected() {
    let s = Scenario::new().await;
    let started = s.start("manager-paused-finish").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let body = json!({"sessionId":"manager-paused-finish"}).to_string();
    for node in ["A", "B", "C"] {
        let identity = format!("node-paused-finish-{node}");
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":identity},"native_cwd":s.data.path().join("node-zone")}));
        assert_eq!(s.dispatch("manager-paused-finish", &[node]).await["results"][0]["status"], "running");
        if node == "C" {
            for event in [json!({"sessionId":identity,"kind":"register","data":{"cwd":s.data.path().join("node-zone")}}), json!({"sessionId":identity,"kind":"state","data":{"value":"working"}})] {
                assert_eq!(post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await.status, 200);
            }
            let paused = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/pause", &body).await;
            assert_eq!(paused.status, 200, "{}", paused.body);
        }
        submit_report(&s, &identity, "Ready while Chat continues working", true).await;
        let complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-paused-finish","nodeId":node,"reason":"Ready for handoff; Chat keeps working"}).to_string()).await;
        assert_eq!(complete.status, 200, "{}", complete.body);
        if node == "C" {
            let complete: Value = serde_json::from_str(&complete.body).unwrap();
            assert_eq!(complete["snapshot"]["status"], "paused");
            assert_eq!(complete["canFinish"], true);
        }
    }
    let calls = s.herdr.calls();
    let finished = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &body).await;
    assert_eq!(finished.status, 200, "{}", finished.body);
    let finished: Value = serde_json::from_str(&finished.body).unwrap();
    assert_eq!(finished["changed"], true);
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["actor"], started["run"]["manager"]);
    assert_eq!(finished["canFinish"], false);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 9);
    assert_eq!(events[8]["action"], "finish");
    assert_eq!(events[8]["before"], "paused");
    assert_eq!(events[8]["after"], "completed");
    for action in ["pause", "resume"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body).await;
        assert_eq!(response.status, 409, "{}", response.body);
        let rejected: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(rejected["code"], "run-terminal");
        assert_eq!(rejected["changed"], false);
        assert_eq!(rejected["snapshot"], finished["snapshot"]);
    }
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &body).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], finished["snapshot"]);
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &body).await;
    assert_eq!(query.status, 200, "{}", query.body);
    assert_eq!(serde_json::from_str::<Value>(&query.body).unwrap()["snapshot"], finished["snapshot"]);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P35.02 observationWindowMs=150 finish={finished} repeat={repeated} history={events:?} calls={calls:?}");
}

// Same approved HTTP/host/record seams: eligibility actions belong only to
// the uniquely associated manager; unknown/node identities and self-reported
// ownership must not change facts or call the host. Expectations are literal
// role/validation outcomes, not derived from the runtime's response.
#[tokio::test]
async fn pause_resume_reject_non_manager_and_caller_supplied_ownership_without_effects() {
    let s = Scenario::new().await;
    let started = s.start("manager-eligibility-owner").await;
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-eligibility-owner"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-eligibility-owner", &["A"]).await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let calls = s.herdr.calls();
    for action in ["pause", "resume"] {
        for (body, expected) in [
            (json!({"sessionId":"node-eligibility-owner"}), 403),
            (json!({"sessionId":"unassociated"}), 403),
            (json!({"sessionId":"manager-eligibility-owner","runId":started["id"]}), 400),
            (json!({"sessionId":"manager-eligibility-owner","nodeId":"A"}), 400),
            (json!({"sessionId":"manager-eligibility-owner","role":"manager"}), 400),
        ] {
            let response = post_json_over_tcp(s.server.addr(), &format!("/api/workflow/manager/{action}"), &body.to_string()).await;
            assert_eq!(response.status, expected, "{}", response.body);
            println!("pause/resume caller guard action={action} expected={expected} result={}", response.body);
        }
    }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls(), calls);
}

// Approved seams: S2 HTTP manager actions/status, S3 external host calls,
// S4 persisted current facts/history. P17.02/P18.02/P20.02/P29.01/P68.03:
// A/B -> C independently requires both predecessors completed. A plausible
// violation is treating A's report or completion as permission to launch C.
// Observe literal blocked/ready fields, unchanged host calls over 150ms, and
// explicit completion's first reason/history through the real HTTP boundary.
#[tokio::test]
async fn explicit_complete_unlocks_only_satisfied_dependencies_and_preserves_first_fact() {
    let s = Scenario::new().await;
    let started = s.start("manager-complete").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let initial = fs::read(root.join("run.json")).unwrap();
    let initial_calls = s.herdr.calls();
    let pending = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"not started"}).to_string()).await;
    assert_eq!(pending.status, 409, "{}", pending.body);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), initial);
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "prepare_inputs");
    assert_eq!(s.herdr.calls(), initial_calls);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-complete-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-complete", &["A"]).await;
    submit_report(&s, "node-complete-A", "Artifact ready; cleanup continues", true).await;
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"A artifact is ready; Chat still cleaning up"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let completed: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(completed["changed"], true);
    assert_eq!(completed["snapshot"]["nodes"]["A"]["status"], "completed");
    assert_eq!(completed["snapshot"]["nodes"]["A"]["completion"]["reason"], "A artifact is ready; Chat still cleaning up");
    assert_eq!(completed["ready"], json!(["B"]));
    assert_eq!(completed["blocked"], json!({"A":["node 状态为 completed，仅 pending 可派发"],"B":[],"C":["前置 B 尚未 completed"]}));
    assert_eq!(completed["canFinish"], false);
    assert_eq!(s.herdr.calls(), calls);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["action"], "complete");
    assert_eq!(events[2]["before"], "running");
    assert_eq!(events[2]["after"], "completed");
    assert_eq!(events[2]["actor"], started["run"]["manager"]);
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-complete","nodeId":"A","reason":"MUST NOT OVERWRITE"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], completed["snapshot"]);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    let rejected = s.dispatch("manager-complete", &["C"]).await;
    assert_eq!(rejected["results"], json!([{"nodeId":"C","status":"rejected","reasons":["前置 B 尚未 completed"]}]));
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-complete"}).to_string()).await;
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], completed[field]); }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P17.02/P18.02/P20.02/P68.03: observationWindowMs=150 calls={calls:?} complete={completed} query={query}");
}

// P17.02/P18.02/P20.02/P35.01: finish cannot infer handoff from live
// Chat state. Independent graph expectations: C is ready only after A and B;
// all completed permits finish, then no node is dispatchable. A plausible
// violation overwrites the first finish or permits complete in terminal state.
#[tokio::test]
async fn finish_requires_every_handoff_and_terminal_actions_preserve_success() {
    let s = Scenario::new().await;
    let started = s.start("manager-finish").await;
    let root = s.config.scheduler.run_directory.join(started["id"].as_str().unwrap());
    let unfinished = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(unfinished.status, 409, "{}", unfinished.body);
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "prepare_inputs");
    for node in ["A", "B", "C"] {
        s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("node-finish-{node}")},"native_cwd":s.data.path().join("node-zone")}));
        let dispatched = s.dispatch("manager-finish", &[node]).await;
        assert_eq!(dispatched["results"][0]["status"], "running");
        submit_report(&s, &format!("node-finish-{node}"), "Final handoff ready", true).await;
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-finish","nodeId":node,"reason":format!("{node} handoff ready while Chat active")}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        if node == "B" {
            assert_eq!(value["ready"], json!(["C"]));
            assert_eq!(value["blocked"]["C"], json!([]));
            assert_eq!(value["canFinish"], false);
        }
        if node == "C" { assert_eq!(value["canFinish"], true); }
    }
    let calls = s.herdr.calls();
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let finished: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(finished["changed"], true);
    assert_eq!(finished["snapshot"]["status"], "completed");
    assert_eq!(finished["snapshot"]["finished"]["actor"], started["run"]["manager"]);
    assert_eq!(finished["ready"], json!([]));
    assert_eq!(finished["canFinish"], false);
    let first = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let events = common::read_json_lines(&root.join("history.jsonl"));
    assert_eq!(events.len(), 8);
    assert_eq!(events[7]["action"], "finish");
    assert_eq!(events[7]["before"], "running");
    assert_eq!(events[7]["after"], "completed");
    let repeated = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(repeated.status, 200, "{}", repeated.body);
    let repeated: Value = serde_json::from_str(&repeated.body).unwrap();
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["snapshot"], finished["snapshot"]);
    let terminal_complete = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-finish","nodeId":"A","reason":"overwrite terminal"}).to_string()).await;
    assert_eq!(terminal_complete.status, 409, "{}", terminal_complete.body);
    let rejected = s.dispatch("manager-finish", &["A", "C"]).await;
    assert_eq!(rejected["results"], json!([
        {"nodeId":"A","status":"rejected","reasons":["Run 状态为 completed","node 状态为 completed，仅 pending 可派发"]},
        {"nodeId":"C","status":"rejected","reasons":["Run 状态为 completed","node 状态为 completed，仅 pending 可派发"]}
    ]));
    let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-finish"}).to_string()).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    for field in ["snapshot", "ready", "blocked", "canFinish"] { assert_eq!(query[field], finished[field]); }
    assert_eq!(fs::read(root.join("run.json")).unwrap(), first);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(s.herdr.calls(), calls);
    println!("P17.02/P18.02/P20.02/P35.01: observationWindowMs=150 calls={calls:?} finish={finished} query={query}");
}

// P28.02/P29.01/P68.03: reports, observed end-of-turn and session exit
// must leave A running, whereas explicit complete clears its actionable report.
// A plausible violation infers lifecycle completion or reopens work on a late
// report. Session events are real HTTP inputs, not evidence of native omp exit.
#[tokio::test]
async fn reports_and_observed_chat_lifecycle_never_replace_explicit_handoff() {
    let s = Scenario::new().await;
    let started = s.start("manager-lifecycle").await;
    let id = started["id"].as_str().unwrap();
    let root = s.config.scheduler.run_directory.join(id);
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"node-lifecycle-A"},"native_cwd":s.data.path().join("node-zone")}));
    s.dispatch("manager-lifecycle", &["A"]).await;
    let first_run = fs::read(root.join("run.json")).unwrap();
    let report = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"A artifact ready; cleanup continues","canComplete":true}).to_string()).await;
    assert_eq!(report.status, 200, "{}", report.body);
    let report: Value = serde_json::from_str(&report.body).unwrap();
    let calls = s.herdr.calls();
    for event in [
        json!({"sessionId":"node-lifecycle-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}),
        json!({"sessionId":"node-lifecycle-A","kind":"state","data":{"value":"awaiting-input"}}),
        json!({"sessionId":"node-lifecycle-A","kind":"unregister"})
    ] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        tokio::time::sleep(Duration::from_millis(150)).await;
        let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
        assert_eq!(query.status, 200, "{}", query.body);
        let query: Value = serde_json::from_str(&query.body).unwrap();
        assert_eq!(query["snapshot"]["nodes"]["A"]["status"], "running");
        assert_eq!(query["snapshot"]["status"], "running");
        assert_eq!(query["pendingReports"], json!({"A":report["report"]["id"]}));
        assert_eq!(query["ready"], json!(["B"]));
        assert_eq!(fs::read(root.join("run.json")).unwrap(), first_run);
        assert_eq!(s.herdr.calls(), calls);
        let rejected = s.dispatch("manager-lifecycle", &["C"]).await;
        assert_eq!(rejected["results"][0]["reasons"], json!(["前置 A 尚未 completed","前置 B 尚未 completed"]));
        assert_eq!(s.herdr.calls(), calls);
        println!("P29.01/P68.03: observationWindowMs=150 event={event} calls={calls:?} query={query}");
    }
    // Register active work again: handoff and finish must not demand an exit.
    for event in [json!({"sessionId":"node-lifecycle-A","kind":"register","data":{"cwd":s.data.path().join("node-zone")}}),json!({"sessionId":"node-lifecycle-A","kind":"state","data":{"value":"working"}})] {
        let response = post_json_over_tcp(s.server.addr(), "/api/sessions/events", &event.to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
    }
    for node in ["A", "B", "C"] {
        if node != "A" {
            s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":format!("node-lifecycle-{node}")},"native_cwd":s.data.path().join("node-zone")}));
            assert_eq!(s.dispatch("manager-lifecycle", &[node]).await["results"][0]["status"], "running");
            submit_report(&s, &format!("node-lifecycle-{node}"), "Handoff ready without waiting for Chat exit", true).await;
        }
        let calls_before_complete = s.herdr.calls();
        let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/complete", &json!({"sessionId":"manager-lifecycle","nodeId":node,"reason":"Handoff accepted; Chat may keep working"}).to_string()).await;
        assert_eq!(response.status, 200, "{}", response.body);
        let value: Value = serde_json::from_str(&response.body).unwrap();
        assert_eq!(value["pendingReports"], json!({}));
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(s.herdr.calls(), calls_before_complete);
        if node == "A" {
            let late = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"Post-handoff cleanup note","canComplete":false}).to_string()).await;
            assert_eq!(late.status, 200, "{}", late.body);
            let query = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/status", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
            let query: Value = serde_json::from_str(&query.body).unwrap();
            assert_eq!(query["pendingReports"], json!({}));
            assert_eq!(query["snapshot"], value["snapshot"]);
        }
        println!("P28.02/P68.03: postCompleteObservationWindowMs=150 calls={calls_before_complete:?} complete={value}");
    }
    let response = post_json_over_tcp(s.server.addr(), "/api/workflow/manager/finish", &json!({"sessionId":"manager-lifecycle"}).to_string()).await;
    assert_eq!(response.status, 200, "{}", response.body);
    let finished = fs::read(root.join("run.json")).unwrap();
    let history = fs::read(root.join("history.jsonl")).unwrap();
    let late = post_json_over_tcp(s.server.addr(), "/api/workflow/node/report", &json!({"sessionId":"node-lifecycle-A","body":"Post-finish cleanup finished","canComplete":true}).to_string()).await;
    assert_eq!(late.status, 200, "{}", late.body);
    let calls = s.herdr.calls();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let query = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{id}")).await;
    assert_eq!(query.status, 200, "{}", query.body);
    let query: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(query["run"]["status"], "completed");
    assert_eq!(query["run"]["nodes"]["A"]["status"], "completed");
    assert_eq!(query["pendingReports"], json!({}));
    assert_eq!(query["reports"].as_array().unwrap().len(), 5);
    assert_eq!(fs::read(root.join("run.json")).unwrap(), finished);
    assert_eq!(fs::read(root.join("history.jsonl")).unwrap(), history);
    assert_eq!(s.herdr.calls(), calls);
    println!("P28.02/P29.01/P68.03: terminalObservationWindowMs=150 calls={calls:?} detail={query}");
}

