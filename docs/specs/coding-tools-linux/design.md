# 设计文档：Admin 密码恢复 CLI

## 概述

本设计覆盖 FR-1、FR-2 以及 NFR-1 至 NFR-3。恢复能力只存在于 Linux 本机 CLI，不向 Web 或公网增加未认证的密码重置接口。

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 恢复入口 | `coding-tools admin reset` | 本机终端已经拥有配置文件权限，不扩大网络攻击面 | FR-1 |
| 配置更新 | `DataStore::update_file` | 直接改持久化 `AppData`，不会触发 `AppState::new()` 的 shared-secret 初始化 | FR-1 |
| 文档 | `README.md` + `README.en.md` | 中英文入口保持一致 | FR-2 |

### 架构设计

```text
Local shell
  -> coding-tools admin reset
  -> DataStore::update_file()
  -> data.admin.username = "admin"
  -> data.admin.password_hash = ""
  -> print restart + /login instructions

restart coding-tools
  -> Admin listener sees empty password_hash
  -> /login enters first-time setup
```

reset 不尝试连接或控制正在运行的服务。原因是当前生产部署既可能是前台独立进程，也可能是 systemd，CLI 不应猜测或强制重启方式。

## 数据模型

不新增数据结构。只修改现有 `AdminConfig` 的两个字段：

| 字段 | reset 后值 | 说明 |
|------|------------|------|
| `admin.username` | `admin` | 恢复默认管理员用户名 |
| `admin.password_hash` | 空字符串 | 触发现有首次设置状态 |

其他配置字段保持原值。

## API 设计

| 方法/函数 | 签名/入口 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| CLI | `coding-tools admin reset` | 无 | 成功提示或错误 | FR-1 |
| helper | `reset_admin_credentials(&mut AppSettings)` 或等价纯函数 | 可变 settings | 无 | FR-1 |

不新增 Web API。

## 文件结构

```text
src-tauri/src/headless.rs                 # 增加 admin 子命令、reset helper、帮助文本与测试
README.md                                 # 中文登录/恢复说明
README.en.md                              # 英文登录/恢复说明
docs/specs/coding-tools-linux/            # 本功能规格
```

## 设计决策

### 决策 1: 本机 CLI，而不是 Web reset endpoint（FR-1）

**问题**: 忘记密码时用户无法通过已认证 Web API 发起重置。
**决策**: 只提供本机 CLI reset。
**理由**: 服务器本机 OS 账户已经是信任边界；不需要新增公网或未认证管理接口。

### 决策 2: reset 后要求显式重启（FR-1）

**问题**: 独立 CLI 进程写入磁盘配置后，正在运行的 server 仍持有旧的内存设置和 session store。
**决策**: reset 只写配置并明确提示重启，不自动杀进程。
**理由**: 避免误判 systemd/前台/外部进程托管方式；运维动作保持可控。

### 决策 3: 不通过 `AppState::new()` 执行 reset（FR-1）

**问题**: `AppState::new()` 会调用 `init_shared_secrets()`，在缺失 shared secret 的配置上可能产生与密码恢复无关的写入。
**决策**: reset 直接使用 `DataStore::update_file` 修改 `AppData.admin`。
**理由**: 严格满足“只改变 Admin username/password hash”的字段隔离承诺。

## 测试策略

- Rust 单测：构造包含 Admin 凭据、Gateway 和其他 secrets 的 `AppSettings`，执行 reset helper 后断言仅 Admin 两字段变化。
- CLI smoke：release/debug binary 的 `--help` 包含 `admin reset`；可选隔离 XDG 配置执行 reset 后验证密码哈希清空。
- 回归：`cargo check --all-targets`、`cargo test --all-targets`、`git diff --check`。
- README：确认中文与英文均不再包含“Admin 无认证”的过时描述，并包含 reset 步骤。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| CLI 路由改动触及 `run_from_env` | 中 | GitNexus impact + 全量 Rust 回归 |
| 用户误以为 reset 立即影响正在运行服务 | 中 | CLI 与 README 双重提示必须重启 |
| reset 意外覆盖其他配置 | 高 | 抽纯 helper 并做字段不变性单测 |

## 检查清单

- [x] 技术方案与 Linux 单 CLI 架构一致
- [x] FR-1、FR-2 均有设计覆盖
- [x] 文件路径来自真实代码库
- [x] 数据与 CLI 契约明确
- [x] 关键安全决策已记录
- [x] 测试策略可验证验收标准
