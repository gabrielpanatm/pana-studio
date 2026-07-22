<script lang="ts">
  import { ANIME_DIRECTIONS, ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import type { PanaMotionExpression, PanaMotionPlayback, PanaMotionWaapiItem } from "$lib/types";
  import MotionPropertyList from "$lib/components/inspector/js/MotionPropertyList.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

  let {
    waapi,
    onChange,
  }: {
    waapi: PanaMotionWaapiItem;
    onChange: (item: PanaMotionWaapiItem) => void;
  } = $props();

  function patch(patchValue: Partial<PanaMotionWaapiItem>) {
    onChange({ ...waapi, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...waapi.playback, ...patchValue } });
  }

  function patchFinished(patchValue: Partial<PanaMotionExpression>) {
    patch({ finished: { ...waapi.finished, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];
</script>

<div class="waapi-editor">
  <section class="editor-card">
    <div class="section-head"><span>WAAPI</span></div>
    <div class="field-grid">
      <label><span>Delay</span><input type="number" value={waapi.playback.delay} oninput={(event) => patchPlayback({ delay: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Durată</span><input type="number" value={waapi.playback.duration} oninput={(event) => patchPlayback({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Iterations</span><input type="number" value={waapi.iterations} oninput={(event) => patch({ iterations: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Direction</span>
        <SelectControl value={waapi.direction} options={ANIME_DIRECTIONS} ariaLabel="WAAPI direction" onchange={(value) => patch({ direction: value })} />
      </label>
      <label>
        <span>Easing</span>
        <SelectControl value={waapi.easing} options={easingOptions} ariaLabel="WAAPI easing" onchange={(value) => patch({ easing: value })} />
      </label>
      <label><span>Playback rate</span><input type="number" step="0.1" value={waapi.playback.playbackRate} oninput={(event) => patchPlayback({ playbackRate: Number(event.currentTarget.value) || 1 })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={waapi.autoplay} onclick={() => patch({ autoplay: !waapi.autoplay })}>autoplay</button>
      <button type="button" class:active={waapi.hardwareAcceleration} onclick={() => patch({ hardwareAcceleration: !waapi.hardwareAcceleration })}>hardware</button>
      <button type="button" class:active={waapi.convertEase} onclick={() => patch({ convertEase: !waapi.convertEase })}>convertEase</button>
      <button type="button" class:active={waapi.playback.alternate} onclick={() => patchPlayback({ alternate: !waapi.playback.alternate })}>alternate</button>
      <button type="button" class:active={waapi.playback.persist} onclick={() => patchPlayback({ persist: !waapi.playback.persist })}>persist</button>
    </div>
  </section>

  <MotionPropertyList
    title="Proprietăți WAAPI"
    properties={waapi.properties}
    onChange={(properties) => patch({ properties })}
  />

  <section class="editor-card">
    <div class="section-head"><span>finished</span></div>
    <div class="toggle-grid two">
      <button type="button" class:active={waapi.finished.enabled} onclick={() => patchFinished({ enabled: !waapi.finished.enabled })}>then()</button>
    </div>
    {#if waapi.finished.enabled}
      <label>
        <span>Cod</span>
        <textarea value={waapi.finished.code} placeholder="(animation, anime, utils) => &#123; ... &#125;" oninput={(event) => patchFinished({ code: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </section>
</div>

<style>
  .waapi-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .waapi-editor {
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

  .toggle-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 5px;
  }

  .toggle-grid.two {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  input,
  textarea,
  button {
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
  }

  input,
  button {
    min-height: 25px;
  }

  input,
  textarea {
    width: 100%;
    min-width: 0;
    padding: 0 6px;
  }

  textarea {
    min-height: 58px;
    padding-block: 6px;
    resize: vertical;
  }

  button {
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }
</style>
