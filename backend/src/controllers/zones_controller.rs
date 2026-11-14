use axum::{Json, body::to_bytes, extract::{Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use crate::{AppState, zones::{self, RegisteredZone, ZoneError}};
use super::{chat_command_body, rejected_owned, MAX_ZONE_REQUEST_BYTES};
/// zone 登记列表的响应：当前完整列表（登记与取消登记也返回同一形状）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ZoneCreationView {
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_directory: Option<String>,
}

#[derive(Debug, Serialize)]
struct ZoneListResponse {
    zones: Vec<RegisteredZone>,
    creation: ZoneCreationView,
}

/// 一次 zone 命令的请求体：服务器上的目录路径。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZonePathRequest {
    id: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZoneIdRequest { id: String }

/// 新建 zone 的请求体：只接受一个目录名称，父目录只来自服务端配置。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateZoneRequest {
    id: String,
    name: String,
}

/// “列出 zone 登记”：读取后端持久保存的同一份列表，不经过任何页面缓存。
pub(crate) async fn list_zones(State(state): State<AppState>) -> Response {
    zone_list_response(&state, state.zones.list().await)
}

/// “登记 zone”：路径必须真实存在且是服务器上的绝对目录；只有持久化成功后返回成功。
pub(crate) async fn register_zone(State(state): State<AppState>, request: Request) -> Response {
    let binding = match zone_path(request).await { Ok(binding) => binding, Err(error) => return error };
    zone_list_response(&state, zones::register_zone(&state, &binding.id, &binding.path).await)
}

pub(crate) async fn create_zone(State(state): State<AppState>, request: Request) -> Response {
    let input = match zone_create_name(request).await { Ok(input) => input, Err(error) => return error };
    match zones::create_zone(&state, input.id, input.name).await {
        Ok(zones) => zone_list_response(&state, Ok(zones)),
        Err(zones::CreationError::Registry(error)) => zone_error_response(error),
        Err(zones::CreationError::Create(error)) => zone_create_error_response(error),
        Err(zones::CreationError::NotConfigured) => rejected_owned(StatusCode::BAD_REQUEST, "create-parent-not-configured", "未配置 [zones] create_parent_directory；本次没有创建任何目录。".to_string()),
        Err(zones::CreationError::TaskFailed) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "create-task-failed", "新建 zone 的任务异常结束，无法确认目录或 Git 状态。".to_string()),
        Err(zones::CreationError::Registration { path, error }) => rejected_owned(StatusCode::INTERNAL_SERVER_ERROR, "zone-registration-failed", format!("目录与 Git 初始化已完成，但 zone 登记失败，目录已保留：{path}；{}", error.message())),
    }
}

pub(crate) async fn unregister_zone(State(state): State<AppState>, request: Request) -> Response {
    let input: ZoneIdRequest = match chat_command_body(request, "取消 zone 登记").await { Ok(input) => input, Err(error) => return error };
    zone_list_response(&state, zones::unregister_zone(&state, &input.id).await)
}

pub(crate) async fn rebind_zone(State(state): State<AppState>, request: Request) -> Response {
    let input = match zone_path(request).await { Ok(input) => input, Err(error) => return error };
    zone_list_response(&state, zones::rebind_zone(&state, &input.id, &input.path).await)
}
/// 读取 zone 命令的请求体；请求面封闭（不接受查询串或未知字段）。
async fn zone_path(request: Request) -> Result<ZonePathRequest, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "zone 命令不接受查询参数：路径只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "zone 请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let body: ZonePathRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析 zone 命令：请发送 {\"id\":\"稳定 zone ID\",\"path\":\"服务器目录\"} 形式的 JSON。".to_string(),
        )
    })?;
    Ok(body)
}

/// 读取新建 zone 的请求体；请求面同样封闭，父目录不会从请求中接受。
async fn zone_create_name(request: Request) -> Result<CreateZoneRequest, Response> {
    if request.uri().query().is_some_and(|query| !query.is_empty()) {
        return Err(rejected_owned(
            StatusCode::BAD_REQUEST,
            "unexpected-parameters",
            "新建 zone 不接受查询参数：名称只来自请求体。".to_string(),
        ));
    }

    let (_, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_ZONE_REQUEST_BYTES).await.map_err(|_| {
        rejected_owned(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload-too-large",
            "新建 zone 的请求体超过 64KB 上限。".to_string(),
        )
    })?;
    let body: CreateZoneRequest = serde_json::from_slice(&bytes).map_err(|_| {
        rejected_owned(
            StatusCode::BAD_REQUEST,
            "invalid-request",
            "无法解析新建 zone 请求：请发送 {\"id\":\"稳定 zone ID\",\"name\":\"目录名称\"} 形式的 JSON。".to_string(),
        )
    })?;
    Ok(body)
}

/// 把创建 zone 的失败翻译成稳定 HTTP 类别；失败消息保留目录路径与失败阶段。
fn zone_create_error_response(error: zones::create::CreateZoneError) -> Response {
    use zones::create::CreateZoneError;
    let status = match &error {
        CreateZoneError::AlreadyExists { .. } => StatusCode::CONFLICT,
        CreateZoneError::ParentNotAbsolute(_)
        | CreateZoneError::ParentNotFound(_)
        | CreateZoneError::ParentNotDirectory(_)
        | CreateZoneError::ParentInaccessible { .. }
        | CreateZoneError::NameRequired
        | CreateZoneError::InvalidName(_) => StatusCode::BAD_REQUEST,
        CreateZoneError::CreateFailed { .. }
        | CreateZoneError::GitInitFailed { .. }
        | CreateZoneError::FinalizeFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    rejected_owned(status, error.code(), error.message())
}

/// 把登记操作的结果翻译成 HTTP 响应：成功返回当前完整列表与创建能力，失败不伪装成空列表。
fn zone_list_response(
    state: &AppState,
    result: Result<Vec<RegisteredZone>, ZoneError>,
) -> Response {
    match result {
        Ok(zones) => {
            let creation = ZoneCreationView {
                available: state.create_parent_directory.is_some(),
                parent_directory: state
                    .create_parent_directory
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            };
            (StatusCode::OK, Json(ZoneListResponse { zones, creation })).into_response()
        }
        Err(error) => zone_error_response(error),
    }
}

pub(crate) fn zone_error_response(error: ZoneError) -> Response {
    let status = match &error {
        ZoneError::AlreadyRegistered => StatusCode::CONFLICT,
        ZoneError::NotRegistered => StatusCode::NOT_FOUND,
        ZoneError::Unreadable(_) | ZoneError::Corrupt(_) | ZoneError::WriteFailed(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        ZoneError::IdRequired
        | ZoneError::Required
        | ZoneError::NotAbsolute
        | ZoneError::NotFound
        | ZoneError::NotADirectory
        | ZoneError::Inaccessible(_) => StatusCode::BAD_REQUEST,
    };
    rejected_owned(status, error.code(), error.message())
}


pub(crate) fn lookup_error_response(error: zones::LookupError) -> Response {
    match error {
        zones::LookupError::Registry(error) => zone_error_response(error),
        zones::LookupError::Browse(error) => super::zone_browser_controller::browse_error_response(error),
    }
}
