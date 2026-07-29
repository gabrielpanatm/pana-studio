<script lang="ts">
  import { t } from "$lib/i18n/runtime.svelte";
  import type { WorkbenchSourceStatus } from "$lib/source-provenance";
  import type { GlobalStatusEvent } from "$lib/status/global-status";

  let {
    globalStatus = null,
    sourceStatus = null,
    openSource = () => {},
  }: {
    globalStatus?: GlobalStatusEvent | null;
    sourceStatus?: WorkbenchSourceStatus | null;
    openSource?: () => void | Promise<void>;
  } = $props();
</script>

<div
  class="status-bar"
  class:status-info={globalStatus?.severity === "info"}
  class:status-success={globalStatus?.severity === "success"}
  class:status-warning={globalStatus?.severity === "warning"}
  class:status-error={globalStatus?.severity === "error"}
  class:status-blocking={globalStatus?.severity === "blocking"}
  class:status-active={globalStatus?.phase === "active"}
>
  <div
    class="status-content"
    title={globalStatus?.detail ?? globalStatus?.message}
    role="status"
    aria-live="polite"
  >
    <span class="dot"></span>
    {#if globalStatus}
      <span class="text">{globalStatus.message}</span>
    {:else}
      <span class="text idle">Pană Studio</span>
    {/if}
  </div>

  {#if sourceStatus}
    {#if sourceStatus.openable}
      <button
        type="button"
        class="selection-source"
        title={t("context-menu-open-code")}
        aria-label={`${t("context-menu-open-code")}: ${sourceStatus.value}`}
        onclick={() => { void openSource(); }}
      >
        <span class="source-label">{sourceStatus.label}</span>
        <span class="source-path">{sourceStatus.value}</span>
      </button>
    {:else}
      <span class="selection-source readonly" title={sourceStatus.value}>
        <span class="source-label">{sourceStatus.label}</span>
        <span class="source-path">{sourceStatus.value}</span>
      </span>
    {/if}
  {/if}
</div>

<style>
  .status-bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, auto);
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    height: 26px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--skeuo-edge-highlight);
    border-top: 1px solid var(--border);
    background: var(--material-panel);
    box-shadow: 0 -1px 2px var(--skeuo-shade-soft);
    font-size: var(--font-meta);
    font-weight: 500;
    color: var(--text-muted);
    user-select: none;
  }

  .status-content {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .selection-source {
    display: inline-flex;
    align-items: center;
    justify-self: end;
    gap: 4px;
    max-width: min(48vw, 520px);
    min-width: 0;
    height: 20px;
    padding: 0 6px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    background: var(--surface-raised);
    box-shadow: var(--shadow-control);
    font: inherit;
    white-space: nowrap;
  }

  button.selection-source {
    cursor: pointer;
  }

  button.selection-source:hover {
    color: var(--text);
    border-color: var(--brand);
  }

  button.selection-source:focus-visible {
    outline: 2px solid var(--focus-ring, var(--brand));
    outline-offset: 1px;
  }

  .selection-source.readonly {
    opacity: 0.72;
  }

  .source-label {
    flex-shrink: 0;
    color: var(--brand-strong);
    font-weight: 700;
  }

  .source-path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--border-4);
  }

  .text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .text.idle {
    opacity: 0.4;
    font-size: 11px;
    letter-spacing: 0.04em;
  }

  .status-info .dot,
  .status-active .dot { background: var(--info); }
  .status-info,
  .status-active { color: var(--info); }

  .status-success .dot { background: var(--success); }
  .status-success { color: var(--success); }

  .status-warning .dot { background: var(--warning); }
  .status-warning { color: var(--warning); }

  .status-error .dot,
  .status-blocking .dot { background: var(--danger); }
  .status-error,
  .status-blocking { color: var(--danger); }
</style>
