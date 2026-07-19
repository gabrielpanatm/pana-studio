<script lang="ts">
  import {
    ANIME_ALL_KNOWN_PROPS,
    ANIME_ANIMATION_TRIGGERS,
    ANIME_DIRECTIONS,
    ANIME_EASING_OPTIONS,
    ANIME_PRESETS,
    ANIME_PROP_DEFAULTS,
    ANIME_PROP_GROUPS,
    inferAnimePropertyCategory,
    toAnimeV4Ease,
    type AnimePreset,
  } from "$lib/js/anime-catalog";
  import { defaultMotionKeyframe, defaultMotionProperty, emptyExpression } from "$lib/js/motion-config";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import type {
    PanaMotionAnimationItem,
    PanaMotionExpression,
    PanaMotionKeyframe,
    PanaMotionPlayback,
    PanaMotionProperty,
    PanaMotionStagger,
    PanaMotionTween,
    PanaMotionValue,
  } from "$lib/types";

  let {
    animation,
    onChange,
  }: {
    animation: PanaMotionAnimationItem;
    onChange: (item: PanaMotionAnimationItem) => void;
  } = $props();

  const valueModes: PanaMotionValue["mode"][] = ["fromTo", "literal", "relative", "cssVariable", "color", "function", "random", "expression"];
  const categories: PanaMotionProperty["category"][] = ["css", "transform", "cssVariable", "object", "htmlAttribute", "svgAttribute", "utility"];
  const compositionOptions = ["replace", "blend", "add", "none"];
  const textEffectOptions = [
    { value: "", label: "none" },
    "chars",
    "words",
    "typewriter",
  ];
  const scrollModeOptions = [
    { value: "once", label: "o dată" },
    "repeat",
    "scrub",
  ];
  const yesNoOptions = [
    { value: "yes", label: "da" },
    { value: "no", label: "nu" },
  ];
  const easingOptions = [{ value: "", label: "default" }, ...ANIME_EASING_OPTIONS];
  const basePropertyOptions = ANIME_PROP_GROUPS.flatMap((group) => group.props.map((propName) => ({
    value: propName,
    label: propName,
    detail: group.label,
  })));

  function patch(patchValue: Partial<PanaMotionAnimationItem>) {
    onChange({ ...animation, ...patchValue });
  }

  function patchPlayback(patchValue: Partial<PanaMotionPlayback>) {
    patch({ playback: { ...animation.playback, ...patchValue } });
  }

  function patchStagger(patchValue: Partial<PanaMotionStagger>) {
    patch({ stagger: { ...animation.stagger, ...patchValue } });
  }

  function patchStaggerModifier(patchValue: Partial<PanaMotionExpression>) {
    patchStagger({ modifier: { ...animation.stagger.modifier, ...patchValue } });
  }

  function propertyFromPreset(prop: AnimePreset["props"][number], index = 0): PanaMotionProperty {
    const property = defaultMotionProperty(prop.prop);
    return {
      ...property,
      id: `${property.id}-${index}`,
      value: {
        ...property.value,
        mode: "fromTo",
        from: prop.from,
        to: prop.to,
      },
    };
  }

  function applyPreset(preset: AnimePreset) {
    patch({
      properties: preset.props.map(propertyFromPreset),
      playback: {
        ...animation.playback,
        duration: preset.duration ?? animation.playback.duration,
        playbackEase: toAnimeV4Ease(preset.easing),
      },
      stagger: {
        ...animation.stagger,
        enabled: Boolean(preset.stagger),
        each: preset.stagger ?? animation.stagger.each,
      },
      textEffect: preset.textEffect ?? "",
    });
  }

  function addProperty() {
    patch({ properties: [...animation.properties, defaultMotionProperty("opacity")] });
  }

  function updateProperty(propertyId: string, patchValue: Partial<PanaMotionProperty>) {
    patch({
      properties: animation.properties.map((property) => property.id === propertyId ? { ...property, ...patchValue } : property),
    });
  }

  function updatePropertyValue(propertyId: string, patchValue: Partial<PanaMotionValue>) {
    patch({
      properties: animation.properties.map((property) => property.id === propertyId
        ? { ...property, value: { ...property.value, ...patchValue } }
        : property),
    });
  }

  function updatePropertyModifier(propertyId: string, patchValue: Partial<PanaMotionExpression>) {
    patch({
      properties: animation.properties.map((property) => property.id === propertyId
        ? { ...property, modifier: { ...property.modifier, ...patchValue } }
        : property),
    });
  }

  function updatePropertyTween(propertyId: string, patchValue: Partial<PanaMotionTween>) {
    patch({
      properties: animation.properties.map((property) => property.id === propertyId
        ? { ...property, tween: { ...property.tween, ...patchValue } }
        : property),
    });
  }

  function removeProperty(propertyId: string) {
    patch({ properties: animation.properties.filter((property) => property.id !== propertyId) });
  }

  function addKeyframe() {
    patch({ keyframes: [...animation.keyframes, defaultMotionKeyframe()] });
  }

  function updateKeyframe(keyframeId: string, patchValue: Partial<PanaMotionKeyframe>) {
    patch({
      keyframes: animation.keyframes.map((keyframe) => keyframe.id === keyframeId ? { ...keyframe, ...patchValue } : keyframe),
    });
  }

  function addKeyframeProperty(keyframe: PanaMotionKeyframe) {
    updateKeyframe(keyframe.id, { properties: [...keyframe.properties, defaultMotionProperty("opacity")] });
  }

  function updateKeyframeProperty(keyframe: PanaMotionKeyframe, propertyId: string, patchValue: Partial<PanaMotionProperty>) {
    updateKeyframe(keyframe.id, {
      properties: keyframe.properties.map((property) => property.id === propertyId ? { ...property, ...patchValue } : property),
    });
  }

  function updateKeyframePropertyValue(keyframe: PanaMotionKeyframe, propertyId: string, patchValue: Partial<PanaMotionValue>) {
    updateKeyframe(keyframe.id, {
      properties: keyframe.properties.map((property) => property.id === propertyId
        ? { ...property, value: { ...property.value, ...patchValue } }
        : property),
    });
  }

  function updateKeyframePropertyTween(keyframe: PanaMotionKeyframe, propertyId: string, patchValue: Partial<PanaMotionTween>) {
    updateKeyframe(keyframe.id, {
      properties: keyframe.properties.map((property) => property.id === propertyId
        ? { ...property, tween: { ...property.tween, ...patchValue } }
        : property),
    });
  }

  function removeKeyframeProperty(keyframe: PanaMotionKeyframe, propertyId: string) {
    updateKeyframe(keyframe.id, { properties: keyframe.properties.filter((property) => property.id !== propertyId) });
  }

  function removeKeyframe(keyframeId: string) {
    patch({ keyframes: animation.keyframes.filter((keyframe) => keyframe.id !== keyframeId) });
  }

  function toggleCallback(name: string, enabled: boolean) {
    patch({
      callbacks: {
        ...animation.callbacks,
        [name]: { ...(animation.callbacks[name] ?? emptyExpression(name)), enabled },
      },
    });
  }

  function updateCallback(name: string, code: string) {
    patch({
      callbacks: {
        ...animation.callbacks,
        [name]: { ...(animation.callbacks[name] ?? emptyExpression(name)), code, enabled: code.trim().length > 0 },
      },
    });
  }

  function addKeyframeAdvanced(keyframe: PanaMotionKeyframe) {
    updateKeyframe(keyframe.id, {
      advanced: [...keyframe.advanced, emptyExpression("Keyframe expression")],
    });
  }

  function updateKeyframeAdvanced(keyframe: PanaMotionKeyframe, index: number, patchValue: Partial<PanaMotionExpression>) {
    updateKeyframe(keyframe.id, {
      advanced: keyframe.advanced.map((expression, expressionIndex) => expressionIndex === index
        ? { ...expression, ...patchValue }
        : expression),
    });
  }

  function tweenNumber(value: string): number {
    const parsed = parseInt(value, 10);
    return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
  }

  function valuePlaceholder(mode: PanaMotionValue["mode"]): string {
    if (mode === "relative") return "+=20";
    if (mode === "cssVariable") return "--radius-xl";
    if (mode === "random") return "0, 100";
    if (mode === "color") return "#3d3846";
    return "valoare";
  }

  function unitPlaceholder(mode: PanaMotionValue["mode"]): string {
    if (mode === "relative") return "px";
    if (mode === "cssVariable" || mode === "color" || mode === "random") return "";
    return "px, %, deg";
  }

  function propertyOptions(currentProperty: string) {
    if (ANIME_ALL_KNOWN_PROPS.has(currentProperty)) return basePropertyOptions;
    return [...basePropertyOptions, { value: currentProperty, label: currentProperty, detail: "custom" }];
  }
</script>

<div class="animation-editor">
  <section class="editor-card">
    <div class="section-head">
      <span>Preseturi</span>
    </div>
    <div class="preset-grid">
      {#each ANIME_PRESETS as preset}
        <button type="button" onclick={() => applyPreset(preset)}>{preset.name}</button>
      {/each}
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Trigger și target</span>
    </div>
    <div class="field-grid">
      <label>
        <span>Trigger</span>
        <SelectControl value={animation.trigger ?? "load"} options={ANIME_ANIMATION_TRIGGERS} ariaLabel="Trigger animație" onchange={(value) => patch({ trigger: value as PanaMotionAnimationItem["trigger"] })} />
      </label>
      <label>
        <span>Țintă internă</span>
        <input class="mono" value={animation.targetSelector ?? ""} placeholder="> * sau .card-title" oninput={(event) => patch({ targetSelector: event.currentTarget.value })} />
      </label>
      <label>
        <span>Text effect</span>
        <SelectControl value={animation.textEffect ?? ""} options={textEffectOptions} ariaLabel="Text effect animație" onchange={(value) => patch({ textEffect: value })} />
      </label>
      {#if animation.trigger === "scroll"}
        <label>
          <span>Scroll mode</span>
          <SelectControl value={animation.scrollScrub ? "scrub" : animation.scrollRepeat ? "repeat" : "once"} options={scrollModeOptions} ariaLabel="Scroll mode animație" onchange={(value) => {
            patch({ scrollScrub: value === "scrub", scrollRepeat: value === "repeat" });
          }} />
        </label>
      {/if}
    </div>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Playback</span>
    </div>
    <div class="field-grid">
      <label><span>Autoplay</span><SelectControl value={animation.playback.autoplay ? "yes" : "no"} options={yesNoOptions} ariaLabel="Autoplay animație" onchange={(value) => patchPlayback({ autoplay: value === "yes" })} /></label>
      <label><span>Delay</span><input type="number" min="0" step="50" value={animation.playback.delay} oninput={(event) => patchPlayback({ delay: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Duration</span><input type="number" min="0" step="50" value={animation.playback.duration} oninput={(event) => patchPlayback({ duration: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Ease</span><SelectControl value={animation.playback.playbackEase} options={easingOptions} ariaLabel="Ease animație" onchange={(value) => patchPlayback({ playbackEase: value })} /></label>
      <label><span>Loop</span><input type="number" value={animation.playback.loop} placeholder="-1 infinit" oninput={(event) => patchPlayback({ loop: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Loop delay</span><input type="number" min="0" step="50" value={animation.playback.loopDelay} oninput={(event) => patchPlayback({ loopDelay: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Frame rate</span><input type="number" min="0" value={animation.playback.frameRate} oninput={(event) => patchPlayback({ frameRate: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Playback rate</span><input type="number" step="0.1" value={animation.playback.playbackRate} oninput={(event) => patchPlayback({ playbackRate: parseFloat(event.currentTarget.value) || 1 })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={animation.playback.alternate} onclick={() => patchPlayback({ alternate: !animation.playback.alternate })}>alternate</button>
      <button type="button" class:active={animation.playback.reversed} onclick={() => patchPlayback({ reversed: !animation.playback.reversed })}>reversed</button>
      <button type="button" class:active={animation.playback.persist} onclick={() => patchPlayback({ persist: !animation.playback.persist })}>persist</button>
    </div>
    <label>
      <span>Direcție preset</span>
      <SelectControl value={animation.playback.alternate ? animation.playback.reversed ? "alternateReverse" : "alternate" : animation.playback.reversed ? "reverse" : "normal"} options={ANIME_DIRECTIONS} ariaLabel="Direcție preset animație" onchange={(direction) => {
        patchPlayback({
          alternate: direction === "alternate" || direction === "alternateReverse",
          reversed: direction === "reverse" || direction === "alternateReverse",
        });
      }} />
    </label>
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Stagger</span>
    </div>
    <div class="field-grid">
      <label><span>Activ</span><SelectControl value={animation.stagger.enabled ? "yes" : "no"} options={[{ value: "no", label: "nu" }, { value: "yes", label: "da" }]} ariaLabel="Stagger activ" onchange={(value) => patchStagger({ enabled: value === "yes" })} /></label>
      <label><span>Each</span><input type="number" value={animation.stagger.each} oninput={(event) => patchStagger({ each: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>Start</span><input type="number" value={animation.stagger.start} oninput={(event) => patchStagger({ start: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
      <label><span>From</span><input value={animation.stagger.from} placeholder="first, center, last, index" oninput={(event) => patchStagger({ from: event.currentTarget.value })} /></label>
      <label><span>Ease</span><input class="mono" value={animation.stagger.ease} oninput={(event) => patchStagger({ ease: event.currentTarget.value })} /></label>
      <label><span>Grid</span><input class="mono" value={animation.stagger.grid} placeholder="3,4" oninput={(event) => patchStagger({ grid: event.currentTarget.value })} /></label>
      <label><span>Axis</span><SelectControl value={animation.stagger.axis} options={[{ value: "", label: "none" }, "x", "y"]} ariaLabel="Stagger axis" onchange={(value) => patchStagger({ axis: value })} /></label>
      <label><span>Use</span><input class="mono" value={animation.stagger.use} placeholder="delay, opacity, translateY" oninput={(event) => patchStagger({ use: event.currentTarget.value })} /></label>
      <label><span>Total</span><input type="number" value={animation.stagger.total} oninput={(event) => patchStagger({ total: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={animation.stagger.reversed} onclick={() => patchStagger({ reversed: !animation.stagger.reversed })}>reversed</button>
      <button type="button" class:active={animation.stagger.modifier.enabled} onclick={() => patchStaggerModifier({ enabled: !animation.stagger.modifier.enabled })}>modifier</button>
    </div>
    {#if animation.stagger.modifier.enabled}
      <label>
        <span>Stagger modifier</span>
        <textarea value={animation.stagger.modifier.code} placeholder="(value) => value * 2" oninput={(event) => patchStaggerModifier({ code: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Proprietăți</span>
      <button type="button" onclick={addProperty}>+ prop</button>
    </div>
    {#each animation.properties as property}
      {@render PropertyEditor(
        property,
        (patchValue) => updateProperty(property.id, patchValue),
        (patchValue) => updatePropertyValue(property.id, patchValue),
        (patchValue) => updatePropertyModifier(property.id, patchValue),
        (patchValue) => updatePropertyTween(property.id, patchValue),
        () => removeProperty(property.id),
      )}
    {/each}
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Keyframes</span>
      <button type="button" onclick={addKeyframe}>+ keyframe</button>
    </div>
    {#if animation.keyframes.length === 0}
      <p class="hint">Fără keyframes. Animația folosește proprietățile principale.</p>
    {/if}
    {#each animation.keyframes as keyframe}
      <div class="keyframe-card">
        <div class="section-head">
          <strong>{keyframe.label || "Keyframe"}</strong>
          <button type="button" class="danger" onclick={() => removeKeyframe(keyframe.id)}>șterge</button>
        </div>
        <div class="field-grid">
          <label><span>Label</span><input value={keyframe.label} oninput={(event) => updateKeyframe(keyframe.id, { label: event.currentTarget.value })} /></label>
          <label><span>At</span><input class="mono" value={keyframe.at} placeholder="50% pentru percentage keyframes" oninput={(event) => updateKeyframe(keyframe.id, { at: event.currentTarget.value })} /></label>
          <label><span>Duration</span><input type="number" min="0" step="50" value={keyframe.duration} oninput={(event) => updateKeyframe(keyframe.id, { duration: parseInt(event.currentTarget.value, 10) || 0 })} /></label>
          <label><span>Ease</span><input class="mono" value={keyframe.ease} oninput={(event) => updateKeyframe(keyframe.id, { ease: event.currentTarget.value })} /></label>
        </div>
        <div class="section-head compact">
          <span>Props keyframe</span>
          <button type="button" onclick={() => addKeyframeProperty(keyframe)}>+ prop</button>
        </div>
        {#each keyframe.properties as property}
          {@render PropertyEditor(
            property,
            (patchValue) => updateKeyframeProperty(keyframe, property.id, patchValue),
            (patchValue) => updateKeyframePropertyValue(keyframe, property.id, patchValue),
            () => undefined,
            (patchValue) => updateKeyframePropertyTween(keyframe, property.id, patchValue),
            () => removeKeyframeProperty(keyframe, property.id),
          )}
        {/each}
        <div class="section-head compact">
          <span>Advanced keyframe</span>
          <button type="button" onclick={() => addKeyframeAdvanced(keyframe)}>+ expresie</button>
        </div>
        {#each keyframe.advanced as expression, index}
          <div class="callback-row">
            <button
              type="button"
              class:active={expression.enabled}
              onclick={() => updateKeyframeAdvanced(keyframe, index, { enabled: !expression.enabled })}
            >
              {expression.label || "Expression"}
            </button>
            {#if expression.enabled}
              <textarea
                value={expression.code}
                placeholder="(params, frame, item, anime) => params"
                oninput={(event) => updateKeyframeAdvanced(keyframe, index, { code: event.currentTarget.value })}
              ></textarea>
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </section>

  <section class="editor-card">
    <div class="section-head">
      <span>Callbacks</span>
    </div>
    {#each Object.entries(animation.callbacks) as [name, callback]}
      <div class="callback-row">
        <button type="button" class:active={callback.enabled} onclick={() => toggleCallback(name, !callback.enabled)}>{name}</button>
        {#if callback.enabled}
          <textarea value={callback.code} placeholder="(self, anime, utils) => ..." oninput={(event) => updateCallback(name, event.currentTarget.value)}></textarea>
        {/if}
      </div>
    {/each}
  </section>
</div>

{#snippet PropertyEditor(
  property: PanaMotionProperty,
  updateProperty: (patchValue: Partial<PanaMotionProperty>) => void,
  updateValue: (patchValue: Partial<PanaMotionValue>) => void,
  updateModifier: (patchValue: Partial<PanaMotionExpression>) => void,
  updateTween: (patchValue: Partial<PanaMotionTween>) => void,
  removeProperty: () => void,
)}
  <div class="property-card">
    <div class="property-grid">
      <label>
        <span>Prop</span>
        <SelectControl value={property.property} options={propertyOptions(property.property)} ariaLabel="Proprietate Anime" onchange={(value) => {
          const defaults = ANIME_PROP_DEFAULTS[value];
          updateProperty({ property: value, category: inferAnimePropertyCategory(value) });
          if (defaults) updateValue({ mode: "fromTo", from: defaults.from, to: defaults.to });
        }} />
      </label>
      <label>
        <span>Custom</span>
        <input class="mono" value={property.property} oninput={(event) => {
          const value = event.currentTarget.value;
          updateProperty({ property: value, category: inferAnimePropertyCategory(value) });
        }} />
      </label>
      <label>
        <span>Categorie</span>
        <SelectControl value={property.category} options={categories} ariaLabel="Categorie proprietate Anime" onchange={(value) => updateProperty({ category: value as PanaMotionProperty["category"] })} />
      </label>
      <button type="button" class="danger remove-prop" onclick={removeProperty}>×</button>
    </div>
    <div class="field-grid">
      <label>
        <span>Value mode</span>
        <SelectControl value={property.value.mode} options={valueModes} ariaLabel="Mod valoare Anime" onchange={(value) => updateValue({ mode: value as PanaMotionValue["mode"] })} />
      </label>
      <label>
        <span>Composition</span>
        <SelectControl value={property.composition} options={compositionOptions} ariaLabel="Compoziție Anime" onchange={(value) => updateProperty({ composition: value })} />
      </label>
      {#if property.value.mode === "fromTo"}
        <label><span>From</span><input value={property.value.from} oninput={(event) => updateValue({ from: event.currentTarget.value })} /></label>
        <label><span>To</span><input value={property.value.to} oninput={(event) => updateValue({ to: event.currentTarget.value })} /></label>
      {:else if property.value.mode === "expression" || property.value.mode === "function"}
        <label class="span-2"><span>Expression</span><textarea value={property.value.expression} placeholder="(target, i, total) => i * 100" oninput={(event) => updateValue({ expression: event.currentTarget.value })}></textarea></label>
      {:else}
        <label><span>Value</span><input value={property.value.value} placeholder={valuePlaceholder(property.value.mode)} oninput={(event) => updateValue({ value: event.currentTarget.value })} /></label>
        <label><span>Unit</span><input value={property.value.unit} placeholder={unitPlaceholder(property.value.mode)} disabled={property.value.mode === "cssVariable" || property.value.mode === "color" || property.value.mode === "random"} oninput={(event) => updateValue({ unit: event.currentTarget.value })} /></label>
      {/if}
    </div>
    <div class="field-grid">
      <label><span>Tween delay</span><input type="number" min="0" step="50" value={property.tween.delay} oninput={(event) => updateTween({ delay: tweenNumber(event.currentTarget.value) })} /></label>
      <label><span>Tween duration</span><input type="number" min="0" step="50" value={property.tween.duration} placeholder="0 = global" oninput={(event) => updateTween({ duration: tweenNumber(event.currentTarget.value) })} /></label>
      <label class="span-2"><span>Tween ease</span><input class="mono" value={property.tween.ease} placeholder="outExpo, spring(...)" oninput={(event) => updateTween({ ease: event.currentTarget.value })} /></label>
    </div>
    <div class="toggle-grid">
      <button type="button" class:active={property.modifier.enabled} onclick={() => updateModifier({ enabled: !property.modifier.enabled })}>modifier</button>
    </div>
    {#if property.modifier.enabled}
      <label>
        <span>Modifier</span>
        <textarea value={property.modifier.code} placeholder="(value) => Math.round(value)" oninput={(event) => updateModifier({ code: event.currentTarget.value })}></textarea>
      </label>
    {/if}
  </div>
{/snippet}

<style>
  .animation-editor {
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-width: 0;
  }

  .editor-card,
  .keyframe-card,
  .property-card {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
    padding: 8px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface-3);
  }

  .keyframe-card,
  .property-card {
    background: var(--surface-4);
  }

  .section-head,
  .preset-grid,
  .toggle-grid {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .section-head {
    justify-content: space-between;
  }

  .section-head.compact {
    margin-top: 3px;
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

  .section-head strong {
    font-size: 12px;
    color: var(--text);
  }

  .section-head button,
  .preset-grid button,
  .toggle-grid button,
  .callback-row button {
    min-height: 24px;
    border: 1px solid var(--border-4);
    border-radius: 5px;
    background: var(--surface-5);
    color: var(--text);
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }

  .preset-grid {
    flex-wrap: wrap;
  }

  .preset-grid button {
    flex: 1 1 72px;
  }

  .field-grid,
  .property-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .property-grid {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 92px 24px;
    align-items: end;
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

  input:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  textarea {
    min-height: 56px;
    padding-block: 6px;
    resize: vertical;
  }

  .mono {
    font-family: "JetBrains Mono", monospace;
  }

  .span-2 {
    grid-column: span 2;
  }

  .toggle-grid {
    flex-wrap: wrap;
  }

  .toggle-grid button.active,
  .callback-row button.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .danger {
    color: var(--danger) !important;
  }

  .remove-prop {
    height: 25px;
    padding: 0;
  }

  .callback-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .hint {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
