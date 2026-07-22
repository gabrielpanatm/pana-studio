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
    previewZoom = 100,
    showPreviewZoom = true,
    sourceLabel = "",
    sourceValue = "",
    sourceOpenable = false,
    aiCoordinationSnapshot = null,
    externalReconciling = false,
    projectionRecoveryRequired = false,
    setPreviewZoom = () => {},
    commitPreviewZoom = () => {},
    resetPreviewZoom = () => {},
    openSource = () => {},
  }: {
    saveState?: SaveState;
    saveStatus?: string;
    controlledPreview?: ControlledPreviewState;
    canvasPatchPerformance?: CanvasPatchPerformanceSnapshot;
    previewZoom?: number;
    showPreviewZoom?: boolean;
    sourceLabel?: string;
    sourceValue?: string;
    sourceOpenable?: boolean;
    aiCoordinationSnapshot?: AiCoordinationSnapshot | null;
    externalReconciling?: boolean;
    projectionRecoveryRequired?: boolean;
    setPreviewZoom?: (value: number) => void;
    commitPreviewZoom?: (value: number) => void | Promise<void>;
    resetPreviewZoom?: () => void;
    openSource?: () => void | Promise<void>;
  } = $props();

  const zoomProgress = $derived(Math.max(0, Math.min(100, ((previewZoom - 25) / 175) * 100)));
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
  class:without-zoom={!showPreviewZoom}
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

  {#if showPreviewZoom}
    <div class="zoom-control" aria-label="Zoom previzualizare">
      <button type="button" class="zoom-reset" onclick={resetPreviewZoom}>Restabilește</button>
      <input
        class="zoom-slider"
        type="range"
        min="25"
        max="200"
        step="5"
        value={previewZoom}
        style={`--zoom-progress: ${zoomProgress}%`}
        title={`Zoom previzualizare ${previewZoom}%`}
        oninput={(event) => setPreviewZoom(Number(event.currentTarget.value))}
        onchange={(event) => { void commitPreviewZoom(Number(event.currentTarget.value)); }}
      />
      <span class="zoom-value">{previewZoom}%</span>
    </div>
  {/if}

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
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    height: 36px;
    flex-shrink: 0;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
    user-select: none;
  }

  .status-left,
  .status-right,
  .zoom-control {
    min-width: 0;
  }

  .status-bar.without-zoom {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
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

  .zoom-control {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    color: var(--text-muted);
  }

  .zoom-reset {
    min-height: 32px;
    padding: 0 4px;
    border: 0;
    border-radius: 0;
    color: inherit;
    background: transparent;
    font-size: 12px;
    cursor: pointer;
  }

  .zoom-reset:hover {
    color: var(--text);
  }

  .zoom-slider {
    width: 160px;
    height: 32px;
    padding: 0;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    cursor: pointer;
  }

  .zoom-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--brand) 0%,
      var(--brand) var(--zoom-progress, 0%),
      var(--border-4) var(--zoom-progress, 0%),
      var(--border-4) 100%
    );
  }

  .zoom-slider::-webkit-slider-thumb {
    appearance: none;
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    margin-top: -4px;
    border: 1px solid var(--border-4);
    border-radius: 50%;
    background: var(--surface-2);
  }

  .zoom-slider::-moz-range-track {
    height: 4px;
    border-radius: 999px;
    background: var(--border-4);
  }

  .zoom-slider::-moz-range-progress {
    height: 4px;
    border-radius: 999px;
    background: var(--brand);
  }

  .zoom-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-width: 1px;
    border-color: var(--border-4);
    background: var(--surface-2);
  }

  .zoom-value {
    min-width: 38px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text);
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
    border-radius: 999px;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--surface-4) 70%, transparent);
    font-size: 12px;
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
    border-radius: 999px;
    overflow: hidden;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--surface-3) 74%, transparent);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .preview-live,
  .preview-saved {
    border-color: color-mix(in srgb, #10b981 44%, var(--border-3));
    color: #10b981;
  }

  .preview-refreshing,
  .zola-running,
  .zola-queued {
    border-color: color-mix(in srgb, #6366f1 44%, var(--border-3));
    color: #6366f1;
  }

  .preview-canonical,
  .zola-valid {
    border-color: color-mix(in srgb, #14b8a6 44%, var(--border-3));
    color: #14b8a6;
  }

  .preview-stale,
  .zola-idle {
    border-color: color-mix(in srgb, #d97706 38%, var(--border-3));
    color: #d97706;
  }

  .preview-error,
  .zola-invalid,
  .zola-error {
    border-color: color-mix(in srgb, #ef4444 48%, var(--border-3));
    color: #ef4444;
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
    font-weight: 800;
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

  .unsaved .dot { background: #d97706; }
  .unsaved      { color: #d97706; }

  .saving .dot  { background: #6366f1; }
  .saving       { color: #6366f1; }

  .saved .dot   { background: #10b981; }
  .saved        { color: #10b981; }

  .restored .dot { background: #0ea5e9; }
  .restored      { color: #0ea5e9; }

  .error .dot   { background: #ef4444; }
  .error        { color: #ef4444; }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--focus-ring, var(--brand));
    outline-offset: 2px;
  }
</style>
