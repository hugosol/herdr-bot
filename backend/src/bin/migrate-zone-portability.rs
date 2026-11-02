//! Explicit offline cutover; never run by normal startup.
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(args.len() == 3,
        "Usage: migrate-zone-portability <registry-file> <map-directory> <mapping.json>\nStop ALL backend processes first. Mapping JSON: [{{\"path\":\"C:/old/project\",\"id\":\"team/project\",\"local_path\":\"D:/current/project\"}}]. local_path is optional; aliases of one ID must declare the same local_path. Missing registry is allowed; Map directory must exist. No directories are created. Every source is validated before writes. Originals retained as *.zone-portability-v1.bak; atomic per file only. Runs and Chats remain unchanged.");
    let changed = herdr_bot::portability_migration::migrate(
        std::path::Path::new(&args[0]), std::path::Path::new(&args[1]), std::path::Path::new(&args[2]))?;
    println!("Migrated {changed} files. Originals retained as *.zone-portability-v1.bak. Historical Runs and Chats were not changed.");
    Ok(())
}
