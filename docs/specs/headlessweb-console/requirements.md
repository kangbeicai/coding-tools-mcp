# 需求文档：新版 Web Gateway Cloudflare 体验

## 功能概述

新版 headless Web Console 应允许用户直接选择 Managed Cloudflare Quick Tunnel 并启动公网暴露，随后自动展示 cloudflared 分配的临时公网 URL；Gateway 运行中还应提供明确的重启按钮，并在 listener 重启时保持独立运行的公网 exposure；选择工作区后页面内容应立即切换。

## 历史经验与坑

- **可复用经验**: `start_gateway_exposure_service` 已返回 `effectivePublicUrl`，`restart_gateway` Admin RPC 与前端 API 已存在。
- **必须规避的坑**: Web 草稿中的 exposure 模式未保存时直接点击启动，后端仍读取旧模式；Quick URL 在 cloudflared 重启后会变化，不能当作长期 canonical URL 永久写死。

## 术语定义

- **Canonical URL**: 用户配置的稳定公网身份地址，适用于 FRP、反向代理或 Named Tunnel。
- **Effective URL**: 当前公网暴露进程实际可访问的地址；Quick Tunnel 每次启动可能不同。

## 范围边界

**In Scope（本次要做）**
- 启动 managed exposure 前保存 Web 当前选择的 exposure 配置。
- Cloudflare Quick 启动或页面刷新后，将有效临时 URL 显示在公网 URL 字段和公网 MCP 状态中。
- Gateway 运行时提供重启按钮，重启后刷新 Gateway 与 exposure 状态。
- Gateway listener 重启时保留仍在运行的 managed exposure 进程和有效公网 URL。
- Web Console 切换工作区时立即加载目标工作区，并阻止过期请求覆盖当前选择。
- 构建并重启当前 headless Web 服务。

**Out of Scope（本次不做）**
- 不将 Quick Tunnel 临时 URL永久保存为 canonical URL。
- 不改变 Cloudflare Named Tunnel、FRP、LAN 或 OAuth 协议实现。
- 不新增管理员认证机制。

## 需求列表

### FR-1: 直接启动 Cloudflare Quick Tunnel

**优先级:** Must
**用户故事:** 作为 Web Console 用户，我想选择 Cloudflare Quick Tunnel 后直接启动公网暴露，以便无需额外保存步骤。

#### 验收标准（EARS）

1. WHEN 用户选择 Cloudflare 并点击启动公网暴露 THEN 系统 SHALL 先保存当前 exposure 配置再启动 cloudflared。
2. WHEN cloudflared 返回 Quick URL THEN 系统 SHALL 将其显示到公网 URL 字段和当前有效 MCP 地址。
3. WHEN ChatGPT 通过 Quick URL读取 OAuth 元数据 THEN issuer、resource、authorization endpoint 和 token endpoint SHALL 使用当前 Quick URL。
4. IF 启动失败 THEN 系统 SHALL 保留现有状态并显示错误提示。

### FR-2: Web 重启 Gateway

**优先级:** Must
**用户故事:** 作为远程管理用户，我想在 Web 页面重启 Gateway listener，同时保持公网 tunnel 稳定。

#### 验收标准（EARS）

1. WHILE Gateway 正在运行 THE 系统 SHALL 显示“重启 Gateway”按钮。
2. WHEN 用户点击重启 THEN 系统 SHALL 调用现有 `restart_gateway`，并刷新 Gateway 与 exposure 状态。
3. WHILE managed exposure 进程仍在运行 WHEN Gateway listener 重启 THEN 系统 SHALL 保留 exposure PID 与 effective URL，并将该 URL 注入新的 listener。
4. WHEN 用户显式停止 Gateway THEN 系统 SHALL 继续先停止 managed exposure，避免留下无后端的公网入口。
5. IF listener 重启失败 THEN 系统 SHALL 停止此前保留的 managed exposure。

### FR-3: 保持临时地址与 canonical 配置边界

**优先级:** Must
**用户故事:** 作为同时使用临时和固定公网地址的用户，我想避免临时地址污染固定配置。

#### 验收标准（EARS）

1. WHILE Quick Tunnel 运行 THE 系统 SHALL 在页面中优先显示 effective URL。
2. WHEN Quick Tunnel 停止 THEN 系统 SHALL 恢复显示后端保存的 canonical URL。
3. WHILE 使用 FRP、Named Tunnel 或非托管公网方式 THE 系统 SHALL 保持现有配置和显示逻辑。

### FR-4: 工作区切换立即更新

**优先级:** Must
**用户故事:** 作为 Web Console 用户，我想在侧栏选择工作区后立即看到目标工作区，以免误改前一个工作区。

#### 验收标准（EARS）

1. WHEN 用户从一个 `/web/workspace/[id]` 切换到另一个工作区 THEN 系统 SHALL 不依赖浏览器刷新而重新加载目标工作区。
2. WHILE 新工作区正在加载 THE 系统 SHALL 清除旧 profile，避免旧工作区的保存或删除操作仍可触发。
3. IF 较早发起的加载晚于当前加载返回 THEN 系统 SHALL 丢弃过期结果。

## 非功能需求

- **NFR-1（性能）**: 每次操作只增加必要的配置保存与状态刷新 RPC，不新增轮询。
- **NFR-2（安全）**: Cloudflare Token 继续通过 Secret API 管理，不写入页面日志或普通配置。
- **NFR-3（兼容性）**: Tauri IPC 与 Web Admin RPC 使用同一现有 API；前端检查、生产构建和 Rust release 构建通过。

## 依赖关系

- 依赖 `start_gateway_exposure_service` 返回当前 `effectivePublicUrl`。
- 依赖现有 `restart_gateway` Tauri/Admin RPC。
- 依赖 headless 进程在端口 `28767` 提供 Gateway，并由 Web Admin 暴露管理页面。

## 检查清单

- [x] 已消化现有代码中的 canonical/effective URL 边界
- [x] 核心场景与异常场景完整
- [x] FR ID 唯一并可追踪
- [x] 验收标准可验证
- [x] 优先级、范围和依赖明确
- [x] 非功能需求明确
