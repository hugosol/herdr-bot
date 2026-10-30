//! Process-wide, overlapping-path coordination and scoped persistence operations.
//! Scopes cover subtrees. Callbacks are short, never nest acquisitions, and never
//! span external effects; unrelated scopes remain independent during filesystem I/O.
use std::{
    ffi::OsString,
    fs::{self, File, FileType, Metadata, OpenOptions},
    future::Future,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::{Condvar, LazyLock, Mutex, atomic::{AtomicU64, Ordering}},
};
use tokio::sync::Notify;

#[derive(Default)]
pub struct FileStore;

#[derive(Clone, Copy)]
pub enum PublishMode { Create, Replace }

/// An owned directory observation, not a filesystem handle or a new capability.
pub struct DirectoryEntry {
    path: PathBuf,
    name: OsString,
    file_type: io::Result<FileType>,
}
impl DirectoryEntry {
    pub fn path(&self) -> PathBuf { self.path.clone() }
    pub fn file_name(&self) -> OsString { self.name.clone() }
    pub fn file_type(&self) -> io::Result<FileType> {
        self.file_type.as_ref().copied().map_err(|error| {
            error.raw_os_error().map(io::Error::from_raw_os_error)
                .unwrap_or_else(|| io::Error::new(error.kind(), error.to_string()))
        })
    }
}

struct Scope { lexical: PathBuf, resolved: PathBuf }
/// Only borrowed inside a coordinated callback; cannot be constructed by callers.
pub struct FileAccess { scopes: Vec<Scope> }

#[derive(Default)]
struct Active { next_id: u64, scopes: Vec<(u64, Vec<PathBuf>)> }
struct Coordinator { active: Mutex<Active>, changed: Condvar, notified: Notify }
fn coordinator() -> &'static Coordinator {
    static COORDINATOR: LazyLock<Coordinator> = LazyLock::new(|| Coordinator {
        active: Mutex::new(Active::default()), changed: Condvar::new(), notified: Notify::new(),
    });
    &COORDINATOR
}
struct Lease(u64);
impl Drop for Lease {
    fn drop(&mut self) {
        let coordinator = coordinator();
        let mut active = coordinator.active.lock().unwrap_or_else(|error| error.into_inner());
        active.scopes.retain(|(id, _)| *id != self.0);
        drop(active);
        coordinator.changed.notify_all();
        coordinator.notified.notify_waiters();
    }
}
fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}
fn reserve(active: &mut Active, paths: &[PathBuf]) -> Option<Lease> {
    if active.scopes.iter().any(|(_, held)| held.iter().any(|held| paths.iter().any(|path| overlaps(held, path)))) {
        return None;
    }
    let id = active.next_id;
    active.next_id = active.next_id.checked_add(1).expect("file scope identifier exhausted");
    active.scopes.push((id, paths.to_vec()));
    Some(Lease(id))
}

fn absolute(path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {},
            Component::ParentDir => {
                if fs::symlink_metadata(&normalized).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "parent traversal through a symbolic link is not a coordinated path"));
                }
                normalized.pop();
            },
            _ => normalized.push(part.as_os_str()),
        }
    }
    Ok(normalized)
}
#[cfg(not(windows))]
fn identity(path: PathBuf) -> PathBuf { path }
#[cfg(windows)]
fn identity(path: PathBuf) -> PathBuf {
    use std::path::Prefix;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let mut plain = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => plain.push(format!("{}:", drive as char)),
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    plain.push(r"\\"); plain.push(server); plain.push(share);
                },
                _ => plain.push(part.as_os_str()),
            },
            _ => plain.push(part.as_os_str()),
        }
    }
    // Preserve non-Unicode filenames while folding the case-insensitive Windows identity.
    let mut folded = Vec::with_capacity(plain.as_os_str().len());
    for item in char::decode_utf16(plain.as_os_str().encode_wide()) {
        match item {
            Ok(character) => for character in character.to_lowercase() {
                let mut units = [0; 2];
                folded.extend_from_slice(character.encode_utf16(&mut units));
            },
            Err(error) => folded.push(error.unpaired_surrogate()),
        }
    }
    PathBuf::from(OsString::from_wide(&folded))
}
// Resolve existing ancestors so aliases share coordination even before a leaf exists.
fn resolved(path: &Path) -> io::Result<PathBuf> {
    let mut ancestor = path;
    let mut tail = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut canonical) => {
                for name in tail.iter().rev() { canonical.push(name); }
                return Ok(identity(canonical));
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound || error.kind() == io::ErrorKind::NotADirectory => {
                // A dangling symlink must not become a lexical escape hatch.
                if fs::symlink_metadata(ancestor).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
                    return Err(error);
                }
                let Some(name) = ancestor.file_name() else { return Err(error); };
                tail.push(name.to_os_string());
                let Some(parent) = ancestor.parent() else { return Err(error); };
                ancestor = parent;
            },
            Err(error) => return Err(error),
        }
    }
}
impl FileStore {
    fn prepare(paths: &[PathBuf]) -> io::Result<(FileAccess, Vec<PathBuf>)> {
        let mut scopes = Vec::with_capacity(paths.len());
        let mut identities = Vec::with_capacity(paths.len() * 2);
        for path in paths {
            let absolute = absolute(path)?;
            let lexical = identity(absolute.clone());
            let resolved = resolved(&absolute)?;
            identities.push(lexical.clone());
            identities.push(resolved.clone());
            scopes.push(Scope { lexical, resolved });
        }
        identities.sort(); identities.dedup();
        Ok((FileAccess { scopes }, identities))
    }
    pub fn with<R>(&self, paths: &[PathBuf], action: impl FnOnce(&FileAccess) -> R) -> io::Result<R> {
        let (access, paths) = Self::prepare(paths)?;
        let coordinator = coordinator();
        let mut active = coordinator.active.lock().unwrap_or_else(|error| error.into_inner());
        let lease = loop {
            if let Some(lease) = reserve(&mut active, &paths) { break lease; }
            active = coordinator.changed.wait(active).unwrap_or_else(|error| error.into_inner());
        };
        drop(active);
        let result = action(&access);
        drop(lease);
        Ok(result)
    }
    pub async fn with_async<R, F>(&self, paths: &[PathBuf], action: F) -> io::Result<R>
    where F: for<'a> FnOnce(&'a FileAccess) -> Pin<Box<dyn Future<Output = R> + Send + 'a>> {
        let (access, paths) = Self::prepare(paths)?;
        let coordinator = coordinator();
        let lease = loop {
            let notified = coordinator.notified.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let lease = {
                let mut active = coordinator.active.lock().unwrap_or_else(|error| error.into_inner());
                reserve(&mut active, &paths)
            };
            if let Some(lease) = lease { break lease; }
            notified.await;
        };
        let result = action(&access).await;
        drop(lease);
        Ok(result)
    }
    pub fn read(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        let path = path.as_ref(); self.with(&[path.to_path_buf()], |access| access.read(path))?
    }
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<String> {
        let path = path.as_ref(); self.with(&[path.to_path_buf()], |access| access.read_to_string(path))?
    }
    pub fn read_json<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, String> {
        let path = path.as_ref();
        self.with(&[path.to_path_buf()], |access| access.read_json(path))
            .map_err(|error| format!("{}：{error}", path.display()))?
    }
}
impl FileAccess {
    fn authorize(&self, path: &Path, ancestor: bool) -> io::Result<()> {
        let absolute = absolute(path)?;
        let lexical = identity(absolute.clone());
        if !self.scopes.iter().any(|scope| lexical.starts_with(&scope.lexical) || lexical.starts_with(&scope.resolved)
            || (ancestor && (scope.lexical.starts_with(&lexical) || scope.resolved.starts_with(&lexical)))) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("outside coordinated file scope: {}", path.display())));
        }
        if ancestor { return Ok(()); }
        let target = resolved(&absolute)?;
        if self.scopes.iter().any(|scope| (lexical.starts_with(&scope.lexical) || lexical.starts_with(&scope.resolved))
            && target.starts_with(&scope.resolved)) { return Ok(()); }
        Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("path resolves outside coordinated file scope: {}", path.display())))
    }
    pub fn read(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        self.authorize(path.as_ref(), false)?; fs::read(path)
    }
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<String> {
        self.authorize(path.as_ref(), false)?; fs::read_to_string(path)
    }
    pub fn metadata(&self, path: impl AsRef<Path>) -> io::Result<Metadata> {
        self.authorize(path.as_ref(), true)?; fs::metadata(path)
    }
    pub fn symlink_metadata(&self, path: impl AsRef<Path>) -> io::Result<Metadata> {
        self.authorize(path.as_ref(), true)?; fs::symlink_metadata(path)
    }
    pub fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        self.authorize(path.as_ref(), true)?; fs::canonicalize(path)
    }
    pub fn try_exists(&self, path: impl AsRef<Path>) -> io::Result<bool> {
        self.authorize(path.as_ref(), true)?; path.as_ref().try_exists()
    }
    pub fn check_readable(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.authorize(path.as_ref(), false)?; File::open(path).map(drop)
    }
    pub fn read_dir(&self, path: impl AsRef<Path>) -> io::Result<Vec<DirectoryEntry>> {
        self.authorize(path.as_ref(), false)?;
        fs::read_dir(path)?.map(|entry| {
            let entry = entry?;
            Ok(DirectoryEntry { path: entry.path(), name: entry.file_name(), file_type: entry.file_type() })
        }).collect()
    }
    pub fn create_dir(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.authorize_creation(path.as_ref())?; fs::create_dir(path)
    }
    pub fn create_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.authorize_creation(path.as_ref())?; fs::create_dir_all(path)
    }
    fn authorize_creation(&self, path: &Path) -> io::Result<()> {
        let lexical = identity(absolute(path)?);
        if self.scopes.iter().any(|scope| scope.lexical.starts_with(&lexical) || scope.resolved.starts_with(&lexical)) {
            self.authorize(path, true)
        } else { self.authorize(path, false) }
    }
    pub fn remove_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.authorize(path.as_ref(), false)?; fs::remove_file(path)
    }
    pub fn remove_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.authorize(path.as_ref(), false)?; fs::remove_dir_all(path)
    }
    pub fn write(&self, path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        self.authorize(path.as_ref(), false)?; fs::write(path, bytes)
    }
    pub fn create(&self, path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        self.authorize(path.as_ref(), false)?;
        let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
        file.write_all(bytes)?; file.sync_all()
    }
    pub fn replace(&self, path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        self.publish_checked(path.as_ref(), bytes, PublishMode::Replace, |_| Ok(()), |error| error)
    }
    /// Validate before preparing bytes on disk and again immediately before publication.
    /// Temporary cleanup only ever removes the temporary created by this operation.
    pub fn publish_checked<E>(&self, target: &Path, bytes: &[u8], mode: PublishMode,
        mut check: impl FnMut(&Self) -> Result<(), E>, map_io: impl Fn(io::Error) -> E) -> Result<(), E> {
        self.authorize(target, false).map_err(&map_io)?;
        check(self)?;
        let mut temporary = Temporary::new(target).map_err(&map_io)?;
        let mut file = OpenOptions::new().create_new(true).write(true).open(&temporary.path).map_err(&map_io)?;
        temporary.owned = true;
        file.write_all(bytes).and_then(|_| file.sync_all()).map_err(&map_io)?;
        drop(file);
        check(self)?;
        self.authorize(target, false).map_err(&map_io)?;
        match mode {
            PublishMode::Create => fs::hard_link(&temporary.path, target),
            PublishMode::Replace => fs::rename(&temporary.path, target),
        }.map_err(&map_io)?;
        Ok(())
    }
    pub fn copy_if_absent(&self, source: impl AsRef<Path>, target: impl AsRef<Path>) -> io::Result<bool> {
        self.authorize(source.as_ref(), false)?;
        self.authorize(target.as_ref(), false)?;
        let mut cleanup = Temporary { path: target.as_ref().to_path_buf(), owned: false };
        let mut target_file = match OpenOptions::new().create_new(true).write(true).open(target.as_ref()) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(error),
        };
        cleanup.owned = true;
        let mut source = File::open(source)?;
        io::copy(&mut source, &mut target_file)?;
        target_file.sync_all()?;
        cleanup.owned = false;
        Ok(true)
    }
    /// Read, modify, encode and publish under the same scope, preserving caller policies.
    pub fn update_json<T, R, E>(&self, path: impl AsRef<Path>,
        load: impl FnOnce(&Self) -> Result<T, E>, update: impl FnOnce(&mut T) -> Result<R, E>,
        encode: impl FnOnce(&T) -> Result<Vec<u8>, E>, map_io: impl FnOnce(io::Error) -> E) -> Result<(T, R), E> {
        let mut value = load(self)?;
        let result = update(&mut value)?;
        let bytes = encode(&value)?;
        self.replace(path, &bytes).map_err(map_io)?;
        Ok((value, result))
    }
    pub fn read_json<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, String> {
        let path = path.as_ref();
        let bytes = self.read(path).map_err(|error| format!("{}：{error}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("{}：记录损坏：{error}", path.display()))
    }
    pub fn read_jsonl<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<Vec<T>, String> {
        let path = path.as_ref();
        let bytes = self.read(path).map_err(|error| format!("{}：{error}", path.display()))?;
        if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
            return Err(format!("{}：JSONL 尾部残缺；拒绝读取或继续追加。", path.display()));
        }
        bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).enumerate()
            .map(|(line, bytes)| serde_json::from_slice(bytes)
                .map_err(|error| format!("{}：记录 {} 损坏：{error}", path.display(), line + 1)))
            .collect()
    }
    /// Missing files and damaged records are never repaired or retried.
    pub fn append_jsonl<T: serde::de::DeserializeOwned>(&self, path: impl AsRef<Path>, record: &impl serde::Serialize) -> Result<(), String> {
        let path = path.as_ref();
        self.read_jsonl::<T>(path)?;
        let mut bytes = serde_json::to_vec(record).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new().append(true).open(path)
            .map_err(|error| format!("{}：{error}", path.display()))?;
        file.write_all(&bytes).and_then(|_| file.sync_all())
            .map_err(|error| format!("{}：{error}", path.display()))
    }
}

struct Temporary { path: PathBuf, owned: bool }
impl Temporary {
    fn new(target: &Path) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "publication target has no filename"))?;
        let mut name = name.to_os_string();
        name.push(format!(".tmp-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        Ok(Self { path: target.with_file_name(name), owned: false })
    }
}
impl Drop for Temporary {
    fn drop(&mut self) { if self.owned { let _ = fs::remove_file(&self.path); } }
}
