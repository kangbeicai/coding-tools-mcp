# 需求文档：history-session-v2

## 功能概述

在当前 Linux Headless-only `coding-tools-mcp` 主分支上演进 History Session：保留 `docs/history-session/N.md` 作为永久、可审阅、可 Git 化的事实源，把 `history_session_bootstrap` 从全历史回灌改为有界当前状态，并新增 `history_session_search` / `history_session_read` 进行按需检索和无损分页读取。该功能面向通过远程 MCP 持续开发的 ChatGPT 用户，解决历史会话增长后 bootstrap 响应越来越大、旧上下文难以精确定位、同一 turn 修订会覆盖旧证据的问题。

## 历史经验与坑

- **可复用经验**: 历史工具继续使用统一 `tools/registry.rs` + `tools/dispatch.rs::call_tool` 契约；Markdown 事实源、跨进程锁、原子写入、工作区路径边界和敏感信息脱敏继续保留。
- **必须规避的坑**: 不再把 `all_history_summary`、`session_summaries`、`latest_handoff`、`inherited_summary` 注入 bootstrap；不 cherry-pick Desktop 分支；不让变化的 `_meta.openai/session` 重定向一个已经由显式 `session_key + expected_path` 锁定的 checkpoint；不引入 SQLite、向量库、外部模型或额外服务。

## 术语定义

- **事实档案**: `docs/history-session/N.md`，长期无损保存首次输入、每轮输入、结构化 checkpoint 与 revision 证据。
- **派生状态**: `docs/history-session/memory/state.json`，由事实档案重建的有界当前工作状态。
- **Manifest**: `docs/history-session/memory/manifest.json`，只保存档案定位、hash、关键词和时间等元数据。
- **稳定目标**: bootstrap 返回的 `session_key + current_path`；checkpoint 必须以同值作为 `session_key + expected_path`，宿主元数据变化不得重定向。

---

## 范围边界

**In Scope（本次要做）**
- bootstrap 返回有界 state、档案统计、搜索/读取指引，不返回全量历史正文或递归摘要。
- 新增 `history_session_search` 与 `history_session_read` 两个只读工具。
- 新增可重建 `memory/state.json` 与 `memory/manifest.json`。
- bootstrap 支持 `initial_user_input`；checkpoint 支持 `raw_user_input`，缺失或脱敏时返回明确 warning。
- 同一 `turn_id` 同内容重试幂等；不同内容追加 revision/supersedes，不覆盖旧记录。
- 保留当前 stable target、防路径逃逸、secret redaction 和 Linux Headless 架构。
- 更新 README、工具 Schema、MCP instructions 与契约/集成测试。

**Out of Scope（本次不做）**
- Tauri、系统托盘、Windows/macOS Desktop UI、WebView 生命周期或 Desktop release 流程。
- SQLite、向量数据库、embedding、外部 LLM、外部检索服务。
- 自动读取 ChatGPT 未作为 MCP 参数传入的完整对话转录。
- 删除、重编号或覆盖已有历史档案以完成迁移。

---

## 需求列表

### FR-1: 有界 bootstrap 与稳定会话目标

**优先级:** Must
**用户故事:** 作为远程 MCP 用户，我希望新会话初始化只返回足以继续工作的当前状态，并锁定稳定的历史目标，以便历史规模增长或宿主会话元数据变化时仍能可靠恢复。

#### 验收标准（EARS）

1. WHEN `history_session_bootstrap` 成功 THEN 系统 SHALL 返回当前编号、路径、状态版本、档案版本、有限当前状态、历史统计和 search/read 指引。
2. WHEN bootstrap 成功 THEN 系统 SHALL NOT 返回 `all_history_summary`、`session_summaries`、`latest_handoff` 或 `inherited_summary`。
3. WHEN bootstrap 的完整 JSON 结果超过 64 KiB 预算 THEN 系统 SHALL 继续缩减派生 state 字段并标记 `state_truncated=true`，不得截断事实档案。
4. WHEN 同时存在显式 `session_key` 与 `_host_session_key` THEN 系统 SHALL 保留当前分支的显式稳定 `session_key` 优先级，并在不一致时返回 warning。
5. WHEN checkpoint 使用 bootstrap 返回的 `session_key + expected_path` THEN 系统 SHALL 即使 `_host_session_key` 变化也继续写入同一档案；IF target 不匹配 THEN SHALL 返回 `SESSION_TARGET_MISMATCH`。

### FR-2: 无损首次输入、每轮输入与 revision

**优先级:** Must
**用户故事:** 作为持续开发用户，我希望首次请求和每轮请求都能与结构化 checkpoint 一起保留，并能看到同一 turn 的修订历史，以便之后精确恢复和审计。

#### 验收标准（EARS）

1. WHEN 新会话 bootstrap 传入 `initial_user_input` THEN 系统 SHALL 将脱敏后的逐字文本写入当前 `N.md` 并返回 `initial_input_captured=true`。
2. IF bootstrap 未传 `initial_user_input` THEN 系统 SHALL 返回 `initial_input_captured=false` 与 warning，不得宣称已自动读取聊天转录。
3. WHEN checkpoint 传入 `raw_user_input` THEN 系统 SHALL 将其与结构化字段共同归档；IF 缺失 THEN SHALL 返回 `user_input_captured=false` 与 warning。
4. WHEN 相同 `turn_id` 以相同归档内容重试 THEN 系统 SHALL 不重复追加并返回 `duplicate_ignored=true`。
5. WHEN 相同 `turn_id` 的归档内容变化 THEN 系统 SHALL 追加 `revision-N` 并记录 `supersedes`，旧 revision 永久保留；派生 state 只投影最新 revision。
6. WHEN 输入命中现有敏感信息规则 THEN 系统 SHALL 继续写入 `[REDACTED]` 并返回 warning，不回显原秘密。

### FR-3: 可重建 state 与 manifest

**优先级:** Must
**用户故事:** 作为维护者，我希望快速状态和索引可以从 Markdown 重建，以便派生文件损坏不会导致历史丢失。

#### 验收标准（EARS）

1. WHEN bootstrap 或 checkpoint 完成 THEN 系统 SHALL 原子写入 `memory/manifest.json` 与 `memory/state.json`。
2. WHEN manifest/state 缺失、损坏或 archive revision 不匹配 THEN 系统 SHALL 从数字 Markdown 档案重建派生文件。
3. WHEN `history_session_validate(repair=true)` 运行 THEN 系统 SHALL 只重建 `index.json`、manifest 和 state，不修改已有 `N.md`。
4. WHEN 构建 manifest THEN 系统 SHALL 保存编号、路径、标题、时间、字节数、SHA-256 与有限关键词，不复制档案全文。

### FR-4: 按需历史搜索

**优先级:** Must
**用户故事:** 作为恢复旧上下文的用户，我希望按关键词找到相关历史档案，以便只加载当前任务需要的部分。

#### 验收标准（EARS）

1. WHEN 调用 `history_session_search` THEN 系统 SHALL 对 manifest 元数据和事实档案进行确定性、大小写不敏感关键词匹配，并返回有限结果页。
2. WHEN query 为空 THEN 系统 SHALL 按最近更新时间返回有限 manifest 条目，不返回全部正文。
3. WHEN query 有匹配 THEN 每个结果 SHALL 包含编号、路径、标题、更新时间、SHA-256、分数和有限 UTF-8 snippet。
4. WHEN 提供 `cursor` / `limit` THEN 系统 SHALL 稳定分页，`limit` 最大 50；无结果时 SHALL 返回空数组而非全历史回退。

### FR-5: 无损分页读取

**优先级:** Must
**用户故事:** 作为需要精确旧上下文的用户，我希望分页读取某个原始 Markdown 档案，以便不经过摘要即可恢复全部细节。

#### 验收标准（EARS）

1. WHEN 调用 `history_session_read` 并传合法编号或 search 返回路径 THEN 系统 SHALL 返回原始 UTF-8 内容页、总字节数、内容 SHA-256 和 `next_cursor`。
2. WHEN 未传 `max_bytes` THEN 系统 SHALL 使用 32 KiB；IF 大于 64 KiB THEN SHALL 拒绝请求。
3. WHEN 分页边界落在多字节字符中 THEN 系统 SHALL 调整到合法 UTF-8 字符边界，不产生损坏文本。
4. WHEN 调用方传 `expected_hash` 且档案已变化 THEN 系统 SHALL 返回可恢复冲突错误，避免静默拼接两个版本。
5. IF path 不是当前历史目录中的纯数字 Markdown 或 cursor 非法 THEN 系统 SHALL 返回结构化错误且不得读取其他工作区文件。

### FR-6: 工具契约与迁移兼容

**优先级:** Must
**用户故事:** 作为现有 MCP 用户，我希望升级 History v2 后其他工具和已有档案仍然可用，以便升级不造成开发工作流回归。

#### 验收标准（EARS）

1. WHEN 客户端调用 `tools/list` THEN core profile SHALL 暴露五个历史工具：bootstrap、checkpoint、validate、search、read。
2. WHEN 调用任意历史工具 THEN 系统 SHALL 继续通过统一 `call_tool` 分发入口，并只为 history 工具注入宿主会话元数据。
3. WHEN 扫描旧格式 `N.md` THEN 系统 SHALL 继续解析原 metadata/checkpoint；迁移不得要求删除旧文件。
4. WHEN 其他现有工具执行 THEN 系统 SHALL 保持原有输入、输出、权限和执行路径不变。
5. WHEN 构建项目 THEN 系统 SHALL 不增加 Desktop/Tauri、SQLite、向量库或外部模型依赖。

### FR-7: 文档与 ChatGPT 使用指引

**优先级:** Must
**用户故事:** 作为项目使用者，我希望 README 与 MCP instructions 准确描述新的历史恢复方式，以便模型和人工用户不会继续依赖已删除的全历史 bootstrap 字段。

#### 验收标准（EARS）

1. WHEN MCP initialize 返回 instructions THEN SHALL 指示 bootstrap 传 `initial_user_input`、checkpoint 传 `raw_user_input`，精确旧上下文通过 search→read 获取。
2. WHEN README 描述 History Session THEN SHALL 列出五个工具，并说明 Markdown 是事实源、state/manifest 为可重建派生文件。
3. WHEN 文档描述稳定目标 THEN SHALL 明确 checkpoint 必须复用 bootstrap 返回的 `session_key + current_path`，不得被宿主元数据变化重定向。

---

## 非功能需求

- **NFR-1（性能）**: bootstrap 序列化结果目标不超过 64 KiB；read 单页默认 32 KiB、最大 64 KiB；100 个总计 10 MiB 档案的 validate/search 应保持本地可交互延迟。
- **NFR-2（安全）**: 所有历史路径限制在当前 Workspace；事实档案继续应用现有敏感信息脱敏；read/search 不允许越界读取。
- **NFR-3（兼容性）**: 产品运行时只要求 Linux Headless；旧数字 Markdown 与 `index.json` 可继续读取，派生 v2 文件可随时重建。
- **NFR-4（完整性）**: bootstrap、validate、派生文件重建不得修改旧数字 Markdown；同 turn 修订保留 supersedes 证据。

---

## 依赖关系

- `src-tauri/src/tools/history/{mod,model,markdown,storage}.rs`
- `src-tauri/src/tools/{registry,dispatch}.rs`
- `src-tauri/src/mcp/server.rs`
- `src-tauri/tests/history_session.rs`、`src-tauri/tests/call_tool_contract.rs`
- `README.md`、`README.en.md`

---

## 检查清单

- [x] 已消化旧 `cd4e6c9` 的有界状态/search/read 经验，并排除其不适合当前分支的 session-key 优先级变化
- [x] 需求覆盖有界恢复、无损输入、revision、search/read、派生状态、迁移兼容和文档
- [x] 每条需求有唯一 ID，并将在 design/tasks 中回链
- [x] 验收标准使用 EARS 且可测
- [x] 已标注 MoSCoW 优先级
- [x] In/Out of Scope 明确
- [x] 非功能需求明确
- [x] 依赖关系完整
