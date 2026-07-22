<script lang="ts">
  import type { ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import { variablesForProperty } from "$lib/editor/controls";
  import {
    IconTypography,
    IconTextSize,
    IconLineHeight,
    IconLetterSpacing,
    IconAlignLeft,
    IconAlignCenter,
    IconAlignRight,
    IconAlignJustified,
    IconBold,
    IconItalic,
    IconUnderline,
    IconStrikethrough,
    IconLetterCase,
    IconLetterCaseUpper,
    IconLetterCaseLower,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
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
    "font-family", "font-size", "font-weight", "line-height",
    "letter-spacing", "text-align", "text-transform", "text-decoration", "font-style",
  ];

  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));

  const textAlignOpts = [
    { value: "left",    icon: IconAlignLeft,      title: "Left"    },
    { value: "center",  icon: IconAlignCenter,    title: "Center"  },
    { value: "right",   icon: IconAlignRight,     title: "Right"   },
    { value: "justify", icon: IconAlignJustified, title: "Justify" },
  ];

  const fontWeightOpts = [
    { value: "300", label: "L",  title: "Subțire (300)"      },
    { value: "400", label: "R",  title: "Regular (400)"    },
    { value: "500", label: "M",  title: "Medium (500)"     },
    { value: "600", label: "Sb", title: "SemiBold (600)"   },
    { value: "700", label: "B",  title: "Bold (700)"       },
    { value: "800", label: "Eb", title: "ExtraBold (800)"  },
    { value: "900", label: "X",  title: "Black (900)"      },
  ];

  const textDecorationOpts = [
    { value: "none",         label: "—",              title: "Niciuna"          },
    { value: "underline",    icon: IconUnderline,     title: "Underline"     },
    { value: "line-through", icon: IconStrikethrough, title: "Strikethrough" },
  ];

  const fontStyleOpts = [
    { value: "normal", label: "R",         title: "Normal" },
    { value: "italic", icon: IconItalic,   title: "Italic" },
  ];

  const textTransformOpts = [
    { value: "none",       label: "—",                    title: "Niciuna"      },
    { value: "uppercase",  icon: IconLetterCaseUpper,     title: "Uppercase" },
    { value: "capitalize", icon: IconLetterCase,          title: "Capitalize"},
    { value: "lowercase",  icon: IconLetterCaseLower,     title: "Lowercase" },
  ];
</script>

<InspectorSection title="Typography" {hasValues}>
  {#snippet icon()}<IconTypography size={13} stroke={1.7} />{/snippet}

  <div class="row-label">Font Family</div>
  <PropInput
    value={getValue("font-family")}
    suggestions={variablesForProperty("font-family", scssVariables)}
    {...edit.continuous("font-family")}
  />

  <div class="row-2">
    <div class="col">
      <div class="row-label">Size</div>
      <PropInput
        value={getValue("font-size")}
        suggestions={variablesForProperty("font-size", scssVariables)}
        {...edit.continuous("font-size")}
      >
        {#snippet prefix()}<IconTextSize size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
    <div class="col">
      <div class="row-label">Line Height</div>
      <PropInput
        value={getValue("line-height")}
        suggestions={variablesForProperty("line-height", scssVariables)}
        {...edit.continuous("line-height")}
      >
        {#snippet prefix()}<IconLineHeight size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
  </div>

  <div class="row-label">Font Weight</div>
  <SegmentedControl
    options={fontWeightOpts}
    value={getValue("font-weight")}
    onchange={(v) => edit.commit("font-weight", v)}
  />

  <div class="row-label">Align</div>
  <SegmentedControl
    options={textAlignOpts}
    value={getValue("text-align")}
    onchange={(v) => edit.commit("text-align", v)}
  />

  <div class="row-2">
    <div class="col">
      <div class="row-label">Letter Spacing</div>
      <PropInput
        value={getValue("letter-spacing")}
        suggestions={variablesForProperty("letter-spacing", scssVariables)}
        {...edit.continuous("letter-spacing")}
      >
        {#snippet prefix()}<IconLetterSpacing size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
    <div class="col">
      <div class="row-label">Style</div>
      <SegmentedControl
        options={fontStyleOpts}
        value={getValue("font-style")}
        onchange={(v) => edit.commit("font-style", v)}
      />
    </div>
  </div>

  <div class="row-label">Transform</div>
  <SegmentedControl
    options={textTransformOpts}
    value={getValue("text-transform")}
    onchange={(v) => edit.commit("text-transform", v)}
  />

  <div class="row-label">Decoration</div>
  <SegmentedControl
    options={textDecorationOpts}
    value={getValue("text-decoration")}
    onchange={(v) => edit.commit("text-decoration", v)}
  />
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
