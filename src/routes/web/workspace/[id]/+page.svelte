<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { getGatewayStatus } from "$lib/api/gateway";
  import { deleteWorkspace, listWorkspaces, updateWorkspace } from "$lib/api/workspaces";
  import { goto } from "$app/navigation";
  import { setLastWorkspace } from "$lib/api/settings";
  import CopyButton from "$lib/components/CopyButton.svelte";
  import { showToast } from "$lib/stores/toast";
  import { workspaces } from "$lib/stores/app";
  import type { WorkspaceProfile } from "$lib/types";

  let profile = $state<WorkspaceProfile | null>(null);
  let gatewayRunning = $state(false);
  let saving = $state(false);

  const workspaceId = $derived($page.params.id);
  const route = $derived(profile ? `/w/${profile.id}/mcp` : "");

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      const [items, status] = await Promise.all([listWorkspaces(), getGatewayStatus()]);
      workspaces.set(items);
      profile = items.find((item) => item.id === workspaceId) ?? null;
      gatewayRunning = status.state === "running";
      if (profile) await setLastWorkspace(profile.id);
    } catch (error) {
      showToast(String(error), { title: "读取工作区失败", kind: "error" });
    }
  }

  async function removeWorkspace() {
    if (!profile || gatewayRunning) return;
    if (!window.confirm(`确定删除工作区“${profile.name}”的注册信息吗？不会删除项目目录本身。`)) return;
    try {
      await deleteWorkspace(profile.id);
      const items = await listWorkspaces();
      workspaces.set(items);
      await goto("/settings/gateway");
      showToast("工作区已从 Coding Tools 中移除；项目文件未删除", { kind: "success" });
    } catch (error) {
      showToast(String(error), { title: "删除失败", kind: "error" });
    }
  }

  async function save() {
    if (!profile || saving || gatewayRunning) return;
    saving = true;
    try {
      await updateWorkspace(profile);
      await load();
      showToast("工作区配置已保存", { kind: "success" });
    } catch (error) {
      showToast(String(error), { title: "保存失败", kind: "error" });
    } finally {
      saving = false;
    }
  }
</script>

<div class="mx-auto grid w-full max-w-5xl gap-5 p-6">
  {#if profile}
    <section class="tx-card p-5">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 class="text-lg font-semibold">{profile.name}</h1>
          <p class="tx-mono mt-1 break-all text-sm text-[var(--color-text-muted)]">{profile.path}</p>
        </div>
        <div class="flex gap-2">
          <button type="button" class="tx-btn-ghost" disabled={gatewayRunning || saving} onclick={removeWorkspace}>
            移除
          </button>
          <button type="button" class="tx-btn-primary" disabled={gatewayRunning || saving} onclick={save}>
            {saving ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
      {#if gatewayRunning}
        <div class="tx-alert mt-4">
          全局 Gateway 正在运行。为避免会话路由到变化中的目录，工作区定义在 Gateway 停止前保持只读。
        </div>
      {/if}
    </section>

    <section class="tx-card p-5">
      <h2 class="text-[15px] font-semibold">工作区</h2>
      <div class="mt-4 grid gap-4 md:grid-cols-2">
        <label class="grid gap-1.5 text-sm">
          <span>名称</span>
          <input class="tx-input" bind:value={profile.name} disabled={gatewayRunning} />
        </label>
        <label class="grid gap-1.5 text-sm md:col-span-2">
          <span>服务器路径</span>
          <input class="tx-input tx-mono" bind:value={profile.path} disabled={gatewayRunning} />
        </label>
        <label class="grid gap-1.5 text-sm">
          <span>权限模式</span>
          <select class="tx-input" bind:value={profile.runtime.permission_mode} disabled={gatewayRunning}>
            <option value="trusted">trusted</option>
            <option value="prompt">prompt</option>
            <option value="dangerous">dangerous</option>
          </select>
        </label>
        <label class="grid gap-1.5 text-sm md:col-span-2">
          <span>允许执行的命令</span>
          <input
            class="tx-input tx-mono"
            bind:value={profile.runtime.allowed_commands}
            disabled={gatewayRunning}
            placeholder="python,python3,npm,node,cargo,git"
          />
        </label>
        <label class="flex items-center gap-2 text-sm md:col-span-2">
          <input
            type="checkbox"
            bind:checked={profile.runtime.workspace_local_entries}
            disabled={gatewayRunning}
          />
          允许解析工作区内的本地脚本/可执行入口
        </label>
      </div>
      <p class="mt-3 text-xs text-[var(--color-text-muted)]">
        Gateway 对外暴露的工具目录由“设置 → Gateway”的全局工具集决定；这里配置的是该工作区自己的执行权限与命令策略。
      </p>
    </section>

    <section class="tx-card p-5">
      <h2 class="text-[15px] font-semibold">Gateway 路由</h2>
      <div class="tx-info-block mt-4">
        <div class="tx-info-row">
          <span class="tx-info-label">内部路径</span>
          <CopyButton value={route} />
        </div>
        <p class="tx-mono mt-1.5 text-sm">{route}</p>
      </div>
      <p class="mt-3 text-sm text-[var(--color-text-muted)]">
        ChatGPT 不需要为这个路径单独创建插件。根 <span class="tx-mono">/mcp</span> 通过
        <span class="tx-mono">select_workspace</span> 将当前会话绑定到此工作区。
      </p>
    </section>

    <section class="tx-card p-5">
      <h2 class="text-[15px] font-semibold">Web Console 迁移状态</h2>
      <p class="mt-2 text-sm leading-6 text-[var(--color-text-muted)]">
        Web 模式目前优先管理全局 Gateway、会话路由和工作区基础策略。旧的每工作区 MCP、Actions、FRP 与桌面专用文件选择仍保留在 Tauri 兼容界面，后续会逐步迁移到统一 Admin API。
      </p>
    </section>
  {:else}
    <section class="tx-card p-5">
      <h1 class="text-lg font-semibold">找不到工作区</h1>
      <p class="mt-2 text-sm text-[var(--color-text-muted)]">该工作区可能已被删除或配置尚未刷新。</p>
    </section>
  {/if}
</div>
