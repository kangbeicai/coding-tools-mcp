use serde::Serialize;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::settings::GatewayConfig;

use super::exposure::{
    normalize_public_origin, start_gateway_exposure_service, stop_gateway_exposure_service,
};
use super::listener::spawn_listener;
use super::state::GatewaySessionInfo;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatusDto {
    pub state: String,
    pub local_endpoint: String,
    pub public_endpoint: String,
    pub workspace_count: usize,
    pub session_count: usize,
    pub sessions: Vec<GatewaySessionInfo>,
}

pub fn get_gateway_config(state: &AppState) -> AppResult<GatewayConfig> {
    state.with_settings(|store| Ok(store.settings().gateway))
}

pub fn set_gateway_config(state: &AppState, mut gateway: GatewayConfig) -> AppResult<()> {
    let running = state.with_gateway(|process| Ok(process.is_some()))?;
    if running {
        return Err(AppError::Message(
            "请先停止全局 Gateway，再修改监听地址、端口或认证配置。".into(),
        ));
    }
    gateway.public_url = normalize_public_origin(&gateway.public_url)?;
    state.with_settings(|store| {
        let mut settings = store.settings();
        settings.gateway = gateway;
        store.update_settings(settings)
    })
}

pub async fn start_gateway_service(state: &AppState) -> AppResult<GatewayStatusDto> {
    if state.with_gateway(|process| Ok(process.is_some()))? {
        return gateway_status(state);
    }
    let (config, profiles) = state.with_data(|store| {
        store.init_shared_secrets()?;
        Ok((store.settings().gateway, store.list().to_vec()))
    })?;
    let process = spawn_listener(config, profiles).map_err(AppError::Message)?;
    state.with_gateway(|slot| {
        *slot = Some(process);
        Ok(())
    })?;
    gateway_status(state)
}

pub async fn stop_gateway_service(state: &AppState) -> AppResult<GatewayStatusDto> {
    let _ = stop_gateway_exposure_service(state).await?;
    let process = state.with_gateway(|slot| Ok(slot.take()))?;
    if let Some(process) = process {
        let _ = process.shutdown.send(());
        let _ = process.handle.await;
    }
    gateway_status(state)
}

pub async fn restart_gateway_service(state: &AppState) -> AppResult<GatewayStatusDto> {
    let exposure_was_running =
        state.with_gateway_exposure(|process| Ok(process.is_some()))?;
    let _ = stop_gateway_service(state).await?;
    let status = start_gateway_service(state).await?;
    if exposure_was_running {
        let _ = start_gateway_exposure_service(state).await?;
    }
    Ok(status)
}

pub fn gateway_status(state: &AppState) -> AppResult<GatewayStatusDto> {
    let config = get_gateway_config(state)?;
    let registered_workspace_count =
        state.with_workspaces(|store| Ok(store.list().len()))?;
    state.with_gateway(|process| {
        let Some(process) = process.as_ref() else {
            return Ok(GatewayStatusDto {
                state: "stopped".into(),
                local_endpoint: local_endpoint(&config),
                public_endpoint: public_endpoint(&config),
                workspace_count: registered_workspace_count,
                session_count: 0,
                sessions: Vec::new(),
            });
        };
        let running = !process.handle.is_finished();
        Ok(GatewayStatusDto {
            state: if running { "running" } else { "error" }.into(),
            local_endpoint: process.local_endpoint.clone(),
            public_endpoint: public_endpoint(&config),
            workspace_count: process.state.workspace_count(),
            session_count: process.state.session_count(),
            sessions: process.state.list_sessions(),
        })
    })
}

fn local_endpoint(config: &GatewayConfig) -> String {
    let raw = config.bind_host.trim();
    let host = match raw.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) if ip.is_unspecified() => "127.0.0.1".to_string(),
        Ok(std::net::IpAddr::V6(ip)) if ip.is_unspecified() => "[::1]".to_string(),
        Ok(std::net::IpAddr::V6(ip)) => format!("[{ip}]"),
        Ok(std::net::IpAddr::V4(ip)) => ip.to_string(),
        Err(_) if raw.is_empty() => "127.0.0.1".to_string(),
        Err(_) => raw.to_string(),
    };
    format!("http://{}:{}/mcp", host, config.local_port)
}

fn public_endpoint(config: &GatewayConfig) -> String {
    let base = normalize_public_origin(&config.public_url).unwrap_or_default();
    if base.is_empty() {
        String::new()
    } else if base.ends_with("/mcp") {
        base.to_string()
    } else {
        format!("{base}/mcp")
    }
}
