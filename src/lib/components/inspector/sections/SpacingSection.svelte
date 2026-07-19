<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import {
    IconBoxPadding,
    IconArrowUp,
    IconArrowRight,
    IconArrowDown,
    IconArrowLeft,
    IconSpacingVertical,
    IconSpacingHorizontal,
    IconLink,
    IconUnlink,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";

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
    "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "margin", "margin-top", "margin-right", "margin-bottom", "margin-left",
    "gap", "row-gap", "column-gap",
    "white-space", "overflow", "overflow-x", "overflow-y",
  ];
  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const hasIndivPadding = $derived(
    ["padding-top","padding-right","padding-bottom","padding-left"].some((p) => getValue(p) !== "")
  );
  const hasIndivMargin = $derived(
    ["margin-top","margin-right","margin-bottom","margin-left"].some((p) => getValue(p) !== "")
  );

  let expandPadding  = $state(false);
  let expandMargin   = $state(false);
  let splitGap       = $state(false);
  let splitOverflow  = $state(false);

  $effect(() => { if (hasIndivPadding) expandPadding = true; });
  $effect(() => { if (hasIndivMargin)  expandMargin  = true; });
  $effect(() => {
    if (getValue("row-gap") !== "" || getValue("column-gap") !== "") splitGap = true;
  });
  $effect(() => {
    if (getValue("overflow-x") !== "" || getValue("overflow-y") !== "") splitOverflow = true;
  });

  const OVERFLOW_OPTS    = ["visible","hidden","clip","scroll","auto"];
  const WHITE_SPACE_OPTS = ["normal","nowrap","pre","pre-wrap","pre-line","break-spaces"];
</script>

<InspectorSection title="Spacing" {hasValues}>
  {#snippet icon()}<IconBoxPadding size={13} stroke={1.7} />{/snippet}

  <!-- PADDING -->
  <div class="sub-header">
    <span class="sub-label">Padding</span>
    <button type="button" class="expand-btn" title="Individual sides" onclick={() => (expandPadding = !expandPadding)}>
      <IconBoxPadding size={12} stroke={1.7} />
    </button>
  </div>

  {#if expandPadding}
    {#if getValue("padding") !== ""}
      <div class="conflict-note">Shorthand activ</div>
      <PropInput label="P" value={getValue("padding")} suggestions={variablesForProperty("padding", scssVariables)} {...edit.continuous("padding")} />
    {/if}
    <div class="row-2">
      <PropInput value={getValue("padding-top")} suggestions={variablesForProperty("padding-top", scssVariables)} {...edit.continuous("padding-top")}>
        {#snippet prefix()}<IconArrowUp size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("padding-right")} suggestions={variablesForProperty("padding-right", scssVariables)} {...edit.continuous("padding-right")}>
        {#snippet prefix()}<IconArrowRight size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
    <div class="row-2">
      <PropInput value={getValue("padding-bottom")} suggestions={variablesForProperty("padding-bottom", scssVariables)} {...edit.continuous("padding-bottom")}>
        {#snippet prefix()}<IconArrowDown size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("padding-left")} suggestions={variablesForProperty("padding-left", scssVariables)} {...edit.continuous("padding-left")}>
        {#snippet prefix()}<IconArrowLeft size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
  {:else}
    <PropInput label="P" value={getValue("padding")} suggestions={variablesForProperty("padding", scssVariables)} {...edit.continuous("padding")} />
  {/if}

  <!-- MARGIN -->
  <div class="sub-header">
    <span class="sub-label">Margin</span>
    <button type="button" class="expand-btn" title="Individual sides" onclick={() => (expandMargin = !expandMargin)}>
      <IconBoxPadding size={12} stroke={1.7} />
    </button>
  </div>

  {#if expandMargin}
    {#if getValue("margin") !== ""}
      <div class="conflict-note">Shorthand activ</div>
      <PropInput label="M" value={getValue("margin")} suggestions={variablesForProperty("margin", scssVariables)} {...edit.continuous("margin")} />
    {/if}
    <div class="row-2">
      <PropInput value={getValue("margin-top")} suggestions={variablesForProperty("margin-top", scssVariables)} {...edit.continuous("margin-top")}>
        {#snippet prefix()}<IconArrowUp size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("margin-right")} suggestions={variablesForProperty("margin-right", scssVariables)} {...edit.continuous("margin-right")}>
        {#snippet prefix()}<IconArrowRight size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
    <div class="row-2">
      <PropInput value={getValue("margin-bottom")} suggestions={variablesForProperty("margin-bottom", scssVariables)} {...edit.continuous("margin-bottom")}>
        {#snippet prefix()}<IconArrowDown size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("margin-left")} suggestions={variablesForProperty("margin-left", scssVariables)} {...edit.continuous("margin-left")}>
        {#snippet prefix()}<IconArrowLeft size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
  {:else}
    <PropInput label="M" value={getValue("margin")} suggestions={variablesForProperty("margin", scssVariables)} {...edit.continuous("margin")} />
  {/if}

  <!-- GAP -->
  <div class="sub-header">
    <span class="sub-label">Gap</span>
    <button
      type="button"
      class="expand-btn"
      class:active={splitGap}
      title={splitGap ? "Gap unificat" : "Gap separat rând/coloană"}
      onclick={() => { splitGap = !splitGap; }}
    >
      {#if splitGap}<IconUnlink size={11} stroke={1.8} />{:else}<IconLink size={11} stroke={1.8} />{/if}
    </button>
  </div>
  {#if splitGap}
    {#if getValue("gap") !== ""}
      <div class="conflict-note">Shorthand activ</div>
      <PropInput label="G" value={getValue("gap")} suggestions={variablesForProperty("gap", scssVariables)} {...edit.continuous("gap")} />
    {/if}
    <div class="row-2">
      <PropInput value={getValue("column-gap")} suggestions={variablesForProperty("column-gap", scssVariables)} {...edit.continuous("column-gap")}>
        {#snippet prefix()}<IconSpacingHorizontal size={11} stroke={1.8} />{/snippet}
      </PropInput>
      <PropInput value={getValue("row-gap")} suggestions={variablesForProperty("row-gap", scssVariables)} {...edit.continuous("row-gap")}>
        {#snippet prefix()}<IconSpacingVertical size={11} stroke={1.8} />{/snippet}
      </PropInput>
    </div>
  {:else}
    <PropInput label="G" value={getValue("gap")} suggestions={variablesForProperty("gap", scssVariables)} {...edit.continuous("gap")} />
  {/if}

  <!-- WHITE SPACE + OVERFLOW -->
  <div class="row-2 label-row">
    <span class="sub-label">White space</span>
    <div class="overflow-label">
      <span class="sub-label">Overflow</span>
      <button
        type="button"
        class="expand-btn"
        class:active={splitOverflow}
        title={splitOverflow ? "Overflow unificat" : "Overflow separat X/Y"}
        onclick={() => { splitOverflow = !splitOverflow; }}
      >
        {#if splitOverflow}
          <IconUnlink size={11} stroke={1.8} />
        {:else}
          <IconLink size={11} stroke={1.8} />
        {/if}
      </button>
    </div>
  </div>

  <div class="row-2">
    <SelectControl
      value={getValue("white-space")}
      placeholder="normal (implicit)"
      options={[{ value: "", label: "— implicit (normal)" }, ...WHITE_SPACE_OPTS.map((value) => ({ value, label: value }))]}
      ariaLabel="White space"
      onchange={(value) => edit.commit("white-space", value)}
    />
    <SelectControl
      value={splitOverflow ? getValue("overflow-x") : getValue("overflow")}
      placeholder="visible (implicit)"
      options={[
        { value: "", label: "— implicit (visible)" },
        ...OVERFLOW_OPTS.map((opt) => ({ value: opt, label: splitOverflow ? `X: ${opt}` : opt })),
      ]}
      ariaLabel={splitOverflow ? "Overflow X" : "Overflow"}
      onchange={(value) => edit.commit(splitOverflow ? "overflow-x" : "overflow", value)}
    />
  </div>

  {#if splitOverflow}
    {#if getValue("overflow") !== ""}
      <div class="conflict-note">Shorthand overflow activ; îl poți elimina din modul unificat.</div>
    {/if}
    <div class="row-2">
      <div></div>
      <SelectControl
        value={getValue("overflow-y")}
        placeholder="visible (implicit)"
        options={[
          { value: "", label: "— implicit (visible)" },
          ...OVERFLOW_OPTS.map((opt) => ({ value: opt, label: `Y: ${opt}` })),
        ]}
        ariaLabel="Overflow Y"
        onchange={(value) => edit.commit("overflow-y", value)}
      />
    </div>
  {/if}
</InspectorSection>

<style>
  .sub-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 4px;
  }

  .sub-label {
    font-size: 11px;
    color: var(--text-muted);
  }

  .conflict-note {
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.3;
  }

  .label-row {
    align-items: center;
  }

  .overflow-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .expand-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: 1px solid var(--border-4);
    border-radius: 4px;
    background: var(--surface-4);
    cursor: pointer;
    color: var(--text-muted);
    padding: 0;
    transition: color 80ms, border-color 80ms, background 80ms;
  }

  .expand-btn:hover { border-color: var(--brand); color: var(--brand); }

  .expand-btn.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

</style>
