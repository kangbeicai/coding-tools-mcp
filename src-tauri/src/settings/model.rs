use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::data::AppData;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConfig {
    /// Admin Web Console defaults to loopback only. Remote administration
    /// should normally use SSH port forwarding rather than exposing this port.
    #[serde(default = "default_admin_bind_host")]
    pub bind_host: String,
    #[serde(default = "default_admin_port")]
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayExposureConfig {
    /// `local`, `direct`, `external`, `frp`, or `cloudflare`.
    #[serde(default = "default_gateway_exposure_mode")]
    pub mode: String,
    /// Reuse a globally configured FRP server profile when non-empty.
    #[serde(default)]
    pub frp_profile_id: String,
    /// Inline FRP server fallback used when no profile is selected.
    #[serde(default)]
    pub frp_server: String,
    #[serde(default = "default_frp_server_port")]
    pub frp_server_port: u16,
    #[serde(default)]
    pub frp_subdomain: String,
    /// `quick` or `named`.
    #[serde(default = "default_cloudflare_mode")]
    pub cloudflare_mode: String,
    /// Reuse the global outbound proxy when starting managed exposure clients.
    #[serde(default = "default_use_proxy")]
    pub use_proxy: bool,
}

fn default_gateway_exposure_mode() -> String {
    "local".to_string()
}

fn default_cloudflare_mode() -> String {
    "quick".to_string()
}

fn default_use_proxy() -> bool {
    true
}

impl Default for GatewayExposureConfig {
    fn default() -> Self {
        Self {
            mode: default_gateway_exposure_mode(),
            frp_profile_id: String::new(),
            frp_server: String::new(),
            frp_server_port: default_frp_server_port(),
            frp_subdomain: String::new(),
            cloudflare_mode: default_cloudflare_mode(),
            use_proxy: default_use_proxy(),
        }
    }
}

fn default_admin_bind_host() -> String {
    "127.0.0.1".to_string()
}

fn default_admin_port() -> u16 {
    28767
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            bind_host: default_admin_bind_host(),
            local_port: default_admin_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    /// Interface address for the single multi-workspace MCP gateway.
    #[serde(default = "default_gateway_bind_host")]
    pub bind_host: String,
    /// Local port used by the single ChatGPT connector endpoint.
    #[serde(default = "default_gateway_port")]
    pub local_port: u16,
    /// Optional externally reachable HTTPS base URL, without a trailing `/mcp`.
    #[serde(default)]
    pub public_url: String,
    /// `oauth`, `bearer`, or `noauth`. Gateway auth always uses shared secrets.
    #[serde(default = "default_gateway_auth_type")]
    pub auth_type: String,
    /// Stable tool catalog exposed by the gateway. Workspace policies still
    /// control what each selected workspace is allowed to execute.
    #[serde(default = "default_gateway_tool_profile")]
    pub tool_profile: String,
    /// When only one workspace exists, allow requests to use it before an
    /// explicit `select_workspace` call. With 2+ workspaces selection is
    /// always required to prevent accidental cross-project operations.
    #[serde(default = "default_gateway_auto_select_single")]
    pub auto_select_single_workspace: bool,
}

fn default_gateway_bind_host() -> String {
    "127.0.0.1".to_string()
}

fn default_gateway_port() -> u16 {
    28766
}

fn default_gateway_auth_type() -> String {
    "oauth".to_string()
}

fn default_gateway_tool_profile() -> String {
    "core".to_string()
}

fn default_gateway_auto_select_single() -> bool {
    true
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind_host: default_gateway_bind_host(),
            local_port: default_gateway_port(),
            public_url: String::new(),
            auth_type: default_gateway_auth_type(),
            tool_profile: default_gateway_tool_profile(),
            auto_select_single_workspace: default_gateway_auto_select_single(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrpProfile {
    pub id: String,
    pub name: String,
    pub server: String,
    #[serde(default = "default_frp_server_port", alias = "serverPort")]
    pub server_port: u16,
}

/// Download settings for fetching frpc / cloudflared binaries.
///
/// GitHub is slow/unreliable from some networks, so downloads try a mirror
/// prefix first (ghproxy-style: `{mirror}/{full_github_url}`) and fall back to
/// the direct GitHub URL. An optional proxy can be layered on top.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadConfig {
    /// Mirror prefix applied before the full GitHub URL. Empty = direct.
    #[serde(default = "default_github_mirror")]
    pub github_mirror: String,
    /// "none" (no proxy) | "system" (env HTTP(S)_PROXY) | "manual".
    #[serde(default = "default_proxy_mode")]
    pub proxy_mode: String,
    /// Proxy URL used when `proxy_mode == "manual"` (e.g. http://127.0.0.1:7890).
    #[serde(default)]
    pub proxy_url: String,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            github_mirror: default_github_mirror(),
            proxy_mode: default_proxy_mode(),
            proxy_url: String::new(),
        }
    }
}

/// Global outbound proxy used by network-facing operations such as the
/// Cloudflare quick tunnel. Binary downloads use `download.proxy` separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    /// "none" (no proxy) | "system" (env HTTP(S)_PROXY) | "manual".
    #[serde(default = "default_proxy_mode")]
    pub mode: String,
    /// Proxy URL used when `mode == "manual"` (e.g. http://127.0.0.1:7890).
    #[serde(default)]
    pub url: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            mode: default_proxy_mode(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub frp_profiles: Vec<FrpProfile>,
    #[serde(default)]
    pub last_workspace_id: String,
    #[serde(default)]
    pub download: DownloadConfig,
    /// Global outbound proxy (Cloudflare tunnel, etc.).
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Global single-URL MCP gateway. This is independent from legacy
    /// per-workspace MCP listener settings.
    #[serde(default)]
    pub gateway: GatewayConfig,
    /// How the Gateway's canonical public URL is exposed. This deliberately
    /// stays separate from `gateway.public_url`: the URL is identity, while
    /// this configuration only describes transport/routing.
    #[serde(default)]
    pub gateway_exposure: GatewayExposureConfig,
    /// Local Web management plane. Kept separate from the public MCP data
    /// plane so the admin API does not need to be exposed to ChatGPT.
    #[serde(default)]
    pub admin: AdminConfig,
    /// Shared secrets indexed by key name (e.g. "bearer_token").
    /// Persisted alongside other app settings in app_settings.json.
    #[serde(default)]
    pub shared_secrets: HashMap<String, String>,
    /// Per-workspace secrets: workspace_id -> secret_key -> value.
    #[serde(default)]
    pub workspace_secrets: HashMap<String, HashMap<String, String>>,
    /// App-scoped secrets: scope -> item_id -> value (e.g. frp profile tokens).
    #[serde(default)]
    pub app_secrets: HashMap<String, HashMap<String, String>>,
}

fn default_frp_server_port() -> u16 {
    7000
}

fn default_github_mirror() -> String {
    "https://gh-proxy.com".to_string()
}

fn default_proxy_mode() -> String {
    "system".to_string()
}

impl AppSettings {
    pub fn from_data(data: &AppData) -> Self {
        Self {
            frp_profiles: data.frp_profiles.clone(),
            last_workspace_id: data.last_workspace_id.clone(),
            download: data.download.clone(),
            proxy: data.proxy.clone(),
            gateway: data.gateway.clone(),
            gateway_exposure: data.gateway_exposure.clone(),
            admin: data.admin.clone(),
            shared_secrets: data.shared_secrets.clone(),
            workspace_secrets: data.workspace_secrets.clone(),
            app_secrets: data.app_secrets.clone(),
        }
    }

    pub fn apply_to(&self, data: &mut AppData) {
        data.frp_profiles = self.frp_profiles.clone();
        data.last_workspace_id = self.last_workspace_id.clone();
        data.download = self.download.clone();
        data.proxy = self.proxy.clone();
        data.gateway = self.gateway.clone();
        data.gateway_exposure = self.gateway_exposure.clone();
        data.admin = self.admin.clone();
        data.shared_secrets = self.shared_secrets.clone();
        data.workspace_secrets = self.workspace_secrets.clone();
        data.app_secrets = self.app_secrets.clone();
    }

    pub fn load_or_default() -> Self {
        crate::data::DataStore::read_file(|data| Ok(Self::from_data(data)))
            .unwrap_or_default()
    }

    pub fn find_frp_profile(&self, id: &str) -> Option<&FrpProfile> {
        if id.trim().is_empty() {
            return None;
        }
        self.frp_profiles.iter().find(|profile| profile.id == id)
    }
}

#[allow(dead_code)]
impl FrpProfile {
    pub fn new(name: String, server: String, server_port: u16) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string().replace('-', ""),
            name,
            server: server.trim().to_string(),
            server_port,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrpProfile;

    #[test]
    fn accepts_frontend_camel_case_server_port() {
        let profile: FrpProfile = serde_json::from_value(serde_json::json!({
            "id": "p1",
            "name": "公司 FRP",
            "server": "frp.example.com",
            "serverPort": 7004
        }))
        .expect("FRP profile should deserialize");

        assert_eq!(profile.server_port, 7004);
    }

    #[test]
    fn keeps_legacy_snake_case_server_port_compatible() {
        let profile: FrpProfile = serde_json::from_value(serde_json::json!({
            "id": "p1",
            "name": "公司 FRP",
            "server": "frp.example.com",
            "server_port": 7005
        }))
        .expect("legacy FRP profile should deserialize");

        assert_eq!(profile.server_port, 7005);
    }
}
