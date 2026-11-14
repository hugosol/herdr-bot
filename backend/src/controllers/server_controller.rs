use axum::{Json, extract::{Path as UrlPath, State}, http::StatusCode, response::{IntoResponse, Response}};
use crate::{AppState, operations};
use super::{rejected, extractors::system::ClosedOperation, responses::HttpError};
/// “启动”操作：执行配置绑定的启动批处理。
pub(crate) async fn start_operation(State(state): State<AppState>, _: ClosedOperation) -> Response {
    run_fixed_operation("start", &state.start_operation, &state.execution).await
}

/// “停止”操作：执行配置绑定的关闭批处理。
pub(crate) async fn stop_operation(State(state): State<AppState>, _: ClosedOperation) -> Response {
    run_fixed_operation("stop", &state.stop_operation, &state.execution).await
}


async fn run_fixed_operation(name: &'static str, operation: &crate::ScriptOperation, execution: &std::sync::Arc<tokio::sync::Mutex<()>>) -> Response {
    match operations::execute(name, operation, execution).await {
        Ok(result) => Json(result).into_response(),
        Err(error) => HttpError(error).into_response(),
    }
}
/// 固定操作之外的所有 `/api/` 请求：以 JSON 类别拒绝，不执行任何脚本，也不交给静态页面。
pub(crate) async fn unknown_api_request(UrlPath(path): UrlPath<String>) -> Response {
    if path == "operations" || path.starts_with("operations/") {
        rejected(
            StatusCode::NOT_FOUND,
            "unknown-operation",
            "本服务只提供“启动”与“停止”两个固定操作，操作名与脚本位置不能由客户端指定。",
        )
    } else {
        rejected(
            StatusCode::NOT_FOUND,
            "unknown-endpoint",
            "该接口路径不存在。",
        )
    }
}

