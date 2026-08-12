# 设计文档：history-session-v2

## 概述

History Session v2 将当前“bootstrap 回灌全量摘要 + 最新全文”的恢复模型改为“Markdown 永久事实档案 + 有界派生状态 + 按需 search/read”。实现继续位于现有 `tools/history` 领域模块，不引入新进程、数据库或 Desktop 运行时，并保留当前分支的稳定 checkpoint target 保护。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, FR-7, NFR-1, NFR-2, NFR-3, NFR-4

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 永久存储 | 数字 Markdown `N.md` | 现有事实源，可审阅、Git 友好、无需迁移数据库 | FR-2, FR-3, FR-6 |
| 派生状态 | `memory/state.json` | bootstrap 只需有界当前状态；损坏可重建 | FR-1, FR-3 |
| 档案索引 | `memory/manifest.json` | 保存定位/hash/关键词，不复制正文 | FR-3, FR-4 |
| 搜索 | Rust 确定性关键词评分 | 零外部依赖，结果可测、可分页 | FR-4 |
| 读取 | UTF-8-safe byte cursor | 精确无损，单次远程负载有界 | FR-5 |
| 一致性 | 现有 `fs2` 锁 + 单文件原子 rename + `memory/snapshot.json` 最后提交标记 | 事实源不变；派生文件跨文件写入中断时可检测 stale/incomplete | FR-2, FR-3 |
| 敏感信息 | 现有 regex redaction | 保持历史仓库安全边界 | FR-2, NFR-2 |

### 架构设计

```text
ChatGPT tools/call
  -> mcp/server.rs: history tool only 注入 _host_session_key
  -> tools/dispatch.rs::call_tool
  -> tools/history/mod.rs
       ├── bootstrap/checkpoint/validate
       ├── search/read
       ├── markdown.rs 事实档案解析/追加/revision/redaction
       ├── storage.rs  锁/扫描/原子写/state/manifest/snapshot
       └── model.rs    record/state/manifest/search DTO

docs/history-session/
  1.md                  <- 永久事实源
  2.md
  index.json            <- 兼容映射，可重建
  memory/
    state.json          <- 有界当前状态，可重建
    manifest.json       <- 有界定位元数据，可重建
    snapshot.json       <- 派生 generation 最后提交标记，可重建
```

核心原则：`N.md` 是唯一不可替代事实；`index.json`、state、manifest、snapshot 都是派生物。bootstrap、validate 和派生重建不能为了方便而改写旧 `N.md`。`snapshot.json` 不是跨文件事务，而是最后写入的 commit marker，用于检测派生文件半写入或过期。

---

## 数据模型

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| `CheckpointRecord.raw_user_input` | String | 缺失允许，但 checkpoint 返回 partial fidelity | 本轮用户逐字输入（脱敏后） |
| `CheckpointRecord.revision` | u64 | 从 1 递增 | 同 turn 的修订序号 |
| `CheckpointRecord.supersedes` | Option<String> | 指向上一 revision | 保留证据而非覆盖 |
| `CheckpointRecord.content_hash` | String | SHA-256 | 忽略 timestamp/revision 后的幂等 fingerprint |
| `InitialInputRecord` | struct | revision + hash | 首次用户输入与修订 |
| `MemoryManifest` | JSON | version=2 | archive revision + entries |
| `ManifestEntry` | JSON | 不含正文 | number/path/title/time/bytes/hash/keywords |
| `MemoryState` | JSON | version=3；严格条目/字符上限 | 当前焦点；当前 session 内近期变化；最新 checkpoint 的未决项快照；有限 references |
| `DerivedSnapshot` | JSON | version=1 | `archive_revision + state_revision`，在 index/manifest/state 全部写成功后最后提交 |
| `SearchHit` | JSON | snippet 有界 | search 返回定位结果 |

事实 Markdown 使用追加块：

```markdown
## 首次用户输入

### initial-input revision-1
```json
{ "raw_user_input": "...", "content_hash": "..." }
```

## 本轮检查点

### turn-123 revision-1
```json
{ "turn_id": "turn-123", "revision": 1, "raw_user_input": "..." }
```

### turn-123 revision-2
```json
{ "turn_id": "turn-123", "revision": 2, "supersedes": "turn-123 revision-1" }
```
```

旧格式 checkpoint JSON 仍可反序列化，新增字段均带 serde default。

---

## API 设计

| 方法/函数 | 入参 | 出参 | 关联需求 |
|-----------|------|------|----------|
| `history_session_bootstrap` | 现有参数 + `initial_user_input` | bounded state、revision、统计、warnings、search/read guide | FR-1, FR-2 |
| `history_session_checkpoint` | 现有 stable target + `raw_user_input` | revision/supersedes/capture/hash + `fidelity` + `persistence_complete` | FR-2 |
| `history_session_search` | `query,cursor,limit,history_dir` | ranked bounded hits + next_cursor | FR-4 |
| `history_session_read` | `number|path,cursor,max_bytes,expected_hash` | raw page + hash + next_cursor | FR-5 |
| `history_session_validate` | `repair` | sequence validity、archive integrity、malformed blocks、派生 freshness/snapshot consistency | FR-3 |

### Bootstrap 有界策略

1. `MemoryState` 本身限制每类条目数量和文本字符数。
2. 序列化 bootstrap payload；若超过 64 KiB，清空 recent/open/references 并缩短 current_focus，设置 `state_truncated=true`。
3. 永不通过截断 `N.md` 来满足预算。

### Search

- query 按非字母数字字符切词、小写化、去重。
- 标题命中权重最高，其次 manifest keywords，再次正文。
- 结果按 score、updated_at、number 稳定降序。
- 空 query 返回最近 manifest entries；limit 最大 50。

### Read

- 仅接受已扫描出的 `N.md` number 或完全匹配的相对 path。
- 默认 32768 bytes，最大 65536 bytes。
- cursor/end 必须落在 UTF-8 char boundary；必要时向前收缩 end。
- `expected_hash` 不匹配返回 `HISTORY_ARCHIVE_CHANGED`。

---

## 文件结构

```text
src-tauri/src/tools/history/model.rs      # 扩展 record/state/manifest/search DTO
src-tauri/src/tools/history/markdown.rs   # 追加式事实档案与 revision
src-tauri/src/tools/history/storage.rs    # 路径、锁、扫描、派生 JSON/Markdown 原子读写
src-tauri/src/tools/history/state.rs      # manifest/state 构建与最新 revision 投影
src-tauri/src/tools/history/bootstrap.rs  # bootstrap 创建/恢复与有界状态编排
src-tauri/src/tools/history/checkpoint.rs # stable target checkpoint 与 revision 编排
src-tauri/src/tools/history/retrieval.rs  # search/read 与 UTF-8-safe 分页
src-tauri/src/tools/history/maintenance.rs# validate/repair 派生文件
src-tauri/src/tools/history/response.rs   # bootstrap 64 KiB 响应收缩与派生文件 warning
src-tauri/src/tools/history/mod.rs        # 五个 public use case 入口与公共错误/路径辅助
src-tauri/src/tools/registry.rs           # 工具列表与 Schema
src-tauri/src/tools/dispatch.rs           # search/read 分发
src-tauri/src/mcp/server.rs               # history instructions/metadata contract
src-tauri/tests/history_session.rs        # 端到端行为
src-tauri/tests/call_tool_contract.rs     # core profile/tool catalog
README.md / README.en.md                  # 用户文档
docs/specs/history-session-archive/*      # 标记旧全量 bootstrap 设计被 v2 supersede
```

---

## 设计决策

### 决策 1: 保留当前 stable session target，不采用旧分支 host-key 优先级（FR-1）

**问题**: 旧 `cd4e6c9` 把 `_host_session_key` 放到显式 `session_key` 之前，会让宿主元数据变化有机会重定向历史。

**选项**:
1. 旧分支行为：host key 永远优先。
2. 当前分支行为：bootstrap 显式 session key 优先；checkpoint 强制 `session_key + expected_path`。

**决策**: 选择 2。

**理由**: 当前已有回归测试保护该行为；稳定目标对跨会话/连接器元数据变化更安全。

### 决策 2: Markdown 是事实源，state/manifest 都可丢弃重建（FR-3）

**问题**: 如何同时保证快速恢复和长期无损。

**决策**: `N.md` 永久保存；JSON 只作为物化视图。validate repair 只重建派生文件。

### 决策 3: search/read 代替 bootstrap 全历史回灌（FR-1, FR-4, FR-5）

**问题**: 历史增长后远程 MCP 初始化负载无界。

**决策**: bootstrap 只返回 bounded state；精确旧上下文由 search→read 显式拉取。

### 决策 4: 同 turn 内容变化追加 revision，不原位覆盖（FR-2）

**问题**: 网络重试需要幂等，但模型修订也需要证据。

**决策**: 对“同 fingerprint”忽略；对“同 turn 不同 fingerprint”追加 revision/supersedes。

### 决策 5: `open_items` 是当前 session 最新 checkpoint 的状态快照，不是历史事件并集（FR-1, FR-3）

**问题**: v2 初版把所有历史档案的 `remaining_issues/next_actions` 累加到 `state.json`，导致后续已经完成的旧事项仍被投影为当前待办。

**决策**: MemoryState v3 只从当前 session 最新有效 checkpoint 读取 `remaining_issues + next_actions`；当前 session 内的 `recent_changes` 可有界汇总，旧 session 只进入 `references` 并通过 search/read 按需读取。

**理由**: checkpoint 中的未决事项字段语义是“该时刻的当前状态”，后一 checkpoint 的空数组天然表示旧待办已清空，无需文本相似度或 NLP 推断 resolved 状态。

### 决策 6: 解析损坏显式诊断，派生文件用 commit marker 检测不一致（FR-3）

**问题**: v2 初版反序列化失败会静默跳过 JSON block；index/manifest/state 又分别原子替换但不是同一事务。

**决策**: parser 同时返回有效 records 与 diagnostics，scan/validate 暴露 `malformed_blocks` 和 `archive_integrity_valid`。派生写入顺序固定为 index → manifest → state → snapshot，最后的 snapshot 绑定 archive/state revision；validate 对每个派生文件做 freshness 检查并返回 `consistent/stale/incomplete/invalid`。

**理由**: 保留 Markdown 事实源和简单单文件原子写，同时把“静默丢结构化记录”和“半写入看起来像成功”改成可观察、可 repair 的状态。

---

## 测试策略

- **单元**: Markdown 首次输入/多 revision 解析；fingerprint 忽略 timestamp/revision；UTF-8 分页；tokenize/truncate；state latest-revision 投影。
- **契约**: core profile 从 24 增至 26 个工具；search/read Schema 与 read-only annotations；dispatch 分支一致。
- **集成**: bootstrap 不含旧全量字段且响应有界；缺 input warning；stable target 不受 host metadata 重定向；manifest/state 删除后 repair 可重建；search 命中原始用户输入；read 多页完整还原且 hash 变化报错。
- **回归**: 全量 `cargo test --all-targets`、`cargo check --all-targets`、前端 `npm run check/build`、release build。
- **变更审查**: GitNexus 对 `history::bootstrap/checkpoint/validate`、registry/dispatch/mcp server 做 impact，提交前 `detect-changes`。

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 旧 Markdown 格式解析回归 | 高 | 新字段 serde default；用现有 history-session fixtures 回归 |
| 派生 state 与档案漂移 | 中 | archive_revision/hash 校验，缺失/失配即重建 |
| bootstrap 仍可能过大 | 高 | state 固定上限 + 二次 64 KiB budget 收缩测试 |
| read 越界或 UTF-8 损坏 | 高 | 只从 scan report 选档案；严格 cursor char boundary 测试 |
| session target 被 host metadata 重定向 | 高 | 保留当前 explicit session key + expected_path 契约测试 |
| 当前生产进程仍运行旧 binary | 中 | 本轮只改源码与构建；未经用户明确授权不重启生产 |

---

## 检查清单

- [x] 技术方案与 Linux Headless 架构一致
- [x] requirements.md 全部 FR 已覆盖
- [x] 文件路径均来自当前代码库
- [x] 数据模型与接口约束明确
- [x] 关键设计决策已记录并关联需求
- [x] 测试策略可验证验收标准
