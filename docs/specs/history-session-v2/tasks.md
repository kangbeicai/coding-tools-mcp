# 任务清单：history-session-v2

## 概述

在当前 Headless 主分支上原生实现 History Session v2，不 cherry-pick 旧 Desktop 分支；先锁定工具/存储契约，再实现事实档案 revision、派生 state/manifest、search/read，最后更新 MCP instructions、README 和回归测试。

## 交付物清单（Scope-lock）

- **实际新建文件数**: 9 个（3 个规格 + 6 个 History 子模块）
- **实际修改文件数**: 15 个（History 4、registry/dispatch/server 3、测试 2、README 2、旧规格 3、会话提示组件 1）
- **预计新增/修改函数数**: 约 25 个
- **交付物逐项列举**:
  1. `docs/specs/history-session-v2/{requirements,design,tasks}.md`
  2. `src-tauri/src/tools/history/{model,markdown,storage,mod}.rs`
  3. `src-tauri/src/tools/history/{bootstrap,checkpoint,retrieval,maintenance,state,response}.rs`
  4. `src-tauri/src/tools/{registry,dispatch}.rs`
  5. `src-tauri/src/mcp/server.rs`
  6. `src-tauri/tests/{history_session,call_tool_contract}.rs`
  7. `src/lib/components/ChatGptSessionPrompt.svelte`
  8. `README.md`、`README.en.md`
  9. `docs/specs/history-session-archive/*` 中与 v2 冲突的旧约束更新/标记 superseded

---

## 任务列表

### 阶段 1: 规格与影响分析

- [x] 1.1 锁定 History v2 范围，明确吸收旧 search/read 设计但保留 stable target 语义
  - **证据块**: `src-tauri/src/tools/history/mod.rs:14-195` 当前 bootstrap 返回全量摘要/最新 handoff；`mod.rs:386-410` 当前显式 `session_key` 优先；`src-tauri/src/mcp/server.rs:182` 有防重定向回归测试。
  - **涉及文件**: 本规格 3 文件
  - _需求: FR-1 至 FR-7_ ｜ _设计: 设计决策 1-4_
- [x] 1.2 对 History 公共入口执行 GitNexus impact 并通过 `check_spec`
  - **证据块**: `tools/registry.rs:27-59` 三个历史工具；`tools/dispatch.rs` 统一分发；`mcp/server.rs` history metadata 注入。
  - **涉及文件**: 只读分析
  - _需求: FR-6_ ｜ _设计: 测试策略_

### 阶段 2: 事实档案与派生状态

- [x] 2.1 扩展 History 数据模型，加入 initial/raw input、revision、manifest/state/search DTO
  - **证据块**: `src-tauri/src/tools/history/model.rs:62-86` 当前 `CheckpointRecord` 无 raw input/revision/hash。
  - **涉及文件**: `model.rs`，预计新增 90-130 行
  - _需求: FR-2, FR-3, FR-4_ ｜ _设计: 数据模型_
- [x] 2.2 将 Markdown 更新改为追加式 revision，并保持旧格式可解析
  - **证据块**: `markdown.rs:38-126` 当前每次 render 重建聚合章节；`mod.rs:264-278` 同 turn 变化会原位覆盖。
  - **涉及文件**: `markdown.rs`，预计净改 180-260 行
  - _需求: FR-2, FR-6_ ｜ _设计: 决策 4_
- [x] 2.3 增加 manifest/state 原子读写、重建和 latest-revision 投影
  - **证据块**: `storage.rs:227-298` 当前只有 `index.json` JSON 读写与原子替换。
  - **涉及文件**: `storage.rs`，预计新增 220-320 行
  - _需求: FR-3_ ｜ _设计: 决策 2_

### 阶段 3: 有界 bootstrap、search/read 与工具契约

- [x] 3.1 重写 bootstrap/checkpoint/validate，删除全历史响应并接入派生状态
  - **证据块**: `history/mod.rs:108-194` 构造 `all_history_summary/session_summaries/latest_handoff`；`mod.rs:224-331` checkpoint 重写整个文档。
  - **涉及文件**: `history/mod.rs`，预计净改 300-450 行
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: Bootstrap 有界策略_
- [x] 3.2 实现 `history_session_search/read` 的确定性检索与 UTF-8-safe 分页
  - **证据块**: 当前 `history/mod.rs` 无 search/read 公共函数；`storage::scan` 已提供安全档案集合。
  - **涉及文件**: `history/mod.rs`、`storage.rs`、`model.rs`
  - _需求: FR-4, FR-5_ ｜ _设计: Search、Read_
- [x] 3.3 注册五个历史工具并更新 Schema、read-only annotations 与 dispatch
  - **证据块**: `registry.rs:27-59`、`CORE_TOOLS:302+`、`input_schema:497+` 当前只含三工具。
  - **涉及文件**: `registry.rs`、`dispatch.rs`
  - _需求: FR-6_ ｜ _设计: API 设计_
- [x] 3.4 更新 MCP initialize instructions，但保留 history-only host metadata 注入和 stable target 回归语义
  - **证据块**: `mcp/server.rs` 当前 instructions 仍要求读取 `all_history_summary/latest_handoff/inherited_summary`；已有显式 session key 防重定向测试。
  - **涉及文件**: `mcp/server.rs`
  - _需求: FR-1, FR-7_ ｜ _设计: 决策 1、API 设计_

### 阶段 4: 测试、文档与收敛

- [x] 4.1 扩展单元/契约/集成测试覆盖有界 bootstrap、revision、state rebuild、search/read 和 stable target
  - **证据块**: `src-tauri/tests/history_session.rs` 已覆盖 v1 编号、幂等、路径和 repair；`call_tool_contract.rs` 固定 core 工具集合。
  - **涉及文件**: 两个测试文件 + history 模块内单测
  - _需求: FR-1 至 FR-6_ ｜ _设计: 测试策略_
- [x] 4.2 更新 README 与旧 history-session-archive 规格，移除全历史 bootstrap 使用指引
  - **证据块**: README 当前描述三个 History 工具和全历史恢复模型；旧 requirements FR-2 明确要求 `all_history_summary/latest_handoff`。
  - **涉及文件**: `README.md`、`README.en.md`、`docs/specs/history-session-archive/*`
  - _需求: FR-7_ ｜ _设计: 文件结构_
- [x] 4.3 运行格式/编译/全量测试/前端构建/release build，并执行 GitNexus detect-changes 与代码审查
  - **证据块**: 当前基线此前 190 项 Rust 测试、npm check/build、release build 已通过；本轮需重新验证。
  - **涉及文件**: 无新增
  - _需求: FR-1 至 FR-7, NFR-1 至 NFR-4_ ｜ _设计: 测试策略、风险评估_

---

## 检查点

- [x] 阶段 1 完成后：`check_spec` 0 error；History 公共入口 impact 已知
- [x] 阶段 2 完成后：旧 Markdown 可读；同 turn 修订不覆盖；state/manifest 可重建
- [x] 阶段 3 完成后：core profile 有五个历史工具；bootstrap 有界；search/read 可分页恢复原文
- [x] 阶段 4 完成后：全量测试/build/detect-changes/review 通过或风险明确记录

---

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | Bootstrap 有界策略、决策 1/3 | 1.1, 3.1, 3.4, 4.1 | 已完成 |
| FR-2 | 数据模型、决策 4 | 2.1, 2.2, 3.1, 4.1 | 已完成 |
| FR-3 | 数据模型、决策 2 | 2.1, 2.3, 3.1, 4.1 | 已完成 |
| FR-4 | Search | 2.1, 3.2, 3.3, 4.1 | 已完成 |
| FR-5 | Read | 3.2, 3.3, 4.1 | 已完成 |
| FR-6 | API 设计、兼容 | 1.2, 2.2, 3.3, 4.1 | 已完成 |
| FR-7 | API 设计、文件结构 | 3.4, 4.2 | 已完成 |

---

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `docs/specs/history-session-v2/*.md` | 新建 | 约 350 | 本轮规格 |
| `src-tauri/src/tools/history/model.rs` | 修改 | +90~130 | DTO/revision/state/manifest |
| `src-tauri/src/tools/history/markdown.rs` | 修改 | 净 +80~180 | 追加式事实档案 |
| `src-tauri/src/tools/history/storage.rs` | 修改 | 净变化 | 路径、锁、扫描、原子读写 |
| `src-tauri/src/tools/history/{bootstrap,checkpoint}.rs` | 新建 | 约 550 合计 | 写入用例与 stable-target 编排 |
| `src-tauri/src/tools/history/{retrieval,maintenance,state,response}.rs` | 新建 | 约 700 合计 | search/read、repair、派生状态、响应预算 |
| `src-tauri/src/tools/history/mod.rs` | 修改 | 约 160 | 五个公共入口与共享辅助 |
| `src-tauri/src/tools/registry.rs` | 修改 | +50~90 | 工具/Schema |
| `src-tauri/src/tools/dispatch.rs` | 修改 | +2~8 | search/read 分发 |
| `src-tauri/src/mcp/server.rs` | 修改 | +10~30 | instructions/tests |
| `src-tauri/tests/history_session.rs` | 修改 | +200~350 | v2 集成测试 |
| `src-tauri/tests/call_tool_contract.rs` | 修改 | +5~20 | 工具集合 |
| `README.md` / `README.en.md` | 修改 | +20~50 | 使用说明 |
| `src/lib/components/ChatGptSessionPrompt.svelte` | 修改 | 小量 | 新会话提示改为 bounded state + search/read |
| `docs/specs/history-session-archive/*` | 修改 | 小量 | 标记 v1 约束由 v2 supersede |

---

## 检查清单

- [x] Scope-lock 已填
- [x] 每条任务标题具体且可验收
- [x] 每条任务包含当前代码证据
- [x] 每条任务标注涉及文件与预算
- [x] 分阶段合理
- [x] 每条任务回链 FR 与设计章节
- [x] 覆盖矩阵无遗漏
- [x] 阶段 4 包含逐条验收与全量回归
- [x] 全文无模板占位符
