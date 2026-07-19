<script lang="ts">
  import {
    IconArrowRight,
    IconBraces,
    IconCode,
    IconEdit,
    IconFileText,
    IconHome,
    IconLayoutGrid,
    IconPalette,
    IconStack2,
  } from "@tabler/icons-svelte";
  import type { ReusablePart } from "$lib/source-graph/architecture";
  import { reusablePartRoleLabel } from "$lib/source-graph/architecture";
  import type { SourceGraphDiagnostic, SourceGraphPage } from "$lib/types";
  import SitePreview from "./SitePreview.svelte";
  import { pageCountLabel, pageTemplateLabel, type SiteWorkspaceSection } from "./workspace-model";

  let {
    page = null,
    pages = [],
    templateCount = 0,
    styleCount = 0,
    reusableParts = [],
    diagnostics = [],
    previewUrl = "",
    previewMarkup = null,
    onOpenPageSource = () => {},
    onOpenExternal = () => {},
    onNavigate = () => {},
  }: {
    page?: SourceGraphPage | null;
    pages?: SourceGraphPage[];
    templateCount?: number;
    styleCount?: number;
    reusableParts?: ReusablePart[];
    diagnostics?: SourceGraphDiagnostic[];
    previewUrl?: string;
    previewMarkup?: string | null;
    onOpenPageSource?: (page: SourceGraphPage) => void | Promise<void>;
    onOpenExternal?: () => void | Promise<void>;
    onNavigate?: (section: SiteWorkspaceSection) => void;
  } = $props();
</script>

<div class="overview-stage">
  <header class="stage-heading">
    <div>
      <span class="section-kicker">Site-ul tău</span>
      <h1>{page?.title ?? "Website fără pagină principală"}</h1>
      <p>
        {#if page}
          Pagina deschisă acum este <strong>{page.url}</strong>. Selectează altă pagină din navigator sau continuă editarea acesteia.
        {:else}
          Creează o pagină pentru a vedea website-ul și structura lui aici.
        {/if}
      </p>
    </div>
    {#if page}
      <div class="heading-actions">
        <button class="primary" type="button" onclick={() => { void onOpenPageSource(page); }}>
          <IconEdit size={16} stroke={1.9} />
          Editează conținutul
        </button>
        <button type="button" onclick={() => onNavigate("structure")}>
          <IconBraces size={16} stroke={1.9} />
          Vezi structura
        </button>
      </div>
    {/if}
  </header>

  <div class="overview-grid">
    <div class="preview-column">
      <SitePreview
        title={page?.title ?? "Preview site"}
        route={page?.url ?? "/"}
        src={previewUrl}
        srcdoc={previewMarkup}
        onOpenExternal={onOpenExternal}
      />

      <section class="site-summary" aria-label="Rezumatul website-ului">
        <button type="button" onclick={() => onNavigate("pages")}>
          <span class="summary-icon pages"><IconFileText size={18} stroke={1.8} /></span>
          <span><strong>{pageCountLabel(pages.length)}</strong><small>Vezi și organizează paginile</small></span>
          <IconArrowRight size={16} stroke={1.8} />
        </button>
        <button type="button" onclick={() => onNavigate("structure")}>
          <span class="summary-icon structure"><IconLayoutGrid size={18} stroke={1.8} /></span>
          <span><strong>{reusableParts.length} zone reutilizabile</strong><small>{templateCount} template-uri, header, footer și componente</small></span>
          <IconArrowRight size={16} stroke={1.8} />
        </button>
        <button type="button" onclick={() => onNavigate("design")}>
          <span class="summary-icon design"><IconPalette size={18} stroke={1.8} /></span>
          <span><strong>{styleCount} fișiere de stil</strong><small>Culori, fonturi și identitate vizuală</small></span>
          <IconArrowRight size={16} stroke={1.8} />
        </button>
      </section>
    </div>

    <aside class="overview-context">
      {#if page}
        <section class="context-card current-page">
          <div class="card-title">
            <span class="card-icon"><IconHome size={18} stroke={1.8} /></span>
            <div><small>Pagina deschisă</small><h2>{page.title}</h2></div>
          </div>
          <dl>
            <div><dt>Adresă</dt><dd>{page.url}</dd></div>
            <div><dt>Aspect</dt><dd>{pageTemplateLabel(page)}</dd></div>
          </dl>
          <button type="button" onclick={() => onNavigate("pages")}>Detalii și opțiuni pagină</button>
        </section>
      {/if}

      <section class="context-card">
        <div class="card-title">
          <span class="card-icon"><IconStack2 size={18} stroke={1.8} /></span>
          <div><small>Componente comune</small><h2>Folosite în site</h2></div>
        </div>
        <div class="compact-list">
          {#each reusableParts.slice(0, 5) as part}
            <button type="button" onclick={() => onNavigate("structure")}>
              <span>{reusablePartRoleLabel(part.role)}</span>
              <strong>{part.usedBy ? `${part.usedBy} utilizări` : "Disponibilă"}</strong>
            </button>
          {:else}
            <p>Nu există încă zone reutilizabile detectate.</p>
          {/each}
        </div>
      </section>

      {#if diagnostics.length}
        <section class="context-card attention">
          <div class="card-title">
            <span class="card-icon"><IconCode size={18} stroke={1.8} /></span>
            <div><small>Necesită atenție</small><h2>{diagnostics.length} mesaje</h2></div>
          </div>
          <p>Există informații tehnice care merită verificate în Fișiere și relații.</p>
          <button type="button" onclick={() => onNavigate("sources")}>Vezi detaliile</button>
        </section>
      {/if}
    </aside>
  </div>
</div>

<style>
  .overview-stage {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 16px;
    min-width: 0;
    min-height: 0;
    padding: 20px;
  }

  .stage-heading {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
  }

  .stage-heading > div:first-child {
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .section-kicker,
  .card-title small {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  p {
    margin: 0;
  }

  h1 {
    color: var(--text-strong);
    font-size: clamp(24px, 2.2vw, 34px);
    line-height: 1.08;
  }

  .stage-heading p {
    color: var(--text-muted);
    font-size: 12px;
  }

  .stage-heading p strong {
    color: var(--text);
    font-family: var(--font-mono, monospace);
  }

  .heading-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }

  button {
    border: 1px solid var(--border);
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    font-size: 11px;
    font-weight: 850;
    cursor: pointer;
  }

  .heading-actions button {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 34px;
    padding: 0 12px;
    border-radius: 8px;
  }

  button:hover,
  button:focus-visible {
    border-color: color-mix(in srgb, var(--brand) 48%, var(--border));
    outline: none;
  }

  button:focus-visible {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 20%, transparent);
  }

  button.primary {
    border-color: var(--brand);
    color: white;
    background: var(--brand);
  }

  .overview-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 320px);
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  .preview-column {
    display: grid;
    grid-template-rows: minmax(420px, 1fr) auto;
    gap: 14px;
    min-width: 0;
    min-height: 0;
  }

  .site-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .site-summary button {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 9px;
    align-items: center;
    min-width: 0;
    min-height: 58px;
    padding: 8px 10px;
    border-radius: 10px;
    text-align: left;
  }

  .site-summary button > span:nth-child(2) {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .site-summary strong,
  .site-summary small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .site-summary small {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 650;
  }

  .summary-icon,
  .card-icon {
    display: grid;
    place-items: center;
    border-radius: 9px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 10%, var(--surface));
  }

  .summary-icon {
    width: 34px;
    height: 34px;
  }

  .summary-icon.structure { color: #7c5ce7; background: color-mix(in srgb, #7c5ce7 11%, var(--surface)); }
  .summary-icon.design { color: #d46a24; background: color-mix(in srgb, #d46a24 11%, var(--surface)); }

  .overview-context {
    display: grid;
    align-content: start;
    gap: 10px;
    min-height: 0;
    overflow: auto;
  }

  .context-card {
    display: grid;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--border-2);
    border-radius: 12px;
    background: var(--surface);
  }

  .context-card.current-page {
    border-color: color-mix(in srgb, var(--brand) 28%, var(--border));
    background: linear-gradient(145deg, color-mix(in srgb, var(--brand) 7%, var(--surface)), var(--surface));
  }

  .card-title {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 9px;
    align-items: center;
  }

  .card-icon {
    width: 34px;
    height: 34px;
  }

  .card-title > div {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .card-title h2 {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 14px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  dl {
    display: grid;
    gap: 0;
    margin: 0;
  }

  dl div {
    display: grid;
    grid-template-columns: 68px minmax(0, 1fr);
    gap: 10px;
    padding: 7px 0;
    border-top: 1px solid var(--border-2);
  }

  dt {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 800;
  }

  dd {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    color: var(--text-strong);
    font-size: 11px;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .context-card > button {
    min-height: 32px;
    border-radius: 8px;
  }

  .compact-list {
    display: grid;
    gap: 4px;
  }

  .compact-list button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 32px;
    padding: 0 9px;
    border-color: var(--border-2);
    border-radius: 8px;
    text-align: left;
  }

  .compact-list button span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compact-list button strong {
    color: var(--text-muted);
    font-size: 9px;
    white-space: nowrap;
  }

  .compact-list p,
  .context-card > p {
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.5;
  }

  .attention {
    border-color: color-mix(in srgb, var(--warning, #d99a24) 30%, var(--border));
  }

  @media (max-width: 1180px) {
    .overview-grid { grid-template-columns: minmax(0, 1fr); }
    .overview-context { grid-template-columns: repeat(2, minmax(0, 1fr)); overflow: visible; }
    .site-summary { grid-template-columns: 1fr; }
  }
</style>
