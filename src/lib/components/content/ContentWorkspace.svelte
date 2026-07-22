<script lang="ts">
  import {
    IconAlertTriangle,
    IconCode,
    IconExternalLink,
    IconFileText,
    IconFolder,
    IconHome,
    IconPlus,
    IconSearch,
    IconX,
  } from "@tabler/icons-svelte";
  import ProjectPageSettingsTab from "$lib/components/project/ProjectPageSettingsTab.svelte";
  import { slugifyPageTitle } from "$lib/project/files";
  import type { AppState } from "$lib/state/app.svelte";
  import type { SourceGraphPage, SourcePageKind } from "$lib/types";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type KindFilter = "all" | SourcePageKind;

  let query = $state("");
  let kindFilter = $state<KindFilter>("all");
  let sectionFilter = $state("all");
  let selectedPageId = $state("");
  let createOpen = $state(false);
  let titleDraft = $state("");
  let slugDraft = $state("");
  let sectionDraft = $state("");
  let slugTouched = $state(false);
  let creating = $state(false);
  let createError = $state("");
  let metadataPageId = $state("");
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
        (kindFilter === "all" || page.pageKind === kindFilter)
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
    const normalized = file.replaceAll("\\", "/").replace(/^sursa\/content\/?/, "");
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

  function beginCreate(section = sectionFilter === "all" ? "" : sectionFilter) {
    sectionDraft = section;
    titleDraft = "";
    slugDraft = "";
    slugTouched = false;
    createError = "";
    createOpen = true;
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
      createOpen = false;
      app.setGlobalStatus(
        `Pagina ${relativePath} este pregătită în sesiunea proiectului. Salvează pentru persistență pe disc.`,
        "unsaved",
      );
    } catch (error) {
      createError = error instanceof Error ? error.message : String(error);
    } finally {
      creating = false;
    }
  }

  async function openSource(page: SourceGraphPage) {
    await openWorkspaceSource(page.file);
  }

  async function openMetadata(page: SourceGraphPage) {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    metadataPageId = page.id;
    metadataLoading = true;
    metadataError = "";
    try {
      const source = await app.readPageSettingsDocument(page.file);
      if (
        app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || metadataPageId !== page.id
      ) return;
      metadataSource = source;
    } catch (error) {
      if (metadataPageId === page.id) {
        metadataError = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (metadataPageId === page.id) metadataLoading = false;
    }
  }

  function updateMetadataSource(relativePath: string, source: string) {
    metadataSource = source;
    app.updatePageFrontmatterSource(relativePath, source);
  }
</script>

<section class="content-workspace" aria-labelledby="content-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconFileText size={15} stroke={1.9} /> Content workspace</span>
      <h1 id="content-title">Conținut</h1>
      <p>Paginile și secțiunile sunt proiectate din harta surselor; Markdown-ul Zola rămâne sursa reală.</p>
    </div>
    <div class="header-summary" aria-label="Rezumat conținut">
      <div><span>Pagini</span><strong>{pages.length}</strong></div>
      <div><span>Secțiuni</span><strong>{pages.filter((page) => page.pageKind === "section").length}</strong></div>
      <div class:warning={contentDiagnostics.length > 0}><span>SEO</span><strong>{contentDiagnostics.length}</strong></div>
      <button type="button" onclick={() => beginCreate()}>
        <IconPlus size={15} stroke={2} /> Pagină nouă
      </button>
    </div>
  </header>

  <div class="workspace-toolbar">
    <label class="search-field">
      <span class="sr-only">Caută în conținut</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută titlu, URL, template sau fișier" />
    </label>
    <label>
      <span class="sr-only">Tip de conținut</span>
      <select bind:value={kindFilter} aria-label="Tip de conținut">
        <option value="all">Toate tipurile</option>
        <option value="home">Pagina principală</option>
        <option value="section">Secțiuni</option>
        <option value="page">Pagini</option>
      </select>
    </label>
  </div>

  <div class:drawer-open={createOpen} class="workspace-body">
    <aside class="collections" aria-label="Colecții de conținut">
      <div class="aside-heading"><span>Colecții</span><strong>{sections.length}</strong></div>
      <button
        type="button"
        class:active={sectionFilter === "all"}
        onclick={() => { sectionFilter = "all"; }}
      >
        <IconFileText size={15} stroke={1.8} />
        <span>Tot conținutul</span>
        <em>{pages.length}</em>
      </button>
      {#each sections as section (section)}
        <button
          type="button"
          class:active={sectionFilter === section}
          onclick={() => { sectionFilter = section; }}
        >
          <IconFolder size={15} stroke={1.8} />
          <span>{sectionLabel(section)}</span>
          <em>{pages.filter((page) => contentSection(page.file) === section).length}</em>
        </button>
      {/each}
    </aside>

    <section class="content-list" aria-labelledby="content-list-title">
      <header>
        <div><h2 id="content-list-title">{sectionFilter === "all" ? "Toate intrările" : sectionLabel(sectionFilter)}</h2><span>{filteredPages.length} rezultate</span></div>
        <button type="button" onclick={() => beginCreate()}><IconPlus size={14} /> Adaugă</button>
      </header>
      <div class="column-head" aria-hidden="true"><span>Conținut</span><span>Tip</span><span>Template</span></div>
      <div class="page-list" role="listbox" aria-label="Intrări de conținut">
        {#each filteredPages as page (page.id)}
          <button
            type="button"
            role="option"
            aria-selected={selectedPage?.id === page.id}
            class:selected={selectedPage?.id === page.id}
            onclick={() => { selectedPageId = page.id; createOpen = false; metadataPageId = ""; }}
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
            <span>{pages.length === 0 ? "Creează prima pagină Markdown pentru acest proiect Zola." : "Schimbă colecția, tipul sau termenul de căutare."}</span>
          </div>
        {/each}
      </div>
    </section>

    <aside class="detail-panel" aria-label={createOpen ? "Creează pagină" : "Detalii conținut"}>
      {#if createOpen}
        <header class="detail-header">
          <div><span>Intrare nouă</span><h2>Pagină Markdown</h2></div>
          <button type="button" aria-label="Închide formularul" onclick={() => { createOpen = false; }}><IconX size={16} /></button>
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
            <button type="button" onclick={() => { createOpen = false; }} disabled={creating}>Renunță</button>
            <button class="primary" type="submit" disabled={creating || !titleDraft.trim()}>
              <IconPlus size={14} /> {creating ? "Se creează prin Rust…" : "Creează în sesiune"}
            </button>
          </div>
        </form>
        <p class="authority-note">Crearea este jurnalizată în sesiunea proiectului. Fișierul ajunge pe disc numai la salvare.</p>
      {:else if selectedPage && metadataPageId === selectedPage.id}
        <header class="detail-header metadata-heading">
          <div><span>Metadata code-native</span><h2>{selectedPage.title}</h2></div>
          <button type="button" aria-label="Închide metadata" onclick={() => { metadataPageId = ""; }}><IconX size={16} /></button>
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
        <button class="primary-action" type="button" onclick={() => { void openSource(selectedPage); }}>
          <IconCode size={15} /> Deschide Markdown
        </button>
        <button class="secondary-action" type="button" onclick={() => { void openMetadata(selectedPage); }}>
          Editează frontmatter și taxonomii
        </button>
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
  .content-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 10px; color: var(--wb-text-primary); background: var(--wb-surface-document); box-shadow: var(--shadow); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: radial-gradient(circle at 18% 0%, var(--wb-accent-soft), transparent 36%), var(--wb-surface-chrome); }
  .eyebrow, .header-summary, .header-summary button, .workspace-toolbar, .search-field, .aside-heading, .collections button, .content-list > header, .content-list > header > div, .content-list > header button, .page-main, .detail-header, .detail-header button, .route, .quality-card strong, .primary-action, .secondary-action, .form-error, .form-actions, .form-actions button { display: flex; align-items: center; }
  .eyebrow { gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; letter-spacing: .06em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 24px; letter-spacing: -.025em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .header-summary { gap: 7px; }
  .header-summary > div { display: grid; min-width: 67px; gap: 2px; padding: 6px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  .header-summary > div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  .header-summary span, .detail-kicker, dt { color: var(--wb-text-muted); font-size: 12px; font-weight: 850; letter-spacing: .04em; text-transform: uppercase; }
  .header-summary strong { color: var(--text-strong); font-size: 14px; }
  .header-summary button, .content-list > header button, .primary-action, .secondary-action, .form-actions button { justify-content: center; gap: 5px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; font-weight: 800; }
  .header-summary button { min-height: 34px; padding: 0 12px; border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .workspace-toolbar { justify-content: flex-end; gap: 7px; padding: 6px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .search-field { position: relative; flex: 1; max-width: 480px; margin-right: auto; }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .workspace-toolbar input, .workspace-toolbar select, form input, form select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .workspace-toolbar input { width: 100%; padding: 0 8px 0 28px; }
  .workspace-toolbar select { min-width: 140px; padding: 0 7px; }
  .workspace-body { display: grid; grid-template-columns: 188px minmax(390px, 1fr) minmax(270px, 320px); min-width: 0; min-height: 0; }
  .collections { min-width: 0; min-height: 0; padding: 10px 8px; overflow: auto; border-right: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .aside-heading { justify-content: space-between; padding: 3px 7px 9px; color: var(--wb-text-muted); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  .aside-heading strong { padding: 1px 5px; border-radius: 999px; background: var(--surface-4); }
  .collections button { width: 100%; min-height: 32px; gap: 7px; padding: 0 8px; border: 0; border-radius: 6px; color: var(--wb-text-muted); background: transparent; font-size: 12px; text-align: left; }
  .collections button span { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .collections button em { font-size: 12px; font-style: normal; }
  .collections button:hover, .collections button.active { color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .collections button.active { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .content-list { display: grid; grid-template-rows: 44px 27px minmax(0, 1fr); min-width: 0; min-height: 0; border-right: 1px solid var(--wb-border-subtle); }
  .content-list > header { justify-content: space-between; gap: 10px; padding: 0 10px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-document); }
  .content-list > header > div { gap: 7px; }
  h2 { margin: 0; color: var(--text-strong); font-size: 13px; }
  .content-list > header span { color: var(--wb-text-muted); font-size: 12px; }
  .content-list > header button { min-height: 27px; padding: 0 8px; }
  .column-head, .page-list > button { display: grid; grid-template-columns: minmax(180px, 1fr) 78px minmax(110px, .7fr); gap: 9px; align-items: center; }
  .column-head { padding: 0 11px; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  .page-list { min-width: 0; min-height: 0; overflow: auto; }
  .page-list > button { width: 100%; min-height: 54px; padding: 7px 11px; border: 0; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: var(--wb-surface-document); text-align: left; }
  .page-list > button:hover, .page-list > button.selected { background: var(--wb-accent-soft); }
  .page-list > button.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .page-main { min-width: 0; gap: 8px; }
  .page-main > i { display: grid; width: 26px; height: 26px; flex: 0 0 auto; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .page-main > span { display: grid; min-width: 0; gap: 3px; }
  .page-main strong { overflow: hidden; color: var(--text-strong); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .page-main small, .page-list code { overflow: hidden; color: var(--wb-text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .kind-badge { justify-self: start; padding: 2px 5px; border-radius: 999px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); font-size: 12px; font-weight: 800; }
  .detail-panel { min-width: 0; min-height: 0; padding: 16px; overflow: auto; background: var(--wb-surface-chrome); }
  .detail-panel > h2 { margin-top: 6px; font-size: 19px; overflow-wrap: anywhere; }
  .route { justify-content: space-between; gap: 6px; margin-top: 9px; padding: 7px 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-surface-document); font-size: 12px; text-decoration: none; }
  .detail-panel dl { display: grid; gap: 6px; margin: 12px 0; }
  .detail-panel dl div { display: grid; gap: 3px; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  dd { margin: 0; overflow-wrap: anywhere; color: var(--wb-text-primary); font-size: 12px; line-height: 1.35; }
  .quality-card { display: grid; gap: 5px; margin: 11px 0; padding: 9px; border: 1px solid color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); border-radius: 7px; background: color-mix(in srgb, var(--wb-warning) 7%, var(--wb-surface-document)); }
  .quality-card.clean { border-color: color-mix(in srgb, var(--success) 35%, var(--wb-border-subtle)); background: color-mix(in srgb, var(--success) 6%, var(--wb-surface-document)); }
  .quality-card strong { gap: 5px; color: var(--text-strong); font-size: 12px; }
  .quality-card span { color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; }
  .primary-action, .secondary-action { width: 100%; min-height: 31px; margin-top: 7px; }
  .primary-action, .form-actions .primary { border-color: var(--wb-accent); color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .detail-header { justify-content: space-between; gap: 10px; padding-bottom: 12px; border-bottom: 1px solid var(--wb-border-subtle); }
  .metadata-heading { margin-bottom: 10px; }
  .detail-header > div { display: grid; gap: 3px; }
  .detail-header span { color: var(--wb-accent-strong); font-size: 12px; font-weight: 850; text-transform: uppercase; }
  .detail-header button { width: 27px; height: 27px; justify-content: center; border: 1px solid var(--wb-border-subtle); border-radius: 6px; color: var(--wb-text-muted); background: var(--wb-surface-document); }
  form { display: grid; gap: 11px; padding: 14px 0; }
  form label { display: grid; gap: 5px; color: var(--wb-text-muted); font-size: 12px; font-weight: 800; }
  form input, form select { width: 100%; padding: 0 8px; }
  form small, .authority-note { margin: 0; color: var(--wb-text-muted); font-size: 12px; font-weight: 500; line-height: 1.4; }
  .form-error { align-items: flex-start; gap: 5px; margin: 0; padding: 7px; border-radius: 6px; color: var(--danger); background: color-mix(in srgb, var(--danger) 8%, var(--wb-surface-document)); font-size: 12px; }
  .form-actions { justify-content: flex-end; gap: 6px; }
  .form-actions button { min-height: 29px; padding: 0 9px; }
  .authority-note { padding: 9px; border: 1px dashed var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .metadata-editor { min-width: 0; }
  .metadata-editor :global(.page-settings-panel) { border: 0; background: transparent; padding: 0; }
  .metadata-editor :global(.page-file-chip) { background: var(--wb-surface-document); }
  .metadata-editor :global(.metadata-group) { border-color: var(--wb-border-subtle); background: var(--wb-surface-document); }
  .empty-state { display: flex; min-height: 180px; align-items: center; justify-content: center; flex-direction: column; gap: 6px; padding: 22px; color: var(--wb-text-muted); text-align: center; font-size: 12px; }
  .empty-state strong { color: var(--text-strong); font-size: 12px; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .55; }
  button:focus-visible, input:focus-visible, select:focus-visible, a:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 1050px) {
    .workspace-body { grid-template-columns: 160px minmax(330px, 1fr) minmax(240px, 280px); }
    .header-summary > div { display: none; }
  }
</style>
