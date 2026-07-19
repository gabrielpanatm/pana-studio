<script lang="ts">
  import {
    IconBraces,
    IconChevronRight,
    IconCode,
    IconFile,
    IconFiles,
    IconHome,
    IconLayoutDashboard,
    IconPalette,
    IconPlus,
    IconSearch,
    IconSitemap,
  } from "@tabler/icons-svelte";
  import type { SourceGraphPage } from "$lib/types";
  import {
    pageCountLabel,
    pageKindShortLabel,
    sitePageList,
    type SiteWorkspaceSection,
  } from "./workspace-model";

  let {
    siteTitle = "Site",
    themeName = null,
    pages = [],
    activeSection = "overview",
    activePageId = null,
    onSectionChange = () => {},
    onPageSelect = () => {},
    onCreatePage = () => {},
  }: {
    siteTitle?: string;
    themeName?: string | null;
    pages?: SourceGraphPage[];
    activeSection?: SiteWorkspaceSection;
    activePageId?: string | null;
    onSectionChange?: (section: SiteWorkspaceSection) => void;
    onPageSelect?: (page: SourceGraphPage) => void;
    onCreatePage?: () => void;
  } = $props();

  let query = $state("");
  const pageItems = $derived(sitePageList(pages));
  const visiblePages = $derived.by(() => {
    const normalized = query.trim().toLocaleLowerCase("ro");
    if (!normalized) return pageItems;
    return pageItems.filter(({ page }) => (
      page.title.toLocaleLowerCase("ro").includes(normalized)
      || page.url.toLocaleLowerCase("ro").includes(normalized)
    ));
  });
</script>

<aside class="site-sidebar" aria-label="Navigatorul site-ului">
  <header class="site-identity">
    <div class="site-mark"><IconSitemap size={19} stroke={1.9} /></div>
    <div>
      <span>Website</span>
      <strong>{siteTitle}</strong>
      <small>{themeName ? `Tema ${themeName}` : "Fără temă activă"}</small>
    </div>
  </header>

  <nav class="workspace-navigation" aria-label="Secțiuni website">
    <button class:active={activeSection === "overview"} type="button" onclick={() => onSectionChange("overview")}>
      <IconLayoutDashboard size={17} stroke={1.9} />
      <span>Prezentare</span>
    </button>
    <button class:active={activeSection === "pages"} type="button" onclick={() => onSectionChange("pages")}>
      <IconFiles size={17} stroke={1.9} />
      <span>Pagini</span>
      <em>{pages.length}</em>
    </button>
    <button class:active={activeSection === "structure"} type="button" onclick={() => onSectionChange("structure")}>
      <IconBraces size={17} stroke={1.9} />
      <span>Structura paginii</span>
    </button>
    <button class:active={activeSection === "design"} type="button" onclick={() => onSectionChange("design")}>
      <IconPalette size={17} stroke={1.9} />
      <span>Designul site-ului</span>
    </button>
    <button class:active={activeSection === "sources"} type="button" onclick={() => onSectionChange("sources")}>
      <IconCode size={17} stroke={1.9} />
      <span>Fișiere și relații</span>
    </button>
  </nav>

  <section class="pages-navigator" aria-label="Paginile site-ului">
    <div class="section-heading">
      <div>
        <strong>Pagini</strong>
        <span>{pageCountLabel(pages.length)}</span>
      </div>
      <button class="create-button" type="button" title="Creează o pagină" aria-label="Creează o pagină" onclick={onCreatePage}>
        <IconPlus size={16} stroke={2} />
      </button>
    </div>

    {#if pages.length > 5}
      <label class="page-search">
        <IconSearch size={14} stroke={1.9} />
        <input bind:value={query} aria-label="Caută o pagină" placeholder="Caută pagini…" />
      </label>
    {/if}

    <div class="page-list">
      {#each visiblePages as item}
        <button
          class:active={activePageId === item.page.id}
          class="page-row"
          style={`--page-indent: ${Math.min(item.depth, 4) * 10}px`}
          type="button"
          title={`${item.page.title} · ${item.routeLabel}`}
          onclick={() => onPageSelect(item.page)}
        >
          <span class="page-icon">
            {#if item.page.pageKind === "home"}
              <IconHome size={15} stroke={1.9} />
            {:else}
              <IconFile size={15} stroke={1.9} />
            {/if}
          </span>
          <span class="page-copy">
            <strong>{item.page.title}</strong>
            <small>{item.routeLabel}</small>
          </span>
          <span class="page-kind">{pageKindShortLabel(item.page.pageKind)}</span>
          <span class="page-arrow"><IconChevronRight size={14} stroke={1.9} /></span>
        </button>
      {:else}
        <div class="page-empty">
          <IconFiles size={22} stroke={1.5} />
          <strong>{query ? "Nicio pagină găsită" : "Site fără pagini"}</strong>
          <span>{query ? "Încearcă alt titlu sau altă rută." : "Creează prima pagină pentru a începe."}</span>
        </div>
      {/each}
    </div>
  </section>
</aside>

<style>
  .site-sidebar {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--border-2);
    background: color-mix(in srgb, var(--surface-2) 76%, var(--surface));
  }

  .site-identity {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 11px;
    align-items: center;
    min-height: 78px;
    padding: 14px 15px;
    border-bottom: 1px solid var(--border-2);
  }

  .site-mark {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    border-radius: 11px;
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 12%, var(--surface));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--brand) 22%, transparent);
  }

  .site-identity div:last-child {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .site-identity span,
  .site-identity small,
  .section-heading span,
  .page-row small,
  .page-empty span {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 700;
  }

  .site-identity > div > span {
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  .site-identity strong {
    overflow: hidden;
    color: var(--text-strong);
    font-size: 14px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .workspace-navigation {
    display: grid;
    gap: 3px;
    padding: 10px;
    border-bottom: 1px solid var(--border-2);
  }

  .workspace-navigation button,
  .page-row,
  .create-button {
    border: 0;
    color: var(--text);
    background: transparent;
    cursor: pointer;
  }

  .workspace-navigation button {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    gap: 7px;
    align-items: center;
    min-height: 34px;
    padding: 0 9px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 800;
    text-align: left;
  }

  .workspace-navigation button:hover,
  .workspace-navigation button:focus-visible,
  .workspace-navigation button.active {
    color: var(--text-strong);
    background: var(--surface);
    outline: none;
  }

  .workspace-navigation button.active {
    color: var(--brand);
    box-shadow: inset 3px 0 0 var(--brand), 0 1px 2px color-mix(in srgb, var(--text-strong) 5%, transparent);
  }

  .workspace-navigation em {
    min-width: 20px;
    padding: 2px 6px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--surface-3);
    font-size: 10px;
    font-style: normal;
    text-align: center;
  }

  .pages-navigator {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;
    padding: 12px 10px 10px;
  }

  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 9px;
  }

  .section-heading > div {
    display: flex;
    align-items: baseline;
    gap: 7px;
  }

  .section-heading strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .create-button {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    border-radius: 7px;
  }

  .create-button:hover,
  .create-button:focus-visible {
    color: var(--brand);
    background: var(--surface-3);
    outline: none;
  }

  .page-search {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 7px;
    align-items: center;
    min-height: 32px;
    margin-bottom: 8px;
    padding: 0 9px;
    border: 1px solid var(--border-2);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
  }

  .page-search:focus-within {
    border-color: color-mix(in srgb, var(--brand) 54%, var(--border));
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--brand) 12%, transparent);
  }

  .page-search input {
    min-width: 0;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
    font: inherit;
    font-size: 11px;
  }

  .page-list {
    display: grid;
    align-content: start;
    gap: 3px;
    min-height: 0;
    overflow: auto;
  }

  .page-row {
    display: grid;
    grid-template-columns: 25px minmax(0, 1fr) auto 14px;
    gap: 7px;
    align-items: center;
    min-height: 43px;
    padding: 5px 7px 5px calc(7px + var(--page-indent));
    border-radius: 9px;
    text-align: left;
  }

  .page-row:hover,
  .page-row:focus-visible {
    background: color-mix(in srgb, var(--surface) 72%, var(--brand) 4%);
    outline: none;
  }

  .page-row.active {
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 10%, var(--surface));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--brand) 22%, transparent);
  }

  .page-icon {
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    border-radius: 7px;
    color: var(--text-muted);
    background: var(--surface-3);
  }

  .active .page-icon {
    color: var(--brand);
    background: color-mix(in srgb, var(--brand) 14%, var(--surface));
  }

  .page-copy {
    display: grid;
    gap: 1px;
    min-width: 0;
  }

  .page-copy strong,
  .page-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page-copy strong {
    color: var(--text-strong);
    font-size: 12px;
  }

  .page-kind {
    color: var(--text-muted);
    font-size: 9px;
    font-weight: 850;
    text-transform: uppercase;
  }

  .page-arrow {
    opacity: 0;
    color: var(--text-muted);
  }

  .page-row:hover .page-arrow,
  .page-row.active .page-arrow {
    opacity: 1;
  }

  .page-empty {
    display: grid;
    justify-items: center;
    gap: 4px;
    margin: 8px 3px;
    padding: 20px 12px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    color: var(--text-muted);
    text-align: center;
  }

  .page-empty strong {
    color: var(--text-strong);
    font-size: 12px;
  }
</style>
