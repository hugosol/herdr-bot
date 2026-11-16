//! Zone Git operations own repository observation and command lifetimes.
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::{AppState, zones};
use super::{self as git, commit, sync};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation { Overview, Status, Diff, Commit, Check, Sync }

pub(crate) enum Error {
    Registry(zones::ZoneError),
    Lookup(zones::LookupError),
    Git(git::GitError),
    Diff(git::DiffError),
    TaskFailed(Operation),
    CommitResultUnavailable,
    SyncResultUnavailable,
    EmptyMessage,
    ReviewRequired,
}

pub(crate) async fn overview(state: &AppState) -> Result<Vec<git::ZoneSummary>, Error> {
    let zones = state.zones.list().await.map_err(Error::Registry)?;
    state.zone_watch.reconcile(&zones).await;
    let timeout = state.git_timeout;
    let sync_state = Arc::clone(&state.sync_state);
    tokio::task::spawn_blocking(move || git::overview(&zones, timeout, &sync_state))
        .await.map_err(|_| Error::TaskFailed(Operation::Overview))
}

pub(crate) async fn status(state: &AppState, zone: &str, refresh: bool) -> Result<git::GitStatus, Error> {
    let registered = zones::registered_zone(state, zone).await.map_err(Error::Lookup)?;
    let timeout = state.git_timeout;
    let sync_state = Arc::clone(&state.sync_state);
    tokio::task::spawn_blocking(move || sync_state.cached_status(&registered, timeout, refresh))
        .await.map_err(|_| Error::TaskFailed(Operation::Status))?.map_err(Error::Git)
}

pub(crate) async fn diff(state: &AppState, zone: &str, path: String, old_path: Option<String>) -> Result<git::ChangeDiff, Error> {
    let registered = zones::registered_zone(state, zone).await.map_err(Error::Lookup)?;
    let limit = state.preview_limit_bytes;
    let timeout = state.git_timeout;
    tokio::task::spawn_blocking(move || git::change_diff(&registered, &path, old_path.as_deref(), limit, timeout))
        .await.map_err(|_| Error::TaskFailed(Operation::Diff))?.map_err(Error::Diff)
}

pub(crate) async fn commit(state: AppState, zone: &str, message: String, review_token: String) -> Result<(String, commit::CommitOutcome), Error> {
    if message.trim().is_empty() { return Err(Error::EmptyMessage); }
    if review_token.trim().is_empty() { return Err(Error::ReviewRequired); }
    let registered = zones::registered_zone(&state, zone).await.map_err(Error::Lookup)?;
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let response_zone = registered.clone();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let operation_zone = registered.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            commit::execute(
                &registered,
                &message,
                &review_token,
                timeout,
                &locks,
                &sync_state,
            )
        })
        .await;
        state.zone_watch.notify(&operation_zone).await;
        if matches!(&outcome, Ok(commit::CommitOutcome::Committed { .. })) {
            let background_state = state.clone();
            tokio::spawn(async move {
                let check_zone = operation_zone.clone();
                let sync_state = Arc::clone(&background_state.sync_state);
                let locks = Arc::clone(&background_state.repository_locks);
                let result = tokio::task::spawn_blocking(move || {
                    sync::check_background(&check_zone, timeout, &locks, &sync_state)
                })
                .await;
                let error = match result {
                    Ok(sync::CheckOutcome::Checked(_)) => None,
                    Ok(sync::CheckOutcome::NoUpstream) => Some("当前分支没有配置 upstream，无法检查远端；本功能不会自动发布分支或配置 upstream。".to_string()),
                    Ok(sync::CheckOutcome::Blocked(reason)) => Some(reason),
                    Ok(sync::CheckOutcome::Failed(error)) => Some(error.message()),
                    Ok(sync::CheckOutcome::Busy) => Some("仓库已有 Git 写操作，提交后的远端检查未执行。".to_string()),
                    Ok(sync::CheckOutcome::NotRepository) => Some("提交后的目录不再是 Git 工作树根，无法检查远端。".to_string()),
                    Err(_) => Some("提交后的远端检查任务异常结束，无法确认远端状态。".to_string()),
                };
                if error.is_some() {
                    background_state
                        .sync_state
                        .cache
                        .invalidate(&operation_zone);
                }
                background_state
                    .sync_state
                    .finish_remote_check(&operation_zone, error);
                background_state.zone_watch.notify(&operation_zone).await;
            });
        }
        let response = outcome.map(|outcome| (response_zone, outcome)).map_err(|_| Error::TaskFailed(Operation::Commit));
        // 连接已经断开时接收端不存在，结果丢弃；已经开始的提交不会被取消或重放。
        let _ = sender.send(response);
    });

    receiver.await.map_err(|_| Error::CommitResultUnavailable)?
}

pub(crate) async fn check(state: &AppState, zone: &str) -> Result<sync::CheckOutcome, Error> {
    let registered = zones::registered_zone(state, zone).await.map_err(Error::Lookup)?;
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let refresh_zone = registered.clone();
    let outcome = tokio::task::spawn_blocking(move || sync::check(&registered, timeout, &locks, &sync_state)).await;
    state.zone_watch.notify(&refresh_zone).await;
    outcome.map_err(|_| Error::TaskFailed(Operation::Check))
}

pub(crate) async fn sync(state: AppState, zone: &str) -> Result<(String, sync::SyncOutcome), Error> {
    let registered = zones::registered_zone(&state, zone).await.map_err(Error::Lookup)?;
    let timeout = state.git_timeout;
    let locks = Arc::clone(&state.repository_locks);
    let sync_state = Arc::clone(&state.sync_state);
    let response_zone = registered.clone();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let outcome = tokio::task::spawn_blocking(move || {
            sync::execute(&registered, timeout, &locks, &sync_state)
        })
        .await;
        state.zone_watch.notify(&response_zone).await;
        let response = outcome.map(|outcome| (response_zone, outcome)).map_err(|_| Error::TaskFailed(Operation::Sync));
        let _ = sender.send(response);
    });

    receiver.await.map_err(|_| Error::SyncResultUnavailable)?
}
