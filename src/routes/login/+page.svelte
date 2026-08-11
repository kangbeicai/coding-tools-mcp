<script lang="ts">
  import { onMount } from "svelte";
  import {
    getAdminAuthStatus,
    loginAdmin,
    setupAdmin,
    type AdminAuthStatus,
  } from "$lib/api/adminAuth";

  let status: AdminAuthStatus | null = null;
  let username = "admin";
  let password = "";
  let confirmPassword = "";
  let error = "";
  let busy = false;

  function nextPath() {
    const value = new URLSearchParams(window.location.search).get("next");
    return value && value.startsWith("/") && !value.startsWith("//") ? value : "/activity";
  }

  async function loadStatus() {
    status = await getAdminAuthStatus();
    username = status.username || "admin";
    if (status.authenticated) {
      window.location.assign(nextPath());
    }
  }

  async function submit() {
    error = "";
    if (!status) return;
    if (!status.configured && password !== confirmPassword) {
      error = "两次输入的密码不一致";
      return;
    }
    busy = true;
    try {
      status = status.configured
        ? await loginAdmin(username.trim(), password)
        : await setupAdmin(username.trim(), password);
      window.location.assign(nextPath());
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void loadStatus().catch((cause) => {
      error = cause instanceof Error ? cause.message : String(cause);
    });
  });
</script>

<svelte:head>
  <title>Admin Login · Coding Tools</title>
</svelte:head>

<main class="min-h-screen bg-slate-950 px-5 py-12 text-slate-100">
  <div class="mx-auto flex min-h-[75vh] max-w-md items-center">
    <section class="w-full rounded-2xl border border-slate-800 bg-slate-900/80 p-7 shadow-2xl">
      <div class="mb-6">
        <div class="mb-2 text-xs font-semibold uppercase tracking-[0.24em] text-cyan-400">Coding Tools</div>
        <h1 class="text-2xl font-semibold">
          {status?.configured === false ? "设置 Web Admin" : "登录 Web Admin"}
        </h1>
        <p class="mt-2 text-sm leading-6 text-slate-400">
          {#if status?.configured === false}
            首次使用请创建管理员账号。密码只保存 Argon2 哈希，不会以明文写入配置。
          {:else}
            登录后才能查看 Activity、Gateway 设置和管理 API。
          {/if}
        </p>
      </div>

      <form class="space-y-4" onsubmit={(event) => { event.preventDefault(); void submit(); }}>
        <label class="block">
          <span class="mb-1.5 block text-sm text-slate-300">用户名</span>
          <input
            class="w-full rounded-xl border border-slate-700 bg-slate-950 px-3.5 py-3 outline-none transition focus:border-cyan-500"
            bind:value={username}
            autocomplete="username"
            minlength="3"
            maxlength="64"
            required
          />
        </label>

        <label class="block">
          <span class="mb-1.5 block text-sm text-slate-300">密码</span>
          <input
            class="w-full rounded-xl border border-slate-700 bg-slate-950 px-3.5 py-3 outline-none transition focus:border-cyan-500"
            type="password"
            bind:value={password}
            autocomplete={status?.configured === false ? "new-password" : "current-password"}
            minlength="8"
            required
          />
        </label>

        {#if status?.configured === false}
          <label class="block">
            <span class="mb-1.5 block text-sm text-slate-300">确认密码</span>
            <input
              class="w-full rounded-xl border border-slate-700 bg-slate-950 px-3.5 py-3 outline-none transition focus:border-cyan-500"
              type="password"
              bind:value={confirmPassword}
              autocomplete="new-password"
              minlength="8"
              required
            />
          </label>
        {/if}

        {#if error}
          <div class="rounded-xl border border-red-900/60 bg-red-950/50 px-3.5 py-3 text-sm text-red-200">
            {error}
          </div>
        {/if}

        <button
          type="submit"
          class="w-full rounded-xl bg-cyan-500 px-4 py-3 font-medium text-slate-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
          disabled={busy || !status}
        >
          {busy ? "处理中…" : status?.configured === false ? "创建管理员并登录" : "登录"}
        </button>
      </form>

      <p class="mt-5 text-xs leading-5 text-slate-500">
        Activity 会显示真实 MCP Request/Response，可能包含敏感参数；请仅在受信任网络中开放 Web Admin。
      </p>
    </section>
  </div>
</main>
