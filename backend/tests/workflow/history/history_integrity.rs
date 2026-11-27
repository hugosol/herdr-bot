//! P30: Map deletion is independent of saved parent/child Run links.
use super::*;

#[tokio::test]
async fn deleting_source_maps_preserves_both_sides_of_nested_history() {
    let s = Scenario::new().await;
    let maps = &s.config.scheduler.map_directory;
    let nested = s.config.scheduler.bot_directory.join("nested.json");
    fs::write(&nested, json!({"id":"nested","name":"Nested","when":"","description":"","input_rule":"","output_rule":"","map_file":"inner.json"}).to_string()).unwrap();
    fs::write(maps.join("inner.json"), json!({"title":"Child","description":"","nodes":{"inside":{"title":"Child work","bot_id":"worker","input":"","zone":"manager","blocked_by":[]}}}).to_string()).unwrap();
    fs::write(maps.join("work.json"), json!({"title":"Parent","description":"","nodes":{"A":{"title":"Call child","bot_id":"nested","manager_task_id":s.manager_task,"input":"","zone":"node","blocked_by":[]}}}).to_string()).unwrap();
    let parent = s.start("retain-parent").await;
    let parent_id = parent["id"].as_str().unwrap();
    s.herdr.set_behavior(json!({"native_session":{"source":"herdr:omp","agent":"omp","kind":"id","value":"retain-child"},"native_cwd":s.data.path().join("node-zone")}));
    let dispatch = s.dispatch("retain-parent", &["A"]).await;
    assert_eq!(dispatch["results"][0]["status"], "running", "{dispatch}");
    let execution = &dispatch["snapshot"]["nodes"]["A"]["execution"];
    let child_id = execution["childRunId"].as_str().unwrap();
    // Remove the live Bot reference to the child definition, not either Run link.
    fs::remove_file(nested).unwrap();
    // This unrelated malformed record must neither block Map removal nor be modified.
    let unrelated = s.config.scheduler.run_directory.join("unrelated-run");
    fs::create_dir(&unrelated).unwrap();
    fs::write(unrelated.join("run.json"), b"unknown original bytes").unwrap();
    let records = |id: &str| ["run.json", "map.json", "history.jsonl", "messages.jsonl"].map(|name| {
        fs::read(s.config.scheduler.run_directory.join(id).join(name)).unwrap()
    });
    let before = [records(parent_id), records(child_id)];
    let calls = s.herdr.calls();
    for name in ["work.json", "inner.json"] {
        let response = post_json_over_tcp(s.server.addr(), &format!("/api/scheduler/maps/{name}/delete"), "{}").await;
        assert_eq!(response.status, 200, "{}", response.body);
        assert!(!maps.join(name).exists());
    }
    assert_eq!([records(parent_id), records(child_id)], before);
    assert_eq!(fs::read(unrelated.join("run.json")).unwrap(), b"unknown original bytes");
    let read = get_over_tcp(s.server.addr(), &format!("/api/workflow/runs/{child_id}")).await;
    assert_eq!(read.status, 200, "{}", read.body);
    let child: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(child["run"]["parent"], json!({"runId":parent_id,"nodeId":"A","executionId":execution["id"]}));
    assert_eq!(s.herdr.calls(), calls);
}
