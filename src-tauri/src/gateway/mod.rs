//! Single-endpoint multi-workspace MCP gateway.
//!
//! ChatGPT connects to `/mcp` once. A conversation then selects one registered
//! workspace with `select_workspace`; subsequent file/Git/exec/history tools
//! are dispatched to that workspace's `ToolContext`. Explicit path routes such
//! as `/w/<workspace-id>/mcp` are also available for debugging and non-ChatGPT
//! clients, but they are not intended to require one connector per project.

mod listener;
mod server;
mod service;
mod state;

pub use listener::{spawn_listener, GatewayProcess};
pub use service::{
    gateway_status, get_gateway_config, restart_gateway_service, set_gateway_config,
    start_gateway_service, stop_gateway_service, GatewayStatusDto,
};
pub use state::{GatewaySessionInfo, GatewayState, GatewayWorkspaceInfo, SharedGatewayState};

