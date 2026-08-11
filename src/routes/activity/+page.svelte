<script lang="ts">
  import { onMount } from "svelte";
  import { getActivity, listActivity, subscribeActivity } from "$lib/api/activity";
  import { showToast } from "$lib/stores/toast";
  import type { ActivityProcess, ActivitySnapshot, ActivityTrace } from "$lib/types";

  let snapshot = $state<ActivitySnapshot>({ traces: [], activeProcesses: [] });
  let selected = $state<ActivityTrace | null>(null);
  let loading = $state(true);
  let connected = $state(false);
  let now = $state(Date.now());
  let workspaceFilter = $state("");
  let sessionFilter = $state("");
  let toolFilter = $state("");
  let statusFilter = $state("");

  const activeCalls = $derived(snapshot.traces.filter((trace) => trace.status === "running"));

  async function load() {
    try {
      snapshot = await listActivity({
        workspace: workspaceFilter,
        session: sessionFilter,
        tool: toolFilter,
        status: statusFilter,
        limit: 250,
      });
      if (selected) selected = await getActivity(selected.traceId);
    } catch (error) {
      showToast(String(error), { title: "读取 Activity 失败", kind: "error" });
    } finally {
      loading = false;
    }
  }

  async function selectTrace(trace: ActivityTrace) {
    selected = (await getActivity(trace.traceId)) ?? trace;
  }

  function resetFilters() {
    workspaceFilter = "";
    sessionFilter = "";
    toolFilter = "";
    statusFilter = "";
    void load();
  }

  function durationMs(trace: ActivityTrace): number {
    return trace.durationMs ?? Math.max(0, now - trace.startedAtMs);
  }

  function formatDuration(value: number): string {
    if (value < 1000) return `${value}ms`;
    if (value < 60_000) return `${(value / 1000).toFixed(1)}s`;
    return `${Math.floor(value / 60_000)}m ${Math.floor((value % 60_000) / 1000)}s`;
  }

  function formatTime(value: number): string {
    return new Date(value).toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  function json(value: unknown): string {
    if (value === null || value === undefined) return "";
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }

  function processDuration(process: ActivityProcess): string {
    return formatDuration(Math.max(0, now - process.startedAtMs));
  }

  onMount(() => {
    void load();
    const source = subscribeActivity(
      () => {
        connected = true;
        void load();
      },
      () => {
        connected = false;
      },
    );
    source.addEventListener("open", () => {
      connected = true;
      void load();
    });
    const timer = window.setInterval(() => {
      now = Date.now();
    }, 250);
    return () => {
      source.close();
      window.clearInterval(timer);
    };
  });
</script>

<section class="page-scroll">
  <div class="mx-auto grid w-full max-w-7xl gap-5 p-6">
    <header class="flex flex-wrap items-start justify-between gap-4">
      <div>
        <div class="flex items-center gap-2">
          <h1 class="text-xl font-semibold">MCP Activity</h1>
          <span class="tx-badge">{connected ? "Live" : "Reconnecting"}</span>
        </div>
        <p class="mt-1 text-sm text-[var(--color-text-muted)]">
          实时查看 ChatGPT 发给 Coding MCP 的工具调用、返回结果，以及仍在运行的底层命令。
        </p>
      </div>
      <button type="button" class="tx-btn-ghost" onclick={() => void load()}>刷新快照</button>
    </header>

    <section class="tx-card p-5">
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="text-[15px] font-semibold">当前调用</h2>
          <p class="mt-1 text-xs text-[var(--color-text-muted)]">MCP request 已收到、但 response 尚未返回的调用。</p>
        </div>
        <span class="tx-mono text-xs text-[var(--color-text-muted)]">{activeCalls.length} active</span>
      </div>

      {#if activeCalls.length > 0}
        <div class="mt-4 grid gap-3 md:grid-cols-2">
          {#each activeCalls as trace (trace.traceId)}
            <button type="button" class="tx-info-block text-left" onclick={() => void selectTrace(trace)}>
              <div class="flex items-center justify-between gap-3">
                <div class="flex min-w-0 items-center gap-2">
                  <span class="h-2 w-2 shrink-0 rounded-full bg-[var(--color-accent)]"></span>
                  <span class="truncate font-medium">{trace.tool || trace.method}</span>
                </div>
                <span class="tx-mono text-xs">{formatDuration(durationMs(trace))}</span>
              </div>
              <div class="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-[var(--color-text-muted)]">
                <span>{trace.workspaceName || "未绑定工作区"}</span>
                <span>{trace.sessionId || "无会话标识"}</span>
                <span>{formatTime(trace.startedAtMs)}</span>
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <div class="mt-4 rounded-xl border border-dashed border-[var(--color-border)] p-5 text-sm text-[var(--color-text-muted)]">
          当前没有正在等待 response 的 MCP 调用。
        </div>
      {/if}
    </section>

    {#if snapshot.activeProcesses.length > 0}
      <section class="tx-card p-5">
        <h2 class="text-[15px] font-semibold">仍在运行的命令</h2>
        <p class="mt-1 text-xs text-[var(--color-text-muted)]">
          这些命令对应的 <span class="tx-mono">exec_command</span> 已经返回给 ChatGPT，但底层进程仍在运行。
        </p>
        <div class="mt-4 grid gap-3">
          {#each snapshot.activeProcesses as process (process.sessionId)}
            <div class="tx-info-block">
              <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="h-2 w-2 rounded-full bg-[var(--color-accent)]"></span>
                    <span class="font-medium">{process.status}</span>
                    <span class="tx-mono text-xs text-[var(--color-text-muted)]">{process.sessionId}</span>
                  </div>
                  <p class="tx-mono mt-2 break-all text-sm">{process.command || "后台命令"}</p>
                </div>
                <span class="tx-mono text-sm">{processDuration(process)}</span>
              </div>
              <div class="mt-2 text-xs text-[var(--color-text-muted)]">
                {process.workspaceName || "未绑定工作区"}
                {#if process.operationId} · operation {process.operationId}{/if}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <section class="tx-card p-5">
      <div class="flex flex-wrap items-end gap-3">
        <label class="grid min-w-40 flex-1 gap-1 text-xs">
          <span>Workspace</span>
          <input class="tx-input" bind:value={workspaceFilter} placeholder="名称" />
        </label>
        <label class="grid min-w-40 flex-1 gap-1 text-xs">
          <span>Session</span>
          <input class="tx-input tx-mono" bind:value={sessionFilter} placeholder="会话片段" />
        </label>
        <label class="grid min-w-40 flex-1 gap-1 text-xs">
          <span>Tool</span>
          <input class="tx-input tx-mono" bind:value={toolFilter} placeholder="exec_command" />
        </label>
        <label class="grid min-w-36 gap-1 text-xs">
          <span>Status</span>
          <select class="tx-input" bind:value={statusFilter}>
            <option value="">全部</option>
            <option value="running">running</option>
            <option value="completed">completed</option>
            <option value="failed">failed</option>
          </select>
        </label>
        <button type="button" class="tx-btn-primary" onclick={() => void load()}>应用筛选</button>
        <button type="button" class="tx-btn-ghost" onclick={resetFilters}>清空</button>
      </div>
    </section>

    <div class="grid gap-5 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
      <section class="tx-card overflow-hidden">
        <div class="border-b border-[var(--color-border)] px-5 py-4">
          <h2 class="text-[15px] font-semibold">最近调用</h2>
          <p class="mt-1 text-xs text-[var(--color-text-muted)]">最新 {snapshot.traces.length} 条匹配记录</p>
        </div>
        {#if loading}
          <div class="p-5 text-sm text-[var(--color-text-muted)]">加载中</div>
        {:else if snapshot.traces.length === 0}
          <div class="p-5 text-sm text-[var(--color-text-muted)]">暂无匹配调用。</div>
        {:else}
          <div class="divide-y divide-[var(--color-border)]">
            {#each snapshot.traces as trace (trace.traceId)}
              <button
                type="button"
                class="block w-full px-5 py-3 text-left transition hover:bg-[var(--color-surface-hover)]"
                onclick={() => void selectTrace(trace)}
              >
                <div class="flex items-center justify-between gap-3">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <span class="truncate text-sm font-medium">{trace.tool || trace.method}</span>
                      <span class="tx-badge">{trace.status}</span>
                    </div>
                    <div class="mt-1 truncate text-xs text-[var(--color-text-muted)]">
                      {trace.workspaceName || "未绑定"} · {trace.sessionId || "无 session"} · {formatTime(trace.startedAtMs)}
                    </div>
                  </div>
                  <span class="tx-mono shrink-0 text-xs">{formatDuration(durationMs(trace))}</span>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </section>

      <section class="tx-card min-w-0 p-5">
        {#if selected}
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <h2 class="truncate text-[15px] font-semibold">{selected.tool || selected.method}</h2>
              <p class="tx-mono mt-1 break-all text-xs text-[var(--color-text-muted)]">{selected.traceId}</p>
            </div>
            <span class="tx-badge">{selected.status}</span>
          </div>
          <div class="mt-4 grid gap-2 text-sm sm:grid-cols-2">
            <div class="tx-info-block"><span class="tx-info-label">Workspace</span><div class="mt-1">{selected.workspaceName || "未绑定"}</div></div>
            <div class="tx-info-block"><span class="tx-info-label">Duration</span><div class="mt-1 tx-mono">{formatDuration(durationMs(selected))}</div></div>
            <div class="tx-info-block"><span class="tx-info-label">Session</span><div class="mt-1 tx-mono">{selected.sessionId || "-"}</div></div>
            <div class="tx-info-block"><span class="tx-info-label">Route</span><div class="mt-1 tx-mono">{selected.route}</div></div>
          </div>
          {#if selected.processSessionId || selected.operationId}
            <div class="tx-alert mt-4">
              Process: <span class="tx-mono">{selected.processSessionId || "-"}</span>
              {#if selected.operationId} · operation <span class="tx-mono">{selected.operationId}</span>{/if}
            </div>
          {/if}
          <div class="mt-5 grid gap-4">
            <div>
              <h3 class="mb-2 text-sm font-medium">Request</h3>
              <pre class="max-h-96 overflow-auto rounded-xl border border-[var(--color-border)] bg-[var(--color-surface-muted)] p-3 text-xs">{json(selected.request)}</pre>
            </div>
            <div>
              <h3 class="mb-2 text-sm font-medium">Response</h3>
              <pre class="max-h-96 overflow-auto rounded-xl border border-[var(--color-border)] bg-[var(--color-surface-muted)] p-3 text-xs">{json(selected.response)}</pre>
            </div>
            {#if selected.status === "failed"}
              <div>
                <h3 class="mb-2 text-sm font-medium">Error</h3>
                <pre class="max-h-72 overflow-auto rounded-xl border border-[var(--color-border)] bg-[var(--color-surface-muted)] p-3 text-xs">{json(selected.error)}</pre>
              </div>
            {/if}
          </div>
        {:else}
          <div class="flex min-h-64 items-center justify-center text-center text-sm text-[var(--color-text-muted)]">
            选择左侧一次调用，查看原始 Request / Response 详情。
          </div>
        {/if}
      </section>
    </div>
  </div>
</section>
