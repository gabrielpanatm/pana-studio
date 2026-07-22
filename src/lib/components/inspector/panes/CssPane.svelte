<script lang="ts">
  import ClassEditor from "$lib/components/inspector/ClassEditor.svelte";
  import type { CssProperty, CssRuleContext, PageCssTarget, ProjectFile, ScssVariable, SelectionInfo } from "$lib/types";
  import type { CssViewport } from "$lib/project/io";
  import type { CssPropertyEditController } from "$lib/inspector/css-property-edit";

  const PSEUDO_OPTIONS = [
    { label: "base", suffix: "" },
    { label: ":hover", suffix: ":hover" },
    { label: ":focus", suffix: ":focus" },
    { label: ":active", suffix: ":active" },
  ];

  let {
    selectedElement = null,
    selectedClass = null,
    effectiveSelector = null,
    activeSuffix = "",
    viewportLabel = "Desktop",
    previewDevice = "desktop",
    targetCssFile = "",
    pageCssTarget = null,
    cssFileCount = 0,
    cssRuleContext = null,
    classRules = [],
    pendingValues = {},
    scssVariables = [],
    scannedAssets = [],
    loadingClassRules = false,
    searchingClass = false,
    selectorSuffix = "",
    customSuffix = "",
    usingCustom = false,
    cssPropertyEdit,
    searchClassInAllFiles,
    setSelectorSuffix,
    setCustomSuffix,
    setUsingCustom,
  }: {
    selectedElement?: SelectionInfo | null;
    selectedClass?: string | null;
    effectiveSelector?: string | null;
    activeSuffix?: string;
    viewportLabel?: string;
    previewDevice?: CssViewport;
    targetCssFile?: string;
    pageCssTarget?: PageCssTarget | null;
    cssFileCount?: number;
    cssRuleContext?: CssRuleContext | null;
    classRules?: CssProperty[];
    pendingValues?: Record<string, string>;
    scssVariables?: ScssVariable[];
    scannedAssets?: ProjectFile[];
    loadingClassRules?: boolean;
    searchingClass?: boolean;
    selectorSuffix?: string;
    customSuffix?: string;
    usingCustom?: boolean;
    cssPropertyEdit: CssPropertyEditController;
    searchClassInAllFiles: () => void;
    setSelectorSuffix: (suffix: string) => void;
    setCustomSuffix: (suffix: string) => void;
    setUsingCustom: (enabled: boolean) => void;
  } = $props();
</script>

{#if selectedClass}
  <section class="inspector-group">
    <div class="group-header">
      <h3>Reguli <code>{effectiveSelector}</code> <span class="viewport-pill">{viewportLabel}</span></h3>
    </div>

    <div class="pseudo-bar">
      {#each PSEUDO_OPTIONS as opt}
        <button
          type="button"
          class="pseudo-btn"
          class:active={!usingCustom && selectorSuffix === opt.suffix}
          onclick={() => { setSelectorSuffix(opt.suffix); setUsingCustom(false); setCustomSuffix(""); }}
        >{opt.label}</button>
      {/each}
      <button
        type="button"
        class="pseudo-btn pseudo-custom-btn"
        class:active={usingCustom}
        title="Selector personalizat"
        onclick={() => {
          const next = !usingCustom;
          setUsingCustom(next);
          if (!next) setCustomSuffix("");
        }}
      >+</button>
    </div>

    {#if usingCustom}
      <input
        type="text"
        class="custom-selector-input"
        placeholder=":nth-child(2) sau .parinte &"
        value={customSuffix}
        oninput={(event) => setCustomSuffix(event.currentTarget.value)}
        onkeydown={(event) => {
          if (event.key === "Escape") {
            setUsingCustom(false);
            setCustomSuffix("");
          }
        }}
      />
    {/if}

    {#if loadingClassRules}
      <p class="hint">Se încarcă...</p>
    {:else if previewDevice === "desktop" && !cssRuleContext?.hasBaseRule && !activeSuffix && !pageCssTarget?.pageOwned}
      <p class="hint">Regula <code>{effectiveSelector}</code> nu a fost găsită pentru {viewportLabel} în <code>{targetCssFile}</code>.</p>
      <button class="save-css" type="button" onclick={searchClassInAllFiles} disabled={searchingClass}>
        {searchingClass ? "Se caută..." : "Caută în toate fișierele SCSS"}
      </button>
    {:else if previewDevice !== "desktop" && !cssRuleContext?.hasBaseRule && !activeSuffix && !pageCssTarget?.pageOwned}
      <p class="hint">
        Regula de bază <code>{effectiveSelector}</code> nu a fost găsită în <code>{targetCssFile}</code>.
        Comută pe Desktop pentru detectarea fișierului sursă.
      </p>
    {:else}
      {#if pageCssTarget?.pageOwned && !cssRuleContext?.hasBaseRule && !activeSuffix}
        <p class="hint">
          Regula <code>{effectiveSelector}</code> va fi creată în <code>{pageCssTarget.file}</code>.
          {#if pageCssTarget.href && !pageCssTarget.linked}La salvare va fi legată în template ca <code>{pageCssTarget.href}</code>.{/if}
        </p>
      {:else if previewDevice !== "desktop" && cssRuleContext?.hasBaseRule && !cssRuleContext?.hasViewportRule}
        <p class="hint">
          Nu există override pentru {viewportLabel}{cssRuleContext.resolvedBreakpoint ? ` (${cssRuleContext.resolvedBreakpoint})` : ""}.
          Valorile afișate vin din Desktop; modificările salvate se vor scrie în media query.
        </p>
      {:else if classRules.length === 0}
        <p class="hint">Regula <code>{effectiveSelector}</code> nu există încă pentru {viewportLabel} — modifică proprietăți și salvează cu Ctrl+S.</p>
      {/if}
      <ClassEditor
        {classRules}
        {pendingValues}
        {scssVariables}
        {scannedAssets}
        {cssPropertyEdit}
      />
    {/if}
  </section>
{:else if !selectedElement}
  <p class="hint">Dă click pe un element din preview sau pe un selector de clasă din Code.</p>
{:else if selectedElement.classes.length}
  <p class="hint">Dă click pe o clasă din cardul de sus pentru a vedea regulile SCSS.</p>
{:else if cssFileCount === 0}
  <p class="hint">Nu există fișiere SCSS/CSS detectate pentru editarea claselor.</p>
{/if}

<style>
  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.45;
  }

  .inspector-group {
    display: grid;
    gap: 9px;
    padding: 10px;
    border: 1px solid var(--border-2);
    border-radius: 9px;
    background: var(--surface-2);
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .group-header h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 900;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .group-header code {
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--code-bg);
    color: var(--code-text);
  }

  .viewport-pill {
    display: inline-flex;
    align-items: center;
    min-height: 18px;
    padding: 0 6px;
    border: 1px solid color-mix(in srgb, var(--brand) 28%, transparent);
    border-radius: 999px;
    color: var(--brand-strong);
    background: var(--brand-soft);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0;
    text-transform: none;
  }

  .pseudo-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .pseudo-btn {
    padding: 2px 7px;
    border: 1px solid var(--border-4);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text-muted);
    font-size: 12px;
    font-family: "JetBrains Mono", monospace;
    cursor: pointer;
    transition: border-color 80ms, color 80ms, background 80ms;
    white-space: nowrap;
  }

  .pseudo-btn:hover {
    border-color: var(--brand);
    color: var(--text);
  }

  .pseudo-btn.active {
    border-color: var(--brand);
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .pseudo-custom-btn {
    font-family: inherit;
    font-size: 14px;
    padding: 2px 8px;
  }

  .custom-selector-input {
    width: 100%;
    height: 26px;
    padding: 0 7px;
    border: 1px solid var(--brand);
    border-radius: 6px;
    background: var(--surface-5);
    color: var(--text);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    outline: none;
  }

  .save-css {
    min-height: 32px;
    border: 1px solid var(--brand);
    border-radius: 8px;
    color: #ffffff;
    font-size: 13px;
    font-weight: 800;
    background: var(--brand);
    cursor: pointer;
  }

  .save-css:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
