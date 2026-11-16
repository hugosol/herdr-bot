use axum::{Json, body::to_bytes, extract::{Request, State}, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use crate::{AppState, zones::{self, RegisteredZone, ZoneError}};
use super::{rejected_owned, MAX_ZONE_REQUEST_BYTES, extractors::{CommandJson, zones::{ZonePathRequest, UnregisterZone}}, responses::HttpError};
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



#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ZoneIdRequest { id: String }

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
pub(crate) async fn register_zone(State(state): State<AppState>, binding: ZonePathRequest) -> Response {
    zone_list_response(&state, zones::register_zone(&state, &binding.id, &binding.path).await)
}

pub(crate) async fn create_zone(State(state): State<AppState>, request: Request) -> Response {
    let input = match zone_create_name(request).await { Ok(input) => input, Err(error) => return error };
    match zones::create_zone(&state, input.id, input.name).await {
        Ok(zones) => zone_list_response(&state, Ok(zones)),
        Err(error) => HttpError(error).into_response(),
    }
}

pub(crate) async fn unregister_zone(State(state): State<AppState>, input: CommandJson<ZoneIdRequest, UnregisterZone>) -> Response {
    let input = input.value;
    zone_list_response(&state, zones::unregister_zone(&state, &input.id).await)
}

pub(crate) async fn rebind_zone(State(state): State<AppState>, input: ZonePathRequest) -> Response {
    zone_list_response(&state, zones::rebind_zone(&state, &input.id, &input.path).await)
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
        Err(error) => HttpError(error).into_response(),
    }
}





