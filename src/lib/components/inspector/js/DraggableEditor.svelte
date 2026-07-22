<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionDraggableItem, PanaMotionExpression } from "$lib/types";

  let {
    draggable,
    onChange,
  }: {
    draggable: PanaMotionDraggableItem;
    onChange: (item: PanaMotionDraggableItem) => void;
  } = $props();

  const callbackPlaceholder = "(draggable, anime, utils) => { ... }";
  const axesOptions: PanaMotionDraggableItem["axes"][] = ["both", "x", "y"];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];

  function patch(patchValue: Partial<PanaMotionDraggableItem>) {
    onChange({ ...draggable, ...patchValue });
  }

  function patchRelease(patchValue: Partial<PanaMotionDraggableItem["release"]>) {
    patch({ release: { ...draggable.release, ...patchValue } });
  }

  function patchModifier(patchValue: Partial<PanaMotionExpression>) {
    patch({ modifier: { ...draggable.modifier, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...draggable.callbacks,
        [name]: { ...(draggable.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...draggable.callbacks,
        [name]: { ...(draggable.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }
</script>

<div class="draggable-editor">
  <section class="editor-card">
    <div class="section-head">
      <span>Axes</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Axe</span>
        <SelectControl value={draggable.axes} options={axesOptions} ariaLabel="Axe draggable" onchange={(value) => patch({ axes: value as PanaMotionDraggableItem["axes"] })} />
      </label>
      <label><span>Snap global</span><input class="mono" value={draggable.snap} placeholder="100 sau [0,100,200]" oninput={(event) => patch({ snap: event.currentTarget.value })} /></label>
      <label><span>Snap X</span><input class="mono" value={draggable.snapX} oninput={(event) => patch({ snapX: event.currentTarget.value })} /></label>
      <label><span>Snap Y</span><input class="mono" value={draggable.snapY} oninput={(event) => patch({ snapY: event.currentTarget.value })} /></label>
      <label><span>Map to</span><input class="mono" value={draggable.mapTo} placeholder="translate / custom prop" oninput={(event) => patch({ mapTo: event.currentTarget.value })} /></label>
    </div>
    <div class="toggle-grid two">
      <button type="button" class:active={draggable.modifier.enabled} onclick={() => patchModifier({ enabled: !draggable.modifier.enabled })}>modifier</button>
    </div>
    {#if draggable.modifier.enabled}
      <label>
        <span>Modifier</span>
        <textarea value={draggable.modifier.code} placeholder="(value, axis, draggable) => value" oninput={(event) => patchModifier({ code: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Setări</span>
    </div>
    <div class="field-grid">
      <label><span>Container</span><input class="mono" value={draggable.container} oninput={(event) => patch({ container: event.currentTarget.value })} /></label>
      <label><span>Trigger</span><input class="mono" value={draggable.trigger} oninput={(event) => patch({ trigger: event.currentTarget.value })} /></label>
      <label><span>Container padding</span><input type="number" value={draggable.containerPadding} oninput={(event) => patch({ containerPadding: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Container friction</span><input type="number" step="0.05" value={draggable.friction} oninput={(event) => patch({ friction: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Frecare container la eliberare</span><input type="number" step="0.05" value={draggable.releaseContainerFriction} oninput={(event) => patch({ releaseContainerFriction: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Velocity multiplier</span><input type="number" step="0.1" value={draggable.velocity} oninput={(event) => patch({ velocity: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Min velocity</span><input type="number" value={draggable.minVelocity} oninput={(event) => patch({ minVelocity: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Max velocity</span><input type="number" value={draggable.maxVelocity} oninput={(event) => patch({ maxVelocity: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Drag speed</span><input type="number" step="0.1" value={draggable.dragSpeed} oninput={(event) => patch({ dragSpeed: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Drag threshold</span><input type="number" value={draggable.dragThreshold} oninput={(event) => patch({ dragThreshold: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Scroll threshold</span><input type="number" value={draggable.scrollThreshold} oninput={(event) => patch({ scrollThreshold: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Scroll speed</span><input type="number" step="0.1" value={draggable.scrollSpeed} oninput={(event) => patch({ scrollSpeed: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Easing la eliberare</span>
        <SelectControl value={draggable.releaseEase} options={easingOptions} ariaLabel="Easing la eliberare" onchange={(value) => patch({ releaseEase: value })} />
      </label>
    </div>
    <div class="toggle-grid two">
      <button type="button" class:active={draggable.cursor} onclick={() => patch({ cursor: !draggable.cursor })}>cursor</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Eliberare</span>
    </div>
    <div class="field-grid">
      <label><span>Mass</span><input type="number" step="0.1" value={draggable.release.mass} oninput={(event) => patchRelease({ mass: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Stiffness</span><input type="number" value={draggable.release.stiffness} oninput={(event) => patchRelease({ stiffness: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Damping</span><input type="number" value={draggable.release.damping} oninput={(event) => patchRelease({ damping: numberValue(event.currentTarget.value) })} /></label>
    </div>
    <div class="toggle-grid two">
      <button type="button" class:active={draggable.release.spring} onclick={() => patchRelease({ spring: !draggable.release.spring })}>spring</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Callbacks</span>
    </div>
    {#each Object.entries(draggable.callbacks ?? {}) as [name, callback]}
      <div class="callback-row">
        <button type="button" class:active={(callback as PanaMotionExpression).enabled} onclick={() => toggleCallback(name, !(callback as PanaMotionExpression).enabled)}>
          {name}
        </button>
        {#if (callback as PanaMotionExpression).enabled}
          <textarea value={(callback as PanaMotionExpression).code} placeholder={callbackPlaceholder} oninput={(event) => updateCallback(name, event.currentTarget.value)}></textarea>
        {/if}
      </div>
    {/each}
  </section>
</div>

<style>
  .draggable-editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .editor-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .section-head,
  .callback-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .section-head span,
  label span {
    display: block;
    font-size: 12px;
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
    font-size: 12px;
    padding: 0 6px;
  }

  input {
    height: 25px;
  }

  textarea {
    min-height: 58px;
    padding-block: 6px;
    resize: vertical;
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }

  .toggle-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 5px;
  }

  .toggle-grid.two {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .toggle-grid button,
  .callback-row button {
    min-height: 25px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .toggle-grid button.active,
  .callback-row button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }
</style>
