# 任务清单：Admin 密码恢复 CLI

## 概述

实现 `coding-tools admin reset`，并同步 README 中的 Web Admin 登录与密码恢复文档。

## 交付物清单（Scope-lock）

- **预计新建实现文件数**: 0 个
- **预计修改实现/文档文件数**: 3 个
- **预计新增/修改函数数**: 约 4 个
- **交付物逐项列举**:
  1. `src-tauri/src/headless.rs`
  2. `README.md`
  3. `README.en.md`

## 任务列表

### 阶段 1: 准备工作

- [x] 1.1 核对现有 Admin 配置、CLI 路由和密码遗忘后的运行时边界
  - **证据块**: `src-tauri/src/settings/model.rs:9-20` 的 `AdminConfig` 已含 `username/password_hash`；`src-tauri/src/headless.rs:18-36` 统一解析 CLI；`src-tauri/src/admin/listener.rs:203-210` 以非空 password hash 阻止重复 setup。
  - **涉及文件**: 仅阅读
  - _需求: FR-1_ ｜ _设计: 架构设计、决策 2_

### 阶段 2: 核心实现

- [x] 2.1 增加 `coding-tools admin reset`，仅清空 Admin 凭据并输出重启说明
  - **证据块**: `src-tauri/src/headless.rs` 当前只有 workspace/config/service/health 子命令；最终 reset 直接使用 `DataStore::update_file`，避免 `AppState::new()` 补齐 shared secrets 的副作用。
  - **涉及文件**: `src-tauri/src/headless.rs` 约 55 行
  - _需求: FR-1_ ｜ _设计: API 设计、决策 1、决策 2_

- [x] 2.2 更新中英文 README 的 Web Admin 认证、首次设置、session 和密码恢复说明
  - **证据块**: `README.md` 当前仍写“Web Console 当前没有独立管理员认证”，与已部署代码不一致。
  - **涉及文件**: `README.md` 约 35 行；`README.en.md` 约 35 行
  - _需求: FR-2_ ｜ _设计: 技术选型、测试策略_

### 阶段 3: 集成测试

- [x] 3.1 验证 reset 字段隔离、CLI 帮助与全量 Rust 回归
  - **证据块**: `headless::tests::admin_reset_only_changes_admin_login_fields` 通过；隔离 XDG smoke 验证 `shared_secrets`、`workspace_secrets`、Gateway URL 均保持原值；`cargo test --all-targets` 共 190 项通过，release build 通过，`--help` 包含 `admin reset`。
  - **涉及文件**: `src-tauri/src/headless.rs` 内测试；全仓测试命令
  - _需求: FR-1, FR-2_ ｜ _设计: 测试策略、风险评估_

## 检查点

- [x] 阶段 1 完成后：确认本地 CLI 是合适的恢复信任边界
- [x] 阶段 2 完成后：reset CLI 与 README 均完成
- [x] 阶段 3 完成后：check/test/diff 与文档核验全部通过

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、API 设计、决策 1/2/3 | 1.1, 2.1, 3.1 | 完成 |
| FR-2 | 技术选型、测试策略 | 2.2, 3.1 | 完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/headless.rs` | 修改 | 55 | admin 子命令、reset helper、help、测试 |
| `README.md` | 修改 | 35 | 中文认证与恢复说明 |
| `README.en.md` | 修改 | 35 | 英文认证与恢复说明 |

## 检查清单

- [x] Scope-lock 已明确
- [x] 每条任务标题具体可验收
- [x] 每条任务包含证据块
- [x] 每条任务标注涉及文件和行数预算
- [x] 每条任务回链 FR 与 design
- [x] 需求覆盖矩阵完整
- [x] 阶段 3 包含逐项验收
- [x] 文档无占位符
