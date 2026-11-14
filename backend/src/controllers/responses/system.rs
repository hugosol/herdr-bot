use axum::{http::StatusCode, response::{IntoResponse, Response}};

use crate::{MonitorRefusal, operations::ExecutionError, sessions::SessionEventError};
use crate::controllers::rejected;
use super::HttpError;

impl IntoResponse for HttpError<SessionEventError> {
    fn into_response(self) -> Response {
        match self.0 {
            SessionEventError::Forbidden => rejected(StatusCode::FORBIDDEN, "forbidden", "会话事件写入只接受本机来源：局域网设备不能伪造或刷面板。"),
            SessionEventError::UnknownSession => rejected(StatusCode::NOT_FOUND, "unknown-session", "该会话尚未注册：非 register 事件不能为未知会话建行。"),
            SessionEventError::Chief(error) => HttpError(error).into_response(),
        }
    }
}

impl IntoResponse for HttpError<MonitorRefusal> {
    fn into_response(self) -> Response {
        match self.0 {
            MonitorRefusal::UnknownSession => rejected(
                StatusCode::NOT_FOUND,
                "unknown-session",
                "该会话尚未注册：监听只针对当前存在的会话。",
            ),
            MonitorRefusal::NotEligible => rejected(
                StatusCode::CONFLICT,
                "session-not-eligible",
                "该会话当前不满足新增监听的条件：只有在线且工作中的会话可以开始监听。",
            ),
            MonitorRefusal::CountdownActive => rejected(
                StatusCode::CONFLICT,
                "shutdown-in-progress",
                "已有活动倒计时：期间不接受新增监听，需要先取消当前安排。",
            ),
        }
    }
}

impl IntoResponse for HttpError<ExecutionError> {
    fn into_response(self) -> Response {
        match self.0 {
            ExecutionError::Busy => rejected(StatusCode::CONFLICT, "operation-in-progress", "已有脚本操作正在执行：本次请求没有被执行，也不会在当前操作结束后被补做。"),
            ExecutionError::ResultUnavailable => rejected(StatusCode::INTERNAL_SERVER_ERROR, "operation-result-unavailable", "本次操作没有送回结果：脚本的执行结果无法确认。"),
        }
    }
}
