//! Local Web management plane for headless and desktop deployments.
//!
//! The admin listener is deliberately separate from the public MCP gateway.
//! It defaults to loopback-only and serves both the static Svelte application
//! and a small JSON RPC surface backed by the same application state as Tauri.

mod listener;
mod rpc;

pub use listener::{spawn_admin_listener, AdminProcess};
