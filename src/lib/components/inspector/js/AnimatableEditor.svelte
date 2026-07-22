<script lang="ts">
  import { ANIME_EASING_OPTIONS } from "$lib/js/anime-catalog";
  import type { PanaMotionAnimatableItem, PanaMotionExpression } from "$lib/types";
  import MotionPropertyList from "$lib/components/inspector/js/MotionPropertyList.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

  let {
    animatable,
    onChange,
  }: {
    animatable: PanaMotionAnimatableItem;
    onChange: (item: PanaMotionAnimatableItem) => void;
  } = $props();

  function patch(patchValue: Partial<PanaMotionAnimatableItem>) {
    onChange({ ...animatable, ...patchValue });
  }

  function patchSetterExpression(patchValue: Partial<PanaMotionExpression>) {
    patch({ setterExpression: { ...animatable.setterExpression, ...patchValue } });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  const modeOptions: PanaMotionAnimatableItem["mode"][] = ["setters", "getters", "both"];
  const liveSourceOptions: PanaMotionAnimatableItem["liveSource"][] = ["none", "pointer", "scroll", "expression"];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];
</script>

<div class="animatable-editor">
  <section class="editor-card">
    <div class="section-head"><span>Animatable</span></div>
    <div class="field-grid">
      <label>
        <span>Mod</span>
        <SelectControl value={animatable.mode} options={modeOptions} ariaLabel="Animatable mode" onchange={(value) => patch({ mode: value as PanaMotionAnimatableItem["mode"] })} />
      </label>
      <label>
        <span>Live source</span>
        <SelectControl value={animatable.liveSource} options={liveSourceOptions} ariaLabel="Animatable live source" onchange={(value) => patch({ liveSource: value as PanaMotionAnimatableItem["liveSource"] })} />
      </label>
      <label><span>Durată</span><input type="number" value={animatable.duration} oninput={(event) => patch({ duration: numberValue(event.currentTarget.value) })} /></label>
      <label>
        <span>Ease</span>
        <SelectControl value={animatable.ease} options={easingOptions} ariaLabel="Animatable ease" onchange={(value) => patch({ ease: value })} />
      </label>
      <label><span>Unit</span><input class="mono" value={animatable.unit} placeholder="px, deg, %" oninput={(event) => patch({ unit: event.currentTarget.value })} /></label>
    </div>
  </section>

  <MotionPropertyList
    title="Proprietăți animatable"
    properties={animatable.properties}
    onChange={(properties) => patch({ properties })}
  />

  <section class="editor-card">
    <div class="section-head"><span>Setter expression</span></div>
    <div class="toggle-grid">
      <button type="button" class:active={animatable.setterExpression.enabled} onclick={() => patchSetterExpression({ enabled: !animatable.setterExpression.enabled })}>expression</button>
    </div>
    {#if animatable.setterExpression.enabled}
      <label>
        <span>Cod</span>
        <textarea
          value={animatable.setterExpression.code}
          placeholder="(animatable, target, anime, utils) => &#123; animatable.x(20); &#125;"
          oninput={(event) => patchSetterExpression({ code: event.currentTarget.value })}
        ></textarea>
      </label>
    {/if}
  </section>
</div>

<style>
  .animatable-editor,
  .editor-card {
    display: flex;
    flex-direction: column;
  }

  .animatable-editor {
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

  input,
  .toggle-grid button {
    min-height: 25px;
  }

  textarea {
    min-height: 74px;
    padding-block: 6px;
    resize: vertical;
  }

  .toggle-grid button {
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .toggle-grid button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }
</style>
