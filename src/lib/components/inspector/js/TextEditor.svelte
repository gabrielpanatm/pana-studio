<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import { emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionTextItem } from "$lib/types";

  let {
    text,
    onChange,
  }: {
    text: PanaMotionTextItem;
    onChange: (item: PanaMotionTextItem) => void;
  } = $props();

  const callbackPlaceholder = "(self, anime, utils) => { ... }";
  const modeOptions: PanaMotionTextItem["mode"][] = ["splitText", "scrambleText"];
  const fromOptions = ["auto", "left", "center", "right", "random"];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];

  function patch(patchValue: Partial<PanaMotionTextItem>) {
    onChange({ ...text, ...patchValue });
  }

  function patchSplit(patchValue: Partial<PanaMotionTextItem["split"]>) {
    patch({ split: { ...text.split, ...patchValue } });
  }

  function patchScramble(patchValue: Partial<PanaMotionTextItem["scramble"]>) {
    patch({ scramble: { ...text.scramble, ...patchValue } });
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
        ...text.callbacks,
        [name]: { ...(text.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...text.callbacks,
        [name]: { ...(text.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }
</script>

<div class="text-editor">
  <section class="editor-card">
    <div class="section-head">
      <span>Text</span>
    </div>
    <label>
      <span>Mode</span>
      <SelectControl value={text.mode} options={modeOptions} ariaLabel="Text mode" onchange={(value) => patch({ mode: value as PanaMotionTextItem["mode"] })} />
    </label>
  </section>

  {#if text.mode === "splitText"}
    <section class="editor-card">
      <div class="section-head">
        <span>splitText</span>
      </div>
      <div class="toggle-grid">
        <button type="button" class:active={text.split.lines} onclick={() => patchSplit({ lines: !text.split.lines })}>lines</button>
        <button type="button" class:active={text.split.words} onclick={() => patchSplit({ words: !text.split.words })}>words</button>
        <button type="button" class:active={text.split.chars} onclick={() => patchSplit({ chars: !text.split.chars })}>chars</button>
        <button type="button" class:active={text.split.includeSpaces} onclick={() => patchSplit({ includeSpaces: !text.split.includeSpaces })}>spaces</button>
        <button type="button" class:active={text.split.accessible} onclick={() => patchSplit({ accessible: !text.split.accessible })}>accessible</button>
        <button type="button" class:active={text.split.debug} onclick={() => patchSplit({ debug: !text.split.debug })}>debug</button>
      </div>
      <div class="field-grid">
        <label><span>Class</span><input class="mono" value={text.split.className} placeholder="split-word" oninput={(event) => patchSplit({ className: event.currentTarget.value })} /></label>
        <label><span>Wrap</span><input class="mono" value={text.split.wrap} placeholder="span / clip" oninput={(event) => patchSplit({ wrap: event.currentTarget.value })} /></label>
      </div>
      <div class="toggle-grid two">
        <button type="button" class:active={text.split.clone} onclick={() => patchSplit({ clone: !text.split.clone })}>clone</button>
      </div>
    </section>
  {:else}
    <section class="editor-card">
      <div class="section-head">
        <span>scrambleText</span>
      </div>
      <label>
        <span>Text</span>
        <textarea value={text.scramble.text} placeholder="Gol = textul curent al elementului" oninput={(event) => patchScramble({ text: event.currentTarget.value })}></textarea>
      </label>
      <div class="field-grid">
        <label><span>Chars</span><input class="mono" value={text.scramble.chars} placeholder="lowercase, uppercase, a-zA-Z0-9" oninput={(event) => patchScramble({ chars: event.currentTarget.value })} /></label>
        <label><span>Cursor</span><input value={text.scramble.cursor} oninput={(event) => patchScramble({ cursor: event.currentTarget.value })} /></label>
        <label>
          <span>From</span>
          <SelectControl value={text.scramble.from} options={fromOptions} ariaLabel="Scramble from" onchange={(value) => patchScramble({ from: value })} />
        </label>
        <label>
          <span>Ease</span>
          <SelectControl value={text.scramble.ease} options={easingOptions} ariaLabel="Scramble ease" onchange={(value) => patchScramble({ ease: value })} />
        </label>
        <label><span>Reveal rate</span><input type="number" value={text.scramble.revealRate} oninput={(event) => patchScramble({ revealRate: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Reveal delay</span><input type="number" value={text.scramble.revealDelay} oninput={(event) => patchScramble({ revealDelay: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Settle rate</span><input type="number" value={text.scramble.settleRate} oninput={(event) => patchScramble({ settleRate: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Settle duration</span><input type="number" value={text.scramble.settleDuration} oninput={(event) => patchScramble({ settleDuration: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Delay</span><input type="number" value={text.scramble.delay} oninput={(event) => patchScramble({ delay: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Duration</span><input type="number" value={text.scramble.duration} oninput={(event) => patchScramble({ duration: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Perturbation</span><input type="number" min="0" max="1" step="0.05" value={text.scramble.perturbation} oninput={(event) => patchScramble({ perturbation: ratioValue(event.currentTarget.value) })} /></label>
        <label><span>Seed</span><input type="number" value={text.scramble.seed} oninput={(event) => patchScramble({ seed: numberValue(event.currentTarget.value) })} /></label>
      </div>
      <div class="toggle-grid two">
        <button type="button" class:active={text.scramble.override} onclick={() => patchScramble({ override: !text.scramble.override })}>override</button>
        <button type="button" class:active={text.scramble.reversed} onclick={() => patchScramble({ reversed: !text.scramble.reversed })}>reversed</button>
      </div>
    </section>
  {/if}

  <section class="editor-card">
    <div class="section-head">
      <span>Callbacks</span>
    </div>
    {#each Object.entries(text.callbacks ?? {}) as [name, callback]}
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
  .text-editor {
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
