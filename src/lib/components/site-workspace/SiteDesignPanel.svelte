<script lang="ts">
  import {
    IconCode,
    IconColorSwatch,
    IconExternalLink,
    IconPalette,
    IconRuler2,
    IconTypography,
  } from "@tabler/icons-svelte";
  import FontManagerPanel from "$lib/components/site-workspace/fonts/FontManagerPanel.svelte";
  import type { FontRoleRow } from "$lib/fonts/model";
  import type {
    FontInventory,
    GoogleFontCatalogFamily,
    LocalFontFamily,
    ScssVariable,
    SourceGraphStyle,
  } from "$lib/types";
  import {
    buildDesignTokenGroups,
    colorPreviewValue,
    editableHexColor,
    safeCssPreviewValue,
    sourceDisplayPath,
    styleScopeLabel,
    tokenHumanLabel,
  } from "./workspace-model";

  let {
    variables = [],
    variablesError = "",
    styles = [],
    fontRoles = [],
    fontInventory = null,
    fontInventoryError = "",
    onOpenSource = () => {},
    onUpdateVariable = () => {},
    onRoleFamilyChange = () => {},
    onGenerateFontFace = () => {},
    onGenerateFontPreloads = () => {},
    onDownloadGoogleFont = async () => {},
    onSearchGoogleFonts = async () => [],
  }: {
    variables?: ScssVariable[];
    variablesError?: string;
    styles?: SourceGraphStyle[];
    fontRoles?: FontRoleRow[];
    fontInventory?: FontInventory | null;
    fontInventoryError?: string;
    onOpenSource?: (path: string) => void | Promise<void>;
    onUpdateVariable?: (variable: ScssVariable, value: string) => void | Promise<void>;
    onRoleFamilyChange?: (role: FontRoleRow, family: LocalFontFamily) => void | Promise<void>;
    onGenerateFontFace?: (family: LocalFontFamily) => void | Promise<void>;
    onGenerateFontPreloads?: () => void | Promise<void>;
    onDownloadGoogleFont?: (family: string, weights: number[], variable: boolean) => void | Promise<void>;
    onSearchGoogleFonts?: (query: string, limit?: number, offset?: number) => Promise<GoogleFontCatalogFamily[]>;
  } = $props();

  const groups = $derived(buildDesignTokenGroups(variables));
  const colors = $derived(groups.find((group) => group.id === "colors")?.variables ?? []);
  const spacing = $derived(groups.find((group) => group.id === "spacing")?.variables ?? []);
  const radii = $derived(groups.find((group) => group.id === "radius")?.variables ?? []);

  function updateColor(event: Event, variable: ScssVariable) {
    const input = event.currentTarget as HTMLInputElement;
    void onUpdateVariable(variable, input.value);
  }

  function fontPreviewStyle(role: FontRoleRow | undefined) {
    const family = safeCssPreviewValue(role?.variable?.value ?? "system-ui, sans-serif");
    return `font-family: ${family || "system-ui, sans-serif"}`;
  }

  function sizePreviewWidth(value: string, index: number) {
    const safe = safeCssPreviewValue(value);
    if (/^-?[0-9.]+(px|rem|em|%)$/.test(safe)) return `width: min(100%, max(18px, ${safe}))`;
    return `width: ${Math.min(92, 22 + index * 12)}%`;
  }

  function radiusPreviewStyle(value: string) {
    const safe = safeCssPreviewValue(value);
    return `border-radius: ${safe || "0"}`;
  }
</script>

<div class="design-stage">
  <header class="stage-heading">
    <div>
      <span class="section-kicker">Designul site-ului</span>
      <h1>Stilul vizual</h1>
      <p>Culorile, fonturile și proporțiile comune întregului website, prezentate ca un sistem vizual.</p>
    </div>
  </header>

  {#if variablesError}
    <div class="design-error">{variablesError}</div>
  {/if}

  <div class="design-canvas">
    <section class="brand-preview">
      <header>
        <span><IconPalette size={18} stroke={1.8} /></span>
        <div><small>Previzualizare identitate</small><h2>Cum arată site-ul</h2></div>
      </header>
      <div class="sample-surface">
        <span class="sample-label">Titlu de pagină</span>
        <h3 style={fontPreviewStyle(fontRoles.find((role) => role.id === "display") ?? fontRoles[0])}>Un website clar, memorabil și coerent.</h3>
        <p style={fontPreviewStyle(fontRoles.find((role) => role.id === "body") ?? fontRoles[0])}>Acesta este un exemplu de text folosit pentru a vedea împreună tipografia, culorile și ritmul vizual al site-ului.</p>
        <div class="sample-actions">
          <span class="sample-primary">Acțiune principală</span>
          <span>Acțiune secundară</span>
        </div>
      </div>
    </section>

    <section class="visual-section color-section">
      <header>
        <div class="section-icon color"><IconColorSwatch size={19} stroke={1.8} /></div>
        <div><small>Paletă</small><h2>Culorile site-ului</h2><p>Selectează o mostră pentru a schimba direct culorile simple.</p></div>
      </header>
      <div class="color-grid">
        {#each colors as variable}
          {@const editable = editableHexColor(variable.value)}
          <div class="color-card">
            <label class:editable>
              <span class="swatch" style={`background: ${colorPreviewValue(variable.value)}`}></span>
              {#if editable}
                <input type="color" value={editable} aria-label={`Schimbă ${tokenHumanLabel(variable.name)}`} onchange={(event) => updateColor(event, variable)} />
              {/if}
            </label>
            <div><strong>{tokenHumanLabel(variable.name)}</strong><code>{variable.value}</code></div>
            <button type="button" title="Deschide variabila în cod" onclick={() => { void onOpenSource(variable.file); }}><IconCode size={14} /></button>
          </div>
        {:else}
          <p class="empty-line">Nu au fost detectate culori SCSS.</p>
        {/each}
      </div>
    </section>

    <section class="visual-section typography-section">
      <header>
        <div class="section-icon typography"><IconTypography size={19} stroke={1.8} /></div>
        <div><small>Tipografie</small><h2>Fonturile site-ului</h2><p>Rolurile de text recunoscute în sistemul de design.</p></div>
      </header>
      <div class="type-grid">
        {#each fontRoles as role}
          <article class:missing={!role.variable}>
            <div><span>{role.label}</span><small>{role.variable ? `$${role.variable.name}` : "Rol nedetectat"}</small></div>
            <strong style={fontPreviewStyle(role)}>Aa</strong>
            <p style={fontPreviewStyle(role)}>Construiește clar.</p>
            {#if role.variable}
              <button type="button" onclick={() => { const file = role.variable?.file; if (file) void onOpenSource(file); }}><IconExternalLink size={14} /> Configurează</button>
            {/if}
          </article>
        {:else}
          <p class="empty-line">Nu au fost detectate roluri tipografice.</p>
        {/each}
      </div>
    </section>

    <section class="visual-section rhythm-section">
      <header>
        <div class="section-icon rhythm"><IconRuler2 size={19} stroke={1.8} /></div>
        <div><small>Formă și ritm</small><h2>Spațiere și colțuri</h2><p>Dimensiunile care dau consistență componentelor site-ului.</p></div>
      </header>
      <div class="rhythm-grid">
        <div class="spacing-preview">
          <h3>Spațiere</h3>
          {#each spacing.slice(0, 8) as variable, index}
            <button type="button" onclick={() => { void onOpenSource(variable.file); }}>
              <span>{tokenHumanLabel(variable.name)}</span>
              <i style={sizePreviewWidth(variable.value, index)}></i>
              <code>{variable.value}</code>
            </button>
          {:else}
            <p class="empty-line">Nicio scară de spațiere detectată.</p>
          {/each}
        </div>
        <div class="radius-preview">
          <h3>Colțuri</h3>
          <div>
            {#each radii.slice(0, 8) as variable}
              <button type="button" onclick={() => { void onOpenSource(variable.file); }}>
                <i style={radiusPreviewStyle(variable.value)}></i>
                <span>{tokenHumanLabel(variable.name)}<code>{variable.value}</code></span>
              </button>
            {:else}
              <p class="empty-line">Nicio scară de radius detectată.</p>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <details class="advanced-section">
      <summary><IconTypography size={17} /><span><strong>Biblioteca de fonturi</strong><small>Fonturi locale, Google Fonts, roluri și preload-uri</small></span></summary>
      <FontManagerPanel
        roles={fontRoles}
        inventory={fontInventory}
        error={fontInventoryError}
        openSource={onOpenSource}
        {onRoleFamilyChange}
        {onGenerateFontFace}
        {onGenerateFontPreloads}
        {onDownloadGoogleFont}
        {onSearchGoogleFonts}
      />
    </details>

    <details class="advanced-section">
      <summary><IconCode size={17} /><span><strong>Fișiere și token-uri SCSS</strong><small>Inventarul tehnic complet al stilurilor</small></span></summary>
      <div class="technical-grid">
        <section>
          <h3>Fișiere de stil</h3>
          {#each styles as style}
            <button type="button" onclick={() => { void onOpenSource(style.file); }}>
              <span>{styleScopeLabel(style.scope)}</span><strong>{sourceDisplayPath(style.file)}</strong><IconCode size={14} />
            </button>
          {:else}<p class="empty-line">Nu există stiluri detectate.</p>{/each}
        </section>
        <section>
          <h3>Toate token-urile</h3>
          {#each groups as group}
            <details>
              <summary><span>{group.label}</span><strong>{group.variables.length}</strong></summary>
              {#each group.variables as variable}
                <button type="button" onclick={() => { void onOpenSource(variable.file); }}><code>${variable.name}</code><strong>{variable.value}</strong></button>
              {/each}
            </details>
          {/each}
        </section>
      </div>
    </details>
  </div>
</div>

<style>
  .design-stage {
    display: grid;
    align-content: start;
    gap: 15px;
    min-width: 0;
    min-height: 0;
    padding: 20px;
  }

  .stage-heading > div {
    display: grid;
    gap: 4px;
  }

  .section-kicker,
  .visual-section header small,
  .brand-preview header small,
  .type-grid article > div span {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3,
  p {
    margin: 0;
  }

  h1 {
    color: var(--text-strong);
    font-size: clamp(24px, 2.2vw, 34px);
    line-height: 1.08;
  }

  .stage-heading p,
  .visual-section header p {
    color: var(--text-muted);
    font-size: 12px;
  }

  .design-error {
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, #dc2626 45%, var(--border));
    border-radius: 9px;
    color: #b91c1c;
    background: color-mix(in srgb, #dc2626 6%, var(--surface));
    font-size: 11px;
    font-weight: 750;
  }

  .design-canvas {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 13px;
  }

  .brand-preview,
  .visual-section,
  .advanced-section {
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--border-2);
    border-radius: 12px;
    background: var(--surface-2);
  }

  .brand-preview,
  .color-section,
  .advanced-section {
    grid-column: 1 / -1;
  }

  .brand-preview {
    display: grid;
    grid-template-columns: minmax(220px, 280px) minmax(0, 1fr);
    min-height: 240px;
  }

  .brand-preview > header,
  .visual-section > header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px;
    align-content: start;
    padding: 16px;
  }

  .brand-preview > header > span,
  .section-icon {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border-radius: 9px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 10%, var(--surface));
  }

  .brand-preview header div,
  .visual-section header > div:last-child {
    display: grid;
    gap: 2px;
  }

  .brand-preview h2,
  .visual-section h2 {
    color: var(--text-strong);
    font-size: 15px;
  }

  .sample-surface {
    display: grid;
    align-content: center;
    gap: 14px;
    padding: 28px clamp(24px, 4vw, 64px);
    color: var(--text-strong);
    background:
      radial-gradient(circle at top right, color-mix(in srgb, var(--brand) 11%, transparent), transparent 45%),
      var(--surface);
  }

  .sample-label {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  .sample-surface h3 {
    max-width: 760px;
    font-size: clamp(28px, 3vw, 46px);
    line-height: 1.04;
  }

  .sample-surface p {
    max-width: 690px;
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1.6;
  }

  .sample-actions {
    display: flex;
    gap: 8px;
  }

  .sample-actions span {
    padding: 9px 13px;
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 11px;
    font-weight: 850;
  }

  .sample-actions .sample-primary {
    border-color: var(--brand);
    color: #fff;
    background: var(--brand);
  }

  .color-section,
  .typography-section,
  .rhythm-section {
    padding-bottom: 14px;
  }

  .section-icon.color { color: #dc2626; background: color-mix(in srgb, #dc2626 9%, var(--surface)); }
  .section-icon.typography { color: #7c3aed; background: color-mix(in srgb, #7c3aed 9%, var(--surface)); }
  .section-icon.rhythm { color: #2563eb; background: color-mix(in srgb, #2563eb 9%, var(--surface)); }

  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    cursor: pointer;
  }

  button:hover {
    border-color: color-mix(in srgb, var(--brand) 55%, var(--border));
  }

  .color-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 8px;
    padding: 0 14px;
  }

  .color-card {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) auto;
    gap: 9px;
    align-items: center;
    min-width: 0;
    min-height: 64px;
    padding: 7px;
    border: 1px solid var(--border-2);
    border-radius: 9px;
    background: var(--surface);
  }

  .color-card label {
    position: relative;
    display: block;
    width: 48px;
    height: 48px;
  }

  .swatch {
    position: absolute;
    inset: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background-image: linear-gradient(45deg, var(--surface-3) 25%, transparent 25%), linear-gradient(-45deg, var(--surface-3) 25%, transparent 25%);
  }

  .color-card input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
  }

  .color-card > div {
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .color-card strong {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  code {
    overflow: hidden;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .color-card button {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
  }

  .type-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    padding: 0 14px;
  }

  .type-grid article {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px 10px;
    padding: 12px;
    border: 1px solid var(--border-2);
    border-radius: 10px;
    background: var(--surface);
  }

  .type-grid article.missing {
    border-style: dashed;
  }

  .type-grid article > div {
    display: grid;
    gap: 2px;
  }

  .type-grid article > div small {
    color: var(--text-muted);
    font-size: 10px;
  }

  .type-grid article > strong {
    grid-row: span 2;
    align-self: center;
    color: var(--text-strong);
    font-size: 30px;
  }

  .type-grid article > p {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 17px;
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .type-grid article > button {
    grid-column: 1 / -1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 30px;
    font-size: 10px;
    font-weight: 800;
  }

  .rhythm-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 10px;
    padding: 0 14px;
  }

  .spacing-preview,
  .radius-preview {
    display: grid;
    align-content: start;
    gap: 6px;
    min-width: 0;
  }

  .spacing-preview h3,
  .radius-preview h3,
  .technical-grid h3 {
    color: var(--text-strong);
    font-size: 12px;
  }

  .spacing-preview button {
    display: grid;
    grid-template-columns: minmax(70px, auto) minmax(40px, 1fr) auto;
    gap: 7px;
    align-items: center;
    min-height: 31px;
    padding: 5px 7px;
    text-align: left;
  }

  .spacing-preview button span {
    overflow: hidden;
    font-size: 10px;
    font-weight: 750;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacing-preview i {
    display: block;
    max-width: 100%;
    height: 7px;
    border-radius: 999px;
    background: #2563eb;
  }

  .radius-preview > div {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .radius-preview button {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 7px;
    align-items: center;
    min-height: 48px;
    padding: 5px;
    text-align: left;
  }

  .radius-preview i {
    display: block;
    width: 40px;
    height: 32px;
    border: 2px solid #7c3aed;
    background: color-mix(in srgb, #7c3aed 9%, var(--surface));
  }

  .radius-preview span {
    display: grid;
    gap: 2px;
    overflow: hidden;
    font-size: 10px;
    font-weight: 800;
  }

  .advanced-section summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 9px;
    align-items: center;
    padding: 12px 14px;
    cursor: pointer;
    list-style: none;
  }

  .advanced-section > summary::-webkit-details-marker {
    display: none;
  }

  .advanced-section > summary {
    color: var(--brand);
  }

  .advanced-section > summary > span {
    display: grid;
    gap: 1px;
  }

  .advanced-section > summary strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .advanced-section > summary small {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 700;
  }

  .technical-grid {
    display: grid;
    grid-template-columns: minmax(0, .8fr) minmax(0, 1.2fr);
    gap: 10px;
    padding: 0 12px 12px;
  }

  .technical-grid > section {
    display: grid;
    align-content: start;
    gap: 5px;
    min-width: 0;
  }

  .technical-grid > section > button {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) auto;
    gap: 7px;
    align-items: center;
    min-height: 34px;
    padding: 5px 7px;
    text-align: left;
  }

  .technical-grid button span {
    color: var(--brand);
    font-size: 9px;
    font-weight: 900;
    text-transform: uppercase;
  }

  .technical-grid button strong {
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .technical-grid section > details {
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface);
  }

  .technical-grid section > details > summary {
    display: flex;
    justify-content: space-between;
    padding: 7px 9px;
    color: var(--text-strong);
    font-size: 10px;
    font-weight: 850;
    cursor: pointer;
  }

  .technical-grid section > details button {
    display: grid;
    grid-template-columns: minmax(100px, .7fr) minmax(0, 1fr);
    gap: 7px;
    width: calc(100% - 10px);
    margin: 0 5px 5px;
    padding: 6px;
    text-align: left;
  }

  .empty-line {
    color: var(--text-muted);
    font-size: 11px;
  }

  @media (max-width: 1050px) {
    .design-canvas,
    .technical-grid {
      grid-template-columns: minmax(0, 1fr);
    }

    .typography-section,
    .rhythm-section {
      grid-column: 1 / -1;
    }
  }
</style>
