# 设计文档：Headless 启动与配置目录优化

## 概述

把 foreground 启动输出重构为“服务状态 + Web Admin 可访问地址”两部分，同时将 Linux / Windows 的配置根目录统一为 `coding-tools-mcp`。监听绑定继续使用现有 Admin listener；LAN IPv4 探测只用于展示。配置迁移只处理目录级路径兼容，不解析或重写其中的数据文件。

**对应需求:** FR-1, FR-2, FR-3, FR-4, FR-5

## 技术方案

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| LAN IPv4 探测 | `UdpSocket::bind` + UDP `connect` 路由选择 + `local_addr` | 不发送应用数据、不依赖外部命令、Linux/Windows 通用 | FR-2 |
| URL 生成 | 纯函数接收 bind host、port、可选 LAN IPv4 | 可完整单测，不把网络环境耦合进格式逻辑 | FR-2, FR-3 |
| 输出位置 | `src-tauri/src/headless.rs` | 当前所有 foreground 摘要已集中在该文件 | FR-1 |
| 配置根目录 | shared platform helper 解析 canonical / legacy root | Linux/Windows 保持相同迁移语义，避免两份实现漂移 | FR-4, FR-5 |
| 迁移方式 | 新目录缺失时 `fs::rename(legacy, canonical)` | 同一配置父目录内 rename 不重写 secret/data，成功时保持原有权限和子目录 | FR-5 |

## 架构设计

```text
AdminConfig(bind_host, port)
        |
        +---- spawn_admin_listener() ----> listener 行为保持不变
        |
        +---- detect_lan_ipv4() ---------> Option<Ipv4Addr>
        |                                     |
        +---- admin_access_urls() <-----------+
                       |
                       v
                 foreground summary
                 Local: 127.0.0.1
                 LAN:   actual IPv4
```

`detect_lan_ipv4()` 通过本地 UDP socket 连接一个文档保留 IPv4 目标，只利用 OS 路由表决定 source address；不发送 payload。得到的地址必须排除 loopback、unspecified 和 link-local。失败直接返回 `None`。

配置目录解析流程：

```text
dirs::config_dir()/home fallback
          |
          +--> coding-tools-mcp exists ----------> use canonical
          |
          +--> legacy missing --------------------> return canonical
          |
          +--> legacy exists, canonical missing
                    |
                    +--> rename succeeds ---------> use canonical
                    |
                    +--> rename fails ------------> warn + use legacy for this run
```

不做 recursive copy fallback：配置目录包含 secrets，半复制或权限变化比保留 legacy 路径更危险。由于两个目录位于同一配置父目录，正常迁移路径应是 rename；失败时保持旧数据原样并允许后续重试。

## 数据模型

不新增持久化数据模型。内部可使用一个小型 `AdminAccessUrls` 结构保存可选 `local` / `lan` URL。配置目录迁移不修改 `AppData` / `profiles.json` schema。

## API 设计

不新增外部 API。内部函数契约：

```rust
fn detect_lan_ipv4() -> Option<Ipv4Addr>;
fn admin_access_urls(bind_host: &str, port: u16, lan_ip: Option<Ipv4Addr>) -> AdminAccessUrls;
fn app_config_dir_with_legacy_migration() -> AppResult<PathBuf>;
```

## 文件结构

```text
src-tauri/src/headless.rs                         # 修改：探测、URL 生成、摘要输出、单测
src-tauri/src/platform/paths.rs                   # 修改：共享配置根目录解析/迁移 helper
src-tauri/src/platform/linux/mod.rs               # 修改：调用共享 config root helper
src-tauri/src/platform/windows/mod.rs             # 修改：调用共享 config root helper
docs/specs/coding-tools-headless/requirements.md # 新增
docs/specs/coding-tools-headless/design.md       # 新增
docs/specs/coding-tools-headless/tasks.md        # 新增
```

## 设计决策

### 决策 1: 不展示 `0.0.0.0` 作为访问 URL（FR-1, FR-2）

`0.0.0.0` 继续是正确的 listener bind，但不是用户应该复制到浏览器的地址。摘要隐藏该实现细节，默认输出 `127.0.0.1` 和实际 LAN IPv4。

### 决策 2: 不枚举所有网卡（FR-2）

网卡枚举在 Windows/Linux 上需要不同 API，也容易把 Docker、VPN、虚拟网卡暴露给用户。使用 OS 默认路由选择得到单个最可能有用的 LAN source IPv4；无法确定时省略 LAN 行。

### 决策 3: 自定义 bind 优先保证准确性（FR-3）

- `0.0.0.0`：Local=`127.0.0.1`，LAN=探测地址。
- loopback bind：只输出 Local。
- 具体非 loopback IPv4：只输出 LAN=该 bind 地址。
- IPv6 bind：保留 listener endpoint 作为单一 Web Admin 地址，不伪造 IPv4 Local/LAN。

### 决策 4: canonical 配置目录统一为 `coding-tools-mcp`（FR-4）

Linux 和 Windows 都通过现有 `dirs::config_dir()` 解析平台配置父目录，只把最后一级应用目录名从 `coding-tools-mcp-desktop` 改成 `coding-tools-mcp`。这样 `data/profiles.json`、`bin/cloudflared`、`bin/frpc`、downloads 和 logs 都自然随根目录切换，无需分别修改每个调用点。

### 决策 5: 只做目录 rename，不做隐式 merge/copy（FR-5）

- canonical 已存在：canonical 永远优先，legacy 保持不动。
- canonical 不存在且 legacy 存在：尝试 rename 整个 legacy root。
- rename 失败：打印不含 secret/内容的 warning，并返回 legacy root，保证旧版本数据仍可运行。

该策略避免把两个配置树自动合并，也避免 copy 过程中产生“canonical 已存在但文件不完整”的不可恢复状态。

## 测试策略

- 单测纯 URL 生成函数：默认 all-interface、loopback、具体 LAN IPv4、无 LAN 探测结果、IPv6。
- 单测 LAN IPv4 可用性过滤：排除 `0.0.0.0`、`127.0.0.1`、`169.254.0.0/16`。
- 单测 config root 选择/迁移：canonical 优先、legacy rename 成功、rename 失败兼容路径、无旧目录时返回 canonical。
- `cargo test --all-targets` 全量回归。
- `cargo build --release --bin coding-tools` 验证 Headless binary。
- `coding-tools --help` 验证不再输出 `<server-ip>`。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 默认路由指向 VPN | 中 | 只展示 OS 实际选择的 source IPv4，不声称枚举全部 LAN 地址 |
| 无默认 IPv4 路由 | 低 | 省略 LAN 行，Local 仍可复制 |
| 自定义 bind 输出错误地址 | 高 | URL 生成根据 bind 分类，使用纯函数测试覆盖 |
| 旧配置 rename 失败 | 中 | 不复制/删除旧目录，本次继续使用 legacy root 并允许后续重试 |
| 新旧目录同时存在 | 高 | canonical 优先且禁止自动 merge，避免覆盖用户新配置 |

## 检查清单

- [x] 所有 FR 有设计覆盖。
- [x] 不修改运行时 HTTP/MCP API。
- [x] 不新增平台依赖。
- [x] 异常网络环境可安全降级。
