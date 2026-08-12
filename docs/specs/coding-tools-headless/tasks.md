# 任务清单：Headless 启动与配置目录优化

## 概述

精简 foreground Headless 启动信息，把 Web Admin 访问地址改为可直接复制的 Local / LAN URL，并将历史 `coding-tools-mcp-desktop` 配置根目录安全迁移为 `coding-tools-mcp`。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 3 个规格文件
- **预计修改文件数**: 4 个运行时源码文件 + 必要文档
- **预计新增/修改函数数**: 约 8 个
- **交付物逐项列举**:
  1. `docs/specs/coding-tools-headless/requirements.md`
  2. `docs/specs/coding-tools-headless/design.md`
  3. `docs/specs/coding-tools-headless/tasks.md`
  4. `src-tauri/src/headless.rs`
  5. `src-tauri/src/platform/paths.rs`
  6. `src-tauri/src/platform/linux/mod.rs`
  7. `src-tauri/src/platform/windows/mod.rs`

## 任务列表

### 阶段 1: 规格与影响面

- [x] 1.1 固化 Local / LAN URL 与 bind 边界契约
  - **证据块**: `src-tauri/src/headless.rs` 当前同时输出 `Web Admin`、`listen` 和 `from LAN: http://<server-ip>`，存在重复和不可复制占位符。
  - **涉及文件**: 本规格 3 文件，约 250 行
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 全文_
- [x] 1.2 对 Headless foreground 启动入口执行 GitNexus impact
  - **证据块**: `run_server` upstream impact 为 LOW，仅 1 个直接调用方 `run_from_env`、1 个相关 process；`print_help` 同为 LOW。
  - **涉及文件**: 只读分析
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 架构设计_
- [x] 1.3 对 `Platform::app_config_dir` 及 Linux/Windows 实现执行 GitNexus impact
  - **证据块**: trait/Linux symbol impact 为 LOW；Windows symbol 在当前 Linux 条件编译索引中未建图，返回 UNKNOWN，因此 Windows 复用同一个 shared helper 并由后续 native CI 继续验证。
  - **涉及文件**: 只读分析
  - _需求: FR-4, FR-5_ ｜ _设计: 决策 4、决策 5_

### 阶段 2: 核心实现

- [x] 2.1 实现跨平台 LAN IPv4 探测并安全降级
  - **证据块**: 当前代码没有 LAN IP 探测 helper，只有 `<server-ip>` 占位符。
  - **涉及文件**: `src-tauri/src/headless.rs`，约 40 行
  - _需求: FR-2_ ｜ _设计: 技术方案、决策 2_
- [x] 2.2 重构 Web Admin URL 生成与 foreground 摘要，删除重复字段
  - **证据块**: 当前 foreground 先打印 `Web Admin : admin.local_endpoint`，随后再次打印 `listen: admin.local_endpoint`。
  - **涉及文件**: `src-tauri/src/headless.rs`，约 50 行
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 决策 1、决策 3_
- [x] 2.3 更新帮助文本，移除 `<server-ip>` 占位符
  - **证据块**: `print_help()` 当前固定打印 `LAN Web Console: http://<server-ip>:28767`。
  - **涉及文件**: `src-tauri/src/headless.rs`，约 5 行
  - _需求: FR-1, FR-2_ ｜ _设计: 测试策略_
- [x] 2.4 统一 Linux/Windows canonical 配置根目录并实现 legacy rename 迁移
  - **证据块**: 两个平台当前都在 `app_config_dir()` 直接返回 `base.join("coding-tools-mcp-desktop")`，所有持久化数据和 tunnel cache 共享该根。
  - **涉及文件**: `src-tauri/src/platform/paths.rs` 约 90 行；`src-tauri/src/platform/linux/mod.rs` / `windows/mod.rs` 各约 5 行
  - _需求: FR-4, FR-5_ ｜ _设计: 决策 4、决策 5_

### 阶段 3: 回归验证

- [x] 3.1 增加 URL/bind/filter 单测并执行全量 Rust 测试与 release build
  - **证据块**: `headless.rs` 现有测试只覆盖 admin reset，没有启动摘要地址测试。
  - **涉及文件**: `src-tauri/src/headless.rs`，约 80 行测试
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 测试策略_
- [x] 3.2 增加配置根目录迁移单测并核对文档中的 Linux/Windows 路径
  - **证据块**: 当前没有 canonical/legacy 配置根目录迁移测试，README/项目上下文仍可能出现 Desktop 历史路径。
  - **涉及文件**: `src-tauri/src/platform/paths.rs` 测试约 80 行；README/项目上下文按检索结果更新
  - _需求: FR-4, FR-5_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1：`check_spec` 通过，`run_server` 与 `app_config_dir` 影响面均已审查。
- [x] 阶段 2：默认启动摘要仅保留一个 Web Admin 区块并显示 Local / LAN 实际 URL；canonical 配置目录切换和 legacy 迁移完成。
- [ ] 阶段 3：专项/全量测试、release build、diff check、GitNexus detect-changes 全通过。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 决策 1 | 1.1, 2.2, 2.3, 3.1 | 已完成 |
| FR-2 | 技术方案、决策 2 | 1.1, 2.1, 2.2, 3.1 | 已完成 |
| FR-3 | 决策 3 | 1.1, 2.2, 3.1 | 已完成 |
| FR-4 | 决策 4 | 1.3, 2.4, 3.2 | 已完成 |
| FR-5 | 决策 5 | 1.3, 2.4, 3.2 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/headless.rs` | 修改 | 约 175 行 | LAN IPv4 探测、地址生成、摘要输出、帮助与测试 |
| `src-tauri/src/platform/paths.rs` | 修改 | 约 170 行 | canonical/legacy 配置目录选择、rename 迁移和单测 |
| `src-tauri/src/platform/linux/mod.rs` | 修改 | 约 5 行 | 复用共享 config root helper |
| `src-tauri/src/platform/windows/mod.rs` | 修改 | 约 5 行 | 复用共享 config root helper |
| `docs/specs/coding-tools-headless/requirements.md` | 新建 | 约 90 行 | 功能需求 |
| `docs/specs/coding-tools-headless/design.md` | 新建 | 约 100 行 | 技术设计 |
| `docs/specs/coding-tools-headless/tasks.md` | 新建 | 约 100 行 | 实施与验证 |

## 检查清单

- [x] 交付物清单已填写。
- [x] 每条任务均回链 FR 与设计章节。
- [x] 无占位符或 TODO。
- [x] 范围锁定为 Headless 启动 UX 与配置根目录兼容迁移，不改变当前服务 listener / auth / Gateway / Tunnel 业务逻辑。
