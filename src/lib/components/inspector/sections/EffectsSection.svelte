<script lang="ts">
  import { t } from "$lib/i18n/runtime.svelte";
  import { IconPlus, IconSparkles } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import TextWithOptions from "../controls/TextWithOptions.svelte";
  import AssetPicker from "../controls/AssetPicker.svelte";
  import {
    projectAssetOriginLabel,
    projectAssetPublicUrl,
  } from "$lib/project/assets";
  import type { ProjectFile as InspectorProjectFile } from "$lib/project/lifecycle-contract";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    calculateAnchoredPopoverPlacement,
    observeAnchoredPopoverPosition,
  } from "$lib/ui/anchored-popover";

  let {
    pendingValues,
    rulesMap,
    scannedAssets = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
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

  const FILTER_FNS = $derived([
    { name: t("inspector-filter-blur"), fn: "blur(4px)" },
    { name: t("inspector-filter-brightness"), fn: "brightness(1.2)" },
    { name: t("inspector-filter-contrast"), fn: "contrast(1.2)" },
    { name: t("inspector-filter-drop-shadow"), fn: "drop-shadow(2px 4px 6px rgba(0,0,0,0.3))" },
    { name: t("inspector-filter-grayscale"), fn: "grayscale(100%)" },
    { name: t("inspector-filter-hue-rotate"), fn: "hue-rotate(90deg)" },
    { name: t("inspector-filter-invert"), fn: "invert(100%)" },
    { name: t("inspector-filter-opacity"), fn: "opacity(0.8)" },
    { name: t("inspector-filter-saturate"), fn: "saturate(2)" },
    { name: t("inspector-filter-sepia"), fn: "sepia(100%)" },
  ]);

  const MASK_SIZE_OPTS   = ["auto", "cover", "contain", "100%", "50%"];
  const MASK_REPEAT_OPTS = ["no-repeat", "repeat", "repeat-x", "repeat-y", "space", "round"];
  const MASK_POS_OPTS    = [
    "center", "top", "bottom", "left", "right",
    "top left", "top right", "bottom left", "bottom right",
  ];

  // ── Asset helpers ─────────────────────────────────────────────────────────

  const imageAssets = $derived(scannedAssets.filter((asset) => asset.kind === "IMAGE"));

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
  let filterPos   = $state({ top: 0, left: 0, width: 220, maxHeight: 280 });
  let bdFilterPos = $state({ top: 0, left: 0, width: 220, maxHeight: 280 });

  function calcPos(btn: HTMLButtonElement | null) {
    if (!btn) return { top: 0, left: 0, width: 220, maxHeight: 280 };
    const rect = btn.getBoundingClientRect();
    return calculateAnchoredPopoverPlacement({
      anchorRect: rect,
      itemCount: FILTER_FNS.length,
      itemHeight: 42,
      preferredWidth: 220,
      horizontalAlign: "end",
      maxHeight: 280,
    });
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

  $effect(() => {
    const anchor = showFilter ? filterBtnRef : showBdFilter ? bdFilterBtnRef : null;
    if (!anchor) return;
    const update = () => {
      if (showFilter) filterPos = calcPos(filterBtnRef);
      if (showBdFilter) bdFilterPos = calcPos(bdFilterBtnRef);
    };
    return observeAnchoredPopoverPosition(anchor, update);
  });
</script>

<!-- Backdrop -->
{#if showFilter || showBdFilter}
  <div class="effects-backdrop" role="presentation" onmousedown={closeAll}></div>
{/if}

<!-- Filter popover -->
{#if showFilter}
  <div
    class="effects-popover ui-popover"
    role="listbox"
    style:top={`${filterPos.top}px`}
    style:left={`${filterPos.left}px`}
    style:width={`${filterPos.width}px`}
    style:max-height={`${filterPos.maxHeight}px`}
  >
    {#each FILTER_FNS as f}
      <button
        type="button"
        class="effects-option ui-option"
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
    class="effects-popover ui-popover"
    role="listbox"
    style:top={`${bdFilterPos.top}px`}
    style:left={`${bdFilterPos.left}px`}
    style:width={`${bdFilterPos.width}px`}
    style:max-height={`${bdFilterPos.maxHeight}px`}
  >
    {#each FILTER_FNS as f}
      <button
        type="button"
        class="effects-option ui-option"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => addBdFilter(f.fn)}
      >
        <span class="effects-opt-name">{f.name}</span>
        <span class="effects-opt-val">{f.fn}</span>
      </button>
    {/each}
  </div>
{/if}

<InspectorSection title={t("inspector-effects-title")} {hasValues}>
  {#snippet icon()}<IconSparkles size={13} stroke={1.7} />{/snippet}

  <!-- Opacity -->
  <div class="row-label">{t("inspector-effects-opacity")}</div>
  <PropInput
    label="Op"
    value={getValue("opacity")}
    placeholder="1"
    {...edit.continuous("opacity")}
  />

  <!-- Mix Blend Mode -->
  <div class="row-label spaced-small">{t("inspector-effects-blend-mode")}</div>
  <TextWithOptions
    value={getValue("mix-blend-mode")}
    placeholder="normal"
    options={BLEND_MODES}
    {...edit.continuous("mix-blend-mode")}
  />

  <!-- Clip Path -->
  <div class="row-label spaced-small">{t("inspector-effects-clip-path")}</div>
  <TextWithOptions
    value={getValue("clip-path")}
    placeholder="none"
    options={CLIP_PATH_OPTS}
    {...edit.continuous("clip-path")}
  />

  <!-- Filter -->
  <div class="effects-subheader spaced-large">
    <span class="effects-label" class:has-value={getValue("filter") !== ""}>{t("inspector-effects-filter")}</span>
    <button
      bind:this={filterBtnRef}
      type="button"
      class="add-btn ui-icon-button mini"
      class:active={showFilter}
      aria-pressed={showFilter}
      title={t("inspector-effects-add-filter")}
      aria-label={t("inspector-effects-add-filter")}
      onclick={openFilter}
    >
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>
  <PropInput
    value={getValue("filter")}
    placeholder="none"
    {...edit.continuous("filter")}
  />

  <!-- Backdrop Filter -->
  <div class="effects-subheader spaced-small">
    <span class="effects-label" class:has-value={getValue("backdrop-filter") !== ""}>{t("inspector-effects-backdrop-filter")}</span>
    <button
      bind:this={bdFilterBtnRef}
      type="button"
      class="add-btn ui-icon-button mini"
      class:active={showBdFilter}
      aria-pressed={showBdFilter}
      title={t("inspector-effects-add-backdrop-filter")}
      aria-label={t("inspector-effects-add-backdrop-filter")}
      onclick={openBdFilter}
    >
      <IconPlus size={13} stroke={1.9} />
    </button>
  </div>
  <PropInput
    value={getValue("backdrop-filter")}
    placeholder="none"
    {...edit.continuous("backdrop-filter")}
  />

  <!-- Mask -->
  <div class="effects-subheader spaced-large">
    <span class="effects-label" class:has-value={getValue("mask-image") !== ""}>{t("inspector-effects-mask")}</span>
  </div>
  <div class="row-label">{t("inspector-effects-mask-image")}</div>
  <AssetPicker
    value={maskImageUrl}
    assets={imageAssets}
    assetUrl={projectAssetPublicUrl}
    assetMeta={projectAssetOriginLabel}
    oninput={(value) => edit.draft("mask-image", maskImageValue(value))}
    oncommit={(value) => edit.commit("mask-image", maskImageValue(value))}
    oncancel={() => edit.cancel("mask-image")}
  />

  <div class="row-2 label-row spaced-small">
    <span class="row-label">{t("inspector-effects-mask-size")}</span>
    <span class="row-label">{t("inspector-effects-mask-repeat")}</span>
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

  <div class="row-label spaced-tiny">{t("inspector-effects-mask-position")}</div>
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
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .effects-label.has-value {
    color: var(--brand-strong);
  }

  .row-label {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .label-row { align-items: center; }

  .spaced-tiny { margin-top: 2px; }
  .spaced-small { margin-top: 4px; }
  .spaced-large { margin-top: 6px; }

  /* ── Backdrop ──────────────────────────────────────────────────────────── */

  .effects-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
  }

  /* ── Popover ───────────────────────────────────────────────────────────── */

  .effects-popover {
    overflow-y: auto;
  }

  .effects-option {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 9px;
  }

  .effects-opt-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }

  .effects-opt-val {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-muted);
  }
</style>
