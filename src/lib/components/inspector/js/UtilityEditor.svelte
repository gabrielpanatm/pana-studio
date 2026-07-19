<script lang="ts">
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionStagger, PanaMotionUtilitiesItem } from "$lib/types";

  let {
    utility,
    onChange,
  }: {
    utility: PanaMotionUtilitiesItem;
    onChange: (item: PanaMotionUtilitiesItem) => void;
  } = $props();

  const utilities = [
    "stagger", "$", "get", "set", "cleanInlineStyles", "remove", "sync", "keepTime",
    "random", "createSeededRandom", "randomPick", "shuffle", "round", "clamp", "snap",
    "wrap", "mapRange", "lerp", "damp", "roundPad", "padStart", "padEnd", "degToRad", "radToDeg",
  ];

  function patch(patchValue: Partial<PanaMotionUtilitiesItem>) {
    onChange({ ...utility, ...patchValue });
  }

  function patchStagger(patchValue: Partial<PanaMotionStagger>) {
    patch({ stagger: { ...utility.stagger, ...patchValue } });
  }

  function patchModifier(patchValue: Partial<PanaMotionExpression>) {
    patchStagger({ modifier: { ...utility.stagger.modifier, ...patchValue } });
  }

  function patchExpression(patchValue: Partial<PanaMotionExpression>) {
    patch({ expression: { ...utility.expression, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }
</script>

<div class="utility-editor">
  <section class="editor-card">
    <div class="section-head"><span>Utility</span></div>
    <div class="field-grid">
      <label>
        <span>Funcție</span>
        <SelectControl value={utility.utility} options={utilities} ariaLabel="Funcție utility Anime" onchange={(value) => patch({ utility: value })} />
      </label>
      <label><span>Args</span><input class="mono" value={utility.args} placeholder="100, &#123; from: 'center' &#125;" oninput={(event) => patch({ args: event.currentTarget.value })} /></label>
    </div>
  </section>

  {#if utility.utility === "stagger"}
    <section class="editor-card">
      <div class="section-head"><span>Stagger params</span></div>
      <div class="field-grid">
        <label><span>Each</span><input type="number" value={utility.stagger.each} oninput={(event) => patchStagger({ each: numberValue(event.currentTarget.value) })} /></label>
        <label><span>Start</span><input type="number" value={utility.stagger.start} oninput={(event) => patchStagger({ start: numberValue(event.currentTarget.value) })} /></label>
        <label><span>From</span><input class="mono" value={utility.stagger.from} placeholder="first, center, last, index" oninput={(event) => patchStagger({ from: event.currentTarget.value })} /></label>
        <label><span>Ease</span><input class="mono" value={utility.stagger.ease} oninput={(event) => patchStagger({ ease: event.currentTarget.value })} /></label>
        <label><span>Grid</span><input class="mono" value={utility.stagger.grid} placeholder="4, 3" oninput={(event) => patchStagger({ grid: event.currentTarget.value })} /></label>
        <label><span>Axis</span><input class="mono" value={utility.stagger.axis} placeholder="x / y" oninput={(event) => patchStagger({ axis: event.currentTarget.value })} /></label>
        <label><span>Use</span><input class="mono" value={utility.stagger.use} placeholder="delay, opacity..." oninput={(event) => patchStagger({ use: event.currentTarget.value })} /></label>
        <label><span>Total</span><input type="number" value={utility.stagger.total} oninput={(event) => patchStagger({ total: numberValue(event.currentTarget.value) })} /></label>
      </div>
      <div class="toggle-grid">
        <button type="button" class:active={utility.stagger.reversed} onclick={() => patchStagger({ reversed: !utility.stagger.reversed })}>reversed</button>
        <button type="button" class:active={utility.stagger.modifier.enabled} onclick={() => patchModifier({ enabled: !utility.stagger.modifier.enabled })}>modifier</button>
      </div>
      {#if utility.stagger.modifier.enabled}
        <label>
          <span>Modifier</span>
          <textarea value={utility.stagger.modifier.code} placeholder="(value, index, total) => value" oninput={(event) => patchModifier({ code: event.currentTarget.value })}></textarea>
        </label>
      {/if}
    </section>
  {/if}

  <section class="editor-card">
    <div class="section-head"><span>Expression fallback</span></div>
    <div class="toggle-grid">
      <button type="button" class:active={utility.expression.enabled} onclick={() => patchExpression({ enabled: !utility.expression.enabled })}>custom expression</button>
    </div>
    {#if utility.expression.enabled}
      <label>
        <span>Cod</span>
        <textarea value={utility.expression.code} placeholder="(anime, item, utils) => utils.clamp(10, 0, 100)" oninput={(event) => patchExpression({ code: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </section>
</div>

<style>
  .utility-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .utility-editor {
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
  textarea,
  button {
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 11px;
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
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }

  button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
