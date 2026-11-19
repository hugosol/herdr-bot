//! A launch report is not a session registration. Only the owner of a new,
//! independently observed terminal can authorize that terminal's first input.

use std::{collections::HashMap, future::Future, path::Path, sync::{Arc, Mutex}, time::{Duration, Instant}};

use serde::Deserialize;
use tokio::sync::watch;

use super::{HerdrClient, HerdrError, NativeSession, client::{LaunchTarget, LaunchVerificationError}, identity::native_error};

pub(crate) const LAUNCH_ID_ENV: &str = "HERDR_BOT_LAUNCH_ID";
pub(crate) const TARGET_ID_ENV: &str = "HERDR_BOT_LAUNCH_TARGET_ID";
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(5);

pub(crate) enum InitialInputProgress<'a> {
    WaitingToRetry(&'a HerdrError),
    RetrySubmitting,
}

#[derive(Debug)]
pub(crate) enum InitialInputError {
    BeforeSend(HerdrError),
    RetryCancelled { blocked: HerdrError, reason: HerdrError },
    Send(HerdrError),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    launch_id: String,
    target_id: String,
    agent_type: String,
    cwd: String,
    pid: u32,
    session_file: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum Phase { Identity, Ready, Ended }

#[derive(Debug, Clone, PartialEq, Eq)]
struct Owner {
    session_id: String,
    pid: u32,
    cwd: String,
    session_file: Option<String>,
}

#[derive(Default, PartialEq, Eq)]
enum Retry {
    #[default]
    Inactive,
    Waiting,
    OwnerChanged,
}

#[derive(Default)]
struct State {
    retry: Retry,
    target: Option<Arc<LaunchTarget>>,
    owner: Option<Owner>,
    identity: bool,
    early_ready: Option<Owner>,
    ready: bool,
    identity_failure: Option<HerdrError>,
    ended: Vec<(u32, String)>,
    retained: bool,
    closed: bool,
}

struct Pending {
    target_id: String,
    kind: &'static str,
    cwd: Option<String>,
    restoring: bool,
    state: watch::Sender<State>,
}

pub(crate) struct Launches {
    pending: Mutex<HashMap<String, Arc<Pending>>>,
    clock: Arc<dyn crate::clock::Clock>,
}

pub(crate) struct Launch {
    launches: Arc<Launches>,
    id: String,
    pending: Arc<Pending>,
}

impl Launches {
    pub(crate) fn new(clock: Arc<dyn crate::clock::Clock>) -> Self {
        Self { pending: Mutex::new(HashMap::new()), clock }
    }

    pub(crate) fn begin(self: &Arc<Self>, kind: &'static str, cwd: Option<&str>) -> Launch {
        self.track(kind, cwd, false)
    }

    pub(crate) fn begin_app_restore(self: &Arc<Self>, kind: &'static str, cwd: Option<&str>) -> Launch {
        self.track(kind, cwd, true)
    }

    fn track(self: &Arc<Self>, kind: &'static str, cwd: Option<&str>, restoring: bool) -> Launch {
        let id = crate::chats::new_creation_id();
        let pending = Arc::new(Pending {
            target_id: crate::chats::new_creation_id(), kind, cwd: cwd.map(str::to_owned), restoring,
            state: watch::channel(State::default()).0,
        });
        self.pending.lock().unwrap().insert(id.clone(), Arc::clone(&pending));
        Launch { launches: Arc::clone(self), id, pending }
    }

    pub(crate) async fn app_target(&self, herdr: &HerdrClient, kind: &str, session_id: &str) -> Result<Vec<Arc<LaunchTarget>>, HerdrError> {
        self.app_targets(herdr, kind, session_id, None).await
    }

    pub(crate) async fn app_target_at(&self, herdr: &HerdrClient, kind: &str, session_id: &str, location: &super::NavigationTarget) -> Result<Option<Arc<LaunchTarget>>, HerdrError> {
        let mut matches = self.app_targets(herdr, kind, session_id, Some(location)).await?;
        if matches.len() > 1 { return Err(native_error("multiple live App owners for the saved Run Chat")); }
        Ok(matches.pop())
    }

    async fn app_targets(&self, herdr: &HerdrClient, kind: &str, session_id: &str, location: Option<&super::NavigationTarget>) -> Result<Vec<Arc<LaunchTarget>>, HerdrError> {
        let candidates = self.pending.lock().unwrap().values().filter_map(|pending| {
            let state = pending.state.borrow();
            let owner = state.owner.as_ref()?;
            let target = state.target.as_ref()?;
            (pending.kind == kind && state.retained && !state.closed && owner.session_id == session_id
                && location.is_none_or(|location| target.location == *location)
                && target.app_coordinates().is_some()).then(|| (pending.clone(), owner.clone(), target.clone()))
        }).collect::<Vec<_>>();
        let mut found = Vec::with_capacity(candidates.len());
        for (pending, owner, target) in candidates {
            target.verify(herdr, kind, &owner.cwd, owner.pid, session_id, owner.session_file.as_deref()).await?;
            if !pending.state.borrow().closed { found.push(target); }
        }
        Ok(found)
    }

    pub(crate) async fn accept(
        &self, herdr: &HerdrClient, session_id: String, report: Report, phase: Phase,
    ) -> Result<(), ()> {
        let pending = self.pending.lock().unwrap().get(&report.launch_id).cloned().ok_or(())?;
        let wrong_kind = report.agent_type != pending.kind;
        let restore_identity = pending.restoring && matches!(phase, Phase::Identity);
        if report.target_id != pending.target_id || (wrong_kind && !restore_identity) || report.pid == 0
            || session_id.trim().is_empty() || !Path::new(&report.cwd).is_absolute()
            || pending.cwd.as_deref().is_some_and(|cwd| !crate::zones::same_path(&report.cwd, cwd))
            || report.session_file.as_deref().is_some_and(|path| !Path::new(path).is_absolute())
        {
            return Err(());
        }
        let owner = Owner { session_id, pid: report.pid, cwd: report.cwd, session_file: report.session_file };
        if matches!(phase, Phase::Ended) {
            let mut accepted = false;
            let mut remove = false;
            pending.state.send_if_modified(|state| {
                if state.closed || state.owner.as_ref().is_some_and(|active| !active.same_session(&owner)) {
                    return false;
                }
                accepted = true;
                if owner.ended(state) { return false; }
                state.ended.push((owner.pid, owner.session_id));
                remove = state.retained;
                state.closed = remove;
                true
            });
            if remove { self.pending.lock().unwrap().remove(&report.launch_id); }
            return accepted.then_some(()).ok_or(());
        }
        let mut state = pending.state.subscribe();
        let target = tokio::time::timeout(READY_TIMEOUT, state.wait_for(|state| state.target.is_some() || state.closed))
            .await.map_err(|_| ())?.map_err(|_| ())?.target.clone().ok_or(())?;
        {
            let state = state.borrow();
            if state.rejects(&owner) {
                if !state.observes_replacement(&owner, phase) { return Err(()); }
            } else if matches!(phase, Phase::Identity) && state.identity || matches!(phase, Phase::Ready) && state.ready {
                return Ok(());
            }
        }
        let verified = target.verify(herdr, pending.kind, &owner.cwd, owner.pid, &owner.session_id, owner.session_file.as_deref()).await;
        let (error, terminal) = match verified {
            Ok(_) if wrong_kind => (Some(native_error("reported recovery agent type conflicts with the selected Chat")), true),
            Ok(_) => (None, false),
            Err(LaunchVerificationError::OfficialIdentity(error)) => (Some(error), true),
            Err(LaunchVerificationError::ReportedIdentity(error)) => (Some(error), restore_identity),
            Err(LaunchVerificationError::Unproven(error)) => (Some(error), false),
        };
        if let Some(error) = error {
            eprintln!("launch evidence rejected: {}", error.message());
            if terminal {
                pending.state.send_if_modified(|state| {
                    if state.rejects(&owner) { return false; }
                    state.identity_failure = Some(error);
                    true
                });
            }
            return Err(());
        }
        let mut accepted = false;
        pending.state.send_if_modified(|state| {
            if state.observes_replacement(&owner, phase) {
                state.retry = Retry::OwnerChanged;
                return true; // Notify the waiter without rebinding this launch.
            }
            if state.rejects(&owner) { return false; }
            accepted = true;
            match phase {
                Phase::Identity => {
                    if state.identity { return false; }
                    state.ready = state.early_ready.take().is_some_and(|early| early == owner);
                    state.owner = Some(owner);
                    state.identity = true;
                    true
                }
                Phase::Ready if state.identity => !std::mem::replace(&mut state.ready, true),
                Phase::Ready => {
                    if state.early_ready.as_ref() == Some(&owner) { return false; }
                    state.early_ready = Some(owner);
                    true
                }
                Phase::Ended => unreachable!(),
            }
        });
        accepted.then_some(()).ok_or(())
    }
}

impl State {
    fn rejects(&self, owner: &Owner) -> bool {
        self.closed || self.retry == Retry::OwnerChanged || self.identity_failure.is_some() || owner.ended(self)
            || self.owner.as_ref().is_some_and(|active| active != owner)
    }

    fn observes_replacement(&self, owner: &Owner, phase: Phase) -> bool {
        self.retry == Retry::Waiting && matches!(phase, Phase::Identity)
            && !self.closed && self.identity_failure.is_none() && !owner.ended(self)
            && self.owner.as_ref().is_some_and(|active| !active.same_session(owner) && !active.ended(self))
    }

    fn cannot_retry(&self) -> bool {
        self.closed || self.retry == Retry::OwnerChanged || self.identity_failure.is_some()
            || self.owner.as_ref().is_none_or(|owner| owner.ended(self))
    }

    fn retry_invalidation(&self) -> HerdrError {
        self.identity_failure.clone().unwrap_or_else(|| native_error(if self.retry == Retry::OwnerChanged {
            "launch owner changed before retry"
        } else {
            "launch owner ended before retry"
        }))
    }
}

impl Owner {
    fn same_session(&self, other: &Self) -> bool { self.pid == other.pid && self.session_id == other.session_id }
    fn ended(&self, state: &State) -> bool {
        state.ended.iter().any(|(pid, id)| *pid == self.pid && id == &self.session_id)
    }
}

impl Launch {
    pub(super) fn correlation(&self) -> (&str, &str) { (&self.id, &self.pending.target_id) }

    pub(super) fn bind_target(&self, target: Arc<LaunchTarget>) {
        self.pending.state.send_modify(|state| state.target = Some(target));
    }

    pub(crate) async fn ready(&self, herdr: &HerdrClient) -> Result<NativeSession, HerdrError> {
        self.wait_for(herdr, true).await
    }

    pub(crate) async fn identity(&self, herdr: &HerdrClient) -> Result<NativeSession, HerdrError> {
        self.wait_for(herdr, false).await
    }

    pub(crate) fn retain_app_target(&self) {
        self.pending.state.send_if_modified(|state| {
            let retain = state.retry != Retry::OwnerChanged
                && state.owner.as_ref().is_some_and(|owner| !owner.ended(state));
            retain && !std::mem::replace(&mut state.retained, true)
        });
    }

    async fn wait_for(&self, herdr: &HerdrClient, require_ready: bool) -> Result<NativeSession, HerdrError> {
        let mut receiver = self.pending.state.subscribe();
        let (owner, target) = {
            let state = tokio::time::timeout(READY_TIMEOUT, receiver.wait_for(|state| {
                state.identity_failure.is_some() || state.owner.as_ref().is_some_and(|owner| owner.ended(state) || state.identity && (!require_ready || state.ready))
            })).await.map_err(|_| HerdrError::Timeout("framework ready"))?
                .map_err(|_| native_error("launch evidence no longer available"))?;
            if let Some(error) = &state.identity_failure { return Err(error.clone()); }
            let owner = state.owner.as_ref().unwrap();
            if owner.ended(&state) { return Err(native_error("launch owner ended before input")); }
            (owner.clone(), state.target.clone().unwrap())
        };
        self.verify(herdr, &target, &owner).await
    }

    /// Submit to the verified launch; only a pre-write block permits another attempt.
    pub(crate) async fn submit_initial<F: Future<Output = ()> + Send>(
        &self, herdr: &HerdrClient, target: &LaunchTarget, text: &str,
        mut progress: impl FnMut(InitialInputProgress<'_>) -> F,
    ) -> Result<(), InitialInputError> {
        self.current(herdr, target).await.map_err(InitialInputError::BeforeSend)?;
        let blocked = match target.submit(herdr, text).await {
            Err(blocked @ HerdrError::InputBlocked { .. }) => blocked,
            result => return result.map_err(InitialInputError::Send),
        };
        let deadline = self.launches.clock.now() + INITIAL_RETRY_DELAY;
        self.pending.state.send_modify(|state| state.retry = Retry::Waiting);
        progress(InitialInputProgress::WaitingToRetry(&blocked)).await;
        if let Err(reason) = self.wait_for_retry(deadline, herdr, target).await {
            return Err(InitialInputError::RetryCancelled { blocked, reason });
        }
        progress(InitialInputProgress::RetrySubmitting).await;
        {
            let state = self.pending.state.borrow();
            if state.cannot_retry() {
                return Err(InitialInputError::RetryCancelled { blocked, reason: state.retry_invalidation() });
            }
        }
        let result = target.submit(herdr, text).await;
        self.pending.state.send_if_modified(|state| {
            if state.retry != Retry::Waiting { return false; }
            state.retry = Retry::Inactive;
            true
        });
        result.map_err(InitialInputError::Send)
    }

    async fn wait_for_retry(&self, deadline: Instant, herdr: &HerdrClient, target: &LaunchTarget) -> Result<(), HerdrError> {
        let mut state = self.pending.state.subscribe();
        tokio::select! {
            biased;
            invalidated = state.wait_for(State::cannot_retry) => Err(match invalidated {
                Ok(state) => state.retry_invalidation(),
                Err(_) => native_error("launch evidence no longer available"),
            }),
            () = crate::clock::sleep_until(self.launches.clock.as_ref(), deadline) => Ok(()),
        }?;
        self.current(herdr, target).await
    }

    pub(crate) async fn current(&self, herdr: &HerdrClient, target: &LaunchTarget) -> Result<(), HerdrError> {
        let owner = self.pending.state.borrow().owner.clone().ok_or_else(|| native_error("no launch owner"))?;
        self.verify(herdr, target, &owner).await.map(drop)
    }

    async fn verify(&self, herdr: &HerdrClient, target: &LaunchTarget, owner: &Owner) -> Result<NativeSession, HerdrError> {
        let identity = target.verify(herdr, self.pending.kind, &owner.cwd, owner.pid, &owner.session_id, owner.session_file.as_deref()).await?;
        let state = self.pending.state.borrow();
        if let Some(error) = &state.identity_failure { return Err(error.clone()); }
        if state.retry == Retry::OwnerChanged { return Err(state.retry_invalidation()); }
        if owner.ended(&state) { return Err(native_error("launch owner ended before input")); }
        Ok(identity)
    }
}

impl Drop for Launch {
    fn drop(&mut self) {
        let closed = self.pending.state.send_if_modified(|state| {
            if state.retained || state.closed { return false; }
            state.closed = true;
            true
        });
        if closed { self.launches.pending.lock().unwrap().remove(&self.id); }
    }
}

// App's Windows frontend starts a launcher through the core. The core pins
// the root PID; check real process ancestry rather than treating that PID or
// the terminal address as the native agent's PID.
#[cfg(windows)]
pub(super) fn process_is_descendant(root: u32, mut child: u32) -> bool {
    use std::{mem::size_of, os::windows::io::{FromRawHandle, OwnedHandle}};
    use windows_sys::Win32::{Foundation::{FILETIME, INVALID_HANDLE_VALUE}, System::{Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS}, Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION}}};

    fn started(pid: u32) -> Option<u64> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() { return None; }
        let _handle = unsafe { OwnedHandle::from_raw_handle(process.cast()) };
        let [mut created, mut exited, mut kernel, mut user] = [FILETIME::default(); 4];
        if unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } == 0 { return None; }
        Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE { return false; }
    let _handle = unsafe { OwnedHandle::from_raw_handle(snapshot.cast()) };
    let mut record = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut found = unsafe { Process32FirstW(snapshot, &mut record) };
    let mut processes = Vec::new();
    while found != 0 {
        processes.push((record.th32ProcessID, record.th32ParentProcessID));
        found = unsafe { Process32NextW(snapshot, &mut record) };
    }
    let Some(mut child_started) = started(child) else { return false; };
    for _ in 0..processes.len() {
        let Some((_, parent)) = processes.iter().find(|(pid, _)| *pid == child) else { break; };
        let Some(parent_started) = started(*parent) else { break; };
        if parent_started > child_started || *parent == child { break; }
        if *parent == root { return true; }
        child = *parent;
        child_started = parent_started;
    }
    false
}

#[cfg(target_os = "linux")]
pub(super) fn process_is_descendant(root: u32, mut child: u32) -> bool {
    while child > 1 {
        let Some(parent) = std::fs::read_to_string(format!("/proc/{child}/stat")).ok()
            .and_then(|stat| stat.rsplit_once(") ")?.1.split_whitespace().nth(1)?.parse::<u32>().ok()) else { return false; };
        if parent == root { return true; }
        if parent == child { return false; }
        child = parent;
    }
    false
}

#[cfg(not(any(windows, target_os = "linux")))]
pub(super) fn process_is_descendant(_root: u32, _child: u32) -> bool { false }

/// The report supplies the native identity; an existing file may corroborate
/// it, but an allocated path is not considered a persisted session file.
pub(super) async fn reported_native_session(kind: &str, cwd: &str, session_id: &str, file: Option<&str>) -> Result<NativeSession, HerdrError> {
    let report = NativeSession { session_id: session_id.into(), cwd: cwd.into(), session_file: file.map(str::to_owned), startup_identity_path: None };
    let Some(file) = file else { return Ok(report); };
    let file = std::path::PathBuf::from(file);
    let allow_title = kind == "omp";
    let deadline = tokio::time::Instant::now() + super::client::QUERY_TIMEOUT;
    let mut identity = tokio::task::spawn_blocking(move || super::identity::native_header_or_pending(file, allow_title, deadline, Some(report)))
        .await.map_err(|error| native_error(&format!("cannot inspect native file: {error}")))??;
    if identity.session_id != session_id { return Err(native_error("reported session file belongs to another native session")); }
    if identity.cwd != cwd { identity.cwd = cwd.into(); }
    Ok(identity)
}
