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
- **Web Admin Plane** 当前强制 loopback，仅用于本机浏览器或 SSH 端口转发；在加入独立管理员认证前不得直接暴露到局域网/公网。
- ChatGPT 只配置一次根 `/mcp`，不为每个 Workspace 创建独立插件。

## 当前项目结构

```text
coding-tools-mcp/
├── src-tauri/
│   ├── src/
│   │   ├── gateway/          # 单入口多 Workspace MCP Gateway
│   │   ├── admin/            # loopback Web Admin server + JSON RPC
│   │   ├── tools/            # 文件/Patch/Exec/Git/History 工具内核
│   │   ├── workspace/        # Workspace 配置与资源校验
│   │   ├── runtime/          # 旧 per-workspace runtime 兼容层
│   │   ├── mcp/              # 单 Workspace MCP 协议实现，Gateway 复用
│   │   ├── actions/          # GPT Actions 兼容服务
│   │   ├── tunnel/           # FRP / Cloudflare 兼容层
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
Web Admin     127.0.0.1:28767/       （当前强制 loopback）
```

Linux 无图形环境不需要 Tauri、WebKit 或 GTK runtime。SvelteKit 使用 `adapter-static` 构建到 `build/`，由 Rust Admin Server 直接提供静态文件。

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
- Workspace 列表、新增、基础策略修改、移除；
- session → workspace 路由查看与解绑；
- 共享 MCP OAuth / Bearer 密钥；
- 旧 per-workspace runtime 的只读状态，用于兼容导航。

尚未完全迁移的桌面兼容能力包括 FRP/Cloudflare 自动管理、软件下载安装、Actions 全套配置以及部分桌面专用操作。

## 安全边界

1. MCP Gateway 和 Web Admin 使用不同端口和不同暴露策略。
2. Admin Server 当前只接受 loopback bind；远程 Linux 管理使用 SSH port forwarding。
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
