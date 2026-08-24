<script lang="ts">
  import { IconPlus } from "@tabler/icons-svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import ClassEditor from "$lib/components/inspector/ClassEditor.svelte";
  import InspectorEmptyState from "$lib/components/inspector/InspectorEmptyState.svelte";
  import type {
    CssInspectorContextResolution,
    CssRuleContext,
    CssViewport,
    PageCssTarget,
    ScssVariable,
  } from "$lib/css/contracts";
  import type { CssProperty } from "$lib/css/property-contract";
  import type { InspectorSelectionSummarySnapshot } from "$lib/editor/contracts";
  import type { InstalledFontVariationAxis } from "$lib/fonts/contracts";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";

  const pseudoOptions = $derived([
    { label: t("inspector-css-pseudo-base"), suffix: "" },
    { label: ":hover", suffix: ":hover" },
    { label: ":focus", suffix: ":focus" },
    { label: ":active", suffix: ":active" },
  ]);

  let {
    selectionSummary = null,
    selectedClass = null,
    effectiveSelector = null,
    viewportLabel = "Desktop",
    previewDevice = "desktop",
    pageCssTarget = null,
    resolution = null,
    cssRuleContext = null,
    classRules = [],
    pendingValues = {},
    scssVariables = [],
    fontFamilies = [],
    installedFontAxes = [],
    scannedAssets = [],
    loadingClassRules = false,
    projectionTransitioning = false,
    editingReady = false,
    selectorSuffix = "",
    customSuffix = "",
    usingCustom = false,
    cssPropertyEdit,
    gridOverlayEnabled = false,
    onGridOverlayChange,
    selectCssVariant,
  }: {
    selectionSummary?: InspectorSelectionSummarySnapshot | null;
    selectedClass?: string | null;
    effectiveSelector?: string | null;
    viewportLabel?: string;
    previewDevice?: CssViewport;
    pageCssTarget?: PageCssTarget | null;
    resolution?: CssInspectorContextResolution | null;
    cssRuleContext?: CssRuleContext | null;
    classRules?: CssProperty[];
    pendingValues?: Record<string, string>;
    scssVariables?: ScssVariable[];
    fontFamilies?: string[];
    installedFontAxes?: InstalledFontVariationAxis[];
    scannedAssets?: ProjectFile[];
    loadingClassRules?: boolean;
    projectionTransitioning?: boolean;
    editingReady?: boolean;
    selectorSuffix?: string;
    customSuffix?: string;
    usingCustom?: boolean;
    cssPropertyEdit: CssPropertyEditController;
    gridOverlayEnabled?: boolean;
    onGridOverlayChange?: (enabled: boolean) => void;
    selectCssVariant: (suffix: string) => void;
  } = $props();

  let customEditorOpen = $state(false);
  const customEditorVisible = $derived(usingCustom || customEditorOpen);
  const hasCssSubject = $derived(
    selectionSummary?.state === "resolved"
      && (
        selectionSummary.subjectKind === "htmlElement"
        || selectionSummary.subjectKind === "runtimeElement"
        || selectionSummary.subjectKind === "cssRule"
      ),
  );
</script>

{#if hasCssSubject && selectedClass}
  <section
    class="css-pane"
    aria-busy={loadingClassRules || projectionTransitioning || (!editingReady && resolution?.state !== "ambiguous")}
  >
    <div class="css-context" inert={!editingReady || projectionTransitioning}>
      <div class="group-header">
        <h3>{t("inspector-css-rules")}</h3>
        <code>{effectiveSelector}</code>
        <span class="viewport-pill">{viewportLabel}</span>
      </div>

      <div class="pseudo-bar ui-segmented compact" aria-label={t("inspector-css-rules")}>
        {#each pseudoOptions as opt}
          <button
            type="button"
            class="pseudo-btn ui-segmented-option"
            class:active={!usingCustom && selectorSuffix === opt.suffix}
            onclick={() => {
              customEditorOpen = false;
              selectCssVariant(opt.suffix);
            }}
          >{opt.label}</button>
        {/each}
        <button
          type="button"
          class="pseudo-btn pseudo-custom-btn ui-segmented-option"
          class:active={customEditorVisible}
          title={t("inspector-css-custom-selector")}
          aria-label={t("inspector-css-toggle-custom")}
          onclick={() => {
            const next = !customEditorVisible;
            customEditorOpen = next;
            if (!next) selectCssVariant("");
          }}
        >
          <IconPlus size={13} stroke={1.9} />
        </button>
      </div>

      {#if customEditorVisible}
        <input
          type="text"
          class="custom-selector-input ui-input compact code"
          placeholder={t("inspector-css-custom-placeholder")}
          value={customSuffix}
          oninput={(event) => selectCssVariant(event.currentTarget.value)}
          onkeydown={(event) => {
            if (event.key === "Escape") {
              customEditorOpen = false;
              selectCssVariant("");
            }
          }}
        />
      {/if}
    </div>

    {#if loadingClassRules}
      <p class="hint">{t("inspector-css-loading")}</p>
    {:else if resolution?.state === "ambiguous"}
      <p class="hint">
        {t("inspector-css-source-ambiguous", {
          selector: effectiveSelector ?? "",
          files: resolution.candidates.map((candidate) => candidate.file).join(", "),
        })}
      </p>
    {:else if !editingReady}
      <p class="hint">{t("inspector-css-target-synchronizing")}</p>
    {:else}
      {#if pageCssTarget?.targetKind === "reusable"}
        <p class="hint css-delivery-note">
          {#if pageCssTarget.consumerFiles.length > 0}
            {t("inspector-css-reusable-consumers", { files: pageCssTarget.consumerFiles.join(", ") })}
          {:else}
            {t("inspector-css-reusable-preview-only")}
          {/if}
        </p>
      {/if}
      {#if resolution?.state === "creation" && pageCssTarget}
        <p class="hint">
          {t("inspector-css-rule-will-create", { selector: effectiveSelector ?? "", file: pageCssTarget.file })}
          {#if pageCssTarget.href && !pageCssTarget.linked}{t("inspector-css-will-link", { href: pageCssTarget.href })}{/if}
        </p>
      {:else if previewDevice !== "desktop" && cssRuleContext?.hasBaseRule && !cssRuleContext?.hasViewportRule}
        <p class="hint">
          {t("inspector-css-no-override", {
            viewport: `${viewportLabel}${cssRuleContext.resolvedBreakpoint ? ` (${cssRuleContext.resolvedBreakpoint})` : ""}`,
          })}
        </p>
      {:else if classRules.length === 0}
        <p class="hint">{t("inspector-css-rule-not-yet", { selector: effectiveSelector ?? "", viewport: viewportLabel })}</p>
      {/if}
      <ClassEditor
        {classRules}
        {pendingValues}
        {scssVariables}
        {fontFamilies}
        {installedFontAxes}
        {scannedAssets}
        {cssPropertyEdit}
        canonicalBackground={cssRuleContext?.background ?? null}
        canonicalGrid={cssRuleContext?.grid ?? null}
        gridViewport={cssRuleContext?.viewport ?? previewDevice}
        gridHasBaseRule={cssRuleContext?.hasBaseRule ?? false}
        gridHasViewportRule={cssRuleContext?.hasViewportRule ?? false}
        {gridOverlayEnabled}
        {onGridOverlayChange}
      />
    {/if}
  </section>
{:else if !hasCssSubject}
  <InspectorEmptyState kind="css" title="CSS" description={t("inspector-css-select-element")} />
{:else if selectionSummary?.classes.length}
  <InspectorEmptyState kind="css" title="CSS" description={t("inspector-css-select-class")} />
{/if}

<style>
  .hint {
    margin: 10px 12px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .css-delivery-note {
    overflow-wrap: anywhere;
  }

  .css-pane {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    flex-direction: column;
  }

  .css-context {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 9px 10px 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .group-header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
    min-width: 0;
  }

  .group-header h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0;
  }

  .group-header code {
    overflow: hidden;
    max-width: 100%;
    font-size: 12px;
    padding: 2px 6px;
    border-radius: calc(var(--radius-control) - 3px);
    background: var(--code-bg);
    color: var(--code-text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .viewport-pill {
    display: inline-flex;
    align-items: center;
    min-height: 18px;
    padding: 0 6px;
    margin-left: auto;
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--material-control);
    box-shadow: var(--shadow-control);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0;
    text-transform: none;
  }

  .pseudo-bar {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr)) var(--control-height-compact);
  }

  .pseudo-btn {
    min-width: 0;
    font-family: var(--font-mono);
  }

  .pseudo-custom-btn {
    font-family: inherit;
    font-size: 14px;
    padding: 0;
  }

  .custom-selector-input {
    min-height: var(--control-height-compact);
  }
</style>
