# Coding Tools MCP

Linux / Windows 自托管 Coding Gateway 与 Web Console。单个 `coding-tools` 进程同时提供多工作区 MCP Gateway、浏览器管理台、OAuth/Bearer 认证，以及可选 FRP/Cloudflare 公网暴露。

[English](README.en.md)

## 产品边界

- 支持 Linux x86_64 / aarch64，以及 Windows x86_64。
- 唯一交付物是 `coding-tools` CLI，不包含桌面应用或系统 WebView。
- Web Console 通过浏览器访问，所有管理调用统一走 `POST /api/rpc`。
- ChatGPT/MCP 客户端只需连接一个根 `/mcp`，会话再选择工作区。
- 不需要 Docker。Linux/Windows 都默认前台运行；仅 Linux 提供可选的 user-level systemd 安装命令。

## 默认端点

| 服务 | 默认地址 | 说明 |
|------|----------|------|
| MCP Gateway | `http://127.0.0.1:28766/mcp` | MCP 数据平面 |
| Web Console | `http://0.0.0.0:28767/` | 浏览器管理平面 |
| 工作区路由 | `/w/<workspace-id>/mcp` | 显式路由与调试 |

Web Console 使用独立的管理员登录保护。首次访问会进入 `/login` 设置管理员用户名和密码；密码只保存 Argon2 哈希。Admin session 保存在服务进程内存中，有效期为 12 小时，服务重启后需要重新登录。

管理员登录与 MCP 数据平面的 OAuth/Bearer 认证彼此独立。Web Admin 默认仍通过 HTTP 监听 `0.0.0.0:28767`，登录认证并不等于传输加密；建议只暴露给可信 LAN/VPN，或在反向代理后使用 HTTPS，不要直接开放到不可信公网。

## 构建

需要 Node.js 20+、npm、Rust stable。Linux 还需要常见系统构建工具；Windows 使用对应 Rust Windows toolchain。

```bash
npm ci
npm run check
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

前端静态资源在 Rust 构建时嵌入 release binary。修改前端后必须先运行 `npm run build`，再运行 Cargo build。

产物：

```text
Linux:   src-tauri/target/release/coding-tools
Windows: src-tauri\target\release\coding-tools.exe
```

## 下载 Release 二进制

正式版本通过 GitHub Release 提供 Linux x86_64、Linux aarch64 和 Windows x86_64 三个平台的 Headless/Web 二进制，以及 `SHA256SUMS`。将 `VERSION` 替换为实际 tag：

Linux x86_64：

```bash
VERSION=vX.Y.Z
curl -fL "https://github.com/kangbeicai/coding-tools-mcp/releases/download/${VERSION}/coding-tools-linux-x86_64" -o coding-tools
chmod +x coding-tools
./coding-tools
```

或使用 wget：

```bash
VERSION=vX.Y.Z
wget "https://github.com/kangbeicai/coding-tools-mcp/releases/download/${VERSION}/coding-tools-linux-x86_64" -O coding-tools
chmod +x coding-tools
./coding-tools
```

Linux aarch64 将文件名替换为 `coding-tools-linux-aarch64`；Windows x86_64 下载 `coding-tools-windows-x86_64.exe`。每个 Release 同时提供 `SHA256SUMS` 用于完整性校验。

## 运行

```bash
./src-tauri/target/release/coding-tools
```

Windows PowerShell：

```powershell
.\src-tauri\target\release\coding-tools.exe
```

两种平台都会启动同一套 Headless MCP Gateway + Web Admin；Windows 不创建桌面窗口、系统托盘或 WebView。

等价的显式命令：

```bash
coding-tools serve
```

常用命令：

```bash
coding-tools --help
coding-tools tui
coding-tools workspace list
coding-tools admin reset
coding-tools config show
coding-tools health
coding-tools health --json
```

Linux 可额外使用 systemd user service：

```bash
coding-tools service install
coding-tools service status
coding-tools service uninstall
```

Windows 当前不内置 Windows Service 安装器；需要后台常驻时可使用系统或第三方 service manager 托管同一个 `coding-tools.exe`。

临时覆盖监听配置：

```bash
coding-tools serve \
  --bind 127.0.0.1 \
  --port 28766 \
  --admin-bind 0.0.0.0 \
  --admin-port 28767 \
  --auth oauth
```

支持的覆盖参数：

- `--bind IP`
- `--port PORT`
- `--public-url URL`
- `--auth oauth|bearer|noauth`
- `--admin-bind IP`
- `--admin-port PORT`
- `--web-root PATH`，仅用于开发或外部静态资源覆盖

## Web Admin 登录与密码恢复

第一次打开 Web Console 时访问：

```text
http://<server-ip>:28767/login
```

页面会要求创建管理员用户名和密码。密码不会以明文保存，配置中只持久化 Argon2 密码哈希。登录成功后浏览器使用 HttpOnly、SameSite=Strict 的 session cookie；session 最长有效 12 小时，并且只保存在 `coding-tools` 进程内存中，因此服务重启后所有 Web Admin session 都会失效。

如果忘记管理员密码，在服务器本机终端执行：

```bash
coding-tools admin reset
```

该命令只会把 Admin 用户名恢复为默认 `admin` 并清空 Admin 密码哈希。它不会修改 Gateway、MCP、OAuth、Cloudflare/FRP、workspace 或其他 secret。

reset 修改的是磁盘配置；正在运行的 `coding-tools` 仍持有旧的内存配置。因此 reset 后必须重启服务，然后重新打开：

```text
http://<server-ip>:28767/login
```

重新设置管理员即可。密码不可从 Argon2 哈希中找回，只能通过该流程重置。

## 配置与数据

为兼容已部署实例，配置目录继续保留历史名称：

```text
Linux:   ~/.config/coding-tools-mcp-desktop/
Windows: %APPDATA%\coding-tools-mcp-desktop\
```

目录名中的 `desktop` 只是兼容旧安装的数据路径；当前 Linux/Windows 版本均为 Headless/Web 形态，不会启动桌面 UI。工作区、Gateway、认证密钥和隧道配置继续从原位置读取。

工作区注册的是服务器本机绝对路径，例如：

```text
/home/user/projects/example
```

多个 ChatGPT 会话可以使用不同 session key 选择不同工作区，但它们仍共享同一服务器进程、文件系统、Git、子进程和 Secret 存储。这是多工作区路由，不是多租户隔离。

## Gateway 与认证

Gateway 支持：

- OAuth authorization code
- Bearer token
- `noauth`，只适合受信任的本地网络

配置稳定公网地址后，MCP 入口通常为：

```text
https://mcp.example.com/mcp
```

OAuth metadata、授权和 token 端点使用同一 canonical public URL。Cloudflare Quick Tunnel 的随机地址只作为运行期 effective URL，不会覆盖稳定 Gateway identity。

## 公网暴露

支持两种受管模式：

- FRP：启动受管 `frpc` 并把 Gateway 暴露到配置的 HTTPS 域名。
- Cloudflare：支持 Quick Tunnel 和 Named Tunnel；Named Tunnel 需要 Token 与固定公网 URL。

`frpc` 与 `cloudflared` 都支持按需自动下载到配置目录缓存。启动 exposure 时会先复用 PATH、平台常见路径或已有缓存；只有找不到对应 binary 时才下载。`cloudflared` 使用 Cloudflare 官方 latest release：Linux x86_64/aarch64 下载独立 binary，Windows x86_64 下载 `cloudflared-windows-amd64.exe` 并缓存为 `cloudflared.exe`。下载继续复用全局 GitHub mirror 与下载代理设置；用户无需单独启动 `cloudflared`。

停止 Gateway 时会停止受管公网暴露。单独重启 Gateway listener 时，运行中的受管 exposure 会尽量保留，避免固定公网连接不必要重建。

## MCP 工作流

推荐客户端流程：

1. 调用 `list_workspaces`。
2. 调用 `select_workspace` 绑定当前会话。
3. 调用 `history_session_bootstrap`，首次请求通过 `initial_user_input` 逐字传入。
4. 使用文件、Git、Exec、Patch 等工具。
5. 需要精确旧上下文时，先调用 `history_session_search` 定位档案，再用 `history_session_read` 按页读取原始 Markdown。
6. 完成任务后调用 `history_session_checkpoint`，并通过 `raw_user_input` 传入本轮用户原始请求。

History Session v2 的五个工具：

| 工具 | 作用 |
|------|------|
| `history_session_bootstrap` | 创建或恢复当前会话，只返回有界当前状态和检索指引，不回灌全部历史 |
| `history_session_checkpoint` | 向当前会话追加结构化进度与本轮原始用户输入；同一 turn 的修改保留 revision/supersedes 证据 |
| `history_session_validate` | 校验档案编号和派生索引，可重建 `index.json`、`memory/state.json`、`memory/manifest.json` |
| `history_session_search` | 按关键词搜索历史档案，返回有界的定位结果和片段 |
| `history_session_read` | 无损读取一个数字 Markdown 档案；默认每页 32 KiB，最大 64 KiB，可用 hash 检测翻页期间的内容变化 |

项目历史保存在各工作区自己的 `docs/history-session/`，不是全局共享历史。数字 Markdown `N.md` 是长期事实源；`memory/state.json` 和 `memory/manifest.json` 只是可从 Markdown 重建的有界派生数据。

`history_session_bootstrap` 返回的 `session_key` 和 `current_path` 是稳定写入目标。后续 checkpoint 必须原样作为 `session_key` 和 `expected_path` 传回；即使 ChatGPT 的宿主会话元数据变化，也不会把已建立的 checkpoint 重定向到其他历史文件。

服务端无法读取没有作为 MCP 参数传入的 ChatGPT 对话文本。因此首次输入和每轮输入是否完整归档，以 `initial_input_captured` / `user_input_captured` 及返回的 warnings 为准。

## 开发验证

```bash
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

主要目录：

| 路径 | 用途 |
|------|------|
| `src/` | Svelte Web Console |
| `src-tauri/src/admin/` | Web 静态资源与 `/api/rpc` 管理平面 |
| `src-tauri/src/gateway/` | 多工作区 MCP Gateway |
| `src-tauri/src/tools/` | 文件、Patch、Exec、Git、History 工具内核 |
| `src-tauri/src/tunnel/` | FRP 与 Cloudflare 进程管理 |
| `docs/specs/` | 需求、设计和任务规格 |
| `old/` | 旧 Python/桌面实现，仅作行为参考 |

## License

Apache-2.0
