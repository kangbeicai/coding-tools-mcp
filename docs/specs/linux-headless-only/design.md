# 设计文档：Linux Headless-only

## 概述

本设计通过先收敛 Web transport、再删除 Tauri 壳、最后删除非 Linux 平台分支的顺序，把双运行时产品转换为单一 Linux Headless 服务。

**对应需求:** FR-1、FR-2、FR-3、FR-4

## 技术方案

| 层 | 调整 | 保留能力 |
|----|------|----------|
| Web | 单一 `/api/rpc` transport，删除桌面路由和 Tauri dialog | Gateway、密钥、工作区管理 |
| Rust entry | 只保留 `src/bin/coding-tools.rs` | Headless CLI、systemd、Web Admin |
| Build | 无 feature 分叉，始终嵌入 Web assets | 单 binary release |
| Platform | 固定 LinuxPlatform | 端口、进程、配置、tunnel binary discovery |
| Tunnel/tools | 删除 Windows/macOS 分支 | Linux FRP/Cloudflare 和工具执行 |

## 架构

```text
Browser
  -> embedded Web Console
  -> POST /api/rpc
  -> Admin RPC
  -> AppState
       -> Workspace store
       -> Gateway listener
       -> Managed FRP/Cloudflare

ChatGPT/MCP client
  -> /mcp or /w/<workspace-id>/mcp
  -> Gateway session binding
  -> Workspace ToolContext
```

## 设计决策

### D-1: 保留现有 `src-tauri` 路径

删除 Tauri 功能但暂不移动 Rust crate，避免大量无价值路径变更。目录名不代表仍依赖 Tauri。

### D-2: Web transport 单一路径

`src/lib/api/transport.ts` 删除 `isTauriRuntime` 和 `tauriInvoke`，所有保留 API 使用 `/api/rpc`。旧桌面路由和其直接 Tauri API 一并删除，不为不可达桌面功能扩张 RPC。

### D-3: Rust 唯一 binary

`Cargo.toml` 删除 desktop feature/binary 和 Tauri dependencies；`coding-tools` 不再要求 feature。`build.rs` 始终生成 embedded assets。

### D-4: Linux-only 平台

删除 `platform/windows`、`platform/macos`，`create_platform()` 只返回 LinuxPlatform。保留旧配置目录名称和 legacy import。

### D-5: 分阶段清理跨平台分支

先删除会阻止依赖移除的 Windows/macOS代码和依赖，再清除 Linux 编译中不可达的 `cfg(windows)`/macOS helpers。每一阶段都运行 cargo check，避免一次性失去定位能力。

## 文件变更

### 删除

- `src-tauri/src/main.rs`
- `src-tauri/src/commands/`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/`
- `src-tauri/icons/`
- `src-tauri/src/platform/windows/`
- `src-tauri/src/platform/macos/`
- `src/routes/workspace/[id]/+page.svelte`
- `src/routes/settings/{general,frp,software}/+page.svelte`
- Tauri-only frontend API、WebView guard 和无剩余调用组件

### 修改

- `src-tauri/Cargo.toml`、`Cargo.lock`、`build.rs`、`src/lib.rs`、`src/async_runtime.rs`
- Linux platform/runtime/tools/tunnel 的跨平台分支
- `package.json`、lockfiles、Vite/Svelte说明
- `src/routes/+layout.svelte`、`src/lib/api/transport.ts`、Web 工作区表单
- README 和 project-context 文档

## 文件结构

```text
src/                         browser-only Svelte Web Console
src-tauri/src/bin/           coding-tools CLI entry
src-tauri/src/admin/         embedded Web + /api/rpc
src-tauri/src/gateway/       global multi-workspace MCP Gateway
src-tauri/src/platform/linux Linux process/path/network primitives
src-tauri/src/tools/         workspace tools
src-tauri/src/tunnel/        FRP/Cloudflare
docs/specs/linux-headless-only/
```

## 验证

```bash
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

运行新 release 后验证 Web 200、Gateway RPC、exposure 状态、公网 `/mcp` 和 OAuth metadata。

## 风险

| 风险 | 等级 | 缓解 |
|------|------|------|
| `invokeCommand` 39 个直接调用者 | Critical | 先删除不可达路由，再统一 transport，执行 Svelte check/build |
| 平台抽象影响 tunnel/tools | High | 保留 Linux trait 实现，分阶段 cargo check/test |
| 删除 desktop commands 误删核心逻辑 | High | 仅删除 wrapper；Gateway/Web 使用的实现位于独立模块 |
| 配置目录改名导致数据丢失 | High | 本轮不改目录名 |
| Web assets 未构建 | Medium | release 前强制 npm build 并做独立目录启动验证 |
