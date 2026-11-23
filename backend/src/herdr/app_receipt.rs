use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    time::Duration,
};

use serde::Deserialize;
use tokio::time::{Instant, sleep_until};

use super::{HerdrError, unexpected};

const BIND: &str = "App launch receipt";
const RECEIPT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RECEIPT_BYTES: usize = 4096;
const UNREPORTED: &str = "-";

// App supplies its normal shell and PTY. Keep bootstrap data out of the script,
// and publish before native startup so neither toolkit nor ready is required.
const BOOTSTRAP: &str = concat!(
    "const [path, nonce, launchId, targetId, agent, ...args] = process.argv.slice(1); ",
    "const { HC_TERMINAL_ID: terminal, HC_TERMINAL_SOCKET: socket } = process.env; ",
    "if (!terminal || !socket) throw new Error('App launch has no core terminal origin'); ",
    "const { writeFileSync } = await import('node:fs'); ",
    "writeFileSync(path, JSON.stringify({nonce, terminal, socket, pid: process.pid}), {flag: 'r+'}); ",
    "const env = {...process.env}; ",
    "if (launchId === '-') { delete env.HERDR_BOT_LAUNCH_ID; delete env.HERDR_BOT_LAUNCH_TARGET_ID; } ",
    "else { env.HERDR_BOT_LAUNCH_ID = launchId; env.HERDR_BOT_LAUNCH_TARGET_ID = targetId; } ",
    "const child = Bun.spawn([agent, ...args], {env, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit'}); ",
    "process.exit(await child.exited);",
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Evidence {
    pub(super) nonce: String,
    pub(super) terminal: String,
    pub(super) socket: String,
    pub(super) pid: u32,
}

pub(super) struct Receipt {
    path: PathBuf,
    file: File,
    nonce: String,
}

impl Receipt {
    pub(super) fn new() -> Result<Self, HerdrError> {
        let not_started = |error| HerdrError::NotStarted(format!("{BIND}：{error}"));
        let nonce = nonce().map_err(not_started)?;
        let path = std::env::temp_dir().join(format!("herdr-bot-app-{}-{nonce}.json", std::process::id()));
        if !path.is_absolute() {
            return Err(HerdrError::NotStarted(format!("{BIND}：临时目录不是绝对路径")));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(not_started)?;
        Ok(Self { path, file, nonce })
    }

    pub(super) fn argv<'a>(
        &'a self, agent: &'a str, args: &'a [String], correlation: Option<(&'a str, &'a str)>,
    ) -> Result<Vec<&'a str>, HerdrError> {
        let path = self.path.to_str().ok_or_else(|| HerdrError::NotStarted(format!("{BIND}：临时路径不是 UTF-8")))?;
        let (launch_id, target_id) = correlation.unwrap_or((UNREPORTED, UNREPORTED));
        let mut argv = Vec::with_capacity(9 + args.len());
        argv.extend(["bun", "-e", BOOTSTRAP, "--", path, &self.nonce, launch_id, target_id, agent]);
        argv.extend(args.iter().map(String::as_str));
        Ok(argv)
    }

    pub(super) async fn receive(mut self, terminal: &str) -> Result<Evidence, HerdrError> {
        let deadline = Instant::now() + RECEIPT_TIMEOUT;
        let mut bytes = Vec::new();
        loop {
            self.file.seek(SeekFrom::Start(0)).and_then(|_| {
                self.file.by_ref().take(MAX_RECEIPT_BYTES as u64 + 1).read_to_end(&mut bytes)
            }).map_err(|error| unexpected(BIND, &format!("无法读取启动回执：{error}")))?;
            if bytes.len() > MAX_RECEIPT_BYTES {
                return Err(unexpected(BIND, "启动回执超出大小限制"));
            }
            match serde_json::from_slice::<Evidence>(&bytes) {
                Ok(evidence) if evidence.nonce == self.nonce && evidence.terminal == terminal && evidence.pid != 0 => return Ok(evidence),
                Ok(_) => return Err(unexpected(BIND, "启动回执的 nonce、终端或 PID 与本次启动不匹配")),
                Err(error) if !error.is_eof() => return Err(unexpected(BIND, "启动回执的 JSON 格式无效")),
                Err(_) => {}
            }
            let now = Instant::now();
            if now >= deadline { return Err(HerdrError::Timeout(BIND)); }
            bytes.clear();
            sleep_until(deadline.min(now + Duration::from_millis(25))).await;
        }
    }
}

impl Drop for Receipt {
    fn drop(&mut self) {
        // The producer opens with r+, so it cannot recreate a cancelled receipt.
        if let Err(error) = std::fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("{BIND}：无法删除临时回执：{error}");
        }
    }
}

fn nonce() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    random_bytes(&mut bytes)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut nonce = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        nonce.push(char::from(HEX[usize::from(byte >> 4)]));
        nonce.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(nonce)
}

#[cfg(unix)]
fn random_bytes(bytes: &mut [u8; 16]) -> io::Result<()> {
    File::open("/dev/urandom")?.read_exact(bytes)
}

#[cfg(windows)]
fn random_bytes(bytes: &mut [u8; 16]) -> io::Result<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(algorithm: *mut std::ffi::c_void, buffer: *mut u8, len: u32, flags: u32) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 2;
    // This flag selects the system RNG with a null algorithm handle.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status >= 0 { Ok(()) } else { Err(io::Error::other(format!("BCryptGenRandom: NTSTATUS {status:#x}"))) }
}
