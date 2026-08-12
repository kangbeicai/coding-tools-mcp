# 项目图谱洞察

更新时间：2026-08-12

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

GitHub 发布链重新建立为 Headless-only：`v*` tag 或手动指定已有 tag 后，先执行前端检查/构建与 Rust 全量测试，再由 GitHub-hosted Linux x86_64、Linux arm64、Windows x86_64 runner 原生构建三个 binary，最终聚合为 GitHub Release 并生成 `SHA256SUMS`。该流程不包含 Tauri bundle、安装器或 macOS desktop 产物。

## 高影响边界

- Web `invokeCommand` 是所有保留管理页面的 transport 汇合点，已统一为 `/api/rpc`。
- `async_runtime::spawn` 仍位于 Gateway、listener、session 和 tunnel 生命周期关键路径，保留纯 Tokio 实现。
- `spawn_cloudflare_tunnel` 同时服务 Gateway exposure 与旧 Workspace runtime tunnel supervisor。
- `spawn_cloudflare_tunnel` 调用异步 `ensure_cloudflared()`：本机已有 binary 时直接复用，缺失时才通过既有 GitHub mirror/download proxy 链下载官方平台 binary 到配置目录缓存；同步 tunnel 配置校验不产生下载副作用。
- `spawn_frpc`/`spawn_frpc_config` 覆盖 Workspace 聚合 FRP 与 Gateway FRP。
- `validate_command_for_workspace` 是所有 `exec_command` 的安全策略入口；默认 workspace script extension 按 target 隔离，Linux 保持 `.sh`，Windows 使用 `.exe/.bat/.cmd/.ps1`。
- `platform()` 是 OS primitive 的全局高影响入口：Linux 返回 `LinuxPlatform`，Windows 返回 `WindowsPlatform`，调用方契约保持一致。
- Headless foreground 摘要不再把 `0.0.0.0` 当作浏览器 URL：默认 Web Admin 区块输出可复制的 `Local=http://127.0.0.1:<port>` 和 OS 路由选择得到的实际 LAN IPv4；探测失败时只省略 LAN，不改变 listener。
- `Platform::app_config_dir()` 现在经 shared path helper 统一到 `coding-tools-mcp` 根目录；仅当 canonical 不存在且 legacy `coding-tools-mcp-desktop` 存在时整体 rename，失败则保持 legacy 可用，新旧并存禁止隐式 merge。
- History Session 派生状态升级为 MemoryState v3：`open_items` 只取当前 session 最新有效 checkpoint 的 `remaining_issues + next_actions` 快照；旧 session 仅保留在 references/search/read，不再跨会话累加污染当前状态。
- History Markdown parser 现在保留有效 records 的同时返回 malformed JSON diagnostics；validate 分离 `sequence_valid` 与 `archive_integrity_valid`，并通过最后写入的 `memory/snapshot.json` 检测派生 index/manifest/state 的 stale/incomplete/invalid 状态。
- Activity 后端仍保存原始结构化调用信息；Web `/activity` 通过 `min-w-0/max-w-full/break-all/overflow-auto` 约束长 JSON、长 ID 和长命令，不再让内容扩大页面宽度。

## 本轮影响评估

| 范围 | 风险 | 结论 |
|------|------|------|
| Browser transport | Critical | 统一 RPC 后通过前端 check/build 与运行态 RPC 验证 |
| Rust crate rename | High | 仅 CLI 和仓库内测试引用，已原子更新并全量编译 |
| Exec policy defaults | High | 影响所有命令执行，24 项 security tests 与 contract tests 全通过 |
| Windows platform restore | Critical | 仅恢复 Headless OS primitive；Linux 195 项全量回归通过，Windows target 因开发机未安装标准库暂无法交叉编译 |
| Activity long-content containment | Low | Svelte check 0 error/0 warning，生产 build 通过 |
| Headless Local/LAN startup summary | Low | `run_server` upstream impact LOW；URL/bind/filter 专项 6/6 |
| Config root migration | Low / Windows index unknown | trait/Linux impact LOW；迁移专项 4/4，Windows 复用 shared helper |
| History current-state projection | GitNexus private-symbol index unknown | 原因层改为 current-session latest checkpoint snapshot；History 专项 22/22、Rust 全量 213/213 |
| Docs/workflows/assets | Low | 不影响运行时 |

## 验证证据

- `npm run check`：0 errors、0 warnings。
- `npm run build`：通过，静态 Web Console 写入 `build/`。
- `cargo check --manifest-path src-tauri/Cargo.toml --all-targets`：通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`：213 项通过。
- `cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools`：通过。
- v0.1.34 GitHub Release 已在 native Windows x86_64 / Linux arm64 / Linux x86_64 runners 成功完成；本轮新增 config-root shared helper 的 Windows native 编译仍待下一次 Release/manual dry-run 再验证。
- `npm run check`：0 errors、0 warnings；`npm run build`：通过。

## GitNexus 状态

Headless 改动已对 `run_server`、`print_help` 和 `app_config_dir` 做修改前 impact。History 私有 helper 未被当前 GitNexus Rust 索引识别，已对公共调用边界做源码取证并以 staged `gitnexus detect-changes` 作为最终风险结论。

## 剩余风险

- Windows x86_64 已由 v0.1.34 GitHub Release runner 真实编译成功；本轮新增配置目录 helper 因本机无 Windows target，需由下一次 GitHub native Windows run 覆盖。
- Windows 当前不内置 Windows Service installer；前台运行或使用外部 service manager。Linux systemd user service 保持可选。
- canonical 配置根目录已去除 Desktop 历史命名：Linux / Windows 均使用 `coding-tools-mcp`。仅在新目录缺失时把 `coding-tools-mcp-desktop` 作为升级来源整体 rename；rename 失败则继续使用旧目录，新旧并存时新目录优先且不自动 merge。
- Web Admin 已有独立管理员认证，但默认监听仍是 HTTP；应放在可信 LAN/VPN 或 HTTPS 反向代理之后。
