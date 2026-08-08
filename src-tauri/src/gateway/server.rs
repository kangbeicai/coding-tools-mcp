use serde_json::{json, Value};

use crate::gateway::state::SharedGatewayState;
use crate::mcp::server::handle_request as handle_workspace_request;
use crate::tools::{list_tools_for_profile, wrap_mcp_tool_result};

const LIST_WORKSPACES: &str = "list_workspaces";
const SELECT_WORKSPACE: &str = "select_workspace";
const GET_CURRENT_WORKSPACE: &str = "get_current_workspace";

pub fn handle_request(
    state: &SharedGatewayState,
    body: &Value,
    transport_session_key: Option<&str>,
    forced_workspace: Option<&str>,
) -> Value {
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(Value::Null);

    if id.is_null() && method.starts_with("notifications/") {
        return Value::Null;
    }

    let result = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": gateway_tool_catalog(state) })),
        "tools/call" => handle_tools_call(state, body, transport_session_key, forced_workspace),
        _ => Err(json!({
            "code": -32601,
            "message": format!("Method not found: {method}")
        })),
    };

    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {
            "tools": { "listChanged": false },
            "logging": {}
        },
        "serverInfo": {
            "name": "coding-tools-gateway",
            "title": "Coding Tools Gateway",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "This is one multi-workspace Coding Tools MCP gateway. Do not create one ChatGPT connector per project. At the start of every new ChatGPT conversation, call list_workspaces and select_workspace before using project tools when more than one workspace exists. After workspace selection, call history_session_bootstrap exactly once before the first project action. Keep that workspace selected for the conversation unless the user explicitly asks to switch projects. After each completed user task call history_session_checkpoint with the session_key/current_path returned by bootstrap. Workspace path routes under /w/<workspace-id>/mcp are for explicit routing/debugging and are not separate ChatGPT plugins."
    })
}

fn handle_tools_call(
    state: &SharedGatewayState,
    body: &Value,
    transport_session_key: Option<&str>,
    forced_workspace: Option<&str>,
) -> Result<Value, Value> {
    let params = body.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| json!({ "code": -32602, "message": "Missing tool name" }))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let session_key = request_session_key(body).or(transport_session_key);

    match name {
        LIST_WORKSPACES => {
            let workspaces = state.list_workspaces();
            Ok(wrap_mcp_tool_result(
                name,
                &args,
                json!({
                    "ok": true,
                    "workspaces": workspaces,
                    "count": workspaces.len(),
                    "selection_required": workspaces.len() > 1
                }),
            ))
        }
        SELECT_WORKSPACE => {
            if forced_workspace.is_some() {
                return Ok(wrap_mcp_tool_result(
                    name,
                    &args,
                    json!({
                        "ok": false,
                        "error": {
                            "code": "path_workspace_locked",
                            "message": "当前请求已通过 /w/<workspace-id>/mcp 固定工作区，不能在该路径内切换。"
                        }
                    }),
                ));
            }
            let Some(session_key) = session_key else {
                return Ok(gateway_error_result(
                    name,
                    &args,
                    "session_key_missing",
                    "客户端没有提供会话标识；ChatGPT 会话使用 _meta.openai/session，其他 MCP 客户端可使用 Mcp-Session-Id。",
                    state,
                ));
            };
            let selector = args
                .get("workspace")
                .and_then(Value::as_str)
                .unwrap_or("");
            match state.select_workspace(session_key, selector) {
                Ok(workspace) => Ok(wrap_mcp_tool_result(
                    name,
                    &args,
                    json!({
                        "ok": true,
                        "workspace": workspace,
                        "next_action": "Call history_session_bootstrap before project work in a new conversation."
                    }),
                )),
                Err(message) => Ok(gateway_error_result(
                    name,
                    &args,
                    "workspace_selection_failed",
                    &message,
                    state,
                )),
            }
        }
        GET_CURRENT_WORKSPACE => match state.current_workspace(session_key, forced_workspace) {
            Ok(workspace) => Ok(wrap_mcp_tool_result(
                name,
                &args,
                json!({ "ok": true, "workspace": workspace }),
            )),
            Err(message) => Ok(gateway_error_result(
                name,
                &args,
                "workspace_required",
                &message,
                state,
            )),
        },
        _ => {
            let context = match state.context_for_request(session_key, forced_workspace) {
                Ok(context) => context,
                Err(message) => {
                    return Ok(gateway_error_result(
                        name,
                        &args,
                        "workspace_required",
                        &message,
                        state,
                    ));
                }
            };

            // `history_session_*` already understands _meta.openai/session.
            // When a generic MCP client only gives us Mcp-Session-Id, mirror it
            // into the same metadata slot before dispatching to the workspace.
            let forwarded = inject_session_metadata(body, session_key);
            let response = handle_workspace_request(&context, &forwarded);
            Ok(response.get("result").cloned().unwrap_or_else(|| {
                wrap_mcp_tool_result(
                    name,
                    &args,
                    json!({
                        "ok": false,
                        "error": {
                            "code": "workspace_dispatch_failed",
                            "message": response
                        }
                    }),
                )
            }))
        }
    }
}

fn request_session_key(body: &Value) -> Option<&str> {
    body.get("params")
        .and_then(|params| params.get("_meta"))
        .and_then(|meta| meta.get("openai/session"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn inject_session_metadata(body: &Value, session_key: Option<&str>) -> Value {
    let Some(session_key) = session_key else {
        return body.clone();
    };
    if request_session_key(body).is_some() {
        return body.clone();
    }

    let mut forwarded = body.clone();
    if !forwarded.get("params").is_some_and(Value::is_object) {
        forwarded["params"] = json!({});
    }
    if !forwarded["params"].get("_meta").is_some_and(Value::is_object) {
        forwarded["params"]["_meta"] = json!({});
    }
    forwarded["params"]["_meta"]["openai/session"] = Value::String(session_key.to_string());
    forwarded
}

fn gateway_error_result(
    tool_name: &str,
    args: &Value,
    code: &str,
    message: &str,
    state: &SharedGatewayState,
) -> Value {
    wrap_mcp_tool_result(
        tool_name,
        args,
        json!({
            "ok": false,
            "error": {
                "code": code,
                "message": message
            },
            "workspaces": state.list_workspaces()
        }),
    )
}

fn gateway_tool_catalog(state: &SharedGatewayState) -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": LIST_WORKSPACES,
            "title": "List workspaces",
            "description": "List projects registered in the single Coding Tools Gateway. Call this before selecting a project in a new multi-workspace conversation.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": SELECT_WORKSPACE,
            "title": "Select workspace",
            "description": "Bind the current MCP/ChatGPT conversation to one registered workspace. Existing file, Git, exec and history tools then operate only in that workspace until explicitly switched.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "description": "Workspace id, unique name, or /w/<workspace-id>/mcp path."
                    }
                },
                "required": ["workspace"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": GET_CURRENT_WORKSPACE,
            "title": "Get current workspace",
            "description": "Return the workspace currently bound to this conversation.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
    ];
    tools.extend(list_tools_for_profile(&state.tool_profile));
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_transport_session_only_when_openai_session_is_absent() {
        let body = json!({
            "method": "tools/call",
            "params": { "name": "history_session_bootstrap", "arguments": {} }
        });
        let forwarded = inject_session_metadata(&body, Some("transport-session"));
        assert_eq!(
            forwarded["params"]["_meta"]["openai/session"],
            "transport-session"
        );
    }
}

