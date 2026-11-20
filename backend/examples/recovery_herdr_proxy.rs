//! Acceptance-only transparent CLI recorder with one explicit notification fault.
//! Native hosts and Herdr remain installed programs; never used by product code.
use serde_json::{json, Value};
use std::{env, fs::{self, OpenOptions}, io::Write, process::{Command, Stdio}};

// A comparable monotonic clock for the separate invocation that makes the retry.
#[cfg(windows)]
fn clock_ms() -> u64 {
    #[link(name = "kernel32")]
    unsafe extern "system" { fn GetTickCount64() -> u64; }
    unsafe { GetTickCount64() }
}

#[cfg(not(windows))]
fn clock_ms() -> u64 { 0 }

fn main() {
    let root = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let config: Value = serde_json::from_slice(&fs::read(root.join("recovery-proxy.json")).unwrap()).unwrap();
    let args: Vec<String> = env::args().skip(1).collect();
    let notification = args.first().is_some_and(|s| s == "agent")
        && args.get(1).is_some_and(|s| s == "prompt")
        && args.get(3).is_some_and(|s| s.starts_with("Chief report\nReport: "));
    let rejected = notification && root.join("reject-notification").exists();
    let calls = root.join("calls");
    fs::create_dir_all(&calls).unwrap();
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let name = format!("{at}-{}", std::process::id());
    let temporary = calls.join(format!("{name}.tmp"));
    let mut record = OpenOptions::new().create_new(true).write(true).open(&temporary).unwrap();
    let bytes = serde_json::to_vec(&json!({"argv": args, "notification": notification, "rejected": rejected})).unwrap();
    record.write_all(&bytes).unwrap();
    drop(record);
    fs::rename(temporary, calls.join(format!("{name}.json"))).unwrap();
    if rejected {
        println!("{}", json!({"error":{"code":"acceptance-notification-rejected","message":"Explicit isolated external CLI fault before accepting input"}}));
        std::process::exit(1);
    }
    let mut child = Command::new(config["herdr"].as_str().unwrap());
    child.args(&args).stdin(Stdio::inherit());
    let submission = args.starts_with(&["agent".into(), "prompt".into()]) || args.first().is_some_and(|arg| arg == "send");
    if submission && config["recordSubmission"].as_bool() == Some(true) {
        let began = clock_ms();
        let output = child.output().unwrap();
        let finished = clock_ms();
        let started_false = output.status.success() && args[0] == "send" && config["startedFalse"].as_bool() == Some(true);
        let record = json!({"argv": args, "beganMs": began, "finishedMs": finished,
            "exit": output.status.code(), "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr), "injectedStartedFalse": started_false});
        let receipts = root.join("submissions");
        fs::create_dir_all(&receipts).unwrap();
        fs::write(receipts.join(format!("{name}.tmp")), serde_json::to_vec(&record).unwrap()).unwrap();
        fs::rename(receipts.join(format!("{name}.tmp")), receipts.join(format!("{name}.json"))).unwrap();
        if started_false {
            let mut response: Value = serde_json::from_slice(&output.stdout).unwrap();
            response["started"] = false.into();
            println!("{response}");
        } else {
            std::io::stdout().write_all(&output.stdout).unwrap();
        }
        std::io::stderr().write_all(&output.stderr).unwrap();
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let status = child.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
