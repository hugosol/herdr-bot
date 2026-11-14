pub(crate) mod sessions_controller;
pub(crate) mod suites_controller;
pub(crate) mod runs_controller;
pub(crate) mod bots_controller;
pub(crate) mod scheduler_controller;
pub(crate) mod chats_controller;
pub(crate) mod chief_controller;
pub(crate) mod chief_delegation_controller;
pub(crate) mod chief_reports_controller;
pub(crate) mod server_controller;
pub(crate) mod shutdown_controller;
pub(crate) mod zones_controller;
pub(crate) mod tasks_controller;
pub(crate) mod zone_browser_controller;
pub(crate) mod zone_reader_controller;
pub(crate) mod zone_git_controller;
pub(crate) mod zone_watch_controller;

pub(crate) mod extractors;
pub(crate) mod responses;

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use serde::Serialize;
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024;
const MAX_ZONE_REQUEST_BYTES: usize = 64 * 1024;
const MAX_TASK_REQUEST_BYTES: usize = 1024 * 1024;
/// 被拒绝的请求：只说明“哪一类请求被拒绝”，不改变、也不回显任何执行目标。
#[derive(Debug, Serialize)]
struct RejectedResponse {
    /// 机器可读的拒绝类别：`unknown-operation`、`unknown-endpoint`、`unexpected-parameters`。
    code: &'static str,
    /// 面向使用者的说明。
    message: &'static str,
}

#[derive(Serialize)]
struct OwnedRejection { code: &'static str, message: String }
/// 构造一次拒绝响应：请求面越界（`400` / `404`）、已有脚本操作在执行（`409`），
/// 或执行任务没能送回结果（`500`，内部故障）。
/// 这些路径都不接受请求、不排队，也从不接触服务端配置绑定的脚本。
pub(crate) fn rejected(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    eprintln!("[herdr-bot] 请求被拒绝：{code}（HTTP {}）", status.as_u16());
    (status, Json(RejectedResponse { code, message })).into_response()
}

pub(crate) fn rejected_owned(status: StatusCode, code: &'static str, message: String) -> Response {
    (status, Json(OwnedRejection { code, message })).into_response()
}
