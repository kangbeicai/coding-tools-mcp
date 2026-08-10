<script lang="ts">
  import { onMount } from "svelte";
  import CopyButton from "$lib/components/CopyButton.svelte";
  import SecretInput from "$lib/components/SecretInput.svelte";
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import {
    getGatewayConfig,
    getGatewayExposure,
    getGatewayExposureStatus,
    getGatewayStatus,
    runGatewayHealthChecks,
    clearGatewaySession,
    setGatewayConfig,
    setGatewayExposure,
    restartGateway,
    startGateway,
    startGatewayExposure,
    stopGateway,
    stopGatewayExposure,
    type GatewayConfig,
    type GatewayExposureConfig,
    type GatewayExposureStatus,
    type GatewayHealthReport,
    type GatewayStatus,
  } from "$lib/api/gateway";
  import { getSharedSecret, setSharedSecret } from "$lib/api/secrets";
  import { listFrpProfiles, type FrpProfileDto } from "$lib/api/settings";
  import { showToast } from "$lib/stores/toast";

  let config = $state<GatewayConfig | null>(null);
  let exposure = $state<GatewayExposureConfig | null>(null);
  let status = $state<GatewayStatus | null>(null);
  let exposureStatus = $state<GatewayExposureStatus | null>(null);
  let frpProfiles = $state<FrpProfileDto[]>([]);
  let gatewayFrpToken = $state("");
  let gatewayCloudflareToken = $state("");
  let busy = $state(false);
  let exposureBusy = $state(false);
  let saving = $state(false);
  let healthBusy = $state(false);
  let health = $state<GatewayHealthReport | null>(null);

  const running = $derived(status?.state === "running");
  const exposureRunning = $derived(exposureStatus?.state === "running");
  const managedExposure = $derived(exposure?.mode === "frp" || exposure?.mode === "cloudflare");
  const displayedPublicEndpoint = $derived(
    exposure?.mode === "cloudflare" && exposure.cloudflareMode === "quick" && exposureRunning
      ? mcpEndpoint(exposureStatus?.effectivePublicUrl ?? "")
      : status?.publicEndpoint || mcpEndpoint(exposureStatus?.effectivePublicUrl ?? ""),
  );

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      const [
        loadedConfig,
        loadedExposure,
        loadedStatus,
        loadedExposureStatus,
        loadedFrpProfiles,
        frpToken,
        cloudflareToken,
      ] = await Promise.all([
        getGatewayConfig(),
        getGatewayExposure(),
        getGatewayStatus(),
        getGatewayExposureStatus(),
        listFrpProfiles().catch(() => []),
        getSharedSecret("gateway_frp_token").catch(() => null),
        getSharedSecret("gateway_cloudflare_token").catch(() => null),
      ]);
      // Passive public networking no longer asks users to distinguish
      // local/direct/external. A configured publicUrl is sufficient.
      if (loadedExposure.mode !== "frp" && loadedExposure.mode !== "cloudflare") {
        loadedExposure.mode = "local";
      }
      config = loadedConfig;
      exposure = loadedExposure;
      status = loadedStatus;
      applyExposureStatus(loadedExposureStatus);
      frpProfiles = loadedFrpProfiles;
      gatewayFrpToken = frpToken ?? "";
      gatewayCloudflareToken = cloudflareToken ?? "";
    } catch (error) {
      showToast(String(error), { title: "读取 Gateway 失败", kind: "error" });
    }
  }

  async function checkHealth() {
    if (healthBusy) return;
    healthBusy = true;
    try {
      health = await runGatewayHealthChecks();
      status = await getGatewayStatus();
      exposureStatus = await getGatewayExposureStatus();
    } catch (error) {
      showToast(String(error), { title: "Gateway 健康检查失败", kind: "error" });
    } finally {
      healthBusy = false;
    }
  }

  function healthBadge(status: string): string {
    if (status === "ok") return "PASS";
    if (status === "warn") return "WARN";
    if (status === "skip") return "SKIP";
    return "FAIL";
  }

  async function toggleExposure() {
    if (exposureBusy || !managedExposure) return;
    exposureBusy = true;
    try {
      if (!exposureRunning && exposure) {
        await setGatewayExposure(exposure);
        if (gatewayFrpToken.trim()) await setSharedSecret("gateway_frp_token", gatewayFrpToken.trim());
        if (gatewayCloudflareToken.trim()) {
          await setSharedSecret("gateway_cloudflare_token", gatewayCloudflareToken.trim());
        }
      }
      applyExposureStatus(exposureRunning ? await stopGatewayExposure() : await startGatewayExposure());
    } catch (error) {
      showToast(String(error), {
        title: exposureRunning ? "停止公网暴露失败" : "启动公网暴露失败",
        kind: "error",
      });
    } finally {
      exposureBusy = false;
    }
  }

  async function unbindSession(sessionKey: string) {
    try {
      await clearGatewaySession(sessionKey);
      status = await getGatewayStatus();
      showToast("会话已解绑；下一次项目操作将重新要求选择工作区", { kind: "success" });
    } catch (error) {
      showToast(String(error), { title: "解绑失败", kind: "error" });
    }
  }

  async function save() {
    if (!config || !exposure || running || exposureRunning || saving) return;
    saving = true;
    try {
      await setGatewayConfig(config);
      await setGatewayExposure(exposure);
      if (gatewayFrpToken.trim()) {
        await setSharedSecret("gateway_frp_token", gatewayFrpToken.trim());
      }
      if (gatewayCloudflareToken.trim()) {
        await setSharedSecret("gateway_cloudflare_token", gatewayCloudflareToken.trim());
      }
      status = await getGatewayStatus();
      exposureStatus = await getGatewayExposureStatus();
      showToast("Gateway 与 Public Access 配置已保存", { kind: "success" });
    } catch (error) {
      showToast(String(error), { title: "保存失败", kind: "error" });
    } finally {
      saving = false;
    }
  }

  async function toggleGateway() {
    if (busy) return;
    busy = true;
    try {
      status = running ? await stopGateway() : await startGateway();
      applyExposureStatus(await getGatewayExposureStatus());
    } catch (error) {
      showToast(String(error), { title: running ? "停止失败" : "启动失败", kind: "error" });
    } finally {
      busy = false;
    }
  }

  async function restartRunningGateway() {
    if (busy || !running) return;
    busy = true;
    try {
      status = await restartGateway();
      applyExposureStatus(await getGatewayExposureStatus());
      showToast("Gateway 已重启", { kind: "success" });
    } catch (error) {
      showToast(String(error), { title: "重启失败", kind: "error" });
    } finally {
      busy = false;
    }
  }

  function applyExposureStatus(next: GatewayExposureStatus) {
    exposureStatus = next;
    if (!config || !exposure) return;
    const quickUrl = exposure.mode === "cloudflare"
      && exposure.cloudflareMode === "quick"
      && next.state === "running"
      ? next.effectivePublicUrl
      : "";
    config.publicUrl = quickUrl || next.canonicalPublicUrl;
  }

  function redactSession(value: string): string {
    if (value.length <= 14) return value;
    return `${value.slice(0, 7)}…${value.slice(-5)}`;
  }


  function mcpEndpoint(base: string): string {
    const normalized = base.trim().replace(/\/+$/, "");
    if (!normalized) return "";
    return normalized.endsWith("/mcp") ? normalized : `${normalized}/mcp`;
  }
</script>

<section class="page-scroll">
  <div class="mx-auto grid w-full max-w-5xl gap-5 p-6">
  <section class="tx-card p-5">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <div class="flex items-center gap-2">
          <StatusOrb state={running ? "running" : "stopped"} />
          <h1 class="text-lg font-semibold">全局 MCP Gateway</h1>
        </div>
        <p class="mt-2 max-w-3xl text-sm text-[var(--color-text-muted)]">
          推荐模式：ChatGPT 只创建一个 Coding Tools 插件并连接根 <span class="tx-mono">/mcp</span>。
          不同项目由会话内的 <span class="tx-mono">select_workspace</span> 绑定，避免每个工作区都创建一个插件。
        </p>
      </div>
      <div class="flex gap-2">
        {#if running}
          <button type="button" class="tx-btn-ghost" disabled={busy} onclick={restartRunningGateway}>
            {busy ? "处理中…" : "重启 Gateway"}
          </button>
        {/if}
        <button
          type="button"
          class="tx-btn-primary"
          class:tx-btn-danger={running}
          disabled={busy}
          onclick={toggleGateway}
        >
          {busy ? "处理中…" : running ? "停止 Gateway" : "启动 Gateway"}
        </button>
      </div>
    </div>

    {#if status}
      <div class="mt-5 grid gap-3 md:grid-cols-2">
        <div class="tx-info-block">
          <div class="tx-info-row">
            <span class="tx-info-label">本地 MCP</span>
            <CopyButton value={status.localEndpoint} />
          </div>
          <p class="tx-mono mt-1.5 break-all text-sm">{status.localEndpoint}</p>
        </div>
        <div class="tx-info-block">
          <div class="tx-info-row">
            <span class="tx-info-label">公网 MCP</span>
            {#if displayedPublicEndpoint}<CopyButton value={displayedPublicEndpoint} />{/if}
          </div>
          <p class="tx-mono mt-1.5 break-all text-sm text-[var(--color-text-secondary)]">
            {displayedPublicEndpoint || "未配置公网 URL"}
          </p>
        </div>
      </div>
      <div class="mt-3 flex gap-6 text-sm text-[var(--color-text-muted)]">
        <span>工作区：{status.workspaceCount}</span>
        <span>已绑定会话：{status.sessionCount}</span>
      </div>
    {/if}
  </section>

  {#if config}
    <section class="tx-card p-5">
      <h2 class="text-[15px] font-semibold">Gateway 配置</h2>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">
        默认只监听 127.0.0.1；需要局域网访问或路由器端口映射时改为 0.0.0.0。运行中配置锁定。
      </p>

      <div class="mt-5 grid gap-4 md:grid-cols-2">
        <label class="grid gap-1.5 text-sm">
          <span>监听地址</span>
          <input class="tx-input" bind:value={config.bindHost} disabled={running} placeholder="127.0.0.1" />
        </label>
        <label class="grid gap-1.5 text-sm">
          <span>端口</span>
          <input class="tx-input" type="number" min="1" max="65535" bind:value={config.localPort} disabled={running} />
        </label>
        <label class="grid gap-1.5 text-sm md:col-span-2">
          <span>公网 URL</span>
          <input
            class="tx-input"
            bind:value={config.publicUrl}
            disabled={running}
            placeholder="https://mcp.example.com（/mcp 可省略）"
          />
          <span class="text-xs text-[var(--color-text-muted)]">
            {#if exposure?.mode === "cloudflare" && exposure.cloudflareMode === "quick" && exposureRunning}
              当前 Cloudflare Quick Tunnel 临时地址；重启后会自动刷新，不会永久覆盖 canonical 配置。
            {:else}
              ChatGPT 对外访问这个 Gateway 的地址。普通使用只需要填好这个 HTTPS URL。
            {/if}
          </span>
        </label>
        <label class="grid gap-1.5 text-sm">
          <span>认证</span>
          <select class="tx-input" bind:value={config.authType} disabled={running}>
            <option value="oauth">OAuth（推荐）</option>
            <option value="bearer">Bearer Token</option>
            <option value="noauth">无认证（仅受控网络）</option>
          </select>
        </label>
        <label class="grid gap-1.5 text-sm">
          <span>工具集</span>
          <select class="tx-input" bind:value={config.toolProfile} disabled={running}>
            <option value="core">core</option>
            <option value="advanced">advanced</option>
            <option value="read-only">read-only</option>
          </select>
        </label>
        <label class="flex items-center gap-2 text-sm md:col-span-2">
          <input type="checkbox" bind:checked={config.autoSelectSingleWorkspace} disabled={running} />
          仅注册一个工作区时允许自动选择；两个及以上工作区始终要求显式选择
        </label>
      </div>

      <div class="mt-5 flex justify-end">
        <button type="button" class="tx-btn-primary" disabled={running || exposureRunning || saving} onclick={save}>
          {saving ? "保存中…" : "保存配置"}
        </button>
      </div>
    </section>
  {/if}

  {#if exposure && exposureStatus}
    <section class="tx-card p-5">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div class="flex items-center gap-2">
            <StatusOrb state={exposureRunning ? "running" : "stopped"} />
            <h2 class="text-[15px] font-semibold">公网访问 · 高级</h2>
          </div>
          <p class="mt-1 max-w-3xl text-sm text-[var(--color-text-muted)]">
            普通使用无需配置这里：在上方填写公网 URL 即可。只有希望 Coding Tools 自己启动 FRP 或 Cloudflare Tunnel 时才需要选择托管方式。
          </p>
        </div>
        {#if managedExposure}
          <button
            type="button"
            class="tx-btn-primary"
            class:tx-btn-danger={exposureRunning}
            disabled={exposureBusy || (!running && !exposureRunning)}
            onclick={toggleExposure}
          >
            {exposureBusy ? "处理中…" : exposureRunning ? "停止公网暴露" : "启动公网暴露"}
          </button>
        {/if}
      </div>

      <div class="mt-4 grid gap-3 md:grid-cols-2">
        <div class="tx-info-block">
          <div class="tx-info-row">
            <span class="tx-info-label">Canonical origin</span>
            {#if exposureStatus.canonicalPublicUrl}<CopyButton value={exposureStatus.canonicalPublicUrl} />{/if}
          </div>
          <p class="tx-mono mt-1.5 break-all text-sm">
            {exposureStatus.canonicalPublicUrl || "未配置"}
          </p>
        </div>
        <div class="tx-info-block">
          <div class="tx-info-row">
            <span class="tx-info-label">当前有效 MCP</span>
            {#if exposureStatus.effectivePublicUrl}<CopyButton value={mcpEndpoint(exposureStatus.effectivePublicUrl)} />{/if}
          </div>
          <p class="tx-mono mt-1.5 break-all text-sm text-[var(--color-text-secondary)]">
            {mcpEndpoint(exposureStatus.effectivePublicUrl) || "当前没有 managed/public endpoint"}
          </p>
        </div>
      </div>
      <p class="mt-3 text-sm text-[var(--color-text-muted)]">{exposureStatus.message}</p>

      <div class="mt-5 grid gap-4 md:grid-cols-2">
        <label class="grid gap-1.5 text-sm">
          <span>托管方式</span>
          <select class="tx-input" bind:value={exposure.mode} disabled={exposureRunning}>
            <option value="local">不由 Coding Tools 管理</option>
            <option value="frp">Managed FRP</option>
            <option value="cloudflare">Managed Cloudflare Tunnel</option>
          </select>
        </label>
        <div class="tx-info-block text-sm text-[var(--color-text-muted)]">
          {#if exposure.mode === "local"}
            Coding Tools 不关心公网 URL 背后是端口映射、Nginx、Caddy、VPS 还是其他隧道；健康检查直接验证你填写的公网 URL。
          {:else if exposure.mode === "frp"}
            Coding Tools 启动一个全局 frpc，仅代理 Gateway，而不是每个 Workspace 各启一条线路。
          {:else}
            Coding Tools 启动 cloudflared；Quick URL 会临时回填到上方公网 URL，停止后恢复 canonical URL。
          {/if}
        </div>

        {#if exposure.mode === "frp"}
          <label class="grid gap-1.5 text-sm">
            <span>FRP 全局配置</span>
            <select class="tx-input" bind:value={exposure.frpProfileId} disabled={exposureRunning}>
              <option value="">手动填写服务器</option>
              {#each frpProfiles as profile (profile.id)}
                <option value={profile.id}>{profile.name} · {profile.server}:{profile.serverPort}</option>
              {/each}
            </select>
          </label>
          <label class="grid gap-1.5 text-sm">
            <span>FRP 子域名</span>
            <input class="tx-input" bind:value={exposure.frpSubdomain} disabled={exposureRunning} placeholder="coding-tools" />
          </label>
          {#if !exposure.frpProfileId}
            <label class="grid gap-1.5 text-sm">
              <span>FRP Server</span>
              <input class="tx-input" bind:value={exposure.frpServer} disabled={exposureRunning} placeholder="frp.example.com" />
            </label>
            <label class="grid gap-1.5 text-sm">
              <span>FRP Server Port</span>
              <input class="tx-input" type="number" min="1" max="65535" bind:value={exposure.frpServerPort} disabled={exposureRunning} />
            </label>
            <label class="grid gap-1.5 text-sm md:col-span-2">
              <span>FRP Token（可选）</span>
              <SecretInput bind:value={gatewayFrpToken} disabled={exposureRunning} />
            </label>
          {/if}
        {:else if exposure.mode === "cloudflare"}
          <label class="grid gap-1.5 text-sm">
            <span>Cloudflare 模式</span>
            <select class="tx-input" bind:value={exposure.cloudflareMode} disabled={exposureRunning}>
              <option value="quick">Quick Tunnel（临时 URL）</option>
              <option value="named">Named Tunnel（固定 URL）</option>
            </select>
          </label>
          {#if exposure.cloudflareMode === "named"}
            <label class="grid gap-1.5 text-sm">
              <span>Tunnel Token</span>
              <SecretInput bind:value={gatewayCloudflareToken} disabled={exposureRunning} />
            </label>
          {/if}
        {/if}

        {#if managedExposure}
          <label class="flex items-center gap-2 text-sm md:col-span-2">
            <input type="checkbox" bind:checked={exposure.useProxy} disabled={exposureRunning} />
            managed exposure 使用“通用设置”中的全局出站代理
          </label>
        {/if}
      </div>
    </section>
  {/if}

  <section class="tx-card p-5">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h2 class="text-[15px] font-semibold">Gateway 健康检查</h2>
        <p class="mt-1 max-w-3xl text-sm text-[var(--color-text-muted)]">
          从本地 Gateway、你填写的公网 URL 到 OAuth metadata 逐层验证；只有托管 FRP/Cloudflare 时才额外检查 provider。
        </p>
      </div>
      <button type="button" class="tx-btn-primary" disabled={healthBusy} onclick={checkHealth}>
        {healthBusy ? "检查中…" : "运行健康检查"}
      </button>
    </div>

    {#if health}
      <div class="mt-4 tx-info-block">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span class="font-medium">{health.chatgptReady ? "ChatGPT-ready" : "Not ready"}</span>
          {#if health.publicBaseUrl}
            <span class="tx-mono text-xs text-[var(--color-text-muted)]">{health.publicBaseUrl}</span>
          {/if}
        </div>
        <p class="mt-1 text-sm text-[var(--color-text-secondary)]">{health.summary}</p>
      </div>

      <div class="mt-4 grid gap-2">
        {#each health.items as item (item.key)}
          <div class="tx-info-block">
            <div class="flex flex-wrap items-center gap-2">
              <span class="tx-mono text-xs">{healthBadge(item.status)}</span>
              <span class="text-xs uppercase text-[var(--color-text-muted)]">{item.layer}</span>
              <span class="text-sm font-medium">{item.label}</span>
            </div>
            <p class="mt-1 break-all text-sm text-[var(--color-text-secondary)]">{item.detail}</p>
            {#if item.hint}
              <p class="mt-1 text-xs text-[var(--color-text-muted)]">建议：{item.hint}</p>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#if status?.sessions?.length}
    <section class="tx-card p-5">
      <h2 class="text-[15px] font-semibold">当前会话路由</h2>
      <div class="mt-4 grid gap-2">
        {#each status.sessions as session (session.sessionKey)}
          <div class="tx-info-block flex flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
              <span class="tx-mono text-sm">{redactSession(session.sessionKey)}</span>
              <span class="ml-3 text-sm">→ {session.workspaceName}</span>
            </div>
            <button type="button" class="tx-btn-ghost px-2.5 py-1 text-xs" onclick={() => unbindSession(session.sessionKey)}>
              解绑
            </button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="tx-card p-5">
    <h2 class="text-[15px] font-semibold">单插件工作流</h2>
    <div class="tx-mono mt-3 text-sm leading-7 text-[var(--color-text-secondary)]">
      ChatGPT → /mcp → list_workspaces → select_workspace → history_session_bootstrap → 项目工具
    </div>
    <p class="mt-2 text-sm text-[var(--color-text-muted)]">
      Gateway 同时提供 <span class="tx-mono">/w/&lt;workspace-id&gt;/mcp</span> 显式路径用于调试或其他 MCP 客户端，
      但这些路径不需要在 ChatGPT 中分别创建插件。
    </p>
  </section>
  </div>
</section>
