<script lang="ts">
  import { IconDownload, IconExternalLink, IconSearch, IconTrash } from "@tabler/icons-svelte";
  import SelectControl from "$lib/components/ui/SelectControl.svelte";
  import {
    cleanFontStack,
    familyMatchesVariable,
    fontRootLabel,
    inventoryFontCount,
    type FontRoleRow,
  } from "$lib/fonts/model";
  import type { FontInventory, GoogleFontAxis, GoogleFontCatalogFamily, LocalFontFamily } from "$lib/types";

  let {
    roles = [],
    inventory = null,
    error = "",
    openSource = () => {},
    onRoleFamilyChange = () => {},
    onGenerateFontFace = () => {},
    onGenerateFontPreloads = () => {},
    onDownloadGoogleFont = async () => {},
    onSearchGoogleFonts = async () => [],
  }: {
    roles?: FontRoleRow[];
    inventory?: FontInventory | null;
    error?: string;
    openSource?: (path: string) => void | Promise<void>;
    onRoleFamilyChange?: (role: FontRoleRow, family: LocalFontFamily) => void | Promise<void>;
    onGenerateFontFace?: (family: LocalFontFamily) => void | Promise<void>;
    onGenerateFontPreloads?: () => void | Promise<void>;
    onDownloadGoogleFont?: (family: string, weights: number[], variable: boolean) => void | Promise<void>;
    onSearchGoogleFonts?: (query: string, limit?: number, offset?: number) => Promise<GoogleFontCatalogFamily[]>;
  } = $props();

  const GOOGLE_FONT_PAGE_SIZE = 40;
  const GOOGLE_FONT_PREVIEW_LIMIT = 120;

  let googleFamily = $state("");
  let googleWeights = $state("400,500,700");
  let googleVariable = $state(false);
  let googleBusy = $state(false);
  let googleSearchBusy = $state(false);
  let googleLoadingMore = $state(false);
  let googleSearchHasMore = $state(false);
  let googleDropdownOpen = $state(false);
  let googleResults = $state<GoogleFontCatalogFamily[]>([]);
  let googleSearchError = $state("");
  let googleSearchSerial = 0;

  const families = $derived(inventory?.families ?? []);
  const localFamilyOptions = $derived([
    { value: "", label: "Alege familie locală" },
    ...families.map((family) => ({ value: family.directory, label: family.family })),
  ]);
  const googlePreviewHref = $derived(googlePreviewStylesheetUrl(googleResults.slice(0, GOOGLE_FONT_PREVIEW_LIMIT)));

  $effect(() => {
    const query = googleFamily.trim();
    if (!googleDropdownOpen) return;
    const serial = ++googleSearchSerial;
    const timer = window.setTimeout(async () => {
      googleSearchBusy = true;
      googleSearchError = "";
      try {
        const results = await onSearchGoogleFonts(query, GOOGLE_FONT_PAGE_SIZE, 0);
        if (serial !== googleSearchSerial) return;
        googleResults = results;
        googleSearchHasMore = results.length >= GOOGLE_FONT_PAGE_SIZE;
      } catch (error) {
        if (serial !== googleSearchSerial) return;
        googleResults = [];
        googleSearchHasMore = false;
        googleSearchError = error instanceof Error ? error.message : String(error);
      } finally {
        if (serial === googleSearchSerial) googleSearchBusy = false;
      }
    }, query ? 220 : 0);
    return () => window.clearTimeout(timer);
  });

  function roleFamilyCount(family: LocalFontFamily) {
    return roles.filter((role) => familyMatchesVariable(family, role.variable)).length;
  }

  function fileWeightLabel(file: LocalFontFamily["files"][number]) {
    const weight = file.weightRange ? `${file.weightRange.start}..${file.weightRange.end}` : (file.weight ? String(file.weight) : "auto");
    const parts = [
      fileVariantLabel(file.fileName),
      weight,
      file.style,
      file.extension.toUpperCase(),
      fileSizeLabel(file.sizeBytes),
      unicodeRangeLabel(file.unicodeRange),
    ].filter(Boolean);
    return parts.join(" · ");
  }

  function fileVariantLabel(fileName: string) {
    const match = fileName.match(/-(\d+)\.[^.]+$/);
    return match ? `#${match[1]}` : "";
  }

  function fileSizeLabel(sizeBytes: number | null | undefined) {
    if (!sizeBytes || sizeBytes < 1) return "";
    if (sizeBytes < 1024) return `${sizeBytes} B`;
    if (sizeBytes < 1024 * 1024) {
      const value = sizeBytes / 1024;
      return `${value < 10 ? value.toFixed(1) : Math.round(value)} KB`;
    }
    return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function unicodeRangeLabel(unicodeRange: string | null | undefined) {
    if (!unicodeRange) return "";
    const firstRange = unicodeRange.split(",")[0]?.trim() ?? "";
    if (!firstRange) return "";
    return firstRange.length > 18 ? `${firstRange.slice(0, 18)}...` : firstRange;
  }

  function familyByDirectory(directory: string) {
    return families.find((family) => family.directory === directory) ?? null;
  }

  function selectGoogleFamily(font: GoogleFontCatalogFamily) {
    const axis = variableWeightAxis(font);
    googleFamily = font.family;
    googleVariable = Boolean(axis);
    googleWeights = axis ? `${Math.round(axis.start)}..${Math.round(axis.end)}` : recommendedWeights(font.weights).join(",");
    googleDropdownOpen = false;
  }

  function parseWeights(value: string) {
    const range = value.match(/(\d{3})\s*\.\.\s*(\d{3})/);
    if (range) {
      const start = clampWeight(Number.parseInt(range[1], 10));
      const end = clampWeight(Number.parseInt(range[2], 10));
      return [Math.min(start, end), Math.max(start, end)];
    }
    const parsed = value
      .split(/[,\s]+/)
      .map((entry) => Number.parseInt(entry.trim(), 10))
      .filter((weight) => Number.isFinite(weight) && weight >= 100 && weight <= 900 && weight % 100 === 0);
    return [...new Set(parsed)].sort((left, right) => left - right);
  }

  async function downloadGoogleFont() {
    const family = googleFamily.trim();
    if (!family || googleBusy) return;
    googleBusy = true;
    try {
      await onDownloadGoogleFont(family, parseWeights(googleWeights), googleVariable);
    } finally {
      googleBusy = false;
    }
  }

  async function loadMoreGoogleFonts() {
    if (googleSearchBusy || googleLoadingMore || !googleSearchHasMore) return;

    const query = googleFamily.trim();
    const serial = googleSearchSerial;
    googleLoadingMore = true;
    googleSearchError = "";
    try {
      const results = await onSearchGoogleFonts(query, GOOGLE_FONT_PAGE_SIZE, googleResults.length);
      if (serial !== googleSearchSerial) return;
      googleResults = mergeGoogleResults(googleResults, results);
      googleSearchHasMore = results.length >= GOOGLE_FONT_PAGE_SIZE;
    } catch (error) {
      if (serial !== googleSearchSerial) return;
      googleSearchError = error instanceof Error ? error.message : String(error);
    } finally {
      if (serial === googleSearchSerial) googleLoadingMore = false;
    }
  }

  function onGoogleDropdownScroll(event: Event) {
    const target = event.currentTarget as HTMLElement | null;
    if (!target) return;
    const remaining = target.scrollHeight - target.scrollTop - target.clientHeight;
    if (remaining < 96) void loadMoreGoogleFonts();
  }

  function mergeGoogleResults(current: GoogleFontCatalogFamily[], next: GoogleFontCatalogFamily[]) {
    const seen = new Set(current.map((font) => font.family));
    return [...current, ...next.filter((font) => !seen.has(font.family))];
  }

  function clampWeight(weight: number) {
    if (!Number.isFinite(weight)) return 400;
    return Math.min(900, Math.max(100, Math.round(weight / 100) * 100));
  }

  function recommendedWeights(weights: number[]) {
    const available = weights.length ? weights : [400, 700];
    const preferred = [400, 500, 600, 700].filter((weight) => available.includes(weight));
    return preferred.length ? preferred : available.slice(0, 4);
  }

  function variableWeightAxis(font: GoogleFontCatalogFamily): GoogleFontAxis | null {
    return font.axes.find((axis) => axis.tag.toLowerCase() === "wght") ?? null;
  }

  function googleFontSummary(font: GoogleFontCatalogFamily) {
    const axis = variableWeightAxis(font);
    if (axis) {
      return `${font.category ?? "google"} · variabil wght ${Math.round(axis.start)}-${Math.round(axis.end)} · WOFF2`;
    }
    return `${font.category ?? "google"} · ${recommendedWeights(font.weights).join(", ")} · WOFF2`;
  }

  function googlePreviewStylesheetUrl(fonts: GoogleFontCatalogFamily[]) {
    if (!fonts.length) return "";
    const params = fonts
      .map((font) => `family=${googlePreviewFamilyQuery(font)}`)
      .join("&");
    return `https://fonts.googleapis.com/css2?${params}&display=swap`;
  }

  function googlePreviewFamilyQuery(font: GoogleFontCatalogFamily) {
    const family = encodeURIComponent(font.family).replace(/%20/g, "+");
    const axis = variableWeightAxis(font);
    if (axis) {
      const start = clampWeight(axis.start);
      const end = clampWeight(axis.end);
      return `${family}:wght@${Math.min(start, end)}..${Math.max(start, end)}`;
    }
    return `${family}:wght@${recommendedWeights(font.weights).join(";")}`;
  }

  function fontFamilyStyle(family: string) {
    return `font-family: '${family.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}', system-ui, sans-serif;`;
  }
</script>

<svelte:head>
  {#if googlePreviewHref}
    <link rel="stylesheet" href={googlePreviewHref} />
  {/if}
</svelte:head>

<section class="font-manager" aria-label="Font Manager">
  <article class="font-panel font-roles-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Fonturi</p>
        <h3>Roluri globale</h3>
        <small>Aceste roluri vor modifica framework-ul CSS al site-ului, prin Save global.</small>
      </div>
      <span class="status-badge">{roles.filter((role) => role.variable).length}/{roles.length} detectate</span>
    </div>

    <div class="font-role-grid">
      {#each roles as role}
        <div class:missing={!role.variable}>
          <span>{role.label}</span>
          <strong>{role.variable ? cleanFontStack(role.variable.value) : "nedetectat"}</strong>
          <small>{role.variable ? `$${role.variable.name} · ${role.variable.file}` : role.description}</small>
          {#if role.dirty}
            <em>modificat, așteaptă Save global</em>
          {/if}
          <SelectControl
            disabled={!role.variable || families.length === 0}
            value=""
            options={localFamilyOptions}
            ariaLabel={`Alege familie locală pentru ${role.label}`}
            onchange={(value) => {
              const family = familyByDirectory(value);
              if (family) void onRoleFamilyChange(role, family);
            }}
          />
          {#if role.variable}
            <button type="button" onclick={() => openSource(role.variable?.file ?? "")}>
              <IconExternalLink size={14} stroke={1.9} />
              Variabilă
            </button>
          {:else}
            <button type="button" disabled>Configurează rol</button>
          {/if}
        </div>
      {/each}
    </div>
  </article>

  <article class="font-panel font-library-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Bibliotecă locală</p>
        <h3>{inventory?.families.length ?? 0} familii · {inventoryFontCount(inventory)} fișiere</h3>
        <small>Scanare din `sursa/static/fonturi/` și fonturile temei active.</small>
      </div>
    </div>

    <div class="google-download">
      <label class="google-family-field">
        <span>Google Fonts</span>
        <div class="google-search-box">
          <IconSearch size={14} stroke={1.9} />
          <input
            bind:value={googleFamily}
            placeholder="Caută font Google"
            disabled={googleBusy}
            onfocus={() => { googleDropdownOpen = true; }}
            onkeydown={(event) => {
              if (event.key === "Escape") googleDropdownOpen = false;
            }}
          />
        </div>
        {#if googleDropdownOpen}
          <div class="google-font-dropdown" onscroll={onGoogleDropdownScroll}>
            {#if googleSearchBusy}
              <p>Caut fonturi...</p>
            {:else if googleSearchError}
              <p>{googleSearchError}</p>
            {:else}
              {#each googleResults as font}
                <button type="button" onclick={() => selectGoogleFamily(font)}>
                  <strong style={fontFamilyStyle(font.family)}>{font.family}</strong>
                  <small>{googleFontSummary(font)}</small>
                </button>
              {:else}
                <p>Scrie cel puțin o parte din numele fontului.</p>
              {/each}
              {#if googleLoadingMore}
                <p class="google-font-list-state">Încarc mai multe fonturi...</p>
              {:else if googleSearchHasMore}
                <p class="google-font-list-state">Derulează pentru mai multe fonturi.</p>
              {/if}
            {/if}
          </div>
        {/if}
      </label>
      <label>
        <span>{googleVariable ? "Range wght" : "Weights"}</span>
        <input bind:value={googleWeights} placeholder={googleVariable ? "100..900" : "400,500,700"} disabled={googleBusy} />
      </label>
      <label class="google-variable-toggle">
        <input type="checkbox" bind:checked={googleVariable} disabled={googleBusy} />
        <span>Variabil WOFF2</span>
      </label>
      <button type="button" disabled={!googleFamily.trim() || googleBusy} onclick={downloadGoogleFont}>
        <IconDownload size={14} stroke={1.9} />
        {googleBusy ? "Descarc..." : "Descarcă"}
      </button>
    </div>

    {#if error}
      <p class="empty-line">{error}</p>
    {:else}
      <div class="font-roots">
        {#each (inventory?.roots ?? []) as root}
          <div class:missing={!root.exists}>
            <span>{fontRootLabel(root.origin, root.themeName)}</span>
            <strong>{root.relativePath}</strong>
            <small>{root.exists ? "activ" : "dosar inexistent încă"}</small>
          </div>
        {/each}
      </div>

      <div class="font-family-list">
        {#each (inventory?.families ?? []) as family}
          {@const usageCount = roleFamilyCount(family)}
          <div class="font-family-card">
            <div>
              <span>{fontRootLabel(family.origin, family.themeName)} · {family.files.length} fișiere</span>
              <strong>{family.family}</strong>
              <small>{family.directory}</small>
            </div>
            <div class="font-file-chips">
              {#each family.files.slice(0, 8) as file}
                <button type="button" onclick={() => openSource(file.file)} title={file.file}>
                  {fileWeightLabel(file)}
                </button>
              {/each}
            </div>
            <div class="font-family-actions">
              <span class:used={usageCount > 0}>{usageCount > 0 ? `${usageCount} roluri` : "nefolosit în roluri"}</span>
              <button type="button" onclick={() => onGenerateFontFace(family)}>
                @font-face
              </button>
              <button type="button" disabled>
                <IconTrash size={14} stroke={1.9} />
                Șterge
              </button>
            </div>
          </div>
        {:else}
          <p class="empty-line">Nu există fonturi locale detectate încă.</p>
        {/each}
      </div>
    {/if}
  </article>

  <article class="font-panel font-next-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Următorul strat</p>
        <h3>Operații controlate</h3>
      </div>
    </div>
    <div class="font-action-grid">
      <button type="button" onclick={() => onGenerateFontPreloads()}><IconDownload size={15} stroke={1.9} />Pregătește preload-uri active</button>
      <button type="button" disabled>Leagă familie la rol</button>
      <button type="button" disabled>Generează @font-face</button>
      <button type="button" disabled><IconTrash size={15} stroke={1.9} />Ștergere sigură</button>
    </div>
  </article>
</section>

<style>
  .font-manager,
  .font-panel,
  .font-role-grid,
  .font-library-panel,
  .font-family-list,
  .google-download,
  .font-action-grid {
    display: grid;
    gap: 12px;
    min-width: 0;
  }

  .font-manager {
    grid-column: 1 / -1;
    grid-template-columns: minmax(340px, 0.95fr) minmax(420px, 1.25fr);
    align-items: start;
  }

  .font-panel {
    align-content: start;
    padding: 14px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface-2);
  }

  .font-next-panel {
    grid-column: 1 / -1;
  }

  .panel-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .panel-heading h3 {
    margin: 0;
    color: var(--text-strong);
    font-size: 15px;
    line-height: 1.2;
  }

  .panel-heading small,
  .empty-line,
  .font-role-grid small,
  .font-roots small,
  .font-family-card small {
    overflow: hidden;
    color: var(--text-muted);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 11px;
    font-weight: 700;
  }

  .eyebrow,
  .font-role-grid span,
  .font-roots span,
  .font-family-card span {
    margin: 0;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .font-role-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .font-role-grid div,
  .font-roots div,
  .font-family-card {
    display: grid;
    gap: 6px;
    min-width: 0;
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .font-role-grid div.missing,
  .font-roots div.missing {
    border-style: dashed;
  }

  .font-role-grid strong,
  .font-roots strong,
  .font-family-card strong {
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 13px;
    font-weight: 900;
  }

  .font-roots {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px;
  }

  .google-download {
    grid-template-columns: minmax(240px, 1fr) minmax(110px, 150px) auto auto;
    align-items: end;
    padding: 10px;
    border: 1px solid var(--border-3);
    border-radius: 8px;
    background: var(--surface-4);
  }

  .google-download label {
    position: relative;
    display: grid;
    gap: 5px;
    min-width: 0;
  }

  .google-download span {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 900;
    text-transform: uppercase;
  }

  .google-search-box {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 6px;
    min-width: 0;
    height: 32px;
    padding: 0 9px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text-muted);
    background: var(--surface);
  }

  .google-download input:not([type="checkbox"]) {
    min-width: 0;
    width: 100%;
    height: 32px;
    padding: 0;
    border: 0;
    border-radius: 0;
    color: var(--text-strong);
    background: transparent;
    font: inherit;
    font-size: 12px;
    font-weight: 800;
  }

  .google-download input:not([type="checkbox"]):focus {
    outline: 0;
  }

  .google-variable-toggle {
    align-content: center;
    grid-template-columns: auto auto;
    align-items: center;
    min-height: 32px;
    padding: 0 9px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    background: var(--surface);
  }

  .google-variable-toggle input {
    width: 14px;
    height: 14px;
    margin: 0;
    accent-color: var(--brand);
  }

  .google-font-dropdown {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 8;
    display: grid;
    gap: 4px;
    width: min(520px, 78vw);
    max-height: 340px;
    padding: 6px;
    overflow: auto;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .google-font-dropdown button {
    display: grid;
    gap: 3px;
    width: 100%;
    min-width: 0;
    padding: 9px 10px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text);
    text-align: left;
    background: transparent;
  }

  .google-font-dropdown button:hover {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  .google-font-dropdown strong {
    overflow: hidden;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 20px;
    font-weight: 500;
    line-height: 1.2;
  }

  .google-font-dropdown small,
  .google-font-dropdown p {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 800;
  }

  .google-font-dropdown .google-font-list-state {
    padding: 8px 10px;
    text-align: center;
  }

  .font-family-card {
    grid-template-columns: minmax(180px, 1fr) minmax(160px, 0.8fr) auto;
    align-items: center;
  }

  .font-file-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    min-width: 0;
  }

  .font-family-actions {
    display: grid;
    gap: 6px;
    justify-items: end;
  }

  .font-family-actions span {
    display: inline-flex;
    align-items: center;
    min-height: 22px;
    padding: 3px 8px;
    border: 1px solid var(--border-3);
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface);
    font-size: 10px;
    font-weight: 900;
  }

  .font-family-actions span.used {
    border-color: color-mix(in srgb, var(--brand) 42%, var(--border-3));
    color: var(--brand-strong);
    background: var(--brand-soft);
  }

  .status-badge,
  .google-download button,
  .font-role-grid button,
  .font-family-actions button,
  .font-file-chips button,
  .font-action-grid button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 30px;
    padding: 6px 9px;
    border: 1px solid var(--border-3);
    border-radius: 7px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    font-size: 11px;
    font-weight: 900;
  }

  .status-badge {
    color: var(--text-muted);
    border-radius: 999px;
    background: var(--surface-4);
  }

  .font-role-grid button,
  .font-family-actions button {
    justify-self: start;
  }

  .font-role-grid em {
    color: var(--brand-strong);
    font-size: 11px;
    font-style: normal;
    font-weight: 800;
  }

  .font-file-chips button {
    min-height: 24px;
    padding: 3px 7px;
    color: var(--text-muted);
    border-radius: 999px;
  }

  .font-action-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  button:hover:not(:disabled) {
    border-color: var(--brand);
    background: var(--brand-soft);
  }

  @media (max-width: 1200px) {
    .font-manager,
    .font-family-card,
    .google-download,
    .font-action-grid {
      grid-template-columns: 1fr;
    }

    .font-family-actions {
      justify-items: start;
    }
  }
</style>
