<script lang="ts">
  import { IconSparkles } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import TextWithOptions from "../controls/TextWithOptions.svelte";
  import AssetPicker from "../controls/AssetPicker.svelte";
  import {
    projectAssetOriginLabel,
    projectAssetPublicUrl,
  } from "$lib/project/assets";
  import type {
    ProjectFile as InspectorProjectFile,
    ScssVariable as InspectorScssVariable,
  } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    scannedAssets = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: InspectorScssVariable[];
    scannedAssets?: InspectorProjectFile[];
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = [
    "opacity", "mix-blend-mode", "clip-path",
    "filter", "backdrop-filter",
    "mask-image", "mask-size", "mask-repeat", "mask-position",
  ];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  // ── Options ──────────────────────────────────────────────────────────────

  const BLEND_MODES = [
    "normal", "multiply", "screen", "overlay", "darken", "lighten",
    "color-dodge", "color-burn", "hard-light", "soft-light",
    "difference", "exclusion", "hue", "saturation", "color", "luminosity",
  ];

  const CLIP_PATH_OPTS = [
    "none",
    "circle(50%)",
    "circle(50% at 50% 50%)",
    "ellipse(50% 30% at 50% 50%)",
    "inset(10px)",
    "inset(10% 20%)",
    "inset(10px round 5px)",
    "polygon(50% 0%, 100% 100%, 0% 100%)",
    "polygon(0 0, 100% 0, 100% 100%, 0 100%)",
    "polygon(25% 0%, 75% 0%, 100% 50%, 75% 100%, 25% 100%, 0% 50%)",
  ];

  const FILTER_FNS = [
    { name: "Blur",         fn: "blur(4px)"                                },
    { name: "Brightness",   fn: "brightness(1.2)"                          },
    { name: "Contrast",     fn: "contrast(1.2)"                            },
    { name: "Drop Shadow",  fn: "drop-shadow(2px 4px 6px rgba(0,0,0,0.3))" },
    { name: "Grayscale",    fn: "grayscale(100%)"                          },
    { name: "Hue Rotate",   fn: "hue-rotate(90deg)"                        },
    { name: "Invert",       fn: "invert(100%)"                             },
    { name: "Opacity",      fn: "opacity(0.8)"                             },
    { name: "Saturate",     fn: "saturate(2)"                              },
    { name: "Sepia",        fn: "sepia(100%)"                              },
  ];

  const MASK_SIZE_OPTS   = ["auto", "cover", "contain", "100%", "50%"];
  const MASK_REPEAT_OPTS = ["no-repeat", "repeat", "repeat-x", "repeat-y", "space", "round"];
  const MASK_POS_OPTS    = [
    "center", "top", "bottom", "left", "right",
    "top left", "top right", "bottom left", "bottom right",
  ];

  // ── Asset helpers ─────────────────────────────────────────────────────────

  const ext = (a: InspectorProjectFile) => a.relativePath.split(".").pop()?.toLowerCase() ?? "";
  const imageAssets = $derived(scannedAssets.filter(
    (a) => a.kind === "IMAGE" || ["svg","webp","avif","png","jpg","jpeg","gif"].includes(ext(a))
  ));

  function maskImageValue(raw: string) {
    const stripped = raw.trim().replace(/^url\(["']?/, "").replace(/["']?\)$/, "");
    return stripped ? `url("${stripped}")` : "";
  }

  const maskImageUrl = $derived.by(() => {
    const v = getValue("mask-image");
    const m = v.match(/^url\(["']?(.*?)["']?\)$/);
    return m ? m[1] : v;
  });

  // ── Popovers ──────────────────────────────────────────────────────────────

  let filterBtnRef   = $state<HTMLButtonElement | null>(null);
  let bdFilterBtnRef = $state<HTMLButtonElement | null>(null);
  let showFilter   = $state(false);
  let showBdFilter = $state(false);
  let filterPos   = $state({ top: 0, left: 0, width: 220 });
  let bdFilterPos = $state({ top: 0, left: 0, width: 220 });

  function calcPos(btn: HTMLButtonElement | null, width = 220) {
    if (!btn) return { top: 0, left: 0, width };
    const rect = btn.getBoundingClientRect();
    const left = Math.max(8, Math.min(rect.right - width, window.innerWidth - width - 8));
    const spaceBelow = window.innerHeight - rect.bottom - 8;
    const spaceAbove = rect.top - 8;
    const top = spaceBelow >= 180 || spaceBelow >= spaceAbove
      ? rect.bottom + 4
      : Math.max(8, rect.top - Math.min(240, spaceAbove) - 4);
    return { top, left, width };
  }

  function openFilter() {
    showBdFilter = false;
    if (showFilter) { showFilter = false; return; }
    filterPos = calcPos(filterBtnRef);
    showFilter = true;
  }

  function openBdFilter() {
    showFilter = false;
    if (showBdFilter) { showBdFilter = false; return; }
    bdFilterPos = calcPos(bdFilterBtnRef);
    showBdFilter = true;
  }

  function closeAll() { showFilter = false; showBdFilter = false; }

  function addFilter(fn: string) {
    const current = getValue("filter").trim();
    const next = (!current || current === "none") ? fn : `${current} ${fn}`;
    edit.commit("filter", next);
    closeAll();
  }

  function addBdFilter(fn: string) {
    const current = getValue("backdrop-filter").trim();
    const next = (!current || current === "none") ? fn : `${current} ${fn}`;
    edit.commit("backdrop-filter", next);
    closeAll();
  }
</script>

<!-- Backdrop -->
{#if showFilter || showBdFilter}
  <div class="effects-backdrop" role="presentation" onmousedown={closeAll}></div>
{/if}

<!-- Filter popover -->
{#if showFilter}
  <div
    class="effects-popover"
    role="listbox"
    style="top:{filterPos.top}px; left:{filterPos.left}px; width:{filterPos.width}px;"
  >
    {#each FILTER_FNS as f}
      <button
        type="button"
        class="effects-option"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => addFilter(f.fn)}
      >
        <span class="effects-opt-name">{f.name}</span>
        <span class="effects-opt-val">{f.fn}</span>
      </button>
    {/each}
  </div>
{/if}

<!-- Backdrop-filter popover -->
{#if showBdFilter}
  <div
    class="effects-popover"
    role="listbox"
    style="top:{bdFilterPos.top}px; left:{bdFilterPos.left}px; width:{bdFilterPos.width}px;"
  >
    {#each FILTER_FNS as f}
      <button
        type="button"
        class="effects-option"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => addBdFilter(f.fn)}
      >
        <span class="effects-opt-name">{f.name}</span>
        <span class="effects-opt-val">{f.fn}</span>
      </button>
    {/each}
  </div>
{/if}

<InspectorSection title="Effects" {hasValues}>
  {#snippet icon()}<IconSparkles size={13} stroke={1.7} />{/snippet}

  <!-- Opacity -->
  <div class="row-label">Opacity</div>
  <PropInput
    label="Op"
    value={getValue("opacity")}
    placeholder="1"
    {...edit.continuous("opacity")}
  />

  <!-- Mix Blend Mode -->
  <div class="row-label" style="margin-top: 4px;">Mix Blend Mode</div>
  <TextWithOptions
    value={getValue("mix-blend-mode")}
    placeholder="normal"
    options={BLEND_MODES}
    {...edit.continuous("mix-blend-mode")}
  />

  <!-- Clip Path -->
  <div class="row-label" style="margin-top: 4px;">Clip Path</div>
  <TextWithOptions
    value={getValue("clip-path")}
    placeholder="none"
    options={CLIP_PATH_OPTS}
    {...edit.continuous("clip-path")}
  />

  <!-- Filter -->
  <div class="effects-subheader" style="margin-top: 6px;">
    <span class="effects-label" class:has-value={getValue("filter") !== ""}>FILTER</span>
    <button
      bind:this={filterBtnRef}
      type="button"
      class="add-btn"
      class:active={showFilter}
      title="Adaugă funcție filter"
      onclick={openFilter}
    >+</button>
  </div>
  <PropInput
    value={getValue("filter")}
    placeholder="none"
    {...edit.continuous("filter")}
  />

  <!-- Backdrop Filter -->
  <div class="effects-subheader" style="margin-top: 4px;">
    <span class="effects-label" class:has-value={getValue("backdrop-filter") !== ""}>BACKDROP FILTER</span>
    <button
      bind:this={bdFilterBtnRef}
      type="button"
      class="add-btn"
      class:active={showBdFilter}
      title="Adaugă funcție backdrop-filter"
      onclick={openBdFilter}
    >+</button>
  </div>
  <PropInput
    value={getValue("backdrop-filter")}
    placeholder="none"
    {...edit.continuous("backdrop-filter")}
  />

  <!-- Mask -->
  <div class="effects-subheader" style="margin-top: 6px;">
    <span class="effects-label" class:has-value={getValue("mask-image") !== ""}>MASK</span>
  </div>
  <div class="row-label">Mask Image</div>
  <AssetPicker
    value={maskImageUrl}
    assets={imageAssets}
    assetUrl={projectAssetPublicUrl}
    assetMeta={projectAssetOriginLabel}
    oninput={(value) => edit.draft("mask-image", maskImageValue(value))}
    oncommit={(value) => edit.commit("mask-image", maskImageValue(value))}
    oncancel={() => edit.cancel("mask-image")}
  />

  <div class="row-2 label-row" style="margin-top: 4px;">
    <span class="row-label">Mask Size</span>
    <span class="row-label">Mask Repeat</span>
  </div>
  <div class="row-2">
    <TextWithOptions
      value={getValue("mask-size")}
      placeholder="auto"
      options={MASK_SIZE_OPTS}
      {...edit.continuous("mask-size")}
    />
    <TextWithOptions
      value={getValue("mask-repeat")}
      placeholder="no-repeat"
      options={MASK_REPEAT_OPTS}
      {...edit.continuous("mask-repeat")}
    />
  </div>

  <div class="row-label" style="margin-top: 2px;">Mask Position</div>
  <TextWithOptions
    value={getValue("mask-position")}
    placeholder="center"
    options={MASK_POS_OPTS}
    {...edit.continuous("mask-position")}
  />
</InspectorSection>

<style>
  .effects-subheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .effects-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .effects-label.has-value {
    color: var(--brand-strong);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-3);
    border-radius: 4px;
    background: var(--surface-4);
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: color 80ms, border-color 80ms, background 80ms;
  }

  .add-btn:hover, .add-btn.active {
    border-color: var(--brand);
    background: var(--brand-soft);
    color: var(--brand-strong);
  }

  .row-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .label-row { align-items: center; }

  /* ── Backdrop ──────────────────────────────────────────────────────────── */

  .effects-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  /* ── Popover ───────────────────────────────────────────────────────────── */

  .effects-popover {
    position: fixed;
    z-index: 1000;
    overflow-y: auto;
    max-height: 280px;
    padding: 4px;
    border: 1px solid var(--border-4);
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
  }

  .effects-option {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    padding: 6px 9px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    cursor: pointer;
    text-align: left;
    transition: background 80ms;
  }

  .effects-option:hover {
    background: var(--brand-soft);
  }

  .effects-opt-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }

  .effects-opt-val {
    font-size: 10px;
    font-family: "JetBrains Mono", monospace;
    color: var(--text-muted);
  }
</style>
