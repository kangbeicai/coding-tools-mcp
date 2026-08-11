# 任务清单：cloudflared-auto-download

## 概述

实现 Cloudflare exposure 启动时的按需自动下载，不增加新的 Web 操作流程。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 3 个规格文件
- **预计修改文件数**: 4 个产品/文档文件
- **预计新增/修改函数数**: 约 8 个
- **交付物逐项列举**:
  1. `docs/specs/cloudflared-auto-download/{requirements,design,tasks}.md`
  2. `src-tauri/src/tunnel/cloudflare.rs`
  3. `src-tauri/src/tunnel/supervisor.rs`
  4. `README.md`
  5. `README.en.md`

---

## 任务列表

### 阶段 1: 规格与影响分析

- [x] 1.1 确认 Cloudflare 官方 latest executable 资产并锁定支持平台
  - **证据块**: Cloudflare 官方 downloads 页面提供 Linux amd64/ARM64 binary 与 Windows 64-bit executable；官方 GitHub Release 资产名为 `cloudflared-linux-amd64`、`cloudflared-linux-arm64`、`cloudflared-windows-amd64.exe`。
  - **涉及文件**: 只读外部资料与 `src-tauri/src/tunnel/cloudflare.rs`
  - _需求: FR-2, NFR-1_ ｜ _设计: 技术方案_
- [x] 1.2 对 resolve/spawn/validation/download 公共入口执行 GitNexus impact
  - **证据块**: `spawn_cloudflare_tunnel` 当前直接 `resolve_cloudflared()?`；`validate_tunnel_requirements` 也提前要求 binary 存在。
  - **涉及文件**: 只读图谱分析
  - _需求: FR-1 至 FR-5_ ｜ _设计: 风险评估_

### 阶段 2: 核心实现

- [x] 2.1 实现 `ensure_cloudflared`，缺失时串行下载并安全安装缓存
  - **证据块**: `frp::ensure_frpc()` 已采用 resolve-first/download-on-missing 模式；`download_release_asset` 已提供 mirror/proxy/fallback。
  - **涉及文件**: `src-tauri/src/tunnel/cloudflare.rs`，预计 +100 行以内
  - _需求: FR-1, FR-2, FR-3, FR-4_ ｜ _设计: 架构设计、决策 2-3_
- [x] 2.2 将自动下载只接入实际 Cloudflare spawn，保持配置校验无网络副作用
  - **证据块**: `validate_tunnel_requirements()` 当前同步调用 `resolve_cloudflared()`；`spawn_cloudflare_tunnel()` 是实际 child process 创建点。
  - **涉及文件**: `src-tauri/src/tunnel/cloudflare.rs`, `src-tauri/src/tunnel/supervisor.rs`
  - _需求: FR-2, FR-5_ ｜ _设计: 决策 1_
- [x] 2.3 更新中英文部署说明为 cloudflared 自动按需下载
  - **证据块**: README 当前写明 cloudflared 必须预装或手工放入 bin。
  - **涉及文件**: `README.md`, `README.en.md`
  - _需求: FR-5_ ｜ _设计: 文件结构_

### 阶段 3: 集成测试

- [x] 3.1 增加资产映射/缓存安装回归并运行全量 Rust 测试与 release build
  - **证据块**: 当前 Cloudflare 模块已有 URL parser tests，FRP 模块已有平台 release asset 映射模式可参考。
  - **涉及文件**: `src-tauri/src/tunnel/cloudflare.rs` tests
  - _需求: FR-1 至 FR-5_ ｜ _设计: 测试策略_

---

## 检查点

- [x] 阶段 1 完成后：规格通过 `check_spec`，所有待改符号完成 GitNexus impact。
- [x] 阶段 2 完成后：启动 Cloudflare 时会 ensure，配置校验不下载，README 同步。
- [x] 阶段 3 完成后：专项/全量测试、release build、diff check、GitNexus detect-changes 全通过。

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、决策 3 | 1.2, 2.1, 3.1 | 已完成 |
| FR-2 | 架构设计、决策 1-2 | 1.1, 2.1, 2.2, 3.1 | 已完成 |
| FR-3 | 技术方案 | 2.1, 3.1 | 已完成 |
| FR-4 | 架构设计 | 2.1, 3.1 | 已完成 |
| FR-5 | 决策 1 | 2.2, 2.3, 3.1 | 已完成 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/tunnel/cloudflare.rs` | 修改 | +80 至 +120 | ensure、下载、资产映射、测试 |
| `src-tauri/src/tunnel/supervisor.rs` | 修改 | -1 至 +3 | validation 移除 binary presence 检查 |
| `README.md` | 修改 | +3/-2 | 中文自动下载说明 |
| `README.en.md` | 修改 | +3/-2 | 英文自动下载说明 |
| `docs/specs/cloudflared-auto-download/*.md` | 新建 | 约 250 | 功能规格 |

---

## 检查清单

- [x] 交付物清单已锁定
- [x] 所有任务包含证据块、涉及文件和 FR/design 回链
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 3 明确对照验收标准验证
- [x] 全文无占位内容
