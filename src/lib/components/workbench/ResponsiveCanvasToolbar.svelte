<script lang="ts">
  import {
    IconArrowsMaximize,
    IconBrush,
    IconDeviceDesktop,
    IconDeviceMobile,
    IconDeviceTablet,
    IconRuler2,
    IconSparkles,
    IconPointer,
  } from "@tabler/icons-svelte";
  import PreviewZoomControl from "$lib/components/workbench/PreviewZoomControl.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type {
    WorkbenchCanvasPreset,
    WorkbenchCanvasViewportSnapshot,
  } from "$lib/workbench/contracts";

  let {
    viewport,
    breakpoints = [],
    setViewport,
    setPreviewZoom,
    commitPreviewZoom,
    resetPreviewZoom,
    previewExecutionMode = "design",
    interactivePreviewUrl = "",
    setPreviewExecutionMode,
  }: {
    viewport: WorkbenchCanvasViewportSnapshot;
    breakpoints?: Array<{ id: string; label: string; widthPx: number }>;
    setViewport: (viewport: Partial<WorkbenchCanvasViewportSnapshot>) => void | Promise<void>;
    setPreviewZoom: (value: number) => void;
    commitPreviewZoom: (value: number) => void | Promise<void>;
    resetPreviewZoom: () => void;
    previewExecutionMode?: "design" | "motion" | "interactive";
    interactivePreviewUrl?: string;
    setPreviewExecutionMode: (mode: "design" | "motion" | "interactive") => void;
  } = $props();

  const presets = $derived<Array<{
    id: Exclude<WorkbenchCanvasPreset, "custom">;
    label: string;
    widthPx: number;
  }>>([
    { id: "desktop", label: `${t("workbench-canvas-desktop")} 1440`, widthPx: 1_440 },
    { id: "tablet", label: `${t("workbench-canvas-tablet")} 768`, widthPx: 768 },
    { id: "mobile", label: `${t("workbench-canvas-mobile")} 390`, widthPx: 390 },
  ]);

  const activeBreakpoint = $derived.by(() => {
    if (viewport.mode === "fit") return t("workbench-canvas-fluid-width");
    const sorted = [...breakpoints].sort((left, right) => left.widthPx - right.widthPx);
    return sorted.find((breakpoint) => viewport.widthPx <= breakpoint.widthPx)?.label
      ?? t("workbench-canvas-over-breakpoints");
  });

  function applyPreset(preset: (typeof presets)[number]) {
    void setViewport({
      mode: "fixed",
      preset: preset.id,
      widthPx: preset.widthPx,
    });
  }

  function commitWidth(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const widthPx = Math.min(3_840, Math.max(320, Number(input.value) || viewport.widthPx));
    input.value = String(Math.round(widthPx));
    void setViewport({ mode: "fixed", preset: "custom", widthPx });
  }

</script>

<div class="canvas-toolbar" aria-label={t("workbench-canvas-controls")}>
  <div class="toolbar-controls">
    <div class="segmented" role="group" aria-label={t("workbench-viewport-preset")}>
      <button
        type="button"
        class="ui-button compact"
        class:active={viewport.mode === "fit"}
        aria-pressed={viewport.mode === "fit" ? "true" : "false"}
        aria-label={t("workbench-canvas-fit-title")}
        title={t("workbench-canvas-fit-title")}
        onclick={() => { void setViewport({ mode: "fit", zoomPercent: 100 }); }}
      >
        <IconArrowsMaximize size={14} stroke={1.8} />
        <span>{t("workbench-canvas-fit")}</span>
      </button>
      {#each presets as preset (preset.id)}
        <button
          type="button"
          class="ui-button compact"
          class:active={viewport.mode === "fixed" && viewport.preset === preset.id}
          aria-pressed={viewport.mode === "fixed" && viewport.preset === preset.id ? "true" : "false"}
          aria-label={preset.label}
          title={preset.label}
          onclick={() => { applyPreset(preset); }}
        >
          {#if preset.id === "desktop"}
            <IconDeviceDesktop size={14} stroke={1.8} />
          {:else if preset.id === "tablet"}
            <IconDeviceTablet size={14} stroke={1.8} />
          {:else}
            <IconDeviceMobile size={14} stroke={1.8} />
          {/if}
          <span>{preset.label.split(" ")[0]}</span>
        </button>
      {/each}
    </div>

    <label class="ui-field compact width-field" title={t("workbench-viewport-exact-width")}>
      <span>L</span>
      <input
        type="number"
        min="320"
        max="3840"
        step="1"
        value={viewport.widthPx}
        disabled={viewport.mode === "fit"}
        aria-label={t("workbench-viewport-width-pixels")}
        onchange={commitWidth}
        onkeydown={(event) => {
          if (event.key === "Enter") commitWidth(event);
        }}
      />
      <small>px</small>
    </label>

    <PreviewZoomControl
      previewZoom={viewport.zoomPercent}
      disabled={viewport.mode === "fit"}
      {setPreviewZoom}
      {commitPreviewZoom}
      {resetPreviewZoom}
    />

    <button
      type="button"
      class="ui-icon-button compact ruler-toggle"
      class:active={viewport.showRulers}
      aria-pressed={viewport.showRulers ? "true" : "false"}
      title={t("workbench-rulers-title")}
      aria-label={t("workbench-rulers-toggle")}
      onclick={() => { void setViewport({ showRulers: !viewport.showRulers }); }}
    >
      <IconRuler2 size={14} stroke={1.8} />
    </button>

    <div class="breakpoint-summary" title={t("workbench-active-breakpoint")}>
      <span>{activeBreakpoint}</span>
      {#if viewport.mode === "fixed"}
        {#each [...breakpoints].sort((left, right) => left.widthPx - right.widthPx) as breakpoint (breakpoint.id)}
          <small class:active={viewport.widthPx <= breakpoint.widthPx}>
            {breakpoint.label} ≤{breakpoint.widthPx}
          </small>
        {/each}
      {/if}
    </div>
  </div>

  <div class="preview-mode segmented" role="group" aria-label={t("workbench-preview-mode")}>
    <button
      type="button"
      class="ui-button compact"
      class:active={previewExecutionMode === "design"}
      aria-pressed={previewExecutionMode === "design"}
      onclick={() => setPreviewExecutionMode("design")}
    ><IconBrush size={13} /> {t("workbench-preview-design")}</button>
    <button
      type="button"
      class="ui-button compact"
      class:active={previewExecutionMode === "motion"}
      aria-pressed={previewExecutionMode === "motion"}
      disabled={!interactivePreviewUrl}
      title={t("workbench-preview-motion-title")}
      onclick={() => setPreviewExecutionMode("motion")}
    ><IconSparkles size={13} /> {t("workbench-preview-motion")}</button>
    <button
      type="button"
      class="ui-button compact"
      class:active={previewExecutionMode === "interactive"}
      aria-pressed={previewExecutionMode === "interactive"}
      disabled={!interactivePreviewUrl}
      title={t("workbench-preview-interactive-title")}
      onclick={() => setPreviewExecutionMode("interactive")}
    ><IconPointer size={13} /> {t("workbench-preview-interactive")}</button>
  </div>
</div>

<style>
  .canvas-toolbar {
    position: relative;
    z-index: 6;
    display: flex;
    align-items: center;
    justify-content: space-between;
    container-type: inline-size;
    gap: 12px;
    min-height: 36px;
    padding: 3px 7px;
    border-top: 1px solid var(--wb-border-subtle, var(--border-2));
    color: var(--wb-text-muted, var(--text-muted));
    background: var(--surface-panel);
    font-size: var(--font-meta);
  }

  .preview-mode {
    flex: 0 0 auto;
  }

  .toolbar-controls,
  .segmented,
  .width-field,
  .breakpoint-summary {
    display: flex;
    align-items: center;
  }

  .toolbar-controls {
    justify-content: flex-end;
    gap: 6px;
    min-width: 0;
  }

  button,
  input {
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    color: var(--wb-text-muted, var(--text-muted));
    background: var(--wb-surface-document, var(--surface));
    font: inherit;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 0 7px;
  }

  button:hover:not(:disabled),
  button.active {
    color: var(--wb-accent-strong, var(--brand-strong));
    background: var(--control-selected);
  }

  button:disabled {
    opacity: 0.42;
    cursor: not-allowed;
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--wb-focus-ring, var(--brand-strong));
    outline-offset: 1px;
  }

  .segmented {
    gap: 0;
  }

  .segmented button {
    border-right-width: 0;
    border-radius: 0;
  }

  .segmented button:first-child {
    border-radius: var(--radius-control) 0 0 var(--radius-control);
  }

  .segmented button:last-child {
    border-right-width: 1px;
    border-radius: 0 var(--radius-control) var(--radius-control) 0;
  }

  .width-field {
    gap: 4px;
    padding-left: 6px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: var(--radius-control);
    background: var(--wb-surface-document, var(--surface));
  }

  .width-field input {
    width: 54px;
    height: 100%;
    min-height: 0;
    padding: 0 3px;
    border: 0;
    color: var(--wb-text-primary, var(--text));
    background: transparent;
    font-variant-numeric: tabular-nums;
  }

  .width-field small {
    padding-right: 6px;
    color: var(--wb-text-muted, var(--text-muted));
  }

  .width-field:has(input:disabled) {
    opacity: 0.45;
  }

  .ruler-toggle {
    padding: 0;
    border-radius: var(--radius-control);
  }

  .breakpoint-summary {
    gap: 4px;
    max-width: 260px;
    overflow: hidden;
  }

  .breakpoint-summary > span,
  .breakpoint-summary small {
    min-height: 20px;
    padding: 0 6px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: 999px;
    line-height: 18px;
    white-space: nowrap;
  }

  .breakpoint-summary > span {
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-document, var(--surface));
  }

  .breakpoint-summary small {
    opacity: 0.55;
  }

  .breakpoint-summary small.active {
    border-color: color-mix(in srgb, var(--wb-accent, var(--brand)) 48%, var(--wb-border-subtle));
    color: var(--wb-accent-strong, var(--brand-strong));
    opacity: 1;
  }

  @container (max-width: 900px) {
    .breakpoint-summary small,
    .segmented button span {
      display: none;
    }
  }
</style>
