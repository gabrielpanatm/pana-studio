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
    IconAlignBoxCenterMiddleFilled,
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

  const displayOpts = [
    { value: "block",        icon: IconSquare,      title: "Block"        },
    { value: "flex",         icon: IconColumns,     title: "Flex"         },
    { value: "grid",         icon: IconLayoutGrid,  title: "Grid"         },
    { value: "inline",       icon: IconMinus,       title: "Inline"       },
    { value: "inline-flex",  icon: IconDots,        title: "Inline Flex"  },
    { value: "inline-grid",  icon: IconLayoutGridAdd, title: "Inline Grid"},
    { value: "none",         icon: IconEyeOff,      title: "None"         },
  ];

  const flexDirectionOpts = [
    { value: "row",            icon: IconArrowRight, title: "Row"            },
    { value: "column",         icon: IconArrowDown,  title: "Column"         },
    { value: "row-reverse",    icon: IconArrowLeft,  title: "Row Reverse"    },
    { value: "column-reverse", icon: IconArrowUp,    title: "Column Reverse" },
  ];

  const justifyOpts = [
    { value: "flex-start",    icon: IconAlignBoxLeftMiddle,        title: "Flex Start"    },
    { value: "center",        icon: IconAlignBoxCenterMiddleFilled, title: "Center"       },
    { value: "flex-end",      icon: IconAlignBoxRightMiddle,       title: "Flex End"      },
    { value: "space-between", icon: IconArrowsHorizontal,          title: "Space Between" },
    { value: "space-around",  icon: IconArrowsSplit,               title: "Space Around"  },
  ];

  const alignOpts = [
    { value: "flex-start", icon: IconLayoutAlignTop,    title: "Flex Start" },
    { value: "center",     icon: IconLayoutAlignMiddle, title: "Center"     },
    { value: "flex-end",   icon: IconLayoutAlignBottom, title: "Flex End"   },
    { value: "stretch",    icon: IconArrowMergeAltRight, title: "Stretch"   },
  ];

  const wrapOpts = [
    { value: "nowrap",       label: "none",    title: "No Wrap"     },
    { value: "wrap",         label: "wrap",    title: "Wrap"        },
    { value: "wrap-reverse", label: "reverse", title: "Wrap Reverse"},
  ];
</script>

<InspectorSection title="Layout" {hasValues}>
  {#snippet icon()}<IconLayoutGrid size={13} stroke={1.7} />{/snippet}

  <div class="row-label">Display</div>
  <SegmentedControl
    options={displayOpts}
    value={getValue("display")}
    onchange={(v) => edit.commit("display", v)}
  />

  {#if isFlex}
    <div class="row-label">Direction</div>
    <SegmentedControl
      options={flexDirectionOpts}
      value={getValue("flex-direction")}
      onchange={(v) => edit.commit("flex-direction", v)}
    />

    <div class="row-label">Justify Content</div>
    <SegmentedControl
      options={justifyOpts}
      value={getValue("justify-content")}
      onchange={(v) => edit.commit("justify-content", v)}
    />

    <div class="row-label">Align Items</div>
    <SegmentedControl
      options={alignOpts}
      value={getValue("align-items")}
      onchange={(v) => edit.commit("align-items", v)}
    />

    <div class="row-label">Wrap</div>
    <SegmentedControl
      options={wrapOpts}
      value={getValue("flex-wrap")}
      onchange={(v) => edit.commit("flex-wrap", v)}
    />

    <div class="row-2">
      <div class="col">
        <div class="row-label">Grow</div>
        <PropInput label="G" value={getValue("flex-grow")} placeholder="0" {...edit.continuous("flex-grow")} />
      </div>
      <div class="col">
        <div class="row-label">Shrink</div>
        <PropInput label="S" value={getValue("flex-shrink")} placeholder="1" {...edit.continuous("flex-shrink")} />
      </div>
    </div>
  {/if}

  {#if isGrid}
    <div class="row-label">Template Columns</div>
    <PropInput
      label="C"
      value={getValue("grid-template-columns")}
      placeholder="e.g. 1fr 1fr"
      {...edit.continuous("grid-template-columns")}
    />

    <div class="row-label">Template Rows</div>
    <PropInput
      label="R"
      value={getValue("grid-template-rows")}
      placeholder="e.g. auto 1fr"
      {...edit.continuous("grid-template-rows")}
    />

    <div class="row-label">Justify Content</div>
    <SegmentedControl
      options={justifyOpts}
      value={getValue("justify-content")}
      onchange={(v) => edit.commit("justify-content", v)}
    />

    <div class="row-label">Align Items</div>
    <SegmentedControl
      options={alignOpts}
      value={getValue("align-items")}
      onchange={(v) => edit.commit("align-items", v)}
    />
  {/if}
</InspectorSection>

<style>
  .row-label {
    font-size: 11px;
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
