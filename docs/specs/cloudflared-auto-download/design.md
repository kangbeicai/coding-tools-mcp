# 设计文档：cloudflared-auto-download

## 概述

本设计把当前 `resolve_cloudflared()` 的“缺失即报错”改造成 resolve/ensure 两层：resolve 继续纯本地查找；新增异步 `ensure_cloudflared()` 在实际启动 tunnel 时调用，缺失才下载。配置校验保持无网络副作用，Web 不新增交互。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5, NFR-1, NFR-2, NFR-3

---

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 下载地址 | `https://github.com/cloudflare/cloudflared/releases/latest/download/<asset>` | Cloudflare 官方下载页提供 latest release binary | FR-2, NFR-1 |
| 网络层 | 复用 `download_release_asset` | 已有 mirror + proxy + fallback | FR-3 |
| 缓存 | `<app-config>/bin/cloudflared(.exe)` | 当前 resolve 已支持该位置 | FR-1, FR-2 |
| 并发控制 | 进程内 Tokio Mutex + lock 后二次 resolve | 避免并发重复安装 | FR-4 |
| 启动触发 | `spawn_cloudflare_tunnel` 内调用 ensure | 只有实际启动时下载 | FR-2, FR-5 |

### 架构设计

```text
Web Admin: Start Cloudflare
          |
          v
Tunnel supervisor
          |
          v
spawn_cloudflare_tunnel()
          |
          v
ensure_cloudflared()
   | resolve success --------------------> existing binary
   |
   + missing -> download lock
                    |
                    + re-resolve success -> cached binary
                    |
                    + still missing
                         -> asset_for(OS, ARCH)
                         -> download_release_asset()
                         -> temporary file
                         -> chmod 0755 on Unix
                         -> install cache path
                         -> continue spawn
```

同步的 `validate_tunnel_requirements()` 只校验 tunnel 类型、token、URL 等配置，不再要求 `cloudflared` 已提前存在。

---

## 数据模型

不新增持久化配置。继续使用现有 `AppSettings.download`：

| 字段 | 用途 |
|------|------|
| `github_mirror` | GitHub Release 下载镜像前缀 |
| `proxy_mode` / `proxy_url` | 下载代理配置 |

---

## API 设计

| 函数 | 契约 | 关联需求 |
|------|------|----------|
| `resolve_cloudflared() -> AppResult<PathBuf>` | 仅查本地现有 binary | FR-1 |
| `ensure_cloudflared() -> async AppResult<PathBuf>` | resolve；缺失时安装并返回缓存路径 | FR-2, FR-3, FR-4 |
| `cloudflared_release_asset(os, arch)` | 返回固定官方 asset 名，未知平台报错 | FR-2, NFR-1 |

---

## 文件结构

```text
src-tauri/src/tunnel/
├── cloudflare.rs       # ensure、资产映射、安装、启动调用
├── download.rs         # 复用下载 client/mirror/proxy
└── supervisor.rs       # 配置校验不再检查 binary 是否预装

README.md
README.en.md
docs/specs/cloudflared-auto-download/
```

---

## 设计决策

### 决策 1: 下载发生在 spawn 而不是 validation（关联需求: FR-2, FR-5）

**问题**: 当前 `validate_tunnel_requirements()` 是同步配置校验，如果在这里下载会把网络副作用混入“校验/保存配置”。

**决策**: validation 移除 binary presence 检查；`spawn_cloudflare_tunnel()` 调 `ensure_cloudflared().await`。

**理由**: 用户只有真正点击启动 Cloudflare 时才触发网络下载，且无需增加前端交互。

### 决策 2: 下载 binary 本体，不打包安装器（关联需求: FR-2）

Linux 直接下载 `cloudflared-linux-amd64` / `cloudflared-linux-arm64`，Windows 下载 `cloudflared-windows-amd64.exe`。不处理 deb/rpm/msi，从而无需管理员权限。

### 决策 3: 不自动更新已有 binary（关联需求: FR-1, NFR-2）

只在 resolve 失败时下载。已有 PATH 或缓存版本继续使用，避免每次启动产生网络请求和版本漂移。

---

## 测试策略

- 单元测试 asset mapping：Linux x86_64、Linux aarch64、Windows x86_64 和 unsupported。
- 单元测试安装后的 Unix permission helper（可在 Unix 条件下验证）。
- 现有 Cloudflare URL/parser 与 tunnel supervisor 测试保持通过。
- Rust `cargo test --all-targets` 全量回归。
- Release build 通过。
- 不在测试中真实请求 GitHub，避免网络依赖；下载 URL 模板由官方 release 资产命名验证。

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 公共 tunnel 启动链变化 | 高 | resolve 保持纯查找，只把 ensure 放在 spawn 入口；全量 tunnel/Gateway 回归 |
| 多任务同时首次启动 | 中 | Tokio Mutex + lock 后二次 resolve |
| 下载失败留下半文件 | 中 | 写临时文件后再安装目标路径，失败不返回成功 |
| GitHub 受限 | 中 | 复用 mirror + direct fallback + download proxy |
