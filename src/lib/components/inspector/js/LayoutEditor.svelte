<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionLayoutItem, PanaMotionPlayback } from "$lib/types";

  let {
    layout,
    onChange,
  }: {
    layout: PanaMotionLayoutItem;
    onChange: (item: PanaMotionLayoutItem) => void;
  } = $props();

  const callbackPlaceholder = "(layout, anime, utils) => { ... }";
  const modeOptions: PanaMotionLayoutItem["mode"][] = ["record", "animate", "update", "revert"];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];

  function patch(patchValue: Partial<PanaMotionLayoutItem>) {
    onChange({ ...layout, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...layout.playback, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...layout.callbacks,
        [name]: { ...(layout.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...layout.callbacks,
        [name]: { ...(layout.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }
</script>

<div class="layout-editor">
  <section class="editor-card">
    <div class="section-head"><span>Layout mode</span></div>
    <div class="field-grid">
      <label>
        <span>Mode</span>
        <SelectControl value={layout.mode} options={modeOptions} ariaLabel="Layout mode" onchange={(value) => patch({ mode: value as PanaMotionLayoutItem["mode"] })} />
      </label>
      <label><span>Children selector</span><input class="mono" value={layout.children} placeholder=":scope > *" oninput={(event) => patch({ children: event.currentTarget.value })} /></label>
      <label><span>Delay</span><input type="number" value={layout.playback.delay} oninput={(event) => patchPlayback({ delay: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Duration</span><input type="number" value={layout.playback.duration} oninput={(event) => patchPlayback({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Ease</span>
        <SelectControl value={layout.playback.playbackEase} options={easingOptions} ariaLabel="Layout ease" onchange={(value) => patchPlayback({ playbackEase: value })} />
      </label>
      <label><span>Swap at</span><input class="mono" value={layout.swapAt} placeholder="50%" oninput={(event) => patch({ swapAt: event.currentTarget.value })} /></label>
    </div>
    <div class="toggle-grid">
      {#each [
        ["includeDisplay", "display"],
        ["includeGrid", "grid"],
        ["includeFlex", "flex"],
        ["includeOrder", "order"],
        ["enterExit", "enter/exit"],
        ["swapParent", "swap parent"],
      ] as toggle}
        <button
          type="button"
          class:active={layout[toggle[0] as keyof PanaMotionLayoutItem]}
          onclick={() => patch({ [toggle[0]]: !layout[toggle[0] as keyof PanaMotionLayoutItem] } as Partial<PanaMotionLayoutItem>)}
        >{toggle[1]}</button>
      {/each}
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head"><span>States</span></div>
    <label>
      <span>Properties</span>
      <textarea class="compact-code" value={layout.properties} placeholder="transform, opacity, width" oninput={(event) => patch({ properties: event.currentTarget.value })}></textarea>
    </label>
    <label>
      <span>Enter from</span>
      <textarea class="compact-code" value={layout.enterFrom} placeholder="opacity: 0; transform: translateY(24px);" oninput={(event) => patch({ enterFrom: event.currentTarget.value })}></textarea>
    </label>
    <label>
      <span>Leave to</span>
      <textarea class="compact-code" value={layout.leaveTo} placeholder="opacity: 0;" oninput={(event) => patch({ leaveTo: event.currentTarget.value })}></textarea>
    </label>
  </section>

  <section class="editor-card">
    <div class="section-head"><span>Callbacks</span></div>
    {#each Object.entries(layout.callbacks ?? {}) as [name, callback]}
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
  .layout-editor,
  .editor-card,
  .callback-row {
    display: flex;
    flex-direction: column;
  }

  .layout-editor {
    gap: 8px;
  }

  .editor-card {
    gap: 7px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .callback-row {
    gap: 5px;
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
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 5px;
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

  input,
  .toggle-grid button,
  .callback-row button {
    min-height: 25px;
  }

  textarea {
    min-height: 58px;
    padding-block: 6px;
    resize: vertical;
  }

  textarea.compact-code {
    min-height: 44px;
    font-family: "JetBrains Mono", monospace;
  }

  .toggle-grid button,
  .callback-row button {
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

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
