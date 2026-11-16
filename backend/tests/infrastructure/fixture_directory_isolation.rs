#[path = "../common/mod.rs"]
mod common;

use std::fs;

#[test]
fn fixture_directory_does_not_reuse_a_previous_process_directory() {
    // A recycled Windows PID can collide with a directory left by an earlier run.
    // This test binary has not allocated a common TempDir yet, so its first suffix is 0.
    let label = "fixture recycled process isolation";
    let stale = std::env::temp_dir()
        .join("herdr-bot fixture checks")
        .join(format!("{label} {}-0", std::process::id()));
    fs::create_dir_all(&stale).expect("prepare stale fixture directory");
    let sentinel = stale.join("previous-run.txt");
    fs::write(&sentinel, "previous process state").expect("write stale state");

    let fresh = common::TempDir::new(label);
    assert_ne!(fresh.path(), stale, "a new fixture must not adopt stale state");
    assert!(!fresh.path().join("previous-run.txt").exists());
    drop(fresh);
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "previous process state");
    fs::remove_dir_all(stale).expect("remove test-owned stale fixture");
}
