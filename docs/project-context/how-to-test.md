# 如何测试

## 必跑检查

```bash
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --release --manifest-path src-tauri/Cargo.toml --bin coding-tools
```

## 测试层级

| 层级 | 位置 | 覆盖 |
|------|------|------|
| Rust 单元测试 | `src-tauri/src/**` | URL、OAuth、配置、路由、隧道和工具函数 |
| Rust 集成测试 | `src-tauri/tests/` | 工具契约、安全边界、Harness、History |
| 前端静态检查 | `npm run check` | Svelte 与 TypeScript |
| 前端生产构建 | `npm run build` | SPA route 与 embedded assets 输入 |
| 运行态 smoke | 隔离端口启动 release | Web、RPC、Gateway、health、shutdown |
| 公网回归 | FRP/Cloudflare 环境 | effective URL、canonical URL、OAuth metadata |

## 运行态原则

- 新 release 先使用隔离 Gateway/Admin 端口启动。
- 验证 `/`、`/api/rpc`、`/mcp` 和 OAuth metadata。
- Named Tunnel 回归需要确认 Gateway restart 不无故重建 exposure PID/URL。
- 验证 SIGINT 后 listener 和受管子进程正常退出。
- 不用未验证 binary 替换当前线上进程。

## 安全回归

`call_tool_security` 与相关 policy 测试不可跳过，重点覆盖 Workspace 路径边界、`.git/.github` 保护、shell chaining、危险命令确认和外部写入阻止。
