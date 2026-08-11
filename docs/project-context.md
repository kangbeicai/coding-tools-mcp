# Coding Tools MCP 项目上下文

## 项目概览

| 属性 | 值 |
|------|-----|
| 产品 | Linux Coding Gateway + Web Console |
| 语言 | Rust + TypeScript |
| 前端 | Svelte 5 + SvelteKit static SPA |
| 服务端 | Tokio + Axum |
| 交付物 | 单一 `coding-tools` Linux CLI |
| 管理协议 | Browser `POST /api/rpc` |

`coding-tools` 在一个进程中运行全局多工作区 MCP Gateway、Web Console，以及可选 FRP/Cloudflare exposure。项目不包含桌面应用、Tauri runtime、Windows 或 macOS 支持。

## 文档导航

- [技术栈](./project-context/tech-stack.md)
- [架构设计](./project-context/architecture.md)
- [开发流程](./project-context/how-to-develop.md)
- [测试策略](./project-context/how-to-test.md)
- [代码图谱洞察](./graph-insights/latest.md)
- [Linux Headless-only 规格](./specs/linux-headless-only/)
- [Windows Headless + Activity 规格](./specs/headless-windows-activity/)

## 关键约束

- 写代码前遵循 `AGENTS.md` 和 MCP Probe Kit 工作流。
- 大改前做影响分析；规格型功能通过 `check_spec` 后再实施。
- 配置目录暂时保留 `~/.config/coding-tools-mcp-desktop`，不得无迁移方案改名。
- Web Admin 默认监听可信 LAN，尚无独立管理员认证，不得直接暴露公网。
- `old/` 是旧 Python/桌面参考实现，不属于当前产品运行时。
