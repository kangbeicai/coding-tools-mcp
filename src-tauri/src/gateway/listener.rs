use std::sync::Arc;

use axum::extract::{Form, Path, Query, State};
use axum::http::{header::CACHE_CONTROL, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use crate::auth::{
    authorization_server_metadata, authorize_get, authorize_post, external_base_url,
    protected_resource_metadata, token_exchange, verify_bearer_header, verify_oauth_bearer_header,
    AuthorizeForm, AuthorizeParams, OAuthRuntime, TokenForm,
};
use crate::gateway::server::handle_request;
use crate::gateway::state::{GatewayState, SharedGatewayState};
use crate::secret::SecretStore;
use crate::settings::GatewayConfig;
use crate::tunnel::append_profile_log;
use crate::workspace::{AuthConfig, WorkspaceProfile};

const GATEWAY_LOG_ID: &str = "gateway";

#[derive(Clone)]
struct ListenerState {
    gateway: SharedGatewayState,
    auth: AuthConfig,
    bind_port: u16,
    configured_public_url: String,
    bearer_token: Option<String>,
    oauth: Option<Arc<OAuthRuntime>>,
    oauth_client_secret: Option<String>,
}

pub struct GatewayProcess {
    pub shutdown: oneshot::Sender<()>,
    pub handle: crate::async_runtime::JoinHandle<()>,
    pub state: SharedGatewayState,
    pub local_endpoint: String,
}

pub fn spawn_listener(
    config: GatewayConfig,
    profiles: Vec<WorkspaceProfile>,
) -> Result<GatewayProcess, String> {
    let client_id = SecretStore::get_shared("oauth_client_id")
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let auth = AuthConfig {
        auth_type: config.auth_type.trim().to_ascii_lowercase(),
        oauth_client_id: client_id,
        use_shared_secrets: true,
    };
    let gateway = GatewayState::new(&profiles, &config, auth.clone())?;

    let bearer_token = if auth.bearer_enabled() {
        SecretStore::get_shared("bearer_token")
            .map_err(|error| error.to_string())?
            .filter(|value| !value.is_empty())
    } else {
        None
    };
    if auth.bearer_enabled() && bearer_token.is_none() {
        return Err("Gateway Bearer 认证已启用，但共享 bearer_token 不存在".into());
    }

    // Match the legacy MCP listener: ChatGPT uses PKCE and client_secret is
    // optional. The gateway therefore only needs the shared Client ID,
    // authorization password and token-signing secret.
    let oauth_client_secret = None;
    let oauth_password = if auth.oauth_enabled() {
        SecretStore::get_shared("oauth_password")
            .map_err(|error| error.to_string())?
            .unwrap_or_default()
    } else {
        String::new()
    };
    let oauth_token_secret = if auth.oauth_enabled() {
        SecretStore::get_shared("oauth_token_secret")
            .map_err(|error| error.to_string())?
            .unwrap_or_default()
    } else {
        String::new()
    };
    if auth.oauth_enabled()
        && (auth.oauth_client_id.trim().is_empty()
            || oauth_password.is_empty()
            || oauth_token_secret.is_empty())
    {
        return Err("Gateway OAuth 已启用，但共享 OAuth 凭据尚未初始化".into());
    }

    let bind_host = config.bind_host.trim().to_string();
    let port = config.local_port;
    let configured_public_url = normalize_public_url(&config.public_url);
    let oauth = if auth.oauth_enabled() {
        let base = external_base_url(&HeaderMap::new(), port, &configured_public_url);
        Some(Arc::new(OAuthRuntime::new(
            base,
            auth.oauth_client_id.clone(),
            oauth_client_secret.clone(),
            oauth_password,
            oauth_token_secret,
        )))
    } else {
        None
    };
    let listener = bind_listener(&bind_host, port)?;
    let state = ListenerState {
        gateway: gateway.clone(),
        auth,
        bind_port: port,
        configured_public_url,
        bearer_token,
        oauth,
        oauth_client_secret,
    };
    let local_endpoint = local_endpoint(&bind_host, port);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = crate::async_runtime::spawn(async move {
        let result = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => serve(listener, &bind_host, port, state, shutdown_rx).await,
            Err(error) => Err(format!("Gateway Tokio 监听器初始化失败: {error}").into()),
        };
        if let Err(error) = result {
            append_profile_log(
                GATEWAY_LOG_ID,
                "stderr.log",
                &format!("[gateway] listener stopped: {error}"),
            );
            eprintln!("gateway listener stopped: {error}");
        }
    });

    Ok(GatewayProcess {
        shutdown: shutdown_tx,
        handle,
        state: gateway,
        local_endpoint,
    })
}

async fn serve(
    listener: tokio::net::TcpListener,
    bind_host: &str,
    port: u16,
    state: ListenerState,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/mcp", get(mcp_discovery).post(mcp_post))
        .route(
            "/w/{workspace}/mcp",
            get(mcp_discovery).post(workspace_mcp_post),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource_metadata),
        )
        .route(
            "/oauth/authorize",
            get(oauth_authorize_get).post(oauth_authorize_post),
        )
        .route("/oauth/token", post(oauth_token_post))
        .with_state(state)
        .layer(CorsLayer::permissive());

    append_profile_log(
        GATEWAY_LOG_ID,
        "stdout.log",
        &format!("[gateway] listening on {}", local_endpoint(bind_host, port)),
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown.await;
        })
        .await?;
    Ok(())
}

async fn mcp_discovery(State(state): State<ListenerState>) -> Response {
    (
        [(CACHE_CONTROL, "no-store")],
        Json(json!({
            "name": "coding-tools-gateway",
            "version": env!("CARGO_PKG_VERSION"),
            "protocolVersion": "2025-06-18",
            "workspaceCount": state.gateway.workspace_count(),
            "endpoint": "/mcp"
        })),
    )
        .into_response()
}

async fn mcp_post(
    State(state): State<ListenerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    dispatch_request(state, headers, body, None).await
}

async fn workspace_mcp_post(
    State(state): State<ListenerState>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    dispatch_request(state, headers, body, Some(workspace)).await
}

async fn dispatch_request(
    state: ListenerState,
    headers: HeaderMap,
    body: Value,
    forced_workspace: Option<String>,
) -> Response {
    if let Some(response) = require_mcp_auth(&state, &headers) {
        return response;
    }

    let request_id = body.get("id").cloned().unwrap_or(Value::Null);
    let method = body.get("method").and_then(Value::as_str).unwrap_or("").to_string();
    let tool_name = body
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let transport_session = session_key_from_headers(&headers).map(str::to_string);
    let route_label = forced_workspace.as_deref().unwrap_or("session");
    append_profile_log(
        GATEWAY_LOG_ID,
        "mcp-requests.log",
        &format!(
            "[rpc] request id={} method={} tool={} route={}",
            request_id, method, tool_name, route_label
        ),
    );

    let gateway = state.gateway.clone();
    let result = tokio::task::spawn_blocking(move || {
        handle_request(
            &gateway,
            &body,
            transport_session.as_deref(),
            forced_workspace.as_deref(),
        )
    })
    .await;

    match result {
        Ok(response) => {
            append_profile_log(
                GATEWAY_LOG_ID,
                "mcp-requests.log",
                &format!("[rpc] completed id={} method={} tool={}", request_id, method, tool_name),
            );
            Json(response).into_response()
        }
        Err(error) => Json(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32603,
                "message": "Gateway RPC worker failed",
                "data": { "error": error.to_string(), "retryable": true }
            }
        }))
        .into_response(),
    }
}

fn session_key_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn bind_listener(bind_host: &str, port: u16) -> Result<std::net::TcpListener, String> {
    let ip = bind_host
        .parse::<std::net::IpAddr>()
        .map_err(|_| format!("Gateway 监听地址无效: {bind_host}"))?;
    let listener = std::net::TcpListener::bind(std::net::SocketAddr::new(ip, port))
        .map_err(|error| format!("Gateway 地址 {bind_host}:{port} 绑定失败: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Gateway 监听器设置非阻塞失败: {error}"))?;
    Ok(listener)
}

fn normalize_public_url(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    value.strip_suffix("/mcp").unwrap_or(value).to_string()
}

fn local_endpoint(bind_host: &str, port: u16) -> String {
    let display_host = match bind_host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) if ip.is_unspecified() => "127.0.0.1".to_string(),
        Ok(std::net::IpAddr::V6(ip)) if ip.is_unspecified() => "[::1]".to_string(),
        Ok(std::net::IpAddr::V6(ip)) => format!("[{ip}]"),
        Ok(std::net::IpAddr::V4(ip)) => ip.to_string(),
        Err(_) => bind_host.to_string(),
    };
    format!("http://{display_host}:{port}/mcp")
}

fn resolve_oauth_base(state: &ListenerState, headers: &HeaderMap) -> String {
    external_base_url(headers, state.bind_port, &state.configured_public_url)
}

fn require_mcp_auth(state: &ListenerState, headers: &HeaderMap) -> Option<Response> {
    if state.auth.bearer_enabled() {
        return verify_bearer_header(headers, state.bearer_token.as_deref().unwrap_or(""));
    }
    if state.auth.oauth_enabled() {
        if let Some(oauth) = state.oauth.as_ref() {
            return verify_oauth_bearer_header(headers, oauth, &resolve_oauth_base(state, headers));
        }
    }
    None
}

async fn oauth_authorization_server_metadata(
    State(state): State<ListenerState>,
    headers: HeaderMap,
) -> Response {
    if !state.auth.oauth_enabled() {
        return oauth_not_configured();
    }
    Json(authorization_server_metadata(
        &resolve_oauth_base(&state, &headers),
        state.oauth_client_secret.as_deref(),
    ))
    .into_response()
}

async fn oauth_protected_resource_metadata(
    State(state): State<ListenerState>,
    headers: HeaderMap,
) -> Response {
    if !state.auth.oauth_enabled() {
        return oauth_not_configured();
    }
    Json(protected_resource_metadata(&resolve_oauth_base(&state, &headers))).into_response()
}

async fn oauth_authorize_get(
    State(state): State<ListenerState>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_get(oauth, params, Some("Coding Tools multi-workspace gateway"))
}

async fn oauth_authorize_post(
    State(state): State<ListenerState>,
    headers: HeaderMap,
    Form(form): Form<AuthorizeForm>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return oauth_not_configured();
    };
    authorize_post(oauth, form, &resolve_oauth_base(&state, &headers))
}

async fn oauth_token_post(
    State(state): State<ListenerState>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    let Some(oauth) = state.oauth.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "unsupported_grant_type" })),
        )
            .into_response();
    };
    token_exchange(oauth, &headers, form, &resolve_oauth_base(&state, &headers))
}

fn oauth_not_configured() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "OAuth not configured" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_url_accepts_base_or_mcp_endpoint() {
        assert_eq!(normalize_public_url("https://example.com/mcp"), "https://example.com");
        assert_eq!(normalize_public_url("https://example.com/"), "https://example.com");
    }

    #[test]
    fn wildcard_bind_still_displays_a_connectable_local_url() {
        assert_eq!(local_endpoint("0.0.0.0", 28766), "http://127.0.0.1:28766/mcp");
    }
}

