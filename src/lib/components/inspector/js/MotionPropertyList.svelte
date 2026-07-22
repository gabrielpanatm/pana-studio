<script lang="ts">
  import {
    ANIME_ALL_KNOWN_PROPS,
    ANIME_PROP_DEFAULTS,
    ANIME_PROP_GROUPS,
    inferAnimePropertyCategory,
  } from "$lib/js/anime-catalog";
  import { defaultMotionProperty } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type { PanaMotionExpression, PanaMotionProperty, PanaMotionTween, PanaMotionValue } from "$lib/types";

  let {
    properties,
    onChange,
    title = "Proprietăți",
    compact = false,
  }: {
    properties: PanaMotionProperty[];
    onChange: (properties: PanaMotionProperty[]) => void;
    title?: string;
    compact?: boolean;
  } = $props();

  const valueModes: PanaMotionValue["mode"][] = ["fromTo", "literal", "relative", "cssVariable", "color", "function", "random", "expression"];
  const categories: PanaMotionProperty["category"][] = ["css", "transform", "cssVariable", "object", "htmlAttribute", "svgAttribute", "utility"];
  const compositionOptions = ["replace", "blend", "add", "none"];

  function addProperty() {
    onChange([...properties, defaultMotionProperty("opacity")]);
  }

  function removeProperty(propertyId: string) {
    onChange(properties.filter((property) => property.id !== propertyId));
  }

  function updateProperty(propertyId: string, patchValue: Partial<PanaMotionProperty>) {
    onChange(properties.map((property) => property.id === propertyId ? { ...property, ...patchValue } : property));
  }

  function updateValue(propertyId: string, patchValue: Partial<PanaMotionValue>) {
    onChange(properties.map((property) => property.id === propertyId
      ? { ...property, value: { ...property.value, ...patchValue } }
      : property));
  }

  function updateTween(propertyId: string, patchValue: Partial<PanaMotionTween>) {
    onChange(properties.map((property) => property.id === propertyId
      ? { ...property, tween: { ...property.tween, ...patchValue } }
      : property));
  }

  function updateModifier(propertyId: string, patchValue: Partial<PanaMotionExpression>) {
    onChange(properties.map((property) => property.id === propertyId
      ? { ...property, modifier: { ...property.modifier, ...patchValue } }
      : property));
  }

  function applyPropertyName(property: PanaMotionProperty, value: string) {
    const defaults = ANIME_PROP_DEFAULTS[value];
    updateProperty(property.id, {
      property: value,
      category: inferAnimePropertyCategory(value),
      value: defaults ? { ...property.value, from: defaults.from, to: defaults.to } : property.value,
    });
  }

  function numberValue(value: string): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }
</script>

<section class="property-card" class:compact>
  <div class="section-head">
    <span>{title}</span>
    <button type="button" onclick={addProperty}>+ prop</button>
  </div>

  {#each properties as property}
    <div class="property-block">
      <div class="field-grid">
        <label>
          <span>Prop</span>
          <input
            class="mono"
            list="anime-motion-props"
            value={property.property}
            oninput={(event) => applyPropertyName(property, event.currentTarget.value)}
          />
        </label>
        <label>
          <span>Categorie</span>
          <SelectControl value={property.category} options={categories} ariaLabel="Categorie proprietate motion" onchange={(value) => updateProperty(property.id, { category: value as PanaMotionProperty["category"] })} />
        </label>
      </div>

      <div class="field-grid three">
        <label>
          <span>Mod</span>
          <SelectControl value={property.value.mode} options={valueModes} ariaLabel="Mod valoare motion" onchange={(value) => updateValue(property.id, { mode: value as PanaMotionValue["mode"] })} />
        </label>
        <label><span>From</span><input value={property.value.from} oninput={(event) => updateValue(property.id, { from: event.currentTarget.value })} /></label>
        <label><span>To / value</span><input value={property.value.to || property.value.value} oninput={(event) => updateValue(property.id, property.value.mode === "literal" ? { value: event.currentTarget.value } : { to: event.currentTarget.value })} /></label>
      </div>

      <div class="field-grid three">
        <label><span>Unit</span><input class="mono" value={property.value.unit} placeholder="px, %, deg" oninput={(event) => updateValue(property.id, { unit: event.currentTarget.value })} /></label>
        <label>
          <span>Composition</span>
          <SelectControl value={property.composition} options={compositionOptions} ariaLabel="Compoziție proprietate motion" onchange={(value) => updateProperty(property.id, { composition: value })} />
        </label>
        <button type="button" class="remove-btn" onclick={() => removeProperty(property.id)}>Șterge</button>
      </div>

      {#if !compact}
        <div class="field-grid three">
          <label><span>Tween delay</span><input type="number" value={property.tween.delay} oninput={(event) => updateTween(property.id, { delay: numberValue(event.currentTarget.value) })} /></label>
          <label><span>Tween duration</span><input type="number" value={property.tween.duration} oninput={(event) => updateTween(property.id, { duration: numberValue(event.currentTarget.value) })} /></label>
          <label><span>Tween ease</span><input class="mono" value={property.tween.ease} oninput={(event) => updateTween(property.id, { ease: event.currentTarget.value })} /></label>
        </div>

        <div class="toggle-row">
          <button type="button" class:active={property.modifier.enabled} onclick={() => updateModifier(property.id, { enabled: !property.modifier.enabled })}>modifier</button>
        </div>
        {#if property.modifier.enabled}
          <label>
            <span>Modifier</span>
            <textarea value={property.modifier.code} placeholder="(value, anime, utils) => value" oninput={(event) => updateModifier(property.id, { code: event.currentTarget.value })}></textarea>
          </label>
        {/if}
      {/if}
    </div>
  {/each}
</section>

<datalist id="anime-motion-props">
  {#each ANIME_PROP_GROUPS as group}
    {#each group.props as prop}
      <option value={prop}>{group.label}</option>
    {/each}
  {/each}
  {#each Array.from(ANIME_ALL_KNOWN_PROPS) as prop}
    <option value={prop}>{prop}</option>
  {/each}
</datalist>

<style>
  .property-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 9px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .property-card.compact {
    padding: 7px;
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
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    text-transform: uppercase;
  }

  .section-head button,
  .remove-btn,
  .toggle-row button {
    min-height: 25px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
    cursor: pointer;
  }

  .remove-btn {
    align-self: end;
    color: var(--danger);
  }

  .property-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 7px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-5);
  }

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .field-grid.three {
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
    background: var(--surface-4);
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

  .toggle-row button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }
</style>
