use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::platform::platform;
use crate::settings::ProxyConfig;

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const CLOUDFLARED_RELEASE_BASE: &str =
    "https://github.com/cloudflare/cloudflared/releases/latest/download";

static CLOUDFLARED_DOWNLOAD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Handle to a supervised `cloudflared` child process.
pub struct CloudflareTunnelHandle {
    pub child: Child,
    pub public_url: String,
    pub pid: Option<u32>,
}

pub fn resolve_cloudflared() -> AppResult<PathBuf> {
    platform()
        .cloudflared_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .or_else(|| cached_cloudflared_path().filter(|path| path.is_file()))
        .ok_or_else(|| AppError::Message("未找到 cloudflared。".into()))
}

pub async fn ensure_cloudflared() -> AppResult<PathBuf> {
    if let Ok(path) = resolve_cloudflared() {
        return Ok(path);
    }

    let lock = CLOUDFLARED_DOWNLOAD_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    if let Ok(path) = resolve_cloudflared() {
        return Ok(path);
    }

    download_cloudflared_to_cache().await
}

/// Path where the app caches a self-managed cloudflared binary.
pub(crate) fn cached_cloudflared_path() -> Option<PathBuf> {
    platform()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("bin").join(cloudflared_binary_name()))
}

pub(crate) fn cloudflared_binary_name() -> &'static str {
    #[cfg(windows)]
    {
        "cloudflared.exe"
    }
    #[cfg(not(windows))]
    {
        "cloudflared"
    }
}

fn cloudflared_release_asset(os: &str, arch: &str) -> AppResult<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("cloudflared-linux-amd64"),
        ("linux", "aarch64") => Ok("cloudflared-linux-arm64"),
        ("windows", "x86_64") => Ok("cloudflared-windows-amd64.exe"),
        _ => Err(AppError::Message(format!(
            "当前平台暂不支持自动下载 cloudflared: {os}/{arch}"
        ))),
    }
}

async fn download_cloudflared_to_cache() -> AppResult<PathBuf> {
    let settings = crate::settings::AppSettings::load_or_default();
    let asset = cloudflared_release_asset(std::env::consts::OS, std::env::consts::ARCH)?;
    let url = format!("{CLOUDFLARED_RELEASE_BASE}/{asset}");
    let bytes =
        crate::tunnel::download::download_release_asset(&settings, &url, "cloudflared").await?;
    let dest = cached_cloudflared_path()
        .ok_or_else(|| AppError::Message("无法确定 cloudflared 缓存目录。".into()))?;

    install_cloudflared_binary(&dest, &bytes)?;
    if dest.is_file() {
        Ok(dest)
    } else {
        Err(AppError::Message("cloudflared 自动安装失败。".into()))
    }
}

fn install_cloudflared_binary(dest: &Path, bytes: &[u8]) -> AppResult<()> {
    if bytes.is_empty() {
        return Err(AppError::Message("下载的 cloudflared 文件为空。".into()));
    }

    let parent = dest
        .parent()
        .ok_or_else(|| AppError::Message("cloudflared 缓存路径无效。".into()))?;
    std::fs::create_dir_all(parent)?;

    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cloudflared");
    let temp = parent.join(format!(".{file_name}.download-{}", Uuid::new_v4().simple()));

    if let Err(error) = std::fs::write(&temp, bytes) {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&temp)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&temp, permissions)?;
    }

    if dest.is_file() {
        let _ = std::fs::remove_file(&temp);
        return Ok(());
    }

    if let Err(error) = std::fs::rename(&temp, dest) {
        if dest.is_file() {
            let _ = std::fs::remove_file(&temp);
            return Ok(());
        }
        let _ = std::fs::remove_file(&temp);
        return Err(AppError::Message(format!("安装 cloudflared 失败: {error}")));
    }

    Ok(())
}

pub fn extract_trycloudflare_url(line: &str) -> Option<String> {
    const PREFIX: &str = "https://";
    const SUFFIX: &str = ".trycloudflare.com";
    let lower = line.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(rel) = lower[search_from..].find(PREFIX) {
        let start = search_from + rel;
        let Some(suffix_rel) = lower[start..].find(SUFFIX) else {
            break;
        };
        let end = start + suffix_rel + SUFFIX.len();
        let host = &line[start + PREFIX.len()..end - SUFFIX.len()];
        if host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !host.is_empty()
        {
            return Some(line[start..end].trim_end_matches('/').to_string());
        }
        search_from = start + PREFIX.len();
    }
    None
}

/// Apply the global proxy to a tunnel child process environment.
pub(crate) fn apply_proxy_env(cmd: &mut Command, proxy: &ProxyConfig) {
    let url = match proxy.mode.as_str() {
        "manual" if !proxy.url.trim().is_empty() => Some(proxy.url.trim().to_string()),
        "system" => std::env::var("HTTPS_PROXY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("HTTP_PROXY").ok().filter(|s| !s.is_empty()))
            .or_else(|| std::env::var("ALL_PROXY").ok().filter(|s| !s.is_empty())),
        _ => None,
    };
    if let Some(url) = url {
        for key in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "https_proxy",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ] {
            cmd.env(key, &url);
        }
        // Some cloudflared builds consult this dedicated variable.
        cmd.env("TUNNEL_HTTP_PROXY", &url);
    }
}

/// Spawn `cloudflared tunnel --url http://127.0.0.1:{port}` (quick) or named `tunnel run --token`.
pub async fn spawn_cloudflare_tunnel(
    port: u16,
    cwd: &Path,
    log_path: &Path,
    cloudflare_mode: &str,
    cloudflare_token: &str,
    named_public_url: &str,
    use_proxy: bool,
) -> AppResult<CloudflareTunnelHandle> {
    let quick = cloudflare_mode != "named";

    if !quick {
        if cloudflare_token.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写 Tunnel Token。".into(),
            ));
        }
        if named_public_url.trim().is_empty() {
            return Err(AppError::Message(
                "Cloudflare 命名隧道模式需要填写固定公网地址。".into(),
            ));
        }
    }

    let cloudflared = ensure_cloudflared().await?;

    let mut cmd = Command::new(&cloudflared);
    cmd.current_dir(cwd);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    let settings = crate::settings::AppSettings::load_or_default();
    if use_proxy {
        apply_proxy_env(&mut cmd, &settings.proxy);
    }

    if quick {
        cmd.args([
            "tunnel",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ]);
    } else {
        cmd.args([
            "tunnel",
            "--protocol",
            "http2",
            "run",
            "--token",
            cloudflare_token.trim(),
        ]);
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| AppError::Message(format!("启动 cloudflared 失败: {err}")))?;
    let pid = child.id();

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (ready_tx, ready_rx) = oneshot::channel();
    let log_path = log_path.to_path_buf();
    let named_url = named_public_url.trim_end_matches('/').to_string();
    let log_path_for_error = log_path.clone();

    if let Some(stdout) = child.stdout.take() {
        let stderr = child.stderr.take();
        tokio::spawn(async move {
            stream_cloudflare_output(stdout, stderr, &log_path, quick, named_url, ready_tx).await;
        });
    } else {
        let _ = ready_tx.send(QuickTunnelReady {
            public_url: if quick {
                None
            } else {
                Some(named_public_url.trim_end_matches('/').to_string())
            },
            named_ready: !quick,
        });
    }

    let ready = time::timeout(READY_TIMEOUT, ready_rx)
        .await
        .map_err(|_| {
            AppError::Message(format!(
                "cloudflared 已启动，但在 {} 秒内没有返回 trycloudflare.com 公网地址。\n\
                 请检查：1) MCP 服务是否已在本机端口 {port} 运行；2) 设置 → 通用 → 网络代理 是否配置为手动代理（如 http://127.0.0.1:7890）；\
                 3) 查看日志 {log_hint}",
                READY_TIMEOUT.as_secs(),
                log_hint = log_path_for_error.display()
            ))
        })?
        .map_err(|_| AppError::Message("cloudflared 输出流意外结束。".into()))?;

    let public_url = if quick {
        ready.public_url.ok_or_else(|| {
            AppError::Message(format!(
                "cloudflared 已启动，但没有解析到 trycloudflare.com 地址。请查看日志：{}",
                log_path_for_error.display()
            ))
        })?
    } else {
        named_public_url.trim_end_matches('/').to_string()
    };

    Ok(CloudflareTunnelHandle {
        child,
        public_url,
        pid,
    })
}

struct QuickTunnelReady {
    public_url: Option<String>,
    #[allow(dead_code)]
    named_ready: bool,
}

async fn stream_cloudflare_output<R, E>(
    stdout: R,
    stderr: Option<E>,
    log_path: &Path,
    quick: bool,
    named_url: String,
    ready_tx: oneshot::Sender<QuickTunnelReady>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    E: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut ready_tx = Some(ready_tx);
    let mut public_url: Option<String> = None;

    let mut log = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
    {
        Ok(file) => file,
        Err(_) => {
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(QuickTunnelReady {
                    public_url: if quick { None } else { Some(named_url) },
                    named_ready: !quick,
                });
            }
            return;
        }
    };

    let send_ready = |tx: &mut Option<oneshot::Sender<QuickTunnelReady>>,
                      url: Option<String>,
                      named_ready: bool| {
        if let Some(sender) = tx.take() {
            let _ = sender.send(QuickTunnelReady {
                public_url: url,
                named_ready,
            });
        }
    };

    let handle_line = |line: &str,
                           public_url: &mut Option<String>,
                           ready_tx: &mut Option<oneshot::Sender<QuickTunnelReady>>| {
        if quick {
            if public_url.is_none() {
                if let Some(url) = extract_trycloudflare_url(line) {
                    *public_url = Some(url.clone());
                    send_ready(ready_tx, Some(url), false);
                }
            }
        } else {
            let lowered = line.to_ascii_lowercase();
            if is_named_tunnel_ready_line(&lowered) {
                send_ready(ready_tx, Some(named_url.clone()), true);
            }
        }
    };

    // cloudflared logs primarily to stderr; read stdout and stderr concurrently.
    let (line_tx, mut line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let stderr_line_tx = line_tx.clone();

    tokio::spawn(async move {
        let mut stdout = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = stdout.next_line().await {
            if line_tx.send(line).is_err() {
                break;
            }
        }
    });

    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            let mut stderr = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = stderr.next_line().await {
                if stderr_line_tx.send(line).is_err() {
                    break;
                }
            }
        });
    }

    while let Some(line) = line_rx.recv().await {
        let _ = log.write_all(line.as_bytes()).await;
        let _ = log.write_all(b"\n").await;
        let _ = log.flush().await;
        handle_line(&line, &mut public_url, &mut ready_tx);
    }

    send_ready(&mut ready_tx, public_url, !quick);
}

fn is_named_tunnel_ready_line(lowered_line: &str) -> bool {
    lowered_line.contains("registered tunnel connection")
}

pub async fn stop_child(mut child: Child, pid: Option<u32>) -> AppResult<()> {
    if let Some(pid) = pid {
        let _ = platform().terminate_process_tree(pid);
    }

    let _ = child.kill().await;
    let _ = time::timeout(Duration::from_secs(3), child.wait()).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        cloudflared_release_asset, extract_trycloudflare_url, install_cloudflared_binary,
        is_named_tunnel_ready_line,
    };

    #[test]
    fn extracts_trycloudflare_url_from_log_line() {
        let line = "INF | https://abc-def.trycloudflare.com is your tunnel URL";
        assert_eq!(
            extract_trycloudflare_url(line).as_deref(),
            Some("https://abc-def.trycloudflare.com")
        );
    }

    #[test]
    fn ignores_invalid_hosts() {
        let line = "https://bad_host.trycloudflare.com";
        assert!(extract_trycloudflare_url(line).is_none());
    }

    #[test]
    fn named_tunnel_waits_for_registered_edge_connection() {
        assert!(!is_named_tunnel_ready_line(
            "inf starting metrics server on 127.0.0.1:20241/metrics"
        ));
        assert!(is_named_tunnel_ready_line(
            "inf registered tunnel connection connindex=0 protocol=http2"
        ));
    }

    #[test]
    fn maps_supported_cloudflared_release_assets() {
        assert_eq!(
            cloudflared_release_asset("linux", "x86_64").unwrap(),
            "cloudflared-linux-amd64"
        );
        assert_eq!(
            cloudflared_release_asset("linux", "aarch64").unwrap(),
            "cloudflared-linux-arm64"
        );
        assert_eq!(
            cloudflared_release_asset("windows", "x86_64").unwrap(),
            "cloudflared-windows-amd64.exe"
        );
        assert!(cloudflared_release_asset("macos", "aarch64").is_err());
    }

    #[test]
    fn rejects_empty_cloudflared_download() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dest = temp.path().join("cloudflared");
        assert!(install_cloudflared_binary(&dest, &[]).is_err());
        assert!(!dest.exists());
    }

    #[cfg(unix)]
    #[test]
    fn installs_cloudflared_cache_with_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let dest = temp.path().join("bin").join("cloudflared");
        install_cloudflared_binary(&dest, b"probe").expect("install cloudflared");

        assert_eq!(std::fs::read(&dest).unwrap(), b"probe");
        assert_ne!(
            std::fs::metadata(&dest).unwrap().permissions().mode() & 0o111,
            0
        );
    }
}
