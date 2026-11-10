//! Public title edits must preserve Map identity/content and optimistic revisions.
use super::*;

#[tokio::test]
async fn map_title_trims_allows_duplicates_and_preserves_other_definitions() {
    let s = Scenario::new().await;
    let path = s.config.scheduler.map_directory.join("work.json");
    let mut original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    original["group"] = json!("保留分组");
    fs::write(&path, original.to_string()).unwrap();
    let mut duplicate = original.clone();
    duplicate["title"] = json!("重复名称");
    fs::write(s.config.scheduler.map_directory.join("other.json"), duplicate.to_string()).unwrap();
    let bot_path = s.config.scheduler.bot_directory.join("worker.json");
    let bot_before = fs::read(&bot_path).unwrap();
    let runs_before = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs_before.status, 200, "{}", runs_before.body);
    let draft_response = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/title").await;
    assert_eq!(draft_response.status, 200, "{}", draft_response.body);
    let draft: Value = serde_json::from_str(&draft_response.body).unwrap();
    assert_eq!(draft["title"], "map");
    assert_eq!(draft["revision"]["file"], "work.json");
    let original_bytes = fs::read(&path).unwrap();
    let blank = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/title", &json!({"title":" \n\t ","revision":draft["revision"]}).to_string()).await;
    assert_eq!(blank.status, 400, "{}", blank.body);
    assert_eq!(fs::read(&path).unwrap(), original_bytes);
    let saved = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/title", &json!({"title":"  重复名称 \n", "revision":draft["revision"]}).to_string()).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let mut expected = original;
    expected["title"] = json!("重复名称");
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(), expected);
    let read = get_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json").await;
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(serde_json::from_str::<Value>(&read.body).unwrap()["title"], "重复名称");
    let saved_bytes = fs::read(&path).unwrap();
    let stale = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/work.json/title", &json!({"title":"过期编辑", "revision":draft["revision"]}).to_string()).await;
    assert_ne!(stale.status, 200, "{}", stale.body);
    assert_eq!(serde_json::from_str::<Value>(&stale.body).unwrap()["code"], "map-conflict");
    assert_eq!(fs::read(&path).unwrap(), saved_bytes);
    let wrong_file = post_json_over_tcp(s.server.addr(), "/api/scheduler/maps/other.json/title", &json!({"title":"错误文件", "revision":draft["revision"]}).to_string()).await;
    assert_ne!(wrong_file.status, 200, "{}", wrong_file.body);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(s.config.scheduler.map_directory.join("other.json")).unwrap()).unwrap(), duplicate);
    assert_eq!(fs::read(&bot_path).unwrap(), bot_before);
    let runs_after = get_over_tcp(s.server.addr(), "/api/workflow/runs").await;
    assert_eq!(runs_after.status, 200, "{}", runs_after.body);
    assert_eq!(runs_after.body, runs_before.body);
    assert!(s.herdr.calls().is_empty(), "renaming cannot launch Chats");
}
