<script lang="ts">
  import type { ScssVariable } from "$lib/css/contracts";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    IconLayoutGrid,
    IconSquare,
    IconColumns,
    IconMinus,
    IconDots,
    IconEyeOff,
    IconArrowRight,
    IconArrowDown,
    IconArrowLeft,
    IconArrowUp,
    IconAlignBoxLeftMiddle,
    IconAlignBoxCenterMiddle,
    IconAlignBoxRightMiddle,
    IconArrowsHorizontal,
    IconLayoutAlignTop,
    IconLayoutAlignMiddle,
    IconLayoutAlignBottom,
    IconArrowMergeAltRight,
    IconArrowsSplit,
    IconLayoutGridAdd,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import SegmentedControl from "../controls/SegmentedControl.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import GridBuilder from "../controls/GridBuilder.svelte";
  import type { CssGrid } from "$lib/inspector/grid-model";
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    canonicalGrid = null,
    viewport = "desktop",
    hasBaseRule = false,
    hasViewportRule = false,
    gridOverlayEnabled = false,
    onGridOverlayChange,
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    canonicalGrid?: CssGrid | null;
    viewport?: "desktop" | "tablet" | "mobile";
    hasBaseRule?: boolean;
    hasViewportRule?: boolean;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = [
    "display", "flex-direction", "justify-content", "align-items",
    "flex-wrap", "grid-template-columns", "grid-template-rows",
    "grid-template-areas", "grid-auto-columns", "grid-auto-rows", "grid-auto-flow",
    "column-gap", "row-gap", "gap", "align-content", "justify-items",
    "grid", "grid-template", "place-content", "place-items",
    "grid-column", "grid-row", "grid-area",
    "align-self", "flex-grow", "flex-shrink",
  ];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const display = $derived(getValue("display"));
  const effectiveDisplay = $derived(display || canonicalGrid?.display || "");
  const isFlex  = $derived(display === "flex" || display === "inline-flex");
  const isGrid  = $derived(effectiveDisplay === "grid" || effectiveDisplay === "inline-grid");

  const displayOpts = $derived([
    { value: "block",        icon: IconSquare,      title: t("inspector-layout-block") },
    { value: "flex",         icon: IconColumns,     title: t("inspector-layout-flex") },
    { value: "grid",         icon: IconLayoutGrid,  title: t("inspector-layout-grid") },
    { value: "inline",       icon: IconMinus,       title: t("inspector-layout-inline") },
    { value: "inline-flex",  icon: IconDots,        title: t("inspector-layout-inline-flex") },
    { value: "inline-grid",  icon: IconLayoutGridAdd, title: t("inspector-layout-inline-grid") },
    { value: "none",         icon: IconEyeOff,      title: t("inspector-none") },
  ]);

  const flexDirectionOpts = $derived([
    { value: "row",            icon: IconArrowRight, title: t("inspector-layout-row") },
    { value: "column",         icon: IconArrowDown,  title: t("inspector-layout-column") },
    { value: "row-reverse",    icon: IconArrowLeft,  title: t("inspector-layout-row-reverse") },
    { value: "column-reverse", icon: IconArrowUp,    title: t("inspector-layout-column-reverse") },
  ]);

  const justifyOpts = $derived([
    { value: "flex-start",    icon: IconAlignBoxLeftMiddle,        title: t("inspector-layout-flex-start") },
    { value: "center",        icon: IconAlignBoxCenterMiddle,       title: t("inspector-layout-center") },
    { value: "flex-end",      icon: IconAlignBoxRightMiddle,       title: t("inspector-layout-flex-end") },
    { value: "space-between", icon: IconArrowsHorizontal,          title: t("inspector-layout-space-between") },
    { value: "space-around",  icon: IconArrowsSplit,               title: t("inspector-layout-space-around") },
  ]);

  const alignOpts = $derived([
    { value: "flex-start", icon: IconLayoutAlignTop,    title: t("inspector-layout-flex-start") },
    { value: "center",     icon: IconLayoutAlignMiddle, title: t("inspector-layout-center") },
    { value: "flex-end",   icon: IconLayoutAlignBottom, title: t("inspector-layout-flex-end") },
    { value: "stretch",    icon: IconArrowMergeAltRight, title: t("inspector-layout-stretch") },
  ]);

  const wrapOpts = $derived([
    { value: "nowrap",       label: "none",    title: t("inspector-layout-no-wrap") },
    { value: "wrap",         label: "wrap",    title: t("inspector-layout-wrap-value") },
    { value: "wrap-reverse", label: "reverse", title: t("inspector-layout-wrap-reverse") },
  ]);

  function placementSpan(property: "grid-column" | "grid-row") {
    const match = getValue(property).match(/(?:^|\/)\s*span\s+([^/]+)(?:\/|$)/i);
    return match?.[1]?.trim() ?? "";
  }

  function updatePlacementSpan(property: "grid-column" | "grid-row", value: string, commit = false) {
    const span = value.trim().replace(/^span\s+/i, "");
    const current = getValue(property).trim();
    const startCandidate = current.split("/")[0]?.trim() ?? "";
    const start = startCandidate && !/^span(?:\s|$)/i.test(startCandidate) ? startCandidate : "auto";
    const next = span ? `${start} / span ${span}` : start === "auto" ? "" : start;
    if (commit) edit.commit(property, next);
    else edit.draft(property, next);
  }
</script>

<InspectorSection title={t("inspector-layout-title")} {hasValues}>
  {#snippet icon()}<IconLayoutGrid size={13} stroke={1.7} />{/snippet}

  <div class="row-label">{t("inspector-layout-display")}</div>
  <SegmentedControl
    options={displayOpts}
    value={getValue("display")}
    onchange={(v) => edit.commit("display", v)}
  />

  {#if isFlex}
    <div class="row-label">{t("inspector-layout-direction")}</div>
    <SegmentedControl
      options={flexDirectionOpts}
      value={getValue("flex-direction")}
      onchange={(v) => edit.commit("flex-direction", v)}
    />

    <div class="row-label">{t("inspector-layout-justify-content")}</div>
    <SegmentedControl
      options={justifyOpts}
      value={getValue("justify-content")}
      onchange={(v) => edit.commit("justify-content", v)}
    />

    <div class="row-label">{t("inspector-layout-align-items")}</div>
    <SegmentedControl
      options={alignOpts}
      value={getValue("align-items")}
      onchange={(v) => edit.commit("align-items", v)}
    />

    <div class="row-label">{t("inspector-layout-wrap")}</div>
    <SegmentedControl
      options={wrapOpts}
      value={getValue("flex-wrap")}
      onchange={(v) => edit.commit("flex-wrap", v)}
    />

    <div class="row-2">
      <div class="col">
        <div class="row-label">{t("inspector-layout-grow")}</div>
        <PropInput label="G" value={getValue("flex-grow")} placeholder="0" {...edit.continuous("flex-grow")} />
      </div>
      <div class="col">
        <div class="row-label">{t("inspector-layout-shrink")}</div>
        <PropInput label="S" value={getValue("flex-shrink")} placeholder="1" {...edit.continuous("flex-shrink")} />
      </div>
    </div>
  {/if}

  {#if isGrid}
    <GridBuilder
      {pendingValues}
      {rulesMap}
      {canonicalGrid}
      {scssVariables}
      {viewport}
      {hasBaseRule}
      {hasViewportRule}
      overlayEnabled={gridOverlayEnabled}
      onOverlayChange={onGridOverlayChange}
      {edit}
    />
  {/if}

  <div class="grid-item-placement" class:active={Boolean(getValue("grid-column") || getValue("grid-row") || getValue("grid-area"))}>
    <div class="grid-item-heading">
      <strong>{t("inspector-grid-item-title")}</strong>
      <span>{t("inspector-grid-item-hint")}</span>
    </div>
    <PropInput label="C" value={getValue("grid-column")} placeholder="1 / span 2" {...edit.continuous("grid-column")} />
    <PropInput label="R" value={getValue("grid-row")} placeholder="auto / span 1" {...edit.continuous("grid-row")} />
    <div class="row-2">
      <div class="col">
        <div class="row-label">{t("inspector-grid-column-span")}</div>
        <PropInput label="C" value={placementSpan("grid-column")} placeholder="2" oninput={(value) => updatePlacementSpan("grid-column", value)} oncommit={(value) => updatePlacementSpan("grid-column", value, true)} oncancel={() => edit.cancel("grid-column")} />
      </div>
      <div class="col">
        <div class="row-label">{t("inspector-grid-row-span")}</div>
        <PropInput label="R" value={placementSpan("grid-row")} placeholder="1" oninput={(value) => updatePlacementSpan("grid-row", value)} oncommit={(value) => updatePlacementSpan("grid-row", value, true)} oncancel={() => edit.cancel("grid-row")} />
      </div>
    </div>
    <PropInput label="A" value={getValue("grid-area")} placeholder={t("inspector-grid-area-name")} {...edit.continuous("grid-area")} />
  </div>
</InspectorSection>

<style>
  .row-label {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 4px;
  min-width: 0;
  }
  .grid-item-placement {
    display: grid;
    gap: 6px;
    margin-top: 2px;
    padding: 7px;
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: var(--surface-4);
  }
  .grid-item-placement.active {
    border-color: color-mix(in srgb, var(--brand) 48%, var(--border-subtle));
    background: color-mix(in srgb, var(--surface-4) 88%, var(--brand-soft));
  }
  .grid-item-heading {
    display: grid;
    gap: 2px;
  }
  .grid-item-heading strong {
    color: var(--text);
    font-size: 11px;
  }
  .grid-item-heading span {
    color: var(--text-muted);
    font-size: 11px;
  }
</style>
