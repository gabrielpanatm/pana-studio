<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconExternalLink,
    IconFileCode,
    IconLanguage,
    IconPlus,
    IconRefresh,
    IconRoute,
    IconSearch,
    IconTags,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    applyTaxonomyMutation,
    planTaxonomyMutation,
    readTaxonomyCatalog,
  } from "$lib/project/io";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    FileBufferRequestIdentity,
    TaxonomyCatalogEntry,
    TaxonomyCatalogSnapshot,
    TaxonomyCatalogTemplate,
    TaxonomyCatalogTerm,
    TaxonomyDefinitionInput,
    TaxonomyMutationInput,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type DetailMode = "info" | "create" | "edit";

  let catalog = $state<TaxonomyCatalogSnapshot | null>(null);
  let selectedId = $state<string | null>(null);
  let selectedTermId = $state<string | null>(null);
  let query = $state("");
  let loading = $state(false);
  let busy = $state(false);
  let loadError = $state("");
  let formError = $state("");
  let loadedKey = $state("");
  let detailMode = $state<DetailMode>("info");
  let deleteConfirmationOpen = $state(false);
  let removeAssignments = $state(false);

  let nameDraft = $state("");
  let languageDraft = $state("");
  let renderDraft = $state(true);
  let feedDraft = $state(false);
  let paginateByDraft = $state("");
  let paginatePathDraft = $state("");
  let taxonomyRootDraft = $state("");
  let termDraft = $state("");

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const visibleEntries = $derived(
    (catalog?.entries ?? []).filter((entry) => (
      !normalizedQuery
      || `${entry.name} ${entry.language} ${entry.slug} ${entry.terms.map((term) => term.name).join(" ")}`
        .toLocaleLowerCase(l10n.locale)
        .includes(normalizedQuery)
    )),
  );
  const selected = $derived(
    visibleEntries.find((entry) => entry.id === selectedId)
      ?? visibleEntries[0]
      ?? null,
  );
  const selectedTerm = $derived(
    selected?.terms.find((term) => term.id === selectedTermId) ?? null,
  );
  const selectedDiagnostics = $derived(
    (catalog?.diagnostics ?? []).filter((diagnostic) => (
      selected
        ? diagnostic.taxonomyName === selected.name
          || diagnostic.file === catalog?.configPath
          || diagnostic.file && selected.pages.some((page) => page.file === diagnostic.file)
        : true
    )),
  );
  const counts = $derived({
    definitions: (catalog?.entries ?? []).filter((entry) => entry.declared).length,
    terms: (catalog?.entries ?? []).reduce((total, entry) => total + entry.terms.length, 0),
    pages: new Set(
      (catalog?.entries ?? []).flatMap((entry) => entry.pages.map((page) => page.file)),
    ).size,
    diagnostics: catalog?.diagnostics.length ?? 0,
  });

  $effect(() => {
    const root = app.sessionProjectRoot.trim();
    const sessionId = app.kernelProjectSessionId.trim();
    const revision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const key = `${root}:${sessionId}:${revision}`;
    if (!root || !sessionId || loading || busy || loadedKey === key) return;
    loadedKey = key;
    void loadCatalog(root, sessionId, revision);
  });

  function currentCatalogKey() {
    return `${app.sessionProjectRoot.trim()}:${app.kernelProjectSessionId.trim()}:${app.projectWorkspaceSnapshot?.revision ?? 0}`;
  }

  async function loadCatalog(
    root = app.sessionProjectRoot,
    sessionId = app.kernelProjectSessionId,
    expectedWorkspaceRevision = app.projectWorkspaceSnapshot?.revision ?? 0,
  ) {
    loading = true;
    loadError = "";
    try {
      const snapshot = await readTaxonomyCatalog({
        expectedProjectRoot: root,
        expectedSessionId: sessionId,
      }, expectedWorkspaceRevision);
      if (
        root !== app.sessionProjectRoot
        || sessionId !== app.kernelProjectSessionId
        || app.projectWorkspaceSnapshot?.revision !== expectedWorkspaceRevision
      ) return;
      catalog = snapshot;
      taxonomyRootDraft = snapshot.taxonomyRoot ?? "";
      if (!snapshot.entries.some((entry) => entry.id === selectedId)) {
        selectedId = snapshot.entries[0]?.id ?? null;
        selectedTermId = null;
      }
    } catch (error) {
      if (root === app.sessionProjectRoot && sessionId === app.kernelProjectSessionId) {
        loadError = errorMessage(error);
      }
    } finally {
      if (root === app.sessionProjectRoot && sessionId === app.kernelProjectSessionId) {
        loading = false;
      }
    }
  }

  function identity(): FileBufferRequestIdentity {
    return {
      expectedProjectRoot: app.sessionProjectRoot,
      expectedSessionId: app.kernelProjectSessionId,
    };
  }

  async function executeMutation(
    input: TaxonomyMutationInput,
    successMessage: string,
    preferredPath: string | null = catalog?.configPath ?? null,
  ): Promise<boolean> {
    if (busy) return false;
    busy = true;
    formError = "";
    try {
      const commandIdentity = identity();
      const plan = await planTaxonomyMutation(input, commandIdentity);
      if (
        commandIdentity.expectedProjectRoot !== app.sessionProjectRoot
        || commandIdentity.expectedSessionId !== app.kernelProjectSessionId
      ) return false;
      const receipt = await applyTaxonomyMutation(input, plan.planId, commandIdentity);
      const settlement = await settleProjectWorkspaceMutation(app, receipt.workspace, {
        preferredRelativePath: preferredPath,
        warningLabel: t("taxonomies-operation-label"),
      });
      loadedKey = currentCatalogKey();
      await loadCatalog();
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("taxonomies-operation-session-warning", { message: successMessage })
          : t("taxonomies-operation-session-success", { message: successMessage }),
        "unsaved",
      );
      return true;
    } catch (error) {
      const message = errorMessage(error);
      formError = message;
      app.setGlobalStatus(t("taxonomies-operation-failed", { message }), "error");
      return false;
    } finally {
      busy = false;
    }
  }

  function selectEntry(entry: TaxonomyCatalogEntry) {
    selectedId = entry.id;
    selectedTermId = null;
    detailMode = "info";
    deleteConfirmationOpen = false;
    formError = "";
  }

  function selectTerm(term: TaxonomyCatalogTerm) {
    selectedTermId = term.id;
    termDraft = term.name;
    detailMode = "info";
    deleteConfirmationOpen = false;
    formError = "";
  }

  function beginCreate() {
    detailMode = "create";
    selectedTermId = null;
    nameDraft = "";
    languageDraft = catalog?.defaultLanguage ?? "en";
    renderDraft = true;
    feedDraft = false;
    paginateByDraft = "";
    paginatePathDraft = "";
    formError = "";
  }

  function beginEdit(entry: TaxonomyCatalogEntry) {
    detailMode = "edit";
    selectedTermId = null;
    nameDraft = entry.name;
    languageDraft = entry.language;
    renderDraft = entry.render;
    feedDraft = entry.feed;
    paginateByDraft = entry.paginateBy === null ? "" : String(entry.paginateBy);
    paginatePathDraft = entry.paginatePath ?? "";
    formError = "";
  }

  function resetDetail() {
    detailMode = "info";
    deleteConfirmationOpen = false;
    formError = "";
  }

  function definitionDraft(): TaxonomyDefinitionInput {
    const parsedPaginateBy = Number.parseInt(paginateByDraft.trim(), 10);
    return {
      name: nameDraft.trim(),
      language: languageDraft.trim(),
      render: renderDraft,
      feed: feedDraft,
      paginateBy: Number.isFinite(parsedPaginateBy) && parsedPaginateBy > 0
        ? parsedPaginateBy
        : null,
      paginatePath: paginatePathDraft.trim() || null,
    };
  }

  async function submitDefinition(event: SubmitEvent) {
    event.preventDefault();
    const definition = definitionDraft();
    if (!definition.name || !definition.language) {
      formError = t("taxonomies-required-error");
      return;
    }
    const original = detailMode === "edit" ? selected : null;
    const input: TaxonomyMutationInput = {
      operation: {
        kind: "upsert_definition",
        originalName: original?.name ?? null,
        originalLanguage: original?.language ?? null,
        definition,
      },
    };
    if (await executeMutation(
      input,
      original ? t("taxonomies-updated") : t("taxonomies-created"),
    )) {
      const next = catalog?.entries.find(
        (entry) => entry.name === definition.name && entry.language === definition.language,
      );
      selectedId = next?.id ?? selectedId;
      detailMode = "info";
    }
  }

  async function saveTaxonomyRoot() {
    if (!catalog || taxonomyRootDraft.trim() === (catalog.taxonomyRoot ?? "")) return;
    await executeMutation(
      {
        operation: {
          kind: "set_taxonomy_root",
          taxonomyRoot: taxonomyRootDraft.trim() || null,
        },
      },
      t("taxonomies-root-updated"),
    );
  }

  async function renameSelectedTerm(entry: TaxonomyCatalogEntry, term: TaxonomyCatalogTerm) {
    const nextTerm = termDraft.trim();
    if (!nextTerm || nextTerm === term.name) return;
    if (await executeMutation(
      {
        operation: {
          kind: "rename_term",
          taxonomyName: entry.name,
          language: entry.language,
          oldTerm: term.name,
          newTerm: nextTerm,
        },
      },
      t("taxonomies-term-renamed", { name: term.name }),
      term.pages[0]?.file ?? catalog?.configPath ?? null,
    )) {
      selectedTermId = catalog?.entries
        .find((candidate) => candidate.name === entry.name && candidate.language === entry.language)
        ?.terms.find((candidate) => candidate.name === nextTerm)?.id ?? null;
      termDraft = nextTerm;
    }
  }

  async function removeSelected(entry: TaxonomyCatalogEntry) {
    const removed = await executeMutation(
      {
        operation: {
          kind: "remove_definition",
          name: entry.name,
          language: entry.language,
          removeAssignments,
          expectedUsageCount: entry.pages.length,
        },
      },
      t("taxonomies-removed", { name: entry.name }),
    );
    if (removed) {
      deleteConfirmationOpen = false;
      removeAssignments = false;
      selectedId = catalog?.entries[0]?.id ?? null;
      selectedTermId = null;
    }
  }

  async function openTemplate(template: TaxonomyCatalogTemplate) {
    if (!template.file) return;
    await openWorkspaceSource(template.file);
    await app.setWorkbenchActivity("templates");
  }

  function templateOrigin(template: TaxonomyCatalogTemplate) {
    if (template.missing) return t("taxonomies-template-missing");
    if (template.origin === "theme") {
      return template.themeName
        ? t("taxonomies-template-theme-named", { name: template.themeName })
        : t("taxonomies-template-theme");
    }
    return t("taxonomies-template-local");
  }
</script>

<section class="activity-workspace taxonomies-workspace" aria-labelledby="taxonomies-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconTags size={15} stroke={1.9} /> {t("taxonomies-eyebrow")}</span>
      <h1 id="taxonomies-title">{t("taxonomies-title")}</h1>
      <p>{t("taxonomies-description")}</p>
    </div>
    <dl>
      <div><dt>{t("taxonomies-stat-definitions")}</dt><dd>{l10n.formatNumber(counts.definitions)}</dd></div>
      <div><dt>{t("taxonomies-stat-terms")}</dt><dd>{l10n.formatNumber(counts.terms)}</dd></div>
      <div><dt>{t("taxonomies-stat-pages")}</dt><dd>{l10n.formatNumber(counts.pages)}</dd></div>
      <div class:warning={counts.diagnostics > 0}><dt>{t("taxonomies-stat-problems")}</dt><dd>{l10n.formatNumber(counts.diagnostics)}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <label class="root-field">
      <span>{t("taxonomies-root-url")}</span>
      <input
        value={taxonomyRootDraft}
        oninput={(event) => { taxonomyRootDraft = event.currentTarget.value; }}
        placeholder={t("taxonomies-root-placeholder")}
        disabled={busy || !catalog}
      />
    </label>
    <button
      type="button"
      class="ui-button toolbar compact-action"
      disabled={busy || !catalog || taxonomyRootDraft.trim() === (catalog.taxonomyRoot ?? "")}
      onclick={() => { void saveTaxonomyRoot(); }}
    ><IconCheck size={14} /> {t("taxonomies-apply")}</button>
    <label class="search-field">
      <span class="sr-only">{t("taxonomies-search-label")}</span>
      <IconSearch size={14} stroke={1.9} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("taxonomies-search-placeholder")} />
    </label>
    <button class="ui-button primary toolbar toolbar-action" type="button" disabled={busy || !catalog} onclick={beginCreate}>
      <IconPlus size={14} stroke={2} /> {t("taxonomies-add")}
    </button>
  </div>

  <div class="workspace-body">
    <div class="taxonomy-list" role="listbox" aria-label={t("taxonomies-catalog-label")}>
      {#if loadError}
        <div class="workspace-state error" role="alert">
          <IconAlertTriangle size={22} />
          <strong>{t("taxonomies-load-error-title")}</strong>
          <span>{loadError}</span>
          <button type="button" onclick={() => { loadedKey = currentCatalogKey(); void loadCatalog(); }}>
            <IconRefresh size={14} /> {t("taxonomies-retry")}
          </button>
        </div>
      {:else if loading && !catalog}
        <div class="workspace-state">
          <span class="spin"><IconRefresh size={20} /></span>
          <strong>{t("taxonomies-loading")}</strong>
        </div>
      {:else if visibleEntries.length === 0 && (catalog?.entries.length ?? 0) === 0}
        <div class="workspace-state empty-catalog">
          <span class="state-icon"><IconTags size={28} stroke={1.5} /></span>
          <strong>{t("taxonomies-empty-title")}</strong>
          <span>{t("taxonomies-empty-description")}</span>
          <button class="ui-button primary" type="button" disabled={busy} onclick={beginCreate}>
            <IconPlus size={14} /> {t("taxonomies-add-first")}
          </button>
        </div>
      {:else}
        {#each visibleEntries as entry (entry.id)}
          <article
            class="taxonomy-card ui-entity-selectable"
            class:undeclared={!entry.declared}
            data-ui-selected={selected?.id === entry.id && !selectedTerm ? "true" : undefined}
          >
            <button
              class="taxonomy-row ui-entity-trigger"
              type="button"
              role="option"
              aria-selected={selected?.id === entry.id && !selectedTerm}
              onclick={() => selectEntry(entry)}
            >
              <span class="resource-icon"><IconTags size={17} stroke={1.8} /></span>
              <span class="card-copy">
                <strong>{entry.name}</strong>
                <small>{entry.path} · {entry.language}</small>
              </span>
              <span class="count">{t("taxonomies-terms-count", { count: entry.terms.length })}</span>
              <span class="status">{entry.declared ? t("taxonomies-declared") : t("taxonomies-undeclared")}</span>
            </button>
            {#if entry.terms.length > 0}
              <div class="term-list" aria-label={t("taxonomies-terms-label", { name: entry.name })}>
                {#each entry.terms as term (term.id)}
                  <button
                    type="button"
                    class="ui-entity-selectable"
                    data-ui-selected={selected?.id === entry.id && selectedTerm?.id === term.id ? "true" : undefined}
                    aria-pressed={selected?.id === entry.id && selectedTerm?.id === term.id}
                    onclick={() => { selectedId = entry.id; selectTerm(term); }}
                  >
                    <span>{term.name}</span>
                    <small>/{term.slug}</small>
                    <em>{term.pages.length}</em>
                  </button>
                {/each}
              </div>
            {/if}
          </article>
        {:else}
          <div class="workspace-state"><strong>{t("taxonomies-no-results")}</strong><span>{t("taxonomies-change-search")}</span></div>
        {/each}
      {/if}
    </div>

    <aside class="taxonomy-detail" aria-label={t("taxonomies-detail-label")}>
      {#if detailMode === "create" || detailMode === "edit" && selected}
        <form class="taxonomy-form" onsubmit={submitDefinition}>
          <div class="detail-heading">
            <div>
              <span class="detail-kicker">{detailMode === "create" ? t("taxonomies-new-definition") : t("taxonomies-semantic-edit")}</span>
              <h2>{detailMode === "create" ? t("taxonomies-add-title") : t("taxonomies-edit-title", { name: selected?.name ?? "" })}</h2>
              <p>{t("taxonomies-form-description")}</p>
            </div>
            <button class="ui-icon-button ui-close-button" type="button" aria-label={t("taxonomies-cancel")} disabled={busy} onclick={resetDetail}><IconX size={14} /></button>
          </div>
          <div class="form-fields">
            <label>
              <span>{t("taxonomies-name")}</span>
              <input bind:value={nameDraft} placeholder="tags" autocomplete="off" disabled={busy} />
            </label>
            <label>
              <span>{t("taxonomies-language")}</span>
              <input bind:value={languageDraft} placeholder={catalog?.defaultLanguage ?? "en"} autocomplete="off" disabled={busy} />
            </label>
            <div class="toggle-grid">
              <label><input type="checkbox" bind:checked={renderDraft} disabled={busy} /> {t("taxonomies-render-pages")}</label>
              <label><input type="checkbox" bind:checked={feedDraft} disabled={busy} /> {t("taxonomies-render-feed")}</label>
            </div>
            <div class="field-grid">
              <label>
                <span>{t("taxonomies-items-per-page")}</span>
                <input
                  type="number"
                  min="1"
                  value={paginateByDraft}
                  oninput={(event) => { paginateByDraft = event.currentTarget.value; }}
                  placeholder={t("taxonomies-no-pagination")}
                  disabled={busy}
                />
              </label>
              <label>
                <span>{t("taxonomies-pagination-path")}</span>
                <input bind:value={paginatePathDraft} placeholder="page" autocomplete="off" disabled={busy} />
              </label>
            </div>
          </div>
          {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
          <div class="form-actions">
            <button type="button" disabled={busy} onclick={resetDetail}>{t("taxonomies-cancel")}</button>
            <button class="ui-button primary" type="submit" disabled={busy || !nameDraft.trim() || !languageDraft.trim()}>
              <IconCheck size={14} /> {busy
                ? t("taxonomies-applying")
                : detailMode === "create"
                  ? t("taxonomies-create-session")
                  : t("taxonomies-apply-changes")}
            </button>
          </div>
        </form>
      {:else if selected && selectedTerm}
        <div class="detail-heading">
          <div>
            <span class="detail-kicker">{t("taxonomies-term-kicker", {
              taxonomy: selected.name,
              language: selected.language,
            })}</span>
            <h2>{selectedTerm.name}</h2>
            <a href={selectedTerm.permalink} onclick={(event) => { event.preventDefault(); void app.openCurrentProjectInBrowser(selectedTerm.path); }}>
              {selectedTerm.path} <IconExternalLink size={13} />
            </a>
          </div>
          <button class="ui-icon-button ui-close-button" type="button" aria-label={t("taxonomies-close-term")} onclick={() => { selectedTermId = null; }}>
            <IconX size={14} />
          </button>
        </div>
        <section class="rename-card">
          <label>
            <span>{t("taxonomies-rename-label")}</span>
            <input bind:value={termDraft} disabled={busy} />
          </label>
          <button
            type="button"
            disabled={busy || !termDraft.trim() || termDraft.trim() === selectedTerm.name}
            onclick={() => { void renameSelectedTerm(selected, selectedTerm); }}
          ><IconCheck size={14} /> {t("taxonomies-rename-atomic")}</button>
        </section>
        <dl class="contract-grid">
          <div><dt>{t("taxonomies-zola-slug")}</dt><dd>{selectedTerm.slug}</dd></div>
          <div><dt>{t("taxonomies-pages")}</dt><dd>{l10n.formatNumber(selectedTerm.pages.length)}</dd></div>
          <div class="wide"><dt>{t("taxonomies-route")}</dt><dd>{selectedTerm.path}</dd></div>
          {#if selectedTerm.aliases.length > 0}
            <div class="wide"><dt>{t("taxonomies-same-slug")}</dt><dd>{selectedTerm.aliases.join(", ")}</dd></div>
          {/if}
        </dl>
        <section class="relation-section">
          <h3>{t("taxonomies-associated-pages")}</h3>
          {#each selectedTerm.pages as page (page.file)}
            <button type="button" onclick={() => { void openWorkspaceSource(page.file); }}>
              <span><strong>{page.title}</strong><small>{page.file}</small></span>
              <em>{page.url}</em>
            </button>
          {:else}<p>{t("taxonomies-no-associated-pages")}</p>{/each}
        </section>
        {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      {:else if selected}
        <div class="detail-heading">
          <div>
            <span class="detail-kicker">{selected.declared
              ? t("taxonomies-definition")
              : t("taxonomies-undeclared-use")} · {selected.language}</span>
            <h2>{selected.name}</h2>
            <a href={selected.permalink} onclick={(event) => { event.preventDefault(); void app.openCurrentProjectInBrowser(selected.path); }}>
              {selected.path} <IconExternalLink size={13} />
            </a>
          </div>
          <button type="button" disabled={busy} onclick={() => beginEdit(selected)}>
            {selected.declared ? t("taxonomies-edit") : t("taxonomies-declare")}
          </button>
        </div>

        <dl class="contract-grid">
          <div><dt>{t("taxonomies-language")}</dt><dd><IconLanguage size={13} /> {selected.language}</dd></div>
          <div><dt>{t("taxonomies-slug")}</dt><dd>{selected.slug}</dd></div>
          <div><dt>{t("taxonomies-terms")}</dt><dd>{l10n.formatNumber(selected.terms.length)}</dd></div>
          <div><dt>{t("taxonomies-affected-pages")}</dt><dd>{l10n.formatNumber(selected.pages.length)}</dd></div>
          <div><dt>{t("taxonomies-rendering")}</dt><dd>{selected.render ? t("taxonomies-active-feminine") : t("taxonomies-disabled-feminine")}</dd></div>
          <div><dt>{t("taxonomies-feed")}</dt><dd>{selected.feed ? t("taxonomies-active-masculine") : t("taxonomies-disabled-masculine")}</dd></div>
          <div><dt>{t("taxonomies-pagination")}</dt><dd>{selected.paginateBy ?? "—"}</dd></div>
          <div><dt>{t("taxonomies-pagination-path")}</dt><dd>{selected.paginatePath ?? "—"}</dd></div>
        </dl>

        <section class="template-section">
          <h3>{t("taxonomies-effective-templates")}</h3>
          {#each [
            { label: t("taxonomies-list-template"), template: selected.listTemplate },
            { label: t("taxonomies-term-template"), template: selected.termTemplate },
          ] as item (item.label)}
            <div class:missing={item.template.missing}>
              <span class="template-icon"><IconFileCode size={15} /></span>
              <span><strong>{item.label}</strong><small>{item.template.file ?? item.template.logicalName}</small></span>
              <em>{templateOrigin(item.template)}{item.template.fallback ? ` · ${t("taxonomies-template-fallback")}` : ""}</em>
              <button
                type="button"
                disabled={!item.template.file}
                onclick={() => { void openTemplate(item.template); }}
              >{t("taxonomies-open-templates")}</button>
            </div>
          {/each}
        </section>

        <section class="relation-section">
          <h3>{t("taxonomies-affected-pages")}</h3>
          {#each selected.pages as page (page.file)}
            <button type="button" onclick={() => { void openWorkspaceSource(page.file); }}>
              <span><strong>{page.title}</strong><small>{page.file}</small></span>
              <em>{page.url}</em>
            </button>
          {:else}<p>{t("taxonomies-no-pages-use")}</p>{/each}
        </section>

        {#if selectedDiagnostics.length > 0}
          <section class="diagnostics-section">
            <h3>{t("taxonomies-rust-diagnostics")}</h3>
            {#each selectedDiagnostics as diagnostic (`${diagnostic.code}:${diagnostic.file}:${diagnostic.term}`)}
              <div class:error={diagnostic.severity === "error"}>
                <IconAlertTriangle size={14} />
                <span><strong>{diagnostic.code}</strong><small>{errorMessage(diagnostic.diagnostic)}</small></span>
              </div>
            {/each}
          </section>
        {/if}

        <div class="detail-actions">
          <button class="ui-button primary" type="button" disabled={busy} onclick={() => beginEdit(selected)}>
            <IconCheck size={14} /> {selected.declared
              ? t("taxonomies-edit-definition")
              : t("taxonomies-declare-taxonomy")}
          </button>
          {#if selected.declared}
            <button class="ui-button danger" type="button" disabled={busy} onclick={() => { deleteConfirmationOpen = true; }}>
              <IconTrash size={14} /> {t("taxonomies-remove")}
            </button>
          {/if}
        </div>

        {#if deleteConfirmationOpen && selected.declared}
          <section class="delete-confirmation" aria-label={t("taxonomies-delete-label")}>
            <strong>{t("taxonomies-delete-title", { name: selected.name })}</strong>
            <span>{t("taxonomies-delete-impact", {
              pageCount: selected.pages.length,
              termCount: selected.terms.length,
            })}</span>
            <label>
              <input type="checkbox" bind:checked={removeAssignments} disabled={busy} />
              {t("taxonomies-remove-assignments")}
            </label>
            <div>
              <button type="button" disabled={busy} onclick={() => { deleteConfirmationOpen = false; }}>{t("taxonomies-cancel")}</button>
              <button class="ui-button danger" type="button" disabled={busy} onclick={() => { void removeSelected(selected); }}>
                <IconTrash size={14} /> {t("taxonomies-confirm-impact")}
              </button>
            </div>
          </section>
        {/if}
        {#if formError}<p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>{/if}
      {:else}
        <div class="workspace-state">
          <IconRoute size={25} />
          <strong>{t("taxonomies-select-title")}</strong>
          <span>{t("taxonomies-select-description")}</span>
        </div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .compact-action, .taxonomy-row, .detail-heading, .detail-heading button, .relation-section button, .detail-actions, .detail-actions button, .form-error, .form-actions, .form-actions button, .rename-card, .rename-card button, .template-section > div, .diagnostics-section > div, .delete-confirmation > div, .delete-confirmation button, .workspace-state button { display: flex; align-items: center; }
  .workspace-header > dl div.warning { border-color: color-mix(in srgb, var(--danger) 38%, var(--wb-border-subtle)); }
  dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .root-field { display: flex; align-items: center; gap: 7px; }
  .root-field span { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; white-space: nowrap; }
  .root-field input { width: 160px; height: var(--control-height-toolbar); padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--material-inset); box-shadow: var(--shadow-inset); font-size: 12px; }
  .compact-action { justify-content: center; gap: 5px; min-height: 28px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .workspace-body { display: grid; grid-template-columns: minmax(390px, 1fr) minmax(340px, .72fr); min-width: 0; min-height: 0; }
  .taxonomy-list, .taxonomy-detail { min-width: 0; min-height: 0; overflow: auto; }
  .taxonomy-list { padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .taxonomy-card { margin-bottom: 5px; border: 1px solid transparent; border-radius: 7px; }
  .taxonomy-card.undeclared { border-color: color-mix(in srgb, var(--danger) 25%, transparent); }
  .taxonomy-row { width: 100%; min-height: 55px; gap: 9px; padding: 7px 9px; border: 0; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .resource-icon, .state-icon { display: grid; flex: 0 0 auto; width: 31px; height: 31px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .card-copy { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .card-copy strong { color: var(--text-strong); font-size: 12px; }
  .card-copy small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .count, .status { padding: 2px 5px; border-radius: 4px; color: var(--wb-text-muted); background: var(--surface-7); font-size: 11px; white-space: nowrap; }
  .taxonomy-card.undeclared .status { color: var(--danger); }
  .term-list { display: grid; grid-template-columns: repeat(auto-fill, minmax(155px, 1fr)); gap: 4px; padding: 0 8px 8px 48px; }
  .term-list button { --ui-entity-background: var(--wb-surface-document); --ui-entity-border-color: var(--wb-border-subtle); display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 5px; align-items: center; min-height: 28px; padding: 4px 6px; border: 1px solid var(--wb-border-subtle); border-radius: 5px; color: var(--wb-text-primary); background: var(--wb-surface-document); text-align: left; }
  .term-list span, .term-list small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .term-list span { font-size: 11px; font-weight: 650; }
  .term-list small, .term-list em { color: var(--wb-text-muted); font-size: 11px; font-style: normal; }
  .taxonomy-detail { padding: 17px; background: var(--wb-surface-chrome); }
  .detail-heading { align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading > div { min-width: 0; }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 750; text-transform: uppercase; }
  h2 { margin: 6px 0 2px; color: var(--text-strong); font-size: 19px; }
  .detail-heading p { max-width: 520px; margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .detail-heading a { display: inline-flex; align-items: center; gap: 4px; color: var(--wb-accent-strong); font-size: 11px; text-decoration: none; }
  .detail-heading button, .detail-actions button, .form-actions button, .rename-card button, .template-section button, .delete-confirmation button, .workspace-state button { justify-content: center; gap: 5px; min-height: 29px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .contract-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; margin: 14px 0; }
  .contract-grid div { min-width: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .contract-grid div.wide { grid-column: 1 / -1; }
  .contract-grid dd { display: flex; align-items: center; gap: 4px; overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .template-section, .relation-section, .diagnostics-section { margin-top: 14px; }
  h3 { margin: 0 0 5px; color: var(--text-strong); font-size: 11px; text-transform: uppercase; }
  .template-section > div { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; gap: 8px; min-height: 48px; padding: 6px 0; border-top: 1px solid var(--wb-border-subtle); }
  .template-section > div.missing { color: var(--danger); }
  .template-icon { display: grid; width: 28px; height: 28px; place-items: center; border-radius: 6px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .template-section > div > span:nth-child(2), .relation-section button > span, .diagnostics-section > div > span { display: grid; min-width: 0; }
  .template-section strong, .relation-section strong, .diagnostics-section strong { font-size: 11px; }
  .template-section small, .relation-section small, .diagnostics-section small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
  .template-section em, .relation-section em { color: var(--wb-text-muted); font-size: 11px; font-style: normal; white-space: nowrap; }
  .template-section button { min-height: 25px; font-size: 11px; }
  .relation-section button { width: 100%; justify-content: space-between; gap: 8px; padding: 6px 7px; border: 0; border-top: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .relation-section p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 11px; }
  .diagnostics-section > div { align-items: flex-start; gap: 7px; padding: 7px; border-top: 1px solid var(--wb-border-subtle); color: var(--wb-warning); }
  .diagnostics-section > div.error { color: var(--danger); }
  .diagnostics-section small { white-space: normal; }
  .detail-actions { gap: 6px; margin-top: 16px; }
  .detail-actions .primary, .form-actions .primary, .workspace-state .primary { color: #fff; border-color: var(--wb-accent); background: var(--wb-accent); }
  .detail-actions .danger { margin-left: auto; color: var(--danger); }
  .taxonomy-form { display: grid; align-content: start; gap: 16px; }
  .form-fields { display: grid; gap: 12px; }
  .form-fields label, .rename-card label { display: grid; gap: 5px; }
  .form-fields label > span, .rename-card label > span { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .form-fields input:not([type="checkbox"]), .rename-card input { width: 100%; min-height: 32px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .field-grid, .toggle-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .toggle-grid label { display: flex; align-items: center; gap: 6px; min-height: 32px; padding: 0 8px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 11px; }
  .form-error { align-items: flex-start; gap: 6px; margin: 0; padding: 8px 9px; border-left: 3px solid var(--danger); border-radius: var(--radius-control); color: var(--danger); background: var(--wb-surface-document); font-size: 12px; }
  .form-actions { justify-content: flex-end; gap: 6px; }
  .rename-card { align-items: end; gap: 8px; margin-top: 14px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .rename-card label { flex: 1; }
  .delete-confirmation { display: grid; gap: 8px; margin-top: 10px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 42%, var(--wb-border-subtle)); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .delete-confirmation > strong { color: var(--text-strong); font-size: 12px; }
  .delete-confirmation > span, .delete-confirmation label { color: var(--wb-text-muted); font-size: 11px; line-height: 1.45; }
  .delete-confirmation label { display: flex; align-items: center; gap: 6px; }
  .delete-confirmation > div { justify-content: flex-end; gap: 6px; }
  .delete-confirmation .danger { color: var(--danger); }
  .workspace-state { display: grid; min-height: 200px; place-items: center; align-content: center; gap: 8px; padding: 24px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state strong { color: var(--text-strong); font-size: 13px; }
  .workspace-state > span:not(.spin):not(.state-icon) { max-width: 430px; line-height: 1.5; }
  .workspace-state.error { color: var(--danger); }
  .spin { animation: spin 1s linear infinite; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { opacity: .45; }
  button:focus-visible, input:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 920px) { .workspace-body { grid-template-columns: 1fr; } .taxonomy-detail { display: none; } .taxonomy-list { border-right: 0; } .workspace-header > dl { display: none; } .root-field { display: none; } }
</style>
