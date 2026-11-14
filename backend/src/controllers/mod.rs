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

use axum::{Json, body::to_bytes, extract::Request, http::StatusCode, response::{IntoResponse, Response}};
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
/// 解析 Chat 命令的 JSON 请求体；查询串与未知字段都被拒绝。
pub(crate) async fn chat_command_body<T: serde::de::DeserializeOwned>(
    request: Request,
    action: &str,
) -> Result<T, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            format!("{action}不接受查询参数：字段只来自请求体。"),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_TASK_REQUEST_BYTES)
        .await
        .map_err(|_| {
            rejected_owned(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload-too-large",
                format!("{action}的请求体超过 1 MiB 上限。"),
            )
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            format!("无法解析{action}请求：{error}"),
        )
    })
}

/// 封闭请求面检查：固定操作只接受不带查询串、不带请求体的 POST。
///
/// 客户端没有指定执行目标、命令或参数的能力，所以任何查询参数或请求体都是越界请求，
/// 一律按同一个类别拒绝。
pub(crate) async fn reject_unclosed_request(request: Request) -> Option<Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "操作请求不接受查询参数：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
        ));
    }

    let (_, body) = request.into_parts();
    match to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) if bytes.is_empty() => None,
        // 读到内容、读不完（超出上限）或读取失败，都说明这不是一个封闭的请求。
        _ => Some(rejected(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "操作请求不接受请求体：执行哪个脚本、用什么命令和参数只由服务端配置决定。",
        )),
    }
}

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
