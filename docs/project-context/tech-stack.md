# 技术栈

| 类别 | 技术 | 用途 |
|------|------|------|
| 服务端 | Rust 2021 | Gateway、Admin、工具与进程监督 |
| 异步运行时 | Tokio | HTTP、信号、子进程和并发状态 |
| HTTP | Axum + tower-http | MCP、OAuth、Web Admin 与 RPC |
| 前端 | Svelte 5 + TypeScript | Browser Web Console |
| 构建 | SvelteKit adapter-static + Vite | 生成嵌入 Rust binary 的 SPA |
| 样式 | Tailwind CSS 4 | Web UI |
| HTTP Client | reqwest + rustls | 健康检查、OAuth、下载与公网探测 |
| 数据 | serde + JSON | 工作区、Gateway、Secret 与隧道配置 |
| 工具内核 | Rust modules | File、Patch、Exec、Git、History、Image |
| 隧道 | frpc + cloudflared | 可选公网 exposure |
| 测试 | cargo test + svelte-check | Rust 单元/集成与前端静态检查 |

当前 Rust crate 保留在 `src-tauri/` 路径以避免无价值的大规模移动，但不依赖 Tauri。

主要依赖以 `src-tauri/Cargo.toml` 和 `package.json` 为准。项目仅支持 Linux，使用 `/proc`、Unix signal 和 process-group 行为完成端口与子进程管理。
