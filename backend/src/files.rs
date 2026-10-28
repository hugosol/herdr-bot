//! Business-independent, instance-local file coordination. No lock spans external I/O.
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Default)]
pub struct FileStore {
    locks: Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>,
}
impl FileStore {
    pub async fn lock(&self, paths: &[PathBuf]) -> Vec<OwnedMutexGuard<()>> {
        let mut paths = paths.to_vec();
        paths.sort();
        paths.dedup();
        let locks = {
            let mut registry = self.locks.lock().await;
            paths
                .into_iter()
                .map(|path| registry.entry(path).or_default().clone())
                .collect::<Vec<_>>()
        };
        let mut guards = Vec::with_capacity(locks.len());
        for lock in locks {
            guards.push(lock.lock_owned().await);
        }
        guards
    }
    pub fn create(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }
    pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let temporary = path.with_extension(format!("tmp-{}", crate::chats::new_creation_id()));
        let result = (|| {
            Self::create(&temporary, bytes)?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
