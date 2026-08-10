# 设计文档：新版 Web Gateway Cloudflare 体验

## 概述

本设计覆盖 Gateway Web 设置页、Gateway listener 重启边界和 Web 工作区动态路由加载。Quick URL 作为运行态 effective URL 回填页面，不永久覆盖 canonical URL；listener 重启保留独立 tunnel；工作区参数变化触发带竞态保护的重新加载。

**对应需求:** FR-1、FR-2、FR-3、FR-4、NFR-1、NFR-2、NFR-3

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 配置同步 | 启动 exposure 前调用 `setGatewayExposure` | 保证后端读取 Web 当前选择 | FR-1 |
| URL 回填 | 使用 `GatewayExposureStatus.effectivePublicUrl` | 它是当前进程的权威地址 | FR-1, FR-3 |
| 重启 | 复用 `restartGateway`，后端只替换 listener | 保留独立运行的 exposure PID 与 URL | FR-2 |
| 工作区切换 | `$effect` 捕获 `workspaceId` + generation | 参数导航不会重新触发 `onMount`，且异步结果可能乱序 | FR-4 |

### 架构设计

```text
Web Gateway 设置页
  -> 选择 Cloudflare Quick
  -> set_gateway_exposure
  -> start_gateway_exposure
  -> effectivePublicUrl
  -> 更新 Gateway Listener 运行态 OAuth/MCP base URL
  -> 页面公网 URL + 公网 MCP 展示

重启 Gateway
  -> restart_gateway
  -> 保留仍存活的 managed exposure
  -> 仅停止并重建 Gateway listener
  -> 新 listener 初始使用 exposure effective URL
  -> get_gateway_exposure_status
  -> 页面继续显示相同公网地址

选择工作区
  -> /web/workspace/[id] 参数变化
  -> $effect 捕获新 id 并清除旧 profile
  -> 带 generation 的异步 load
  -> 仅最新请求更新页面与 last workspace
```

## 数据模型

不新增持久化字段。

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| `GatewayConfig.publicUrl` | string | canonical origin | 后端保存的稳定地址 |
| `GatewayExposureStatus.effectivePublicUrl` | string | 运行态 | FRP/Cloudflare 当前实际地址 |

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| `setGatewayExposure` | 现有前端 API | 当前 exposure 草稿 | void | FR-1 |
| `startGatewayExposure` | 现有前端 API | 无 | exposure status | FR-1 |
| `restartGateway` | 现有前端 API | 无 | gateway status | FR-2 |
| `applyExposureStatus` | 新增页面函数 | exposure status | 无 | FR-1, FR-3 |
| `GatewayProcess.set_public_url` | 新增 Rust 方法 | effective/canonical URL | 无 | FR-1, FR-3 |
| `spawn_listener` | Rust Gateway listener | 可选初始运行态公网 URL | GatewayProcess | FR-2 |
| Web workspace `load` | Svelte 页面函数 | 捕获的 workspace id | 页面状态 | FR-4 |

## 文件结构

```text
docs/specs/headlessweb-console/
├── requirements.md
├── design.md
└── tasks.md
src/routes/settings/gateway/+page.svelte
src/routes/web/workspace/[id]/+page.svelte
src-tauri/src/gateway/service.rs
src-tauri/src/gateway/listener.rs
src-tauri/src/gateway/exposure.rs
```

## 设计决策

### 决策 1: Quick URL 只做运行态回填（关联需求: FR-3）

**问题**: Quick Tunnel 每次重启都会生成新地址，持久化会留下失效 canonical URL。

**选项**:
1. 写入后端 `GatewayConfig.publicUrl`: 页面刷新自然保留，但进程重启后会失效。
2. 从 exposure 状态动态回填: 页面始终显示当前地址，停止后恢复 canonical。

**决策**: 选择从 exposure 状态动态回填。

**理由**: 满足用户直接复制使用的需求，同时不破坏固定公网身份。

Gateway Listener 内部使用共享 `RwLock<String>` 保存当前运行态公网 URL。Managed exposure 成功启动后写入 effective URL，停止时恢复 canonical URL，使 OAuth 元数据、授权、Token 交换和访问令牌校验始终使用同一当前地址。

### 决策 2: 启动动作隐式保存 exposure（关联需求: FR-1）

**问题**: Gateway 运行时全局“保存配置”按钮不可用，但后端允许在 exposure 停止时单独更新 exposure 配置。

**决策**: 启动 managed exposure 前保存 exposure 和相关 Token。

**理由**: 用户选择 Cloudflare 后可以直接点击启动，符合 Web 操作直觉。

### 决策 3: Gateway 重启只替换 listener（关联需求: FR-2）

**问题**: 完整停止 Gateway 会杀掉 managed exposure；Quick Tunnel 因此更换随机 URL，Named/FRP 也产生不必要中断。

**决策**: 显式停止保持“exposure 后 listener”的完整关闭语义；重启仅停止 listener，保留存活 exposure，并在新 listener 启动前注入 effective URL。若 listener 启动失败，清理保留的 exposure。

**理由**: tunnel 的目标是固定本地端口，与 listener 进程身份无关，两者应保持独立生命周期。

### 决策 4: Web 工作区按路由参数响应式加载（关联需求: FR-4）

**问题**: SvelteKit 在同一动态路由仅参数变化时复用组件，`onMount` 不会再次执行。

**决策**: 使用 `$effect` 同步读取 `workspaceId`，立即清除旧 profile，并使用递增 generation 丢弃过期异步结果。

**理由**: 与现有桌面工作区页的成熟实现一致，同时避免旧工作区被误保存或删除。

## 测试策略

- `npm run check` 验证 Svelte/TypeScript。
- `npm run build` 验证 Web 静态资源。
- `cargo test --manifest-path src-tauri/Cargo.toml gateway` 验证 Gateway 相关测试。
- `node --test tests/web-workspace-navigation.test.mjs` 验证 Web 工作区参数响应契约。
- `cargo build --release --manifest-path src-tauri/Cargo.toml` 构建实际运行二进制。
- 重启后验证新 PID、`28767` 监听、Web Admin HTTP 响应和 Gateway 状态 RPC。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| listener 重启失败时 tunnel 仍暴露 | 高 | 启动失败路径主动停止保留的 exposure |
| 快速切换工作区导致旧请求覆盖新页面 | 高 | generation 与当前 id 双重校验 |
| 用户误把临时 URL 当固定配置 | 中 | 文案明确“当前临时地址，不永久保存” |
| 远程重启短暂断连 | 中 | 按钮仅 Gateway 运行时出现并显示处理中状态 |

## 检查清单

- [x] 技术方案沿用现有 Gateway 架构
- [x] 覆盖全部 FR
- [x] 文件路径来自真实代码库
- [x] 数据边界和 API 契约明确
- [x] 设计决策与测试策略完整
