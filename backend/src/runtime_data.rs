//! Startup-only disk cutover. This never restores transient process state or controls agents.
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Context;
use herdr_bot::Config;

pub fn prepare(config: &Config) -> anyhow::Result<()> {
    fs::create_dir_all(".data").context("创建运行数据目录 .data 失败")?;
    for file in [
        &config.zones.registry_file,
        &config.bots.storage_file,
    ] {
        if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建持久文件目录失败：{}", parent.display()))?;
        }
    }
    let marker = Path::new(".data/.runtime-layout-v1");
    if !marker.try_exists().context("读取运行数据切换标记失败")? {
        migrate_zones(&config.zones.registry_file)?;
        // Terminology migration is an explicit offline operation. Startup must never
        // delete an old preset store, native session tree, or temporary user file.
        fs::write(marker, b"runtime layout v1\n").context("写入运行数据切换标记失败")?;
    }
    Ok(())
}

fn migrate_zones(target: &Path) -> anyhow::Result<()> {
    let source = Path::new("zones.json");
    // Preserve explicit registry configuration; never delete the configured registry itself.
    if std::path::absolute(source)? == std::path::absolute(target)? {
        return Ok(());
    }
    if !source.try_exists().context("读取旧 zone 登记位置失败")? {
        return Ok(());
    }
    if !target
        .try_exists()
        .with_context(|| format!("读取 zone 登记目标失败：{}", target.display()))?
    {
        // create_new keeps an existing target authoritative, including one created concurrently.
        let mut destination = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
        {
            Ok(file) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("创建 zone 登记目标失败：{}", target.display()));
            }
        };
        if let Some(file) = destination.as_mut() {
            let copied = (|| -> io::Result<()> {
                let mut old = fs::File::open(source)?;
                io::copy(&mut old, file)?;
                file.flush()?;
                file.sync_all()
            })();
            if let Err(error) = copied {
                drop(destination);
                let _ = fs::remove_file(target);
                return Err(error)
                    .with_context(|| format!("迁移 zone 登记失败：{}", target.display()));
            }
        }
    }
    anyhow::ensure!(
        fs::metadata(target)
            .with_context(|| format!("读取 zone 登记目标失败：{}", target.display()))?
            .is_file(),
        "zone 登记目标不是文件：{}；保留旧登记。",
        target.display()
    );
    // The configured target remains authoritative, but a different old registry
    // is user data, not garbage. Retain it even when the target already exists.
    Ok(())
}
