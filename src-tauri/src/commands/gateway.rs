use tauri::State;

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::gateway::{
    gateway_exposure_status, gateway_status,
    get_gateway_config as get_gateway_config_service, get_gateway_exposure_config,
    restart_gateway_service, set_gateway_config as set_gateway_config_service,
    set_gateway_exposure_config, start_gateway_exposure_service, start_gateway_service,
    stop_gateway_exposure_service, stop_gateway_service, GatewayExposureStatusDto, GatewayStatusDto,
};
use crate::settings::{GatewayConfig, GatewayExposureConfig};
use crate::health::{run_gateway_health_checks as execute_gateway_health_checks, GatewayHealthReport};
use serde_json::json;

#[tauri::command]
pub fn get_gateway_config(state: State<'_, AppState>) -> AppResult<GatewayConfig> {
    get_gateway_config_service(&state)
}

#[tauri::command]
pub fn set_gateway_config(state: State<'_, AppState>, gateway: GatewayConfig) -> AppResult<()> {
    set_gateway_config_service(&state, gateway)
}

#[tauri::command]
pub fn get_gateway_exposure(
    state: State<'_, AppState>,
) -> AppResult<GatewayExposureConfig> {
    get_gateway_exposure_config(&state)
}

#[tauri::command]
pub fn set_gateway_exposure(
    state: State<'_, AppState>,
    exposure: GatewayExposureConfig,
) -> AppResult<()> {
    set_gateway_exposure_config(&state, exposure)
}

#[tauri::command]
pub fn get_gateway_exposure_status(
    state: State<'_, AppState>,
) -> AppResult<GatewayExposureStatusDto> {
    gateway_exposure_status(&state)
}

#[tauri::command]
pub async fn start_gateway_exposure(
    state: State<'_, AppState>,
) -> AppResult<GatewayExposureStatusDto> {
    start_gateway_exposure_service(&state).await
}

#[tauri::command]
pub async fn stop_gateway_exposure(
    state: State<'_, AppState>,
) -> AppResult<GatewayExposureStatusDto> {
    stop_gateway_exposure_service(&state).await
}

#[tauri::command]
pub async fn start_gateway(state: State<'_, AppState>) -> AppResult<GatewayStatusDto> {
    start_gateway_service(&state).await
}

#[tauri::command]
pub async fn stop_gateway(state: State<'_, AppState>) -> AppResult<GatewayStatusDto> {
    stop_gateway_service(&state).await
}

#[tauri::command]
pub async fn restart_gateway(state: State<'_, AppState>) -> AppResult<GatewayStatusDto> {
    restart_gateway_service(&state).await
}

#[tauri::command]
pub fn get_gateway_status(state: State<'_, AppState>) -> AppResult<GatewayStatusDto> {
    gateway_status(&state)
}

#[tauri::command]
pub async fn run_gateway_health_checks(
    state: State<'_, AppState>,
) -> AppResult<GatewayHealthReport> {
    execute_gateway_health_checks(&state).await
}

#[tauri::command]
pub fn clear_gateway_session(
    state: State<'_, AppState>,
    session_key: String,
) -> AppResult<serde_json::Value> {
    let removed = state.with_gateway(|process| {
        let Some(process) = process.as_ref() else {
            return Err(crate::error::AppError::Message("Gateway 当前未运行。".into()));
        };
        process
            .state
            .clear_session(&session_key)
            .map_err(crate::error::AppError::Message)
    })?;
    Ok(json!({ "removed": removed }))
}
