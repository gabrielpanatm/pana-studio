<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionEngineItem } from "$lib/types";

  let {
    engine,
    onChange,
  }: {
    engine: PanaMotionEngineItem;
    onChange: (item: PanaMotionEngineItem) => void;
  } = $props();

  function patch(patchValue: Partial<PanaMotionEngineItem>) {
    onChange({ ...engine, ...patchValue });
  }

  function numberValue(value: string, fallback = 0): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : fallback;
  }
</script>

<div class="engine-editor">
  <section class="editor-card">
    <div class="section-head"><span>Engine global</span></div>
    <div class="field-grid">
      <label>
        <span>Time unit</span>
        <SelectControl value={engine.timeUnit} options={["ms", "s"]} ariaLabel="Unitate timp engine" onchange={(value) => patch({ timeUnit: value as PanaMotionEngineItem["timeUnit"] })} />
      </label>
      <label><span>Speed</span><input type="number" step="0.1" value={engine.speed} oninput={(event) => patch({ speed: numberValue(event.currentTarget.value, 1) })} /></label>
      <label><span>FPS</span><input type="number" value={engine.fps} oninput={(event) => patch({ fps: numberValue(event.currentTarget.value, 120) })} /></label>
      <label><span>Precision</span><input type="number" value={engine.precision} oninput={(event) => patch({ precision: numberValue(event.currentTarget.value, 3) })} /></label>
      <label><span>Default priority</span><input type="number" value={engine.priority} oninput={(event) => patch({ priority: numberValue(event.currentTarget.value, 1) })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={engine.pauseOnDocumentHidden} onclick={() => patch({ pauseOnDocumentHidden: !engine.pauseOnDocumentHidden })}>pause hidden</button>
    </div>
  </section>
</div>

<style>
  .engine-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .engine-editor {
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

  .toggle-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  button {
    width: 100%;
    min-width: 0;
    min-height: 25px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 11px;
    padding: 0 6px;
  }

  button {
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }
</style>
