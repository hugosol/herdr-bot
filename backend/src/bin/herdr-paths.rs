//! Read the same machine roots used by the service, without starting it.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use herdr_bot::paths::Roots;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--paths")) {
        bail!("usage: herdr-paths --paths <file>");
    }
    let file = args.next().context("usage: herdr-paths --paths <file>")?;
    if args.next().is_some() { bail!("usage: herdr-paths --paths <file>"); }
    let roots = Roots::load(&PathBuf::from(file))?;
    let stdout = std::io::stdout();
    serde_json::to_writer(stdout.lock(), &roots).context("写入路径根 JSON 失败")?;
    Ok(())
}
