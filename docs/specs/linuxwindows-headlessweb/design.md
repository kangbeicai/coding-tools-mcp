# 设计文档：GitHub Actions 自动 Release

## 概述

保留轻量 CI workflow，并让 Release workflow 既承担 tag 发布，也承担 `main` 上的三平台 native build 验证。每个平台独立执行前端生产构建后再执行 Cargo release，从而继续满足 `build.rs` 将 `build/` 静态资源嵌入 `coding-tools` binary 的现有约束。普通 `main` push / PR 自动运行 CI；Release workflow 的 publish job 默认关闭，只有 tag 发布或显式手动 publish 才获得写权限。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5

## 技术方案

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| Linux x86_64 | `ubuntu-latest` | GitHub 标准 x64 runner | FR-2 |
| Linux arm64 | `ubuntu-24.04-arm` | 原生 arm64，无需 cross/QEMU | FR-2 |
| Windows x86_64 | `windows-latest` | 原生 MSVC Windows runner | FR-2 |
| 中间产物 | `upload-artifact@v7` / `download-artifact@v8` | build 与 publish 解耦 | FR-4 |
| Release | `gh release create` | GitHub 官方 CLI，可直接使用 `GITHUB_TOKEN` | FR-1, FR-4 |

### 流程

```text
main push / tag v* / workflow_dispatch
          |
          v
        verify
  npm check + build
  cargo test --all-targets
          |
          +---------------------------+
          |             |             |
          v             v             v
     linux-x64      linux-arm64    windows-x64
     npm build      npm build      npm build
     cargo release  cargo release  cargo release
          |             |             |
          +------ upload artifacts ---+
                        |
                        |
              +---------+---------+
              |                   |
              v                   v
       main/manual dry-run     tag/manual publish
          artifacts only          publish
                              SHA256SUMS
                              gh release create
```

## 数据模型 / API

不涉及运行时数据模型或产品 API。仅新增 CI/CD 配置。

## 文件结构

```text
.github/workflows/release.yml                 # 新增
docs/specs/linuxwindows-headlessweb/*.md      # 新增
README.md / README.en.md                      # 修改：Release 下载说明
docs/graph-insights/latest.md                 # 修改：发布架构说明
```

## 设计决策

### 决策 1: ARM64 使用原生 runner

选择 `ubuntu-24.04-arm`，不使用 cross/QEMU。这样 ARM64 构建同时验证真实 native 编译链，并降低交叉链接器复杂度。

### 决策 2: 每个平台重新构建 Web Console

不从 verify job 复用 `build/`。每个 runner 自行 `npm ci && npm run build`，保证 Cargo `build.rs` 在对应 OS 上读取真实生产静态资源，并避免跨 OS artifact 路径/权限差异。

### 决策 3: Release 使用 GitHub CLI

publish job 使用 `permissions: contents: write` 与 `GH_TOKEN=${{ github.token }}` 执行 `gh release create`。避免第三方 Release action，并让发布权限边界直接对应 GitHub 官方机制。

### 决策 4: 原始二进制作为 Release asset

Linux 发布 raw binary，用户下载后执行 `chmod +x`；Windows 发布 `.exe`。保持 curl/wget 路径稳定，不额外引入压缩包层。

### 决策 5: 普通 push 必须能发现 Action 问题

`CI` 改为 `main` push、PR、manual 都可触发，且顺序固定为前端 check/build -> Rust all-target tests -> release binary build。这样 CI 验证的是实际带内嵌 Web Console 的 Headless binary，而不是在不存在 `build/` 的独立 Rust job 中构建一个 0 embedded assets 的退化产物。

`Release` 也监听影响发布产物的 `main` push，但此时仅 verify/build/upload artifact，不授予 publish job 写权限。真正 Release 只在 `v*` tag push，或 manual dispatch 明确 `publish=true` 且 ref 为已有 `v*` tag 时执行。`gh release create --verify-tag` 防止手动输入错误时隐式创建新 tag。

## 测试策略

- 本地验证 YAML 可解析、引用路径存在、`git diff --check` 通过。
- 现有 Linux `npm check/build`、Rust tests/release build 作为工作流命令基线。
- fork 仓库先在 GitHub Actions 页面启用 workflows；之后普通 `main` push 应立即产生 CI run，并在相关发布代码变更时产生三平台 Release build 验证。
- 推送 workflow 后通过 GitHub Actions 实际 runner 验证 Windows x86_64 与 Linux arm64 native build。
- 首次发布检查 Release 中三个 binary 和 `SHA256SUMS` 文件名/哈希。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| ARM64 hosted runner 可用性变化 | 中 | 使用 GitHub 当前正式公布 runner label；失败时流水线明确报 runner 阶段 |
| Windows 源码仍有隐藏 cfg 问题 | 高 | Windows native build 作为必须 job，失败阻止 Release |
| Release token 权限不足 | 中 | workflow 显式设置 `contents: write` |
| Fork 默认不运行 workflow | 高 | 将“在 fork Actions 页面启用 workflows”列为仓库级前置条件；YAML 无法自行开启 |
| Tag 与 Cargo/package 版本不一致 | 低 | 本轮不自动修改版本；发布者负责在打 tag 前完成版本同步 |

## 检查清单

- [x] 所有 FR 有设计覆盖。
- [x] 不修改运行时 API。
- [x] 三平台均为 native runner。
- [x] Release 权限最小化。
