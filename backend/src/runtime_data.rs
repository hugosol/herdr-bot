//! Startup-only disk cutover. This never restores transient process state or controls agents.
use std::path::{Path, PathBuf};

use anyhow::Context;
use herdr_bot::Config;
use herdr_bot::files::{FileAccess, FileStore};

pub fn prepare(config: &Config) -> anyhow::Result<()> {
    let mut paths = vec![PathBuf::from(".data"), PathBuf::from("zones.json")];
    for file in [&config.zones.registry_file, &config.tasks.storage_file] {
        paths.push(file.clone());
        if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
            paths.push(parent.to_path_buf());
        }
    }
    FileStore::default().with(&paths, |files| -> anyhow::Result<()> {
        files.create_dir_all(".data").context("创建运行数据目录 .data 失败")?;
        for file in [&config.zones.registry_file, &config.tasks.storage_file] {
            if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
                files.create_dir_all(parent)
                    .with_context(|| format!("创建持久文件目录失败：{}", parent.display()))?;
            }
        }
        let marker = Path::new(".data/.runtime-layout-v1");
        if !files.try_exists(marker).context("读取运行数据切换标记失败")? {
            migrate_zones(files, &config.zones.registry_file)?;
            // Terminology migration is explicit and offline. Retain old user data.
            files.write(marker, b"runtime layout v1\n").context("写入运行数据切换标记失败")?;
        }
        Ok(())
    }).context("协调运行数据初始化失败")?
}

fn migrate_zones(files: &FileAccess, target: &Path) -> anyhow::Result<()> {
    let source = Path::new("zones.json");
    if std::path::absolute(source)? == std::path::absolute(target)? {
        return Ok(());
    }
    if !files.try_exists(source).context("读取旧 zone 登记位置失败")? {
        return Ok(());
    }
    if !files.try_exists(target)
        .with_context(|| format!("读取 zone 登记目标失败：{}", target.display()))?
    {
        // create_new keeps an existing target authoritative, even if it appeared
        // after the existence check. Only a failed copy's own target is cleaned up.
        files.copy_if_absent(source, target)
            .with_context(|| format!("迁移 zone 登记失败：{}", target.display()))?;
    }
    anyhow::ensure!(
        files.metadata(target)
            .with_context(|| format!("读取 zone 登记目标失败：{}", target.display()))?
            .is_file(),
        "zone 登记目标不是文件：{}；保留旧登记。",
        target.display()
    );
    // The configured target is authoritative; a different old registry is user data.
    Ok(())
}
