# 任务清单：Linux Headless-only

## 交付物清单

- Browser-only Web Console 和单一 `/api/rpc` transport。
- 唯一 Linux `coding-tools` binary。
- 删除 Tauri/Windows/macOS 代码和依赖。
- 更新的 Headless-only README、项目上下文和规格。
- 前端、Rust、release、运行态与影响检测证据。

## 任务列表

- [x] 1.1 完成 Tauri/transport/platform 影响分析
  - **证据块**: GitNexus 标记 `invokeCommand` CRITICAL、`isTauriRuntime` MEDIUM，桌面 `run` 仅 LOW。
- [x] 1.2 通过规格校验
  - **证据块**: `check_spec linux-headless-only` 已通过，0 error、0 warning。
- [x] 2.1 删除桌面前端路由和 Tauri transport/dialog
  - **证据块**: 保留页面统一使用 `/api/rpc`，旧 desktop routes、dialog API 和 runtime 分支已删除。
- [x] 2.2 删除 npm Tauri 依赖并更新 lockfiles
  - **证据块**: `package.json` 只包含 browser Web Console 依赖，npm/pnpm lockfile 已更新。
- [x] 3.1 删除 Rust desktop entry/commands/config/assets
  - **证据块**: desktop binary、commands、Tauri config/capabilities/icons 和桌面 release workflows 已删除。
- [x] 3.2 将 Cargo/build/lib/async runtime 收敛为唯一 headless binary
  - **证据块**: Cargo 唯一 binary 为 `coding-tools`，Web assets 始终嵌入，runtime 为纯 Tokio。
- [x] 4.1 删除 Windows/macOS platform 实现
  - **证据块**: platform selector 固定 Linux，非 Linux 源码目录和条件分支已删除。
- [x] 4.2 清理 runtime/tools/tunnel 跨平台分支和依赖
  - **证据块**: process、exec、session、history、Cloudflare 和 FRP 已收敛为 Linux 行为，Windows release assets 与 ZIP 依赖已删除。
- [x] 5.1 更新 README 与项目上下文
  - **证据块**: README 与核心 project-context 已改为 Linux CLI、Browser Web Console 和单 Gateway 架构。
- [x] 6.1 运行前端和 Rust 全量验证
  - **证据块**: frontend check/build 通过；Rust all-targets 共 184 项测试通过。
- [ ] 6.2 构建并重启 release，验证 Web/Gateway/Cloudflare/OAuth
  - **证据块**: 隔离 release 的 Web/RPC/Gateway/health/SIGINT smoke 已通过；生产进程运行于 abandoned SSH session scope，尚未替换，现有 PID/URL 保持不变。
- [ ] 6.3 GitNexus detect-changes 和最终审查
  - **证据块**: 最终源码/调用点审查已完成；GitNexus managed install 因 ONNX HTTP 302 失败，当前 CLI 也未注册 detect-changes，待 runtime 恢复后补跑。

## 需求覆盖矩阵

| 需求 | 任务 |
|------|------|
| FR-1 | 3.1, 3.2, 6.1 |
| FR-2 | 2.1, 2.2, 6.1 |
| FR-3 | 3.2, 4.1, 4.2, 6.2 |
| FR-4 | 2.1, 3.1, 4.1, 5.1, 6.3 |

## 文件变更清单

| 范围 | 操作 | 主要路径 |
|------|------|----------|
| Web | 修改/删除 | `src/routes`、`src/lib/api`、桌面组件 |
| npm | 修改 | `package.json`、两个 lockfile、Vite/Svelte 配置 |
| Rust desktop | 删除 | `src/main.rs`、`commands/`、Tauri config/assets |
| Rust build | 修改 | `Cargo.toml`、`Cargo.lock`、`build.rs`、`lib.rs`、`async_runtime.rs` |
| Platform | 删除/修改 | `platform/windows`、`platform/macos`、Linux platform selector |
| Runtime | 修改 | tools、runtime、tunnel 的非 Linux 分支 |
| Docs | 修改/新增 | README、project-context、Headless-only spec |
