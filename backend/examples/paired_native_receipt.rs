//! Test-only CLI recorder. Forward to the installed executable unless an
//! explicitly gated invocation is refused before spawning it.
use serde_json::{json, Value};
use std::io::{self, Write};
use std::{env, fs, path::Path, process::Command, thread, time::{Duration, Instant, SystemTime}};

fn decision(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match fs::read(path) {
            Ok(bytes) => return Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let root = executable.parent().ok_or("recorder directory is missing")?;
    let installed: Value = serde_json::from_slice(&fs::read(root.join("installed.json"))?)?;
    let installed = installed.as_str().ok_or("installed executable must be a JSON string")?;
    let args: Vec<_> = env::args_os().skip(1).collect();
    let started = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let stem = format!("{started}-{}", std::process::id());
    let argv: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
    let temporary = root.join(format!("{stem}.tmp"));
    fs::write(&temporary, serde_json::to_vec(&argv)?)?;
    fs::rename(&temporary, root.join(format!("{stem}.request")))?;
    let gated = match fs::read(root.join("gate")) {
        Ok(bytes) => serde_json::from_slice::<Vec<Vec<String>>>(&bytes)?.iter()
            .any(|prefix| argv.iter().map(|arg| arg.as_ref()).zip(prefix).all(|(arg, expected)| arg == expected)
                && argv.len() >= prefix.len()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let decision = if gated { decision(&root.join(format!("{stem}.decision")))? } else { Value::Null };
    let forwarded = decision.is_null();
    let (exit, stdout, stderr) = if forwarded {
        let output = Command::new(installed).args(&args).output()?;
        (output.status.code().unwrap_or(1), output.stdout, output.stderr)
    } else {
        let exit = decision["exit"].as_i64().and_then(|value| i32::try_from(value).ok())
            .filter(|value| *value != 0).ok_or("injected refusal needs a nonzero exit")?;
        let bytes = |key| decision[key].as_str().unwrap_or("").as_bytes().to_vec();
        (exit, bytes("stdout"), bytes("stderr"))
    };
    let receipt = json!({"argv": argv, "exit": exit, "forwarded": forwarded,
        "stdout": String::from_utf8_lossy(&stdout), "stderr": String::from_utf8_lossy(&stderr)});
    fs::write(&temporary, serde_json::to_vec(&receipt)?)?;
    fs::rename(temporary, root.join(format!("{stem}.json")))?;
    io::stdout().write_all(&stdout)?;
    io::stderr().write_all(&stderr)?;
    std::process::exit(exit);
}
