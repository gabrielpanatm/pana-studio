<script lang="ts">
  import { t } from "$lib/i18n/runtime.svelte";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import {
    IconMapPin,
    IconArrowUp,
    IconArrowRight,
    IconArrowDown,
    IconArrowLeft,
    IconStack2,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import SegmentedControl from "$lib/components/ui/SegmentedControl.svelte";
  import PropInput from "../controls/PropInput.svelte";

  let {
    pendingValues,
    rulesMap,
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = ["position", "top", "right", "bottom", "left", "z-index"];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const position = $derived(getValue("position"));
  const isPositioned = $derived(
    position === "relative" || position === "absolute" ||
    position === "fixed" || position === "sticky"
  );

  const positionOpts = $derived([
    { value: "static", label: t("inspector-position-static-short"), title: t("inspector-position-static") },
    { value: "relative", label: t("inspector-position-relative-short"), title: t("inspector-position-relative") },
    { value: "absolute", label: t("inspector-position-absolute-short"), title: t("inspector-position-absolute") },
    { value: "fixed", label: t("inspector-position-fixed-short"), title: t("inspector-position-fixed") },
    { value: "sticky", label: t("inspector-position-sticky-short"), title: t("inspector-position-sticky") },
  ]);
</script>

<InspectorSection title={t("inspector-position-title")} {hasValues}>
  {#snippet icon()}<IconMapPin size={13} stroke={1.7} />{/snippet}

  <div class="row-label">{t("inspector-position-title")}</div>
  <SegmentedControl
    options={positionOpts}
    value={getValue("position")}
    onchange={(v) => edit.commit("position", v)}
  />

  {#if isPositioned}
    <div class="row-2">
      <PropInput value={getValue("top")} placeholder="auto" {...edit.continuous("top")}>
        {#snippet prefix()}<IconArrowUp size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("right")} placeholder="auto" {...edit.continuous("right")}>
        {#snippet prefix()}<IconArrowRight size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
    <div class="row-2">
      <PropInput value={getValue("bottom")} placeholder="auto" {...edit.continuous("bottom")}>
        {#snippet prefix()}<IconArrowDown size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("left")} placeholder="auto" {...edit.continuous("left")}>
        {#snippet prefix()}<IconArrowLeft size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
  {/if}

  <div class="row-label">{t("inspector-position-z-index")}</div>
  <PropInput value={getValue("z-index")} placeholder="auto" {...edit.continuous("z-index")}>
    {#snippet prefix()}<IconStack2 size={11} stroke={1.8} />{/snippet}
  </PropInput>
</InspectorSection>

<style>
  .row-label { font-size: 12px; color: var(--text-muted); margin-top: 2px; }
  .row-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
</style>
