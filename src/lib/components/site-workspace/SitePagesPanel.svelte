<script lang="ts">
  import {
    IconBraces,
    IconCode,
    IconEdit,
    IconExternalLink,
    IconFileDescription,
    IconLayout,
    IconLink,
    IconPlus,
    IconRoute,
    IconStack2,
  } from "@tabler/icons-svelte";
  import type { SourceGraphPage, SourceGraphStyle, SourceGraphTemplate } from "$lib/types";
  import SitePreview from "./SitePreview.svelte";
  import {
    pageKindLabel,
    pageTemplateLabel,
    sourceDisplayPath,
    styleScopeLabel,
  } from "./workspace-model";

  let {
    page = null,
    templateChain = [],
    styles = [],
    previewUrl = "",
    previewMarkup = null,
    onCreatePage = () => {},
    onOpenContent = () => {},
    onOpenSource = () => {},
    onOpenStructure = () => {},
    onOpenExternal = () => {},
  }: {
    page?: SourceGraphPage | null;
    templateChain?: SourceGraphTemplate[];
    styles?: SourceGraphStyle[];
    previewUrl?: string;
    previewMarkup?: string | null;
    onCreatePage?: () => void;
    onOpenContent?: (page: SourceGraphPage) => void | Promise<void>;
    onOpenSource?: (path: string) => void | Promise<void>;
    onOpenStructure?: () => void;
    onOpenExternal?: () => void | Promise<void>;
  } = $props();
</script>

<div class="pages-stage">
  <header class="stage-heading">
    <div>
      <span class="section-kicker">Pagini</span>
      <h1>{page?.title ?? "Alege o pagină"}</h1>
      <p>
        {#if page}
          Vezi pagina așa cum apare în site și schimbă-i conținutul, structura sau fișierele asociate.
        {:else}
          Selectează o pagină din navigatorul din stânga sau creează prima pagină.
        {/if}
      </p>
    </div>
    <button class="create-page" type="button" onclick={onCreatePage}>
      <IconPlus size={16} stroke={2} />
      Pagină nouă
    </button>
  </header>

  {#if page}
    <div class="page-workspace">
      <section class="page-canvas">
        <SitePreview
          title={page.title}
          route={page.url}
          src={previewUrl}
          srcdoc={previewMarkup}
          onOpenExternal={onOpenExternal}
        />
      </section>

      <aside class="page-inspector">
        <section class="inspector-card page-identity">
          <div class="card-icon"><IconFileDescription size={20} stroke={1.8} /></div>
          <div>
            <span>Pagina selectată</span>
            <h2>{page.title}</h2>
            <p><IconRoute size={14} stroke={1.9} /> {page.url}</p>
          </div>
        </section>

        <div class="primary-actions">
          <button class="primary" type="button" onclick={() => { void onOpenContent(page); }}>
            <IconEdit size={16} stroke={1.9} />
            Editează conținutul
          </button>
          <button type="button" onclick={onOpenStructure}>
            <IconBraces size={16} stroke={1.9} />
            Structura paginii
          </button>
        </div>

        <section class="inspector-card essentials">
          <h3>Despre această pagină</h3>
          <dl>
            <div><dt>Rol în site</dt><dd>{pageKindLabel(page.pageKind)}</dd></div>
            <div><dt>Aspect folosit</dt><dd>{pageTemplateLabel(page)}</dd></div>
            <div><dt>Adresă publică</dt><dd>{page.url}</dd></div>
          </dl>
        </section>

        <details class="build-details">
          <summary>
            <span class="summary-icon"><IconStack2 size={17} stroke={1.8} /></span>
            <span><strong>Cum este construită pagina</strong><small>Conținut, template, layout și stiluri</small></span>
          </summary>
          <div class="build-flow">
            <button type="button" onclick={() => { void onOpenContent(page); }}>
              <span class="flow-icon content"><IconFileDescription size={16} stroke={1.8} /></span>
              <span><small>Conținutul paginii</small><strong>{sourceDisplayPath(page.file)}</strong></span>
              <IconCode size={15} stroke={1.8} />
            </button>
            {#each templateChain as template, index}
              <button type="button" onclick={() => { void onOpenSource(template.file); }}>
                <span class="flow-icon template"><IconLayout size={16} stroke={1.8} /></span>
                <span><small>{index === 0 ? "Template-ul paginii" : "Layout moștenit"}</small><strong>{template.name}</strong></span>
                <IconCode size={15} stroke={1.8} />
              </button>
            {/each}
            {#each styles as style}
              <button type="button" onclick={() => { void onOpenSource(style.file); }}>
                <span class="flow-icon style"><IconLink size={16} stroke={1.8} /></span>
                <span><small>{styleScopeLabel(style.scope)}</small><strong>{sourceDisplayPath(style.file)}</strong></span>
                <IconCode size={15} stroke={1.8} />
              </button>
            {/each}
          </div>
        </details>

        <button class="open-browser" type="button" onclick={() => { void onOpenExternal(); }}>
          <IconExternalLink size={16} stroke={1.9} />
          Deschide versiunea salvată în browser
        </button>
      </aside>
    </div>
  {:else}
    <div class="empty-page">
      <IconFileDescription size={34} stroke={1.5} />
      <h2>Nu este selectată nicio pagină</h2>
      <p>Navigatorul din stânga reprezintă harta site-ului. Alege o pagină pentru a o vedea aici.</p>
      <button type="button" onclick={onCreatePage}><IconPlus size={16} /> Creează o pagină</button>
    </div>
  {/if}
</div>

<style>
  .pages-stage {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 15px;
    min-width: 0;
    min-height: 0;
    padding: 20px;
  }

  .stage-heading {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 18px;
  }

  .stage-heading > div {
    display: grid;
    gap: 4px;
  }

  .section-kicker,
  .page-identity span,
  dt {
    color: var(--brand);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3,
  p,
  dl,
  dd {
    margin: 0;
  }

  h1 {
    color: var(--text-strong);
    font-size: clamp(24px, 2.2vw, 34px);
    line-height: 1.08;
  }

  .stage-heading p,
  .empty-page p {
    max-width: 690px;
    color: var(--text-muted);
    font-size: 12px;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 34px;
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-strong);
    background: var(--surface);
    font: inherit;
    font-size: 11px;
    font-weight: 850;
    cursor: pointer;
  }

  button:hover {
    border-color: color-mix(in srgb, var(--brand) 55%, var(--border));
  }

  .create-page {
    padding: 0 12px;
  }

  .page-workspace {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 330px);
    gap: 14px;
    min-width: 0;
    min-height: 0;
  }

  .page-canvas {
    min-width: 0;
    min-height: 0;
  }

  .page-inspector {
    display: grid;
    align-content: start;
    gap: 10px;
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }

  .inspector-card,
  .build-details {
    border: 1px solid var(--border-2);
    border-radius: 11px;
    background: var(--surface-2);
  }

  .page-identity {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 11px;
    align-items: center;
    padding: 13px;
  }

  .card-icon {
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border-radius: 10px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 11%, var(--surface));
  }

  .page-identity > div:last-child {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .page-identity h2 {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page-identity p {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
  }

  .primary-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 7px;
  }

  .primary-actions button {
    min-height: 40px;
  }

  .primary-actions .primary {
    border-color: var(--brand);
    color: #fff;
    background: var(--brand);
  }

  .essentials {
    display: grid;
    gap: 9px;
    padding: 13px;
  }

  .essentials h3 {
    color: var(--text-strong);
    font-size: 13px;
  }

  .essentials dl {
    display: grid;
  }

  .essentials dl div {
    display: grid;
    grid-template-columns: 108px minmax(0, 1fr);
    gap: 8px;
    padding: 8px 0;
    border-top: 1px solid var(--border-2);
  }

  .essentials dd {
    overflow: hidden;
    color: var(--text);
    font-size: 11px;
    font-weight: 800;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .build-details {
    overflow: hidden;
  }

  .build-details summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 9px;
    align-items: center;
    padding: 12px;
    cursor: pointer;
    list-style: none;
  }

  .build-details summary::-webkit-details-marker {
    display: none;
  }

  .summary-icon,
  .flow-icon {
    display: grid;
    place-items: center;
    color: var(--brand);
  }

  .build-details summary > span:last-child {
    display: grid;
    gap: 1px;
  }

  .build-details summary strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .build-details summary small,
  .build-flow small {
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 700;
  }

  .build-flow {
    display: grid;
    gap: 5px;
    padding: 0 9px 9px;
  }

  .build-flow button {
    display: grid;
    grid-template-columns: 25px minmax(0, 1fr) auto;
    gap: 7px;
    justify-content: stretch;
    min-height: 45px;
    padding: 6px 8px;
    text-align: left;
  }

  .build-flow button > span:nth-child(2) {
    display: grid;
    gap: 1px;
    min-width: 0;
  }

  .build-flow button strong {
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .flow-icon {
    width: 25px;
    height: 25px;
    border-radius: 7px;
    background: color-mix(in srgb, var(--brand) 9%, var(--surface));
  }

  .open-browser {
    width: 100%;
    min-height: 39px;
  }

  .empty-page {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 9px;
    min-height: 360px;
    border: 1px dashed var(--border);
    border-radius: 13px;
    color: var(--text-muted);
    text-align: center;
  }

  .empty-page h2 {
    color: var(--text-strong);
    font-size: 18px;
  }

  .empty-page button {
    margin-top: 5px;
    padding: 0 12px;
  }

  @media (max-width: 1050px) {
    .page-workspace {
      grid-template-columns: minmax(0, 1fr);
    }

    .page-inspector {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      overflow: visible;
    }

    .build-details,
    .open-browser {
      grid-column: 1 / -1;
    }
  }
</style>
