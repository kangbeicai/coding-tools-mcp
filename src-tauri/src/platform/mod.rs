use std::path::PathBuf;

use crate::error::AppResult;

/// Linux OS primitives used by the server runtime.
#[allow(dead_code)]
pub trait Platform: Send + Sync {
    fn os_name(&self) -> &'static str;

    fn app_config_dir(&self) -> AppResult<PathBuf>;

    fn find_pid_listening_on_port(&self, port: u16) -> AppResult<Option<u32>>;

    /// Best-effort reclaim of a TCP listener on the given port.
    fn reclaim_listening_port(&self, port: u16) -> AppResult<bool> {
        let _ = port;
        Ok(false)
    }

    fn process_image_path(&self, pid: u32) -> AppResult<Option<String>>;

    fn is_process_alive(&self, pid: u32) -> bool;

    fn terminate_process_tree(&self, pid: u32) -> AppResult<()>;

    fn resolve_executable(&self, name: &str) -> Option<PathBuf>;

    fn cloudflared_candidates(&self) -> Vec<PathBuf>;

    fn frpc_candidates(&self) -> Vec<PathBuf>;
}

mod linux;

mod paths;

pub use linux::LinuxPlatform;

static PLATFORM: std::sync::OnceLock<LinuxPlatform> = std::sync::OnceLock::new();

pub fn platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(|| LinuxPlatform)
}
