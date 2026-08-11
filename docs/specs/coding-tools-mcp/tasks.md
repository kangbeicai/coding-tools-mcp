# 任务清单：MCP Activity Monitor

## 概述

实现 Gateway 结构化追踪、Admin SSE/RPC 与 Svelte Activity 页面。所有实现保持现有 MCP 协议和 Linux headless-only 架构。

## 交付物清单（Scope-lock）

- **实现新建文件数**: 6 个；另新增 3 个规格文档
- **实现修改文件数**: 14 个
- **新增/修改函数数**: 约 35 个
- **交付物逐项列举**:
  1. `src-tauri/src/activity.rs`
  2. `src/lib/api/activity.ts`
  3. `src/routes/activity/+page.svelte`
  4. `src-tauri/src/app_state.rs`
  5. `src-tauri/src/gateway/listener.rs`
  6. `src-tauri/src/gateway/service.rs`
  7. `src-tauri/src/admin/listener.rs`
  8. `src-tauri/src/admin/rpc.rs`
  9. `src-tauri/src/lib.rs`
  10. `src-tauri/Cargo.toml`
  11. `src-tauri/Cargo.lock`
  12. `src/lib/types.ts`
  13. `src/routes/+layout.svelte`
  14. `docs/specs/coding-tools-mcp/requirements.md`
  15. `docs/specs/coding-tools-mcp/design.md`
  16. `docs/specs/coding-tools-mcp/tasks.md`
  17. `src-tauri/src/admin/auth.rs`
  18. `src/lib/api/adminAuth.ts`
  19. `src/routes/login/+page.svelte`
  20. `src-tauri/src/admin/mod.rs`
  21. `src-tauri/src/headless.rs`
  22. `src-tauri/src/settings/model.rs`
  23. `src/lib/api/transport.ts`

## 任务列表

### 阶段 1: 准备工作

- [x] 1.1 确认 Gateway、Admin 与 Web Console 的统一调用入口和安全边界
  - **证据块**: `src-tauri/src/gateway/listener.rs:243` 的 `dispatch_request` 已统一记录 request/completed；`src-tauri/src/admin/listener.rs:58` 只暴露 `/api/rpc`、`/api/health` 与静态页面；`src/routes/+layout.svelte:58` 定义管理导航。
  - **涉及文件**: 仅阅读，无产品代码改动
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 架构设计_

- [x] 1.2 确认 retained process 的现有返回契约
  - **证据块**: `src-tauri/src/tools/exec.rs:260-327` 在 `yield_time` 到达后返回 snapshot 并保留 session；`src-tauri/src/tools/exec.rs:468-510` 在结果中保留 `status`、`session_id`、`command_ok` 等状态。
  - **涉及文件**: 仅阅读，无产品代码改动
  - _需求: FR-4_ ｜ _设计: 决策 2_

### 阶段 2: 核心实现

- [x] 2.1 实现 ActivityStore，提供有界 Trace、原始 payload、截断、SSE broadcast 与 retained process 关联
  - **证据块**: `src-tauri/src/app_state.rs:7-14` 当前集中持有 data/runtime/gateway/exposure 共享状态，适合加入 `Arc<ActivityStore>`。
  - **涉及文件**: `src-tauri/src/activity.rs` 新增约 350 行；`src-tauri/src/lib.rs` 或模块入口约 2 行；`src-tauri/src/app_state.rs` 约 10 行
  - _需求: FR-1, FR-4, FR-5_ ｜ _设计: 数据模型、决策 2、决策 3_

- [x] 2.2 在 Gateway 请求入口接入 trace begin/complete，并在 Gateway service 注入共享 Store
  - **证据块**: `src-tauri/src/gateway/listener.rs:243-310` 目前在 `spawn_blocking(handle_request)` 前后记录文本日志，是最小侵入的追踪切入点。
  - **涉及文件**: `src-tauri/src/gateway/listener.rs` 约 80 行；`src-tauri/src/gateway/service.rs` 约 10 行
  - _需求: FR-1, FR-4_ ｜ _设计: 架构设计、API 设计_

- [x] 2.3 为 Admin 增加 Activity 快照 RPC 与 SSE 事件流，保持可信管理面边界
  - **证据块**: `src-tauri/src/admin/rpc.rs:76` 统一 dispatch command；`src-tauri/src/admin/listener.rs:58-64` 构建 Axum Admin Router。
  - **涉及文件**: `src-tauri/src/admin/rpc.rs` 约 45 行；`src-tauri/src/admin/listener.rs` 约 45 行
  - _需求: FR-2, FR-3_ ｜ _设计: API 设计、决策 1_

- [x] 2.4 新增 Web Activity 页面，实时展示 Active Calls、Retained Processes、Recent Traces 与详情筛选
  - **证据块**: `src/routes/+layout.svelte:58-78` 已有 Gateway/密钥导航；`src/lib/api/transport.ts` 是 RPC transport 汇合点。
  - **涉及文件**: `src/lib/api/activity.ts` 新增约 70 行；`src/lib/types.ts` 约 70 行；`src/routes/activity/+page.svelte` 新增约 300 行；`src/routes/+layout.svelte` 约 15 行
  - _需求: FR-2, FR-3, FR-4_ ｜ _设计: Frontend、API 设计_

- [x] 2.5 删除 Activity 脱敏并增加 Admin 首次设置、登录/session cookie 与受保护管理路由
  - **证据块**: `src-tauri/src/activity.rs` 已删除 `sanitize_value`/`sensitive_key`/`sanitize_command`，保存原始 request/response/session/command，仅保留 16 KiB 整体上限；`src-tauri/src/admin/auth.rs` 使用 Argon2 哈希与 12 小时内存 session；`src-tauri/src/admin/listener.rs` 的 `/api/rpc`、SSE 和管理页面均要求有效 HttpOnly、SameSite=Strict cookie，首次 `/login` 可初始化管理员。
  - **涉及文件**: `src-tauri/src/activity.rs`、`src-tauri/src/admin/auth.rs`、`src-tauri/src/admin/mod.rs`、`src-tauri/src/admin/listener.rs`、`src-tauri/src/settings/model.rs`、`src-tauri/Cargo.toml`、`src/lib/api/adminAuth.ts`、`src/lib/api/transport.ts`、`src/routes/+layout.svelte`、`src/routes/login/+page.svelte`
  - _需求: FR-1, FR-3, FR-5_ ｜ _设计: 决策 3、API 设计_

### 阶段 3: 集成测试

- [x] 3.1 对照 FR 验证 Rust TraceStore、Gateway/Admin 接入、Admin Auth 和 retained process 关联
  - **证据块**: Activity 单测已改为断言原始 payload/session/command 完整保留；Admin Auth 单测覆盖 Argon2 正误密码与 session 撤销；`cargo check --all-targets` 通过，`cargo test --all-targets` 共 189 项通过、0 失败。
  - **涉及文件**: `src-tauri/src/activity.rs` 内单测及既有相关测试
  - _需求: FR-1, FR-4, FR-5_ ｜ _设计: 测试策略_

- [x] 3.2 对照 FR 验证 Svelte 类型检查、静态构建和 SSE 页面行为
  - **证据块**: `npm run check` 为 0 errors / 0 warnings；`npm run build` 成功写入 `build/`。隔离端口 smoke 验证未登录 `/api/rpc` 与 SSE 均 401、`/activity` 307 到 `/login`；首次 setup 返回 session cookie；登录后 RPC 200 且原始 OpenAI session/request 完整可见；SSE 实际收到事件；logout 后旧 cookie 再次 401；配置只含 `$argon2...` 哈希且无测试明文密码。
  - **涉及文件**: 前端交付物，不新增独立测试框架
  - _需求: FR-2, FR-3_ ｜ _设计: 测试策略_

- [x] 3.3 运行 Rust 全量回归与 GitNexus detect-changes，确认影响范围符合预期
  - **证据块**: Rust 全量 189 项通过；release 构建通过；GitNexus 重建后 `detect-changes --scope unstaged` 覆盖 23 个文件、195 个符号、25 条执行流，风险为 critical，主要来自 Gateway/Admin listener、RPC 和 run_server 核心路径，均已被编译、全量测试与隔离运行态 smoke 覆盖。
  - **涉及文件**: 全部本轮改动
  - _需求: FR-1 至 FR-5_ ｜ _设计: 风险评估、测试策略_

## 检查点

- [x] 阶段 1 完成后：入口、生命周期差异、安全边界已确认
- [x] 阶段 2 完成后：Web 页面能通过 RPC 快照 + SSE 实时反映 Trace/Process
- [x] 阶段 3 完成后：前端 check/build、Rust test/check、release build、隔离认证/SSE smoke 与 GitNexus detect-changes 全部完成

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、数据模型 | 2.1, 2.2, 3.1 | 完成 |
| FR-2 | API 设计、决策 1 | 2.3, 2.4, 3.2 | 完成 |
| FR-3 | API 设计 | 2.3, 2.4, 3.2 | 完成 |
| FR-4 | 决策 2 | 2.1, 2.2, 2.4, 3.1 | 完成 |
| FR-5 | 决策 3 | 2.5, 3.1, 3.2 | 完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/activity.rs` | 新建 | 350 | Store、DTO、原始 payload、SSE event、测试 |
| `src-tauri/src/admin/auth.rs` | 新建 | 220 | Argon2、session cookie、认证状态 |
| `src-tauri/src/app_state.rs` | 修改 | 10 | 共享 ActivityStore |
| `src-tauri/src/gateway/listener.rs` | 修改 | 60 | Gateway trace lifecycle |
| `src-tauri/src/gateway/service.rs` | 修改 | 10 | Store 注入 |
| `src-tauri/src/admin/mod.rs` | 修改 | 2 | Admin Auth 模块接入 |
| `src-tauri/src/admin/listener.rs` | 修改 | 220 | Auth 路由、受保护 RPC/SSE/页面 |
| `src-tauri/src/admin/rpc.rs` | 修改 | 55 | list/get Activity RPC；Admin 配置输出不返回 password hash |
| `src-tauri/src/headless.rs` | 修改 | 10 | 启动时提示 Admin 登录/首次设置状态 |
| `src-tauri/src/settings/model.rs` | 修改 | 15 | Admin username/password_hash 持久配置 |
| `src-tauri/src/lib.rs` | 修改 | 2 | Activity 模块导出 |
| `src-tauri/Cargo.toml` | 修改 | 3 | tokio-stream、Argon2、rand_core/getrandom |
| `src-tauri/Cargo.lock` | 修改 | 自动 | 锁定 SSE 与 Argon2 相关依赖 |
| `src/lib/api/activity.ts` | 新建 | 70 | Web API/SSE helper |
| `src/lib/types.ts` | 修改 | 70 | Activity DTO |
| `src/routes/+layout.svelte` | 修改 | 15 | Activity navigation |
| `src/routes/activity/+page.svelte` | 新建 | 300 | Activity UI |
| `src/lib/api/adminAuth.ts` | 新建 | 80 | Admin Auth HTTP helper |
| `src/lib/api/transport.ts` | 修改 | 5 | Admin RPC 401 自动转登录页 |
| `src/routes/login/+page.svelte` | 新建 | 220 | 首次设置与登录 UI |

## 检查清单

- [x] Scope-lock 已明确
- [x] 每条任务标题具体且可验收
- [x] 每条任务包含证据块
- [x] 每条任务标注文件与行数预算
- [x] 每条任务回链 FR 与 design
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 3 包含逐项验收
- [x] 文档无占位符
