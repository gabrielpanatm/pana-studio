<script lang="ts">
  import {
    IconArrowDown,
    IconArrowUp,
    IconCopy,
    IconLayersIntersect,
    IconPhoto,
    IconPlus,
    IconTrash,
  } from "@tabler/icons-svelte";
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { ScssVariable } from "$lib/css/contracts";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    BACKGROUND_LONGHAND_PROPERTIES,
    backgroundFromProperties,
    cloneBackgroundLayer,
    createBackgroundLayer,
    parseCssGradient,
    serializeBackgroundLonghands,
    serializeCssGradient,
    type CssBackground,
    type CssBackgroundLayer,
  } from "$lib/inspector/background-model";
  import { variablesForProperty } from "$lib/editor/controls";
  import { projectAssetOriginLabel, projectAssetPublicUrl } from "$lib/project/assets";
  import InspectorSection from "../InspectorSection.svelte";
  import AssetPicker from "../controls/AssetPicker.svelte";
  import ColorInput from "../controls/ColorInput.svelte";
  import GradientEditor from "../controls/GradientEditor.svelte";
  import TextWithOptions from "../controls/TextWithOptions.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

  let {
    pendingValues,
    rulesMap,
    canonicalBackground = null,
    scssVariables = [],
    scannedAssets = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    canonicalBackground?: CssBackground | null;
    scssVariables?: ScssVariable[];
    scannedAssets?: ProjectFile[];
    edit: CssPropertyEditController;
  } = $props();

  const TRACKED = ["background", ...BACKGROUND_LONGHAND_PROPERTIES] as const;
  const BLEND_MODES = [
    "normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn",
    "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation", "color", "luminosity",
  ];
  const REPEATS = ["repeat", "no-repeat", "repeat-x", "repeat-y", "space", "round"];
  const ATTACHMENTS = ["scroll", "fixed", "local"];
  const BOXES = ["border-box", "padding-box", "content-box", "text"];

  const imageAssets = $derived(scannedAssets.filter((asset) => asset.kind === "IMAGE"));

  function getValue(property: string) {
    return pendingValues[property] ?? rulesMap[property] ?? "";
  }

  function currentProperties() {
    return Object.fromEntries(TRACKED.map((property) => [property, getValue(property)]));
  }

  function cloneBackground(value: CssBackground): CssBackground {
    return JSON.parse(JSON.stringify(value)) as CssBackground;
  }

  function inputBackground(): CssBackground {
    const hasPendingBackground = TRACKED.some((property) => Object.prototype.hasOwnProperty.call(pendingValues, property));
    if (!hasPendingBackground && canonicalBackground?.schemaVersion === 1) return cloneBackground(canonicalBackground);
    if (canonicalBackground?.schemaVersion === 1) {
      const canonicalProperties = {
        ...serializeBackgroundLonghands(canonicalBackground),
        background: canonicalBackground.shorthand ?? "",
      };
      const pendingBackground = Object.fromEntries(TRACKED
        .filter((property) => Object.prototype.hasOwnProperty.call(pendingValues, property))
        .map((property) => [property, pendingValues[property]]));
      return backgroundFromProperties({ ...canonicalProperties, ...pendingBackground });
    }
    return backgroundFromProperties(currentProperties());
  }

  let background = $state<CssBackground>(backgroundFromProperties({}));
  let activeLayerId = $state<string | null>(null);
  let lastEmittedFingerprint = "";
  const hasPendingBackground = $derived(TRACKED.some((property) => Object.prototype.hasOwnProperty.call(pendingValues, property)));
  const sourceFingerprint = $derived([
    TRACKED.map((property) => `${property}\u0000${getValue(property)}`).join("\u0001"),
    hasPendingBackground ? "" : JSON.stringify(canonicalBackground),
  ].join("\u0002"));
  const hasValues = $derived(TRACKED.some((property) => getValue(property).trim() !== "" && getValue(property).trim() !== "none"));
  const structuralChangesBlocked = $derived(Object.keys(background.opaqueProperties).length > 0);

  $effect(() => {
    const fingerprint = sourceFingerprint;
    if (fingerprint === lastEmittedFingerprint) return;
    const nextBackground = inputBackground();
    // Parsing source CSS creates fresh editor-only IDs. Reading activeLayerId
    // or the just-written background reactively here would make this effect
    // depend on the same local state it updates and can therefore form an
    // infinite update loop after a canonical mutation settles.
    const previousActiveLayerId = untrack(() => activeLayerId);
    background = nextBackground;
    activeLayerId = nextBackground.layers.some(
      (layer) => layer.id === previousActiveLayerId,
    )
      ? previousActiveLayerId
      : nextBackground.layers[0]?.id ?? null;
  });

  function fingerprintFor(properties: Readonly<Record<string, string>>) {
    return [
      TRACKED.map((property) => `${property}\u0000${properties[property] ?? getValue(property)}`).join("\u0001"),
      "",
    ].join("\u0002");
  }

  function emit(next: CssBackground, commit = false) {
    background = next;
    const properties = serializeBackgroundLonghands(next);
    lastEmittedFingerprint = fingerprintFor(properties);
    if (commit) edit.commitMany(properties);
    else edit.draftMany(properties);
  }

  function emitSelected(next: CssBackground, propertiesToWrite: readonly string[], commit = false) {
    background = next;
    const serialized = serializeBackgroundLonghands(next);
    const properties = Object.fromEntries(propertiesToWrite.map((property) => [property, serialized[property] ?? ""]));
    lastEmittedFingerprint = fingerprintFor(properties);
    if (commit) edit.commitMany(properties);
    else edit.draftMany(properties);
  }

  function patchLayer(id: string, patch: Partial<CssBackgroundLayer>, commit = false) {
    const next = {
      ...background,
      layers: background.layers.map((layer) => layer.id === id ? { ...layer, ...patch } : layer),
    };
    const propertyByField: Partial<Record<keyof CssBackgroundLayer, string>> = {
      source: "background-image",
      gradient: "background-image",
      kind: "background-image",
      structurallyEditable: "background-image",
      position: "background-position",
      size: "background-size",
      repeat: "background-repeat",
      attachment: "background-attachment",
      origin: "background-origin",
      clip: "background-clip",
      blendMode: "background-blend-mode",
    };
    const properties = [...new Set(Object.keys(patch)
      .map((field) => propertyByField[field as keyof CssBackgroundLayer])
      .filter((property): property is string => Boolean(property)))];
    emitSelected(next, properties.length ? properties : BACKGROUND_LONGHAND_PROPERTIES, commit);
  }

  function addLayer(kind: "image" | "gradient") {
    const layer = createBackgroundLayer(kind);
    activeLayerId = layer.id;
    emit({ ...background, layers: [layer, ...background.layers], structurallyEditable: background.shorthand === null }, true);
  }

  function duplicateLayer(layer: CssBackgroundLayer) {
    const duplicate = cloneBackgroundLayer(layer);
    const index = background.layers.findIndex((candidate) => candidate.id === layer.id);
    const layers = [...background.layers];
    layers.splice(index + 1, 0, duplicate);
    activeLayerId = duplicate.id;
    emit({ ...background, layers }, true);
  }

  function removeLayer(id: string) {
    const layers = background.layers.filter((layer) => layer.id !== id);
    activeLayerId = layers[0]?.id ?? null;
    emit({ ...background, layers }, true);
  }

  function moveLayer(id: string, direction: -1 | 1) {
    const index = background.layers.findIndex((layer) => layer.id === id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= background.layers.length) return;
    const layers = [...background.layers];
    [layers[index], layers[target]] = [layers[target], layers[index]];
    emit({ ...background, layers }, true);
  }

  function sourceLabel(layer: CssBackgroundLayer) {
    if (layer.kind === "gradient") {
      const repeating = layer.gradient?.repeating ? `${t("inspector-background-repeating")} ` : "";
      return `${repeating}${layer.gradient?.kind ?? t("inspector-background-gradient")}`;
    }
    if (layer.kind === "image") {
      const url = imageUrl(layer.source);
      return url ? url.split("/").pop() || url : t("inspector-background-image");
    }
    return t("inspector-background-opaque-layer");
  }

  function imageUrl(source: string) {
    const match = source.trim().match(/^url\(\s*(["']?)([\s\S]*?)\1\s*\)$/i);
    return match?.[2] ?? "";
  }

  function urlSource(value: string) {
    return `url("${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}")`;
  }

  function patchLayerSource(layer: CssBackgroundLayer, source: string, commit = false) {
    const gradient = parseCssGradient(source);
    const patch: Partial<CssBackgroundLayer> = {
      source,
      gradient: gradient ?? layer.gradient,
      kind: gradient ? "gradient" : layer.kind,
      structurallyEditable: gradient?.structurallyEditable ?? layer.structurallyEditable,
    };
    patchLayer(layer.id, patch, commit);
  }

  function patchGradient(layer: CssBackgroundLayer, gradient: NonNullable<CssBackgroundLayer["gradient"]>, commit = false) {
    patchLayer(layer.id, {
      kind: "gradient",
      gradient,
      source: serializeCssGradient(gradient),
      structurallyEditable: gradient.structurallyEditable,
    }, commit);
  }

  function isOpaqueProperty(property: string) {
    return Object.prototype.hasOwnProperty.call(background.opaqueProperties, property);
  }
</script>

<InspectorSection title={t("inspector-background-title")} {hasValues}>
  {#snippet icon()}<IconLayersIntersect size={13} stroke={1.7} />{/snippet}

  {#if background.shorthand}
    <div class="compatibility-note">
      <strong>{t("inspector-background-shorthand-title")}</strong>
      <span>{t("inspector-background-shorthand-description")}</span>
    </div>
    <TextWithOptions value={getValue("background")} {...edit.continuous("background")} />
  {:else}
    <div class="field-label">{t("inspector-background-base-color")}</div>
    <ColorInput
      property="background-color"
      value={background.color ?? ""}
      suggestions={variablesForProperty("background-color", scssVariables)}
      oninput={(value) => emitSelected({ ...background, color: value || null }, ["background-color"])}
      oncommit={(value) => emitSelected({ ...background, color: value || null }, ["background-color"], true)}
      oncancel={() => edit.cancel("background-color")}
    />

    {#if structuralChangesBlocked}
      <div class="compatibility-note compact">
        <strong>{t("inspector-background-dynamic-lists-title")}</strong>
        <span>{t("inspector-background-dynamic-lists-description")}</span>
      </div>
      {#each Object.entries(background.opaqueProperties) as [property, value] (property)}
        <div class="field-label"><code>{property}</code></div>
        <TextWithOptions value={value} {...edit.continuous(property)} />
      {/each}
    {/if}

    <div class="layers-heading">
      <div>
        <strong>{t("inspector-background-layers")}</strong>
        <span>{t("inspector-background-layer-order")}</span>
      </div>
      <div class="add-actions">
        <button type="button" disabled={structuralChangesBlocked} title={t("inspector-background-add-image")} onclick={() => addLayer("image")}><IconPhoto size={13} /><span>{t("inspector-background-image")}</span></button>
        <button type="button" disabled={structuralChangesBlocked} title={t("inspector-background-add-gradient")} onclick={() => addLayer("gradient")}><IconPlus size={13} /><span>{t("inspector-background-gradient")}</span></button>
      </div>
    </div>

    {#if !background.layers.length}
      <button type="button" class="empty-layers" disabled={structuralChangesBlocked} onclick={() => addLayer("gradient")}>
        <IconLayersIntersect size={18} stroke={1.5} />
        <span>{t("inspector-background-empty")}</span>
      </button>
    {/if}

    <div class="layer-list">
      {#each background.layers as layer, index (layer.id)}
        <article class="layer-card" class:active={activeLayerId === layer.id}>
          <button type="button" class="layer-summary" aria-expanded={activeLayerId === layer.id} onclick={() => activeLayerId = activeLayerId === layer.id ? null : layer.id}>
            <span class="layer-index">{index + 1}</span>
            <span class="layer-swatch" style:background={layer.kind === "gradient" ? layer.source : layer.kind === "image" ? "var(--surface-4)" : "var(--surface-inset)"}>
              {#if layer.kind === "image"}<IconPhoto size={12} />{/if}
            </span>
            <span class="layer-title"><strong>{sourceLabel(layer)}</strong><small>{layer.kind}</small></span>
          </button>
          <div class="layer-actions">
            <button type="button" disabled={structuralChangesBlocked || index === 0} title={t("inspector-background-move-up")} aria-label={t("inspector-background-move-up")} onclick={() => moveLayer(layer.id, -1)}><IconArrowUp size={12} /></button>
            <button type="button" disabled={structuralChangesBlocked || index === background.layers.length - 1} title={t("inspector-background-move-down")} aria-label={t("inspector-background-move-down")} onclick={() => moveLayer(layer.id, 1)}><IconArrowDown size={12} /></button>
            <button type="button" disabled={structuralChangesBlocked} title={t("inspector-duplicate")} aria-label={t("inspector-duplicate")} onclick={() => duplicateLayer(layer)}><IconCopy size={12} /></button>
            <button type="button" disabled={structuralChangesBlocked} class="danger" title={t("inspector-delete")} aria-label={t("inspector-delete")} onclick={() => removeLayer(layer.id)}><IconTrash size={12} /></button>
          </div>

          {#if activeLayerId === layer.id}
            <div class="layer-editor">
              {#if layer.kind === "gradient" && layer.gradient}
                <GradientEditor
                  gradient={layer.gradient}
                  oninput={(gradient) => patchGradient(layer, gradient)}
                  oncommit={(gradient) => patchGradient(layer, gradient, true)}
                  oncancel={() => edit.cancelMany(BACKGROUND_LONGHAND_PROPERTIES)}
                  onsourceinput={(source) => patchLayerSource(layer, source)}
                  onsourcecommit={(source) => patchLayerSource(layer, source, true)}
                />
              {:else if layer.kind === "image" && /^url\(/i.test(layer.source.trim())}
                <div class="field-label">{t("inspector-background-image-source")}</div>
                <AssetPicker
                  value={imageUrl(layer.source)}
                  assets={imageAssets}
                  assetUrl={projectAssetPublicUrl}
                  assetMeta={projectAssetOriginLabel}
                  contextKey={layer.id}
                  oninput={(value) => patchLayerSource(layer, urlSource(value))}
                  oncommit={(value) => patchLayerSource(layer, urlSource(value), true)}
                  oncancel={() => edit.cancelMany(BACKGROUND_LONGHAND_PROPERTIES)}
                />
              {:else}
                <div class="compatibility-note compact">
                  <strong>{t("inspector-background-opaque-layer")}</strong>
                  <span>{t("inspector-background-opaque-description")}</span>
                </div>
                <TextWithOptions
                  value={layer.source}
                  oninput={(value) => patchLayerSource(layer, value)}
                  oncommit={(value) => patchLayerSource(layer, value, true)}
                  oncancel={() => edit.cancelMany(BACKGROUND_LONGHAND_PROPERTIES)}
                />
              {/if}

              <div class="two-fields">
                <div>
                  <div class="field-label">{t("inspector-background-position")}</div>
                  <TextWithOptions
                    label="P"
                    disabled={isOpaqueProperty("background-position")}
                    value={layer.position}
                    options={["center", "top", "right", "bottom", "left", "top left", "top right", "bottom left", "bottom right", "50% 50%"]}
                    oninput={(value) => patchLayer(layer.id, { position: value })}
                    oncommit={(value) => patchLayer(layer.id, { position: value }, true)}
                    oncancel={() => edit.cancelMany(BACKGROUND_LONGHAND_PROPERTIES)}
                  />
                </div>
                <div>
                  <div class="field-label">{t("inspector-background-size")}</div>
                  <TextWithOptions
                    label="S"
                    disabled={isOpaqueProperty("background-size")}
                    value={layer.size}
                    options={["auto", "cover", "contain", "100% 100%", "50%", "50% 50%"]}
                    oninput={(value) => patchLayer(layer.id, { size: value })}
                    oncommit={(value) => patchLayer(layer.id, { size: value }, true)}
                    oncancel={() => edit.cancelMany(BACKGROUND_LONGHAND_PROPERTIES)}
                  />
                </div>
              </div>

              <div class="two-fields">
                <div>
                  <div class="field-label">{t("inspector-background-repeat")}</div>
                  <SelectControl disabled={isOpaqueProperty("background-repeat")} value={layer.repeat} options={REPEATS.map((value) => ({ value, label: value }))} ariaLabel={t("inspector-background-repeat")} onchange={(value) => patchLayer(layer.id, { repeat: value }, true)} />
                </div>
                <div>
                  <div class="field-label">{t("inspector-background-attachment")}</div>
                  <SelectControl disabled={isOpaqueProperty("background-attachment")} value={layer.attachment} options={ATTACHMENTS.map((value) => ({ value, label: value }))} ariaLabel={t("inspector-background-attachment")} onchange={(value) => patchLayer(layer.id, { attachment: value }, true)} />
                </div>
              </div>

              <div class="two-fields">
                <div>
                  <div class="field-label">{t("inspector-background-origin")}</div>
                  <SelectControl disabled={isOpaqueProperty("background-origin")} value={layer.origin} options={BOXES.slice(0, 3).map((value) => ({ value, label: value }))} ariaLabel={t("inspector-background-origin")} onchange={(value) => patchLayer(layer.id, { origin: value }, true)} />
                </div>
                <div>
                  <div class="field-label">{t("inspector-background-clip")}</div>
                  <SelectControl disabled={isOpaqueProperty("background-clip")} value={layer.clip} options={BOXES.map((value) => ({ value, label: value }))} ariaLabel={t("inspector-background-clip")} onchange={(value) => patchLayer(layer.id, { clip: value }, true)} />
                </div>
              </div>

              <div class="field-label">{t("inspector-background-blend-mode")}</div>
              <SelectControl disabled={isOpaqueProperty("background-blend-mode")} value={layer.blendMode} options={BLEND_MODES.map((value) => ({ value, label: value }))} ariaLabel={t("inspector-background-blend-mode")} onchange={(value) => patchLayer(layer.id, { blendMode: value }, true)} />
            </div>
          {/if}
        </article>
      {/each}
    </div>

    {#if !background.structurallyEditable && background.layers.length}
      <p class="preservation-note">{t("inspector-background-preservation-note")}</p>
    {/if}
  {/if}
</InspectorSection>

<style>
  .field-label { margin-top: 1px; color: var(--text-muted); font-size: 11px; }
  .compatibility-note { display: flex; flex-direction: column; gap: 3px; padding: 8px; border: 1px solid color-mix(in srgb, var(--warning, #b7791f) 35%, var(--border-subtle)); border-radius: 8px; background: color-mix(in srgb, var(--warning, #b7791f) 7%, transparent); font-size: 11px; line-height: 1.35; }
  .compatibility-note span, .preservation-note { color: var(--text-muted); }
  .compatibility-note.compact { padding: 6px; }
  .layers-heading { display: flex; align-items: flex-end; justify-content: space-between; gap: 6px; }
  .layers-heading > div:first-child { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .layers-heading strong { font-size: 12px; }
  .layers-heading span { color: var(--text-muted); font-size: 11px; }
  .add-actions { display: flex; gap: 4px; }
  .add-actions button, .layer-actions button {
    display: inline-flex; align-items: center; justify-content: center; gap: 3px; min-height: 24px;
    border: 1px solid var(--border-4); border-radius: 6px; background: var(--surface-8); color: var(--text-muted); cursor: pointer;
  }
  .add-actions button { padding: 0 6px; font-size: 11px; }
  .add-actions button:hover, .layer-actions button:hover { color: var(--text); border-color: var(--brand); }
  .add-actions button:disabled { opacity: .38; cursor: not-allowed; }
  .empty-layers { display: flex; align-items: center; justify-content: center; gap: 6px; min-height: 52px; border: 1px dashed var(--border-4); border-radius: 8px; background: var(--surface-4); color: var(--text-muted); cursor: pointer; font-size: 11px; }
  .empty-layers:hover { color: var(--brand); border-color: var(--brand); }
  .layer-list { display: flex; flex-direction: column; gap: 6px; }
  .layer-card { display: grid; grid-template-columns: minmax(0, 1fr) auto; border: 1px solid var(--border-subtle); border-radius: 9px; background: var(--surface-4); overflow: hidden; }
  .layer-card.active { border-color: color-mix(in srgb, var(--brand) 55%, var(--border-subtle)); }
  .layer-summary { display: grid; grid-template-columns: 17px 24px minmax(0, 1fr); align-items: center; gap: 5px; min-width: 0; padding: 6px; border: none; background: transparent; color: var(--text); text-align: left; cursor: pointer; }
  .layer-index { color: var(--text-muted); font: 11px "JetBrains Mono", monospace; text-align: center; }
  .layer-swatch { display: flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: 1px solid var(--border-4); border-radius: 5px; color: var(--text-muted); }
  .layer-title { display: flex; flex-direction: column; min-width: 0; }
  .layer-title strong { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .layer-title small { color: var(--text-muted); font-size: 11px; text-transform: uppercase; }
  .layer-actions { display: flex; align-items: center; gap: 2px; padding: 4px 5px 4px 0; }
  .layer-actions button { width: 22px; min-height: 22px; padding: 0; }
  .layer-actions button:disabled { opacity: .3; cursor: not-allowed; }
  .layer-actions .danger:hover { color: var(--danger, #c0392b); border-color: var(--danger, #c0392b); }
  .layer-editor { grid-column: 1 / -1; display: flex; flex-direction: column; gap: 7px; padding: 8px; border-top: 1px solid var(--border-subtle); background: var(--surface-8); }
  .two-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .two-fields > div { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .preservation-note { margin: 0; font-size: 11px; line-height: 1.4; }
</style>
