<script lang="ts">
  import {
    IconAlertTriangle,
    IconArrowRight,
    IconBox,
    IconBrush,
    IconCircleCheck,
    IconCode,
    IconExternalLink,
    IconFileText,
    IconGitBranch,
    IconPhoto,
    IconRocket,
    IconShieldCheck,
    IconSitemap,
  } from "@tabler/icons-svelte";
  import {
    pageKindLabel,
    pageTemplateLabel,
    siteOverviewPages,
    sourceDisplayPath,
  } from "$lib/site/overview";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SourceGraphPage, WorkbenchActivity } from "$lib/types";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  const graph = $derived(app.sourceGraph);
  const pages = $derived(siteOverviewPages(graph?.pages ?? []));
  const partialCount = $derived(graph?.templates.filter((template) => template.isPartial).length ?? 0);
  const diagnostics = $derived(graph?.diagnostics ?? []);
  const errorCount = $derived(diagnostics.filter((diagnostic) => diagnostic.severity === "error").length);
  const warningCount = $derived(diagnostics.filter((diagnostic) => diagnostic.severity === "warning").length);
  const activePage = $derived(
    graph?.pages.find((page) => page.file === app.activeScannedPath)
      ?? graph?.pages.find((page) => page.pageKind === "home")
      ?? graph?.pages[0]
      ?? null,
  );

  const destinations: {
    activity: WorkbenchActivity;
    title: string;
    description: string;
    icon: "content" | "components" | "design" | "assets" | "versioning" | "audit" | "publish";
  }[] = [
    { activity: "content", title: "Conținut", description: "Pagini, colecții, frontmatter și taxonomii", icon: "content" },
    { activity: "components", title: "Componente", description: "Parțiale, macro-uri și componente interactive", icon: "components" },
    { activity: "design_system", title: "Sistem de design", description: "Tokeni, clase, stiluri, fonturi și planșă vizuală", icon: "design" },
    { activity: "assets", title: "Resurse", description: "Imagini, fonturi și fișiere statice cu utilizările lor", icon: "assets" },
    { activity: "versioning", title: "Control versiuni", description: "Modificări, commit-uri, ramuri și sincronizare Git", icon: "versioning" },
    { activity: "audit", title: "Probleme și audit", description: "Erori, avertismente și verificări de calitate", icon: "audit" },
    { activity: "publish", title: "Publicare", description: "Verificare, configurare, construire și livrare", icon: "publish" },
  ];

  async function navigate(activity: WorkbenchActivity) {
    await app.setWorkbenchActivity(activity);
  }

  async function openPage(page: SourceGraphPage) {
    await openWorkspaceSource(page.file);
    await app.setWorkbenchActivity("editor");
  }
</script>

<section class="site-overview" aria-labelledby="site-overview-title">
  <header class="overview-header">
    <div class="title-block">
      <span class="title-icon"><IconSitemap size={20} stroke={1.8} /></span>
      <div>
        <span class="eyebrow">Privire de ansamblu</span>
        <h1 id="site-overview-title">Site</h1>
        <p>Harta proiectului și punctul unic de intrare către activitățile specializate.</p>
      </div>
    </div>
    <div class="header-actions">
      <button type="button" class="ui-button secondary-button" disabled={!activePage} onclick={() => activePage && openPage(activePage)}>
        <IconCode size={16} /> Deschide în editor
      </button>
      <button type="button" class="ui-button primary primary-button" onclick={() => app.openCurrentProjectInBrowser(activePage?.url ?? "/")}>
        <IconExternalLink size={16} /> Deschide site-ul
      </button>
    </div>
  </header>

  <div class="overview-scroll">
    <section class="summary-grid" aria-label="Rezumat proiect">
      <article class="ui-card"><span>Pagini</span><strong>{pages.length}</strong><small>{activePage?.url ?? "Nicio rută activă"}</small></article>
      <article class="ui-card"><span>Componente</span><strong>{partialCount}</strong><small>Parțiale Tera detectate</small></article>
      <article class="ui-card"><span>Stiluri</span><strong>{graph?.styles.length ?? 0}</strong><small>{graph?.activeTheme ? `Tema ${graph.activeTheme}` : "Stiluri locale"}</small></article>
      <article class="ui-card" class:attention={errorCount > 0}>
        <span>Calitate</span><strong>{errorCount + warningCount}</strong><small>{errorCount} erori · {warningCount} avertismente</small>
      </article>
    </section>

    <div class="overview-grid">
      <section class="ui-panel pages-panel" aria-labelledby="site-pages-title">
        <header class="section-header">
          <div><span>Documente canonice</span><h2 id="site-pages-title">Pagini detectate</h2></div>
          <button type="button" class="text-button" onclick={() => navigate("content")}>Gestionează conținutul <IconArrowRight size={15} /></button>
        </header>
        {#if pages.length > 0}
          <div class="page-list">
            {#each pages as item (item.page.id)}
              <button type="button" class:active={item.page.id === activePage?.id} onclick={() => openPage(item.page)}>
                <span class="ui-badge page-kind">{pageKindLabel(item.page.pageKind)}</span>
                <span class="page-copy" style={`--route-depth: ${item.depth};`}>
                  <strong>{item.page.title}</strong>
                  <small>{item.routeLabel} · {pageTemplateLabel(item.page)}</small>
                </span>
                <span class="page-source">{sourceDisplayPath(item.page.file)}</span>
                <IconArrowRight size={16} aria-hidden="true" />
              </button>
            {/each}
          </div>
        {:else}
          <div class="empty-state"><IconFileText size={22} /><strong>Nu au fost detectate pagini</strong><span>Deschide Conținut pentru a crea prima pagină în sesiunea proiectului.</span></div>
        {/if}
      </section>

      <aside class="ui-panel orchestrator-panel" aria-labelledby="site-destinations-title">
        <header class="section-header"><div><span>Un singur flux</span><h2 id="site-destinations-title">Continuă în activitatea potrivită</h2></div></header>
        <div class="destination-list">
          {#each destinations as destination (destination.activity)}
            <button type="button" onclick={() => navigate(destination.activity)}>
              <span class="destination-icon">
                {#if destination.icon === "content"}<IconFileText size={18} />
                {:else if destination.icon === "components"}<IconBox size={18} />
                {:else if destination.icon === "design"}<IconBrush size={18} />
                {:else if destination.icon === "assets"}<IconPhoto size={18} />
                {:else if destination.icon === "versioning"}<IconGitBranch size={18} />
                {:else if destination.icon === "audit"}<IconShieldCheck size={18} />
                {:else}<IconRocket size={18} />{/if}
              </span>
              <span><strong>{destination.title}</strong><small>{destination.description}</small></span>
              <IconArrowRight size={16} aria-hidden="true" />
            </button>
          {/each}
        </div>
      </aside>
    </div>

    <section class="ui-panel health-panel" aria-labelledby="site-health-title">
      <header class="section-header">
        <div><span>Hartă comună a surselor</span><h2 id="site-health-title">Starea structurii</h2></div>
        <button type="button" class="text-button" onclick={() => navigate("audit")}>Deschide auditul complet <IconArrowRight size={15} /></button>
      </header>
      {#if diagnostics.length === 0}
        <div class="health-ok"><IconCircleCheck size={19} /><span><strong>Structură coerentă</strong><small>Harta surselor nu raportează probleme pentru revizia curentă.</small></span></div>
      {:else}
        <div class="diagnostic-list">
          {#each diagnostics.slice(0, 5) as diagnostic, index (`${diagnostic.file ?? "project"}:${index}`)}
            <button type="button" onclick={() => diagnostic.file ? openWorkspaceSource(diagnostic.file) : navigate("audit")}>
              <IconAlertTriangle size={17} />
              <span><strong>{diagnostic.message}</strong><small>{diagnostic.file ? sourceDisplayPath(diagnostic.file) : "Proiect"}</small></span>
              <IconArrowRight size={16} aria-hidden="true" />
            </button>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</section>

<style>
  .site-overview { display: grid; grid-template-rows: auto minmax(0, 1fr); width: 100%; height: 100%; min-width: 0; min-height: 0; overflow: hidden; border-radius: 10px; color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .overview-header, .title-block, .header-actions, .section-header, .text-button, .health-ok { display: flex; align-items: center; }
  .overview-header { justify-content: space-between; gap: 24px; padding: 20px 22px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .title-block { gap: 12px; min-width: 0; }
  .title-icon, .destination-icon { display: grid; place-items: center; flex: 0 0 auto; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .title-icon { width: 40px; height: 40px; border-radius: 10px; }
  .title-block > div { min-width: 0; }
  .eyebrow, .section-header span, .summary-grid span { color: var(--wb-text-muted); font-size: 12px; font-weight: 800; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 2px 0 0; color: var(--text-strong); font-size: 24px; line-height: 1.15; }
  .title-block p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .header-actions { gap: 8px; }
  button { font: inherit; }
  .primary-button, .secondary-button, .text-button { min-height: 32px; border-radius: var(--wb-radius-control); font-size: 12px; font-weight: 750; }
  .primary-button, .secondary-button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; padding: 0 12px; border: 1px solid var(--wb-border-subtle); }
  .primary-button { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .secondary-button { color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .overview-scroll { min-height: 0; padding: 14px; overflow: auto; }
  .summary-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin-bottom: 10px; }
  .summary-grid article { display: grid; gap: 4px; min-width: 0; padding: 13px 14px; border-radius: 8px; background: var(--wb-surface-chrome); }
  .summary-grid strong { color: var(--text-strong); font-size: 22px; }
  .summary-grid small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .summary-grid article.attention strong { color: var(--wb-warning); }
  .overview-grid { display: grid; grid-template-columns: minmax(440px, 1fr) minmax(320px, .72fr); gap: 10px; }
  .pages-panel, .orchestrator-panel, .health-panel { min-width: 0; overflow: hidden; border-radius: 8px; background: var(--wb-surface-chrome); }
  .section-header { justify-content: space-between; gap: 12px; min-height: 58px; padding: 10px 13px; }
  .section-header > div { display: grid; gap: 3px; }
  h2 { margin: 0; color: var(--text-strong); font-size: 15px; }
  .text-button { justify-content: center; gap: 5px; padding: 0 6px; border: 0; color: var(--wb-accent-strong); background: transparent; }
  .page-list, .destination-list, .diagnostic-list { display: grid; gap: 1px; background: var(--wb-border-subtle); }
  .page-list > button { display: grid; grid-template-columns: 72px minmax(0, 1fr) minmax(120px, .65fr) 18px; align-items: center; gap: 9px; min-height: 54px; padding: 7px 12px; border: 0; color: var(--wb-text-primary); text-align: left; background: var(--wb-surface-document); }
  .page-list > button:hover, .page-list > button.active { background: var(--wb-control-hover); }
  .page-kind { color: var(--wb-text-muted); font-size: 12px; font-weight: 750; }
  .page-copy { display: grid; gap: 3px; min-width: 0; padding-left: calc(var(--route-depth) * 12px); }
  .page-copy strong, .destination-list strong, .diagnostic-list strong, .health-ok strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .page-copy small, .destination-list small, .diagnostic-list small, .health-ok small { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; text-overflow: ellipsis; white-space: nowrap; }
  .page-source { overflow: hidden; color: var(--wb-text-muted); font-family: var(--font-mono, monospace); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .destination-list > button, .diagnostic-list > button { display: grid; align-items: center; gap: 10px; min-height: 58px; padding: 8px 12px; border: 0; color: var(--wb-text-primary); text-align: left; background: var(--wb-surface-document); }
  .destination-list > button { grid-template-columns: 34px minmax(0, 1fr) 18px; }
  .destination-list > button:hover, .diagnostic-list > button:hover { background: var(--wb-control-hover); }
  .destination-icon { width: 34px; height: 34px; border-radius: 8px; }
  .destination-list > button > span:nth-child(2), .diagnostic-list > button > span { display: grid; gap: 3px; min-width: 0; }
  .health-panel { margin-top: 10px; }
  .health-ok { gap: 10px; min-height: 58px; padding: 10px 13px; color: var(--success); background: var(--wb-surface-document); }
  .health-ok > span { display: grid; gap: 3px; min-width: 0; }
  .diagnostic-list > button { grid-template-columns: 20px minmax(0, 1fr) 18px; color: var(--wb-warning); }
  .empty-state { display: grid; justify-items: center; gap: 6px; padding: 30px 16px; color: var(--wb-text-muted); text-align: center; }
  .empty-state strong { color: var(--text-strong); font-size: 13px; }
  .empty-state span { font-size: 12px; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .5; }
  button:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: -2px; }
  @media (max-width: 1000px) { .overview-grid { grid-template-columns: 1fr; } .summary-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .header-actions { align-items: stretch; flex-direction: column; } }
</style>
