# 需求文档：MCP Activity Monitor

## 功能概述

为 Linux Web Console 增加 MCP Activity Monitor，让用户在 ChatGPT 使用 Coding MCP 时实时看到当前调用、最近调用、请求参数、返回结果和命令运行状态，解决“看不到 ChatGPT 调了什么、返回了什么、当前卡在哪一步”的可观测性问题。

## 历史经验与坑

- **可复用经验**: 现有 Gateway 已在 `dispatch_request` 统一接收 MCP 请求，并使用 `mcp-requests.log` 记录基础调用信息；Web 管理面已统一通过 Admin listener 和 `/api/rpc`。
- **必须规避的坑**: JSON-RPC `id` 可能重复为 `0`，不能作为 trace 主键；Activity 为调试用途需要保留真实 request/response，因此必须用独立 Web Admin 登录保护这些数据，不能依赖内容脱敏作为安全边界。

## 术语定义

- **Trace**: 一次 MCP 请求在 Gateway 内的结构化调用记录，使用 Gateway 自生成的 `trace_id` 唯一标识。
- **Active Call**: 已收到但尚未完成 MCP response 的 Trace。
- **Retained Process**: `exec_command` 已向 ChatGPT 返回、但底层命令仍在 SessionStore 中运行的进程。

## 范围边界

**In Scope**
- Gateway 结构化 Trace、唯一 `trace_id`、session/workspace/tool/method/状态/耗时/request/response/error。
- 保留原始 request/response/error 内容，仅对超大结构做有界截断，便于真实调试。
- 识别 `exec_command` 返回的 `operation_id`/`session_id`/running 状态，并关联后续 `read_output`、`write_stdin`、`kill_session`。
- Web Admin 提供 Activity 列表/详情 RPC 和 SSE 实时事件流。
- Web Console 新增 Activity 页面，展示 Active Calls、Retained Processes、最近调用、详情和基本筛选。
- Web Admin 增加首次管理员设置、用户名/密码登录、HttpOnly session cookie 和退出登录；`/api/rpc`、Activity SSE 与管理页面必须要求有效 Admin session。

**Out of Scope**
- 模型隐藏思考过程、ChatGPT 消息正文抓取、完整会话录屏。
- WebSocket、外部 telemetry/数据库、跨机器集中日志。
- 多用户、RBAC、SSO、外部身份提供商和账号找回流程。

## 需求列表

### FR-1: 结构化捕获 MCP 调用

**优先级:** Must
**用户故事:** 作为 Coding MCP 用户，我想看到每次 MCP 调用的结构化记录，以便确认 ChatGPT 实际调用了什么。

#### 验收标准（EARS）
1. WHEN Gateway 收到 MCP 请求 THEN 系统 SHALL 生成不依赖 JSON-RPC `id` 的唯一 `trace_id` 并记录 method、tool、session、route、开始时间和原始 request。
2. WHEN 请求完成 THEN 系统 SHALL 更新 completion 状态、耗时、workspace、原始 response 或 error。
3. IF 同一 JSON-RPC `id` 被重复使用 THEN 系统 SHALL 仍为每次请求生成不同 `trace_id`。

### FR-2: 实时展示当前调用

**优先级:** Must
**用户故事:** 作为用户，我想实时看到正在执行的 MCP 调用，以便知道 ChatGPT 当前在做什么。

#### 验收标准（EARS）
1. WHEN Trace 开始、更新或结束 THEN Admin SHALL 通过 SSE 向已连接 Web Console 推送事件。
2. WHILE 一个或多个 Trace 处于 running 状态 THEN Activity 页面 SHALL 在顶部显示 Active Calls 及实时耗时。
3. IF SSE 连接断开 THEN 浏览器 SHALL 可依靠 EventSource 自动重连，并通过列表 RPC 重新同步当前快照。

### FR-3: 查看最近调用与详情

**优先级:** Must
**用户故事:** 作为用户，我想查看近期调用和详细 Request/Response，以便追踪一轮 ChatGPT 操作发生了什么。

#### 验收标准（EARS）
1. WHEN 用户打开 Activity 页面 THEN 系统 SHALL 返回有界的最近 Trace 列表。
2. WHEN 用户选择某条 Trace THEN 页面 SHALL 展示 request、response/error、workspace、session、状态和耗时。
3. WHEN 用户设置 workspace、session、tool 或状态筛选 THEN 页面 SHALL 仅显示匹配记录。

### FR-4: 区分 MCP 调用与底层 retained process

**优先级:** Must
**用户故事:** 作为用户，我想知道 `exec_command` 已返回但命令仍在运行，以免把 MCP response 完成误认为任务已经结束。

#### 验收标准（EARS）
1. IF `exec_command` response 表示 `status=running` 且包含 `session_id` THEN 系统 SHALL 单独记录 Retained Process，并关联来源 trace。
2. WHEN 后续 `read_output`、`write_stdin` 或 `kill_session` 携带相同 `session_id` THEN 系统 SHALL 将调用关联到该 Retained Process。
3. WHEN 后续结果表明进程 exited/failed/killed/timeout THEN 系统 SHALL 更新 Retained Process 的终态并从“当前运行”区域移出。

### FR-5: Web Admin 登录保护

**优先级:** Must
**用户故事:** 作为管理员，我想通过独立 Web 登录保护管理控制台，以便 Activity 可以保留真实调用数据而不向未认证访问者暴露。

#### 验收标准（EARS）
1. WHEN Admin 尚未设置密码 THEN Web Console SHALL 只允许访问登录/首次设置入口，并允许用户设置管理员用户名与密码；系统 SHALL 仅持久化 Argon2 密码哈希。
2. WHEN 用户以正确凭据登录 THEN 系统 SHALL 创建随机、有过期时间的服务端 session，并通过 HttpOnly、SameSite=Strict cookie 返回浏览器。
3. WHEN 未认证请求访问 `/api/rpc`、`/api/activity/events` 或管理页面 THEN 系统 SHALL 拒绝 API/SSE 或重定向到 `/login`，且 SHALL NOT 影响 `/mcp`、MCP OAuth 或 Gateway 公网链路。
4. WHEN 用户退出登录或 session 过期 THEN 后续 Admin API/SSE 请求 SHALL 不再被授权。
5. IF Activity request/response 超过配置内置上限 THEN 系统 SHALL 截断并标记 truncated，而不是无限保留；除大小限制外不得对 Trace 内容做脱敏或改写。

## 非功能需求

- **NFR-1（性能）**: 默认只保留最近 1000 条 Trace；单条 request/response 结构化序列化后各限制在约 16 KiB；Activity 记录不得阻塞工具执行的主路径。
- **NFR-2（安全）**: Admin 密码不得明文持久化；Admin session 仅存服务端内存并设置 HttpOnly、SameSite=Strict cookie；Activity 原始数据只能通过已认证 Admin API/SSE 读取。
- **NFR-3（兼容性）**: 不改变 MCP 协议、现有工具返回结构、OAuth/Cloudflare 行为；保持 Linux headless-only、Svelte 5/SvelteKit static SPA、Tokio/Axum 架构。

## 依赖关系

- `src-tauri/src/gateway/listener.rs` 统一 MCP 请求入口。
- `src-tauri/src/app_state.rs` 作为 Admin 与 Gateway 共享状态容器。
- `src-tauri/src/admin/listener.rs` 和 `src-tauri/src/admin/rpc.rs` 作为 Web 管理面。
- `src/lib/api/transport.ts` 和 Svelte layout 作为现有浏览器 transport / navigation。

## 检查清单

- [x] 已消化当前项目历史经验与安全边界
- [x] 需求覆盖核心与边界场景
- [x] 每条需求有唯一 ID
- [x] 验收标准使用 EARS 且可测
- [x] 已标注 MoSCoW 优先级
- [x] In/Out of Scope 明确
- [x] 非功能需求明确且有边界值
- [x] 依赖关系完整
