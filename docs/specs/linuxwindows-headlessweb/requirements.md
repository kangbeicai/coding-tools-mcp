# 需求文档：GitHub Actions 自动 Release

## 功能概述

为当前 Linux / Windows Headless/Web 项目增加可持续验证的 GitHub Actions 自动发布能力。普通 `main` push / PR 只自动执行 CI；Release workflow 只由 `v*` tag 或手动 dispatch 触发，避免版本提交与紧随其后的 tag 重复执行三平台构建。手动 dispatch 可以针对任意 ref 做三平台 build-only 验证，只有显式选择 publish 且 ref 为已有 `v*` tag 时才允许发布。

## 范围边界

**In Scope**
- 新增 `.github/workflows/release.yml`。
- 每个平台都先构建最新 Web Console，再构建 Rust release binary，确保静态资源嵌入二进制。
- 使用原生 GitHub-hosted runners：Ubuntu x86_64、Ubuntu arm64、Windows x86_64。
- 生成统一的 Release 资产名和 `SHA256SUMS`。
- 使用仓库 `GITHUB_TOKEN` 和 `contents: write` 发布 Release。
- CI 在 `main` push / PR 自动运行，不再依赖手工 dispatch 才能发现工作流问题。
- Release workflow 不监听普通 `main` push；三平台 release dry-run 通过手动 dispatch 显式触发。
- 本地 `main` 跟踪 `github/main`；`forgejo` remote 保留备用。

**Out of Scope**
- macOS / Tauri Desktop / 安装包。
- Windows Service installer。
- musl 静态构建与代码签名。
- 修改 MCP / Web Admin / Gateway 运行时接口。

---

## 需求列表

### FR-1: Tag 自动触发发布

**优先级:** Must
**用户故事:** 作为维护者，我希望推送 `v*` tag 后自动进入发布流水线，以便无需本机部署 runner。

#### 验收标准（EARS）
1. WHEN GitHub 收到 `v*` tag push THEN Release workflow SHALL 自动运行。
2. WHEN 普通 commit 被 push 到 `main` THEN Release workflow SHALL NOT 自动运行；CI workflow SHALL 负责常规回归验证。
3. WHEN 维护者手动 dispatch 且 `publish=false` THEN Release workflow SHALL 对指定 ref 执行 verify 与三平台 build 且 SHALL NOT 创建 Release。
4. WHEN 维护者手动 dispatch 且 `publish=true` THEN ref SHALL 为已存在的 `v*` tag，否则 workflow SHALL 失败而不创建 tag。

### FR-2: 三平台原生构建

**优先级:** Must
**用户故事:** 作为 Linux/Windows 用户，我希望下载与平台架构对应的预编译 binary，以便无需安装 Node.js 或 Rust。

#### 验收标准（EARS）
1. WHEN 构建 Linux x86_64 THEN workflow SHALL 产出 `coding-tools-linux-x86_64`。
2. WHEN 构建 Linux arm64 THEN workflow SHALL 在 GitHub ARM64 runner 原生产出 `coding-tools-linux-aarch64`。
3. WHEN 构建 Windows x86_64 THEN workflow SHALL 在 Windows runner 原生产出 `coding-tools-windows-x86_64.exe`。
4. BEFORE 每个平台 Cargo release build THEN workflow SHALL 执行 `npm ci` 与 `npm run build`。

### FR-3: 发布前验证

**优先级:** Must
**用户故事:** 作为维护者，我希望 Release 只在基础校验通过后发布，以避免上传明显损坏的构建。

#### 验收标准（EARS）
1. BEFORE 平台构建 THEN verify job SHALL 执行 `npm run check`、`npm run build` 和 Rust 全量测试。
2. IF verify 或任一平台 build 失败 THEN publish job SHALL 不创建 Release。
3. WHEN `main` 收到普通 push OR PR THEN CI workflow SHALL 自动执行前端构建后再执行 Rust tests/release build，以验证嵌入 Web Console 的真实 binary 构建路径。

### FR-4: Release 资产与校验和

**优先级:** Must
**用户故事:** 作为下载用户，我希望 Release 中有固定文件名与 SHA-256 校验文件，以便 curl/wget 安装并验证完整性。

#### 验收标准（EARS）
1. WHEN 三个平台构建完成 THEN workflow SHALL 聚合三个 binary。
2. WHEN 创建 Release THEN workflow SHALL 上传三个 binary 与 `SHA256SUMS`。
3. WHEN 生成 `SHA256SUMS` THEN 文件 SHALL 只包含发布资产文件名而非 runner 临时绝对路径。

### FR-5: Git 主远端边界

**优先级:** Should
**用户故事:** 作为维护者，我希望当前工作分支默认推 GitHub，同时保留 Forgejo 备用。

#### 验收标准（EARS）
1. WHEN 查看本地 branch tracking THEN `main` SHALL 跟踪 `github/main`。
2. WHEN 查看 remotes THEN `forgejo` SHALL 继续存在且不被自动删除。

---

## 非功能需求

- **NFR-1（安全）**: Release job 仅授予 `contents: write`，其它 jobs 使用最小权限。
- **NFR-2（兼容性）**: 不修改 Headless 运行时代码和 HTTP/MCP 契约。
- **NFR-3（可维护性）**: 不依赖 self-hosted runner、QEMU 或本机 secret；发布认证优先使用 `GITHUB_TOKEN`。
- **NFR-4（Actions 可用性）**: 仓库级 GitHub Actions 必须处于可运行状态；该状态不由 workflow YAML 自动修改，实际发布前以远端 run 为准。

## 依赖关系

- GitHub-hosted `ubuntu-24.04`、`ubuntu-24.04-arm`、`windows-latest` runners。
- `actions/checkout@v6`、`actions/setup-node@v6`、`actions/upload-artifact@v7`、`actions/download-artifact@v8`。
- GitHub CLI `gh release create` 与 workflow `GITHUB_TOKEN`。

## 检查清单

- [x] 需求覆盖 tag、手动发布、三平台构建、验证、Release 资产与 upstream。
- [x] 每条需求有稳定 FR ID。
- [x] 验收标准可测试。
- [x] In/Out of Scope 明确。
- [x] 不恢复 Desktop/Tauri。
