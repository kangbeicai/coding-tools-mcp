# 需求文档：Linux Headless-only

## 功能概述

将 Coding Tools MCP 收敛为 Linux 服务器产品：唯一可执行文件 `coding-tools` 提供嵌入式 Web Console、全局 MCP Gateway、工作区工具、OAuth/Bearer、FRP/Cloudflare exposure 和 systemd 服务。删除 Tauri 桌面应用、Windows/macOS 支持及桌面专用 UI/API。

## 范围

**保留**
- `/settings/gateway`、`/settings/keys`、`/web/workspace/[id]`。
- 工作区 CRUD、共享密钥、Gateway、健康检查、FRP Profile 列表和 managed exposure。
- 该历史规格最初要求保留 `~/.config/coding-tools-mcp-desktop` 以避免已有部署丢失数据；现已由 `coding-tools-headless` 规格的自动目录迁移方案取代，canonical 路径为 `~/.config/coding-tools-mcp/`。
- 现有 MCP 工具、Gateway 多工作区会话和 tunnel 实现。

**删除**
- Tauri desktop binary、commands、配置、capabilities、icons 和 npm/Cargo 依赖。
- `/workspace/[id]` 桌面页面、WebView 内存功能、原生文件/目录对话框。
- Windows/macOS platform 实现和桌面发布说明。
- 当前 Headless Web 不可达的 desktop-only 通用、FRP 编辑和软件管理页面。

## 需求列表

### FR-1: 唯一 Linux Headless 构建

1. WHEN 构建 Rust 项目 THEN 系统 SHALL 只生成 `coding-tools` binary。
2. WHEN 构建 release THEN Web assets SHALL 无条件嵌入二进制。
3. WHILE 编译非 Linux 目标 THE 系统 SHALL 给出明确的不支持错误。
4. WHEN 安装依赖 THEN 项目 SHALL 不再安装 Tauri、WebKit/GTK 或 Windows runtime 依赖。

### FR-2: 浏览器 Web Console

1. WHEN 前端调用管理命令 THEN 系统 SHALL 始终 POST `/api/rpc`，不得检测或调用 Tauri runtime。
2. WHEN 用户添加工作区 THEN 系统 SHALL 使用服务器绝对路径输入。
3. WHEN 用户选择工作区 THEN 系统 SHALL 统一导航到 `/web/workspace/[id]`。
4. WHILE 浏览器运行 THE 系统 SHALL 不加载 WebView 内存、桌面文件选择器或原生弹窗模块。

### FR-3: 保留核心服务能力

1. WHEN 用户管理工作区、共享密钥、Gateway 或 exposure THEN 现有 Web Admin 行为 SHALL 保持可用。
2. WHEN Gateway 重启 THEN 运行中的 managed exposure SHALL 保持独立生命周期。
3. WHEN 使用 Quick/Named Cloudflare THEN OAuth effective URL SHALL 保持现有语义。
4. WHEN 服务重启 THEN 现有 Linux 配置目录中的数据 SHALL 继续可读。

### FR-4: 删除非 Linux 桌面实现

1. WHEN 重构完成 THEN 仓库 SHALL 不再包含 Tauri desktop entry/config/capability/icon。
2. WHEN 重构完成 THEN Rust 平台选择 SHALL 固定为 LinuxPlatform。
3. WHEN 重构完成 THEN npm scripts 和文档 SHALL 不再提供 Windows/macOS/Tauri 构建方式。
4. WHEN 运行测试 THEN Linux tools、Gateway、tunnel 和 Web Admin 回归 SHALL 通过。

## 非功能需求

- **NFR-1**: `invokeCommand` 的 CRITICAL 调用面必须由 `npm run check`、Web build 和运行态 RPC 验证覆盖。
- **NFR-2**: 不在本轮迁移 Linux 配置目录名称。
- **NFR-3**: 删除代码优先于保留无调用的兼容层。
- **NFR-4**: 不新增 Docker、CI 或多租户账号隔离功能。

## 依赖关系

- FR-2 的浏览器 transport 必须在删除 Tauri npm 依赖前完成。
- FR-1 的 Cargo 收敛必须在删除 Rust desktop entry/commands 后完成。
- FR-4 的平台删除依赖 LinuxPlatform 已覆盖所有保留调用。
- FR-3 要求配置目录和 Gateway/exposure 行为在全部阶段保持兼容。

## 检查清单

- [x] 范围已由用户确认只保留 Headless。
- [x] 已明确保留的数据兼容边界。
- [x] 已记录 transport CRITICAL 风险。
- [x] 验收覆盖 Web、Rust、Gateway 和运行态。
