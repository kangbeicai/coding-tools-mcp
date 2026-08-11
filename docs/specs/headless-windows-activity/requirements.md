# 需求文档：headless-windows-activity

## 功能概述

本功能解决两个当前 Headless/Web 版本的可用性缺口：其一，Web Admin 的 Activity 页面在 Request、Response、Error、Session ID、命令等字段包含很长内容或无空格长串时会撑破卡片和页面宽度；其二，`coding-tools` 当前平台层被收敛为 Linux-only，导致 Windows 无法以同样的前台 Headless + Web Admin + MCP Gateway 形态运行。本次恢复 Windows 的纯服务端平台能力，但明确不恢复 Tauri Desktop、窗口、托盘或 WebView UI。

## 历史经验与坑

- **可复用经验**: Linux-only 重构前已有 Windows 端口探测、进程树终止、可执行文件候选、frpc/cloudflared 子进程隐藏启动与 Windows 脚本执行实现；可选择性恢复这些纯 Headless 原语。
- **必须规避的坑**: 不得恢复 `tauri`、`tauri-plugin-*`、桌面 binary、tray/window/WebView memory sampling；不得让 Windows host metadata 或平台差异改变现有 Web Admin、Gateway、History/Auth HTTP 契约。

## 术语定义

- **Headless/Web 形态**: `coding-tools` 作为控制台/服务进程运行，由浏览器访问 Web Admin，不创建原生桌面窗口。
- **Activity 详情**: `/activity` 页面展示的一次 MCP 调用的 Request、Response、Error 与关联 session/process 元数据。

---

## 范围边界

**In Scope**
- Activity 页面任何用户/模型产生的长文本、长 JSON、长 ID、长命令均限制在所属卡片/列宽内，允许换行和内部滚动。
- Windows 构建并运行同一个 `coding-tools` binary，默认启动 Web Admin + MCP Gateway，行为与 Linux Headless 一致。
- 恢复 Windows 端口监听 PID 探测、端口回收、进程存活/镜像路径/进程树终止、PATH 与 cloudflared/frpc 候选解析。
- 恢复 Windows `exec_command` 对 `.cmd/.bat/.ps1`、UTF-8 Python 和隐藏控制台子进程的支持。
- 恢复 Windows cloudflared/frpc 的 `.exe` 路径和无窗口子进程启动；Windows x86_64 的 frpc 自动下载恢复 ZIP 资产支持。
- `service` 管理继续仅实现 Linux systemd；Windows 调用时返回清晰说明，并指向前台运行或外部 Windows Service 管理器。
- 更新中英文 README、Cargo 元数据、规格和跨平台测试。

**Out of Scope**
- 不恢复 Tauri Desktop、系统托盘、原生窗口、WebView、桌面安装器逻辑。
- 本轮不实现 Windows Service/Scheduled Task 的自动安装器。
- 本轮不恢复 macOS 平台支持。
- 不改变 Activity 后端原始数据保存策略；本轮只修复展示溢出。

---

## 需求列表

### FR-1: Activity 长内容不得撑破页面

**优先级:** Must
**用户故事:** 作为管理员，我想查看任意长度的 MCP 调用内容，而不让详情或列表破坏页面布局。

#### 验收标准（EARS）

1. WHEN Request/Response/Error 含长 JSON 或无空格长串 THEN 页面 SHALL 将内容限制在详情列宽内，并允许换行与内部滚动。
2. WHEN Session ID、Process Session ID、Operation ID 或命令很长 THEN 页面 SHALL 在所属卡片内折行或截断，不扩大 grid/page intrinsic width。
3. WHILE 浏览器宽度处于桌面或窄屏布局 THEN Activity 页面 SHALL 不产生由内容引起的整页横向溢出。

### FR-2: Windows 运行相同 Headless/Web 服务

**优先级:** Must
**用户故事:** 作为 Windows 用户，我想直接运行 `coding-tools` 并通过浏览器管理 Gateway，而不安装或启动桌面 UI。

#### 验收标准（EARS）

1. WHEN Windows 用户运行 `coding-tools` 或 `coding-tools serve` THEN 系统 SHALL 启动 Web Admin + MCP Gateway，不创建原生 GUI。
2. WHILE Web Admin/Gateway 在 Windows 上运行 THEN HTTP API、Admin Auth、Activity、History、workspace 路由 SHALL 与 Linux 使用同一实现。
3. IF 平台不是已支持的 Linux/Windows THEN 编译期或运行期 SHALL 给出明确 unsupported 结果，不错误选用 LinuxPlatform。

### FR-3: Windows 平台进程与网络原语可用

**优先级:** Must
**用户故事:** 作为 Gateway 运行时，我需要在 Windows 上探测监听端口和管理受控子进程，以便启动、重启和清理服务。

#### 验收标准（EARS）

1. WHEN 查询端口监听者 THEN WindowsPlatform SHALL 返回监听 PID 或 None。
2. WHEN 查询/停止受管进程 THEN WindowsPlatform SHALL 支持镜像路径、存活检测和进程树终止。
3. WHEN PATH 中只有 `.exe` 文件 THEN 可执行文件解析 SHALL 自动识别 Windows 扩展名。

### FR-4: Windows Exec 与隧道子进程保持 Headless

**优先级:** Must
**用户故事:** 作为远程 MCP 用户，我希望 Windows 下执行脚本和公网隧道时不会弹出额外控制台窗口。

#### 验收标准（EARS）

1. WHEN Windows 执行 `.cmd/.bat/.ps1` THEN `exec_command` SHALL 使用对应系统 runner 并保留含空格路径/参数。
2. WHEN Windows 启动 Python/cmd/PowerShell/frpc/cloudflared THEN 子进程 SHALL 使用无窗口创建标志，且保持可终止。
3. WHEN Windows x86_64 缺少 frpc THEN 自动下载 SHALL 使用 Windows ZIP 资产并提取 `frpc.exe`。

### FR-5: Linux-only service 管理边界清晰

**优先级:** Should
**用户故事:** 作为 Windows 用户，我希望 CLI 不把 systemd 指令当成可用功能。

#### 验收标准（EARS）

1. WHEN Windows 调用 `coding-tools service install|status|uninstall` THEN 系统 SHALL 返回 Windows 上不提供内置 service installer 的明确说明。
2. WHEN Linux 调用相同命令 THEN 现有 systemd 行为 SHALL 保持不变。
3. WHEN 查看 `--help` 或 README THEN 文档 SHALL 清楚区分跨平台前台 Web 模式与 Linux-only systemd 可选模式。

### FR-6: 跨平台验证与文档

**优先级:** Must
**用户故事:** 作为维护者，我希望 Linux 回归不退化，并能在 CI/开发机验证 Windows target 的编译契约。

#### 验收标准（EARS）

1. WHEN 完成本轮修改 THEN Linux `cargo test --all-targets`、`npm run check`、`npm run build` SHALL 通过。
2. IF Windows Rust target 可用 THEN `cargo check --target x86_64-pc-windows-gnu` 或等价 Windows target SHALL 通过；若环境缺 target/toolchain，结果 SHALL 明确记录。
3. WHEN 阅读 README THEN 用户 SHALL 能找到 Windows Headless/Web 启动和依赖说明。

---

## 非功能需求

- **NFR-1（性能）**: Activity UI 不额外复制/截断后端 payload；仅通过 CSS/layout 约束解决溢出。
- **NFR-2（安全）**: Windows 支持不得引入远程服务安装、桌面 IPC 或新开放端口；继续复用现有 Auth/Gateway 安全边界。
- **NFR-3（兼容性）**: Linux 默认行为、配置文件路径和 HTTP 契约保持兼容；Windows x86_64 为本轮主要目标。
- **NFR-4（可维护性）**: Windows 代码通过 `cfg(windows)` 隔离，跨平台公共接口继续集中在 `platform::Platform`。

---

## 依赖关系

- Rust `windows` crate 仅作为 `cfg(windows)` target dependency。
- Windows frpc ZIP 自动下载依赖 `zip` crate；Linux tar.gz 路径继续使用现有 `flate2`/`tar`。
- Web Activity 继续依赖现有 Svelte/Tailwind utility classes，不新增前端依赖。

---

## 检查清单

- [x] 已消化历史 Windows 平台实现，只复用 Headless 原语
- [x] 需求覆盖 Activity 溢出与 Windows Headless 两个核心场景
- [x] 每条需求有唯一 ID，并可由设计/任务回链
- [x] 验收标准可测试
- [x] 已标注 MoSCoW 优先级
- [x] In/Out of Scope 明确排除桌面 UI
- [x] 非功能需求明确
- [x] 依赖关系完整
