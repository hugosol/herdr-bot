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
    pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
        let bytes = fs::read(path).map_err(|error| format!("{}：{error}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("{}：记录损坏：{error}", path.display()))
    }
    pub fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, String> {
        let bytes = fs::read(path).map_err(|error| format!("{}：{error}", path.display()))?;
        if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
            return Err(format!("{}：JSONL 尾部残缺；拒绝读取或继续追加。", path.display()));
        }
        bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).enumerate()
            .map(|(line, bytes)| serde_json::from_slice(bytes)
                .map_err(|error| format!("{}：记录 {} 损坏：{error}", path.display(), line + 1)))
            .collect()
    }
    // Caller holds collection coordination across validation and append.
    // Neither missing files nor damaged tails are repaired by this operation.
    pub fn append_jsonl<T: serde::de::DeserializeOwned>(path: &Path, record: &impl serde::Serialize) -> Result<(), String> {
        Self::read_jsonl::<T>(path)?;
        let mut bytes = serde_json::to_vec(record).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new().append(true).open(path)
            .map_err(|error| format!("{}：{error}", path.display()))?;
        file.write_all(&bytes).and_then(|_| file.sync_all())
            .map_err(|error| format!("{}：{error}", path.display()))
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
