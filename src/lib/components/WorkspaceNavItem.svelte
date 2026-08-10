<script lang="ts">
  import type { WorkspaceProfile } from "$lib/types";

  interface Props {
    workspace: WorkspaceProfile;
    active: boolean;
    onClick: () => void;
  }

  let { workspace, active, onClick }: Props = $props();
</script>

<div class="tx-nav-item" class:active>
  <button type="button" class="tx-nav-button" onclick={onClick}>
    <span class="min-w-0 flex-1 truncate text-sm font-medium">{workspace.name}</span>
  </button>
</div>
