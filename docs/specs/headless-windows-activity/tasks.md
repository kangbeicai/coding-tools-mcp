# 任务清单：headless-windows-activity

## 概述

在不恢复 Desktop/Tauri 的前提下，修复 Activity 长内容布局，并恢复 Windows Headless/Web 运行所需的平台、exec 和隧道能力。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 7 个（3 个规格 + 4 个 Windows platform 源文件）
- **预计修改文件数**: 11-13 个
- **预计新增/修改函数数**: 约 25 个
- **交付物逐项列举**:
  1. `docs/specs/headless-windows-activity/{requirements,design,tasks}.md`
  2. `src/routes/activity/+page.svelte`
  3. `src-tauri/src/platform/windows/{mod,net,process,paths}.rs`
  4. `src-tauri/src/platform/{mod,paths}.rs`
  5. `src-tauri/src/tools/{exec,session}.rs`
  6. `src-tauri/src/tunnel/cloudflare.rs`
  7. `src-tauri/src/tunnel/frp/client.rs`
  8. `src-tauri/src/headless.rs`
  9. `src-tauri/Cargo.toml` / `Cargo.lock`
  10. `README.md` / `README.en.md`

---

## 任务列表

### 阶段 1: 规格与影响分析

- [x] 1.1 盘点 Activity intrinsic overflow 与 Windows 编译阻塞点，锁定只恢复 Headless 原语
  - **证据块**: `activity/+page.svelte:272-281` 的 `<pre class="max-h-* overflow-auto">` 缺少 `max-w-full/min-w-0/whitespace-pre-wrap/break-*`；`platform/mod.rs` 当前无 cfg 且固定 `LinuxPlatform`；`tools/session.rs:487+` 无条件使用 `libc`；tunnel 直接调用 Unix `process_group(0)`。
  - **涉及文件**: 只读分析
  - _需求: FR-1 至 FR-5_ ｜ _设计: 架构设计、设计决策 1-4_
- [x] 1.2 运行 GitNexus impact、落盘规格并通过 `check_spec`
  - **证据块**: 公共入口包括 `platform()`, `command_for_program`, `send_session_signal`, `spawn_cloudflare_tunnel`, `spawn_frpc`, `service_*` 与 Activity 页面。
  - **涉及文件**: 本规格 3 文件
  - _需求: FR-6_ ｜ _设计: 风险评估_

### 阶段 2: Activity 与平台基础

- [x] 2.1 收紧 Activity 所有长内容容器，确保无空格长串不会扩大页面宽度
  - **证据块**: `activity/+page.svelte` 的 Session、Process、Operation 与 `<pre>` 均直接渲染原文；部分父容器缺 `min-w-0`。
  - **涉及文件**: `src/routes/activity/+page.svelte`，预计净改 15-30 行
  - _需求: FR-1_ ｜ _设计: 决策 2_
- [x] 2.2 恢复 Linux/Windows Platform dispatch 与 Windows Win32 primitives
  - **证据块**: `platform/mod.rs` 当前 `OnceLock<LinuxPlatform>`；历史 `17476a7^` 有纯 Win32 net/process/paths 模块可复用。
  - **涉及文件**: `platform/mod.rs`, `platform/paths.rs`, 新增 `platform/windows/*.rs`，预计 250-350 行
  - _需求: FR-2, FR-3_ ｜ _设计: 决策 1_

### 阶段 3: Windows exec、隧道与 CLI

- [x] 3.1 恢复 Windows exec/session cfg，支持脚本 runner、UTF-8 与无窗口进程
  - **证据块**: `tools/exec.rs` 当前只构造直接 `Command`，health probe 固定 `sh`，Unix test 未 cfg；`tools/session.rs` 无条件 `libc::kill`。
  - **涉及文件**: `tools/exec.rs`, `tools/session.rs`，预计净增 130-190 行
  - _需求: FR-4_ ｜ _设计: 决策 4_
- [x] 3.2 恢复 cloudflared/frpc Windows binary、ZIP 与 process creation 分支
  - **证据块**: `cloudflare.rs` 和 `frp/client.rs` 的 Linux-only 重构 diff 明确删除 `.exe`、ZIP、creation_flags；现有核心逻辑本身跨平台。
  - **涉及文件**: `tunnel/cloudflare.rs`, `tunnel/frp/client.rs`, Cargo deps，预计净增 100-180 行
  - _需求: FR-4_ ｜ _设计: 决策 4_
- [x] 3.3 将 Headless CLI 文案改为 Linux/Windows Web，并明确 service 仍 Linux-only
  - **证据块**: `headless.rs` 文件注释和 `--help` 写死 Linux；service commands 直接调用 systemd 模块。
  - **涉及文件**: `headless.rs`，预计 30-60 行
  - _需求: FR-2, FR-5_ ｜ _设计: 决策 3_

### 阶段 4: 文档与验证

- [x] 4.1 更新 README 中英文 Windows Headless/Web 启动、依赖与 service 边界
  - **证据块**: 当前 Cargo description/README 仍将产品描述为 Linux Headless。
  - **涉及文件**: `README.md`, `README.en.md`, `Cargo.toml`
  - _需求: FR-6_ ｜ _设计: 文件结构_
- [x] 4.2 运行前端、Linux Rust 全量回归、release build 和 Windows target compile check
  - **证据块**: 当前基线上一轮为 Rust 195 tests + npm check/build + release build 通过。
  - **结果**: `npm run check` 0 error/0 warning；`npm run build` 通过；`cargo check --all-targets` 通过；`cargo test --all-targets` 195/195 通过；release build 通过。Windows `cargo check --target x86_64-pc-windows-gnu` 已实际尝试，但开发机未安装该 Rust target（`can't find crate for core`）；客户端不支持权限 elicitation，无法在本会话运行 `rustup target add`。这是验证环境限制，不是项目编译诊断。
  - **涉及文件**: 测试/构建，无产品新文件
  - _需求: FR-1 至 FR-6_ ｜ _设计: 测试策略_
- [x] 4.3 执行 GitNexus detect-changes、代码审查与 Probe converge
  - **证据块**: 本轮触及公共 platform/exec/tunnel 链，必须在提交前确认 blast radius。
  - **涉及文件**: 无新增
  - _需求: FR-6_ ｜ _设计: 风险评估_

---

## 检查点

- [x] 阶段 1：`check_spec` 0 error，公共入口 impact 已知
- [x] 阶段 2：Activity 不再 intrinsic overflow；`platform()` 可按 target 选择 Linux/Windows
- [x] 阶段 3：Windows cfg 下无 Unix-only 编译引用，Headless/Web CLI 不恢复 Desktop
- [x] 阶段 4：Linux 全量回归通过；Windows compile check 通过或环境限制明确；detect/converge 完成

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 决策 2 | 2.1, 4.2 | 已完成 |
| FR-2 | 决策 1/3 | 2.2, 3.3, 4.1 | 已完成 |
| FR-3 | 决策 1 | 2.2, 4.2 | 已完成 |
| FR-4 | 决策 4 | 3.1, 3.2, 4.2 | 已完成（Windows target 编译受环境限制） |
| FR-5 | 决策 3 | 3.3, 4.1 | 已完成 |
| FR-6 | 测试策略 | 1.2, 4.1, 4.2, 4.3 | 已完成 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `docs/specs/headless-windows-activity/*.md` | 新建 | 约 300 | 规格 |
| `src/routes/activity/+page.svelte` | 修改 | +15~30 | overflow containment |
| `src-tauri/src/platform/windows/*.rs` | 新建 | +250~350 | Win32 platform |
| `src-tauri/src/platform/{mod,paths}.rs` | 修改 | +30~60 | target dispatch/PATH |
| `src-tauri/src/tools/{exec,session}.rs` | 修改 | +130~190 | Windows child execution |
| `src-tauri/src/tunnel/cloudflare.rs` | 修改 | +30~70 | `.exe`/creation flags |
| `src-tauri/src/tunnel/frp/client.rs` | 修改 | +70~120 | `.exe`/ZIP/creation flags |
| `src-tauri/src/headless.rs` | 修改 | +30~60 | platform wording/service boundary |
| `src-tauri/Cargo.toml` / `Cargo.lock` | 修改 | 小量 | target dependencies |
| `README.md` / `README.en.md` | 修改 | +30~70 | Windows usage |

---

## 检查清单

- [x] Scope-lock 已填
- [x] 任务均有现状证据
- [x] 文件预算明确
- [x] 每条任务回链需求与设计
- [x] 覆盖矩阵无遗漏
- [x] 测试包含 Windows target compile 与 Linux 回归
- [x] 无模板占位符
