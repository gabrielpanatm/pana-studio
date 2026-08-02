<script lang="ts">
  import type { CssProperty, InstalledFontVariationAxis, ProjectFile, ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import TypographySection from "./sections/TypographySection.svelte";
  import BackgroundSection from "./sections/BackgroundSection.svelte";
  import SpacingSection    from "./sections/SpacingSection.svelte";
  import LayoutSection     from "./sections/LayoutSection.svelte";
  import PositionSection   from "./sections/PositionSection.svelte";
  import SizeSection       from "./sections/SizeSection.svelte";
  import BorderSection     from "./sections/BorderSection.svelte";
  import ShadowSection     from "./sections/ShadowSection.svelte";
  import TransformSection  from "./sections/TransformSection.svelte";
  import EffectsSection    from "./sections/EffectsSection.svelte";
  let {
    classRules,
    pendingValues,
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    scannedAssets = [],
    cssPropertyEdit,
    canonicalBackground = null,
    canonicalGrid = null,
    gridViewport = "desktop",
    gridHasBaseRule = false,
    gridHasViewportRule = false,
    gridOverlayEnabled = false,
    onGridOverlayChange,
  }: {
    classRules: CssProperty[];
    pendingValues: Record<string, string>;
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    scannedAssets?: ProjectFile[];
    cssPropertyEdit: CssPropertyEditController;
    canonicalBackground?: import("$lib/inspector/background-model").CssBackground | null;
    canonicalGrid?: import("$lib/inspector/grid-model").CssGrid | null;
    gridViewport?: "desktop" | "tablet" | "mobile";
    gridHasBaseRule?: boolean;
    gridHasViewportRule?: boolean;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
  } = $props();

  const rulesMap = $derived(
    Object.fromEntries(classRules.map((r) => [r.property, r.value]))
  );
</script>

<div class="class-editor">
  <TypographySection {pendingValues} {rulesMap} {scssVariables} {fontFamilies} {installedFontAxes} edit={cssPropertyEdit} />
  <BackgroundSection {pendingValues} {rulesMap} {canonicalBackground} {scssVariables} {scannedAssets} edit={cssPropertyEdit} />
  <SpacingSection    {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <LayoutSection
    {pendingValues}
    {rulesMap}
    {scssVariables}
    {canonicalGrid}
    viewport={gridViewport}
    hasBaseRule={gridHasBaseRule}
    hasViewportRule={gridHasViewportRule}
    {gridOverlayEnabled}
    {onGridOverlayChange}
    edit={cssPropertyEdit}
  />
  <PositionSection   {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <SizeSection       {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <BorderSection     {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <ShadowSection     {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <TransformSection  {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <EffectsSection    {pendingValues} {rulesMap} {scssVariables} {scannedAssets} edit={cssPropertyEdit} />
</div>

<style>
  .class-editor {
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }
</style>
