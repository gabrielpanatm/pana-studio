<script lang="ts">
  import type { CssProperty, ProjectFile, ScssVariable } from "$lib/types";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";
  import TypographySection from "./sections/TypographySection.svelte";
  import ColorsSection     from "./sections/ColorsSection.svelte";
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
    scannedAssets = [],
    cssPropertyEdit,
  }: {
    classRules: CssProperty[];
    pendingValues: Record<string, string>;
    scssVariables?: ScssVariable[];
    scannedAssets?: ProjectFile[];
    cssPropertyEdit: CssPropertyEditController;
  } = $props();

  const rulesMap = $derived(
    Object.fromEntries(classRules.map((r) => [r.property, r.value]))
  );
</script>

<div class="class-editor">
  <TypographySection {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <ColorsSection     {pendingValues} {rulesMap} {scssVariables} {scannedAssets} edit={cssPropertyEdit} />
  <SpacingSection    {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
  <LayoutSection     {pendingValues} {rulesMap} {scssVariables} edit={cssPropertyEdit} />
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
