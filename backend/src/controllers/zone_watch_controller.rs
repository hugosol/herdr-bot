//! Read-only SSE framing for shared zone observations.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::{IntoResponse, Response, Sse, sse::{Event, KeepAlive}};
use futures_util::StreamExt;
use serde::Serialize;

use crate::AppState;
use crate::zones::watch::{self, ZoneWatchFrame, ZoneWatchSnapshot};
use super::responses::HttpError;

pub(crate) async fn watch_zones(State(state): State<AppState>) -> Response {
    let observations = match watch::observe(&state).await {
        Ok(observations) => observations,
        Err(error) => return HttpError(error).into_response(),
    };
    let stream = observations.map(|observation| Ok::<Event, Infallible>(match observation {
        watch::Observation::Snapshot(snapshot) => snapshot_event(snapshot),
        watch::Observation::Change(frame) => frame_event(frame),
    }));
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshData {
    zone: String,
    at: u64,
    generation: u64,
}

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

fn frame_event(frame: ZoneWatchFrame) -> Event {
    match frame {
        ZoneWatchFrame::Refresh { zone, at, generation } => named_event(
            "refresh", &RefreshData { zone, at, generation },
        ),
        ZoneWatchFrame::Health { zone, health, message, at, generation } => named_event(
            "health", &HealthData { zone, health, message, at, generation },
        ),
    }
}

fn snapshot_event(snapshot: ZoneWatchSnapshot) -> Event {
    named_event("snapshot", &snapshot).retry(Duration::from_secs(2))
}

fn named_event(name: &'static str, data: &impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(data)
        .expect("监听帧只含字符串、数字与布尔，序列化不会失败")
}
