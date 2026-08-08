<script lang="ts">
  import { onMount } from "svelte";
  import CopyButton from "$lib/components/CopyButton.svelte";
  import StatusOrb from "$lib/components/StatusOrb.svelte";
  import {
    getGatewayConfig,
    getGatewayStatus,
    clearGatewaySession,
    setGatewayConfig,
    startGateway,
    stopGateway,
    type GatewayConfig,
    type GatewayStatus,
  } from "$lib/api/gateway";
  import { showToast } from "$lib/stores/toast";

  let config = $state<GatewayConfig | null>(null);
  let status = $state<GatewayStatus | null>(null);
  let busy = $state(false);
  let saving = $state(false);

  const running = $derived(status?.state === "running");

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      [config, status] = await Promise.all([getGatewayConfig(), getGatewayStatus()]);
    } catch (error) {
      showToast(String(error), { title: "读取 Gateway 失败", kind: "error" });
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
    if (!config || running || saving) return;
    saving = true;
    try {
      await setGatewayConfig(config);
      status = await getGatewayStatus();
      showToast("全局 Gateway 配置已保存", { kind: "success" });
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
    } catch (error) {
      showToast(String(error), { title: running ? "停止失败" : "启动失败", kind: "error" });
    } finally {
      busy = false;
    }
  }

  function redactSession(value: string): string {
    if (value.length <= 14) return value;
    return `${value.slice(0, 7)}…${value.slice(-5)}`;
  }
</script>

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
            {#if status.publicEndpoint}<CopyButton value={status.publicEndpoint} />{/if}
          </div>
          <p class="tx-mono mt-1.5 break-all text-sm text-[var(--color-text-secondary)]">
            {status.publicEndpoint || "未配置；可使用端口映射或 HTTPS 反向代理"}
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
          <span>公网基地址</span>
          <input
            class="tx-input"
            bind:value={config.publicUrl}
            disabled={running}
            placeholder="https://mcp.example.com（可不带 /mcp）"
          />
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
        <button type="button" class="tx-btn-primary" disabled={running || saving} onclick={save}>
          {saving ? "保存中…" : "保存配置"}
        </button>
      </div>
    </section>
  {/if}

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

