<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionEasingItem } from "$lib/types";

  let {
    easing,
    onChange,
  }: {
    easing: PanaMotionEasingItem;
    onChange: (item: PanaMotionEasingItem) => void;
  } = $props();

  function patch(patchValue: Partial<PanaMotionEasingItem>) {
    onChange({ ...easing, ...patchValue });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function placeholder(mode: PanaMotionEasingItem["mode"]): string {
    if (mode === "cubicBezier") return ".7, .1, .5, .9";
    if (mode === "linear") return "0, 0, .5, .5, 1, 1";
    if (mode === "steps") return "4, end";
    if (mode === "irregular") return "10, .5";
    if (mode === "spring") return "{ bounce: .35 }";
    if (mode === "custom") return "(anime) => anime.easings.outQuad";
    return "outExpo";
  }

  const modeOptions: PanaMotionEasingItem["mode"][] = ["builtIn", "cubicBezier", "linear", "steps", "irregular", "spring", "custom"];
</script>

<div class="easing-editor">
  <section class="editor-card">
    <div class="section-head"><span>Easing</span></div>
    <div class="field-grid">
      <label>
        <span>Mode</span>
        <SelectControl value={easing.mode} options={modeOptions.map((mode) => ({ value: mode, label: mode === "builtIn" ? "built-in" : mode }))} ariaLabel="Easing mode" onchange={(value) => patch({ mode: value as PanaMotionEasingItem["mode"] })} />
      </label>
      <label><span>Preview duration</span><input type="number" value={easing.previewDuration} oninput={(event) => patch({ previewDuration: numberValue(event.currentTarget.value) })} /></label>
    </div>

    {#if easing.mode === "builtIn"}
      <label>
        <span>Value</span>
        <SelectControl value={easing.value} options={ANIME_EASING_OPTIONS} ariaLabel="Easing value" onchange={(value) => patch({ value })} />
      </label>
    {:else}
      <label>
        <span>Value / args</span>
        <textarea class="mono" value={easing.value} placeholder={placeholder(easing.mode)} oninput={(event) => patch({ value: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </section>
</div>

<style>
  .easing-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .easing-editor {
    gap: 8px;
  }

  .editor-card {
    gap: 7px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .section-head span,
  label span {
    display: block;
    font-size: 9px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  textarea {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 11px;
    padding: 0 6px;
  }

  input {
    min-height: 25px;
  }

  textarea {
    min-height: 58px;
    padding-block: 6px;
    resize: vertical;
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
