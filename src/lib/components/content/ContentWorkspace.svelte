<script lang="ts">
  import {
    IconAlertTriangle,
    IconCode,
    IconEdit,
    IconExternalLink,
    IconFileText,
    IconHome,
    IconPlus,
    IconSearch,
    IconX,
  } from "@tabler/icons-svelte";
  import ProjectPageSettingsTab from "$lib/components/project/ProjectPageSettingsTab.svelte";
  import { slugifyPageTitle } from "$lib/project/files";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SourceGraphPage, SourcePageKind } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type ContentView = "all" | "pages" | "sections";
  type DetailMode = "info" | "create" | "edit";

  const contentViews: { id: ContentView; label: string }[] = [
    { id: "all", label: "Toate" },
    { id: "pages", label: "Pagini" },
    { id: "sections", label: "Secțiuni" },
  ];

  let activeView = $state<ContentView>("all");
  let detailMode = $state<DetailMode>("info");
  let query = $state("");
  let sectionFilter = $state("all");
  let selectedPageId = $state("");
  let titleDraft = $state("");
  let slugDraft = $state("");
  let sectionDraft = $state("");
  let slugTouched = $state(false);
  let creating = $state(false);
  let createError = $state("");
  let metadataSource = $state("");
  let metadataLoading = $state(false);
  let metadataError = $state("");

  const pages = $derived(app.sourceGraph?.pages ?? []);
  const sections = $derived.by(() => {
    const values = new Set<string>();
    for (const page of pages) values.add(contentSection(page.file));
    values.add("");
    return [...values].sort((left, right) => left.localeCompare(right, "ro"));
  });
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const filteredPages = $derived(
    pages
      .filter((page) => (
        (activeView === "all"
          || activeView === "sections" && page.pageKind === "section"
          || activeView === "pages" && page.pageKind !== "section")
        && (sectionFilter === "all" || contentSection(page.file) === sectionFilter)
        && (!normalizedQuery || `${page.title} ${page.url} ${page.file} ${page.resolvedTemplate ?? ""}`
          .toLocaleLowerCase("ro")
          .includes(normalizedQuery))
      ))
      .sort((left, right) => {
        if (left.pageKind === "home" && right.pageKind !== "home") return -1;
        if (right.pageKind === "home" && left.pageKind !== "home") return 1;
        return left.url.localeCompare(right.url, "ro");
      }),
  );
  const selectedPage = $derived(
    pages.find((page) => page.id === selectedPageId) ?? filteredPages[0] ?? null,
  );
  const contentDiagnostics = $derived(
    (app.projectAuditSnapshot?.diagnostics ?? []).filter((diagnostic) => diagnostic.category === "seo"),
  );
  const selectedDiagnostics = $derived(
    selectedPage
      ? (app.projectAuditSnapshot?.diagnostics ?? []).filter((diagnostic) => diagnostic.file === selectedPage.file)
      : [],
  );

  function contentSection(file: string) {
    const normalized = file.replaceAll("\\", "/").replace(/^content\/?/, "");
    const slash = normalized.lastIndexOf("/");
    return slash < 0 ? "" : normalized.slice(0, slash);
  }

  function sectionLabel(section: string) {
    return section || "Rădăcină content";
  }

  function kindLabel(kind: SourcePageKind) {
    if (kind === "home") return "Acasă";
    if (kind === "section") return "Secțiune";
    return "Pagină";
  }

  function relationCount(page: SourceGraphPage) {
    const ids = new Set([page.id, page.contentNodeId]);
    return (app.sourceGraph?.relations ?? []).filter(
      (relation) => ids.has(relation.from) || ids.has(relation.to),
    ).length;
  }

  function resetPanel() {
    detailMode = "info";
    createError = "";
    metadataError = "";
  }

  function selectView(view: ContentView) {
    activeView = view;
    resetPanel();
  }

  function selectPage(id: string) {
    selectedPageId = id;
    resetPanel();
  }

  function beginCreate(section = sectionFilter === "all" ? "" : sectionFilter) {
    sectionDraft = section;
    titleDraft = "";
    slugDraft = "";
    slugTouched = false;
    createError = "";
    detailMode = "create";
  }

  function updateTitle(value: string) {
    titleDraft = value;
    if (!slugTouched) slugDraft = slugifyPageTitle(value);
  }

  async function createPage() {
    if (creating) return;
    const title = titleDraft.trim();
    const slug = slugifyPageTitle(slugDraft || title);
    if (!title) {
      createError = "Adaugă un titlu pentru pagina nouă.";
      return;
    }
    if (!slug) {
      createError = "Slug-ul nu conține caractere care pot forma un URL.";
      return;
    }
    creating = true;
    createError = "";
    try {
      const relativePath = await app.createContentPageFromInput({
        title,
        slug,
        section: sectionDraft,
      });
      if (!relativePath) {
        createError = app.projectStatus || "Pagina nu a putut fi creată.";
        return;
      }
      selectedPageId = app.sourceGraph?.pages.find((page) => page.file === relativePath)?.id ?? "";
      detailMode = "info";
      app.setGlobalStatus(
        `Pagina ${relativePath} este pregătită în sesiunea proiectului — Ctrl+S persistă pe disc.`,
        "unsaved",
      );
    } catch (error) {
      createError = errorMessage(error);
    } finally {
      creating = false;
    }
  }

  async function openSource(page: SourceGraphPage) {
    await openWorkspaceSource(page.file);
  }

  async function beginEdit(page: SourceGraphPage) {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    detailMode = "edit";
    metadataLoading = true;
    metadataError = "";
    try {
      const source = await app.readPageSettingsDocument(page.file);
      if (
        app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || selectedPage?.id !== page.id
      ) return;
      metadataSource = source;
    } catch (error) {
      if (selectedPage?.id === page.id) metadataError = errorMessage(error);
    } finally {
      if (selectedPage?.id === page.id) metadataLoading = false;
    }
  }

  function updateMetadataSource(relativePath: string, source: string) {
    metadataSource = source;
    app.updatePageFrontmatterSource(relativePath, source);
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + contentViews.length) % contentViews.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % contentViews.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = contentViews.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = contentViews[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`content-tab-${next.id}`)?.focus());
  }
</script>

<section class="content-workspace" aria-labelledby="content-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconFileText size={15} stroke={1.9} /> Spațiu de conținut</span>
      <h1 id="content-title">Conținut</h1>
      <p>Paginile și secțiunile sunt proiectate din harta surselor; Markdown-ul Zola rămâne sursa reală.</p>
    </div>
    <dl>
      <div><dt>Pagini</dt><dd>{pages.filter((page) => page.pageKind !== "section").length}</dd></div>
      <div><dt>Secțiuni</dt><dd>{pages.filter((page) => page.pageKind === "section").length}</dd></div>
      <div class:warning={contentDiagnostics.length > 0}><dt>SEO</dt><dd>{contentDiagnostics.length}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="view-tabs" role="tablist" aria-label="Tipuri de conținut">
      {#each contentViews as view, index (view.id)}
        <button
          id={`content-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`content-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <label class="section-field">
      <span class="sr-only">Colecție de conținut</span>
      <select bind:value={sectionFilter} aria-label="Colecție de conținut">
        <option value="all">Toate colecțiile</option>
        {#each sections as section (section)}
          <option value={section}>{sectionLabel(section)}</option>
        {/each}
      </select>
    </label>
    <label class="search-field">
      <span class="sr-only">Caută în conținut</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută titlu, URL, template sau fișier" />
    </label>
    <button class="toolbar-action" type="button" disabled={creating} onclick={() => beginCreate()}>
      <IconPlus size={14} stroke={2} /> Adaugă
    </button>
  </div>

  <div class="workspace-body">
    <div
      class="content-list"
      id={`content-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`content-tab-${activeView}`}
    >
      <div class="column-head" aria-hidden="true"><span>Conținut</span><span>Tip</span><span>Template</span></div>
      <div class="page-list" role="listbox" aria-label="Intrări de conținut">
        {#each filteredPages as page (page.id)}
          <button
            type="button"
            role="option"
            aria-selected={selectedPage?.id === page.id}
            class:selected={selectedPage?.id === page.id}
            onclick={() => selectPage(page.id)}
          >
            <span class="page-main">
              <i aria-hidden="true">{#if page.pageKind === "home"}<IconHome size={15} />{:else}<IconFileText size={15} />{/if}</i>
              <span><strong>{page.title}</strong><small>{page.url || "/"} · {page.file}</small></span>
            </span>
            <span class="kind-badge">{kindLabel(page.pageKind)}</span>
            <code>{page.resolvedTemplate ?? page.frontmatterTemplate ?? "implicit"}</code>
          </button>
        {:else}
          <div class="empty-state">
            <IconSearch size={25} stroke={1.5} />
            <strong>{pages.length === 0 ? "Nu există conținut indexat" : "Nicio intrare nu corespunde filtrelor"}</strong>
            <span>{pages.length === 0 ? "Creează prima pagină Markdown pentru acest proiect Zola." : "Schimbă tabul, colecția sau termenul de căutare."}</span>
          </div>
        {/each}
      </div>
    </div>

    <aside class="detail-panel" aria-label="Panou contextual conținut">
      {#if detailMode === "create"}
        <header class="detail-header">
          <div><span>Intrare nouă</span><h2>Pagină Markdown</h2><p>Crearea este validată de Rust și devine o singură tranzacție ProjectWorkspace.</p></div>
          <button type="button" aria-label="Renunță la creare" disabled={creating} onclick={resetPanel}><IconX size={14} /></button>
        </header>
        <form onsubmit={(event) => { event.preventDefault(); void createPage(); }}>
          <label>
            <span>Titlu</span>
            <input
              value={titleDraft}
              oninput={(event) => updateTitle(event.currentTarget.value)}
              placeholder="Despre noi"
              disabled={creating}
            />
          </label>
          <label>
            <span>Slug URL</span>
            <input
              value={slugDraft}
              oninput={(event) => { slugTouched = true; slugDraft = event.currentTarget.value; }}
              placeholder="despre-noi"
              disabled={creating}
            />
            <small>Va deveni <code>{slugifyPageTitle(slugDraft || titleDraft) || "slug"}.md</code>.</small>
          </label>
          <label>
            <span>Secțiune</span>
            <select bind:value={sectionDraft} disabled={creating}>
              {#each sections as section (section)}
                <option value={section}>{sectionLabel(section)}</option>
              {/each}
            </select>
            <small>Fișierul va fi creat sub <code>content/{sectionDraft ? `${sectionDraft}/` : ""}</code>.</small>
          </label>
          {#if createError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {createError}</p>{/if}
          <div class="form-actions">
            <button type="button" onclick={resetPanel} disabled={creating}>Renunță</button>
            <button class="primary" type="submit" disabled={creating || !titleDraft.trim()}>
              <IconPlus size={14} /> {creating ? "Se creează prin Rust…" : "Creează în sesiune"}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit" && selectedPage}
        <header class="detail-header">
          <div><span>Modificare controlată</span><h2>{selectedPage.title}</h2><p>Frontmatter-ul rămâne draft până la Ctrl+S, când intră într-o tranzacție coerentă.</p></div>
          <button type="button" aria-label="Încheie editarea" onclick={resetPanel}><IconX size={14} /></button>
        </header>
        {#if metadataError}
          <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {metadataError}</p>
        {:else if metadataLoading}
          <div class="empty-state">Se citește frontmatter-ul din sesiunea proiectului…</div>
        {:else}
          <div class="metadata-editor">
            <ProjectPageSettingsTab
              activeScannedPath={selectedPage.file}
              scannedPages={app.scannedPages}
              scannedTemplates={app.scannedTemplates}
              activeTheme={app.scannedProject?.activeTheme ?? null}
              pageSource={metadataSource}
              updatePageFrontmatterSource={updateMetadataSource}
            />
          </div>
          <button class="secondary-action" type="button" onclick={resetPanel}>Încheie editarea</button>
        {/if}
      {:else if selectedPage}
        <span class="detail-kicker">{kindLabel(selectedPage.pageKind)} · {contentSection(selectedPage.file) || "root"}</span>
        <h2>{selectedPage.title}</h2>
        <a class="route" href={selectedPage.url || "/"} onclick={(event) => { event.preventDefault(); void app.openCurrentProjectInBrowser(selectedPage.url || "/"); }}>
          {selectedPage.url || "/"} <IconExternalLink size={13} />
        </a>
        <dl>
          <div><dt>Fișier Markdown</dt><dd>{selectedPage.file}</dd></div>
          <div><dt>Template rezolvat</dt><dd>{selectedPage.resolvedTemplate ?? "Template implicit Zola"}</dd></div>
          <div><dt>Template declarat</dt><dd>{selectedPage.frontmatterTemplate ?? "—"}</dd></div>
          <div><dt>Relații în harta surselor</dt><dd>{relationCount(selectedPage)}</dd></div>
        </dl>
        {#if selectedDiagnostics.length > 0}
          <section class="quality-card" aria-label="Probleme pentru această pagină">
            <strong><IconAlertTriangle size={14} /> {selectedDiagnostics.length} {selectedDiagnostics.length === 1 ? "problemă" : "probleme"}</strong>
            {#each selectedDiagnostics.slice(0, 3) as diagnostic (diagnostic.id)}
              <span>{diagnostic.message}</span>
            {/each}
          </section>
        {:else}
          <section class="quality-card clean"><strong>Fără probleme cunoscute pentru această pagină</strong><span>Rezultatul provine din ultimul audit Rust.</span></section>
        {/if}
        <div class="detail-actions">
          <button class="primary-action" type="button" onclick={() => { void beginEdit(selectedPage); }}>
            <IconEdit size={14} /> Editează
          </button>
          <button class="secondary-action" type="button" onclick={() => { void openSource(selectedPage); }}>
            <IconCode size={14} /> Deschide Markdown
          </button>
        </div>
        <button class="secondary-action" type="button" onclick={() => { void app.openCurrentProjectInBrowser(selectedPage.url || "/"); }}>
          Vezi pagina publică <IconExternalLink size={13} />
        </button>
      {:else}
        <div class="empty-state"><strong>Selectează o intrare</strong><span>Detaliile despre rută, template și calitate vor apărea aici.</span></div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .content-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .eyebrow, .workspace-toolbar, .view-tabs, .search-field, .toolbar-action, .page-main, .detail-header, .route, .quality-card strong, .detail-actions, .primary-action, .secondary-action, .form-error, .form-actions, .form-actions button { display: flex; align-items: center; }
  .eyebrow { gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 76px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  dt, .detail-kicker { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .workspace-toolbar { justify-content: flex-end; gap: 8px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; margin-right: auto; }
  .view-tabs button { height: 100%; padding: 0 10px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 600; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .search-field { position: relative; width: min(320px, 30vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .workspace-toolbar input, .workspace-toolbar select, form input, form select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; }
  .section-field select { min-width: 145px; padding: 0 7px; }
  .toolbar-action { flex: 0 0 auto; justify-content: center; gap: 5px; min-height: 28px; padding: 0 10px; border: 1px solid var(--wb-accent); border-radius: var(--radius-control); color: #fff; background: var(--wb-accent); font-size: 12px; font-weight: 650; }
  .workspace-body { display: grid; grid-template-columns: minmax(390px, 1fr) minmax(300px, .58fr); min-width: 0; min-height: 0; }
  .content-list { display: grid; grid-template-rows: 28px minmax(0, 1fr); min-width: 0; min-height: 0; border-right: 1px solid var(--wb-border-subtle); }
  .column-head, .page-list > button { display: grid; grid-template-columns: minmax(180px, 1fr) 78px minmax(110px, .7fr); gap: 9px; align-items: center; }
  .column-head { padding: 0 11px; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .page-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; }
  .page-list > button { width: 100%; min-height: 54px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .page-list > button:hover, .page-list > button.selected { border-color: var(--wb-border-subtle); background: var(--control-hover); }
  .page-list > button.selected { background: var(--control-selected); box-shadow: inset 3px 0 0 var(--wb-accent); }
  .page-main { min-width: 0; gap: 8px; }
  .page-main > i { display: grid; width: 27px; height: 27px; flex: 0 0 auto; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .page-main > span { display: grid; min-width: 0; gap: 3px; }
  .page-main strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .page-main small, .page-list code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .kind-badge { justify-self: start; padding: 2px 5px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  .detail-panel { min-width: 0; min-height: 0; padding: 17px; overflow: auto; background: var(--wb-surface-chrome); }
  .detail-panel > h2 { margin: 6px 0 0; color: var(--text-strong); font-size: 19px; overflow-wrap: anywhere; }
  .route { justify-content: space-between; gap: 6px; margin-top: 9px; padding: 7px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-surface-document); font-size: 12px; text-decoration: none; }
  .detail-panel dl { display: grid; gap: 6px; margin: 12px 0; }
  .detail-panel dl div { display: grid; gap: 3px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .detail-panel dl dd { margin: 0; overflow-wrap: anywhere; color: var(--wb-text-primary); font-size: 12px; font-weight: 500; line-height: 1.35; }
  .quality-card { display: grid; gap: 5px; margin: 11px 0; padding: 9px; border: 1px solid color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); border-radius: 7px; background: color-mix(in srgb, var(--wb-warning) 7%, var(--wb-surface-document)); }
  .quality-card.clean { border-color: color-mix(in srgb, var(--success) 35%, var(--wb-border-subtle)); background: color-mix(in srgb, var(--success) 6%, var(--wb-surface-document)); }
  .quality-card strong { gap: 5px; color: var(--text-strong); font-size: 12px; }
  .quality-card span { color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; }
  .detail-header { align-items: flex-start; justify-content: space-between; gap: 10px; padding-bottom: 12px; border-bottom: 1px solid var(--wb-border-subtle); }
  .detail-header > div { display: grid; gap: 3px; }
  .detail-header span { color: var(--wb-accent-strong); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .detail-header h2 { margin: 0; color: var(--text-strong); font-size: 19px; }
  .detail-header p { margin: 2px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .detail-header button { display: grid; flex: 0 0 auto; width: 28px; height: 28px; padding: 0; place-items: center; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-muted); background: var(--wb-surface-document); }
  form { display: grid; gap: 11px; padding-top: 14px; }
  form label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 700; }
  form input, form select { width: 100%; height: 34px; padding: 0 8px; }
  form small { margin: 0; color: var(--wb-text-muted); font-size: 12px; font-weight: 500; line-height: 1.4; }
  .form-error { align-items: flex-start; gap: 5px; margin: 9px 0 0; padding: 8px; border: 1px solid color-mix(in srgb, var(--danger) 36%, var(--wb-border-subtle)); border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, var(--wb-surface-document)); font-size: 12px; }
  .form-actions { justify-content: flex-end; gap: 7px; }
  .form-actions button, .primary-action, .secondary-action { justify-content: center; gap: 5px; min-height: 32px; padding: 0 10px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 600; }
  .form-actions .primary, .primary-action { border-color: var(--wb-accent); color: #fff; background: var(--wb-accent); }
  .detail-actions { align-items: stretch; gap: 7px; margin-top: 10px; }
  .detail-actions .primary-action, .detail-actions .secondary-action { flex: 1; }
  .detail-panel > .secondary-action { width: 100%; margin-top: 7px; }
  .metadata-editor { min-width: 0; margin-top: 10px; }
  .metadata-editor :global(.page-settings-panel) { padding: 0; border: 0; background: transparent; }
  .metadata-editor :global(.page-file-chip) { background: var(--wb-surface-document); }
  .metadata-editor :global(.metadata-group) { border-color: var(--wb-border-subtle); background: var(--wb-surface-document); }
  .empty-state { display: flex; min-height: 180px; align-items: center; justify-content: center; flex-direction: column; gap: 6px; padding: 22px; color: var(--wb-text-muted); text-align: center; font-size: 12px; }
  .empty-state strong { color: var(--text-strong); font-size: 12px; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .55; }
  button:focus-visible, input:focus-visible, select:focus-visible, a:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .detail-panel { display: none; } .content-list { border-right: 0; } }
</style>
