use serde::Deserialize;
use serde_json::{json, Value};

use crate::app_state::{bootstrap_workspace, teardown_workspace, AppState};
use crate::error::{AppError, AppResult};
use crate::gateway::{
    gateway_exposure_status, gateway_status, get_gateway_config, get_gateway_exposure_config,
    restart_gateway_service, set_gateway_config, set_gateway_exposure_config,
    start_gateway_exposure_service, start_gateway_service, stop_gateway_exposure_service,
    stop_gateway_service,
};
use crate::health::run_gateway_health_checks;
use crate::tunnel::drop_workspace as drop_tunnel_workspace;
use crate::workspace::resources::{
    assign_free_workspace_ports, validate_workspace_resources_update,
};
use crate::workspace::WorkspaceProfile;

const SHARED_KEYS: &[&str] = &[
    "oauth_client_id",
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
    "actions_api_key",
    "actions_oauth_client_secret",
    "actions_oauth_password",
    "actions_oauth_token_secret",
    "gateway_frp_token",
    "gateway_cloudflare_token",
];

const GATEWAY_SHARED_KEYS: &[&str] = &[
    "oauth_client_id",
    "bearer_token",
    "oauth_client_secret",
    "oauth_password",
    "oauth_token_secret",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcRequest {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

fn gateway_is_running(state: &AppState) -> AppResult<bool> {
    state.with_gateway(|process| {
        Ok(process
            .as_ref()
            .is_some_and(|process| !process.handle.is_finished()))
    })
}

fn shared_key<'a>(args: &'a Value) -> AppResult<&'a str> {
    let key = args
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Message("缺少参数: key".into()))?;
    if SHARED_KEYS.contains(&key) {
        Ok(key)
    } else {
        Err(AppError::Message(format!("invalid shared key: {key}")))
    }
}

pub async fn dispatch(state: &AppState, request: RpcRequest) -> Result<Value, String> {
    dispatch_inner(state, request)
        .await
        .map_err(|error| error.to_string())
}

async fn dispatch_inner(state: &AppState, request: RpcRequest) -> AppResult<Value> {
    match request.command.as_str() {
        "list_workspaces" => state.with_workspaces(|store| serde_value(store.list())),
        "create_workspace" => {
            let path = arg_str(&request.args, "path")?;
            let name = request
                .args
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string);
            state.with_workspaces(|store| {
                let mut profile = WorkspaceProfile::new(path, name);
                assign_free_workspace_ports(store.list(), &mut profile)?;
                bootstrap_workspace(store, &profile.id)?;
                store.add(profile.clone())?;
                serde_value(profile)
            })
        }
        "update_workspace" => {
            ensure_gateway_stopped(state)?;
            let profile: WorkspaceProfile = serde_json::from_value(
                request
                    .args
                    .get("profile")
                    .cloned()
                    .ok_or_else(|| AppError::Message("缺少 profile".into()))?,
            )?;
            state.with_workspaces(|store| {
                let current = store.get(&profile.id).cloned().ok_or_else(|| {
                    AppError::Message(format!("workspace not found: {}", profile.id))
                })?;
                validate_workspace_resources_update(store.list(), &current, &profile)?;
                store.update(profile)?;
                Ok(Value::Null)
            })
        }
        "delete_workspace" => {
            ensure_gateway_stopped(state)?;
            let id = arg_str(&request.args, "id")?;
            let profile = state.with_workspaces(|store| {
                store
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
            })?;
            drop_tunnel_workspace(&id).await?;
            state.with_runtime(|runtime| {
                runtime.drop_workspace(&profile);
                Ok(())
            })?;
            state.with_workspaces(|store| {
                if store.remove(&id)?.is_some() {
                    teardown_workspace(store, &id)?;
                }
                Ok(Value::Null)
            })
        }
        "get_runtime_status" => runtime_status(state, &request.args, false),
        "get_actions_runtime_status" => runtime_status(state, &request.args, true),
        "get_last_workspace_id" => {
            state.with_settings(|store| serde_value(store.settings().last_workspace_id))
        }
        "set_last_workspace" => {
            let id = arg_str(&request.args, "id")?;
            state.with_settings(|store| {
                let mut settings = store.settings();
                settings.last_workspace_id = id;
                store.update_settings(settings)?;
                Ok(Value::Null)
            })
        }
        "get_gateway_config" => serde_value(get_gateway_config(state)?),
        "set_gateway_config" => {
            let gateway = serde_json::from_value(
                request
                    .args
                    .get("gateway")
                    .cloned()
                    .ok_or_else(|| AppError::Message("缺少 gateway".into()))?,
            )?;
            set_gateway_config(state, gateway)?;
            Ok(Value::Null)
        }
        "get_gateway_exposure" => serde_value(get_gateway_exposure_config(state)?),
        "set_gateway_exposure" => {
            let exposure = serde_json::from_value(
                request
                    .args
                    .get("exposure")
                    .cloned()
                    .ok_or_else(|| AppError::Message("缺少 exposure".into()))?,
            )?;
            set_gateway_exposure_config(state, exposure)?;
            Ok(Value::Null)
        }
        "get_gateway_exposure_status" => serde_value(gateway_exposure_status(state)?),
        "start_gateway_exposure" => serde_value(start_gateway_exposure_service(state).await?),
        "stop_gateway_exposure" => serde_value(stop_gateway_exposure_service(state).await?),
        "get_gateway_status" => serde_value(gateway_status(state)?),
        "run_gateway_health_checks" => serde_value(run_gateway_health_checks(state).await?),
        "clear_gateway_session" => {
            let session_key = arg_str(&request.args, "sessionKey")?;
            let removed = state.with_gateway(|process| {
                let Some(process) = process.as_ref() else {
                    return Err(AppError::Message("Gateway 当前未运行。".into()));
                };
                process
                    .state
                    .clear_session(&session_key)
                    .map_err(AppError::Message)
            })?;
            serde_value(json!({ "removed": removed }))
        }
        "start_gateway" => serde_value(start_gateway_service(state).await?),
        "stop_gateway" => serde_value(stop_gateway_service(state).await?),
        "restart_gateway" => serde_value(restart_gateway_service(state).await?),
        "get_admin_config" => state.with_settings(|store| serde_value(store.settings().admin)),
        "list_frp_profiles" => state.with_settings(|store| {
            let profiles: Vec<Value> = store
                .data()
                .frp_profiles
                .iter()
                .map(|profile| {
                    json!({
                        "id": profile.id,
                        "name": profile.name,
                        "server": profile.server,
                        "serverPort": profile.server_port,
                        "hasToken": store
                            .get_app_secret("frp_profile_token", &profile.id)
                            .is_some_and(|value| !value.trim().is_empty())
                    })
                })
                .collect();
            Ok(Value::Array(profiles))
        }),
        "get_shared_secret" => {
            let key = shared_key(&request.args)?;
            state.with_data(|store| serde_value(store.get_shared_secret(key)))
        }
        "set_shared_secret" => {
            let key = shared_key(&request.args)?.to_string();
            let value = arg_str(&request.args, "value")?;
            let changed = state.with_data(|store| {
                if store.get_shared_secret(&key).as_deref() == Some(value.as_str()) {
                    return Ok(false);
                }
                store.set_shared_secret(&key, &value)?;
                Ok(true)
            })?;
            if changed && GATEWAY_SHARED_KEYS.contains(&key.as_str()) && gateway_is_running(state)?
            {
                let _ = restart_gateway_service(state).await?;
            }
            Ok(Value::Null)
        }
        "regenerate_shared_secret" => {
            let key = shared_key(&request.args)?.to_string();
            let value = state.with_data(|store| store.regenerate_shared_secret(&key))?;
            if GATEWAY_SHARED_KEYS.contains(&key.as_str()) && gateway_is_running(state)? {
                let _ = restart_gateway_service(state).await?;
            }
            serde_value(value)
        }
        other => Err(AppError::Message(format!(
            "Web Admin 尚未迁移命令: {other}"
        ))),
    }
}

fn runtime_status(state: &AppState, args: &Value, actions: bool) -> AppResult<Value> {
    let id = arg_str(args, "id")?;
    let profile = state.with_workspaces(|store| {
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })?;
    state.with_runtime(|runtime| {
        let status = if actions {
            runtime.actions_status(&profile)
        } else {
            runtime.mcp_status(&profile)
        };
        serde_value(status)
    })
}

fn ensure_gateway_stopped(state: &AppState) -> AppResult<()> {
    if state.with_gateway(|process| Ok(process.is_some()))? {
        return Err(AppError::Message(
            "全局 Gateway 运行时不能修改工作区定义；请先停止 Gateway。".into(),
        ));
    }
    Ok(())
}

fn arg_str(args: &Value, key: &str) -> AppResult<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::Message(format!("缺少参数: {key}")))
}

fn serde_value<T: serde::Serialize>(value: T) -> AppResult<Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn success(value: Value) -> Value {
    json!({ "ok": true, "result": value })
}

pub fn failure(message: String) -> Value {
    json!({ "ok": false, "error": message })
}
