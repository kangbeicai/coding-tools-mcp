use std::convert::Infallible;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{OriginalUri, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::oneshot;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::admin::auth::{
    clear_session_cookie, hash_password, session_cookie, validate_credentials, verify_password,
    AdminCredentials, AdminSessionStore,
};
use crate::admin::rpc::{dispatch, failure, success, RpcRequest};
use crate::admin::{embedded_web_asset, embedded_web_asset_count};
use crate::app_state::AppState;
use crate::settings::AdminConfig;

#[derive(Clone)]
struct AdminState {
    app: Arc<AppState>,
    sessions: Arc<AdminSessionStore>,
    web_root: PathBuf,
}

fn admin_config(state: &AdminState) -> Result<AdminConfig, String> {
    state
        .app
        .with_settings(|store| Ok(store.settings().admin))
        .map_err(|error| error.to_string())
}

fn authenticated_response(state: &AdminState, username: String) -> Response {
    let token = state.sessions.create();
    let mut response = Json(serde_json::json!({
        "ok": true,
        "configured": true,
        "authenticated": true,
        "username": username,
    }))
    .into_response();
    set_cookie(&mut response, &session_cookie(&token));
    response
}

fn unauthorized_response() -> Response {
    auth_error(StatusCode::UNAUTHORIZED, "需要管理员登录".into())
}

fn auth_error(status: StatusCode, message: String) -> Response {
    (status, Json(serde_json::json!({"ok": false, "error": message}))).into_response()
}

fn set_cookie(response: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
}

fn is_public_web_path(path: &str) -> bool {
    matches!(path, "login" | "login/")
        || path.starts_with("_app/")
        || path.rsplit('/').next().is_some_and(|name| name.contains('.'))
}

async fn activity_events(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> Response {
    if !state.sessions.is_authenticated(&headers) {
        return unauthorized_response();
    }
    let stream = BroadcastStream::new(state.app.activity.subscribe()).filter_map(|result| {
        let event = result.ok()?;
        let kind = event.kind.clone();
        let data = serde_json::to_string(&event).ok()?;
        Some(Ok::<Event, Infallible>(
            Event::default().event(kind).data(data),
        ))
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub struct AdminProcess {
    pub shutdown: oneshot::Sender<()>,
    pub handle: crate::async_runtime::JoinHandle<()>,
    pub local_endpoint: String,
    pub web_source: String,
}

pub fn spawn_admin_listener(
    app: Arc<AppState>,
    config: AdminConfig,
    web_root: PathBuf,
) -> Result<AdminProcess, String> {
    let bind_ip = parse_bind_host(&config.bind_host)?;
    let listener = std::net::TcpListener::bind((bind_ip, config.local_port)).map_err(|error| {
        format!(
            "Admin 地址 {}:{} 绑定失败: {error}",
            config.bind_host, config.local_port
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Admin 监听器设置非阻塞失败: {error}"))?;
    let filesystem_web = web_root.join("index.html").is_file();
    let embedded_count = embedded_web_asset_count();
    let web_source = if filesystem_web {
        format!("filesystem ({})", web_root.display())
    } else if embedded_count > 0 {
        format!("embedded ({embedded_count} assets)")
    } else {
        "unavailable".into()
    };
    let state = AdminState {
        app,
        sessions: Arc::new(AdminSessionStore::default()),
        web_root,
    };
    let endpoint = match bind_ip {
        std::net::IpAddr::V4(ip) => format!("http://{ip}:{}", config.local_port),
        std::net::IpAddr::V6(ip) => format!("http://[{ip}]:{}", config.local_port),
    };
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
            .route("/api/auth/status", get(auth_status))
            .route("/api/auth/setup", post(auth_setup))
            .route("/api/auth/login", post(auth_login))
            .route("/api/auth/logout", post(auth_logout))
            .route("/api/rpc", post(rpc_post))
            .route("/api/health", get(api_health))
            .route("/api/activity/events", get(activity_events))
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
        web_source,
    })
}

async fn auth_status(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    let config = match admin_config(&state) {
        Ok(config) => config,
        Err(error) => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    };
    let configured = !config.password_hash.trim().is_empty();
    let authenticated = configured && state.sessions.is_authenticated(&headers);
    Json(serde_json::json!({
        "ok": true,
        "configured": configured,
        "authenticated": authenticated,
        "username": config.username,
    }))
    .into_response()
}

async fn auth_setup(
    State(state): State<AdminState>,
    Json(credentials): Json<AdminCredentials>,
) -> Response {
    if let Err(error) = validate_credentials(&credentials) {
        return auth_error(StatusCode::BAD_REQUEST, error);
    }
    let password_hash = match hash_password(&credentials.password) {
        Ok(hash) => hash,
        Err(error) => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    };
    let username = credentials.username.trim().to_string();
    let update = state.app.with_settings(|store| {
        let mut settings = store.settings();
        if !settings.admin.password_hash.trim().is_empty() {
            return Err(crate::error::AppError::Message(
                "管理员已完成初始化，请直接登录".into(),
            ));
        }
        settings.admin.username = username.clone();
        settings.admin.password_hash = password_hash.clone();
        store.update_settings(settings)
    });
    if let Err(error) = update {
        return auth_error(StatusCode::CONFLICT, error.to_string());
    }
    authenticated_response(&state, username)
}

async fn auth_login(
    State(state): State<AdminState>,
    Json(credentials): Json<AdminCredentials>,
) -> Response {
    let config = match admin_config(&state) {
        Ok(config) => config,
        Err(error) => return auth_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    };
    if config.password_hash.trim().is_empty() {
        return auth_error(StatusCode::CONFLICT, "请先设置管理员账号".into());
    }
    if credentials.username.trim() != config.username
        || !verify_password(&credentials.password, &config.password_hash)
    {
        return auth_error(StatusCode::UNAUTHORIZED, "用户名或密码错误".into());
    }
    authenticated_response(&state, config.username)
}

async fn auth_logout(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    state.sessions.revoke_from_headers(&headers);
    let mut response = Json(serde_json::json!({"ok": true})).into_response();
    set_cookie(&mut response, &clear_session_cookie());
    response
}

async fn rpc_post(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(request): Json<RpcRequest>,
) -> Response {
    if !state.sessions.is_authenticated(&headers) {
        return unauthorized_response();
    }
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

async fn static_file(
    State(state): State<AdminState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let raw_relative = uri.path().trim_start_matches('/');
    if !is_public_web_path(raw_relative) && !state.sessions.is_authenticated(&headers) {
        return Redirect::temporary("/login").into_response();
    }
    let relative = raw_relative;
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

    if let Some(bytes) = embedded_web_asset(&relative_path(&requested, &state.web_root)) {
        return bytes_response(bytes.to_vec(), content_type(&requested));
    }

    // SPA fallback for SvelteKit routes such as /settings/gateway.
    let index = state.web_root.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => bytes_response(bytes, "text/html; charset=utf-8"),
        Err(_) => embedded_web_asset("index.html")
            .map(|bytes| bytes_response(bytes.to_vec(), "text/html; charset=utf-8"))
            .unwrap_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Web UI 不可用。开发构建请先运行 `npm run build` 再构建 coding-tools；也可以使用 --web-root 指向外部构建目录。",
                )
                    .into_response()
            }),
    }
}

fn relative_path(path: &std::path::Path, root: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

fn parse_bind_host(value: &str) -> Result<std::net::IpAddr, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("localhost") {
        return Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    value
        .parse::<std::net::IpAddr>()
        .map_err(|_| format!("Admin 监听地址无效: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_listener_accepts_lan_and_wildcard_bindings() {
        assert!(parse_bind_host("0.0.0.0").is_ok());
        assert!(parse_bind_host("192.168.3.19").is_ok());
        assert!(parse_bind_host("127.0.0.1").is_ok());
        assert!(parse_bind_host("localhost").is_ok());
        assert!(parse_bind_host("not-an-ip").is_err());
    }

    #[test]
    fn static_path_rejects_parent_traversal() {
        assert!(safe_relative_path("../data/profiles.json").is_none());
        assert!(safe_relative_path("_app/app.js").is_some());
    }

    #[test]
    fn only_login_and_static_assets_are_public_web_paths() {
        assert!(is_public_web_path("login"));
        assert!(is_public_web_path("_app/immutable/app.js"));
        assert!(is_public_web_path("favicon.ico"));
        assert!(!is_public_web_path(""));
        assert!(!is_public_web_path("activity"));
        assert!(!is_public_web_path("settings/gateway"));
    }
}
