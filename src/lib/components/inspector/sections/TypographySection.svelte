<script lang="ts">
  import type { CssPropertySuggestion, ScssVariable } from "$lib/css/contracts";
  import type { InstalledFontVariationAxis } from "$lib/fonts/contracts";
  import { t } from "$lib/i18n/runtime.svelte";
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
    IconItalic,
    IconUnderline,
    IconStrikethrough,
    IconLetterCase,
    IconLetterCaseUpper,
    IconLetterCaseLower,
  } from "@tabler/icons-svelte";
  import InspectorSection from "../InspectorSection.svelte";
  import PropInput from "../controls/PropInput.svelte";
  import ColorInput from "../controls/ColorInput.svelte";
  import SegmentedControl from "../controls/SegmentedControl.svelte";

  let {
    pendingValues,
    rulesMap,
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    edit,
  }: {
    pendingValues: Record<string, string>;
    rulesMap: Record<string, string>;
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    edit: CssPropertyEditController;
  } = $props();

  function getValue(prop: string): string {
    return pendingValues[prop] ?? rulesMap[prop] ?? "";
  }

  const PROPS = [
    "color",
    "font-family", "font-size", "font-weight", "line-height",
    "letter-spacing", "text-align", "text-transform", "text-decoration", "font-style",
    "font-variation-settings", "font-optical-sizing",
  ];

  const hasValues = $derived(PROPS.some((p) => getValue(p) !== ""));
  const fontFamilySuggestions = $derived.by(() => {
    const suggestions: CssPropertySuggestion[] = [
      ...variablesForProperty("font-family", scssVariables),
      ...fontFamilies.map((family) => ({
        name: family,
        value: t("inspector-typography-installed-family"),
        file: "Font Manager Rust",
        insertValue: quoteFontFamily(family),
        directValue: true,
      })),
    ];
    return suggestions.filter((value, index, values) => values.findIndex((entry) => (
      (entry.insertValue ?? `$${entry.name}`) === (value.insertValue ?? `$${value.name}`)
    )) === index);
  });
  const fontAxisSuggestions = $derived.by(() => installedFontAxes.flatMap((axis) => {
    const positions = [
      { label: t("inspector-typography-axis-min"), value: axis.min },
      { label: t("inspector-typography-axis-default"), value: axis.default },
      { label: t("inspector-typography-axis-max"), value: axis.max },
    ];
    return positions
      .filter((position, index) => positions.findIndex((candidate) => candidate.value === position.value) === index)
      .map((position): CssPropertySuggestion => ({
        name: `${axis.family} · ${axis.tag} · ${position.label}`,
        value: `${axis.min}–${axis.max}`,
        file: "Font Manager Rust · tabela fvar",
        insertValue: `'${axis.tag.replaceAll("'", "\\'")}' ${position.value}`,
        directValue: true,
      }));
  }));

  function quoteFontFamily(family: string) {
    return `'${family.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`;
  }

  const textAlignOpts = $derived([
    { value: "left", icon: IconAlignLeft, title: t("inspector-typography-left") },
    { value: "center", icon: IconAlignCenter, title: t("inspector-typography-center") },
    { value: "right", icon: IconAlignRight, title: t("inspector-typography-right") },
    { value: "justify", icon: IconAlignJustified, title: t("inspector-typography-justify") },
  ]);

  const fontWeightOpts = $derived([
    { value: "300", label: "L", title: t("inspector-typography-weight-light") },
    { value: "400", label: "R", title: t("inspector-typography-weight-regular") },
    { value: "500", label: "M", title: t("inspector-typography-weight-medium") },
    { value: "600", label: "Sb", title: t("inspector-typography-weight-semibold") },
    { value: "700", label: "B", title: t("inspector-typography-weight-bold") },
    { value: "800", label: "Eb", title: t("inspector-typography-weight-extrabold") },
    { value: "900", label: "X", title: t("inspector-typography-weight-black") },
  ]);

  const textDecorationOpts = $derived([
    { value: "none", label: "—", title: t("inspector-none") },
    { value: "underline", icon: IconUnderline, title: t("inspector-typography-underline") },
    { value: "line-through", icon: IconStrikethrough, title: t("inspector-typography-strikethrough") },
  ]);

  const fontStyleOpts = $derived([
    { value: "normal", label: "R", title: t("inspector-typography-normal") },
    { value: "italic", icon: IconItalic, title: t("inspector-typography-italic") },
  ]);

  const textTransformOpts = $derived([
    { value: "none", label: "—", title: t("inspector-none") },
    { value: "uppercase", icon: IconLetterCaseUpper, title: t("inspector-typography-uppercase") },
    { value: "capitalize", icon: IconLetterCase, title: t("inspector-typography-capitalize") },
    { value: "lowercase", icon: IconLetterCaseLower, title: t("inspector-typography-lowercase") },
  ]);
</script>

<InspectorSection title={t("inspector-typography-title")} {hasValues}>
  {#snippet icon()}<IconTypography size={13} stroke={1.7} />{/snippet}

  <div class="row-label">{t("inspector-typography-color")}</div>
  <ColorInput
    property="color"
    value={getValue("color")}
    suggestions={variablesForProperty("color", scssVariables)}
    {...edit.continuous("color")}
  />

  <div class="row-label">{t("inspector-typography-font-family")}</div>
  <PropInput
    value={getValue("font-family")}
    suggestions={fontFamilySuggestions}
    {...edit.continuous("font-family")}
  />

  <div class="row-2">
    <div class="col">
      <div class="row-label">{t("inspector-typography-size")}</div>
      <PropInput
        value={getValue("font-size")}
        suggestions={variablesForProperty("font-size", scssVariables)}
        {...edit.continuous("font-size")}
      >
        {#snippet prefix()}<IconTextSize size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
    <div class="col">
      <div class="row-label">{t("inspector-typography-line-height")}</div>
      <PropInput
        value={getValue("line-height")}
        suggestions={variablesForProperty("line-height", scssVariables)}
        {...edit.continuous("line-height")}
      >
        {#snippet prefix()}<IconLineHeight size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
  </div>

  <div class="row-label">{t("inspector-typography-font-weight")}</div>
  <SegmentedControl
    options={fontWeightOpts}
    value={getValue("font-weight")}
    onchange={(v) => edit.commit("font-weight", v)}
  />

  {#if installedFontAxes.length}
    <div class="row-label">{t("inspector-typography-variable-axes")}</div>
    <PropInput
      value={getValue("font-variation-settings")}
      suggestions={fontAxisSuggestions}
      placeholder="'wdth' 100, 'opsz' 16"
      {...edit.continuous("font-variation-settings")}
    />
    {#if installedFontAxes.some((axis) => axis.tag === "opsz")}
      <div class="row-label">{t("inspector-typography-optical-sizing")}</div>
      <SegmentedControl
        options={[
          { value: "auto", label: t("inspector-auto"), title: t("inspector-typography-use-optical-axis") },
          { value: "none", label: t("inspector-none"), title: t("inspector-typography-disable-optical") },
        ]}
        value={getValue("font-optical-sizing")}
        onchange={(v) => edit.commit("font-optical-sizing", v)}
      />
    {/if}
  {/if}

  <div class="row-label">{t("inspector-typography-align")}</div>
  <SegmentedControl
    options={textAlignOpts}
    value={getValue("text-align")}
    onchange={(v) => edit.commit("text-align", v)}
  />

  <div class="row-2">
    <div class="col">
      <div class="row-label">{t("inspector-typography-letter-spacing")}</div>
      <PropInput
        value={getValue("letter-spacing")}
        suggestions={variablesForProperty("letter-spacing", scssVariables)}
        {...edit.continuous("letter-spacing")}
      >
        {#snippet prefix()}<IconLetterSpacing size={12} stroke={1.7} />{/snippet}
      </PropInput>
    </div>
    <div class="col">
      <div class="row-label">{t("inspector-typography-style")}</div>
      <SegmentedControl
        options={fontStyleOpts}
        value={getValue("font-style")}
        onchange={(v) => edit.commit("font-style", v)}
      />
    </div>
  </div>

  <div class="row-label">{t("inspector-typography-transform")}</div>
  <SegmentedControl
    options={textTransformOpts}
    value={getValue("text-transform")}
    onchange={(v) => edit.commit("text-transform", v)}
  />

  <div class="row-label">{t("inspector-typography-decoration")}</div>
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
