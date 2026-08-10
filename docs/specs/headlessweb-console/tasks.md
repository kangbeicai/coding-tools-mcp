# 任务清单：新版 Web Gateway Cloudflare 体验

## 概述

修复新版 Web Console 的 managed exposure、Gateway listener 重启和工作区动态切换体验。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 3 个规格文档
- **预计修改文件数**: 3 个实现文件
- **预计新增/修改函数数**: 8 个
- **交付物逐项列举**:
  1. `docs/specs/headlessweb-console/requirements.md`
  2. `docs/specs/headlessweb-console/design.md`
  3. `docs/specs/headlessweb-console/tasks.md`
  4. `src/routes/settings/gateway/+page.svelte`
  5. `src-tauri/src/gateway/listener.rs`
  6. `src-tauri/src/gateway/exposure.rs`
  7. `src-tauri/target/release/coding-tools` 构建产物

## 任务列表

### 阶段 1: 准备工作

- [x] 1.1 分析 Gateway exposure 与 Web Admin RPC 状态流
  - **证据块**: `gateway/exposure.rs:197-221` 已返回 cloudflared `public_url`；`settings/gateway/+page.svelte:107-120` 启动前未保存草稿；`api/gateway.ts:108-110` 已提供重启 API。
  - **涉及文件**: 只读分析
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 架构设计_

### 阶段 2: 核心实现

- [x] 2.1 启动 exposure 前保存当前 Web 选择并回填 effective URL
  - **证据块**: `settings/gateway/+page.svelte:132-152` 全局保存被 Gateway 运行状态锁定，但后端 `set_gateway_exposure_config` 只要求 exposure 已停止。
  - **涉及文件**: `src/routes/settings/gateway/+page.svelte`，约 30 行修改
  - _需求: FR-1, FR-3_ ｜ _设计: 决策 1、决策 2_
- [x] 2.2 增加 Gateway 重启按钮并刷新重启后的 Quick URL
  - **证据块**: `api/gateway.ts:108-110` 和 `admin/rpc.rs:192` 已完成 API 与 RPC，页面未导入或调用。
  - **涉及文件**: `src/routes/settings/gateway/+page.svelte`，约 20 行修改
  - _需求: FR-2_ ｜ _设计: 架构设计_
- [x] 2.3 将 managed exposure effective URL 注入 Gateway OAuth/MCP 运行态
  - **证据块**: 公网实测显示 Quick `/mcp` 返回 200，但 OAuth issuer/resource 仍指向旧 NAT canonical URL；`gateway/listener.rs:327-403` 的所有 OAuth 路径均读取启动时固定字符串。
  - **涉及文件**: `src-tauri/src/gateway/listener.rs`、`src-tauri/src/gateway/exposure.rs`，约 60 行修改
  - _需求: FR-1, FR-3_ ｜ _设计: 决策 1_
- [x] 2.4 Gateway listener 重启时保留 managed exposure
  - **证据块**: `restart_gateway_service` 原先调用完整 `stop_gateway_service`，会终止独立的 cloudflared/frpc；现拆分 listener-only stop，并在新 listener 构造时注入存活 exposure URL。
  - **涉及文件**: `src-tauri/src/gateway/service.rs`、`src-tauri/src/gateway/listener.rs`
  - _需求: FR-2_ ｜ _设计: 决策 3_
- [x] 2.5 Web 工作区切换响应动态路由参数
  - **证据块**: `/web/workspace/[id]` 原先只在 `onMount` 调用 `load`；同路由参数切换不会重挂载组件。
  - **涉及文件**: `src/routes/web/workspace/[id]/+page.svelte`、`tests/web-workspace-navigation.test.mjs`
  - _需求: FR-4_ ｜ _设计: 决策 4_

### 阶段 3: 集成测试与部署

- [x] 3.1 对照验收标准执行前端检查、Rust 测试和 release 构建
  - **证据块**: `package.json` 提供 `check`/`build`；`src-tauri/Cargo.toml` 定义 release 二进制。
  - **涉及文件**: 运行现有检查与构建，不新增测试文件
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 测试策略_
- [x] 3.2 优雅重启实际进程并验证 PID、端口和 Web 响应
  - **证据块**: 当前 `/proc/96429/exe` 指向本仓库 release 二进制，端口 `28767` 由该进程监听。
  - **涉及文件**: `src-tauri/target/release/coding-tools` 运行产物
  - _需求: FR-2_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1 完成后：确认问题是 Web 草稿未保存与 effective URL 未同步。
- [x] 阶段 2 完成后：确认 Quick URL、重启按钮和 canonical 回退行为正确。
- [x] 阶段 3 完成后：新 release PID `755178` 正在监听 `28766`/`28767`，Cloudflare Quick、Web 页面、OAuth 元数据与 `restart_gateway` RPC 验证通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、决策 2 | 1.1, 2.1, 2.3, 3.1 | 已完成 |
| FR-2 | 架构设计、决策 3 | 1.1, 2.2, 2.4, 3.1, 3.2 | 已完成 |
| FR-3 | 决策 1 | 1.1, 2.1, 2.3, 3.1 | 已完成 |
| FR-4 | 决策 4 | 2.5, 3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `docs/specs/headlessweb-console/requirements.md` | 新建 | 约 85 行 | 功能与验收标准 |
| `docs/specs/headlessweb-console/design.md` | 新建 | 约 100 行 | 状态同步与重启设计 |
| `docs/specs/headlessweb-console/tasks.md` | 新建 | 约 80 行 | 实现、构建和部署步骤 |
| `src/routes/settings/gateway/+page.svelte` | 修改 | 约 50 行 | Web 交互与状态回填 |
| `src-tauri/src/gateway/listener.rs` | 修改 | 约 45 行 | 动态 OAuth/MCP 公网 URL |
| `src-tauri/src/gateway/exposure.rs` | 修改 | 约 15 行 | exposure 启停同步运行态 URL |

## 检查清单

- [x] 交付物清单已填写
- [x] 任务标题具体并含证据
- [x] 文件与行数预算明确
- [x] 每项任务回链到 FR 与设计
- [x] 需求覆盖矩阵完整
- [x] 包含构建和实际重启验证
- [x] 全文无占位内容
