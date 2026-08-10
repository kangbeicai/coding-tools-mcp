//! Local Web management plane for Linux server deployments.
//!
//! The admin listener is deliberately separate from the public MCP gateway.
//! In headless/manual-run deployments it defaults to all IPv4 interfaces so
//! the Web Console can be opened directly from a trusted LAN. It serves both
//! the static Svelte application and a small JSON RPC surface backed by the
//! same application state as the Gateway runtime.

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
