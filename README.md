<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="Coding Tools MCP 图标">
</p>

<h1 align="center">Coding Tools MCP</h1>

<p align="center">
  把本地项目变成 AI 可直接开发、能够跨会话延续上下文的持久工作区。
</p>

<p align="center">
  <a href="https://github.com/mybolide/coding-tools-mcp/releases/latest"><img src="https://img.shields.io/github/v/release/mybolide/coding-tools-mcp?label=Release" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/Windows-x64-0078D4?logo=windows" alt="Windows x64">
  <img src="https://img.shields.io/badge/macOS-Apple%20Silicon-000000?logo=apple" alt="macOS Apple Silicon">
  <img src="https://img.shields.io/badge/Linux-headless%20%2B%20Web-FCC624?logo=linux&logoColor=black" alt="Linux headless + Web">
  <a href="https://www.apache.org/licenses/LICENSE-2.0"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="Apache-2.0"></a>
</p>

<p align="center">
  <a href="README.md">中文</a> · <a href="README.en.md">English</a> · <a href="https://github.com/mybolide/coding-tools-mcp/releases/latest">下载最新版</a>
</p>

Coding Tools MCP 是一个可自托管的多工作区 Coding Gateway。Rust 服务端提供单一 MCP 入口和本地 Web 管理台；现有 Svelte 界面既可以直接由浏览器访问，也可以继续由 Tauri 2 作为桌面壳承载。注册项目目录后，AI Agent 可以通过一个 MCP Gateway 读取文件、修改代码、运行命令和测试、查看 Git 状态，并把关键进度保存为各项目自己的历史会话。

![Coding Tools MCP 工作区总览](docs/images/workspace-overview.png)

*一个桌面端同时管理工作区、MCP 服务、连接信息与会话恢复提示词。*

## 30 秒看懂怎么用

```text
安装桌面端或 Linux headless 版本
  → 添加一个或多个项目目录
  → coding-tools serve / Web Console 启动全局 MCP Gateway
  → 用端口映射 / HTTPS 反向代理 / 隧道暴露一个公网地址
  → ChatGPT 开启开发人员模式
  → 只新建一个 Coding Tools MCP 插件并粘贴根 /mcp 地址
  → 新对话：list_workspaces → select_workspace → history_session_bootstrap
```

第一次使用只需要记住两件事：**ChatGPT 只需要连接一次全局 `/mcp`；项目切换在 Gateway 内部完成，不需要为每个工作区创建一个插件。**

- [查看完整安装和桌面端启动步骤](#五分钟开始使用)
- [直接查看 ChatGPT 插件配置](#mcp-connector)

## 五分钟开始使用

### 1. 安装客户端

#### 桌面模式

打开 [Releases](https://github.com/mybolide/coding-tools-mcp/releases/latest) 并下载对应安装包：

| 系统 | 安装包 |
| --- | --- |
| Windows 10/11 x64 | `Coding.Tools.MCP_*_x64-setup.exe` |
| macOS Apple Silicon | `Coding Tools MCP_*_aarch64.dmg` |

macOS 安装包目前未签名。如果系统阻止首次打开，请在“系统设置 → 隐私与安全性”中确认打开。

#### Linux 无桌面 / Web 模式

Linux 服务器可以只构建 `coding-tools`，不启用 Tauri/WebKit/GTK runtime。Web Console 使用同一套 Svelte 静态构建：

```bash
npm ci
npm run build

cd src-tauri
cargo build --release --no-default-features --features headless --bin coding-tools
```

常用入口：

```bash
# 推荐：同时运行 MCP Gateway 和本地 Web Admin
./target/release/coding-tools serve

# 可选：轻量终端监视器；Web Console 仍然同时运行
./target/release/coding-tools tui

# 查看已注册工作区和 Gateway 配置
./target/release/coding-tools workspace list
./target/release/coding-tools config show
```

也可以在启动时覆盖并保存网络设置，例如：

```bash
coding-tools serve --bind 0.0.0.0 --port 28766 \
  --public-url https://mcp.example.com --auth oauth \
  --admin-port 28767 --web-root ../build
```

启动后默认有两个彼此独立的监听面：

```text
MCP data plane   http://127.0.0.1:28766/mcp
Web admin plane  http://127.0.0.1:28767/
```

MCP 可以按需要改为 `0.0.0.0`、端口映射或放到 HTTPS 反向代理后；**Web Admin 当前强制只监听 loopback**。远程管理 Linux 服务器推荐使用 SSH 转发：

```bash
ssh -L 28767:127.0.0.1:28767 user@server
```

然后在本机浏览器打开 `http://127.0.0.1:28767`。在独立管理员认证完成前，不建议也不允许直接把 Admin API 暴露到局域网/公网。

### 2. 打开管理界面并添加工作区

桌面模式直接使用应用窗口；headless 模式在浏览器打开 Web Console。两者使用同一套 Svelte 管理界面和同一份持久化配置。

1. 点击左侧的“添加工作区”。
2. Tauri 桌面端使用原生目录选择器；Web 模式输入**服务器上的**项目绝对路径，例如 `/home/user/project`。
3. 设置工作区名称和该项目自己的执行/工具策略。
4. 保存后，工作区会长期保留在左侧列表中。

### 3. 启动全局 Gateway

在“设置 → Gateway”中配置并启动全局 MCP Gateway：

- 默认监听 `127.0.0.1:28766`，只允许本机访问；
- 需要局域网访问或路由器端口映射时，把监听地址改为 `0.0.0.0` 或指定网卡 IP；
- Gateway 使用共享 OAuth/Bearer 凭据，一个 ChatGPT 插件即可访问所有已注册工作区；
- 当注册了两个及以上工作区时，项目工具在执行前必须先通过 `select_workspace` 绑定当前会话；
- `/w/<workspace-id>/mcp` 是显式路径路由，主要用于调试或其他 MCP 客户端，并不要求在 ChatGPT 中分别创建插件。

> 旧的“每个工作区独立启动 MCP + 独立公网地址”仍保留为兼容模式。推荐新部署优先使用全局 Gateway；同一端口不能同时被全局 Gateway 和旧工作区 listener 占用。

### 4. 暴露一个公网入口

如果 AI 客户端不在本机，需要把本地 MCP 暴露为 HTTPS 地址：

- 路由器/NAT 端口映射：Gateway 必须监听 `0.0.0.0` 或对应局域网 IP；
- Nginx/Caddy/Traefik：把一个 HTTPS 域名反向代理到 Gateway 本地端口；
- FRP / Cloudflare：旧工作区模式仍可由桌面端自动管理；全局 Gateway 的内置单隧道管理正在从旧的 per-workspace tunnel supervisor 迁移，当前也可以让外部 `frpc` / `cloudflared` 直接代理 Gateway 端口。

![FRP 配置页面](docs/images/frp-configuration.png)

*FRP 服务器配置集中保存，各工作区只需选择配置并填写自己的子域名。*

如果还没有可用的 FRPS 服务端，可以参考：[FRPS 服务端安装教程（微信公众号）](https://mp.weixin.qq.com/s/kmpQhHsvmHlaLfj4rw3A0Q)。安装完成后，把服务端地址、端口和 Token 填入客户端的“FRP 配置”即可。

### 5. 连接 AI 客户端

全局 Gateway 页面会显示：

- 本地 MCP 地址，例如 `http://127.0.0.1:28766/mcp`；
- 配置后的公网 HTTPS MCP 地址；
- 已注册工作区数量；
- 当前 ChatGPT/MCP 会话到工作区的绑定关系。

![MCP 本地、公网与 ChatGPT 连接信息](docs/images/workspace-connection.png)

启动后可以直接检查本地与公网端点、OAuth 元数据和 MCP 受保护资源：

![MCP 健康检查结果](docs/images/health-check.png)

*健康检查会逐项显示连接和认证元数据是否可用。*

遇到连接问题时，无需离开桌面端即可查看最近的 MCP 请求日志：

![MCP 运行日志](docs/images/runtime-logs.png)

*日志可快速确认工具列表、历史初始化和检查点调用是否真正到达服务端。*

支持 MCP 的客户端使用同一个 Gateway 公网 URL。使用 OAuth 时，Gateway 使用共享 OAuth 凭据，不再要求每个工作区单独授权。

新对话的推荐初始化顺序：

```text
list_workspaces
select_workspace
history_session_bootstrap
server_info
get_default_cwd
git_status
check_exec_environment
```

这样 Agent 不需要依赖聊天上下文猜测当前项目、工作目录和执行能力。

## ChatGPT 的两种接入方式

| 方式 | 适合场景 | 在客户端中使用什么 |
| --- | --- | --- |
| MCP Connector | ChatGPT 直接使用文件、命令和 Git 工具 | 全局 Gateway 的唯一公网 `/mcp` 地址 |
| GPT Actions | 在自定义 GPT 中导入 OpenAPI 工具 | Actions 面板中的 `/openapi.json` 地址 |

### MCP Connector

配置前请先确认：

1. 全局 Gateway 处于运行状态。
2. 公网 HTTPS 地址已经正确转发到 Gateway 的本地端口。
3. 从“设置 → Gateway”复制唯一公网 MCP 地址；如果使用 OAuth，使用“共享密钥”中的 Gateway OAuth 凭据。

> ChatGPT 必须使用公网 HTTPS `/mcp` 地址，不能使用 `http://127.0.0.1:28766/mcp` 之类的本地地址。ChatGPT 的菜单名称可能随版本和语言设置略有变化。

#### 1. 开启 ChatGPT 开发人员模式

打开 ChatGPT 设置，进入“账户安全与登录”，开启“开发人员模式”。该开关允许添加未经验证的 MCP 连接器。

![在 ChatGPT 中开启开发人员模式](docs/images/gpt-config-1.png)

*开发人员模式具有较高权限，只应连接你自己部署或明确可信的 MCP 服务。*

#### 2. 创建 MCP 插件

在 ChatGPT 左侧进入“插件”，点击右上角的 `+` 新建插件，然后选择 MCP（测试版）并填写：

| ChatGPT 字段 | 填写内容 |
| --- | --- |
| 名称 | 自定义一个容易识别的名称，例如 `Coding Tools MCP` |
| 描述 | 例如“访问我的本地 Coding Tools 多工作区 Gateway” |
| 连接 | 粘贴全局 Gateway 的公网 MCP 地址，URL 应以 `/mcp` 结尾 |
| 身份验证 | 与桌面端保持一致；截图以 OAuth 为例 |

![在 ChatGPT 中新建 MCP 插件并填写连接信息](docs/images/gpt-config-2-detail.png)

使用 OAuth 时，展开“高级 OAuth 设置”，选择静态/手动 OAuth 凭据并填写桌面端提供的 Client ID 和 Client Secret，不需要选择 CIMD。保存或连接后，ChatGPT 会打开授权页面；输入桌面端“GPT 配置”卡片中的授权口令完成首次授权。

> Client Secret、授权口令和 Bearer Token 都属于敏感信息，不要粘贴到对话、Issue 或公开截图中。若桌面端使用 Bearer 或不启用认证，请在 ChatGPT 中选择当前界面提供的对应认证方式。

#### 3. 验证连接

创建一个启用了该插件的新对话，并发送：

```text
请使用 Coding Tools MCP：
1. 调用 list_workspaces；
2. 选择我要开发的工作区并调用 select_workspace；
3. 调用 history_session_bootstrap；
4. 再调用 server_info、get_default_cwd 和 git_status。
```

如果能够列出多个项目、只选择其中一个并返回该项目的信息，说明“一个 ChatGPT 插件 → 一个 Gateway → 会话路由 → Workspace ToolContext”的链路已经打通。

如果 ChatGPT 仍显示旧的工具列表，请断开并重新连接插件，或创建一个新对话后再次验证。

#### 常见问题

| 现象 | 优先检查 |
| --- | --- |
| ChatGPT 无法连接 | 是否使用公网 HTTPS `/mcp` 地址，而不是 `127.0.0.1`；桌面端公网 MCP 健康检查是否通过 |
| OAuth 授权失败 | Gateway 是否使用共享 OAuth 凭据；公网基地址是否与实际 HTTPS 地址一致 |
| 看不到新增工具 | 断开并重新连接插件，然后创建一个新对话 |
| 返回“尚未选择工作区” | 先调用 `list_workspaces` 和 `select_workspace`；多工作区模式不会猜测项目 |
| 工具调用失败 | 检查 Gateway 日志和当前会话绑定的 workspace，确认请求没有路由到错误项目 |

### GPT Actions

1. 启动工作区的 Actions 服务。
2. 复制 Actions 面板中的 OpenAPI URL。
3. 在 GPT 编辑器的 Actions 页面导入该 URL。
4. 根据桌面端配置选择 None、API Key 或 OAuth。

MCP 和 Actions 可以为同一个工作区同时运行，也可以分别使用不同端口和子域名。

## 为什么需要它

- **面向真实开发**：文件、命令、Git、测试和长时间运行的进程都在同一个 Workspace 中。
- **跨会话持续开发**：新对话可以读取全部历史摘要和最近一次完整交接，不必反复向 AI 解释项目背景和当前进度。
- **进度可追溯**：每轮任务完成后可保存结构化检查点，决策、修改、测试结果和下一步都留在项目目录中。
- **单插件多工作区**：一个 Gateway、一个公网 `/mcp`、一个 ChatGPT 插件，通过会话绑定安全路由到不同项目。
- **Web-first 管理**：同一套 Svelte Console 可由 Rust Admin Server 直接托管，也可放进 Tauri 桌面壳；Linux headless 不需要 Tauri/WebKit/GTK runtime。
- **连接 ChatGPT 更直接**：内置 Streamable HTTP、OAuth、Bearer Token、OpenAPI、FRP 和 Cloudflare 隧道。
- **默认工具面保持简单**：稳定的核心工具默认可用，高级 Harness 能力按需开启。

## 让项目记住每次对话

普通聊天记录适合回看交流内容，但不适合作为长期开发交接。Gateway 会先把当前 ChatGPT conversation 绑定到一个 Workspace，然后历史工具只在该项目自己的 `docs/history-session/` 中读写，因此多个项目不会共享一份历史目录。

![ChatGPT 新会话启动提示词](docs/images/history-session-prompt.png)

*复制完整提示词到新会话，即可初始化或恢复历史；每轮任务完成后再保存检查点。*

它提供三个互相配合的历史工具：

| 工具 | 作用 |
| --- | --- |
| `history_session_bootstrap` | 新对话开始时初始化或恢复项目会话；新文件会固化前序会话的压缩摘要，并返回稳定的 `session_key` 和 `current_path` |
| `history_session_checkpoint` | 每轮任务完成后按 bootstrap 返回的稳定目标保存结构化进度；目标不一致时拒绝写入，避免串到其他历史文件 |
| `history_session_validate` | 检查历史编号、文件和会话映射；必要时重建派生索引，不删除已有历史 |

典型效果：

```text
对话 1：选择 Workspace → 初始化历史 → 分析项目 → 修改代码 → 测试 → 保存检查点
                                      ↓
对话 2：选择 Workspace → 读取历史摘要和最新交接 → 继续 → 保存新检查点
```

历史文件使用可读的 Markdown 格式，可以随项目备份或纳入 Git，也方便开发者直接审阅和修订。每个新文件顶部都带有有长度上限的“继承的历史摘要”，旧摘要不会递归复制；检查点采用幂等写入，并要求返回 `ok=true` 且会话目标一致后才确认保存成功。

> 历史持久化由 AI 调用 MCP 工具完成，并非桌面端在后台录制聊天内容。若客户端未触发工具调用，服务端无法凭空感知新的对话或任务进度。

## Agent 可以做什么

默认 `core` profile 提供一组稳定、可组合的开发工具：

| 类别 | 主要工具 |
| --- | --- |
| 文件读取 | `read_file`、`list_dir`、`list_files`、`search_text`、`grep_text`、`view_image` |
| 文件修改 | `apply_patch` |
| 命令执行 | `exec_command`、`write_stdin`、`read_output`、`kill_session` |
| Git | `git_status`、`git_diff`、`git_log`、`git_show`、`git_blame` |
| 环境 | `server_info`、`check_exec_environment`、`get_default_cwd`、`set_default_cwd` |
| 历史会话 | `history_session_bootstrap`、`history_session_checkpoint`、`history_session_validate` |

典型开发过程：

```text
打开 Workspace
  → 理解项目和 Git 状态
  → 搜索并读取代码
  → 事务化应用 Patch
  → 运行命令和测试
  → 检查 diff 并提交
```

高级 profile 还保留项目状态、操作记录等 Harness 能力，但普通文件修改和命令执行不要求先创建 Task。

## 权限与恢复模型

项目采用 Workspace-first 权限模型：

- Workspace 内普通文件可以读取、创建、修改、删除和执行。
- Workspace 外允许完整只读：`read_file`、`list_dir`、`list_files`、`search_text`、`view_image`。
- Workspace 外写入、删除和执行会被阻止。
- `.git` 和 `.github` 不能被普通文件工具、Patch 或解释器命令破坏。
- Patch 在单次操作内进行预检和失败恢复；长期恢复统一使用 Git，不创建全量 Workspace Snapshot。

> Windows 子进程目前仍是 `policy_only` 执行边界，返回中的 `sandbox_enforced: false` 是真实状态。静态命令策略不能等同于完整的操作系统文件系统沙箱。

## 本地开发

环境要求：Node.js 20+、Rust stable，以及当前系统的 [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)。

```bash
npm install
npm run desktop
```

常用验证命令：

```bash
npm run check
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

Windows 也可以双击 `dev-desktop.cmd`。不要只用 `npm run dev` 验证桌面应用，它只启动 Vite，不会启动 Tauri 外壳。

## 项目结构

| 路径 | 作用 |
| --- | --- |
| `src-tauri/src/tools/` | 文件、Patch、Exec、Git 等共享工具内核 |
| `src-tauri/src/mcp/` | MCP Streamable HTTP 服务 |
| `src-tauri/src/actions/` | ChatGPT Actions OpenAPI 网关 |
| `src-tauri/src/tunnel/` | FRP / Cloudflare 隧道和进程管理 |
| `src/` | SvelteKit 桌面界面 |
| `old/` | Python 参考实现和兼容性基线 |

## 致谢
感谢 [Linux.do](https://linux.do/) 社区对项目推广与反馈的支持。

## License

[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
