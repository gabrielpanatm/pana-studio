<script lang="ts">
  import {
    previewFreshnessLabel,
    zolaValidationLabel,
    type ControlledPreviewState,
  } from "$lib/preview/controlled";
  import type { SaveState } from "$lib/types";
  import type { AiCoordinationSnapshot } from "$lib/types";
  import type { CanvasPatchPerformanceSnapshot } from "$lib/editor-runtime/preview-runtime";
  import AiEditAuthorityIndicator from "$lib/components/ai/AiEditAuthorityIndicator.svelte";

  let {
    saveState = "idle",
    saveStatus = "",
    controlledPreview = undefined,
    canvasPatchPerformance = undefined,
    sourceLabel = "",
    sourceValue = "",
    sourceOpenable = false,
    aiCoordinationSnapshot = null,
    externalReconciling = false,
    projectionRecoveryRequired = false,
    openSource = () => {},
  }: {
    saveState?: SaveState;
    saveStatus?: string;
    controlledPreview?: ControlledPreviewState;
    canvasPatchPerformance?: CanvasPatchPerformanceSnapshot;
    sourceLabel?: string;
    sourceValue?: string;
    sourceOpenable?: boolean;
    aiCoordinationSnapshot?: AiCoordinationSnapshot | null;
    externalReconciling?: boolean;
    projectionRecoveryRequired?: boolean;
    openSource?: () => void | Promise<void>;
  } = $props();

  const previewLabel = $derived(controlledPreview ? previewFreshnessLabel(controlledPreview) : "");
  const zolaLabel = $derived(controlledPreview ? zolaValidationLabel(controlledPreview) : "");
</script>

<div
  class="status-bar"
  class:unsaved={saveState === "unsaved"}
  class:saving={saveState === "saving"}
  class:saved={saveState === "saved"}
  class:restored={saveState === "restored"}
  class:error={saveState === "error"}
  role="status"
  aria-live="polite"
>
  <div class="status-left" title={saveStatus || undefined}>
    <span class="dot"></span>
    {#if saveState !== "idle" && saveStatus}
      <span class="text">{saveStatus}</span>
    {:else}
      <span class="text idle">Pană Studio</span>
    {/if}
  </div>

  <div class="status-right">
    <AiEditAuthorityIndicator
      snapshot={aiCoordinationSnapshot}
      {externalReconciling}
      {projectionRecoveryRequired}
    />
    {#if controlledPreview}
      <span
        class={`preview-chip preview-${controlledPreview.freshness}`}
        title={controlledPreview.message}
      >
        {previewLabel}
      </span>
      <span
        class={`preview-chip zola-${controlledPreview.validation}`}
        title={controlledPreview.validationMessage}
      >
        {zolaLabel}
      </span>
    {/if}
    {#if canvasPatchPerformance && canvasPatchPerformance.sampleCount > 0}
      <span
        class:patch-budget-ok={canvasPatchPerformance.budgetMet === true}
        class:patch-budget-failed={canvasPatchPerformance.budgetMet === false}
        class="preview-chip patch-performance"
        title={`CanvasPatch receipt→commit · n=${canvasPatchPerformance.sampleCount} · p50=${canvasPatchPerformance.receiptToCommitP50Ms?.toFixed(1) ?? "—"} ms · max=${canvasPatchPerformance.receiptToCommitMaxMs?.toFixed(1) ?? "—"} ms · bridge p95=${canvasPatchPerformance.bridgeCommitP95Ms?.toFixed(1) ?? "—"} ms`}
      >
        Patch p95 {canvasPatchPerformance.receiptToCommitP95Ms?.toFixed(1) ?? "—"} ms
      </span>
    {/if}
    {#if sourceValue}
      {#if sourceOpenable}
        <button type="button" class="source-chip" title="Deschide sursa" onclick={() => { void openSource(); }}>
          <span class="source-label">{sourceLabel}</span>
          <span class="source-path">{sourceValue}</span>
        </button>
      {:else}
        <span class="source-chip readonly" title={sourceValue}>
          <span class="source-label">{sourceLabel}</span>
          <span class="source-path">{sourceValue}</span>
        </span>
      {/if}
    {/if}
  </div>
</div>

<style>
  .status-bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    height: 36px;
    flex-shrink: 0;
    border-top: 1px solid var(--border);
    background: var(--surface-panel);
    font-size: var(--font-meta);
    font-weight: 500;
    color: var(--text-muted);
    user-select: none;
  }

  .status-left,
  .status-right {
    min-width: 0;
  }

  .status-left {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .status-right {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: flex-end;
    overflow: hidden;
  }

  .status-right > :global(.ai-authority) {
    flex: 0 1 auto;
  }

  .patch-performance.patch-budget-ok {
    color: var(--success, #0f766e);
    border-color: color-mix(in srgb, currentColor 35%, var(--border));
  }

  .patch-performance.patch-budget-failed {
    color: var(--danger, #dc2626);
    border-color: color-mix(in srgb, currentColor 35%, var(--border));
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

  .source-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    min-width: 0;
    min-height: 22px;
    padding: 0 6px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    background: var(--surface-raised);
    font-size: var(--font-meta);
    white-space: nowrap;
  }

  .preview-chip {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    max-width: 118px;
    height: 22px;
    padding: 0 6px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control);
    overflow: hidden;
    color: var(--text-muted);
    background: var(--surface-raised);
    font-size: var(--font-meta);
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .preview-live,
  .preview-saved {
    border-color: color-mix(in srgb, var(--success) 44%, var(--border-3));
    color: var(--success);
  }

  .preview-refreshing,
  .zola-running,
  .zola-queued {
    border-color: color-mix(in srgb, var(--info) 44%, var(--border-3));
    color: var(--info);
  }

  .preview-canonical,
  .zola-valid {
    border-color: color-mix(in srgb, var(--success) 44%, var(--border-3));
    color: var(--success);
  }

  .preview-stale,
  .zola-idle {
    border-color: color-mix(in srgb, var(--warning) 38%, var(--border-3));
    color: var(--warning);
  }

  .preview-error,
  .zola-invalid,
  .zola-error {
    border-color: color-mix(in srgb, var(--danger) 48%, var(--border-3));
    color: var(--danger);
  }

  button.source-chip {
    min-height: 32px;
    cursor: pointer;
  }

  button.source-chip:hover {
    color: var(--text);
    border-color: var(--brand);
  }

  .source-label {
    flex-shrink: 0;
    color: var(--brand-strong);
    font-weight: 650;
  }

  .source-path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .text.idle {
    opacity: 0.4;
    font-size: 12px;
    letter-spacing: 0.04em;
  }

  .unsaved .dot { background: var(--warning); }
  .unsaved      { color: var(--warning); }

  .saving .dot  { background: var(--info); }
  .saving       { color: var(--info); }

  .saved .dot   { background: var(--success); }
  .saved        { color: var(--success); }

  .restored .dot { background: var(--info); }
  .restored      { color: var(--info); }

  .error .dot   { background: var(--danger); }
  .error        { color: var(--danger); }

  button:focus-visible {
    outline: 2px solid var(--focus-ring, var(--brand));
    outline-offset: 2px;
  }
</style>
