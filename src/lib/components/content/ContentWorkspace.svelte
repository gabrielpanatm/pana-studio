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
    IconSettings,
    IconTags,
    IconX,
  } from "@tabler/icons-svelte";
  import PageCustomFieldsPanel from "$lib/components/content/PageCustomFieldsPanel.svelte";
  import MarkdownEditor from "$lib/components/markdown/MarkdownEditor.svelte";
  import PageTaxonomyAssignments from "$lib/components/content/PageTaxonomyAssignments.svelte";
  import ProjectPageSettingsTab from "$lib/components/project/ProjectPageSettingsTab.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type {
    PageFrontmatterField,
    PageFrontmatterMutationValue,
  } from "$lib/markdown/frontmatter";
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
  type DetailMode = "info" | "create";
  type PageSettingsView = "settings" | "seo" | "custom_fields";

  const contentViews = $derived([
    { id: "all" as const, label: t("content-view-all") },
    { id: "pages" as const, label: t("content-view-pages") },
    { id: "sections" as const, label: t("content-view-sections") },
  ]);

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
  let loadedMetadataPath = $state("");
  let metadataRequestSerial = 0;
  let contentSessionId = "";
  let pageListElement = $state<HTMLDivElement | null>(null);
  let pageListScrollTop = $state(0);
  let pageSettingsView = $state<PageSettingsView>("settings");

  const pages = $derived(app.sourceGraph?.pages ?? []);
  const sections = $derived.by(() => {
    const values = new Set<string>();
    for (const page of pages) values.add(contentSection(page.file));
    values.add("");
    return [...values].sort((left, right) => l10n.compare(left, right));
  });
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const filteredPages = $derived(
    pages
      .filter((page) => (
        (activeView === "all"
          || activeView === "sections" && page.pageKind === "section"
          || activeView === "pages" && page.pageKind !== "section")
        && (sectionFilter === "all" || contentSection(page.file) === sectionFilter)
        && (!normalizedQuery || `${page.title} ${page.url} ${page.file} ${page.resolvedTemplate ?? ""}`
          .toLocaleLowerCase(l10n.locale)
          .includes(normalizedQuery))
      ))
      .sort((left, right) => {
        if (left.pageKind === "home" && right.pageKind !== "home") return -1;
        if (right.pageKind === "home" && left.pageKind !== "home") return 1;
        return l10n.compare(left.url, right.url);
      }),
  );
  const selectedPage = $derived(
    pages.find((page) => page.id === selectedPageId) ?? filteredPages[0] ?? null,
  );
  const editingPagePath = $derived(
    app.workbenchSnapshot?.contentWorkspace.mode === "edit"
      ? app.workbenchSnapshot.contentWorkspace.pagePath
      : null,
  );
  const editingPage = $derived(
    editingPagePath
      ? pages.find((page) => page.file === editingPagePath) ?? null
      : null,
  );
  const currentAudit = $derived(app.currentProjectAuditReceipt());
  const contentDiagnostics = $derived(
    (currentAudit?.findings ?? []).filter((finding) => (
      finding.category === "seo"
      && ["violation", "needs_review", "engine_error"].includes(finding.outcome)
    )),
  );
  const selectedDiagnostics = $derived(
    selectedPage
      ? (currentAudit?.findings ?? []).filter(
        (finding) => finding.primaryLocation?.file === selectedPage.file,
      )
      : [],
  );

  function contentSection(file: string) {
    const normalized = file.replaceAll("\\", "/").replace(/^content\/?/, "");
    const slash = normalized.lastIndexOf("/");
    return slash < 0 ? "" : normalized.slice(0, slash);
  }

  function sectionLabel(section: string) {
    return section || t("content-root-section");
  }

  function kindLabel(kind: SourcePageKind) {
    if (kind === "home") return t("content-kind-home");
    if (kind === "section") return t("content-kind-section");
    return t("content-kind-page");
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
      createError = t("content-title-required");
      return;
    }
    if (!slug) {
      createError = t("content-slug-invalid");
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
        createError = app.projectStatus || t("content-create-failed");
        return;
      }
      selectedPageId = app.sourceGraph?.pages.find((page) => page.file === relativePath)?.id ?? "";
      detailMode = "info";
      app.setGlobalStatus(
        t("content-created-status", { path: relativePath }),
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
    selectedPageId = page.id;
    await app.openContentPageEditor(page.file);
  }

  function updateMetadataSource(relativePath: string, source: string) {
    metadataSource = source;
    app.updatePageFrontmatterSource(relativePath, source);
  }

  async function updateMetadataField(
    relativePath: string,
    field: PageFrontmatterField,
    value: PageFrontmatterMutationValue,
  ) {
    metadataSource = await app.updatePageFrontmatterField(relativePath, field, value);
    loadedMetadataPath = relativePath;
  }

  async function refreshMetadataSource() {
    if (!editingPagePath) return;
    metadataSource = await app.readPageSettingsDocument(editingPagePath);
    loadedMetadataPath = editingPagePath;
  }

  $effect(() => {
    const sessionId = app.kernelProjectSessionId;
    if (contentSessionId === sessionId) return;
    contentSessionId = sessionId;
    activeView = "all";
    detailMode = "info";
    query = "";
    sectionFilter = "all";
    selectedPageId = "";
    createError = "";
    metadataSource = "";
    metadataError = "";
    loadedMetadataPath = "";
    pageListScrollTop = 0;
    pageSettingsView = "settings";
  });

  $effect(() => {
    const relativePath = editingPagePath;
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const refreshToken = app.refreshToken;
    void workspaceRevision;
    void refreshToken;

    const requestSerial = ++metadataRequestSerial;
    if (!relativePath) {
      loadedMetadataPath = "";
      metadataLoading = false;
      metadataError = "";
      return;
    }

    metadataLoading = loadedMetadataPath !== relativePath;
    metadataError = "";
    void app.readPageSettingsDocument(relativePath).then((source) => {
      if (
        requestSerial !== metadataRequestSerial
        || app.sessionProjectRoot !== projectRoot
        || app.kernelProjectSessionId !== runtimeSessionId
        || editingPagePath !== relativePath
      ) return;
      metadataSource = source;
      loadedMetadataPath = relativePath;
    }).catch((error) => {
      if (requestSerial === metadataRequestSerial && editingPagePath === relativePath) {
        metadataError = errorMessage(error);
      }
    }).finally(() => {
      if (requestSerial === metadataRequestSerial && editingPagePath === relativePath) {
        metadataLoading = false;
      }
    });
  });

  $effect(() => {
    if (editingPagePath || !pageListElement) return;
    const element = pageListElement;
    requestAnimationFrame(() => {
      if (pageListElement === element) element.scrollTop = pageListScrollTop;
    });
  });

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

{#if editingPagePath}
  <section class="content-page-workspace" aria-label={t("content-page-editor-label")}>
    <div class="content-editor-panel">
      {#if metadataError}
        <div class="editor-diagnostic" role="alert">
          <IconAlertTriangle size={24} stroke={1.7} />
          <strong>{t("content-editor-unavailable")}</strong>
          <span>{metadataError}</span>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(editingPagePath); }}>
            <IconCode size={14} /> {t("content-open-markdown")}
          </button>
        </div>
      {:else if editingPage?.frontmatterParseError}
        <div class="editor-diagnostic" role="alert">
          <IconAlertTriangle size={24} stroke={1.7} />
          <strong>{t("content-frontmatter-invalid")}</strong>
          <span>{editingPage.frontmatterParseError}</span>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(editingPagePath); }}>
            <IconCode size={14} /> {t("content-open-markdown")}
          </button>
        </div>
      {:else if metadataLoading && loadedMetadataPath !== editingPagePath}
        <div class="editor-diagnostic">{t("content-loading-frontmatter")}</div>
      {:else if editingPage}
        <MarkdownEditor
          source={metadataSource}
          path={editingPagePath}
          refreshToken={app.refreshToken}
          onChange={(source, path) => updateMetadataSource(path, source)}
        />
      {:else}
        <div class="editor-diagnostic" role="alert">
          <IconAlertTriangle size={24} stroke={1.7} />
          <strong>{t("content-editor-unavailable")}</strong>
          <span>{t("content-page-no-longer-indexed", { path: editingPagePath })}</span>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openWorkspaceSource(editingPagePath); }}>
            <IconCode size={14} /> {t("content-open-markdown")}
          </button>
        </div>
      {/if}
    </div>

    <aside class="content-settings-panel" aria-label={t("content-page-settings-label")}>
      {#if editingPage?.frontmatterParseError}
        <div class="settings-diagnostic" role="alert">
          <IconAlertTriangle size={18} stroke={1.8} />
          <strong>{t("content-frontmatter-settings-blocked")}</strong>
          <span>{editingPage.frontmatterParseError}</span>
        </div>
      {:else if metadataError}
        <div class="settings-diagnostic" role="alert"><span>{metadataError}</span></div>
      {:else if metadataLoading && loadedMetadataPath !== editingPagePath}
        <div class="settings-diagnostic"><span>{t("content-loading-frontmatter")}</span></div>
      {:else if editingPage}
        <div class="page-settings-tabs" role="tablist" aria-label="Setările paginii">
          <button type="button" role="tab" class:active={pageSettingsView === "settings"} aria-selected={pageSettingsView === "settings"} onclick={() => { pageSettingsView = "settings"; }}><IconSettings size={13} /> Setări</button>
          <button type="button" role="tab" class:active={pageSettingsView === "seo"} aria-selected={pageSettingsView === "seo"} onclick={() => { pageSettingsView = "seo"; }}><IconSearch size={13} /> SEO</button>
          <button type="button" role="tab" class:active={pageSettingsView === "custom_fields"} aria-selected={pageSettingsView === "custom_fields"} onclick={() => { pageSettingsView = "custom_fields"; }}><IconTags size={13} /> Câmpuri</button>
        </div>
        <div class="metadata-editor">
          {#if pageSettingsView === "settings" || pageSettingsView === "seo"}
            <ProjectPageSettingsTab
              activeScannedPath={editingPage.file}
              scannedPages={app.scannedPages}
              scannedTemplates={app.scannedTemplates}
              activeTheme={app.scannedProject?.activeTheme ?? null}
              pageSource={metadataSource}
              pageKind={editingPage.pageKind}
              updatePageFrontmatterField={updateMetadataField}
              view={pageSettingsView === "seo" ? "seo" : "settings"}
            />
            {#if pageSettingsView === "settings"}<PageTaxonomyAssignments {app} page={editingPage} />{/if}
          {:else}
            <PageCustomFieldsPanel
              {app}
              pageFile={editingPage.file}
              onSourceChanged={refreshMetadataSource}
            />
          {/if}
        </div>
      {/if}
    </aside>
  </section>
{:else}
<section class="activity-workspace content-workspace" aria-labelledby="content-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconFileText size={15} stroke={1.9} /> {t("content-eyebrow")}</span>
      <h1 id="content-title">{t("content-title")}</h1>
      <p>{t("content-description")}</p>
    </div>
    <dl>
      <div><dt>{t("content-stat-pages")}</dt><dd>{l10n.formatNumber(pages.filter((page) => page.pageKind !== "section").length)}</dd></div>
      <div><dt>{t("content-stat-sections")}</dt><dd>{l10n.formatNumber(pages.filter((page) => page.pageKind === "section").length)}</dd></div>
      <div class:warning={contentDiagnostics.length > 0}><dt>SEO</dt><dd>{contentDiagnostics.length}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("content-types-label")}>
      {#each contentViews as view, index (view.id)}
        <button
          id={`content-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`content-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <div class="toolbar-query-group with-filter">
      <label class="toolbar-filter">
        <span class="sr-only">{t("content-collection-label")}</span>
        <select
          class="ui-field toolbar"
          bind:value={sectionFilter}
          aria-label={t("content-collection-label")}
        >
          <option value="all">{t("content-all-collections")}</option>
          {#each sections as section (section)}
            <option value={section}>{sectionLabel(section)}</option>
          {/each}
        </select>
      </label>
      <label class="search-field">
        <span class="sr-only">{t("content-search-label")}</span>
        <IconSearch size={14} stroke={1.9} />
        <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("content-search-placeholder")} />
      </label>
    </div>
    <button class="ui-button primary toolbar toolbar-action" type="button" disabled={creating} onclick={() => beginCreate()}>
      <IconPlus size={14} stroke={2} /> {t("content-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div
      class="content-list"
      id={`content-panel-${activeView}`}
      role="tabpanel"
      aria-labelledby={`content-tab-${activeView}`}
    >
      <div class="column-head" aria-hidden="true"><span>{t("content-column-content")}</span><span>{t("content-column-kind")}</span><span>{t("content-column-template")}</span></div>
      <div
        class="page-list"
        role="listbox"
        aria-label={t("content-entries-label")}
        bind:this={pageListElement}
        onscroll={(event) => { pageListScrollTop = event.currentTarget.scrollTop; }}
      >
        {#each filteredPages as page (page.id)}
          <button
            type="button"
            role="option"
            aria-selected={selectedPage?.id === page.id}
            class="ui-entity-selectable"
            data-ui-selected={selectedPage?.id === page.id ? "true" : undefined}
            onclick={() => selectPage(page.id)}
          >
            <span class="page-main">
              <i aria-hidden="true">{#if page.pageKind === "home"}<IconHome size={15} />{:else}<IconFileText size={15} />{/if}</i>
              <span><strong>{page.title}</strong><small>{page.url || "/"} · {page.file}</small></span>
            </span>
            <span class="kind-badge">{kindLabel(page.pageKind)}</span>
            <code>{page.resolvedTemplate ?? page.frontmatterTemplate ?? t("content-template-default")}</code>
          </button>
        {:else}
          <div class="empty-state">
            <IconSearch size={25} stroke={1.5} />
            <strong>{pages.length === 0
              ? t("content-empty-index-title")
              : t("content-empty-filter-title")}</strong>
            <span>{pages.length === 0
              ? t("content-empty-index-description")
              : t("content-empty-filter-description")}</span>
          </div>
        {/each}
      </div>
    </div>

    <aside class="detail-panel" aria-label={t("content-detail-label")}>
      {#if detailMode === "create"}
        <header class="detail-header">
          <div><span>{t("content-new-entry")}</span><h2>{t("content-markdown-page")}</h2><p>{t("content-create-description")}</p></div>
          <button class="ui-icon-button ui-close-button" type="button" aria-label={t("content-cancel-create")} disabled={creating} onclick={resetPanel}><IconX size={14} /></button>
        </header>
        <form onsubmit={(event) => { event.preventDefault(); void createPage(); }}>
          <label>
            <span>{t("content-page-title")}</span>
            <input
              value={titleDraft}
              oninput={(event) => updateTitle(event.currentTarget.value)}
              placeholder={t("content-title-placeholder")}
              disabled={creating}
            />
          </label>
          <label>
            <span>{t("content-url-slug")}</span>
            <input
              value={slugDraft}
              oninput={(event) => { slugTouched = true; slugDraft = event.currentTarget.value; }}
              placeholder={t("content-slug-placeholder")}
              disabled={creating}
            />
            <small>{t("content-file-result", {
              file: slugifyPageTitle(slugDraft || titleDraft) || "slug",
            })}</small>
          </label>
          <label>
            <span>{t("content-section")}</span>
            <select bind:value={sectionDraft} disabled={creating}>
              {#each sections as section (section)}
                <option value={section}>{sectionLabel(section)}</option>
              {/each}
            </select>
            <small>{t("content-file-location", {
              path: `content/${sectionDraft ? `${sectionDraft}/` : ""}`,
            })}</small>
          </label>
          {#if createError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {createError}</p>{/if}
          <div class="form-actions">
            <button type="button" onclick={resetPanel} disabled={creating}>{t("content-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={creating || !titleDraft.trim()}>
              <IconPlus size={14} /> {creating ? t("content-creating") : t("content-create-session")}
            </button>
          </div>
        </form>
      {:else if selectedPage}
        <span class="detail-kicker">{kindLabel(selectedPage.pageKind)} · {contentSection(selectedPage.file) || t("content-root-short")}</span>
        <h2>{selectedPage.title}</h2>
        <a class="route" href={selectedPage.url || "/"} onclick={(event) => { event.preventDefault(); void app.openCurrentProjectInBrowser(selectedPage.url || "/"); }}>
          {selectedPage.url || "/"} <IconExternalLink size={13} />
        </a>
        <dl>
          <div><dt>{t("content-markdown-file")}</dt><dd>{selectedPage.file}</dd></div>
          <div><dt>{t("content-resolved-template")}</dt><dd>{selectedPage.resolvedTemplate ?? t("content-zola-default-template")}</dd></div>
          <div><dt>{t("content-declared-template")}</dt><dd>{selectedPage.frontmatterTemplate ?? "—"}</dd></div>
          <div><dt>{t("content-source-relations")}</dt><dd>{l10n.formatNumber(relationCount(selectedPage))}</dd></div>
        </dl>
        {#if selectedDiagnostics.length > 0}
          <section class="quality-card" aria-label={t("content-page-problems-label")}>
            <strong><IconAlertTriangle size={14} /> {t("content-problems-count", { count: selectedDiagnostics.length })}</strong>
            {#each selectedDiagnostics.slice(0, 3) as diagnostic (diagnostic.id)}
              <span>{errorMessage(diagnostic.messageDiagnostic)}</span>
            {/each}
          </section>
        {:else}
          <section class="quality-card clean"><strong>{t("content-no-known-problems")}</strong><span>{t("content-audit-origin")}</span></section>
        {/if}
        <div class="detail-actions">
          <button class="ui-button primary primary-action" type="button" onclick={() => { void beginEdit(selectedPage); }}>
            <IconEdit size={14} /> {t("content-edit")}
          </button>
          <button class="ui-button secondary-action" type="button" onclick={() => { void openSource(selectedPage); }}>
            <IconCode size={14} /> {t("content-open-markdown")}
          </button>
        </div>
        <button class="ui-button secondary-action" type="button" onclick={() => { void app.openCurrentProjectInBrowser(selectedPage.url || "/"); }}>
          {t("content-view-public")} <IconExternalLink size={13} />
        </button>
      {:else}
        <div class="empty-state"><strong>{t("content-select-entry")}</strong><span>{t("content-select-description")}</span></div>
      {/if}
    </aside>
  </div>
</section>
{/if}

<style>
  .page-main, .detail-header, .route, .quality-card strong, .detail-actions, .primary-action, .secondary-action, .form-error, .form-actions, .form-actions button { display: flex; align-items: center; }
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--wb-warning) 45%, var(--wb-border-subtle)); }
  dt, .detail-kicker { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  form input, form select { height: 28px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--material-inset); box-shadow: var(--shadow-inset); font-size: 12px; }
  .workspace-body { display: grid; grid-template-columns: minmax(390px, 1fr) minmax(300px, .58fr); min-width: 0; min-height: 0; }
  .content-list { display: grid; grid-template-rows: 28px minmax(0, 1fr); min-width: 0; min-height: 0; border-right: 1px solid var(--wb-border-subtle); }
  .column-head, .page-list > button { display: grid; grid-template-columns: minmax(180px, 1fr) 78px minmax(110px, .7fr); gap: 9px; align-items: center; }
  .column-head { padding: 0 11px; border-bottom: 1px solid var(--wb-border-subtle); color: var(--wb-text-muted); background: var(--wb-surface-chrome); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .page-list { min-width: 0; min-height: 0; overflow: auto; padding: 8px; }
  .page-list > button { width: 100%; min-height: 54px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
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
  .page-settings-tabs { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 3px; padding: 3px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--material-inset); }
  .page-settings-tabs button { display: flex; min-width: 0; min-height: 28px; align-items: center; justify-content: center; gap: 4px; padding: 0 5px; border: 1px solid transparent; border-radius: 5px; color: var(--wb-text-muted); background: transparent; font-size: 11px; font-weight: 700; }
  .page-settings-tabs button.active { border-color: var(--wb-border-subtle); color: var(--wb-accent-strong); background: var(--wb-surface-document); box-shadow: var(--shadow-control); }
  .metadata-editor :global(.page-settings-panel) { padding: 0; border: 0; background: transparent; }
  .metadata-editor :global(.page-file-chip) { background: var(--wb-surface-document); }
  .metadata-editor :global(.metadata-group) { border-color: var(--wb-border-subtle); background: var(--wb-surface-document); }
  .content-page-workspace { display: grid; grid-template-columns: minmax(0, 1fr) minmax(290px, 360px); gap: 12px; width: 100%; height: 100%; min-width: 0; min-height: 0; padding: 12px; background: var(--wb-surface-chrome); }
  .content-editor-panel, .content-settings-panel { min-width: 0; min-height: 0; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: 12px; background: var(--wb-surface-document); box-shadow: var(--shadow-panel); }
  .content-editor-panel :global(.markdown-editor) { height: 100%; }
  .content-settings-panel { padding: 12px; overflow: auto; }
  .content-settings-panel .metadata-editor { margin-top: 0; }
  .editor-diagnostic, .settings-diagnostic { display: flex; height: 100%; align-items: center; justify-content: center; flex-direction: column; gap: 8px; padding: 24px; color: var(--wb-text-muted); text-align: center; }
  .editor-diagnostic strong, .settings-diagnostic strong { color: var(--text-strong); }
  .editor-diagnostic span, .settings-diagnostic span { max-width: 560px; font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
  .settings-diagnostic { height: auto; min-height: 180px; }
  .empty-state { display: flex; min-height: 180px; align-items: center; justify-content: center; flex-direction: column; gap: 6px; padding: 22px; color: var(--wb-text-muted); text-align: center; font-size: 12px; }
  .empty-state strong { color: var(--text-strong); font-size: 12px; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .55; }
  button:focus-visible, input:focus-visible, select:focus-visible, a:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .detail-panel { display: none; } .content-list { border-right: 0; } .content-page-workspace { grid-template-columns: 1fr; grid-template-rows: minmax(420px, 1fr) minmax(220px, auto); overflow: auto; } .content-settings-panel { max-height: 440px; } }
</style>
