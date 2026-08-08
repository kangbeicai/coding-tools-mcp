use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

const UNIT_NAME: &str = "coding-tools.service";
const MANAGED_MARKER: &str = "# Managed by Coding Tools Gateway";

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub web_root: PathBuf,
    pub dry_run: bool,
    pub start: bool,
}

#[cfg(target_os = "linux")]
fn service_path_env() -> String {
    let mut paths = Vec::<String>::new();
    let mut push = |value: String| {
        if !value.is_empty() && !paths.iter().any(|existing| existing == &value) {
            paths.push(value);
        }
    };
    if let Some(home) = dirs::home_dir() {
        push(home.join(".local/bin").to_string_lossy().into_owned());
        push(home.join(".cargo/bin").to_string_lossy().into_owned());
    }
    if let Some(current) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&current) {
            push(path.to_string_lossy().into_owned());
        }
    }
    for path in [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
    ] {
        push(path.into());
    }
    paths.join(":")
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub unit_path: PathBuf,
    pub binary_path: PathBuf,
    pub web_root: PathBuf,
    pub unit_text: String,
    pub started: bool,
    pub linger_enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub unit_path: PathBuf,
    pub binary_path: PathBuf,
    pub web_root: PathBuf,
    pub installed: bool,
    pub active: String,
    pub enabled: String,
    pub linger_enabled: Option<bool>,
}

pub fn install(options: InstallOptions) -> AppResult<InstallResult> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = options;
        return Err(AppError::Message(
            "systemd service 安装仅支持 Linux headless 环境。".into(),
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let source_binary = std::env::current_exe()?;
        let paths = service_paths()?;
        let web_source = validate_web_root(&options.web_root)?;
        let unit_text = build_unit(&paths.binary, &paths.web_root);

        if options.dry_run {
            return Ok(InstallResult {
                unit_path: paths.unit,
                binary_path: paths.binary,
                web_root: paths.web_root,
                unit_text,
                started: false,
                linger_enabled: linger_enabled(),
            });
        }

        if let Some(parent) = paths.binary.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = paths.unit.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = paths.web_root.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if paths.unit.exists() {
            let existing = std::fs::read_to_string(&paths.unit)?;
            if !existing.contains(MANAGED_MARKER) {
                return Err(AppError::Message(format!(
                    "拒绝覆盖非 Coding Tools 管理的 systemd unit: {}",
                    paths.unit.display()
                )));
            }
        }

        atomic_copy_file(&source_binary, &paths.binary)?;
        replace_directory(web_source, &paths.web_root)?;
        atomic_write(&paths.unit, unit_text.as_bytes())?;

        run_systemctl(&["--user", "daemon-reload"])?;
        let started = if options.start {
            run_systemctl(&["--user", "enable", UNIT_NAME])?;
            run_systemctl(&["--user", "restart", UNIT_NAME])?;
            true
        } else {
            false
        };

        Ok(InstallResult {
            unit_path: paths.unit,
            binary_path: paths.binary,
            web_root: paths.web_root,
            unit_text,
            started,
            linger_enabled: linger_enabled(),
        })
    }
}

pub fn uninstall(keep_bundle: bool, dry_run: bool) -> AppResult<ServiceStatus> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (keep_bundle, dry_run);
        return Err(AppError::Message(
            "systemd service 卸载仅支持 Linux headless 环境。".into(),
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let paths = service_paths()?;
        let before = status()?;
        if dry_run {
            return Ok(before);
        }
        if paths.unit.exists() {
            let existing = std::fs::read_to_string(&paths.unit)?;
            if !existing.contains(MANAGED_MARKER) {
                return Err(AppError::Message(format!(
                    "拒绝删除非 Coding Tools 管理的 systemd unit: {}",
                    paths.unit.display()
                )));
            }
            let _ = run_systemctl(&["--user", "disable", "--now", UNIT_NAME]);
            std::fs::remove_file(&paths.unit)?;
            run_systemctl(&["--user", "daemon-reload"])?;
        }
        if !keep_bundle {
            if paths.binary.exists() {
                std::fs::remove_file(&paths.binary)?;
            }
            if paths.web_root.exists() {
                std::fs::remove_dir_all(&paths.web_root)?;
            }
            if let Some(bin_dir) = paths.binary.parent() {
                remove_dir_if_empty(bin_dir);
            }
            if let Some(root) = paths.web_root.parent() {
                remove_dir_if_empty(root);
            }
        }
        status()
    }
}

pub fn status() -> AppResult<ServiceStatus> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err(AppError::Message(
            "systemd service 状态仅支持 Linux headless 环境。".into(),
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let paths = service_paths()?;
        Ok(ServiceStatus {
            unit_path: paths.unit.clone(),
            binary_path: paths.binary.clone(),
            web_root: paths.web_root.clone(),
            installed: paths.unit.exists(),
            active: systemctl_state(&["--user", "is-active", UNIT_NAME]),
            enabled: systemctl_state(&["--user", "is-enabled", UNIT_NAME]),
            linger_enabled: linger_enabled(),
        })
    }
}

#[cfg(target_os = "linux")]
struct ServicePaths {
    unit: PathBuf,
    binary: PathBuf,
    web_root: PathBuf,
}

#[cfg(target_os = "linux")]
fn service_paths() -> AppResult<ServicePaths> {
    let config =
        dirs::config_dir().ok_or_else(|| AppError::Message("无法确定 XDG config 目录。".into()))?;
    let data = dirs::data_local_dir()
        .ok_or_else(|| AppError::Message("无法确定 XDG data 目录。".into()))?;
    let root = data.join("coding-tools");
    Ok(ServicePaths {
        unit: config.join("systemd/user").join(UNIT_NAME),
        binary: root.join("bin/coding-tools"),
        web_root: root.join("web"),
    })
}

#[cfg(target_os = "linux")]
fn validate_web_root(path: &Path) -> AppResult<&Path> {
    if !path.join("index.html").is_file() {
        return Err(AppError::Message(format!(
            "Web build 不完整：{} 中没有 index.html；先运行 npm run build，或使用 --web-root。",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn build_unit(binary: &Path, web_root: &Path) -> String {
    let mut environment = String::new();
    environment.push_str(&format!(
        "Environment={}\n",
        systemd_assignment("PATH", &service_path_env())
    ));
    for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            environment.push_str(&format!(
                "Environment={}\n",
                systemd_assignment(key, &value.to_string_lossy())
            ));
        }
    }
    format!(
        "{MANAGED_MARKER}\n[Unit]\nDescription=Coding Tools multi-workspace MCP Gateway\nDocumentation=https://github.com/mybolide/coding-tools-mcp\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart={} serve --web-root {}\nWorkingDirectory=%h\nRestart=on-failure\nRestartSec=3s\nKillSignal=SIGINT\nTimeoutStopSec=20s\nNoNewPrivileges=true\n{environment}\n[Install]\nWantedBy=default.target\n",
        systemd_arg(&binary.to_string_lossy()),
        systemd_arg(&web_root.to_string_lossy()),
    )
}

#[cfg(target_os = "linux")]
fn systemd_arg(value: &str) -> String {
    let escaped = value
        .replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(target_os = "linux")]
fn systemd_assignment(key: &str, value: &str) -> String {
    systemd_arg(&format!("{key}={value}"))
}

#[cfg(target_os = "linux")]
fn atomic_copy_file(source: &Path, destination: &Path) -> AppResult<()> {
    if source.canonicalize().ok().as_ref() == destination.canonicalize().ok().as_ref()
        && destination.exists()
    {
        return Ok(());
    }
    let temp = destination.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::copy(source, &temp)?;
    std::fs::rename(&temp, destination)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn replace_directory(source: &Path, destination: &Path) -> AppResult<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Message("Web bundle destination 没有父目录。".into()))?;
    let staging = parent.join(format!("web.tmp-{}", uuid::Uuid::new_v4()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    copy_tree(source, &staging)?;
    if destination.exists() {
        std::fs::remove_dir_all(destination)?;
    }
    std::fs::rename(&staging, destination)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_tree(source: &Path, destination: &Path) -> AppResult<()> {
    std::fs::create_dir_all(destination)?;
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Message(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| AppError::Message(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), target)?;
        } else {
            return Err(AppError::Message(format!(
                "Web build 中不支持符号链接或特殊文件: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn atomic_write(path: &Path, contents: &[u8]) -> AppResult<()> {
    let temp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temp, contents)?;
    std::fs::rename(temp, path)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_systemctl(args: &[&str]) -> AppResult<()> {
    let output = std::process::Command::new("systemctl")
        .args(args)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(AppError::Message(format!(
        "systemctl {} 失败: {}{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

#[cfg(target_os = "linux")]
fn systemctl_state(args: &[&str]) -> String {
    match std::process::Command::new("systemctl").args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                if output.status.success() {
                    "ok".into()
                } else {
                    "unknown".into()
                }
            } else {
                stdout
            }
        }
        Err(error) => format!("unavailable: {error}"),
    }
}

#[cfg(target_os = "linux")]
fn linger_enabled() -> Option<bool> {
    let user = std::env::var("USER").ok()?;
    let output = std::process::Command::new("loginctl")
        .args(["show-user", &user, "-p", "Linger", "--value"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn remove_dir_if_empty(path: &Path) {
    if std::fs::read_dir(path)
        .ok()
        .is_some_and(|mut entries| entries.next().is_none())
    {
        let _ = std::fs::remove_dir(path);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn unit_uses_restart_and_graceful_sigint() {
        let unit = build_unit(
            Path::new("/opt/coding tools/coding-tools"),
            Path::new("/opt/web"),
        );
        assert!(unit.contains(MANAGED_MARKER));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("KillSignal=SIGINT"));
        assert!(unit.contains("TimeoutStopSec=20s"));
        assert!(unit.contains("serve --web-root"));
        assert!(unit.contains("\"/opt/coding tools/coding-tools\""));
    }

    #[test]
    fn systemd_arg_escapes_specifiers_and_quotes() {
        assert_eq!(systemd_arg("/tmp/a%b\"c"), "\"/tmp/a%%b\\\"c\"");
    }

    #[test]
    fn service_path_contains_common_user_and_system_bins() {
        let path = service_path_env();
        assert!(path.contains("/.local/bin"));
        assert!(path.contains("/.cargo/bin"));
        assert!(path.split(':').any(|item| item == "/usr/bin"));
    }
}
