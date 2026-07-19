<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import {
    IconBorderAll,
    IconBorderRadius,
    IconRadiusTopLeft,
    IconRadiusTopRight,
    IconRadiusBottomLeft,
    IconRadiusBottomRight,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import ColorInput from "../controls/ColorInput.svelte";
  import SegmentedControl from "../controls/SegmentedControl.svelte";

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
    "border", "border-width", "border-style", "border-color", "border-radius",
    "border-top-left-radius", "border-top-right-radius",
    "border-bottom-left-radius", "border-bottom-right-radius",
    "outline", "outline-width", "outline-color", "outline-style",
  ];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const borderStyleOpts = [
    { value: "none",   label: "—",    title: "None"   },
    { value: "solid",  label: "─",    title: "Solid"  },
    { value: "dashed", label: "- -",  title: "Dashed" },
    { value: "dotted", label: "···",  title: "Dotted" },
  ];

  let expandRadius = $state(false);
  const hasIndivRadius = $derived(
    ["border-top-left-radius","border-top-right-radius",
     "border-bottom-left-radius","border-bottom-right-radius"].some((p) => getValue(p) !== "")
  );
  $effect(() => { if (hasIndivRadius) expandRadius = true; });
</script>

<InspectorSection title="Border" {hasValues}>
  {#snippet icon()}<IconBorderAll size={13} stroke={1.7} />{/snippet}

  <div class="row-label">Border shorthand</div>
  <PropInput label="B" value={getValue("border")} placeholder="—" {...edit.continuous("border")} />

  <div class="row-2">
    <div class="col">
      <div class="row-label">Width</div>
      <PropInput label="W" value={getValue("border-width")} placeholder="0" {...edit.continuous("border-width")} />
    </div>
    <div class="col">
      <div class="row-label">Style</div>
      <SegmentedControl
        options={borderStyleOpts}
        value={getValue("border-style")}
        onchange={(v) => edit.commit("border-style", v)}
      />
    </div>
  </div>

  <div class="row-label">Color</div>
  <ColorInput
    property="border-color"
    value={getValue("border-color")}
    suggestions={variablesForProperty("border-color", scssVariables)}
    {...edit.continuous("border-color")}
  />

  <div class="sub-header">
    <span class="row-label">Radius</span>
    <button type="button" class="expand-btn" title="Individual corners" onclick={() => (expandRadius = !expandRadius)}>
      <IconBorderRadius size={12} stroke={1.7} />
    </button>
  </div>

  {#if expandRadius}
    {#if getValue("border-radius") !== ""}
      <div class="conflict-note">Shorthand radius activ</div>
      <PropInput value={getValue("border-radius")} suggestions={variablesForProperty("border-radius", scssVariables)} placeholder="0" {...edit.continuous("border-radius")}>
        {#snippet prefix()}<IconBorderRadius size={11} stroke={1.8} />{/snippet}
      </PropInput>
    {/if}
    <div class="row-2">
      <PropInput value={getValue("border-top-left-radius")} suggestions={variablesForProperty("border-top-left-radius", scssVariables)} {...edit.continuous("border-top-left-radius")}>
        {#snippet prefix()}<IconRadiusTopLeft size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("border-top-right-radius")} suggestions={variablesForProperty("border-top-right-radius", scssVariables)} {...edit.continuous("border-top-right-radius")}>
        {#snippet prefix()}<IconRadiusTopRight size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
    <div class="row-2">
      <PropInput value={getValue("border-bottom-left-radius")} suggestions={variablesForProperty("border-bottom-left-radius", scssVariables)} {...edit.continuous("border-bottom-left-radius")}>
        {#snippet prefix()}<IconRadiusBottomLeft size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("border-bottom-right-radius")} suggestions={variablesForProperty("border-bottom-right-radius", scssVariables)} {...edit.continuous("border-bottom-right-radius")}>
        {#snippet prefix()}<IconRadiusBottomRight size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
  {:else}
    <PropInput value={getValue("border-radius")} suggestions={variablesForProperty("border-radius", scssVariables)} placeholder="0" {...edit.continuous("border-radius")}>
      {#snippet prefix()}<IconBorderRadius size={11} stroke={1.8} />{/snippet}
    </PropInput>
  {/if}

  <div class="sub-header" style="margin-top:4px">
    <span class="row-label">Outline</span>
  </div>
  <PropInput label="O" value={getValue("outline")} placeholder="—" {...edit.continuous("outline")} />
  <div class="row-2">
    <PropInput label="W" value={getValue("outline-width")} placeholder="0" {...edit.continuous("outline-width")} />
    <SegmentedControl options={borderStyleOpts} value={getValue("outline-style")} onchange={(v) => edit.commit("outline-style", v)} />
  </div>
  <ColorInput
    property="outline-color"
    value={getValue("outline-color")}
    suggestions={variablesForProperty("outline-color", scssVariables)}
    {...edit.continuous("outline-color")}
  />
</InspectorSection>

<style>
  .row-label { font-size: 11px; color: var(--text-muted); margin-top: 2px; }
  .row-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; } .row-2 > * { min-width: 0; overflow: hidden; }
  .col { display: flex; flex-direction: column; gap: 4px;
  min-width: 0; }
  .sub-header { display: flex; align-items: center; justify-content: space-between; margin-top: 4px; }
  .conflict-note { color: var(--text-muted); font-size: 10px; line-height: 1.3; }
  .expand-btn {
    display: flex; align-items: center; justify-content: center;
    width: 20px; height: 20px; padding: 0;
    border: 1px solid var(--border-4); border-radius: 4px;
    background: var(--surface-4); cursor: pointer; color: var(--text-muted);
  }
  .expand-btn:hover { border-color: var(--brand); color: var(--brand); }
</style>
