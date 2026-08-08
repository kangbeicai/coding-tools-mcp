# 架构设计

> 本文档描述 Coding Tools MCP 当前架构。项目定位已经从“每个工作区一个桌面 MCP 服务”演进为“单入口、多工作区、自托管 Coding Gateway”；Tauri 是可选桌面壳，Web Console 是跨平台管理界面。

## 产品边界

Coding Tools MCP 分为两个独立平面：

```text
ChatGPT / MCP Client
        │
        │ HTTPS / MCP
        ▼
┌─────────────────────────────┐
│ MCP Data Plane              │
│ /mcp                        │
│ /w/<workspace-id>/mcp       │
└──────────────┬──────────────┘
               │
               ▼
       Multi-workspace Gateway
               │
       session → workspace
               │
      ┌────────┼────────┐
      ▼        ▼        ▼
 Workspace A  B        C
 ToolContext ToolContext ...


Browser / Tauri shell
        │
        │ local admin HTTP / IPC
        ▼
┌─────────────────────────────┐
│ Management Plane            │
│ Web Console + Admin API     │
└─────────────────────────────┘
```

- **MCP Data Plane** 可以按部署需要监听 `127.0.0.1`、`0.0.0.0` 或指定网卡，并放在 NAT、反向代理、FRP、Cloudflare 后面。
- **Web Admin Plane** 默认监听 `0.0.0.0:28767`，便于手动启动 headless 服务后从可信 LAN 直接管理；在加入独立管理员认证前不得直接暴露到不可信网络/公网。
- ChatGPT 只配置一次根 `/mcp`，不为每个 Workspace 创建独立插件。

### Gateway Identity 与 Public Access 分离

Gateway 的公网身份和传输方式必须分开建模：

```text
Gateway Identity
  canonical public_url = https://mcp.example.com
  MCP endpoint         = https://mcp.example.com/mcp
  OAuth metadata       = https://mcp.example.com/...

Public Access Provider
  local | direct | external | frp | cloudflare
```

`gateway.public_url` 是稳定的 canonical external origin。FRP/Cloudflare 只是将网络流量送到 Gateway 的 provider，不允许反向决定或覆盖这个 URL。

Cloudflare Quick Tunnel 返回的 `trycloudflare.com` 地址只记录为运行期 `effective_public_url`。它是临时 transport endpoint，不是 Gateway canonical identity。

## 当前项目结构

```text
coding-tools-mcp/
├── src-tauri/
│   ├── src/
│   │   ├── gateway/          # 单入口多 Workspace MCP Gateway
│   │   ├── admin/            # LAN-capable Web Admin server + JSON RPC
│   │   ├── tools/            # 文件/Patch/Exec/Git/History 工具内核
│   │   ├── workspace/        # Workspace 配置与资源校验
│   │   ├── runtime/          # 旧 per-workspace runtime 兼容层
│   │   ├── mcp/              # 单 Workspace MCP 协议实现，Gateway 复用
│   │   ├── actions/          # GPT Actions 兼容服务
│   │   ├── tunnel/           # FRP / Cloudflare provider + 旧兼容层
│   │   ├── auth/             # OAuth / Bearer
│   │   ├── data/             # 持久化 AppData
│   │   ├── settings/         # Gateway/Admin/下载/代理等设置
│   │   ├── platform/         # Windows/Linux/macOS 差异
│   │   ├── async_runtime.rs  # Tauri/Tokio runtime 抽象
│   │   ├── headless.rs       # `coding-tools serve/tui/...`
│   │   ├── main.rs           # Tauri desktop binary
│   │   └── bin/coding-tools.rs # headless binary
│   └── Cargo.toml
├── src/
│   ├── lib/api/
│   │   └── transport.ts      # Tauri IPC / Web Admin HTTP 统一调用层
│   ├── lib/components/
│   └── routes/
│       ├── settings/gateway/ # 全局 Gateway 管理
│       ├── settings/keys/    # 共享认证密钥
│       ├── web/workspace/    # Web-native Workspace 管理
│       └── workspace/        # 旧 Tauri 完整兼容页面
├── docs/
└── old/                      # 旧 Python 参考实现
```

## 运行模式

### 1. Headless / Server（推荐核心形态）

```bash
coding-tools serve
```

一个进程同时承载：

```text
MCP Gateway   127.0.0.1:28766/mcp   （可改监听地址）
Web Admin     0.0.0.0:28767/         （默认 LAN 可访问）
```

Linux 无图形环境不需要 Tauri、WebKit 或 GTK runtime。产品的首选运行模式是 CLI-first/manual-run：用户像启动 OpenCode/Pi 一样手动运行 `coding-tools`，进程在前台托管 Web Admin、Global Gateway 和可选 managed exposure，`Ctrl+C` 后完整退出。

SvelteKit 仍使用 `adapter-static` 构建到 `build/`，但 headless release 构建会把这套静态资源嵌入 `coding-tools` binary。Admin Server 优先使用显式/开发态 filesystem Web root，找不到时回退到 embedded assets，因此正式 Linux 分发可以只有一个可执行文件。

```text
coding-tools
    │
    ├── Web Admin      0.0.0.0:28767
    ├── Global Gateway 127.0.0.1:28766/mcp
    └── Managed Exposure（可选 FRP/Cloudflare）

Ctrl+C
    └── graceful shutdown → process exits
```

对于确实需要无人值守/开机自启动的服务器，headless binary 仍保留 user-level systemd service 作为高级兼容部署方式，但它不再是默认产品路线：

```text
coding-tools install-service
        │
        ├── ~/.local/share/coding-tools/bin/coding-tools
        ├── ~/.local/share/coding-tools/web/
        └── ~/.config/systemd/user/coding-tools.service
                    │
                    ├── Restart=on-failure
                    ├── KillSignal=SIGINT
                    └── coding-tools serve --web-root <stable bundle>
```

安装器不调用 sudo。若需要 user manager 在注销后继续存在，由管理员显式执行 `loginctl enable-linger <user>`。

### 2. Tauri Desktop

Tauri 继续作为桌面壳存在，并复用同一套 Svelte 源码和 Rust Core。桌面专用能力，例如原生目录选择、打开系统文件管理器、WebView 内存管理，保留在 `desktop` feature 下。

长期目标是让绝大多数业务操作通过 UI-independent service 完成，Tauri command 只做薄适配。

### 3. Lightweight TUI / CLI

`coding-tools tui` 只承担轻量状态查看，不再维护第二套完整管理 UI。脚本化操作逐步通过：

```text
coding-tools workspace ...
coding-tools config ...
```

补充。Web Console 是主要交互式管理界面。

## 单入口多 Workspace 路由

### 外部接口

ChatGPT 始终连接：

```text
https://mcp.example.com/mcp
```

Gateway 额外提供：

```text
/w/<workspace-id>/mcp
```

作为显式路径路由和调试入口，但这些路径**不是**要求用户创建多个 ChatGPT 插件。

### 会话绑定

Gateway 维护：

```text
ChatGPT / MCP session key → Workspace ID
```

基本流程：

```text
new conversation
      │
      ▼
list_workspaces
      │
      ▼
select_workspace
      │
      ▼
history_session_bootstrap
      │
      ▼
file / git / exec / patch / ...
```

两个及以上 Workspace 时不允许自动猜测项目。Web Console 可以查看并手工解绑 session → workspace 关系；解绑后下一次项目工具调用会重新要求选择 Workspace。

### ToolContext 隔离

每个 Workspace 建立独立 `ToolContext`：

```text
ToolContext
├── Workspace root
├── execution policy
├── permission mode
├── default cwd
├── command sessions
└── project-local history
```

Gateway 只负责选择正确的 `ToolContext`，实际 File/Git/Exec/Patch 工具仍统一经过 `tools::call_tool`，不复制工具实现。

## 历史会话

历史目录仍属于项目本身：

```text
workspace-a/docs/history-session/
workspace-b/docs/history-session/
```

因此多 Workspace Gateway 不建立一份全局开发历史。正确顺序是先选择 Workspace，再执行 `history_session_bootstrap`。

历史工具继续使用 ChatGPT `_meta.openai/session` 或 MCP transport session 标识建立稳定 session key，并要求 checkpoint 使用 bootstrap 返回的 `session_key/current_path`。

## Web 管理架构

Svelte API 调用统一走：

```text
invokeCommand(command, args)
        │
        ├── Tauri runtime → tauri.invoke(...)
        │
        └── Browser       → POST /api/rpc
```

这样页面本身不需要复制成“Desktop UI”和“Web UI”两套。

当前 Web Admin 已优先覆盖：

- Gateway 配置 / 启停 / 状态；
- Gateway canonical 公网 URL 与 Public Access（local/direct/external/FRP/Cloudflare）；
- Gateway 级 managed FRP/Cloudflare 的独立启停与运行状态；
- Workspace 列表、新增、基础策略修改、移除；
- session → workspace 路由查看与解绑；
- 共享 MCP OAuth / Bearer 密钥；
- 旧 per-workspace runtime 的只读状态，用于兼容导航。
- Gateway 分层健康检查：配置 → owning runtime → local listener → public provider/transport → canonical public MCP → OAuth metadata。

健康检查特别区分两个地址概念：

```text
effective transport URL      例如 Cloudflare Quick 的随机 trycloudflare.com
canonical connector identity 例如 https://mcp.example.com
```

当两者不同时分别探测，`ChatGPT-ready` 以 ChatGPT 实际应该连接的 canonical identity 为准；只有未配置 canonical 的 Quick 临时模式才把临时 URL 当作 connector base。

尚未完全迁移的桌面兼容能力包括旧 per-workspace tunnel 的全部编辑 UI、软件下载安装、Actions 全套配置以及部分桌面专用操作。推荐 Gateway 模式已经拥有独立的 Public Access 管理。

## Gateway Public Access

推荐模式的公网暴露属于 Gateway，而不是 Workspace：

```text
                     canonical public URL
                              │
                              ▼
                     Global MCP Gateway
                         127.0.0.1:28766
                              │
                 session → selected Workspace

Public Access:
  local     → no public transport
  direct    → bind/NAT/router managed externally
  external  → Nginx/Caddy/VPS/WireGuard/etc.
  frp       → one managed gateway-mcp frpc route
  cloudflare→ one managed cloudflared process
```

- `local/direct/external` 是被动模式，不创建子进程。
- `frp/cloudflare` 是 managed 模式，有独立于 Gateway listener 的 start/stop 生命周期。
- 停止 Gateway 会先停止 managed exposure，避免留下指向已关闭后端的公网进程。
- headless `coding-tools` / `coding-tools serve` 在持久化模式为 `frp/cloudflare` 时会在 Gateway 成功启动后恢复 managed exposure；正常使用由用户手动启动进程，systemd 只是可选托管层。
- Managed FRP/Cloudflare 从本机 `127.0.0.1:<gateway-port>` 回连，因此 Gateway 必须监听 loopback 或 wildcard 地址；仅绑定某个非 loopback 网卡地址时拒绝启动 managed exposure。
- FRP 和 Cloudflare Named 模式面向 ChatGPT 时要求显式 HTTPS canonical URL。
- Cloudflare Quick 可以没有 canonical URL，但生成地址只进入运行状态，不持久化为 Gateway identity。

## 安全边界

1. MCP Gateway 和 Web Admin 使用不同端口和不同暴露策略。
2. Admin Server 默认监听 `0.0.0.0:28767` 以支持可信 LAN 直接管理；由于尚无独立管理员认证，部署侧必须用防火墙/VPN 限制管理端口的可达范围。
3. 多 Workspace 下必须显式 session 选择，避免工具调用落入错误项目。
4. Workspace 文件/Patch 工具继续使用现有路径边界与 symlink 检查。
5. `exec_command` 的进程级隔离仍属于后续安全债：当前主要是 policy/parser boundary，并非完整 OS filesystem sandbox。

## 兼容层

旧版“每 Workspace 一个 MCP listener / 一个本地端口 / 一个 tunnel”暂时保留，主要服务已有桌面配置和 Actions/Tunnel 功能。新功能应优先进入全局 Gateway，不应继续强化 per-workspace MCP 作为主架构。

迁移原则：

```text
业务逻辑 → UI-independent service
              ↑             ↑
          Web Admin API   Tauri command
```

避免在两种 UI 适配层分别实现同一套状态机或安全规则。

---
*返回索引: [../project-context.md](../project-context.md)*
