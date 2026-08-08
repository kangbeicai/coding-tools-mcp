use serde::Serialize;
use tokio::process::Child;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::secret::SecretStore;
use crate::settings::{GatewayConfig, GatewayExposureConfig};
use crate::tunnel::{
    clear_managed_frpc_pid, gateway_frp_server_config, log_dir_for_profile,
    spawn_cloudflare_tunnel, spawn_frpc_config, stop_child, stop_recorded_frpc_instance,
};

const GATEWAY_EXPOSURE_ID: &str = "gateway-exposure";

pub struct GatewayExposureProcess {
    pub mode: String,
    pub effective_public_url: String,
    pub pid: Option<u32>,
    pub child: Child,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayExposureStatusDto {
    pub state: String,
    pub mode: String,
    pub managed: bool,
    pub canonical_public_url: String,
    pub effective_public_url: String,
    pub pid: Option<u32>,
    pub message: String,
}

pub fn get_gateway_exposure_config(state: &AppState) -> AppResult<GatewayExposureConfig> {
    state.with_settings(|store| Ok(store.settings().gateway_exposure))
}

pub fn set_gateway_exposure_config(
    state: &AppState,
    exposure: GatewayExposureConfig,
) -> AppResult<()> {
    if state.with_gateway_exposure(|process| Ok(process.is_some()))? {
        return Err(AppError::Message(
            "请先停止 Gateway 公网暴露，再修改 Public Access 配置。".into(),
        ));
    }
    validate_mode(&exposure.mode)?;
    state.with_settings(|store| {
        let mut settings = store.settings();
        settings.gateway_exposure = exposure;
        store.update_settings(settings)
    })
}

pub fn gateway_exposure_status(state: &AppState) -> AppResult<GatewayExposureStatusDto> {
    let (gateway, exposure) = state.with_settings(|store| {
        let settings = store.settings();
        Ok((settings.gateway, settings.gateway_exposure))
    })?;
    let canonical = normalize_public_origin(&gateway.public_url)?;
    let mode = normalized_mode(&exposure.mode);
    state.with_gateway_exposure(|process| {
        if let Some(process) = process.as_mut() {
            let running = process.child.try_wait()?.is_none();
            return Ok(GatewayExposureStatusDto {
                state: if running { "running" } else { "error" }.into(),
                mode: process.mode.clone(),
                managed: true,
                canonical_public_url: canonical,
                effective_public_url: process.effective_public_url.clone(),
                pid: process.pid,
                message: if running {
                    "Coding Tools 正在管理公网暴露进程。".into()
                } else {
                    "公网暴露进程已退出，请检查日志。".into()
                },
            });
        }

        let (state_name, managed, effective, message) = match mode.as_str() {
            "local" => (
                "local",
                false,
                String::new(),
                "仅本机访问，不启动公网暴露进程。".into(),
            ),
            "direct" => (
                "configured",
                false,
                canonical.clone(),
                "由 Gateway 监听地址、端口映射或公网网络直接提供访问。".into(),
            ),
            "external" => (
                "configured",
                false,
                canonical.clone(),
                "公网路由由外部 Nginx/Caddy/VPS/其他设施管理。".into(),
            ),
            "frp" | "cloudflare" => (
                "stopped",
                true,
                String::new(),
                "Managed exposure 尚未启动。".into(),
            ),
            _ => unreachable!("mode validated by normalized_mode"),
        };
        Ok(GatewayExposureStatusDto {
            state: state_name.into(),
            mode,
            managed,
            canonical_public_url: canonical,
            effective_public_url: effective,
            pid: None,
            message,
        })
    })
}

pub async fn start_gateway_exposure_service(
    state: &AppState,
) -> AppResult<GatewayExposureStatusDto> {
    if state.with_gateway_exposure(|process| Ok(process.is_some()))? {
        return gateway_exposure_status(state);
    }
    let gateway_running = state.with_gateway(|process| {
        Ok(process
            .as_ref()
            .is_some_and(|process| !process.handle.is_finished()))
    })?;
    let settings = state.with_settings(|store| Ok(store.settings()))?;
    let gateway = settings.gateway.clone();
    let exposure = settings.gateway_exposure.clone();
    let mode = normalized_mode(&exposure.mode);
    validate_mode(&mode)?;

    if matches!(mode.as_str(), "local" | "direct" | "external") {
        return gateway_exposure_status(state);
    }
    if !gateway_running {
        return Err(AppError::Message(
            "请先启动全局 MCP Gateway，再启动 managed public exposure。".into(),
        ));
    }
    if !gateway_reachable_on_loopback(&gateway) {
        return Err(AppError::Message(
            "FRP/Cloudflare 由本机 127.0.0.1 回连 Gateway；请将 Gateway 监听地址设为 127.0.0.1、0.0.0.0、::1 或 ::。".into(),
        ));
    }

    let canonical = normalize_public_origin(&gateway.public_url)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let log_dir = log_dir_for_profile("gateway");
    std::fs::create_dir_all(&log_dir)?;

    let process = match mode.as_str() {
        "frp" => {
            require_https_canonical(&canonical, "FRP")?;
            if exposure.frp_subdomain.trim().is_empty() {
                return Err(AppError::Message("Gateway FRP 需要配置子域名。".into()));
            }
            let inline_token = if exposure.frp_profile_id.trim().is_empty() {
                SecretStore::get_shared("gateway_frp_token")?
                    .filter(|value| !value.trim().is_empty())
            } else {
                None
            };
            let config = gateway_frp_server_config(
                &gateway,
                &exposure,
                &settings,
                inline_token,
            );
            // A hard-killed server can leave its managed frpc child alive.
            // Reclaim only the PID/image previously recorded for this dedicated
            // Gateway exposure instance before replacing the record.
            let _ = stop_recorded_frpc_instance(GATEWAY_EXPOSURE_ID).await?;
            let handle = spawn_frpc_config(
                GATEWAY_EXPOSURE_ID,
                &cwd,
                &config,
                &settings,
                exposure.use_proxy,
                &log_dir.join("frpc-gateway.log"),
            )
            .await?;
            GatewayExposureProcess {
                mode,
                effective_public_url: canonical,
                pid: handle.pid,
                child: handle.child,
            }
        }
        "cloudflare" => {
            let cloudflare_mode = exposure.cloudflare_mode.trim().to_ascii_lowercase();
            if !matches!(cloudflare_mode.as_str(), "quick" | "named") {
                return Err(AppError::Message(
                    "Gateway Cloudflare 模式仅支持 quick 或 named。".into(),
                ));
            }
            if cloudflare_mode == "named" {
                require_https_canonical(&canonical, "Cloudflare named tunnel")?;
            }
            let token = SecretStore::get_shared("gateway_cloudflare_token")?
                .unwrap_or_default();
            let handle = spawn_cloudflare_tunnel(
                gateway.local_port,
                &cwd,
                &log_dir.join("cloudflare-gateway.log"),
                &cloudflare_mode,
                &token,
                &canonical,
                exposure.use_proxy,
            )
            .await?;
            GatewayExposureProcess {
                mode,
                effective_public_url: handle.public_url,
                pid: handle.pid,
                child: handle.child,
            }
        }
        _ => unreachable!(),
    };

    state.with_gateway_exposure(|slot| {
        *slot = Some(process);
        Ok(())
    })?;
    gateway_exposure_status(state)
}

pub async fn stop_gateway_exposure_service(
    state: &AppState,
) -> AppResult<GatewayExposureStatusDto> {
    let process = state.with_gateway_exposure(|slot| Ok(slot.take()))?;
    if let Some(process) = process {
        let mode = process.mode.clone();
        let _ = stop_child(process.child, process.pid).await;
        if mode == "frp" {
            clear_managed_frpc_pid(GATEWAY_EXPOSURE_ID);
        }
    }
    gateway_exposure_status(state)
}

pub fn normalize_public_origin(value: &str) -> AppResult<String> {
    let raw = value.trim().trim_end_matches('/');
    let raw = raw.strip_suffix("/mcp").unwrap_or(raw).trim_end_matches('/');
    if raw.is_empty() {
        return Ok(String::new());
    }
    let parsed = reqwest::Url::parse(raw)
        .map_err(|error| AppError::Message(format!("公网 URL 无效: {error}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::Message("公网 URL 只支持 http:// 或 https://。".into()));
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AppError::Message(
            "公网 URL 必须是 origin/base URL，例如 https://mcp.example.com；不要包含额外路径、查询参数或片段。".into(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::Message("公网 URL 不能包含用户名或密码。".into()));
    }
    Ok(raw.to_string())
}

fn require_https_canonical(canonical: &str, label: &str) -> AppResult<()> {
    if canonical.is_empty() {
        return Err(AppError::Message(format!(
            "{label} 需要先配置 Gateway 公网 URL；该 URL 是 canonical identity，不会从 tunnel 配置自动推导。"
        )));
    }
    if !canonical.to_ascii_lowercase().starts_with("https://") {
        return Err(AppError::Message(format!(
            "{label} 面向 ChatGPT 时需要 HTTPS 公网 URL。"
        )));
    }
    Ok(())
}

fn normalized_mode(value: &str) -> String {
    let mode = value.trim().to_ascii_lowercase();
    if mode.is_empty() {
        "local".into()
    } else {
        mode
    }
}

fn validate_mode(value: &str) -> AppResult<()> {
    match normalized_mode(value).as_str() {
        "local" | "direct" | "external" | "frp" | "cloudflare" => Ok(()),
        other => Err(AppError::Message(format!(
            "不支持的 Gateway public access mode: {other}"
        ))),
    }
}

fn gateway_reachable_on_loopback(gateway: &GatewayConfig) -> bool {
    gateway
        .bind_host
        .trim()
        .parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback() || ip.is_unspecified())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_public_url_accepts_origin_or_mcp_endpoint() {
        assert_eq!(
            normalize_public_origin("https://mcp.example.com/mcp").unwrap(),
            "https://mcp.example.com"
        );
        assert_eq!(
            normalize_public_origin("https://mcp.example.com/").unwrap(),
            "https://mcp.example.com"
        );
    }

    #[test]
    fn canonical_public_url_rejects_extra_paths() {
        assert!(normalize_public_origin("https://example.com/api/mcp").is_err());
    }

    #[test]
    fn managed_exposure_requires_loopback_reachable_bind() {
        let mut gateway = GatewayConfig::default();
        gateway.bind_host = "192.168.1.10".into();
        assert!(!gateway_reachable_on_loopback(&gateway));
        gateway.bind_host = "0.0.0.0".into();
        assert!(gateway_reachable_on_loopback(&gateway));
    }
}
