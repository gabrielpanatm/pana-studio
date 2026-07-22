<script lang="ts">
  import {
    IconArrowsMaximize,
    IconDeviceDesktop,
    IconDeviceMobile,
    IconDeviceTablet,
    IconMinus,
    IconPlus,
    IconRuler2,
  } from "@tabler/icons-svelte";
  import type {
    WorkbenchCanvasPreset,
    WorkbenchCanvasViewportSnapshot,
  } from "$lib/types";

  let {
    viewport,
    documentPath = "",
    breakpoints = [],
    setViewport,
  }: {
    viewport: WorkbenchCanvasViewportSnapshot;
    documentPath?: string;
    breakpoints?: Array<{ id: string; label: string; widthPx: number }>;
    setViewport: (viewport: Partial<WorkbenchCanvasViewportSnapshot>) => void | Promise<void>;
  } = $props();

  const presets: Array<{
    id: Exclude<WorkbenchCanvasPreset, "custom">;
    label: string;
    widthPx: number;
  }> = [
    { id: "desktop", label: "Desktop 1440", widthPx: 1_440 },
    { id: "tablet", label: "Tabletă 768", widthPx: 768 },
    { id: "mobile", label: "Telefon 390", widthPx: 390 },
  ];

  const activeBreakpoint = $derived.by(() => {
    if (viewport.mode === "fit") return "Lățime fluidă";
    const sorted = [...breakpoints].sort((left, right) => left.widthPx - right.widthPx);
    return sorted.find((breakpoint) => viewport.widthPx <= breakpoint.widthPx)?.label
      ?? "Peste breakpointuri";
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

  function changeZoom(delta: number) {
    if (viewport.mode === "fit") return;
    const zoomPercent = Math.min(200, Math.max(25, viewport.zoomPercent + delta));
    void setViewport({ zoomPercent });
  }
</script>

<div class="canvas-toolbar" aria-label="Canvas responsive">
  <div class="surface-copy">
    <strong>Vizual</strong>
    <span title={documentPath}>{documentPath}</span>
  </div>

  <div class="toolbar-controls">
    <div class="segmented" role="group" aria-label="Preset viewport">
      <button
        type="button"
        class:active={viewport.mode === "fit"}
        aria-pressed={viewport.mode === "fit" ? "true" : "false"}
        aria-label="Potrivește canvas-ul în spațiul disponibil"
        title="Potrivește canvas-ul în spațiul disponibil"
        onclick={() => { void setViewport({ mode: "fit", zoomPercent: 100 }); }}
      >
        <IconArrowsMaximize size={14} stroke={1.8} />
        <span>Fit</span>
      </button>
      {#each presets as preset (preset.id)}
        <button
          type="button"
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

    <label class="width-field" title="Lățime exactă viewport">
      <span>L</span>
      <input
        type="number"
        min="320"
        max="3840"
        step="1"
        value={viewport.widthPx}
        disabled={viewport.mode === "fit"}
        aria-label="Lățime viewport în pixeli"
        onchange={commitWidth}
        onkeydown={(event) => {
          if (event.key === "Enter") commitWidth(event);
        }}
      />
      <small>px</small>
    </label>

    <div class="zoom-control" role="group" aria-label="Zoom canvas">
      <button type="button" disabled={viewport.mode === "fit"} aria-label="Micșorează zoom" onclick={() => { changeZoom(-25); }}>
        <IconMinus size={13} stroke={2} />
      </button>
      <span>{viewport.zoomPercent}%</span>
      <button type="button" disabled={viewport.mode === "fit"} aria-label="Mărește zoom" onclick={() => { changeZoom(25); }}>
        <IconPlus size={13} stroke={2} />
      </button>
    </div>

    <button
      type="button"
      class:active={viewport.showRulers}
      class="ruler-toggle"
      aria-pressed={viewport.showRulers ? "true" : "false"}
      title="Arată sau ascunde rigla și breakpointurile"
      aria-label="Comută rigla canvas-ului"
      onclick={() => { void setViewport({ showRulers: !viewport.showRulers }); }}
    >
      <IconRuler2 size={14} stroke={1.8} />
    </button>

    <div class="breakpoint-summary" title="Breakpoint activ pentru lățimea curentă">
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
</div>

<style>
  .canvas-toolbar {
    position: relative;
    z-index: 6;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 36px;
    padding: 3px 7px 3px 10px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border-2));
    color: var(--wb-text-muted, var(--text-muted));
    background: var(--wb-surface-chrome, var(--surface-2));
    font-size: 12px;
  }

  .surface-copy,
  .toolbar-controls,
  .segmented,
  .width-field,
  .zoom-control,
  .breakpoint-summary {
    display: flex;
    align-items: center;
  }

  .surface-copy {
    gap: 8px;
    min-width: 70px;
    overflow: hidden;
  }

  .surface-copy strong {
    color: var(--wb-accent-strong, var(--brand-strong));
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .surface-copy span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar-controls {
    justify-content: flex-end;
    gap: 6px;
    min-width: 0;
  }

  button,
  input {
    height: 26px;
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
    background: var(--wb-accent-soft, var(--brand-soft));
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
  }

  .segmented button:first-child {
    border-radius: 6px 0 0 6px;
  }

  .segmented button:last-child {
    border-right-width: 1px;
    border-radius: 0 6px 6px 0;
  }

  .width-field {
    gap: 4px;
    height: 26px;
    padding-left: 6px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: 6px;
    background: var(--wb-surface-document, var(--surface));
  }

  .width-field input {
    width: 54px;
    height: 22px;
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

  .zoom-control {
    gap: 0;
  }

  .zoom-control button {
    width: 26px;
    padding: 0;
  }

  .zoom-control button:first-child {
    border-radius: 6px 0 0 6px;
  }

  .zoom-control button:last-child {
    border-radius: 0 6px 6px 0;
  }

  .zoom-control span {
    display: grid;
    min-width: 42px;
    height: 26px;
    border-block: 1px solid var(--wb-border-subtle, var(--border-3));
    background: var(--wb-surface-document, var(--surface));
    place-items: center;
    color: var(--wb-text-primary, var(--text));
    font-variant-numeric: tabular-nums;
  }

  .ruler-toggle {
    width: 28px;
    padding: 0;
    border-radius: 6px;
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

  @media (max-width: 1320px) {
    .breakpoint-summary small,
    .segmented button span,
    .surface-copy span {
      display: none;
    }
  }
</style>
