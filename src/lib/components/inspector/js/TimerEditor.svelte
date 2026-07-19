<script lang="ts">
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionPlayback, PanaMotionTimerItem } from "$lib/types";

  let {
    timer,
    onChange,
  }: {
    timer: PanaMotionTimerItem;
    onChange: (item: PanaMotionTimerItem) => void;
  } = $props();

  const callbackPlaceholder = "(timer, anime, utils) => { ... }";
  const yesNoOptions = [
    { value: "yes", label: "da" },
    { value: "no", label: "nu" },
  ];

  function patch(patchValue: Partial<PanaMotionTimerItem>) {
    onChange({ ...timer, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...timer.playback, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...timer.callbacks,
        [name]: { ...(timer.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...timer.callbacks,
        [name]: { ...(timer.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }
</script>

<div class="timer-editor">
  <section class="editor-card">
    <div class="section-head"><span>Playback</span></div>
    <div class="field-grid">
      <label><span>Delay</span><input type="number" value={timer.playback.delay} oninput={(event) => patchPlayback({ delay: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Duration</span><input type="number" value={timer.playback.duration} oninput={(event) => patchPlayback({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Loop</span><input type="number" value={timer.playback.loop} oninput={(event) => patchPlayback({ loop: Number(event.currentTarget.value) || 0 })} /></label>
      <label><span>Loop delay</span><input type="number" value={timer.playback.loopDelay} oninput={(event) => patchPlayback({ loopDelay: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Frame rate</span><input type="number" value={timer.playback.frameRate} oninput={(event) => patchPlayback({ frameRate: numberValue(event.currentTarget.value) })} /></label>
      <label><span>Playback rate</span><input type="number" step="0.1" value={timer.playback.playbackRate} oninput={(event) => patchPlayback({ playbackRate: Number(event.currentTarget.value) || 1 })} /></label>
      <label><span>Playback ease</span><input class="mono" value={timer.playback.playbackEase} oninput={(event) => patchPlayback({ playbackEase: event.currentTarget.value })} /></label>
      <label><span>Autoplay</span><SelectControl value={timer.playback.autoplay ? "yes" : "no"} options={yesNoOptions} ariaLabel="Autoplay timer" onchange={(value) => patchPlayback({ autoplay: value === "yes" })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={timer.playback.alternate} onclick={() => patchPlayback({ alternate: !timer.playback.alternate })}>alternate</button>
      <button type="button" class:active={timer.playback.reversed} onclick={() => patchPlayback({ reversed: !timer.playback.reversed })}>reversed</button>
      <button type="button" class:active={timer.playback.persist} onclick={() => patchPlayback({ persist: !timer.playback.persist })}>persist</button>
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head"><span>Callbacks</span></div>
    {#each Object.entries(timer.callbacks ?? {}) as [name, callback]}
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
  .timer-editor,
  .editor-card,
  .callback-row {
    display: flex;
    flex-direction: column;
  }

  .timer-editor {
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
