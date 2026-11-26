//! Launch a new Chat or restore a selected native conversation. Business
//! persistence and public progress belong to the callers, not to the launch.

use std::{future::{Future, ready}, sync::Arc, time::Duration};

use tokio::time::{Instant, sleep_until, timeout_at};

use crate::{
    herdr::{HerdrClient, HerdrError, LaunchTarget, NativeSession, RestoreEvidence,
        launch::{InitialInputError, InitialInputProgress, Launch, Launches}},
    tasks::catalog::ResolvedPreset,
};

pub(crate) struct Destination<'a> {
    pub(crate) workspace: &'a str,
    pub(crate) cwd: Option<&'a str>,
    pub(crate) title: &'a str,
    pub(crate) kind: &'static str,
}

pub(crate) enum Progress<'a> {
    Tab(&'a LaunchTarget),
    Started,
}

pub(crate) enum StartError {
    Tab(HerdrError),
    Agent { target: Arc<LaunchTarget>, error: HerdrError },
}

async fn start<F: Future<Output = ()> + Send>(
    herdr: &HerdrClient, destination: &Destination<'_>, args: &[String], report: Option<&Launch>,
    mut progress: impl FnMut(Progress<'_>) -> F,
) -> Result<Arc<LaunchTarget>, StartError> {
    let target = herdr.launch_tab(destination.workspace, destination.cwd, destination.title,
        destination.kind, args, report).await.map_err(StartError::Tab)?;
    progress(Progress::Tab(&target)).await;
    if let Err(error) = target.start_agent(herdr, destination.kind, args).await {
        return Err(StartError::Agent { target, error });
    }
    progress(Progress::Started).await;
    Ok(target)
}

pub(crate) enum TaskStartError {
    Start(StartError),
    Ready(HerdrError),
}

pub(crate) struct StartedTask<'a> {
    herdr: &'a HerdrClient,
    launch: Launch,
    pub(crate) target: Arc<LaunchTarget>,
    pub(crate) identity: NativeSession,
}

pub(crate) async fn start_task<'a, F: Future<Output = ()> + Send>(
    herdr: &'a HerdrClient, launches: &Arc<Launches>, destination: Destination<'_>, preset: &ResolvedPreset,
    progress: impl FnMut(Progress<'_>) -> F,
) -> Result<StartedTask<'a>, TaskStartError> {
    let args = super::agent_launch_args(&preset.env, Some(&preset.trigger));
    let launch = launches.begin(destination.kind, destination.cwd);
    let target = start(herdr, &destination, &args, Some(&launch), progress).await.map_err(TaskStartError::Start)?;
    let identity = launch.ready(herdr).await.map_err(TaskStartError::Ready)?;
    Ok(StartedTask { herdr, launch, target, identity })
}

impl StartedTask<'_> {
    pub(crate) async fn submit_retrying<F: Future<Output = ()> + Send>(
        &self, text: &str, progress: impl FnMut(InitialInputProgress<'_>) -> F,
    ) -> Result<(), InitialInputError> {
        self.launch.submit_initial(self.herdr, &self.target, text, progress).await
    }

    pub(crate) async fn submit_once(&self, text: &str, attempting: impl FnOnce()) -> Result<(), HerdrError> {
        self.launch.current(self.herdr, &self.target).await?;
        attempting();
        self.target.submit(self.herdr, text).await
    }
}

pub(crate) enum RestoreError {
    Start(StartError),
    Unconfirmed { target: Arc<LaunchTarget>, detail: String },
    Confirm { target: Arc<LaunchTarget>, error: ConfirmationError },
    Focus { target: Arc<LaunchTarget>, error: HerdrError },
}

pub(crate) enum ConfirmationError {
    Herdr(HerdrError),
    NativeMismatch,
    CwdMismatch,
}

impl From<HerdrError> for ConfirmationError {
    fn from(error: HerdrError) -> Self { Self::Herdr(error) }
}

pub(crate) struct Restored {
    pub(crate) target: Arc<LaunchTarget>,
    pub(crate) actual_cwd: String,
    pub(crate) confirmed: bool,
}

pub(crate) async fn restore(
    herdr: &HerdrClient, launches: &Arc<Launches>, destination: Destination<'_>,
    id: &str, load: String, environment: Option<Vec<String>>,
) -> Result<Restored, RestoreError> {
    let configured = environment.is_some();
    let mut args = environment.unwrap_or_default();
    args.extend([if destination.kind == "omp" { "--resume" } else { "--session" }.into(), load]);
    let expected = ExpectedNative {
        id,
        file: args.last().filter(|load| std::path::Path::new(load).is_absolute()).map(String::as_str),
        cwd: destination.cwd,
    };
    // TUI uses its official identity; App can only bind a reported owner when
    // starting a configured environment. Native default discovery stays untouched.
    let launch = (herdr.is_app() && configured).then(|| launches.begin_app_restore(destination.kind, destination.cwd));
    let target = start(herdr, &destination, &args, launch.as_ref(), |_| ready(())).await.map_err(RestoreError::Start)?;
    let confirm = |error| RestoreError::Confirm { target: target.clone(), error };
    let unconfirmed = |detail| RestoreError::Unconfirmed { target: target.clone(), detail };
    let focus_error = |error| RestoreError::Focus { target: target.clone(), error };
    let (actual_cwd, confirmed) = if let Some(alias) = target.agent_name() {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last_cwd = None;
        let restored = loop {
            match timeout_at(deadline, herdr.restored_session(alias, destination.kind, &target.location, deadline)).await {
                Ok(Ok(RestoreEvidence::Identified(native))) => {
                    expected.verify(&native).map_err(confirm)?;
                    break (native.cwd, true);
                }
                Ok(Ok(RestoreEvidence::Unidentified(actual_cwd))) => last_cwd = actual_cwd,
                Ok(Err(error)) => return Err(confirm(error.into())),
                Err(_) => {},
            }
            let next = Instant::now() + Duration::from_millis(100);
            if next >= deadline {
                if !configured && let Some(actual_cwd) = last_cwd {
                    if !recovery_cwd_matches(&actual_cwd, destination.cwd) {
                        return Err(confirm(ConfirmationError::CwdMismatch));
                    }
                    break (actual_cwd, false);
                }
                return Err(unconfirmed("未取得可靠的原生会话身份。".into()));
            }
            sleep_until(next).await;
        };
        herdr.focus_target(&target.location).await.map_err(focus_error)?;
        restored
    } else {
        let native = if let Some(launch) = &launch {
            match launch.identity(herdr, |native| expected.verify(native)).await {
                Ok(native) => Some(native),
                Err(ConfirmationError::Herdr(error @ HerdrError::Timeout(_))) => return Err(unconfirmed(error.message())),
                Err(error) => return Err(confirm(error)),
            }
        } else { None };
        let core = herdr.focus_app_target(&target).await.map_err(focus_error)?;
        match native {
            Some(native) => (native.cwd, true),
            None => {
                let actual_cwd = core.pointer("/foreground/cwd").and_then(serde_json::Value::as_str)
                    .or_else(|| core.get("cwd").and_then(serde_json::Value::as_str))
                    .filter(|actual| recovery_cwd_matches(actual, destination.cwd))
                    .ok_or_else(|| confirm(ConfirmationError::CwdMismatch))?;
                (actual_cwd.to_owned(), false)
            }
        }
    };
    Ok(Restored { target, actual_cwd, confirmed })
}

struct ExpectedNative<'a> {
    id: &'a str,
    file: Option<&'a str>,
    cwd: Option<&'a str>,
}

impl ExpectedNative<'_> {
    fn verify(&self, native: &NativeSession) -> Result<(), ConfirmationError> {
        if native.session_id != self.id || !recovery_cwd_matches(&native.cwd, self.cwd)
            || matches!((self.file, native.session_file.as_deref()), (Some(expected), Some(actual)) if !recovery_path_matches(std::path::Path::new(actual), expected))
        {
            return Err(ConfirmationError::NativeMismatch);
        }
        Ok(())
    }
}

fn recovery_path_matches(actual: &std::path::Path, expected: &str) -> bool {
    use crate::zones::{display_path, same_path};
    same_path(&display_path(actual), &display_path(std::path::Path::new(expected)))
}

fn recovery_cwd_matches(actual: &str, expected: Option<&str>) -> bool {
    let actual = std::path::Path::new(actual);
    actual.is_absolute() && actual.is_dir() && expected.is_none_or(|cwd| recovery_path_matches(actual, cwd))
}
