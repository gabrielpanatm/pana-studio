<script lang="ts">
  import type { ProjectFile, ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
  import { variablesForProperty } from "$lib/editor/controls";
  import {
    DEFAULT_BACKGROUND_GRADIENT,
    createDefaultGradientState,
    createGradientStop,
    gradientPositionFromClientX,
    gradientPreviewBackground,
    gradientStopAppearanceAtPosition,
    isBackgroundGradientStructurallyEditable,
    parseBackgroundGradient,
    serializeBackgroundGradient,
    type GradientState,
    type GradientStop,
    type GradientType,
  } from "$lib/inspector/background-gradient";
  import { IconPalette, IconTrash } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import ColorInput from "../controls/ColorInput.svelte";
  import SegmentedControl from "../controls/SegmentedControl.svelte";
  import TextWithOptions from "../controls/TextWithOptions.svelte";
  import AssetPicker from "../controls/AssetPicker.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import {
    projectAssetOriginLabel,
    projectAssetPublicUrl,
  } from "$lib/project/assets";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    scannedAssets = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    scannedAssets?: ProjectFile[];
    edit: CssPropertyEditController;
  } = $props();

  // Asset helpers for BG Image
  const ext = (a: ProjectFile) => a.relativePath.split(".").pop()?.toLowerCase() ?? "";
  const imageAssets = $derived(scannedAssets.filter(
    (a) => a.kind === "IMAGE" || ["svg","webp","avif","png","jpg","jpeg","gif"].includes(ext(a))
  ));

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const TRACKED = [
    "color", "background-color", "background-image",
    "background-size", "background-repeat", "background-position",
    "background-attachment", "background-blend-mode", "background-clip",
  ];
  const hasValues = $derived(TRACKED.some((p) => getValue(p) !== ""));

  // ── Background type ──────────────────────────────────────────────────────

  type BgType = "color" | "image" | "gradient";

  const bgType = $derived.by<BgType>(() => {
    const v = getValue("background-image").trim();
    if (!v || v === "none") return "color";
    if (/^url\(/i.test(v)) return "image";
    if (/gradient/i.test(v)) return "gradient";
    return "color";
  });

  const BG_TYPE_OPTS = [
    { value: "color",    label: "Color"    },
    { value: "image",    label: "Image"    },
    { value: "gradient", label: "Gradient" },
  ];

  function setBgType(next: string) {
    if (next === "color") {
      edit.commit("background-image", "none");
    } else if (next === "image") {
      if (bgType !== "image") edit.commit("background-image", 'url("")');
    } else if (next === "gradient") {
      if (bgType !== "gradient") {
        initGradient(DEFAULT_BACKGROUND_GRADIENT);
        edit.commit("background-image", DEFAULT_BACKGROUND_GRADIENT);
      }
    }
  }

  // ── Gradient state ───────────────────────────────────────────────────────

  let gradientState  = $state<GradientState>(createDefaultGradientState());
  let activeStopId   = $state<number | null>(null);
  let lastEmittedGrad = "";

  function initGradient(css: string) {
    gradientState = parseBackgroundGradient(css);
    lastEmittedGrad = css;
    if (activeStopId === null && gradientState.stops.length > 0) {
      activeStopId = gradientState.stops[0].id;
    }
  }

  // Re-parse when background-image changes externally (undo/redo/snapshot)
  $effect(() => {
    const v = getValue("background-image");
    if (!/gradient/i.test(v)) return;
    if (v === lastEmittedGrad) return;
    gradientState = parseBackgroundGradient(v);
    lastEmittedGrad = v;
    if (!gradientState.stops.find((s) => s.id === activeStopId)) {
      activeStopId = gradientState.stops[0]?.id ?? null;
    }
  });

  function emitGradient(commit = false) {
    const css = serializeBackgroundGradient(gradientState);
    lastEmittedGrad = css;
    if (commit) edit.commit("background-image", css);
    else edit.draft("background-image", css);
  }

  function patchGradient(patch: Partial<GradientState>, commit = false) {
    gradientState = { ...gradientState, ...patch };
    emitGradient(commit);
  }

  function patchStop(id: number, patch: Partial<GradientStop>, commit = false) {
    gradientState = {
      ...gradientState,
      stops: gradientState.stops.map((s) => (s.id === id ? { ...s, ...patch } : s)),
    };
    emitGradient(commit);
  }

  function removeStop(id: number) {
    if (gradientState.stops.length <= 2) return;
    const remaining = gradientState.stops.filter((s) => s.id !== id);
    gradientState = { ...gradientState, stops: remaining };
    if (activeStopId === id) activeStopId = remaining[0]?.id ?? null;
    emitGradient(true);
  }

  // ── Gradient bar drag ────────────────────────────────────────────────────

  let barEl    = $state<HTMLElement | null>(null);
  let draggingId = $state<number | null>(null);

  function onBarClick(e: MouseEvent) {
    if (!barEl) return;
    const tgt = e.target as HTMLElement;
    if (tgt.closest(".g-stop")) return;
    const rect = barEl.getBoundingClientRect();
    const pos = gradientPositionFromClientX(e.clientX, rect);
    const { color, opacity } = gradientStopAppearanceAtPosition(gradientState.stops, pos);
    const ns = createGradientStop(color, opacity, pos);
    gradientState = {
      ...gradientState,
      stops: [...gradientState.stops, ns].sort((a, b) => a.position - b.position),
    };
    activeStopId = ns.id;
    emitGradient(true);
  }

  function onStopPointerDown(e: PointerEvent, id: number) {
    e.preventDefault();
    e.stopPropagation();
    activeStopId = id;
    draggingId   = id;
    let stopExternalReconcileBarrier = () => {};

    function onMove(me: PointerEvent) {
      if (!barEl) return;
      const rect = barEl.getBoundingClientRect();
      const pos = gradientPositionFromClientX(me.clientX, rect);
      gradientState = {
        ...gradientState,
        stops: gradientState.stops.map((s) => (s.id === id ? { ...s, position: pos } : s)),
      };
      emitGradient();
    }

    function finishDrag(action: "commit" | "cancel") {
      draggingId = null;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
      stopExternalReconcileBarrier();
      if (action === "commit") edit.commit("background-image");
      else edit.cancel("background-image");
    }

    function onUp() {
      finishDrag("commit");
    }

    function onCancel() {
      finishDrag("cancel");
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
    stopExternalReconcileBarrier = listenForExternalReconcileInteractionBarrier(() => finishDrag("commit"));
  }

  const barCss = $derived.by(() => {
    return gradientPreviewBackground(gradientState.stops);
  });

  const activeStop = $derived(gradientState.stops.find((s) => s.id === activeStopId) ?? null);
  const gradientStructured = $derived(isBackgroundGradientStructurallyEditable(getValue("background-image")));

  // ── Dropdowns ────────────────────────────────────────────────────────────

  const BLEND_MODES = [
    "normal","multiply","screen","overlay","darken","lighten",
    "color-dodge","color-burn","hard-light","soft-light",
    "difference","exclusion","hue","saturation","color","luminosity",
  ];
  const BG_CLIPS    = ["border-box","padding-box","content-box","text"];
  const BG_REPEATS  = ["repeat","no-repeat","repeat-x","repeat-y","space","round"];
  const BG_ATTACHES = ["scroll","fixed","local"];
  const GRAD_TYPES  = ["linear","radial","conic"];

  // BG image URL extraction / construction
  const bgImgUrl = $derived.by(() => {
    const v = getValue("background-image");
    const m = v.match(/^url\(["']?(.*?)["']?\)$/i);
    return m ? m[1] : "";
  });

  function backgroundImageValue(raw: string) {
    const value = raw.trim();
    return value ? `url("${value}")` : 'url("")';
  }
</script>

<InspectorSection title="Colors" {hasValues}>
  {#snippet icon()}<IconPalette size={13} stroke={1.7} />{/snippet}

  <!-- Text Color -->
  <div class="row-label">Text Color</div>
  <ColorInput
    property="color"
    value={getValue("color")}
    suggestions={variablesForProperty("color", scssVariables)}
    {...edit.continuous("color")}
  />

  <!-- Background Color -->
  <div class="row-label">Background Color</div>
  <ColorInput
    property="background-color"
    value={getValue("background-color")}
    suggestions={variablesForProperty("background-color", scssVariables)}
    {...edit.continuous("background-color")}
  />

  <!-- Background Type -->
  <div class="row-label">Background Type</div>
  <SegmentedControl
    options={BG_TYPE_OPTS}
    value={bgType}
    toggleable={false}
    onchange={setBgType}
  />

  <!-- ── IMAGE ────────────────────────────────────────────────────────── -->
  {#if bgType === "image"}
    <div class="row-label">Background Image</div>
    <AssetPicker
      value={bgImgUrl}
      assets={imageAssets}
      assetUrl={projectAssetPublicUrl}
      assetMeta={projectAssetOriginLabel}
      oninput={(value) => edit.draft("background-image", backgroundImageValue(value))}
      oncommit={(value) => edit.commit("background-image", backgroundImageValue(value))}
      oncancel={() => edit.cancel("background-image")}
    />

    <div class="row-2">
      <div class="col">
        <div class="row-label">BG Size</div>
        <TextWithOptions
          label="S"
          value={getValue("background-size")}
          placeholder="auto"
          options={["auto","cover","contain","100%","100% 100%","50%","50% 50%","75%","75% 75%"]}
          {...edit.continuous("background-size")}
        />
      </div>
      <div class="col">
        <div class="row-label">BG Repeat</div>
        <SelectControl
          value={getValue("background-repeat")}
          placeholder="repeat (implicit)"
          options={[{ value: "", label: "— implicit (repeat)" }, ...BG_REPEATS.map((value) => ({ value, label: value }))]}
          ariaLabel="Background repeat"
          onchange={(value) => edit.commit("background-repeat", value)}
        />
      </div>
    </div>

    <div class="row-2">
      <div class="col">
        <div class="row-label">BG Position</div>
        <TextWithOptions
          label="P"
          value={getValue("background-position")}
          placeholder="0% 0%"
          options={[
            "center","top","bottom","left","right",
            "top left","top right","top center",
            "bottom left","bottom right","bottom center",
            "center left","center right","center center",
            "0% 0%","50% 50%","100% 100%","0% 50%","50% 0%",
          ]}
          {...edit.continuous("background-position")}
        />
      </div>
      <div class="col">
        <div class="row-label">BG Attach</div>
        <SelectControl
          value={getValue("background-attachment")}
          placeholder="scroll (implicit)"
          options={[{ value: "", label: "— implicit (scroll)" }, ...BG_ATTACHES.map((value) => ({ value, label: value }))]}
          ariaLabel="Background attachment"
          onchange={(value) => edit.commit("background-attachment", value)}
        />
      </div>
    </div>
  {/if}

  <!-- ── GRADIENT ─────────────────────────────────────────────────────── -->
  {#if bgType === "gradient"}
    <div class="row-label">Gradient</div>
    {#if gradientStructured}
    <div class="grad-header">
      <SelectControl
        value={gradientState.type}
        options={GRAD_TYPES.map((type) => ({ value: type, label: type.charAt(0).toUpperCase() + type.slice(1) }))}
        ariaLabel="Gradient type"
        onchange={(value) => patchGradient({ type: value as GradientType }, true)}
      />
      {#if gradientState.type === "linear" || gradientState.type === "conic"}
        <input
          class="angle-input"
          type="number"
          min="-360"
          max="360"
          value={gradientState.angle}
          oninput={(e) => patchGradient({ angle: parseInt(e.currentTarget.value) || 0 })}
          onchange={() => edit.commit("background-image")}
          onkeydown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              edit.cancel("background-image");
              event.currentTarget.blur();
            }
          }}
        />
        <span class="angle-unit">deg</span>
      {/if}
    </div>

    <!-- Gradient bar -->
    <div
      class="grad-bar"
      style="background: {barCss}"
      bind:this={barEl}
      role="presentation"
      onclick={onBarClick}
    >
      {#each gradientState.stops as stop (stop.id)}
        <button
          type="button"
          class="g-stop"
          class:active={activeStopId === stop.id}
          style="left: {stop.position}%; background: {stop.color}; opacity: {stop.opacity / 100};"
          onpointerdown={(e) => onStopPointerDown(e, stop.id)}
          aria-label="Stop la {stop.position}%"
        ></button>
      {/each}
    </div>

    <!-- Active stop editor -->
    {#if activeStop}
      <div class="stop-editor">
        <div class="stop-color">
          <ColorInput
            property="gradient-stop"
            value={activeStop.color}
            oninput={(value) => patchStop(activeStop.id, { color: value })}
            oncommit={() => edit.commit("background-image")}
            oncancel={() => edit.cancel("background-image")}
          />
        </div>
        <div class="stop-opacity-field">
          <input
            class="opacity-input"
            type="number"
            min="0"
            max="100"
            value={activeStop.opacity}
            oninput={(e) => patchStop(activeStop.id, { opacity: Math.max(0, Math.min(100, parseInt(e.currentTarget.value) || 0)) })}
            onchange={() => edit.commit("background-image")}
            onkeydown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                edit.cancel("background-image");
                event.currentTarget.blur();
              }
            }}
          />
          <span class="opacity-unit">%</span>
        </div>
        {#if gradientState.stops.length > 2}
          <button
            type="button"
            class="stop-del"
            title="Șterge stop"
            onclick={() => removeStop(activeStop.id)}
          >
            <IconTrash size={11} stroke={1.8} />
          </button>
        {/if}
      </div>
      <div class="stop-pos-row">
        <span class="row-label" style="margin:0">Poziție</span>
        <div class="stop-pos-input">
          <input
            class="opacity-input"
            type="number"
            min="0"
            max="100"
            value={activeStop.position}
            oninput={(e) => patchStop(activeStop.id, { position: Math.max(0, Math.min(100, parseInt(e.currentTarget.value) || 0)) })}
            onchange={() => edit.commit("background-image")}
            onkeydown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                edit.cancel("background-image");
                event.currentTarget.blur();
              }
            }}
          />
          <span class="opacity-unit">%</span>
        </div>
      </div>
    {/if}
    {:else}
      <p class="complex-value-note">Gradientul folosește o sintaxă care nu poate fi proiectată fără pierderi. Valoarea brută este păstrată.</p>
      <TextWithOptions
        value={getValue("background-image")}
        placeholder="linear-gradient(...)"
        options={[DEFAULT_BACKGROUND_GRADIENT]}
        {...edit.continuous("background-image")}
      />
    {/if}
  {/if}

  <!-- Blend Mode + BG Clip (image & gradient) -->
  {#if bgType === "image" || bgType === "gradient"}
    <div class="row-2">
      <div class="col">
        <div class="row-label">Blend Mode</div>
        <SelectControl
          value={getValue("background-blend-mode")}
          placeholder="normal (implicit)"
          options={[{ value: "", label: "— implicit (normal)" }, ...BLEND_MODES.map((value) => ({ value, label: value }))]}
          ariaLabel="Background blend mode"
          onchange={(value) => edit.commit("background-blend-mode", value)}
        />
      </div>
      <div class="col">
        <div class="row-label">BG Clip</div>
        <SelectControl
          value={getValue("background-clip")}
          placeholder="border-box (implicit)"
          options={[{ value: "", label: "— implicit (border-box)" }, ...BG_CLIPS.map((value) => ({ value, label: value }))]}
          ariaLabel="Background clip"
          onchange={(value) => edit.commit("background-clip", value)}
        />
      </div>
    </div>
  {/if}

</InspectorSection>

<style>
  .row-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .complex-value-note {
    margin: 0;
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.35;
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .row-2 > * { min-width: 0; overflow: hidden; }

  .col {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  /* ── Gradient header ───────────────────────────────────────────────────── */

  .grad-header {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .grad-header :global(.select-control-root) {
    flex: 1;
  }

  .angle-input {
    width: 46px;
    flex-shrink: 0;
    height: 26px;
    padding: 0 5px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
    color: var(--text);
    font-size: 11px;
    font-family: "JetBrains Mono", monospace;
    outline: none;
    text-align: right;
    transition: border-color 80ms;
  }

  .angle-input:focus { border-color: var(--brand); }

  .angle-unit {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  /* ── Gradient bar ──────────────────────────────────────────────────────── */

  .grad-bar {
    position: relative;
    height: 22px;
    border-radius: 6px;
    cursor: crosshair;
    border: 1px solid var(--border-4);
    overflow: visible;
  }

  .g-stop {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1.5px rgba(0, 0, 0, 0.4);
    cursor: grab;
    padding: 0;
    transition: box-shadow 80ms;
  }

  .g-stop:active { cursor: grabbing; }

  .g-stop.active {
    border-color: var(--brand);
    box-shadow: 0 0 0 2px var(--brand);
    z-index: 2;
  }

  /* ── Stop editor ───────────────────────────────────────────────────────── */

  .stop-editor {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .stop-color {
    flex: 1;
    min-width: 0;
  }

  .stop-opacity-field,
  .stop-pos-input {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .opacity-input {
    width: 40px;
    height: 26px;
    padding: 0 5px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-8);
    color: var(--text);
    font-size: 11px;
    font-family: "JetBrains Mono", monospace;
    text-align: right;
    outline: none;
    transition: border-color 80ms;
  }

  .opacity-input:focus { border-color: var(--brand); }

  .opacity-unit {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .stop-del {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 80ms, background 80ms, border-color 80ms;
  }

  .stop-del:hover {
    border-color: #cf4a4a;
    background: color-mix(in srgb, #cf4a4a 12%, transparent);
    color: #cf4a4a;
  }

  .stop-pos-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
</style>
