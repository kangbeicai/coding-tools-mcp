<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import AppShell from "$lib/components/AppShell.svelte";
  import ToastHost from "$lib/components/ToastHost.svelte";
  import WorkspaceNavItem from "$lib/components/WorkspaceNavItem.svelte";
  import { createWorkspace, listWorkspaces } from "$lib/api/workspaces";
  import { workspaces } from "$lib/stores/app";
  import { showToast } from "$lib/stores/toast";

  let { children } = $props();

  async function refreshWorkspaces() {
    const items = await listWorkspaces();
    workspaces.set(items);
  }

  async function addWorkspace() {
    try {
      const selected = window.prompt("请输入服务器上的工作区绝对路径，例如 /home/user/project：")?.trim();
      if (!selected) return;
      const profile = await createWorkspace(selected);
      await refreshWorkspaces();
      goto(`/web/workspace/${profile.id}`);
    } catch (error) {
      showToast(String(error), {
        title: "添加工作区失败",
        kind: "error",
        duration: 8000,
      });
    }
  }

  function openWorkspace(id: string) {
    goto(`/web/workspace/${id}`);
  }

  function openGatewaySettings() {
    goto("/settings/gateway");
  }

  function openKeysSettings() {
    goto("/settings/keys");
  }

  onMount(() => {
    void (async () => {
      await refreshWorkspaces();
      const path = $page.url.pathname;
      if (path === "/") {
        goto("/settings/gateway");
      }
    })();
  });
</script>

<AppShell onAddWorkspace={addWorkspace}>
  {#snippet settingsNav()}
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/gateway' ? 'active' : ''}"
      onclick={openGatewaySettings}
    >
      Gateway
    </button>
    <button
      type="button"
      class="tx-settings-link {$page.url.pathname === '/settings/keys' ? 'active' : ''}"
      onclick={openKeysSettings}
    >
      共享密钥
    </button>
  {/snippet}
  {#snippet sidebar()}
    <div class="space-y-1">
      {#each $workspaces as workspace (workspace.id)}
        <WorkspaceNavItem
          workspace={workspace}
          active={$page.url.pathname === `/web/workspace/${workspace.id}`}
          onClick={() => openWorkspace(workspace.id)}
        />
      {/each}
    </div>
  {/snippet}

  {#snippet children()}
    {@render children()}
  {/snippet}
</AppShell>

<ToastHost />
