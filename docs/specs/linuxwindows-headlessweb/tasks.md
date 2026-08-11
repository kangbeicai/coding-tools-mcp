# 任务清单：GitHub Actions 自动 Release

## 概述

完成 GitHub tag 自动 Release，并把 Windows/ARM64 的真实 native 编译纳入发布门禁。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 4 个（1 workflow + 3 spec）
- **预计修改文件数**: 3 个（README 中英文 + graph insights）
- **运行时源码修改数**: 0
- **交付物**:
  1. `.github/workflows/release.yml`
  2. `docs/specs/linuxwindows-headlessweb/requirements.md`
  3. `docs/specs/linuxwindows-headlessweb/design.md`
  4. `docs/specs/linuxwindows-headlessweb/tasks.md`
  5. `README.md`
  6. `README.en.md`
  7. `docs/graph-insights/latest.md`

## 任务列表

### 阶段 1: 规格与环境确认

- [x] 1.1 确认当前 Headless build 契约和 GitHub 原生 runner
  - **证据块**: `src-tauri/build.rs` 从仓库 `build/` 生成 `embedded_web.rs`；GitHub 当前公布 `ubuntu-24.04-arm` ARM64 runner。
  - **涉及文件**: 只读分析
  - _需求: FR-2, FR-3_ ｜ _设计: 技术方案、决策 1-2_
- [x] 1.2 落盘规格并通过 `check_spec`
  - **证据块**: 规格覆盖 tag、manual dispatch、verify、三平台 build、checksum、release、upstream。
  - **涉及文件**: 本规格 3 文件
  - _需求: FR-1 至 FR-5_ ｜ _设计: 全文_

### 阶段 2: Release workflow

- [x] 2.1 新增 tag/manual Release workflow，使用 native Linux x64/arm64/Windows x64 runners
  - **证据块**: 现有 `.github/workflows/ci.yml` 仅有手动 Linux build/test 与 frontend check/build；旧 Desktop workflow 使用过 artifact 聚合，但包含已删除的 Tauri/macOS 流程，不可直接复用。
  - **涉及文件**: `.github/workflows/release.yml`，约 150 行
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 流程、决策 1-3_
- [x] 2.2 聚合二进制、生成 SHA256SUMS 并创建 GitHub Release
  - **证据块**: GitHub CLI `gh release create` 支持上传资产和自动生成 release notes；workflow `contents: write` 可创建 Release。
  - **涉及文件**: `.github/workflows/release.yml`
  - _需求: FR-4_ ｜ _设计: 决策 3-4_

### 阶段 3: 文档、验证与 Git tracking

- [x] 3.1 更新 README 下载方式和 graph insights
  - **证据块**: README 当前只有源码 build/run，没有 GitHub Release 下载入口；graph insights 仍描述 desktop release workflow 已删除。
  - **涉及文件**: `README.md`, `README.en.md`, `docs/graph-insights/latest.md`
  - _需求: FR-4_ ｜ _设计: 文件结构_
- [x] 3.2 验证 workflow 配置、提交并推送 GitHub
  - **证据块**: 本地 `main` 已跟踪 `github/main`，`forgejo` remote 仍存在。
  - **涉及文件**: Git metadata（不进入 commit）与上述交付物
  - _需求: FR-5_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1：规格完整并经过 `check_spec`。
- [x] 阶段 2：workflow 结构已核对，三个 native build job 与 publish dependency 正确。
- [x] 阶段 3：README/graph 同步，detect-changes、commit、push 完成。

## 需求覆盖矩阵

| 需求 ID | 任务 | 状态 |
|---------|------|------|
| FR-1 | 2.1 | 已完成 |
| FR-2 | 2.1 | 已完成 |
| FR-3 | 2.1 | 已完成 |
| FR-4 | 2.2, 3.1 | 已完成 |
| FR-5 | 3.2 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `.github/workflows/release.yml` | 新建 | 约 150 行 | tag/manual Release、verify、三平台 native build、checksum、publish |
| `docs/specs/linuxwindows-headlessweb/requirements.md` | 新建 | 约 100 行 | 功能需求与验收标准 |
| `docs/specs/linuxwindows-headlessweb/design.md` | 新建 | 约 100 行 | runner、artifact、Release 设计 |
| `docs/specs/linuxwindows-headlessweb/tasks.md` | 新建 | 约 100 行 | 实施任务和验证矩阵 |
| `README.md` | 修改 | 约 20 行 | GitHub Release curl/wget 下载说明 |
| `README.en.md` | 修改 | 约 20 行 | English Release download instructions |
| `docs/graph-insights/latest.md` | 修改 | 约 10 行 | 记录新的 Headless Release workflow |

## 检查清单

- [x] 无模板占位符。
- [x] 每条任务回链 FR。
- [x] Scope-lock 明确不修改运行时源码。
- [x] Windows native build 是发布门禁，不恢复 Desktop/Tauri。
