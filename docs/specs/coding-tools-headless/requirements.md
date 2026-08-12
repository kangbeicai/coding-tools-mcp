# 需求文档：Headless 启动与配置目录优化

## 功能概述

优化 `coding-tools` Headless 的首次使用体验：前台启动摘要保留 Web Admin 默认监听 `0.0.0.0` 的行为，但不再把监听地址重复当作访问地址展示，默认同时给出可直接复制的本机 URL `http://127.0.0.1:<port>` 和实际 LAN IPv4 URL；同时把用户配置根目录从历史遗留的 `coding-tools-mcp-desktop` 迁移为 `coding-tools-mcp`，升级时自动兼容已有配置，避免丢失 workspace、Admin、Gateway、Tunnel、proxy 和 secrets 等持久化数据。

## 历史经验与坑

- **可复用经验**: 启动摘要只负责展示状态，不应改变 listener、认证、Gateway 或 exposure 生命周期。
- **必须规避的坑**: `0.0.0.0` 是监听绑定而不是浏览器可访问地址；`http://<server-ip>:PORT` 也不是可复制结果。
- **迁移原则**: 配置目录包含敏感配置和自动下载 binary，迁移优先使用同一父目录内的原子 rename；迁移失败不得删除旧目录或创建一个看似成功但内容不完整的新目录。

## 术语定义

- **Local URL**: 当前机器本地浏览器可访问的 Web Admin 地址，默认使用 `127.0.0.1`。
- **LAN URL**: 根据当前机器路由选择得到的非 loopback IPv4 生成的 Web Admin 地址。
- **Canonical config root**: 新版统一使用的配置根目录，Linux 通常为 `~/.config/coding-tools-mcp/`，Windows 通常为 `%APPDATA%\coding-tools-mcp\`。
- **Legacy config root**: 历史 Desktop 时代使用的 `coding-tools-mcp-desktop` 配置根目录，仅用于兼容迁移。

## 范围边界

**In Scope**
- 精简 foreground 启动摘要，删除重复的 Web Admin/listen 信息。
- 默认 `0.0.0.0` 监听时输出 Local URL 和实际 LAN URL。
- Linux / Windows Headless 使用同一套无额外依赖的 LAN IPv4 探测逻辑。
- LAN IPv4 无法确定时优雅省略 LAN 行。
- 增加纯函数测试覆盖 URL 生成、bind 边界和 LAN 地址过滤。
- 同步 `--help` 中不再展示 `<server-ip>` 占位符。
- Linux / Windows 配置根目录改为 `coding-tools-mcp`。
- 新目录不存在且旧 `coding-tools-mcp-desktop` 存在时，首次访问配置目录自动尝试整体迁移。
- 若新旧目录同时存在，则优先使用新目录且不自动合并/覆盖旧目录。
- 若自动 rename 迁移失败，则本次继续兼容使用旧目录并给出非敏感 warning，后续启动可再次尝试迁移。

**Out of Scope**
- 修改 Web Admin 默认 bind host / port。
- 修改管理员认证、Gateway、Public Access、Cloudflare / FRP。
- 枚举并展示所有网卡地址、IPv6 地址或 Docker/VPN 网卡列表。
- 修改 `profiles.json` 数据结构或 secret 序列化格式。
- 自动合并两个同时存在且内容不同的新旧配置目录。
- 修改当前正在运行的服务进程。

## 需求列表

### FR-1: 精简启动摘要

**优先级:** Must
**用户故事:** 作为 Headless 用户，我希望启动信息紧凑且没有重复字段，以便快速找到真正需要复制的地址。

#### 验收标准（EARS）
1. WHEN `coding-tools` 以前台模式启动 THEN 摘要 SHALL 只展示一次 Web Admin 访问区块。
2. WHEN Web Console 使用 embedded assets THEN 摘要 SHALL 继续展示 `embedded (N assets)` 状态。
3. WHEN foreground 摘要结束 THEN 系统 SHALL 继续展示管理员认证提示和 `Press Ctrl+C to stop.`。

### FR-2: 输出可复制的 Local / LAN Web Admin URL

**优先级:** Must
**用户故事:** 作为用户，我希望看到本机和局域网两个可以直接复制的 Web Admin 地址，以便分别从服务器本机和其他设备访问。

#### 验收标准（EARS）
1. WHEN Admin bind 为默认 `0.0.0.0` THEN 摘要 SHALL 输出 `Local : http://127.0.0.1:<port>`。
2. WHEN 可以确定一个非 loopback、非 unspecified 的本机 IPv4 THEN 摘要 SHALL 输出 `LAN : http://<actual-ip>:<port>`。
3. IF 无法可靠确定 LAN IPv4 THEN 摘要 SHALL 省略 LAN 行且 SHALL NOT 输出 `<server-ip>` 占位符。

### FR-3: bind 覆盖场景不得展示不可访问地址

**优先级:** Must
**用户故事:** 作为使用 `--admin-bind` 的用户，我希望摘要只显示与实际 listener 一致的访问地址，以免复制一个实际无法连接的 URL。

#### 验收标准（EARS）
1. WHEN Admin 仅绑定 loopback THEN 摘要 SHALL 展示 Local URL 且 SHALL NOT 展示 LAN URL。
2. WHEN Admin 绑定具体非 loopback IPv4 THEN 摘要 SHALL 展示该地址作为 LAN URL 且 SHALL NOT 声称 `127.0.0.1` 可访问。
3. WHEN Admin 绑定所有 IPv4 接口 THEN URL 生成 SHALL 不改变原有 `0.0.0.0:<port>` listener 行为。

### FR-4: 配置根目录去除 Desktop 历史命名

**优先级:** Must
**用户故事:** 作为 Headless/Web 用户，我希望配置目录名称与当前产品形态一致，以便部署、备份和排障时不再出现误导性的 Desktop 路径。

#### 验收标准（EARS）
1. WHEN Linux 解析配置根目录 THEN canonical root SHALL 为 `$XDG_CONFIG_HOME/coding-tools-mcp`，未设置时遵循 `dirs::config_dir()` 的现有平台行为。
2. WHEN Windows 解析配置根目录 THEN canonical root SHALL 为 `%APPDATA%\coding-tools-mcp` 对应的 `dirs::config_dir()/coding-tools-mcp`。
3. WHEN 新目录已经存在 THEN 系统 SHALL 直接使用新目录且 SHALL NOT 自动覆盖、删除或合并旧目录。

### FR-5: 旧配置目录自动兼容迁移

**优先级:** Must
**用户故事:** 作为从旧版本升级的用户，我希望继续使用原有 workspace、Admin、Tunnel 和 secrets 配置，而不需要手动搬目录。

#### 验收标准（EARS）
1. WHEN 新目录不存在且旧 `coding-tools-mcp-desktop` 目录存在 THEN 系统 SHALL 在同一父目录内优先通过 rename 将旧目录迁移为 `coding-tools-mcp`。
2. WHEN rename 成功 THEN 后续所有 `data/`、`bin/`、`logs/` 等路径 SHALL 从新目录读取和写入。
3. IF rename 失败 THEN 系统 SHALL 保留旧目录完整内容并继续使用旧目录完成本次运行，且 SHALL NOT 创建半迁移的新目录。
4. WHEN 新旧目录同时存在 THEN 系统 SHALL 选择新目录并保持旧目录不变，以避免隐式数据覆盖。

## 非功能需求

- **NFR-1（启动开销）**: LAN 地址探测不得进行 HTTP 请求、DNS 依赖或阻塞式外部命令调用。
- **NFR-2（安全）**: 终端摘要不得输出 token、password hash 或其他 secret。
- **NFR-3（兼容性）**: Linux / Windows 均使用 `std::net` 可用能力，不新增系统包依赖。
- **NFR-4（数据安全）**: 配置目录迁移不得读取/重写 JSON 内容，不改变文件权限语义；失败时优先保留旧目录可用性而不是强制迁移。

## 依赖关系

- `src-tauri/src/headless.rs` foreground 输出路径。
- `AdminConfig.bind_host` / `local_port` 与现有 Admin listener 绑定语义。
- Rust 标准库 `std::net::{IpAddr, Ipv4Addr, UdpSocket}`。
- `Platform::app_config_dir()` 及 Linux / Windows 平台实现。
- `data/profiles.json`、`bin/`、`logs/` 等现有路径均通过 `app_config_dir()` 派生。

## 检查清单

- [x] 需求覆盖默认 bind、自定义 bind、探测失败和新旧配置目录迁移边界。
- [x] 每条需求有稳定 FR ID。
- [x] 验收标准可测试。
- [x] 明确不改变监听和认证逻辑。
