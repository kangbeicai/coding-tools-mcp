# 项目图谱洞察

更新时间：2026-08-10

## 当前架构

项目已收敛为 Linux-only 单进程产品：

```text
Browser Web Console
  -> POST /api/rpc
  -> Admin listener

MCP Client / ChatGPT
  -> /mcp or /w/<workspace-id>/mcp
  -> Global Gateway
  -> session -> workspace
  -> ToolContext

coding-tools
  -> Tokio/Axum runtime
  -> optional FRP/cloudflared children
```

Tauri desktop binary、commands、配置、assets、Windows/macOS platform 实现和桌面 release workflows 已删除。Rust crate 虽继续位于 `src-tauri/`，但不再依赖 Tauri。

## 高影响边界

- Web `invokeCommand` 是所有保留管理页面的 transport 汇合点，已统一为 `/api/rpc`。
- `async_runtime::spawn` 仍位于 Gateway、listener、session 和 tunnel 生命周期关键路径，保留纯 Tokio 实现。
- `spawn_cloudflare_tunnel` 同时服务 Gateway exposure 与旧 Workspace runtime tunnel supervisor。
- `spawn_frpc`/`spawn_frpc_config` 覆盖 Workspace 聚合 FRP 与 Gateway FRP。
- `validate_command_for_workspace` 是所有 `exec_command` 的安全策略入口，Linux 默认已删除 Windows command/script allowlist。

## 本轮影响评估

| 范围 | 风险 | 结论 |
|------|------|------|
| Browser transport | Critical | 统一 RPC 后通过前端 check/build 与运行态 RPC 验证 |
| Rust crate rename | High | 仅 CLI 和仓库内测试引用，已原子更新并全量编译 |
| Exec policy defaults | High | 影响所有命令执行，24 项 security tests 与 contract tests 全通过 |
| Tunnel platform cleanup | Medium | Gateway/Workspace 两条 exposure 路径均编译并通过现有隧道测试 |
| Docs/workflows/assets | Low | 不影响运行时 |

## 验证证据

- `npm run check`：0 errors、0 warnings。
- `npm run build`：通过，静态 Web Console 写入 `build/`。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`：184 项通过。
- 隔离 release 构建：`/tmp/opencode/coding-tools-headless-target/release/coding-tools`。
- 临时配置和端口 smoke：Web `/`、`POST /api/rpc`、Gateway `/mcp` HTTP 200，SIGINT 正常退出。
- 生产旧进程和 Named Tunnel 未被替换，固定公网 `/mcp` 仍 HTTP 200。

## GitNexus 状态

本轮调用 GitNexus managed runtime 时，`onnxruntime-node` 下载因 HTTP 302 安装失败；`gitnexus_detect_changes` 在当前 MCP Probe CLI 中不可用。已使用现有图谱结论、直接调用点搜索、全量编译/测试和运行态 smoke 作为降级证据。提交前若 GitNexus runtime 恢复，仍应补跑 detect-changes。

## 剩余风险

- 生产进程运行在已 abandoned 的 SSH session scope，不属于 `coding-tools.service`；未明确安全重启方式前不自动替换。
- 保留的配置目录名 `coding-tools-mcp-desktop` 是已有数据兼容边界，不代表仍有桌面 runtime。
- Web Admin 尚无独立管理员认证，必须限制在可信 LAN/VPN/防火墙后。
