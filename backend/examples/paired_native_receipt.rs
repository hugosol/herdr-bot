//! Test-only transparent CLI recorder. No commands are implemented or emulated here.
use serde_json::{json, Value};
use std::io::{self, Write};
use std::{env, fs, process::Command, time::SystemTime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let root = executable.parent().ok_or("recorder directory is missing")?;
    let installed: Value = serde_json::from_slice(&fs::read(root.join("installed.json"))?)?;
    let installed = installed.as_str().ok_or("installed executable must be a JSON string")?;
    let args: Vec<_> = env::args_os().skip(1).collect();
    let started = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let stem = format!("{started}-{}", std::process::id());
    let argv: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
    fs::write(root.join(format!("{stem}.request")), serde_json::to_vec(&argv)?)?;
    let output = Command::new(installed).args(&args).output()?;
    let receipt = json!({"argv": argv, "exit": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout), "stderr": String::from_utf8_lossy(&output.stderr)});
    let temporary = root.join(format!("{stem}.tmp"));
    fs::write(&temporary, serde_json::to_vec(&receipt)?)?;
    fs::rename(temporary, root.join(format!("{stem}.json")))?;
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;
    std::process::exit(output.status.code().unwrap_or(1));
}
