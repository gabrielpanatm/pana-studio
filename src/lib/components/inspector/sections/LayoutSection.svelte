<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    IconLayoutGrid,
    IconSquare,
    IconColumns,
    IconLayoutRows,
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
  import { t } from "$lib/i18n/runtime.svelte";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = [
    "display", "flex-direction", "justify-content", "align-items",
    "flex-wrap", "grid-template-columns", "grid-template-rows",
    "align-self", "flex-grow", "flex-shrink",
  ];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const display = $derived(getValue("display"));
  const isFlex  = $derived(display === "flex" || display === "inline-flex");
  const isGrid  = $derived(display === "grid" || display === "inline-grid");

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
    <div class="row-label">{t("inspector-layout-template-columns")}</div>
    <PropInput
      label="C"
      value={getValue("grid-template-columns")}
      placeholder={t("inspector-layout-columns-placeholder")}
      {...edit.continuous("grid-template-columns")}
    />

    <div class="row-label">{t("inspector-layout-template-rows")}</div>
    <PropInput
      label="R"
      value={getValue("grid-template-rows")}
      placeholder={t("inspector-layout-rows-placeholder")}
      {...edit.continuous("grid-template-rows")}
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
  {/if}
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
</style>
