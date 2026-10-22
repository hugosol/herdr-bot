//! zone 文件事件监听、按 zone 合并节流与监听健康上报。
//!
//! 本模块只观察**本地**状态：每个已登记 zone（以及 Git 工作树外部元数据目录）各有一条真实
//! 文件系统监听，事件按 zone 合并成一个节流窗口，窗口结束补一次刷新通知。它不查询远端、
//! 不替代手动读取，也不把「事件已合并」说成「Git 查询一定在窗口内完成」。
//!
//! 页面读取仍然走既有公开读取接口；本模块只通过一条只读 SSE 长连接下发「可以重新读取」的通知与
//! 监听健康。一个 zone 只有一条监听和一份节流状态，多个观察者共享同一份通知，不会各自放大刷新。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::response::sse::Event;
use notify::{
    Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    recommended_watcher,
};
use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{Instant, interval_at};

use crate::zones::RegisteredZone;

/// 一个 zone 的自动刷新节流状态。
///
/// 语义只有三条：没有等待窗口时立即刷新；窗口内的事件只记一次待刷新；窗口结束补一次。
/// 窗口从**上一次刷新**（立即刷新或补刷新）起算，因此连续写入最多每 `interval` 刷新一次，
/// 不会因为一直有事件而把最终刷新无限推后。
#[derive(Debug)]
struct RefreshGate {
    interval: Duration,
    last_refresh: Option<Instant>,
    pending: bool,
}

impl RefreshGate {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_refresh: None,
            pending: false,
        }
    }

    /// 收到一个文件事件：返回 `true` 表示应立即刷新，`false` 表示已记入当前窗口的待刷新。
    fn on_event(&mut self, now: Instant) -> bool {
        match self.last_refresh {
            Some(last) if now < last + self.interval => {
                self.pending = true;
                false
            }
            _ => {
                self.last_refresh = Some(now);
                self.pending = false;
                true
            }
        }
    }

    /// 当前窗口的结束时刻；没有待刷新时没有需要等待的窗口。
    fn deadline(&self) -> Option<Instant> {
        if !self.pending {
            return None;
        }
        self.last_refresh.map(|last| last + self.interval)
    }

    /// 窗口到点：有待刷新时补一次刷新并返回 `true`；否则什么也不做。
    fn on_deadline(&mut self, now: Instant) -> bool {
        if !self.pending {
            return false;
        }
        self.pending = false;
        self.last_refresh = Some(now);
        true
    }
}

/// 每个订阅者的有界帧缓冲：落后太多时丢弃增量，随后由一帧新快照收敛。
const FRAME_BUFFER: usize = 64;

/// 监听健康：只有「正在监听」与「已停止并给出原因」两种。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Health {
    Live,
    Error(String),
}

impl Health {
    fn label(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Error(_) => "error",
        }
    }

    fn message(&self) -> Option<String> {
        match self {
            Self::Live => None,
            Self::Error(message) => Some(message.clone()),
        }
    }
}

/// 一个 zone 在首帧里的监听视图：健康、最近一次自动刷新时刻与刷新代数。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneWatchView {
    pub zone: String,
    pub health: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh_at: Option<u64>,
    pub generation: u64,
}

/// `snapshot` 帧的内容：当前生效的刷新间隔与全部被监听 zone。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneWatchSnapshot {
    pub interval_seconds: u32,
    pub zones: Vec<ZoneWatchView>,
}

/// 一次自动刷新通知。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshData {
    zone: String,
    at: u64,
    generation: u64,
}

/// 一次监听健康变化。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthData {
    zone: String,
    health: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    at: u64,
    generation: u64,
}

/// 广播给所有观察者的增量帧。
#[derive(Debug, Clone)]
pub enum ZoneWatchFrame {
    /// 该 zone 到达一个节流窗口的刷新点。
    Refresh {
        zone: String,
        at: u64,
        generation: u64,
    },
    /// 该 zone 的监听健康发生变化（建立成功或失败）。
    Health {
        zone: String,
        health: &'static str,
        message: Option<String>,
        at: u64,
        generation: u64,
    },
}

/// 把广播帧组装成 SSE 命名事件。
pub fn frame_event(frame: ZoneWatchFrame) -> Event {
    match frame {
        ZoneWatchFrame::Refresh {
            zone,
            at,
            generation,
        } => named_event(
            "refresh",
            &RefreshData {
                zone,
                at,
                generation,
            },
        ),
        ZoneWatchFrame::Health {
            zone,
            health,
            message,
            at,
            generation,
        } => named_event(
            "health",
            &HealthData {
                zone,
                health,
                message,
                at,
                generation,
            },
        ),
    }
}

/// 把首帧（或落后太多后的新快照）组装成带重连提示的 `snapshot` 事件。
pub fn snapshot_event(snapshot: ZoneWatchSnapshot) -> Event {
    named_event("snapshot", &snapshot).retry(Duration::from_secs(2))
}

fn named_event(name: &'static str, data: &impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(data)
        .expect("监听帧只含字符串、数字与布尔，序列化不会失败")
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

/// 一个 zone 的共享状态：监听任务写，所有读连接读。
struct Shared {
    zone: String,
    inner: Mutex<SharedInner>,
    hub: broadcast::Sender<ZoneWatchFrame>,
}

struct SharedInner {
    health: Health,
    last_refresh_at: Option<u64>,
    generation: u64,
}

impl Shared {
    fn new(zone: String, hub: broadcast::Sender<ZoneWatchFrame>) -> Self {
        Self {
            zone,
            inner: Mutex::new(SharedInner {
                health: Health::Live,
                last_refresh_at: None,
                generation: 0,
            }),
            hub,
        }
    }

    fn error(zone: String, message: String, hub: broadcast::Sender<ZoneWatchFrame>) -> Self {
        Self {
            zone,
            inner: Mutex::new(SharedInner {
                health: Health::Error(message),
                last_refresh_at: None,
                generation: 0,
            }),
            hub,
        }
    }

    fn view(&self) -> ZoneWatchView {
        let inner = self.inner.lock().expect("监听状态锁不会被毒化");
        ZoneWatchView {
            zone: self.zone.clone(),
            health: inner.health.label(),
            message: inner.health.message(),
            last_refresh_at: inner.last_refresh_at,
            generation: inner.generation,
        }
    }

    /// 到达刷新点：代数加一，记录刷新时刻，并通知所有观察者。
    fn refresh(&self) {
        let (at, generation) = {
            let mut inner = self.inner.lock().expect("监听状态锁不会被毒化");
            inner.generation += 1;
            let at = now_millis();
            inner.last_refresh_at = Some(at);
            (at, inner.generation)
        };
        let _ = self.hub.send(ZoneWatchFrame::Refresh {
            zone: self.zone.clone(),
            at,
            generation,
        });
    }

    /// 监听停止：只第一次错误生效，绝不把后续写入当成「已经恢复」。
    fn fail(&self, message: String) {
        {
            let mut inner = self.inner.lock().expect("监听状态锁不会被毒化");
            if matches!(inner.health, Health::Error(_)) {
                return;
            }
            inner.health = Health::Error(message);
        }
        self.publish_health();
    }

    /// 把当前健康状态下发一次：监听建立成功，或观察者在连接上时新登记了 zone。
    fn publish_health(&self) {
        let (health, message, at, generation) = {
            let inner = self.inner.lock().expect("监听状态锁不会被毒化");
            (
                inner.health.label(),
                inner.health.message(),
                now_millis(),
                inner.generation,
            )
        };
        let _ = self.hub.send(ZoneWatchFrame::Health {
            zone: self.zone.clone(),
            health,
            message,
            at,
            generation,
        });
    }
}

struct WatchEntry {
    shared: Arc<Shared>,
    task: Option<JoinHandle<()>>,
}

/// zone 监听的拥有者：每个 zone 只有一条真实监听与一份节流状态。
pub struct ZoneWatchManager {
    interval: Duration,
    hub: broadcast::Sender<ZoneWatchFrame>,
    watches: AsyncMutex<BTreeMap<String, WatchEntry>>,
    sync_state: Option<Arc<crate::sync::SyncState>>,
}

impl ZoneWatchManager {
    pub fn new(interval: Duration) -> Self {
        let (hub, _) = broadcast::channel(FRAME_BUFFER);
        Self {
            interval,
            hub,
            watches: AsyncMutex::new(BTreeMap::new()),
            sync_state: None,
        }
    }

    pub fn with_sync_state(interval: Duration, sync_state: Arc<crate::sync::SyncState>) -> Self {
        let mut manager = Self::new(interval);
        manager.sync_state = Some(sync_state);
        manager
    }

    /// Application-initiated refreshes do not depend on filesystem events or the local event throttle.
    pub async fn notify(&self, zone: &str) {
        // Writers have already invalidated or refreshed their cache before broadcasting.
        let watches = self.watches.lock().await;
        let key = crate::status_cache::key(zone);
        for entry in watches.values() {
            if crate::status_cache::key(&entry.shared.zone) == key {
                entry.shared.refresh();
            }
        }
    }

    pub fn interval_seconds(&self) -> u32 {
        self.interval.as_secs().try_into().unwrap_or(u32::MAX)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ZoneWatchFrame> {
        self.hub.subscribe()
    }

    pub async fn snapshot(&self) -> ZoneWatchSnapshot {
        let watches = self.watches.lock().await;
        ZoneWatchSnapshot {
            interval_seconds: self.interval_seconds(),
            zones: watches.values().map(|entry| entry.shared.view()).collect(),
        }
    }

    /// 对齐当前登记列表：新登记的开始监听，取消登记的停止监听；监听失败保留明确的错误状态。
    ///
    /// 每个 zone 只建一条监听，多个观察者共享它；监听失败后不会自动重启，新的订阅或登记
    /// 才可能重新尝试建立监听。
    pub async fn reconcile(&self, zones: &[RegisteredZone]) {
        let mut watches = self.watches.lock().await;
        let registered: BTreeSet<&str> = zones.iter().map(|zone| zone.path.as_str()).collect();
        let stale: Vec<String> = watches
            .keys()
            .filter(|path| !registered.contains(path.as_str()))
            .cloned()
            .collect();
        for path in stale {
            if let Some(entry) = watches.remove(&path) {
                if let Some(task) = entry.task {
                    task.abort();
                }
            }
        }

        for zone in zones {
            let already_watching = watches
                .get(&zone.path)
                .is_some_and(|entry| entry.task.is_some());
            if already_watching {
                continue;
            }
            match setup_watch(
                zone.path.clone(),
                self.interval,
                self.hub.clone(),
                self.sync_state.clone(),
            )
            .await
            {
                Ok((shared, task)) => {
                    watches.insert(
                        zone.path.clone(),
                        WatchEntry {
                            shared: shared.clone(),
                            task: Some(task),
                        },
                    );
                    shared.publish_health();
                }
                Err(message) => {
                    let shared =
                        Arc::new(Shared::error(zone.path.clone(), message, self.hub.clone()));
                    watches.insert(
                        zone.path.clone(),
                        WatchEntry {
                            shared: shared.clone(),
                            task: None,
                        },
                    );
                    shared.publish_health();
                }
            }
        }
    }
}

struct PreparedWatcher {
    watcher: RecommendedWatcher,
    identities: Vec<WatchedIdentity>,
}

/// 建立真实监听：zone 根递归监听，Git 工作树的外部元数据目录另外递归监听。
async fn setup_watch(
    zone: String,
    interval: Duration,
    hub: broadcast::Sender<ZoneWatchFrame>,
    sync_state: Option<Arc<crate::sync::SyncState>>,
) -> Result<(Arc<Shared>, JoinHandle<()>), String> {
    let root = PathBuf::from(&zone);
    let watch_root = root.clone();
    let (sender, receiver) = mpsc::unbounded_channel();
    let prepared = tokio::task::spawn_blocking(move || prepare_watcher(&watch_root, sender))
        .await
        .map_err(|_| "建立文件系统监听的任务异常结束。".to_string())??;
    let shared = Arc::new(Shared::new(zone, hub));
    let task = tokio::spawn(run_watch(
        Arc::clone(&shared),
        receiver,
        prepared.watcher,
        prepared.identities,
        interval,
        sync_state,
    ));
    Ok((shared, task))
}

fn prepare_watcher(
    root: &Path,
    sender: mpsc::UnboundedSender<WatchSignal>,
) -> Result<PreparedWatcher, String> {
    let mut watcher = recommended_watcher(move |result: Result<NotifyEvent, notify::Error>| {
        match result {
            Ok(event) => {
                // 读取只产生 Access 事件：它不是内容变化，不触发刷新。
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                let _ = sender.send(WatchSignal::Changed);
            }
            Err(error) => {
                let _ = sender.send(WatchSignal::Error(format!("文件系统监听失败：{error}")));
            }
        }
    })
    .map_err(|error| format!("无法建立文件系统监听：{error}"))?;

    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| format!("无法监听 zone 目录：{error}"))?;
    let mut identities = vec![watched_identity(root, "zone 目录")?];

    if let Some(git_dir) = external_git_dir(root) {
        watcher
            .watch(&git_dir, RecursiveMode::Recursive)
            .map_err(|error| format!("无法监听 Git 元数据目录 {}：{error}", git_dir.display()))?;
        identities.push(watched_identity(&git_dir, "Git 元数据目录")?);
    }

    Ok(PreparedWatcher {
        watcher,
        identities,
    })
}

/// Git 工作树根的 `.git` 是文件时，真正的元数据目录可能在 zone 之外；返回它以便一起监听。
fn external_git_dir(root: &Path) -> Option<PathBuf> {
    let entry = root.join(".git");
    let metadata = fs::metadata(&entry).ok()?;
    if !metadata.is_file() {
        // `.git` 是目录（或不存在）：zone 根的递归监听已经覆盖它。
        return None;
    }
    let content = fs::read_to_string(&entry).ok()?;
    let value = content
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    if value.is_empty() {
        return None;
    }
    let candidate = Path::new(value);
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    fs::canonicalize(candidate).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchedIdentity {
    path: PathBuf,
    label: &'static str,
    canonical: String,
    #[cfg(windows)]
    created: u64,
}

/// 读取被监听目录当前身份：路径与目录创建时间共同识别「同名目录被替换」。
fn watched_identity(path: &Path, label: &'static str) -> Result<WatchedIdentity, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label}已不存在或无法访问，自动更新已停止：{error}"))?;
    if !metadata.is_dir() {
        return Err(format!("{label}已不再是目录，自动更新已停止。"));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("{label}无法解析，自动更新已停止：{error}"))?;
    let canonical = crate::zones::display_path(&canonical);
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(WatchedIdentity {
            path: path.to_path_buf(),
            label,
            canonical,
            created: metadata.creation_time(),
        })
    }
    #[cfg(not(windows))]
    {
        Ok(WatchedIdentity {
            path: path.to_path_buf(),
            label,
            canonical,
        })
    }
}

#[derive(Debug)]
enum WatchSignal {
    Changed,
    Error(String),
}

async fn run_watch(
    shared: Arc<Shared>,
    mut signals: mpsc::UnboundedReceiver<WatchSignal>,
    _watcher: RecommendedWatcher,
    identities: Vec<WatchedIdentity>,
    interval: Duration,
    sync_state: Option<Arc<crate::sync::SyncState>>,
) {
    let mut gate = RefreshGate::new(interval);
    let health_interval = interval.min(Duration::from_secs(5));
    let mut health = interval_at(Instant::now() + health_interval, health_interval);

    loop {
        let deadline = gate.deadline();
        let wake = deadline.unwrap_or_else(|| Instant::now() + health_interval);
        tokio::select! {
            signal = signals.recv() => match signal {
                Some(WatchSignal::Changed) => {
                    if let Some(state) = &sync_state { state.cache.invalidate(&shared.zone); }
                    if gate.on_event(Instant::now()) {
                        shared.refresh();
                    }
                }
                Some(WatchSignal::Error(message)) => {
                    shared.fail(message);
                    return;
                }
                None => return,
            },
            _ = tokio::time::sleep_until(wake), if deadline.is_some() => {
                if gate.on_deadline(Instant::now()) {
                    shared.refresh();
                }
            }
            _ = health.tick() => {
                for expected in &identities {
                    match watched_identity(&expected.path, expected.label) {
                        Ok(current) if current == *expected => {}
                        Ok(_) => {
                            shared.fail(format!(
                                "{}已被替换，自动更新已停止；请手动刷新确认当前内容。",
                                expected.label
                            ));
                            return;
                        }
                        Err(message) => {
                            shared.fail(message);
                            return;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn first_event_without_waiting_window_refreshes_immediately() {
        let mut gate = RefreshGate::new(Duration::from_secs(10));
        let origin = Instant::now();

        assert!(gate.on_event(origin), "没有等待窗口时首个事件应立即刷新");
        assert_eq!(gate.deadline(), None, "立即刷新后不应留下待刷新窗口");
    }

    #[test]
    fn events_inside_window_are_merged_into_one_trailing_refresh_at_window_end() {
        let origin = Instant::now();
        let mut gate = RefreshGate::new(Duration::from_secs(10));

        assert!(gate.on_event(origin));
        assert!(!gate.on_event(origin + Duration::from_secs(3)));
        assert!(!gate.on_event(origin + Duration::from_secs(9)));
        assert_eq!(
            gate.deadline(),
            Some(origin + Duration::from_secs(10)),
            "窗口内的事件只记一次待刷新，窗口在首个刷新后整 10 秒结束"
        );

        assert!(
            gate.on_deadline(origin + Duration::from_secs(10)),
            "窗口结束必须补一次刷新"
        );
        assert_eq!(gate.deadline(), None);
        assert!(
            !gate.on_deadline(origin + Duration::from_secs(10)),
            "同一窗口只能补一次"
        );
    }

    #[test]
    fn an_event_exactly_at_the_window_end_refreshes_immediately() {
        let origin = Instant::now();
        let mut gate = RefreshGate::new(Duration::from_secs(10));

        assert!(gate.on_event(origin));
        assert!(
            gate.on_event(origin + Duration::from_secs(10)),
            "窗口边界上的事件没有等待窗口，应立即刷新"
        );
        assert_eq!(gate.deadline(), None);
    }

    #[test]
    fn continuous_events_flush_once_per_window_instead_of_postponing_forever() {
        let origin = Instant::now();
        let mut gate = RefreshGate::new(Duration::from_secs(10));
        let mut emissions = Vec::new();

        // 连续 25 秒、每 100ms 一次写入：最终刷新不能因持续写入而无限延期。
        for step in 0..=250 {
            let now = origin + Duration::from_millis(step * 100);
            if gate.on_event(now) {
                emissions.push(now);
            }
        }

        assert_eq!(
            emissions,
            vec![
                origin,
                origin + Duration::from_secs(10),
                origin + Duration::from_secs(20)
            ],
            "连续写入必须按窗口稳定刷新，而不是只留下一个永远不到期的待刷新"
        );
    }

    #[tokio::test]
    async fn application_notifications_reach_all_clients_without_filesystem_events_or_throttling() {
        let manager = ZoneWatchManager::new(Duration::from_secs(60));
        let zone = ".".to_string();
        let shared = Arc::new(Shared::new(zone.clone(), manager.hub.clone()));
        manager
            .watches
            .lock()
            .await
            .insert(zone.clone(), WatchEntry { shared, task: None });
        let mut first = manager.subscribe();
        let mut second = manager.subscribe();
        manager.notify(&zone).await;
        manager.notify(&zone).await;
        for receiver in [&mut first, &mut second] {
            for expected in [1, 2] {
                assert!(
                    matches!(receiver.try_recv().unwrap(), ZoneWatchFrame::Refresh { generation, .. } if generation == expected)
                );
            }
        }
    }

    #[test]
    fn events_after_a_trailing_flush_open_a_new_window() {
        let origin = Instant::now();
        let mut gate = RefreshGate::new(Duration::from_secs(10));

        assert!(gate.on_event(origin));
        assert!(!gate.on_event(origin + Duration::from_secs(5)));
        assert!(gate.on_deadline(origin + Duration::from_secs(10)));
        assert!(
            !gate.on_event(origin + Duration::from_secs(12)),
            "补刷新之后的新事件仍在新的等待窗口内"
        );
        assert_eq!(
            gate.deadline(),
            Some(origin + Duration::from_secs(20)),
            "新窗口从补刷新那一刻起算"
        );
    }
}
