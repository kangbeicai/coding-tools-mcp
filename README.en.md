# Coding Tools MCP

A self-hosted Linux / Windows Coding Gateway and browser Web Console. One `coding-tools` process provides a multi-workspace MCP Gateway, OAuth/Bearer authentication, and optional managed FRP or Cloudflare exposure.

[中文](README.md)

## Scope

- Linux x86_64 / aarch64 and Windows x86_64.
- A single `coding-tools` CLI binary; no desktop shell or system WebView.
- Browser management through `POST /api/rpc`.
- One root `/mcp` connector with per-session workspace selection.
- No Docker requirement. Foreground mode is the default on Linux and Windows; the built-in user-level systemd installer is Linux-only.

## Default endpoints

| Service | Default | Purpose |
|---------|---------|---------|
| MCP Gateway | `http://127.0.0.1:28766/mcp` | MCP data plane |
| Web Console | `http://0.0.0.0:28767/` | Browser management plane |
| Workspace route | `/w/<workspace-id>/mcp` | Explicit routing and diagnostics |

The Web Console has separate administrator authentication. First access goes through `/login`, where you create the administrator username and password. Only an Argon2 password hash is persisted. Admin sessions live in process memory for up to 12 hours, so a server restart requires signing in again.

Web Admin authentication is separate from MCP OAuth/Bearer authentication. The Admin listener still defaults to plain HTTP on `0.0.0.0:28767`; authentication does not provide transport encryption. Keep it on a trusted LAN/VPN or place it behind an HTTPS reverse proxy instead of exposing it directly to an untrusted Internet.

## Build

Node.js 20+, npm, and Rust stable are required. Linux additionally needs the normal system build tools; Windows uses a Rust Windows toolchain.

```bash
npm ci
npm run check
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

The Rust build embeds the generated frontend. Always run `npm run build` before Cargo after frontend changes.

The binary is written to:

```text
Linux:   src-tauri/target/release/coding-tools
Windows: src-tauri\target\release\coding-tools.exe
```

## Download Release binaries

Tagged GitHub Releases publish Headless/Web binaries for Linux x86_64, Linux aarch64, and Windows x86_64 together with `SHA256SUMS`. Replace `VERSION` with the release tag:

Linux x86_64:

```bash
VERSION=vX.Y.Z
curl -fL "https://github.com/kangbeicai/coding-tools-mcp/releases/download/${VERSION}/coding-tools-linux-x86_64" -o coding-tools
chmod +x coding-tools
./coding-tools
```

Or with wget:

```bash
VERSION=vX.Y.Z
wget "https://github.com/kangbeicai/coding-tools-mcp/releases/download/${VERSION}/coding-tools-linux-x86_64" -O coding-tools
chmod +x coding-tools
./coding-tools
```

For Linux aarch64 use `coding-tools-linux-aarch64`. Windows x86_64 uses `coding-tools-windows-x86_64.exe`. Every Release also contains `SHA256SUMS` for integrity verification.

## Run

```bash
./src-tauri/target/release/coding-tools
```

Windows PowerShell:

```powershell
.\src-tauri\target\release\coding-tools.exe
```

Both platforms run the same headless MCP Gateway + Web Admin. Windows does not create a native window, tray icon, or WebView.

Useful commands:

```bash
coding-tools serve
coding-tools tui
coding-tools workspace list
coding-tools admin reset
coding-tools config show
coding-tools health --json
```

Linux can additionally install the user-level systemd service:

```bash
coding-tools service install
coding-tools service status
coding-tools service uninstall
```

Windows does not currently include a Windows Service installer. To run it persistently, supervise the same `coding-tools.exe` with Windows or a third-party service manager.

Runtime overrides:

```bash
coding-tools serve \
  --bind 127.0.0.1 \
  --port 28766 \
  --admin-bind 0.0.0.0 \
  --admin-port 28767 \
  --auth oauth
```

Supported flags are `--bind`, `--port`, `--public-url`, `--auth`, `--admin-bind`, `--admin-port`, and the development-only `--web-root` override.

## Web Admin login and password recovery

On first use, open:

```text
Local:  http://127.0.0.1:28767/login
LAN:    append /login to the actual LAN URL printed in the startup summary
```

The page asks you to create the administrator username and password. The password is never stored in plaintext; only its Argon2 hash is persisted. After login the browser uses an HttpOnly, SameSite=Strict session cookie. Sessions last up to 12 hours and are stored only in the `coding-tools` process, so restarting the service invalidates all Admin sessions.

If you forget the administrator password, run this locally on the server:

```bash
coding-tools admin reset
```

The command only restores the Admin username to the default `admin` value and clears the Admin password hash. It does not change Gateway, MCP, OAuth, Cloudflare/FRP, workspace, or other secret configuration.

The reset command updates persisted configuration, while an already running `coding-tools` process still has the old settings in memory. Restart the service after the reset, then open `http://127.0.0.1:28767/login` locally, or append `/login` to the actual `LAN` URL printed in the startup summary. The original password cannot be recovered from the Argon2 hash; it can only be reset.

## Configuration

The canonical configuration directory is now:

```text
Linux:   ~/.config/coding-tools-mcp/
Windows: %APPDATA%\coding-tools-mcp\
```

Upgrade compatibility is automatic: when the canonical directory does not yet exist but the historical `coding-tools-mcp-desktop` directory does, `coding-tools` first tries to rename the entire legacy root to the canonical root. This keeps workspaces, Gateway/Admin settings, credentials, tunnel binaries/caches, and logs together. If the rename fails, the current run keeps using the legacy root without deleting data. If both roots already exist, the canonical root wins and they are not merged automatically.

Workspace paths are absolute paths on the server, such as `/home/user/projects/example`.

Different ChatGPT session keys may select different workspaces, but they share the same process, filesystem, Git repositories, subprocesses, secrets, and permissions. This is multi-workspace routing, not tenant isolation.

## Public access

The Gateway supports OAuth authorization code, Bearer tokens, and local-only `noauth`. A configured canonical origin produces an MCP endpoint such as `https://mcp.example.com/mcp` and matching OAuth metadata.

Managed exposure options:

- FRP through a managed `frpc` process.
- Cloudflare Quick or Named Tunnel. Named mode requires a tunnel token and fixed public URL.

Both `frpc` and `cloudflared` support on-demand download into the application cache. Starting an exposure first reuses a binary from `PATH`, common platform locations, or the existing cache; only a missing binary triggers a download. `cloudflared` uses Cloudflare's official latest release: Linux x86_64/aarch64 downloads the standalone binary, while Windows x86_64 downloads `cloudflared-windows-amd64.exe` and caches it as `cloudflared.exe`. Downloads reuse the global GitHub mirror and download-proxy settings, and users do not start `cloudflared` separately.

## Client workflow

1. Call `list_workspaces`.
2. Call `select_workspace` for the current session.
3. Call `history_session_bootstrap` and pass the verbatim first request as `initial_user_input`.
4. Use file, Git, Exec, Patch, and other tools.
5. When exact earlier context is needed, call `history_session_search` first and then page the original Markdown with `history_session_read`.
6. Call `history_session_checkpoint` when the task is complete and pass the verbatim current request as `raw_user_input`.

History Session v2 exposes five tools:

| Tool | Purpose |
|------|---------|
| `history_session_bootstrap` | Create or resume the current session and return bounded current state plus retrieval guidance instead of all history |
| `history_session_checkpoint` | Append structured progress and the current raw user input; changed content for the same turn is retained as revision/supersedes evidence |
| `history_session_validate` | Validate archive numbering and rebuild `index.json`, `memory/state.json`, and `memory/manifest.json` when requested |
| `history_session_search` | Search history archives by deterministic keywords and return bounded locations/snippets |
| `history_session_read` | Read one numeric Markdown archive losslessly; pages default to 32 KiB and are capped at 64 KiB, with a content hash for change detection |

History remains project-local under each workspace's `docs/history-session/`. Numeric `N.md` files are the durable source of truth; `memory/state.json` and `memory/manifest.json` are bounded derived data that can be rebuilt from Markdown.

The `session_key` and `current_path` returned by `history_session_bootstrap` form the stable write target. Every checkpoint must pass them back unchanged as `session_key` and `expected_path`; changing ChatGPT host-session metadata cannot redirect an established checkpoint to another archive.

The server cannot read ChatGPT transcript text that was not supplied as an MCP argument. Use `initial_input_captured`, `user_input_captured`, and returned warnings to determine whether the first/current request was actually archived.

## Verification

```bash
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

## License

Apache-2.0
