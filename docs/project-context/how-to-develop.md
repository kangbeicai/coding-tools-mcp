# 如何开发

## 工作流

1. 先读取 `AGENTS.md`、本项目上下文和相关规格。
2. 新功能调用 `start_feature`，Bug 调用 `start_bugfix`，大重构先做 `code_insight`/影响分析。
3. 规格通过 `check_spec` 后再实施。
4. 修改符号前评估上游调用和执行流。
5. 使用小批次编辑，每批运行对应检查。
6. 完成后运行全量前端、Rust、release 和变更范围验证。

## 前端

```bash
npm ci
npm run dev
npm run check
npm run build
```

前端是 browser-only SPA。管理调用必须经 `src/lib/api/transport.ts` 的 `/api/rpc`，不要添加桌面 IPC 或第二套页面。

## Rust

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

Rust crate 虽位于 `src-tauri/`，但它是普通 Linux Rust crate。新代码不得添加 Tauri、Windows 或 macOS 条件分支。

## 本地运行

```bash
npm run build
cargo run --manifest-path src-tauri/Cargo.toml --bin coding-tools -- serve
```

默认 Gateway 为 `127.0.0.1:28766`，Web Console 为 `0.0.0.0:28767`。开发机已有服务占用端口时，使用 `--port` 和 `--admin-port` 选择隔离端口，不要中断现有实例。

## 发布纪律

- 前端变更后先构建 `build/`，再构建 Rust release，确保 embedded assets 最新。
- 同步 `package.json`、lockfile、`src-tauri/Cargo.toml` 和 `Cargo.lock` 的项目版本。
- 不覆盖正在运行的已验证 binary，先在隔离端口验证新 release。
- 未经用户明确要求，不提交、不推送、不替换 systemd 服务。
