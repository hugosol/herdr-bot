//! One-time offline cutover; normal server startup never migrates handoff data.
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let maps = args.next();
    let runs = args.next();
    anyhow::ensure!(maps.is_some() && runs.is_some() && args.next().is_none(),
        "Usage: migrate-file-handoff <map-directory> <run-directory>\nStop all backend processes first. Originals are retained as *.file-handoff-v1.bak; publication is atomic per file, not across the whole tree.");
    let changed = herdr_bot::handoff_migration::migrate(
        std::path::Path::new(&maps.unwrap()), std::path::Path::new(&runs.unwrap()))?;
    println!("Migrated {changed} files. Original files retained as *.file-handoff-v1.bak. No workflow status was changed.");
    Ok(())
}
