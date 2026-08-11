# 设计文档：MCP Activity Monitor

## 概述

本设计覆盖 FR-1、FR-2、FR-3、FR-4、FR-5 以及 NFR-1、NFR-2、NFR-3。实现采用进程内 `ActivityStore` + Axum SSE + 现有 `/api/rpc` 快照读取，不改变 MCP 协议。

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| Trace Store | Rust `Arc<ActivityStore>` + `Mutex`/ring buffer | 与单进程架构一致，无外部依赖 | FR-1, FR-3 |
| Realtime | Tokio `broadcast` + Axum SSE | 单向 Server→Browser，EventSource 原生重连 | FR-2 |
| Admin snapshot | 现有 `/api/rpc` 新 command | 复用现有管理 transport | FR-3 |
| Frontend | Svelte Activity page + EventSource | 与现有 browser-only SPA 一致 | FR-2, FR-3 |
| Admin Auth | Argon2 password hash + in-memory opaque session + HttpOnly cookie | 用访问控制保护原始 Activity 数据，不修改调试内容 | FR-5 |

### 架构设计

```text
ChatGPT / MCP client
  -> Gateway listener dispatch_request
      -> ActivityStore.begin(trace)
      -> gateway::server::handle_request
      -> ActivityStore.complete(trace, response)
          -> ring buffer snapshot
          -> broadcast ActivityEvent

exec_command response(status=running, session_id)
  -> ActivityStore.retained_processes
  -> later read_output/write_stdin/kill_session correlates by session_id

Browser Web Console
  -> GET/POST /api/auth/* (首次设置、登录、退出、状态)
  -> HttpOnly Admin session cookie
  -> POST /api/rpc list_activity/get_activity
  -> GET /api/activity/events (SSE)
  -> /activity page updates Active Calls / Processes / Recent / Detail
```

`ActivityStore` 由 `AppState` 持有，因此 Admin listener 与当前 Gateway 实例访问同一份状态；Gateway restart 不会要求 Web 页面切换对象。`spawn_listener` 接收 `Arc<ActivityStore>` 并放入 `ListenerState`。

## 数据模型

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| `ActivityTrace.trace_id` | String | 唯一 | Gateway 自生成 monotonic id |
| `session_id` | Option<String> | 原始值 | OpenAI session 或 transport session |
| `workspace_id/name` | Option<String> | 完成后可补全 | 当前路由工作区 |
| `method/tool` | String | 有界 | MCP method 与 tool name |
| `status` | enum-like String | running/completed/failed | MCP 生命周期 |
| `started_at_ms/finished_at_ms/duration_ms` | u64/Option | epoch + duration | 排序和 UI |
| `request/response/error` | serde_json::Value | 原始内容；超限时整体截断 | 详情数据 |
| `operation_id/process_session_id` | Option<String> | exec 专用 | retained process 关联 |
| `ActivityProcess.status` | String | running/exited/failed/killed/timeout | 底层进程生命周期 |

Store 默认最多保留 1000 条 Trace；request/response 各最多约 16 KiB。超过限制用带 `truncated=true` 的摘要对象代替。

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| Admin RPC | `list_activity` | filters + limit | traces + activeProcesses | FR-2, FR-3, FR-4 |
| Admin RPC | `get_activity` | traceId | ActivityTrace/null | FR-3 |
| SSE | `GET /api/activity/events` | 无 | `activity.started/updated/completed` JSON events | FR-2 |
| Admin Auth | `GET /api/auth/status` | cookie | configured/authenticated/username | FR-5 |
| Admin Auth | `POST /api/auth/setup` | username/password | 创建首个管理员并登录 | FR-5 |
| Admin Auth | `POST /api/auth/login` | username/password | 创建 Admin session cookie | FR-5 |
| Admin Auth | `POST /api/auth/logout` | cookie | 撤销 Admin session | FR-5 |
| Store | `begin_trace(...)` | request context | trace_id | FR-1 |
| Store | `complete_trace(...)` | trace_id + response | void | FR-1, FR-4 |

## 文件结构

```text
src-tauri/src/
├── activity.rs                         # 新增：TraceStore、原始数据、进程关联、事件
├── app_state.rs                        # 修改：持有 Arc<ActivityStore>
├── gateway/listener.rs                 # 修改：请求 begin/complete、trace_id
├── gateway/service.rs                  # 修改：启动 Gateway 时传入 store
├── gateway/mod.rs                      # 修改：导出所需类型
├── admin/auth.rs                       # 新增：Argon2 凭据验证与 session store
├── admin/listener.rs                   # 修改：登录路由、Admin 访问控制、SSE route
└── admin/rpc.rs                        # 修改：list/get activity RPC
src/
├── lib/api/activity.ts                 # 新增：RPC + EventSource helpers
├── lib/api/adminAuth.ts                # 新增：Admin 登录/设置/退出 API
├── lib/types.ts                        # 修改：Activity DTO 类型
├── routes/+layout.svelte               # 修改：Activity 导航
├── routes/activity/+page.svelte        # 新增：实时 Activity 页面
└── routes/login/+page.svelte           # 新增：首次设置/登录页面
```

## 设计决策

### 决策 1: 使用 SSE 而不是 WebSocket（FR-2）

**问题**: Activity 是高频 Server→Browser 状态更新，但浏览器反向操作仍可走 `/api/rpc`。
**选项**: 轮询；SSE；WebSocket。
**决策**: SSE。
**理由**: 单向语义匹配、实现更轻、EventSource 自动重连；列表 RPC 作为首次加载与丢事件后的快照恢复。

### 决策 2: Trace 与 retained process 分层（FR-4）

**问题**: `exec_command` MCP response 可能已完成而命令继续运行。
**决策**: Trace 只表示一次 MCP request/response；单独维护 `ActivityProcess`，通过 `session_id` 与后续控制/读输出调用关联。
**理由**: 避免把“工具调用已返回”和“命令已结束”混成一个状态。

### 决策 3: 原始 Trace + Admin 登录边界（FR-5）

**问题**: 调试 MCP 时，脱敏会隐藏恰好需要核对的真实参数和返回值；但原始 Trace 可能包含凭据。
**决策**: ActivityStore 保留原始 request/response/error，只保留有界大小限制；安全边界改为独立 Admin 登录。密码使用 Argon2 哈希持久化，登录后创建内存 opaque session，通过 HttpOnly、SameSite=Strict cookie 访问管理面。
**理由**: 可观测性数据保持真实，同时未认证用户无法读取 `/api/rpc` 和 SSE；MCP/OAuth 数据面与 Admin 登录完全分离。

## 测试策略

- Rust 单测：唯一 trace id、原始 payload 保留、截断、begin/complete、ring buffer、exec retained process 关联与终态更新；Admin Argon2 校验、session 创建/撤销/过期。
- Gateway 单测/现有测试：MCP response 结构不改变，trace 捕获不影响 initialize/tools/call。
- Admin 单测：list/get RPC 和 SSE route 可构建、Gateway 未运行时仍可读取 store。
- Frontend：`npm run check`、`npm run build`；验证 EventSource 生命周期、筛选和空态。
- 全量回归：`cargo test --manifest-path src-tauri/Cargo.toml --all-targets`、`cargo check`，最后 GitNexus detect-changes。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Gateway 热路径增加锁竞争 | 中 | 小型内存对象、快速锁、1000 条上限，不做磁盘同步写 |
| request/response 含敏感数据 | 高 | Admin API/SSE 强制登录；密码只存 Argon2 hash；session 使用 HttpOnly cookie；明确 Web Admin 不应直接暴露到不可信公网 |
| SSE 客户端错过事件 | 低 | Activity RPC 提供权威快照；重连后重新加载 |
| `exec_command` 进程状态只能通过后续 tool result 更新 | 中 | 明确为“已知最近状态”，后续 read/kill/write 自动更新；本轮不引入独立 SessionStore watcher |
| Admin 登录首次设置被绕过 | 高 | 未配置时只允许 `/login` 与 `/api/auth/*`，setup 原子检查 password_hash 为空；配置后 setup 返回冲突 |

## 检查清单

- [x] 技术方案与现有单进程 Linux Web 架构一致
- [x] 所有 FR 均有设计覆盖
- [x] 文件路径来自真实代码库
- [x] 数据模型和接口契约明确
- [x] 关键决策已记录
- [x] 测试策略可验证验收标准
