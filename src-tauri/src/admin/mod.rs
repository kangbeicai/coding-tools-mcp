//! Local Web management plane for headless and desktop deployments.
//!
//! The admin listener is deliberately separate from the public MCP gateway.
//! It defaults to loopback-only and serves both the static Svelte application
//! and a small JSON RPC surface backed by the same application state as Tauri.

mod listener;
mod rpc;

mod embedded_web {
    include!(concat!(env!("OUT_DIR"), "/embedded_web.rs"));
}

pub use listener::{spawn_admin_listener, AdminProcess};

pub(crate) fn embedded_web_asset(path: &str) -> Option<&'static [u8]> {
    embedded_web::embedded_web_asset(path)
}

pub(crate) fn embedded_web_asset_count() -> usize {
    embedded_web::EMBEDDED_WEB_ASSET_COUNT
}
