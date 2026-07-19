<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionScrollItem } from "$lib/types";

  let {
    scroll,
    onChange,
  }: {
    scroll: PanaMotionScrollItem;
    onChange: (item: PanaMotionScrollItem) => void;
  } = $props();

  const syncModes: PanaMotionScrollItem["syncMode"][] = ["methods", "progress", "smooth", "eased"];
  const axisOptions: PanaMotionScrollItem["axis"][] = ["y", "x"];
  const easingOptions = [{ value: "", label: "alege ease" }, ...ANIME_EASING_OPTIONS];
  const callbackPlaceholder = "(observer, anime, utils) => { ... }";

  function patch(patchValue: Partial<PanaMotionScrollItem>) {
    onChange({ ...scroll, ...patchValue });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, Math.min(1, parsed)) : 0;
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...scroll.callbacks,
        [name]: { ...(scroll.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...scroll.callbacks,
        [name]: {
          ...(scroll.callbacks[name] ?? emptyExpression(name)),
          code,
          enabled: code.trim().length > 0,
        },
      },
    });
  }

  function updateSyncMode(syncMode: PanaMotionScrollItem["syncMode"]) {
    patch({
      syncMode,
      sync: syncMode === "progress" ? "progress" : syncMode === "smooth" ? "smooth" : syncMode === "eased" ? "eased" : "play",
    });
  }
</script>

<div class="scroll-editor">
  <section class="editor-card">
    <div class="section-head">
      <span>Observer</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Container</span>
        <input class="mono" value={scroll.container} placeholder="window sau .scroll-container" oninput={(event) => patch({ container: event.currentTarget.value })} />
      </label>
      <label>
        <span>Axis</span>
        <SelectControl value={scroll.axis} options={axisOptions} ariaLabel="Axa scroll" onchange={(value) => patch({ axis: value as PanaMotionScrollItem["axis"] })} />
      </label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={scroll.repeat} onclick={() => patch({ repeat: !scroll.repeat })}>repeat</button>
      <button type="button" class:active={scroll.debug} onclick={() => patch({ debug: !scroll.debug })}>debug</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Thresholds</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Enter</span>
        <input class="mono" value={scroll.enter} placeholder="bottom top" oninput={(event) => patch({ enter: event.currentTarget.value, threshold: event.currentTarget.value })} />
      </label>
      <label>
        <span>Leave</span>
        <input class="mono" value={scroll.leave} placeholder="top bottom" oninput={(event) => patch({ leave: event.currentTarget.value })} />
      </label>
    </div>
    <p class="hint">Acceptă poziții, valori relative, min/max sau shorthand Anime.js.</p>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Sync</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Mode</span>
        <SelectControl value={scroll.syncMode} options={syncModes} ariaLabel="Scroll sync mode" onchange={(value) => updateSyncMode(value as PanaMotionScrollItem["syncMode"])} />
      </label>
      {#if scroll.syncMode === "methods"}
        <label>
          <span>Methods</span>
          <input class="mono" value={scroll.syncMethods} placeholder="play pause / resume pause reverse reset" oninput={(event) => patch({ syncMethods: event.currentTarget.value })} />
        </label>
      {:else if scroll.syncMode === "smooth"}
        <label>
          <span>Smooth</span>
          <input type="number" min="0" max="1" step="0.05" value={scroll.smooth} oninput={(event) => patch({ smooth: numberValue(event.currentTarget.value) })} />
        </label>
      {:else if scroll.syncMode === "eased"}
        <label>
          <span>Ease</span>
          <SelectControl value={scroll.syncEase} options={easingOptions} ariaLabel="Scroll sync ease" onchange={(value) => patch({ syncEase: value })} />
        </label>
      {:else}
        <p class="span-2 hint">Playback progress: `sync: true`.</p>
      {/if}
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Callbacks</span>
    </div>
    {#each Object.entries(scroll.callbacks ?? {}) as [name, callback]}
      <div class="callback-row">
        <button type="button" class:active={(callback as PanaMotionExpression).enabled} onclick={() => toggleCallback(name, !(callback as PanaMotionExpression).enabled)}>
          {name}
        </button>
        {#if (callback as PanaMotionExpression).enabled}
          <textarea
            value={(callback as PanaMotionExpression).code}
            placeholder={callbackPlaceholder}
            oninput={(event) => updateCallback(name, event.currentTarget.value)}
          ></textarea>
        {/if}
      </div>
    {/each}
  </section>
</div>

<style>
  .scroll-editor {
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

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
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

  .span-2 {
    grid-column: 1 / -1;
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
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  .toggle-grid button,
  .callback-row button {
    min-height: 25px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }

  .toggle-grid button.active,
  .callback-row button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .callback-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.35;
  }
</style>
