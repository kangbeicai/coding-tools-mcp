//! Linux/server-oriented entry points that do not require Tauri, WebKit or a
//! graphical session. `serve` is the primary deployment mode; the Web Admin
//! console is served locally and can be reached remotely through SSH port
//! forwarding.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use crate::admin::{spawn_admin_listener, AdminProcess};
use crate::app_state::AppState;
use crate::gateway::{
    gateway_exposure_status, gateway_status, start_gateway_exposure_service, start_gateway_service,
    stop_gateway_service,
};
use crate::settings::{AdminConfig, GatewayConfig};

pub fn run_from_env() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("help") | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some("serve") => run_server(false, &args[1..]),
        Some("tui") => run_server(true, &args[1..]),
        Some("workspace") => workspace_command(&args[1..]),
        Some("config") => config_command(&args[1..]),
        Some(other) => Err(format!("未知命令: {other}\n运行 `coding-tools --help` 查看用法")),
    }
}

fn run_server(tui: bool, args: &[String]) -> Result<(), String> {
    let app = Arc::new(AppState::new().map_err(|error| error.to_string())?);
    let mut web_root = default_web_root();

    app.with_settings(|store| {
        let mut settings = store.settings();
        apply_overrides(&mut settings.gateway, &mut settings.admin, &mut web_root, args)
            .map_err(crate::error::AppError::Message)?;
        store.update_settings(settings)
    })
    .map_err(|error| error.to_string())?;

    let admin_config = app
        .with_settings(|store| Ok(store.settings().admin))
        .map_err(|error| error.to_string())?;
    let admin_port = admin_config.local_port;
    let admin = spawn_admin_listener(app.clone(), admin_config, web_root)?;
    let gateway_start = crate::async_runtime::block_on(start_gateway_service(&app));
    let exposure_mode = app
        .with_settings(|store| Ok(store.settings().gateway_exposure.mode))
        .map_err(|error| error.to_string())?;
    let managed_exposure = matches!(
        exposure_mode.trim().to_ascii_lowercase().as_str(),
        "frp" | "cloudflare"
    );
    let exposure_start = if gateway_start.is_ok() && managed_exposure {
        Some(crate::async_runtime::block_on(start_gateway_exposure_service(&app)))
    } else {
        None
    };

    if tui {
        run_terminal_ui(app, admin)
    } else {
        println!("Coding Tools server running");
        match gateway_start {
            Ok(gateway) => {
                println!("  MCP Gateway: {}", gateway.local_endpoint);
                if !gateway.public_endpoint.is_empty() {
                    println!("  Public MCP : {}", gateway.public_endpoint);
                }
                println!("  Workspaces : {}", gateway.workspace_count);
            }
            Err(error) => {
                println!("  MCP Gateway: not started ({error})");
                println!("               Open Web Admin, add/fix a workspace, then start Gateway there.");
            }
        }
        if let Some(result) = exposure_start {
            match result {
                Ok(exposure) => {
                    println!("  Exposure   : {} ({})", exposure.state, exposure.mode);
                    if !exposure.effective_public_url.is_empty() {
                        println!(
                            "  Effective  : {}/mcp",
                            exposure.effective_public_url.trim_end_matches('/')
                        );
                    }
                }
                Err(error) => {
                    println!("  Exposure   : failed ({error})");
                    println!("               Gateway remains available locally; fix Public Access in Web Admin.");
                }
            }
        }
        println!("  Web Admin  : {}", admin.local_endpoint);
        println!();
        println!("Remote Linux admin (recommended):");
        println!("  ssh -L {admin_port}:127.0.0.1:{admin_port} user@server");
        println!("  then open http://127.0.0.1:{admin_port}");
        println!("Press Ctrl+C to stop.");

        crate::async_runtime::block_on(async {
            tokio::signal::ctrl_c()
                .await
                .map_err(|error| format!("等待 Ctrl+C 失败: {error}"))
        })?;
        shutdown(app, admin)
    }
}

fn run_terminal_ui(app: Arc<AppState>, admin: AdminProcess) -> Result<(), String> {
    let stdin = io::stdin();
    loop {
        let status = gateway_status(&app).map_err(|error| error.to_string())?;
        let exposure = gateway_exposure_status(&app).map_err(|error| error.to_string())?;
        clear_terminal()?;
        println!("┌──────────────────────────────────────────────────────────┐");
        println!("│ Coding Tools Gateway · lightweight terminal monitor      │");
        println!("├──────────────────────────────────────────────────────────┤");
        println!("│ Gateway    {:<46} │", status.state);
        println!("│ MCP        {:<46} │", truncate(&status.local_endpoint, 46));
        println!("│ Web Admin  {:<46} │", truncate(&admin.local_endpoint, 46));
        println!("│ Workspaces {:<46} │", status.workspace_count);
        println!("│ Sessions   {:<46} │", status.session_count);
        println!("│ Exposure   {:<46} │", truncate(&format!("{} / {}", exposure.mode, exposure.state), 46));
        println!("└──────────────────────────────────────────────────────────┘");
        println!();
        println!("Web Console is the primary UI. This terminal view is intentionally minimal.");
        print!("[r]刷新  [s]会话  [q]退出 > ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut command = String::new();
        stdin.read_line(&mut command).map_err(|error| error.to_string())?;
        match command.trim().to_ascii_lowercase().as_str() {
            "q" | "quit" | "exit" => break,
            "s" | "sessions" => {
                clear_terminal()?;
                if status.sessions.is_empty() {
                    println!("暂无已绑定会话。");
                } else {
                    for session in status.sessions {
                        println!(
                            "{} → {} ({})",
                            redact_session(&session.session_key),
                            session.workspace_name,
                            session.workspace_id
                        );
                    }
                }
                pause()?;
            }
            "r" | "" => {}
            other => pause_with(&format!("未知命令: {other}"))?,
        }
    }
    shutdown(app, admin)
}

fn shutdown(app: Arc<AppState>, admin: AdminProcess) -> Result<(), String> {
    let _ = admin.shutdown.send(());
    let _ = crate::async_runtime::block_on(admin.handle);
    crate::async_runtime::block_on(stop_gateway_service(&app))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn workspace_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let state = AppState::new().map_err(|error| error.to_string())?;
            state
                .with_workspaces(|store| {
                    for profile in store.list() {
                        println!("{}\t{}\t{}", profile.id, profile.name, profile.path);
                    }
                    Ok(())
                })
                .map_err(|error| error.to_string())
        }
        Some(other) => Err(format!(
            "当前 CLI 暂不支持 workspace {other}；请使用 Web Console 或 `workspace list`。"
        )),
    }
}

fn config_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("show") | None => {
            let state = AppState::new().map_err(|error| error.to_string())?;
            let settings = state
                .with_settings(|store| Ok(store.settings()))
                .map_err(|error| error.to_string())?;
            println!("gateway.bindHost={}", settings.gateway.bind_host);
            println!("gateway.localPort={}", settings.gateway.local_port);
            println!("gateway.publicUrl={}", settings.gateway.public_url);
            println!("gateway.authType={}", settings.gateway.auth_type);
            println!("gateway.exposure.mode={}", settings.gateway_exposure.mode);
            println!("gateway.exposure.frpProfileId={}", settings.gateway_exposure.frp_profile_id);
            println!("gateway.exposure.frpSubdomain={}", settings.gateway_exposure.frp_subdomain);
            println!("gateway.exposure.cloudflareMode={}", settings.gateway_exposure.cloudflare_mode);
            println!("admin.bindHost={}", settings.admin.bind_host);
            println!("admin.localPort={}", settings.admin.local_port);
            Ok(())
        }
        Some(other) => Err(format!("未知 config 子命令: {other}")),
    }
}

fn apply_overrides(
    gateway: &mut GatewayConfig,
    admin: &mut AdminConfig,
    web_root: &mut PathBuf,
    args: &[String],
) -> Result<(), String> {
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        let value = args.get(index + 1).map(String::as_str);
        match key {
            "--bind" => gateway.bind_host = required(value, "--bind")?.to_string(),
            "--port" => gateway.local_port = parse_port(required(value, "--port")?, "--port")?,
            "--public-url" => gateway.public_url = required(value, "--public-url")?.to_string(),
            "--auth" => {
                let auth = required(value, "--auth")?.to_ascii_lowercase();
                if !matches!(auth.as_str(), "oauth" | "bearer" | "noauth") {
                    return Err("--auth 仅支持 oauth、bearer、noauth".into());
                }
                gateway.auth_type = auth;
            }
            "--admin-bind" => admin.bind_host = required(value, "--admin-bind")?.to_string(),
            "--admin-port" => {
                admin.local_port = parse_port(required(value, "--admin-port")?, "--admin-port")?
            }
            "--web-root" => *web_root = PathBuf::from(required(value, "--web-root")?),
            unknown => return Err(format!("未知参数: {unknown}")),
        }
        index += 2;
    }
    Ok(())
}

fn required<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, String> {
    value.ok_or_else(|| format!("{name} 缺少值"))
}

fn parse_port(value: &str, name: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("{name} 必须是 1-65535 的整数"))
}

fn default_web_root() -> PathBuf {
    if let Some(path) = std::env::var_os("CODING_TOOLS_WEB_ROOT") {
        return PathBuf::from(path);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for base in cwd.ancestors().take(3) {
        let candidate = base.join("build");
        if candidate.join("index.html").exists() {
            return candidate;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join("web");
            if sibling.join("index.html").exists() {
                return sibling;
            }
            for base in parent.ancestors().take(5) {
                let candidate = base.join("build");
                if candidate.join("index.html").exists() {
                    return candidate;
                }
            }
        }
    }
    cwd.join("build")
}

fn clear_terminal() -> Result<(), String> {
    print!("\x1b[2J\x1b[H");
    io::stdout().flush().map_err(|error| error.to_string())
}

fn pause() -> Result<(), String> {
    pause_with("按 Enter 返回…")
}

fn pause_with(message: &str) -> Result<(), String> {
    println!();
    print!("{message}");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|error| error.to_string())?;
    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    format!("{}…", value.chars().take(max_chars.saturating_sub(1)).collect::<String>())
}

fn redact_session(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 12 {
        return value.to_string();
    }
    let head: String = chars.iter().take(6).copied().collect();
    let tail: String = chars[chars.len() - 4..].iter().copied().collect();
    format!("{head}…{tail}")
}

fn print_help() {
    println!("Coding Tools Gateway + Web Console\n");
    println!("Usage:");
    println!("  coding-tools serve [gateway/admin overrides]");
    println!("  coding-tools tui   [gateway/admin overrides]  # optional terminal monitor");
    println!("  coding-tools workspace list");
    println!("  coding-tools config show");
    println!();
    println!("Overrides:");
    println!("  --bind IP --port PORT --public-url URL --auth oauth|bearer|noauth");
    println!("  --admin-bind 127.0.0.1 --admin-port 28767 --web-root ./build");
    println!();
    println!("Linux headless build:");
    println!("  npm ci && npm run build");
    println!("  cargo build --release --no-default-features --features headless --bin coding-tools");
    println!();
    println!("Remote admin:");
    println!("  ssh -L 28767:127.0.0.1:28767 user@server");
}
