# 项目图谱洞察

更新时间：2026-08-11

## 当前架构

项目为 Linux / Windows Headless 单进程产品：

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

Tauri desktop binary、commands、桌面窗口/托盘/WebView 和 desktop release workflows 已删除。Rust crate 虽继续位于 `src-tauri/`，但不再依赖 Tauri。Windows 仅恢复服务端所需的 Win32 platform/exec/tunnel 原语，继续使用同一个 Web Admin，不恢复桌面壳；macOS 仍未支持。

## 高影响边界

- Web `invokeCommand` 是所有保留管理页面的 transport 汇合点，已统一为 `/api/rpc`。
- `async_runtime::spawn` 仍位于 Gateway、listener、session 和 tunnel 生命周期关键路径，保留纯 Tokio 实现。
- `spawn_cloudflare_tunnel` 同时服务 Gateway exposure 与旧 Workspace runtime tunnel supervisor。
- `spawn_frpc`/`spawn_frpc_config` 覆盖 Workspace 聚合 FRP 与 Gateway FRP。
- `validate_command_for_workspace` 是所有 `exec_command` 的安全策略入口；默认 workspace script extension 按 target 隔离，Linux 保持 `.sh`，Windows 使用 `.exe/.bat/.cmd/.ps1`。
- `platform()` 是 OS primitive 的全局高影响入口：Linux 返回 `LinuxPlatform`，Windows 返回 `WindowsPlatform`，调用方契约保持一致。
- Activity 后端仍保存原始结构化调用信息；Web `/activity` 通过 `min-w-0/max-w-full/break-all/overflow-auto` 约束长 JSON、长 ID 和长命令，不再让内容扩大页面宽度。

## 本轮影响评估

| 范围 | 风险 | 结论 |
|------|------|------|
| Browser transport | Critical | 统一 RPC 后通过前端 check/build 与运行态 RPC 验证 |
| Rust crate rename | High | 仅 CLI 和仓库内测试引用，已原子更新并全量编译 |
| Exec policy defaults | High | 影响所有命令执行，24 项 security tests 与 contract tests 全通过 |
| Windows platform restore | Critical | 仅恢复 Headless OS primitive；Linux 195 项全量回归通过，Windows target 因开发机未安装标准库暂无法交叉编译 |
| Activity long-content containment | Low | Svelte check 0 error/0 warning，生产 build 通过 |
| Docs/workflows/assets | Low | 不影响运行时 |

## 验证证据

- `npm run check`：0 errors、0 warnings。
- `npm run build`：通过，静态 Web Console 写入 `build/`。
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`：通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`：195 项通过。
- `cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools`：通过。
- Windows `cargo check --target x86_64-pc-windows-gnu` 已尝试，开发机缺少该 Rust target（`can't find crate for core`）；当前客户端又不支持权限 elicitation，无法执行 `rustup target add`。
- `npm run check`：0 errors、0 warnings；`npm run build`：通过。

## GitNexus 状态

最终 `gitnexus detect-changes --scope unstaged` 可用；当前整棵未提交工作树包含 History Session v2 与 Windows/Activity 两轮改动，结果为 CRITICAL，主要由全局 `platform()`、History bootstrap、统一 call_tool、exec policy 和 tunnel lifecycle 公共链产生。提交前如继续修改源码，应再次执行 detect-changes。

## 剩余风险

- Windows x86_64 源码支持已恢复，但本开发机尚未完成真实 Windows target 编译或 Windows 实机 smoke；需要在 Windows CI/机器补这一层验证。
- Windows 当前不内置 Windows Service installer；前台运行或使用外部 service manager。Linux systemd user service 保持可选。
- 保留的配置目录名 `coding-tools-mcp-desktop` 是已有数据兼容边界，不代表仍有桌面 runtime。
- Web Admin 已有独立管理员认证，但默认监听仍是 HTTP；应放在可信 LAN/VPN 或 HTTPS 反向代理之后。
