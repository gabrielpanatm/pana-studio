<script lang="ts">
  import type { ScssVariable } from "$lib/types";
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
  import SegmentedControl from "../controls/SegmentedControl.svelte";
  import PropInput from "../controls/PropInput.svelte";

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

  const PROPS = ["position", "top", "right", "bottom", "left", "z-index"];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const position = $derived(getValue("position"));
  const isPositioned = $derived(
    position === "relative" || position === "absolute" ||
    position === "fixed" || position === "sticky"
  );

  const positionOpts = [
    { value: "static",   label: "Sta", title: "Static"   },
    { value: "relative", label: "Rel", title: "Relative" },
    { value: "absolute", label: "Abs", title: "Absolute" },
    { value: "fixed",    label: "Fix", title: "Fixed"    },
    { value: "sticky",   label: "Stk", title: "Sticky"   },
  ];
</script>

<InspectorSection title="Position" {hasValues}>
  {#snippet icon()}<IconMapPin size={13} stroke={1.7} />{/snippet}

  <div class="row-label">Position</div>
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

  <div class="row-label">Z-Index</div>
  <PropInput value={getValue("z-index")} placeholder="auto" {...edit.continuous("z-index")}>
    {#snippet prefix()}<IconStack2 size={11} stroke={1.8} />{/snippet}
  </PropInput>
</InspectorSection>

<style>
  .row-label { font-size: 11px; color: var(--text-muted); margin-top: 2px; }
  .row-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
</style>
