//! Startup-only disk cutover. This never restores transient process state or controls agents.
use std::path::{Path, PathBuf};

use anyhow::Context;
use herdr_bot::Config;
use herdr_bot::files::{FileAccess, FileStore, PublishMode};

pub fn prepare(config: &Config) -> anyhow::Result<()> {
    let mut paths = vec![PathBuf::from(".data"), PathBuf::from("zones.json"), PathBuf::from("meta-data")];
    for file in [&config.zones.registry_file, &config.tasks.storage_file] {
        paths.push(file.clone());
        if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
            paths.push(parent.to_path_buf());
        }
    }
    FileStore::default().with(&paths, |files| -> anyhow::Result<()> {
        migrate_definitions(files, config)?;
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

// Each definition has its own completion record: custom targets never consume a
// migration, while completed defaults never import a retained backup again.
fn migrate_definitions(files: &FileAccess, config: &Config) -> anyhow::Result<()> {
    let candidates = [
        (&config.tasks.storage_file, ".data/tasks.json", "meta-data/tasks.json", "tasks", false),
        (&config.scheduler.bot_directory, ".data/bots", "meta-data/bots", "bots", true),
        (&config.scheduler.map_directory, ".data/workflow-maps", "meta-data/workflow-maps", "maps", true),
    ];
    let mut pending = Vec::new();
    // Inspect every eligible source and complete target before creating anything.
    for (configured, source, target, name, directory) in candidates {
        if std::path::absolute(configured)? != std::path::absolute(target)? {
            continue;
        }
        let marker = PathBuf::from(format!(".data/.definition-layout-v1-{name}"));
        check_definition_path(files, &marker)?;
        if files.symlink_metadata(&marker).is_ok() {
            anyhow::ensure!(files.metadata(&marker)?.is_file(), "定义迁移标记不是文件：{}", marker.display());
            continue;
        }
        let source = PathBuf::from(source);
        let target = PathBuf::from(target);
        let old = definition_inventory(files, &source, directory)?;
        let new = definition_inventory(files, &target, directory)?;
        if let (Some(old), Some(new)) = (&old, &new) {
            anyhow::ensure!(old == new, "定义迁移冲突：{} 与 {} 内容不同；未覆盖或合并，保留旧源。", source.display(), target.display());
            for (relative, is_directory) in old {
                if !is_directory {
                    let old_file = if directory { source.join(relative) } else { source.clone() };
                    let new_file = if directory { target.join(relative) } else { target.clone() };
                    anyhow::ensure!(files.read(&old_file)? == files.read(&new_file)?, "定义迁移冲突：{} 与 {} 内容不同；未覆盖或合并，保留旧源。", old_file.display(), new_file.display());
                }
            }
        }
        pending.push((source, target, marker, directory, old, new.is_some()));
    }
    for (source, target, _, directory, old, target_exists) in &pending {
        if *target_exists || old.is_none() { continue; }
        let inventory = old.as_ref().expect("source checked");
        if let Some(parent) = target.parent() { files.create_dir_all(parent)?; }
        let mut owns_directory = false;
        let copied = (|| -> anyhow::Result<()> {
            if *directory {
                files.create_dir(target)?;
                owns_directory = true;
                for (relative, is_directory) in inventory {
                    let destination = target.join(relative);
                    if *is_directory {
                        if !relative.as_os_str().is_empty() { files.create_dir(&destination)?; }
                    } else {
                        anyhow::ensure!(files.copy_if_absent(source.join(relative), &destination)?, "定义迁移目标已出现：{}", destination.display());
                    }
                }
            } else {
                anyhow::ensure!(files.copy_if_absent(source, target)?, "定义迁移目标已出现：{}", target.display());
            }
            Ok(())
        })();
        if let Err(error) = copied {
            // Only this attempt's newly allocated directory can be removed.
            if owns_directory {
                files.remove_dir_all(target).with_context(|| format!("清理失败的定义迁移目录失败：{}；原始错误：{error:#}", target.display()))?;
            }
            return Err(error).with_context(|| format!("复制定义失败：{} -> {}", source.display(), target.display()));
        }
    }
    if !pending.is_empty() { files.create_dir_all(".data")?; }
    for (_, _, marker, _, _, _) in pending {
        files.publish_checked(&marker, b"definition layout v1\n", PublishMode::Create,
            |_| Ok::<(), std::io::Error>(()), |error| error)
            .with_context(|| format!("写入定义迁移完成标记失败：{}", marker.display()))?;
    }
    Ok(())
}

// Reject links (including Windows junction/reparse points) before reading any
// contents; check ancestors too, not merely the final directory entry.
fn check_definition_path(files: &FileAccess, path: &Path) -> anyhow::Result<()> {
    let absolute = std::path::absolute(path)?;
    let mut ancestor = PathBuf::new();
    for component in absolute.components() {
        ancestor.push(component);
        if !ancestor.is_absolute() { continue; }
        match files.symlink_metadata(&ancestor) {
            Ok(metadata) => {
                let linked = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    linked || metadata.file_attributes() & 0x400 != 0
                };
                anyhow::ensure!(!linked, "定义迁移拒绝链接或 junction：{}", ancestor.display());
                anyhow::ensure!(ancestor == absolute || metadata.is_dir(), "定义迁移父路径不是目录：{}", ancestor.display());
                anyhow::ensure!(metadata.is_file() || metadata.is_dir(), "定义迁移拒绝异常文件类型：{}", ancestor.display());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("检查定义迁移路径失败：{}", ancestor.display())),
        }
    }
    Ok(())
}

fn definition_inventory(files: &FileAccess, root: &Path, directory: bool)
    -> anyhow::Result<Option<std::collections::BTreeMap<PathBuf, bool>>>
{
    check_definition_path(files, root)?;
    let metadata = match files.symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("读取定义迁移路径失败：{}", root.display())),
    };
    anyhow::ensure!(if directory { metadata.is_dir() } else { metadata.is_file() }, "定义迁移路径类型错误：{}", root.display());
    let mut inventory = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        check_definition_path(files, &path)?;
        let metadata = files.symlink_metadata(&path)?;
        inventory.insert(path.strip_prefix(root)?.to_path_buf(), metadata.is_dir());
        if metadata.is_dir() {
            for entry in files.read_dir(&path)? { pending.push(entry.path()); }
        }
    }
    Ok(Some(inventory))
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
