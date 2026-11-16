//! Raw reader resources, URL decoding and browser security headers.

use std::path::Path;
use axum::body::Body;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::zones::browse;
use super::rejected_owned;
use super::responses::HttpError;

pub(crate) async fn read_zone_reader_resource(
    State(state): State<AppState>,
    UrlPath((zone_token, path)): UrlPath<(String, String)>,
) -> Response {
    let requested = match decode_zone_token(&zone_token) {
        Ok(zone) => zone,
        Err(message) => return rejected_owned(StatusCode::BAD_REQUEST, "invalid-zone-token", message),
    };
    let content_type = resource_content_type(&path);
    match browse::resource(&state, &requested, path).await {
        Ok(bytes) => resource_response(bytes, content_type),
        Err(error) => HttpError(error).into_response(),
    }
}

fn decode_zone_token(token: &str) -> Result<String, String> {
    if token.is_empty() || token.len() % 2 != 0 {
        return Err("HTML 阅读资源地址中的 zone 编码无效。".to_string());
    }
    let mut bytes = Vec::with_capacity(token.len() / 2);
    for pair in token.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair)
            .map_err(|_| "HTML 阅读资源地址中的 zone 编码无效。".to_string())?;
        let byte = u8::from_str_radix(text, 16)
            .map_err(|_| "HTML 阅读资源地址中的 zone 编码无效。".to_string())?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| "HTML 阅读资源地址中的 zone 编码不是 UTF-8。".to_string())
}

fn resource_response(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src 'self' data: blob:; media-src 'self' data: blob:; \
             font-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'none'; \
             object-src 'none'; frame-src 'none'; child-src 'none'; connect-src 'none'; \
             form-action 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let mut response = (StatusCode::OK, Body::from(bytes)).into_response();
    *response.headers_mut() = headers;
    response
}

fn resource_content_type(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "txt" | "md" | "markdown" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
