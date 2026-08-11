# 需求文档：cloudflared-auto-download

## 功能概述

当用户启动 Cloudflare exposure 时，`coding-tools` 继续优先使用系统已经安装的 `cloudflared`。如果 PATH、平台常见路径和应用缓存都找不到可执行文件，则自动从 Cloudflare 官方 GitHub Release 下载当前平台/架构对应的最新 `cloudflared` 到应用配置目录缓存，然后继续现有 Quick Tunnel 或 Named Tunnel 启动流程。该行为不增加新的 Web 按钮或确认步骤。

## 历史经验与坑

- **可复用经验**: 复用 `frpc` 已有的按需安装模式、下载镜像 fallback、下载代理配置和配置目录缓存。
- **必须规避的坑**: 不在同步配置校验阶段做网络下载；下载只能发生在实际启动 Cloudflare exposure 的异步路径。并发启动必须避免多个任务同时覆盖同一个缓存文件。

## 术语定义

- **resolve**: 只查找本机现有 binary，不产生网络或写盘副作用。
- **ensure**: 先 resolve；缺失时下载并安装到缓存，最终返回可执行路径。

---

## 范围边界

**In Scope**
- Linux x86_64 自动下载 `cloudflared-linux-amd64`。
- Linux aarch64 自动下载 `cloudflared-linux-arm64`。
- Windows x86_64 自动下载 `cloudflared-windows-amd64.exe`。
- 使用 GitHub `releases/latest/download` 官方资产地址。
- 复用 `download.githubMirror` 与 download proxy 配置。
- Linux 下载后设置可执行权限。
- 并发启动时只执行一次缺失 binary 的安装流程。
- 更新中英文 README。

**Out of Scope**
- 不内嵌 `cloudflared` 到 `coding-tools` Rust binary。
- 不新增 Web 下载按钮、下载确认框或独立安装 API。
- 不自动更新已经存在的 `cloudflared`。
- 不增加 macOS 支持。
- 不改变 FRP 自动下载行为。

---

## 需求列表

### FR-1: 优先复用已有 cloudflared

**优先级:** Must
**用户故事:** 作为已安装 Cloudflare CLI 的用户，我希望启动 tunnel 时继续使用现有安装，避免无意义下载。

#### 验收标准（EARS）

1. WHEN PATH、平台常见路径或应用缓存存在 `cloudflared` THEN 系统 SHALL 直接使用该路径且不触发下载。
2. WHEN `resolve_cloudflared` 被单独调用 THEN 系统 SHALL 只执行本地查找，不产生网络请求。

### FR-2: 启动 Cloudflare 时按需自动下载

**优先级:** Must
**用户故事:** 作为首次使用 Cloudflare exposure 的用户，我希望无需手动安装 cloudflared 即可直接启动 tunnel。

#### 验收标准（EARS）

1. WHEN 用户实际启动 Cloudflare exposure 且本机不存在 `cloudflared` THEN 系统 SHALL 自动下载当前受支持平台/架构对应的官方最新 binary。
2. WHEN 下载成功 THEN 系统 SHALL 将 binary 安装到应用配置目录的 `bin/cloudflared` 或 `bin/cloudflared.exe`，随后继续原有 tunnel 启动。
3. IF 当前平台不在 Linux x86_64、Linux aarch64、Windows x86_64 范围 THEN 系统 SHALL 返回明确 unsupported 错误。

### FR-3: 下载复用现有网络配置并安全落盘

**优先级:** Must
**用户故事:** 作为受网络限制环境中的用户，我希望 cloudflared 下载沿用项目已有的 GitHub mirror 和下载代理设置。

#### 验收标准（EARS）

1. WHEN 配置 GitHub mirror THEN 下载 SHALL 先尝试镜像 URL，失败后回退官方 GitHub URL。
2. WHEN配置 download proxy THEN 下载 SHALL 复用现有下载客户端行为。
3. WHEN Linux 安装完成 THEN 文件 SHALL 具备可执行权限。
4. IF 下载或写盘失败 THEN 系统 SHALL 返回明确错误且不得把不完整文件当成可用 binary。

### FR-4: 并发与幂等

**优先级:** Should
**用户故事:** 作为同时启动多个 exposure 的用户，我希望缺失 cloudflared 时不会发生重复下载或缓存竞争。

#### 验收标准（EARS）

1. WHEN 多个任务同时调用 ensure 且 binary 缺失 THEN 系统 SHALL 串行化安装阶段。
2. WHEN 第一个任务完成安装 THEN 后续等待任务 SHALL 重新 resolve 并直接复用缓存结果。

### FR-5: 不增加用户交互分支

**优先级:** Must
**用户故事:** 作为 Web Admin 用户，我希望继续使用现有“启动 Cloudflare”动作，不需要先处理额外安装界面。

#### 验收标准（EARS）

1. WHEN 用户启动 Cloudflare Quick 或 Named Tunnel THEN 前端 SHALL 不新增 cloudflared 下载按钮或确认步骤。
2. WHEN仅保存/校验 tunnel 配置而未实际启动 THEN 系统 SHALL 不进行 cloudflared 网络下载。

---

## 非功能需求

- **NFR-1（安全）**: 下载仅允许固定的 Cloudflare 官方 GitHub Release 资产模板；不接受用户传入任意下载 URL。
- **NFR-2（兼容性）**: 已安装 cloudflared、FRP、Gateway、OAuth、Web Admin 的现有行为保持不变。
- **NFR-3（可靠性）**: 下载成功后再把目标路径视为可用，避免并发或失败留下半成品被后续启动误用。

---

## 依赖关系

- 复用 `src-tauri/src/tunnel/download.rs` 的 GitHub mirror/proxy 下载能力。
- 复用 `platform().app_config_dir()` 与现有 `cached_cloudflared_path()`。
- 官方资产名以 Cloudflare 最新 Release 当前提供的 Linux amd64、Linux arm64、Windows amd64 executable 为准。
