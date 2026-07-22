<script lang="ts">
  import type { RecoveryCoordinatorDiagnostic } from "$lib/types";
  import { recoverySeverityLabel } from "$lib/kernel/recovery-control";

  let {
    diagnostics = [],
    compact = false,
    label = "Diagnostics Transaction Log",
    highlighted = false,
  }: {
    diagnostics?: RecoveryCoordinatorDiagnostic[];
    compact?: boolean;
    label?: string;
    highlighted?: boolean;
  } = $props();
</script>

{#if diagnostics.length}
  <div class="diagnostic-list" class:compact class:kernel-context-focus={highlighted} aria-label={label}>
    {#each diagnostics as diagnostic}
      <article class={`diagnostic-item ${diagnostic.severity}`}>
        <strong>{recoverySeverityLabel(diagnostic.severity)} · {diagnostic.code}</strong>
        <span>{diagnostic.message}</span>
      </article>
    {/each}
  </div>
{/if}

<style>
  .diagnostic-list {
    display: grid;
    gap: 8px;
    padding: 0 12px 12px;
  }

  .diagnostic-list.compact {
    padding: 0;
  }

  .diagnostic-item {
    display: grid;
    gap: 4px;
    padding: 9px 10px;
    border: 1px solid var(--border-2);
    border-radius: 7px;
    background: var(--surface-2);
  }

  .diagnostic-item.warning {
    border-color: color-mix(in srgb, #f0b25b 42%, var(--border));
    background: color-mix(in srgb, #f0b25b 8%, var(--surface-4));
  }

  .diagnostic-item.error {
    border-color: color-mix(in srgb, #ef4444 40%, var(--border));
    background: color-mix(in srgb, #ef4444 8%, var(--surface-4));
  }

  .diagnostic-item strong {
    color: var(--text-strong);
    font-size: 12px;
    font-weight: 850;
  }

  .diagnostic-item span {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }
</style>
