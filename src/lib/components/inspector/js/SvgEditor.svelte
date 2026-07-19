<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionPlayback, PanaMotionSvgItem } from "$lib/types";

  let {
    svg,
    onChange,
  }: {
    svg: PanaMotionSvgItem;
    onChange: (item: PanaMotionSvgItem) => void;
  } = $props();

  const callbackPlaceholder = "(self, anime, utils) => { ... }";
  const modeOptions: PanaMotionSvgItem["mode"][] = ["morphTo", "createDrawable", "createMotionPath"];
  const attributeOptions: PanaMotionSvgItem["attribute"][] = ["d", "points"];
  const yesNoOptions = [
    { value: "yes", label: "da" },
    { value: "no", label: "nu" },
  ];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];

  function patch(patchValue: Partial<PanaMotionSvgItem>) {
    onChange({ ...svg, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...svg.playback, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function ratioValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, Math.min(1, parsed)) : 0;
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...svg.callbacks,
        [name]: { ...(svg.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...svg.callbacks,
        [name]: { ...(svg.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }
</script>

<div class="svg-editor">
  <section class="editor-card">
    <div class="section-head">
      <span>SVG Utility</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Mode</span>
        <SelectControl value={svg.mode} options={modeOptions} ariaLabel="SVG mode" onchange={(value) => patch({ mode: value as PanaMotionSvgItem["mode"] })} />
      </label>
      <label>
        <span>{svg.mode === "createMotionPath" ? "Path selector" : svg.mode === "morphTo" ? "Shape target" : "Drawable target"}</span>
        <input class="mono" value={svg.source || svg.path} placeholder=".path-final sau path" oninput={(event) => patch({ source: event.currentTarget.value, path: event.currentTarget.value })} />
      </label>
    </div>
    {#if svg.mode === "morphTo"}
      <div class="field-grid">
        <label>
          <span>Attribute</span>
          <SelectControl value={svg.attribute} options={attributeOptions} ariaLabel="SVG attribute" onchange={(value) => patch({ attribute: value as PanaMotionSvgItem["attribute"] })} />
        </label>
        <label><span>Precision</span><input type="number" min="0" max="1" step="0.01" value={svg.precision} oninput={(event) => patch({ precision: ratioValue(event.currentTarget.value) })} /></label>
      </div>
    {:else if svg.mode === "createMotionPath"}
      <label><span>Offset</span><input type="number" min="0" max="1" step="0.01" value={svg.offset} oninput={(event) => patch({ offset: ratioValue(event.currentTarget.value) })} /></label>
    {:else}
      <label><span>Draw</span><input class="mono" value={svg.draw} placeholder="0 1 sau 0 0, 0 1, 1 1" oninput={(event) => patch({ draw: event.currentTarget.value })} /></label>
    {/if}
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Playback</span>
    </div>
    <div class="field-grid">
      <label><span>Autoplay</span><SelectControl value={svg.playback.autoplay ? "yes" : "no"} options={yesNoOptions} ariaLabel="SVG autoplay" onchange={(value) => patchPlayback({ autoplay: value === "yes" })} /></label>
      <label><span>Delay</span><input type="number" min="0" step="50" value={svg.playback.delay} oninput={(event) => patchPlayback({ delay: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Duration</span><input type="number" min="0" step="50" value={svg.playback.duration} oninput={(event) => patchPlayback({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Ease</span>
        <SelectControl value={svg.playback.playbackEase} options={easingOptions} ariaLabel="SVG ease" onchange={(value) => patchPlayback({ playbackEase: value })} />
      </label>
      <label><span>Loop</span><input type="number" value={svg.playback.loop} placeholder="-1 infinit" oninput={(event) => patchPlayback({ loop: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Loop delay</span><input type="number" min="0" step="50" value={svg.playback.loopDelay} oninput={(event) => patchPlayback({ loopDelay: numberValue(event.currentTarget.value) })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={svg.playback.alternate} onclick={() => patchPlayback({ alternate: !svg.playback.alternate })}>alternate</button>
      <button type="button" class:active={svg.playback.reversed} onclick={() => patchPlayback({ reversed: !svg.playback.reversed })}>reversed</button>
      <button type="button" class:active={svg.playback.persist} onclick={() => patchPlayback({ persist: !svg.playback.persist })}>persist</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Callbacks</span>
    </div>
    {#each Object.entries(svg.callbacks ?? {}) as [name, callback]}
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
  .svg-editor {
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
</style>
