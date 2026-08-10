# 架构设计

## 总览

```text
Browser
  -> Web Console
  -> POST /api/rpc
  -> Admin listener :28767

ChatGPT / MCP client
  -> /mcp or /w/<workspace-id>/mcp
  -> Gateway listener :28766
  -> session key -> selected workspace
  -> ToolContext -> File/Git/Exec/Patch/History

Optional public exposure
  -> managed FRP or Cloudflare
  -> Gateway listener
```

## 进程边界

唯一 `coding-tools` 进程负责：

- 全局多工作区 MCP Gateway。
- 静态 Web Console 和 `/api/rpc` 管理面。
- OAuth/Bearer/noauth 认证。
- 工作区配置和 session 路由。
- 工具执行、命令 session 和项目历史。
- 可选 FRP/Cloudflare 子进程监督。
- SIGINT/SIGTERM 优雅退出和可选 user-level systemd 托管。

## 目录

```text
src/                         Svelte Web Console
src-tauri/src/bin/           coding-tools CLI 入口
src-tauri/src/headless.rs    CLI、前台运行、TUI、health、service
src-tauri/src/admin/         embedded Web 与 /api/rpc
src-tauri/src/gateway/       多工作区 MCP Gateway
src-tauri/src/tools/         工具内核与执行策略
src-tauri/src/tunnel/        FRP/Cloudflare provider 与监督
src-tauri/src/platform/      Linux 平台实现
docs/specs/                  需求、设计和任务
old/                         历史参考实现
```

## 数据与路由

Gateway 根入口是 `/mcp`。会话通过 `list_workspaces` 和 `select_workspace` 绑定工作区；`/w/<workspace-id>/mcp` 只用于显式路由和调试。每个工作区有独立 `ToolContext` 和项目本地 `docs/history-session/`，但服务器进程和 OS 权限仍然共享。

## 公网身份

`gateway.public_url` 是 canonical external origin。FRP/Cloudflare 是可选传输层，不应反向覆盖稳定身份。Cloudflare Quick URL 仅记录为运行期 effective URL；Named Tunnel 与 FRP 面向 ChatGPT 时应使用显式 HTTPS canonical URL。

## 安全边界

- Gateway 与 Web Admin 使用不同端口。
- Web Admin 尚无独立管理员认证，只能放在可信 LAN/VPN/防火墙后。
- Workspace 文件操作执行 canonical path 和 symlink 边界检查。
- `exec_command` 使用策略与 parser 边界，不是完整 OS 沙箱。
- 多 session/多工作区不是多租户隔离。
