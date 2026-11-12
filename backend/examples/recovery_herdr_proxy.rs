//! Acceptance-only transparent CLI recorder with one explicit notification fault.
//! Native hosts and Herdr remain installed programs; never used by product code.
use serde_json::{json, Value};
use std::{env, fs::{self, OpenOptions}, io::Write, process::{Command, Stdio}};

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
    let status = Command::new(config["herdr"].as_str().unwrap()).args(args)
        .stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
