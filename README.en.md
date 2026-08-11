# Coding Tools MCP

A self-hosted Linux Coding Gateway and browser Web Console. One `coding-tools` process provides a multi-workspace MCP Gateway, OAuth/Bearer authentication, and optional managed FRP or Cloudflare exposure.

[中文](README.md)

## Scope

- Linux x86_64 and aarch64 only.
- A single `coding-tools` CLI binary; no desktop shell or system WebView.
- Browser management through `POST /api/rpc`.
- One root `/mcp` connector with per-session workspace selection.
- No Docker requirement. Run in the foreground by default or install an optional user-level systemd service.

## Default endpoints

| Service | Default | Purpose |
|---------|---------|---------|
| MCP Gateway | `http://127.0.0.1:28766/mcp` | MCP data plane |
| Web Console | `http://0.0.0.0:28767/` | Browser management plane |
| Workspace route | `/w/<workspace-id>/mcp` | Explicit routing and diagnostics |

The Web Console has separate administrator authentication. First access goes through `/login`, where you create the administrator username and password. Only an Argon2 password hash is persisted. Admin sessions live in process memory for up to 12 hours, so a server restart requires signing in again.

Web Admin authentication is separate from MCP OAuth/Bearer authentication. The Admin listener still defaults to plain HTTP on `0.0.0.0:28767`; authentication does not provide transport encryption. Keep it on a trusted LAN/VPN or place it behind an HTTPS reverse proxy instead of exposing it directly to an untrusted Internet.

## Build

Node.js 20+, npm, Rust stable, and standard Linux build tools are required.

```bash
npm ci
npm run check
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

The Rust build embeds the generated frontend. Always run `npm run build` before Cargo after frontend changes.

The binary is written to:

```text
src-tauri/target/release/coding-tools
```

## Run

```bash
./src-tauri/target/release/coding-tools
```

Useful commands:

```bash
coding-tools serve
coding-tools tui
coding-tools workspace list
coding-tools admin reset
coding-tools config show
coding-tools health --json
coding-tools service install
coding-tools service status
coding-tools service uninstall
```

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
http://<server-ip>:28767/login
```

The page asks you to create the administrator username and password. The password is never stored in plaintext; only its Argon2 hash is persisted. After login the browser uses an HttpOnly, SameSite=Strict session cookie. Sessions last up to 12 hours and are stored only in the `coding-tools` process, so restarting the service invalidates all Admin sessions.

If you forget the administrator password, run this locally on the server:

```bash
coding-tools admin reset
```

The command only restores the Admin username to the default `admin` value and clears the Admin password hash. It does not change Gateway, MCP, OAuth, Cloudflare/FRP, workspace, or other secret configuration.

The reset command updates persisted configuration, while an already running `coding-tools` process still has the old settings in memory. Restart the service after the reset, then open `/login` again and create the administrator credentials. The original password cannot be recovered from the Argon2 hash; it can only be reset.

## Configuration

Existing Linux installations continue to use this directory to avoid losing persisted workspaces, secrets, and tunnel settings:

```text
~/.config/coding-tools-mcp-desktop/
```

Workspace paths are absolute paths on the server, such as `/home/user/projects/example`.

Different ChatGPT session keys may select different workspaces, but they share the same process, filesystem, Git repositories, subprocesses, secrets, and permissions. This is multi-workspace routing, not tenant isolation.

## Public access

The Gateway supports OAuth authorization code, Bearer tokens, and local-only `noauth`. A configured canonical origin produces an MCP endpoint such as `https://mcp.example.com/mcp` and matching OAuth metadata.

Managed exposure options:

- FRP through a managed `frpc` process.
- Cloudflare Quick or Named Tunnel. Named mode requires a tunnel token and fixed public URL.

`frpc` can be downloaded to the application cache. Install `cloudflared` on `PATH`, in a common Linux location, or as `bin/cloudflared` under the configuration directory.

## Client workflow

1. Call `list_workspaces`.
2. Call `select_workspace` for the current session.
3. Call `history_session_bootstrap`.
4. Use file, Git, Exec, Patch, and other tools.
5. Call `history_session_checkpoint` when the task is complete.

History remains project-local under each workspace's `docs/history-session/`.

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
