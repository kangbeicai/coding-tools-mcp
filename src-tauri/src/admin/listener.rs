use std::path::{Component, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::oneshot;

use crate::admin::rpc::{dispatch, failure, success, RpcRequest};
use crate::app_state::AppState;
use crate::settings::AdminConfig;

#[derive(Clone)]
struct AdminState {
    app: Arc<AppState>,
    web_root: PathBuf,
}

pub struct AdminProcess {
    pub shutdown: oneshot::Sender<()>,
    pub handle: crate::async_runtime::JoinHandle<()>,
    pub local_endpoint: String,
}

pub fn spawn_admin_listener(
    app: Arc<AppState>,
    config: AdminConfig,
    web_root: PathBuf,
) -> Result<AdminProcess, String> {
    let bind_ip = parse_loopback(&config.bind_host)?;
    let listener = std::net::TcpListener::bind((bind_ip, config.local_port)).map_err(|error| {
        format!(
            "Admin 地址 {}:{} 绑定失败: {error}",
            config.bind_host, config.local_port
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Admin 监听器设置非阻塞失败: {error}"))?;
    let state = AdminState { app, web_root };
    let endpoint = format!("http://127.0.0.1:{}", config.local_port);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = crate::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("admin web listener init failed: {error}");
                return;
            }
        };
        let app = Router::new()
            .route("/api/rpc", post(rpc_post))
            .route("/api/health", get(api_health))
            .fallback(get(static_file))
            .with_state(state);
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
        {
            eprintln!("admin web listener stopped: {error}");
        }
    });

    Ok(AdminProcess {
        shutdown: shutdown_tx,
        handle,
        local_endpoint: endpoint,
    })
}

async fn rpc_post(State(state): State<AdminState>, Json(request): Json<RpcRequest>) -> Response {
    match dispatch(&state.app, request).await {
        Ok(value) => Json(success(value)).into_response(),
        Err(message) => (StatusCode::BAD_REQUEST, Json(failure(message))).into_response(),
    }
}

async fn api_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "coding-tools-admin",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn static_file(State(state): State<AdminState>, OriginalUri(uri): OriginalUri) -> Response {
    let relative = uri.path().trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let requested = match safe_relative_path(relative) {
        Some(path) => state.web_root.join(path),
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if let Ok(bytes) = tokio::fs::read(&requested).await {
        return bytes_response(bytes, content_type(&requested));
    }

    // SPA fallback for SvelteKit routes such as /settings/gateway.
    let index = state.web_root.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => bytes_response(bytes, "text/html; charset=utf-8"),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Web UI 尚未构建。先在项目根目录运行 `npm run build`，或使用 --web-root 指向构建目录。",
        )
            .into_response(),
    }
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(path)
}

fn bytes_response(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn parse_loopback(value: &str) -> Result<std::net::IpAddr, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("localhost") {
        return Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    let ip = value
        .parse::<std::net::IpAddr>()
        .map_err(|_| format!("Admin 监听地址无效: {value}"))?;
    if !ip.is_loopback() {
        return Err(
            "Admin Web 当前仅允许监听 loopback 地址。远程 Linux 管理请使用 SSH 端口转发；在加入独立管理员认证前不允许直接暴露管理 API。"
                .into(),
        );
    }
    Ok(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_listener_rejects_non_loopback_bindings() {
        assert!(parse_loopback("0.0.0.0").is_err());
        assert!(parse_loopback("127.0.0.1").is_ok());
        assert!(parse_loopback("localhost").is_ok());
    }

    #[test]
    fn static_path_rejects_parent_traversal() {
        assert!(safe_relative_path("../data/profiles.json").is_none());
        assert!(safe_relative_path("_app/app.js").is_some());
    }
}
