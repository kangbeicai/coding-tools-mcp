# 需求文档：Admin 密码恢复 CLI

## 功能概述

为 Linux-only `coding-tools` 增加本机管理员凭据恢复命令。当用户忘记 Web Admin 密码时，可通过服务器终端执行 `coding-tools admin reset` 清空 Admin 登录凭据并恢复 `/login` 首次设置状态；该操作不得影响 Gateway、MCP、OAuth、Cloudflare、workspace 或其他 secret。README 同步说明当前 Web Admin 登录机制、首次设置、session 生命周期和密码恢复方法。

## 历史经验与坑

- **可复用经验**: Admin 用户名和 Argon2 `password_hash` 已集中存放在 `AdminConfig`；Web Admin 首次设置以空 `password_hash` 作为未配置状态。
- **必须规避的坑**: 正在运行的服务持有进程内 `DataStore` 与 Admin session，因此独立 CLI 修改磁盘配置后必须重启正在运行的 `coding-tools` 才能让恢复状态生效。

## 术语定义

- **Admin reset**: 清除 Web Admin 的用户名/密码哈希，使下一次服务启动进入首次管理员设置流程。
- **Admin session**: Web Admin 登录后保存在服务进程内存中的会话，当前 TTL 为 12 小时。

## 范围边界

**In Scope**
- 新增 `coding-tools admin reset` 本机 CLI。
- reset 仅重置 `AdminConfig.username` 与 `AdminConfig.password_hash`。
- CLI 明确提示用户重启服务并访问 `/login` 重新设置管理员。
- 更新 `coding-tools --help`、`README.md`，并同步 `README.en.md` 的对应说明。
- 增加针对 reset 行为的 Rust 测试。

**Out of Scope**
- Web 页面内的“忘记密码”流程、邮件找回、多用户/RBAC。
- 自动重启正在运行的生产进程。
- 修改 Gateway/OAuth/MCP/Cloudflare/workspace/其他 secrets。

## 需求列表

### FR-1: 本机重置 Admin 登录凭据

**优先级:** Must
**用户故事:** 作为服务器管理员，我想在忘记 Web Admin 密码时通过本机 CLI 重置管理员登录，以便恢复管理台访问。

#### 验收标准（EARS）
1. WHEN 用户执行 `coding-tools admin reset` THEN 系统 SHALL 将 Admin 用户名恢复为默认值 `admin` 并清空 `password_hash`。
2. WHEN reset 成功 THEN 系统 SHALL 明确提示需要重启正在运行的 `coding-tools`，并在重启后访问 `/login` 设置新管理员。
3. IF reset 执行成功 THEN 系统 SHALL 保持 Gateway、Gateway exposure、workspace 和全部非 Admin secret 配置不变。

### FR-2: 文档说明登录与恢复流程

**优先级:** Must
**用户故事:** 作为部署者，我想从 README 了解 Web Admin 的登录和忘记密码恢复方式，以便正确部署和运维。

#### 验收标准（EARS）
1. WHEN 用户阅读 README THEN 文档 SHALL 说明 Web Admin 需要管理员登录、首次访问 `/login` 初始化、session 为进程内 12 小时会话。
2. WHEN 用户忘记密码 THEN 文档 SHALL 给出 `coding-tools admin reset`、重启服务和重新访问 `/login` 的完整恢复步骤。
3. WHEN 文档描述安全边界 THEN 文档 SHALL 区分 Web Admin 登录与 MCP/OAuth 数据平面认证，且不再声称 Web Admin“没有独立管理员认证”。

## 非功能需求

- **NFR-1（性能）**: reset 为单次本地配置写入，不增加 Gateway/Web 热路径开销。
- **NFR-2（安全）**: CLI 不输出旧密码哈希或其他 secret；密码不可逆恢复，只允许重置。
- **NFR-3（兼容性）**: 保持现有配置目录、MCP 协议、Admin HTTP API 与 Linux headless-only 架构不变。

## 依赖关系

- `src-tauri/src/headless.rs`：CLI 路由、帮助文本。
- `src-tauri/src/settings/model.rs`：`AdminConfig` 默认用户名和密码哈希。
- `src-tauri/src/data/store.rs` / `DataStore::update_file`：直接持久化 Admin reset，避免初始化其他 secret。
- `README.md`、`README.en.md`：部署与恢复说明。

## 检查清单

- [x] 已消化当前 Admin Auth 实现与独立进程配置刷新边界
- [x] 需求覆盖核心和忘记密码边界场景
- [x] 每条需求有唯一 ID
- [x] 验收标准使用 EARS 且可测
- [x] 已标注 MoSCoW 优先级
- [x] In/Out of Scope 明确
- [x] 非功能需求明确
- [x] 依赖关系完整
