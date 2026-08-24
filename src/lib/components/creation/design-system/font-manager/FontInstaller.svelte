<script lang="ts">
  import { IconAlertTriangle, IconBrandGoogle, IconCircleCheck, IconDownload, IconFolderOpen, IconSearch, IconTypography, IconX } from "@tabler/icons-svelte";
  import CheckboxControl from "$lib/components/ui/CheckboxControl.svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { FontCreateSource, FontManagerController } from "./controller.svelte";

  let { controller }: { controller: FontManagerController } = $props();
  const sources: FontCreateSource[] = ["google", "bundled", "local"];

  function handleSourceKeydown(event: KeyboardEvent, source: FontCreateSource) {
    const index = sources.indexOf(source);
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % sources.length;
    else if (event.key === "ArrowLeft") nextIndex = (index - 1 + sources.length) % sources.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = sources.length - 1;
    else return;
    event.preventDefault();
    const next = sources[nextIndex];
    if (!next) return;
    controller.selectFontCreateSource(next);
    requestAnimationFrame(() => document.getElementById(`font-source-tab-${next}`)?.focus());
  }
</script>

<form class="resource-form" onsubmit={(event) => { event.preventDefault(); void controller.installSelectedFont(); }}>
  <header class="detail-heading">
    <div><span class="detail-kicker">{t("design-new-resource")}</span><h2>{t("design-add-resource", { resource: t("design-view-fonts").toLocaleLowerCase() })}</h2><p>{t("design-create-description")}</p></div>
    <button class="ui-icon-button ui-close-button" type="button" aria-label={t("design-cancel-create")} disabled={controller.mutating} onclick={() => controller.resetPanel()}><IconX size={14} /></button>
  </header>

  <div class="ui-tabs font-source-switch" role="tablist" aria-label={t("design-font-source") }>
    {#each sources as source (source)}
      <button
        id={`font-source-tab-${source}`}
        class="ui-tab"
        type="button"
        role="tab"
        aria-selected={controller.fontCreateSource === source}
        class:active={controller.fontCreateSource === source}
        tabindex={controller.fontCreateSource === source ? 0 : -1}
        disabled={controller.mutating || controller.localFontPlanning}
        onclick={() => controller.selectFontCreateSource(source)}
        onkeydown={(event) => handleSourceKeydown(event, source)}
      >
        {#if source === "google"}<IconBrandGoogle size={14} /> Google Fonts
        {:else if source === "bundled"}<IconTypography size={14} /> {t("design-included-library")}
        {:else}<IconFolderOpen size={14} /> {t("design-from-computer")}{/if}
      </button>
    {/each}
  </div>

  {#if controller.fontCreateSource === "google"}
    <div class="google-source"><span class="google-source-title"><IconBrandGoogle size={15} stroke={1.9} /> {t("design-google-catalog")}</span><p>{t("design-google-description")}</p></div>
    <div class="font-search-field"><span>{t("design-search-family")}</span><span class="google-search"><input class="ui-input" bind:value={controller.googleFontQuery} disabled={controller.mutating || controller.googleFontLoading} placeholder="Space Grotesk" onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); void controller.searchGoogleFontCatalog(); } }} /><button class="ui-button" type="button" disabled={controller.mutating || controller.googleFontLoading} onclick={() => controller.searchGoogleFontCatalog()}><IconSearch size={14} /> {controller.googleFontLoading ? t("design-searching") : t("design-search")}</button></span></div>
    {#if controller.googleFontError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {controller.googleFontError}</p>
    {:else if controller.googleFontLoading}<div class="google-state">{t("design-loading-google-catalog")}</div>
    {:else}
      <div class="google-results" aria-label={t("design-google-families-label") }>
        {#each controller.googleFontResults as font (font.family)}
          <button type="button" class="ui-entity-selectable" data-ui-selected={controller.selectedGoogleFont?.family === font.family ? "true" : undefined} aria-pressed={controller.selectedGoogleFont?.family === font.family} onclick={() => controller.selectGoogleFont(font)}>
            <span class="google-font-sample">Ag</span><span><strong>{font.family}</strong><small>{font.category ?? t("design-web-font")} · {t("design-variants-count", { count: font.variants.length })}</small></span>{#if controller.selectedGoogleFont?.family === font.family}<IconCircleCheck size={16} stroke={2} />{/if}
          </button>
        {:else}<div class="google-state">{t("design-empty-google-search")}</div>{/each}
      </div>
    {/if}
    {#if controller.selectedGoogleFont}
      <div class="font-install-options">
        <span>{t("design-installed-styles")}</span><div class="weight-options">{#each controller.availableGoogleStyles(controller.selectedGoogleFont) as style (style)}<button type="button" class:selected={controller.formGoogleStyles.includes(style)} aria-pressed={controller.formGoogleStyles.includes(style)} disabled={controller.mutating} onclick={() => controller.toggleGoogleStyle(style)}>{style === "normal" ? t("design-style-normal") : t("design-style-italic")}</button>{/each}</div>
        <span>{t("design-installed-weights")}</span><div class="weight-options">{#each controller.selectedGoogleFont.weights as weight (weight)}<button type="button" class:selected={controller.selectedGoogleWeights().includes(weight)} aria-pressed={controller.selectedGoogleWeights().includes(weight)} disabled={controller.mutating || controller.formVariableFont} onclick={() => controller.toggleGoogleWeight(weight)}>{weight}</button>{/each}</div>
        {#if controller.selectedGoogleFont.axes.some((axis) => axis.tag === "wght")}<CheckboxControl compact label={t("design-full-variable-range")} checked={controller.formVariableFont} disabled={controller.mutating} onchange={(checked) => controller.setVariableFont(checked)} />{/if}
        {#if controller.advancedGoogleAxes(controller.selectedGoogleFont).length}
          <span>{t("design-advanced-axes")}</span><div class="axis-options">{#each controller.advancedGoogleAxes(controller.selectedGoogleFont) as axis (axis.tag)}<button type="button" class:selected={controller.formGoogleAxes.includes(axis.tag)} aria-pressed={controller.formGoogleAxes.includes(axis.tag)} disabled={controller.mutating} title={t("design-google-axis-range", { start: axis.start, end: axis.end })} onclick={() => controller.toggleGoogleAxis(axis.tag)}><strong>{axis.tag}</strong><small>{axis.start}–{axis.end}</small></button>{/each}</div><small class="axis-help">{t("design-axis-help")}</small>
        {/if}
        <label class="ui-form-field font-character-set"><span class="ui-form-label">{t("design-character-optimization")}</span><textarea class="ui-textarea" bind:value={controller.formGoogleCharacterSet} disabled={controller.mutating} maxlength="640" rows="3" placeholder={t("design-character-example")}></textarea><small class="ui-form-help">{t("design-character-help")}</small></label>
      </div>
    {/if}
  {:else if controller.fontCreateSource === "bundled"}
    <div class="google-source"><span class="google-source-title"><IconTypography size={15} stroke={1.9} /> {t("design-bundled-catalog")}</span><p>{t("design-bundled-description")}</p></div>
    <div class="bundled-font-filters"><input class="ui-input" bind:value={controller.bundledFontQuery} disabled={controller.mutating || controller.bundledFontLoading} aria-label={t("design-search-family")} placeholder={t("design-search-family")} /><SelectControl value={controller.bundledFontCategory} options={[{ value: "all", label: t("design-all-categories") }, ...controller.bundledFontCategories]} disabled={controller.mutating || controller.bundledFontLoading} ariaLabel={t("design-font-category")} onchange={(value) => { controller.bundledFontCategory = value; }} /></div>
    {#if controller.bundledFontError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {controller.bundledFontError}</p>
    {:else if controller.bundledFontLoading}<div class="google-state">{t("design-loading-bundled-catalog")}</div>
    {:else}<div class="google-results" aria-label={t("design-bundled-families-label")}>{#each controller.visibleBundledFonts as font (font.id)}<button type="button" class="ui-entity-selectable" data-ui-selected={controller.selectedBundledFontId === font.id ? "true" : undefined} aria-pressed={controller.selectedBundledFontId === font.id} onclick={() => controller.selectBundledFont(font)}><span class="google-font-sample" class:bundled-font-preview={controller.selectedBundledFontId === font.id && !controller.bundledFontPreviewLoading && !controller.bundledFontPreviewError}>Ag</span><span><strong>{font.family}</strong><small>{font.category} · {font.weightRange.start}–{font.weightRange.end} · {Math.max(1, Math.round(font.sizeBytes / 1024))} KB</small></span>{#if controller.selectedBundledFontId === font.id}<IconCircleCheck size={16} stroke={2} />{/if}</button>{:else}<div class="google-state">{t("design-empty-bundled-search")}</div>{/each}</div>{/if}
    {#if controller.selectedBundledFont}<div class="font-install-options bundled-font-details"><span>{t("design-bundled-preview")}</span><strong class="bundled-font-preview-text">{t("design-bundled-preview-text")}</strong><small>{controller.bundledFontPreviewLoading ? t("design-bundled-preview-loading") : controller.bundledFontPreviewError || t("design-bundled-preview-ready")}</small><span>{t("design-bundled-contract")}</span><small>WOFF2 · Latin + Latin Extended · wght {controller.selectedBundledFont.weightRange.start}–{controller.selectedBundledFont.weightRange.end} · {controller.selectedBundledFont.styles.join(" + ")} · {controller.selectedBundledFont.license.description ?? "—"}</small></div>{/if}
  {:else}
    <div class="google-source"><span class="google-source-title"><IconFolderOpen size={15} stroke={1.9} /> {t("design-local-files")}</span><p>{t("design-local-description")}</p></div>
    <button class="ui-button local-font-picker" type="button" disabled={controller.mutating || controller.localFontPlanning} onclick={() => controller.chooseAndPlanLocalFonts()}><IconFolderOpen size={15} />{controller.localFontPlanning ? t("design-analyzing-rust") : controller.localFontPaths.length ? t("design-choose-other-files") : t("design-choose-font-files")}</button>
    {#if controller.localFontPlanning}<div class="google-state">{t("design-checking-fonts")}</div>
    {:else if controller.localFontPlan}<div class="local-font-plan" aria-label={t("design-local-plan-label")}><div class="local-plan-summary"><strong>{controller.localFontPlan.families.map((family) => family.family).join(", ")}</strong><small>{t("design-plan-files", { count: controller.localFontPlan.files.length })} · {controller.localFontPlan.families.some((family) => family.variable) ? ` ${t("design-includes-variable-font")} ·` : ""}{controller.localFontPlan.stylesheetPath}</small></div>{#each controller.localFontPlan.files as file (file.destinationPath)}<div class="local-plan-file"><span><strong>{file.subfamily ?? `${file.weightRange ? `${file.weightRange.start}–${file.weightRange.end}` : file.weight ?? 400} ${file.style}`}</strong><small>{file.family} · {file.format.toUpperCase()} · {Math.max(1, Math.round(file.sizeBytes / 1024))} KB</small></span><code>{file.destinationPath}</code></div>{/each}</div>{#each controller.localFontPlan.warnings as warning}<p class="ui-message warning"><IconAlertTriangle size={14} /> {warning}</p>{/each}{#each controller.localFontPlan.conflicts as conflict}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {conflict}</p>{/each}
    {:else}<div class="google-state">{t("design-local-selection-help")}</div>{/if}
  {/if}

  {#if controller.formError}<p class="ui-message error" role="alert"><IconAlertTriangle size={14} /> {controller.formError}</p>{/if}
  <div class="form-actions"><button class="ui-button" type="button" disabled={controller.mutating} onclick={() => controller.resetPanel()}>{t("design-cancel")}</button><button class="ui-button primary" type="submit" disabled={controller.mutating || !controller.formReady}><IconDownload size={14} /> {controller.mutating ? t("design-installing-rust") : controller.fontCreateSource === "local" ? t("design-confirm-import") : t("design-install-project")}</button></div>
</form>

<style>
  .font-source-switch { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .font-source-switch .ui-tab { width: 100%; min-width: 0; }
  .google-source { display: grid; gap: 4px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .google-source-title { display: flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; }
  .google-source p { margin: 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.4; }
  .font-search-field { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .google-search { display: grid; grid-template-columns: minmax(0, 1fr) auto; }
  .google-search :global(.ui-input) { min-width: 0; border-right: 0; border-radius: 6px 0 0 6px; }
  .google-search :global(.ui-button) { min-width: 82px; border-radius: 0 6px 6px 0; }
  .google-results { display: grid; max-height: 250px; overflow: auto; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .google-results > button { display: grid; grid-template-columns: 34px minmax(0, 1fr) 18px; gap: 8px; align-items: center; min-height: 49px; padding: 6px 8px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .google-results > button > span:nth-child(2) { display: grid; min-width: 0; gap: 2px; }
  .google-results strong, .google-results small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .google-results strong { color: var(--text-strong); font-size: 12px; }
  .google-results small, .google-state { color: var(--wb-text-muted); font-size: 11px; }
  .google-font-sample { display: grid; width: 31px; height: 31px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 16px; font-weight: 650; }
  .google-font-sample.bundled-font-preview, .bundled-font-preview-text { font-family: "Pana Studio Bundled Font Preview", system-ui, sans-serif; }
  .bundled-font-filters { display: grid; grid-template-columns: minmax(0, 1fr) 120px; gap: 6px; }
  .google-state { display: grid; min-height: 58px; padding: 10px; place-items: center; text-align: center; }
  .font-install-options { display: grid; gap: 7px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .font-install-options > span { color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  .weight-options { display: flex; flex-wrap: wrap; gap: 5px; }
  .weight-options button, .axis-options button { min-height: 28px; padding: 0 7px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-chrome); font-size: 11px; }
  .weight-options button.selected, .axis-options button.selected { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-weight: 750; }
  .axis-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  .axis-options button { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 6px; text-align: left; }
  .font-character-set :global(.ui-textarea) { min-height: 56px; }
  .local-font-picker { display: inline-flex; min-height: 34px; align-items: center; justify-content: center; gap: 6px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 700; }
  .local-font-plan { display: grid; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .local-plan-summary, .local-plan-file { display: grid; gap: 3px; padding: 9px; border-bottom: 1px solid var(--wb-border-subtle); }
  .local-plan-file span { display: flex; justify-content: space-between; gap: 8px; }
  .local-plan-summary small, .local-plan-file small, .local-plan-file code, .bundled-font-details small, .axis-help { color: var(--wb-text-muted); font-size: 11px; }
</style>
