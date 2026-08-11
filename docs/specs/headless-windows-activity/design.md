# 设计文档：headless-windows-activity

## 概述

本设计在现有 Linux Headless/Web 架构上增加 Windows 平台实现，同时修复 Activity 页面的 intrinsic width 溢出。Windows 继续运行同一个 `coding-tools` console binary，由 Axum/Tokio 提供 Web Admin 与 MCP Gateway，不引入 Tauri Desktop。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, NFR-1 至 NFR-4

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| Activity 布局 | Tailwind `min-w-0`, `max-w-full`, `overflow-*`, `break-all`, `whitespace-pre-wrap` | 不改变 payload，只限制视觉布局 | FR-1 |
| Windows OS API | `windows` 0.61 target dependency | 仓库历史已验证，避免解析本地化命令输出 | FR-2, FR-3 |
| Windows process creation | `std::os::windows::process::CommandExt` + creation flags | 保持 console/headless，不弹子窗口 | FR-4 |
| Windows frpc archive | `zip` crate | 官方 Windows frp 发布为 ZIP | FR-4 |
| Service 管理 | Linux `systemd` 继续原实现；Windows 返回 unsupported guidance | 本轮不引入 Windows Service 安装器 | FR-5 |

### 架构设计

```text
coding-tools (same binary entry)
  ├─ headless.rs
  │    ├─ Web Admin (Axum/static build)
  │    └─ MCP Gateway
  ├─ platform::platform()
  │    ├─ LinuxPlatform  -> /proc + libc
  │    └─ WindowsPlatform -> Win32 APIs
  ├─ tools::exec/session
  │    └─ cfg(unix/windows) process behavior
  ├─ tunnel::{cloudflare,frp}
  │    └─ platform-specific child creation/binary names
  └─ Web /activity
       └─ content containment only; backend payload unchanged
```

`platform::Platform` 仍是 OS primitive 抽象。`platform()` 改回 `OnceLock<Box<dyn Platform>>`，只注册 Linux 与 Windows；不会恢复 macOS/Desktop 的 open/file-manager API。

---

## 数据模型

不新增持久化实体。Activity 后端 `ActivityTrace` 与配置 JSON 均保持不变。

---

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| `platform()` | `src-tauri/src/platform/mod.rs` | 无 | `&'static dyn Platform` | FR-2, FR-3 |
| `WindowsPlatform` | `platform/windows/mod.rs` | OS primitives | `Platform` impl | FR-3 |
| `command_for_program` | `tools/exec.rs` | program + args | `tokio::process::Command` | FR-4 |
| `send_session_signal` | `tools/session.rs` | pid + signal | best-effort termination | FR-4 |
| `cloudflared_binary_name` / `frpc_binary_name` | tunnel modules | target cfg | executable name | FR-4 |
| `service_*_command` | `headless.rs` | CLI args | Linux action / Windows guidance | FR-5 |

HTTP/API 不增加新 endpoint。

---

## 文件结构

```text
src/routes/activity/+page.svelte          # Activity 长内容 containment
src-tauri/Cargo.toml                      # windows + zip target/runtime deps
src-tauri/src/platform/mod.rs             # Linux/Windows platform dispatch
src-tauri/src/platform/paths.rs           # Windows .exe PATH resolution
src-tauri/src/platform/windows/
├── mod.rs                                # WindowsPlatform
├── net.rs                                # TCP listener PID/reclaim
├── process.rs                            # process path/alive/tree termination
└── paths.rs                              # AppData/cloudflared/frpc candidates
src-tauri/src/tools/exec.rs               # Windows script/encoding/no-window
src-tauri/src/tools/session.rs            # cfg-specific signal/terminate
src-tauri/src/tunnel/cloudflare.rs        # Windows binary/process creation
src-tauri/src/tunnel/frp/client.rs        # Windows exe/zip/process creation
src-tauri/src/headless.rs                 # cross-platform wording/service boundary
README.md / README.en.md                  # Windows Headless usage
```

---

## 设计决策

### 决策 1: 恢复 Windows platform，不恢复 Desktop（FR-2, FR-3）

**问题**: Linux-only 重构删除 Windows 平台模块时，同时删除了桌面 UI 与纯 OS primitive。

**选项**:
1. 恢复旧 Desktop feature/Tauri：功能多但违背当前产品形态。
2. 只恢复 `platform/windows` 与必要 target dependency：复用成熟 Win32 行为，同时维持单一 Web UI。

**决策**: 选择 2。

**理由**: 当前 Admin/Gateway 已完全基于 Axum/Tokio，Windows 只缺 OS primitive，不需要桌面框架。

### 决策 2: Activity 后端不截断，UI 自己 containment（FR-1）

**问题**: 页面溢出来自 `<pre>`、ID 和 flex/grid 子项的 intrinsic width，而非必须缩短原始 Activity 内容。

**决策**: 通过 `min-w-0/max-w-full/overflow-auto/break-all/whitespace-pre-wrap` 限制展示；不再增加后端截断。

**理由**: 保持诊断数据完整，同时避免布局破坏。

### 决策 3: Windows service installer 暂不内置（FR-5）

**问题**: systemd 是 Linux 专属；Windows Service 注册涉及权限、服务账户和卸载语义。

**决策**: 本轮 Windows 仅支持前台/外部 service manager；CLI 明确返回说明。

**理由**: 用户目标是 Windows 可运行 Web 版，不是本轮实现服务安装器。

### 决策 4: 恢复 Windows 隧道与 exec 的无窗口创建标志（FR-4）

**问题**: headless server 启动 cmd/PowerShell/Python/frpc/cloudflared 时不能弹控制台窗口。

**决策**: Windows 使用 `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`；Unix 保持 `process_group(0)`。

---

## 测试策略

- Activity: `npm run check` + `npm run build`，并通过 class/DOM 静态断言或人工代码审查确认 Request/Response/Error、session/process/command 均有 containment。
- Linux Rust: `cargo check --all-targets`、`cargo test --all-targets`、release build。
- Windows compile: 优先 `cargo check --target x86_64-pc-windows-gnu --all-targets`；若 target 未安装，尝试安装或明确记录环境限制。
- Windows cfg 单测: 恢复 PATH `.exe`、script runner、creation flags、TCP port encoding 等纯单测；这些可在 Windows target CI 执行。
- Regression: Gateway/Admin/Auth/History/Activity 的现有测试全部保留。

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 公共 `platform()` 影响 Gateway/tunnel/process lifecycle | 高 | 修改前 GitNexus impact；复用历史 Win32 实现；全量回归 |
| `exec_command` Windows quoting/runner 复杂 | 高 | 恢复历史已验证实现和 Windows-only tests |
| Linux 隧道行为被跨平台 cfg 改坏 | 中 | Unix 分支保持原 `process_group(0)`；全量 tunnel tests |
| Windows target 环境缺 GNU/MSVC 标准库或 linker | 中 | `cargo check` 优先；无法交叉验证时明确说明并保留 cfg 单测 |
| Activity CSS 仍有某个 metadata 字段产生 intrinsic overflow | 中 | 所有 grid/flex 父项统一 `min-w-0`，长 ID 统一 `break-all` |

---

## 检查清单

- [x] 技术方案与当前 Headless/Web 架构一致
- [x] 全部 FR 有设计覆盖
- [x] 文件路径基于当前仓库和历史实现
- [x] 不改变 HTTP 数据模型
- [x] 关键平台/UI 决策已记录
- [x] 测试策略覆盖 Linux 与 Windows compile contract
