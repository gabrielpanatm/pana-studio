<script lang="ts">
  import {
    IconAlertTriangle,
    IconCopy,
    IconDeviceFloppy,
    IconEdit,
    IconExternalLink,
    IconFileCode,
    IconLayout,
    IconPlus,
    IconRefresh,
    IconSearch,
    IconTemplate,
    IconTrash,
    IconX,
  } from "@tabler/icons-svelte";
  import {
    createTemplate,
    deleteTemplate,
    duplicateTemplate,
    overrideThemeTemplate,
    readTemplateCatalog,
    renameTemplate,
  } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    FileBufferRequestIdentity,
    TemplateCatalogEntry,
    TemplateCatalogRole,
    TemplateCatalogSnapshot,
    TemplateDraftRole,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type CatalogView = "all" | TemplateCatalogRole;
  type DetailMode = "info" | "create" | "edit";

  const views: { id: CatalogView; label: string }[] = [
    { id: "all", label: "Toate" },
    { id: "page", label: "Pagini" },
    { id: "layout", label: "Layout-uri" },
    { id: "partial", label: "Fragmente" },
    { id: "macro_library", label: "Macro-uri" },
  ];
  const draftViews = views.filter(
    (view): view is { id: TemplateDraftRole; label: string } => view.id !== "all",
  );

  let activeView = $state<CatalogView>("all");
  let query = $state("");
  let catalog = $state<TemplateCatalogSnapshot | null>(null);
  let selectedId = $state<string | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let loadError = $state("");
  let loadedKey = $state("");
  let detailMode = $state<DetailMode>("info");
  let draftName = $state("");
  let draftRole = $state<TemplateDraftRole>("page");
  let duplicateSourcePath = $state<string | null>(null);
  let formError = $state("");
  let deleteConfirmationOpen = $state(false);
  let draftNameInput = $state<HTMLInputElement | null>(null);

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const visibleEntries = $derived(
    (catalog?.entries ?? []).filter((entry) => (
      entry.effective
      && (activeView === "all" || entry.roles.includes(activeView))
      && (
        !normalizedQuery
        || `${entry.name} ${entry.file} ${entry.roles.join(" ")}`
          .toLocaleLowerCase("ro")
          .includes(normalizedQuery)
      )
    )),
  );
  const selected = $derived(
    visibleEntries.find((entry) => entry.id === selectedId)
      ?? visibleEntries[0]
      ?? null,
  );
  const draftPath = $derived(templateDraftPath(draftName));
  const counts = $derived({
    total: (catalog?.entries ?? []).filter((entry) => entry.effective).length,
    local: (catalog?.entries ?? []).filter((entry) => entry.effective && entry.editable).length,
    theme: (catalog?.entries ?? []).filter((entry) => entry.effective && !entry.editable).length,
    impacted: (catalog?.entries ?? []).reduce(
      (total, entry) => total + entry.affectedPages.length,
      0,
    ),
  });

  $effect(() => {
    const root = app.sessionProjectRoot.trim();
    const sessionId = app.kernelProjectSessionId.trim();
    const revision = app.projectWorkspaceSnapshot?.revision ?? 0;
    const key = `${root}:${sessionId}:${revision}`;
    if (!root || !sessionId || loading || loadedKey === key) return;
    loadedKey = key;
    void loadCatalog(root, sessionId);
  });

  async function loadCatalog(root = app.sessionProjectRoot, sessionId = app.kernelProjectSessionId) {
    loading = true;
    loadError = "";
    try {
      const snapshot = await readTemplateCatalog({
        expectedProjectRoot: root,
        expectedSessionId: sessionId,
      });
      if (root !== app.sessionProjectRoot || sessionId !== app.kernelProjectSessionId) return;
      catalog = snapshot;
      if (!snapshot.entries.some((entry) => entry.id === selectedId && entry.effective)) {
        selectedId = snapshot.entries.find((entry) => entry.effective)?.id ?? null;
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

  async function finishMutation(
    operation: () => Promise<{ relativePath: string | null }>,
    successMessage: string,
  ): Promise<boolean> {
    if (busy) return false;
    busy = true;
    formError = "";
    try {
      const receipt = await operation();
      await app.rescanCurrentProject(receipt.relativePath, { strict: true });
      loadedKey = "";
      await loadCatalog();
      const next = catalog?.entries.find((entry) => entry.file === receipt.relativePath);
      if (next) selectedId = next.id;
      app.setGlobalStatus(`${successMessage} — Ctrl+S persistă pe disc`, "unsaved");
      return true;
    } catch (error) {
      const message = errorMessage(error);
      formError = message;
      app.setGlobalStatus(`Operația pe șablon a eșuat: ${message}`, "error");
      return false;
    } finally {
      busy = false;
    }
  }

  function suggestedName(role: TemplateDraftRole) {
    return role === "partial"
      ? "partials/fragment-nou"
      : role === "macro_library"
        ? "macros/utilitare"
        : role === "layout"
          ? "layout-nou"
          : "pagina-noua";
  }

  function templateDraftPath(name: string) {
    const logical = name.trim().replaceAll("\\", "/").replace(/^templates\//, "");
    if (!logical) return "templates/…";
    return `templates/${logical.endsWith(".html") ? logical : `${logical}.html`}`;
  }

  function localLogicalName(entry: TemplateCatalogEntry) {
    return entry.file.replace(/^templates\//, "").replace(/\.html$/i, "");
  }

  function focusDraftName() {
    requestAnimationFrame(() => {
      draftNameInput?.focus();
      draftNameInput?.select();
    });
  }

  function resetPanelState() {
    detailMode = "info";
    draftName = "";
    duplicateSourcePath = null;
    formError = "";
    deleteConfirmationOpen = false;
  }

  function beginCreate() {
    draftRole = activeView === "all" ? "page" : activeView;
    draftName = suggestedName(draftRole);
    duplicateSourcePath = null;
    formError = "";
    deleteConfirmationOpen = false;
    detailMode = "create";
    focusDraftName();
  }

  function beginDuplicate(entry: TemplateCatalogEntry) {
    const stem = entry.name.replace(/\.html$/i, "");
    draftRole = entry.roles[0] ?? "page";
    draftName = `${stem}-copie`;
    duplicateSourcePath = entry.file;
    formError = "";
    deleteConfirmationOpen = false;
    detailMode = "create";
    focusDraftName();
  }

  function beginEdit(entry: TemplateCatalogEntry) {
    if (!entry.editable) return;
    draftName = localLogicalName(entry);
    draftRole = entry.roles[0] ?? "page";
    duplicateSourcePath = null;
    formError = "";
    deleteConfirmationOpen = false;
    detailMode = "edit";
    focusDraftName();
  }

  async function submitCreate(event: SubmitEvent) {
    event.preventDefault();
    const name = draftName.trim();
    if (!name) {
      formError = "Numele șablonului este obligatoriu.";
      return;
    }
    const created = duplicateSourcePath
      ? await finishMutation(
          () => duplicateTemplate({
            sourceRelativePath: duplicateSourcePath ?? "",
            destinationName: name,
          }, identity()),
          `Șablon duplicat: ${name}`,
        )
      : await finishMutation(
          () => createTemplate({ name, role: draftRole }, identity()),
          `Șablon creat: ${name}`,
        );
    if (created) resetPanelState();
  }

  async function submitEdit(event: SubmitEvent, entry: TemplateCatalogEntry) {
    event.preventDefault();
    const destinationName = draftName.trim();
    if (!destinationName) {
      formError = "Numele șablonului este obligatoriu.";
      return;
    }
    if (templateDraftPath(destinationName) === entry.file) {
      resetPanelState();
      return;
    }
    const renamed = await finishMutation(
      () => renameTemplate({
        sourceRelativePath: entry.file,
        destinationName,
      }, identity()),
      `Șablon redenumit: ${destinationName}`,
    );
    if (renamed) resetPanelState();
  }

  async function overrideSelected(entry: TemplateCatalogEntry) {
    const overridden = await finishMutation(
      () => overrideThemeTemplate({ sourceRelativePath: entry.file }, identity()),
      `Suprascriere locală creată: ${entry.localOverridePath}`,
    );
    if (overridden) resetPanelState();
  }

  async function removeSelected(entry: TemplateCatalogEntry) {
    if (!entry.canDelete) return;
    const removed = await finishMutation(
      () => deleteTemplate({ relativePath: entry.file }, identity()),
      `Șablon șters: ${entry.name}`,
    );
    if (removed) resetPanelState();
  }

  async function openInEditor(entry: TemplateCatalogEntry) {
    await openPathInEditor(entry.file);
  }

  async function openPathInEditor(path: string) {
    await openWorkspaceSource(path);
    await app.setWorkbenchActivity("editor");
  }

  function roleLabel(role: TemplateCatalogRole) {
    return views.find((view) => view.id === role)?.label ?? role;
  }

  function selectEntry(id: string) {
    selectedId = id;
    resetPanelState();
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + views.length) % views.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % views.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = views.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    activeView = views[nextIndex]?.id ?? "all";
    requestAnimationFrame(() => document.getElementById(`templates-tab-${nextIndex}`)?.focus());
  }
</script>

<section class="templates-workspace" aria-labelledby="templates-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconTemplate size={15} stroke={1.9} /> Catalog semantic Rust</span>
      <h1 id="templates-title">Șabloane</h1>
      <p>Structura Tera, dependențele și impactul asupra paginilor, fără un al doilea editor.</p>
    </div>
    <dl>
      <div><dt>Active</dt><dd>{counts.total}</dd></div>
      <div><dt>Locale</dt><dd>{counts.local}</dd></div>
      <div><dt>Temă</dt><dd>{counts.theme}</dd></div>
      <div><dt>Impact</dt><dd>{counts.impacted}</dd></div>
    </dl>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label="Roluri șabloane">
      {#each views as view, index (view.id)}
        <button
          id={`templates-tab-${index}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id}
          tabindex={activeView === view.id ? 0 : -1}
          class:active={activeView === view.id}
          onclick={() => { activeView = view.id; }}
          onkeydown={(event) => handleViewKeydown(event, index)}
        >{view.label}</button>
      {/each}
    </div>
    <label class="search-field">
      <span class="sr-only">Caută șabloane</span>
      <IconSearch size={14} stroke={1.9} />
      <input bind:value={query} type="search" placeholder="Caută șabloane" />
    </label>
    <button class="toolbar-action" type="button" disabled={busy} onclick={beginCreate}>
      <IconPlus size={14} stroke={2} /> Adaugă șablon
    </button>
  </div>

  <div class="workspace-body">
    <div class="template-list" role="listbox" aria-label="Catalog șabloane">
      {#if loadError}
        <div class="workspace-state error" role="alert">{loadError}</div>
      {:else if loading && !catalog}
        <div class="workspace-state">
          <span class="spin"><IconRefresh size={18} /></span>
          Se proiectează catalogul Rust…
        </div>
      {:else}
        {#each visibleEntries as entry (entry.id)}
          <button
            class:selected={selected?.id === entry.id}
            class="template-card"
            type="button"
            role="option"
            aria-selected={selected?.id === entry.id}
            onclick={() => selectEntry(entry.id)}
          >
            <span class="resource-icon">
              {#if entry.roles.includes("layout")}<IconLayout size={17} stroke={1.8} />{:else}<IconFileCode size={17} stroke={1.8} />{/if}
            </span>
            <span class="card-copy"><strong>{entry.name}</strong><small>{entry.file}</small></span>
            <span class="role-list">
              {#each entry.roles as role}<em>{roleLabel(role)}</em>{/each}
            </span>
            <span class:theme={!entry.editable} class="origin">{entry.editable ? "Local" : entry.themeName ?? "Temă"}</span>
          </button>
        {:else}
          <div class="workspace-state">Nu există șabloane pentru filtrul curent.</div>
        {/each}
      {/if}
    </div>

    <aside class="template-detail" aria-label="Panou contextual șablon">
      {#if detailMode === "create"}
        <form class="template-form" onsubmit={submitCreate}>
          <div class="detail-heading">
            <div>
              <span class="detail-kicker">{duplicateSourcePath ? "Creează dintr-un șablon existent" : "Șablon Tera nou"}</span>
              <h2>{duplicateSourcePath ? "Duplică șablonul" : "Adaugă șablon"}</h2>
              <p>{duplicateSourcePath ? "Conținutul sursei este copiat într-un șablon local nou." : "Fișierul este pregătit în sesiunea Rust și ajunge pe disc la Ctrl+S."}</p>
            </div>
            <button type="button" aria-label="Renunță la creare" disabled={busy} onclick={resetPanelState}>
              <IconX size={14} />
            </button>
          </div>

          <div class="form-fields">
            <label>
              <span>Nume logic</span>
              <input
                bind:this={draftNameInput}
                bind:value={draftName}
                type="text"
                autocomplete="off"
                placeholder="partials/card"
                disabled={busy}
                aria-describedby="template-path-preview"
              />
              <small id="template-path-preview">Cale rezultată: <code>{draftPath}</code></small>
            </label>
            <label>
              <span>Rol</span>
              <select bind:value={draftRole} disabled={busy || duplicateSourcePath !== null}>
                {#each draftViews as view (view.id)}
                  <option value={view.id}>{view.label}</option>
                {/each}
              </select>
              <small>{duplicateSourcePath ? "Rolul este preluat din șablonul sursă." : "Rolul stabilește conținutul inițial generat de nucleul Rust."}</small>
            </label>
            {#if duplicateSourcePath}
              <div class="source-summary">
                <span>Sursă copiată</span>
                <code>{duplicateSourcePath}</code>
              </div>
            {/if}
          </div>

          {#if formError}
            <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>
          {/if}

          <div class="form-actions">
            <button type="button" disabled={busy} onclick={resetPanelState}>Renunță</button>
            <button class="primary" type="submit" disabled={busy || !draftName.trim()}>
              <IconPlus size={14} /> {busy ? "Se creează prin Rust…" : duplicateSourcePath ? "Creează copia" : "Creează în sesiune"}
            </button>
          </div>
        </form>
      {:else if detailMode === "edit" && selected?.editable}
        <form class="template-form" onsubmit={(event) => submitEdit(event, selected)}>
          <div class="detail-heading">
            <div>
              <span class="detail-kicker">Modifică informațiile șablonului</span>
              <h2>Editează {selected.name}</h2>
              <p>Redenumirea actualizează atomic referințele Tera prin nucleul Rust.</p>
            </div>
            <button type="button" aria-label="Renunță la editare" disabled={busy} onclick={resetPanelState}>
              <IconX size={14} />
            </button>
          </div>

          <div class="form-fields">
            <label>
              <span>Nume logic</span>
              <input
                bind:this={draftNameInput}
                bind:value={draftName}
                type="text"
                autocomplete="off"
                disabled={busy}
                aria-describedby="template-edit-path-preview"
              />
              <small id="template-edit-path-preview">Cale rezultată: <code>{draftPath}</code></small>
            </label>
            <div class="source-summary">
              <span>Rol detectat</span>
              <strong>{selected.roles.map(roleLabel).join(", ")}</strong>
            </div>
          </div>

          <section class="code-note">
            <div>
              <strong>Conținutul Tera rămâne în editorul existent</strong>
              <span>Panoul contextual modifică identitatea resursei, fără să dubleze editorul de cod.</span>
            </div>
            <button type="button" onclick={() => { void openInEditor(selected); }}>
              <IconExternalLink size={14} /> Deschide în Editor
            </button>
          </section>

          {#if formError}
            <p class="form-error" role="alert"><IconAlertTriangle size={14} /> {formError}</p>
          {/if}

          <div class="form-actions">
            <button type="button" disabled={busy} onclick={resetPanelState}>Renunță</button>
            <button class="primary" type="submit" disabled={busy || !draftName.trim()}>
              <IconDeviceFloppy size={14} /> {busy ? "Se actualizează prin Rust…" : "Salvează modificările"}
            </button>
          </div>
        </form>
      {:else if selected}
        <div class="detail-heading">
          <div>
            <span class="detail-kicker">{selected.editable ? "Sursă locală editabilă" : "Sursă read-only din temă"}</span>
            <h2>{selected.name}</h2>
            <code>{selected.file}</code>
          </div>
          <button type="button" onclick={() => { void openInEditor(selected); }}>
            <IconExternalLink size={14} /> Deschide în Editor
          </button>
        </div>

        <dl class="contract-grid">
          <div><dt>Extinde</dt><dd>{selected.extends ?? "—"}</dd></div>
          <div><dt>Blocuri</dt><dd>{selected.blocks.length}</dd></div>
          <div><dt>Macro-uri</dt><dd>{selected.macros.length}</dd></div>
          <div><dt>Pagini afectate</dt><dd>{selected.affectedPages.length}</dd></div>
        </dl>

        <section class="relation-section">
          <h3>Folosit de șabloane</h3>
          {#each selected.usedByTemplates as usage}
            <button type="button" onclick={() => { void openPathInEditor(usage.file); }}>
              <span><strong>{usage.name}</strong><small>{usage.file}</small></span>
              <em>{usage.kind}</em>
            </button>
          {:else}<p>Nicio referință Tera directă.</p>{/each}
        </section>

        <section class="relation-section">
          <h3>Pagini afectate</h3>
          {#each selected.affectedPages as page}
            <button type="button" onclick={() => { void openPathInEditor(page.file); }}>
              <span><strong>{page.title}</strong><small>{page.file}</small></span>
              <em>{page.url}</em>
            </button>
          {:else}<p>Nicio pagină proiectată prin acest șablon.</p>{/each}
        </section>

        <div class="detail-actions">
          {#if selected.editable}
            <button class="primary" type="button" disabled={busy} onclick={() => beginEdit(selected)}>
              <IconEdit size={14} /> Editează
            </button>
            <button type="button" disabled={busy} onclick={() => beginDuplicate(selected)}>
              <IconCopy size={14} /> Duplică
            </button>
            <button
              class="danger"
              type="button"
              disabled={busy || !selected.canDelete}
              title={selected.deleteBlockedReason ?? "Șterge șablonul"}
              onclick={() => { deleteConfirmationOpen = true; }}
            ><IconTrash size={14} /> Șterge</button>
          {:else}
            <button class="primary" type="button" disabled={busy} onclick={() => { void overrideSelected(selected); }}>
              <IconCopy size={14} /> Creează suprascriere locală
            </button>
            <button type="button" disabled={busy} onclick={() => beginDuplicate(selected)}>
              <IconCopy size={14} /> Duplică local
            </button>
          {/if}
        </div>
        {#if deleteConfirmationOpen && selected.editable}
          <section class="delete-confirmation" aria-label="Confirmare ștergere">
            <div>
              <strong>Ștergi {selected.name}?</strong>
              <span>Operația intră în istoricul sesiunii și poate fi anulată înainte sau după salvare.</span>
            </div>
            <div>
              <button type="button" disabled={busy} onclick={() => { deleteConfirmationOpen = false; }}>Renunță</button>
              <button class="danger" type="button" disabled={busy} onclick={() => { void removeSelected(selected); }}>
                <IconTrash size={14} /> Confirmă ștergerea
              </button>
            </div>
          </section>
        {/if}
        {#if selected.deleteBlockedReason && selected.editable}
          <p class="guard-message">{selected.deleteBlockedReason}</p>
        {/if}
      {:else}
        <div class="workspace-state">Selectează un șablon pentru detalii.</div>
      {/if}
    </aside>
  </div>
</section>

<style>
  .templates-workspace { display: grid; grid-template-rows: auto 42px minmax(0, 1fr); min-width: 0; min-height: 0; height: 100%; overflow: hidden; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); color: var(--wb-text-primary); background: var(--wb-surface-document); }
  .workspace-header { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 17px 20px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .eyebrow { display: inline-flex; align-items: center; gap: 6px; color: var(--wb-accent-strong); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 20px; font-weight: 650; letter-spacing: -.015em; }
  .workspace-header p { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; }
  .workspace-header > dl { display: flex; gap: 7px; margin: 0; }
  .workspace-header > dl div { min-width: 70px; padding: 7px 9px; border: 1px solid var(--wb-border-subtle); border-radius: 7px; background: var(--wb-surface-document); }
  dt { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  dd { margin: 3px 0 0; color: var(--text-strong); font-size: 15px; font-weight: 650; }
  .workspace-toolbar, .view-tabs, .search-field, .toolbar-action, .template-card, .detail-heading, .detail-heading button, .relation-section button, .detail-actions, .detail-actions button, .form-error, .form-actions, .form-actions button, .code-note, .code-note button, .delete-confirmation > div:last-child, .delete-confirmation button { display: flex; align-items: center; }
  .workspace-toolbar { gap: 9px; padding: 5px 9px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .view-tabs { align-self: stretch; gap: 2px; }
  .view-tabs button { height: 100%; padding: 0 9px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted); background: transparent; font-size: 12px; font-weight: 600; }
  .view-tabs button.active { border-bottom-color: var(--wb-accent); color: var(--wb-accent-strong); }
  .search-field { position: relative; width: min(260px, 27vw); margin-left: auto; }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted); }
  .search-field input { width: 100%; height: 28px; padding: 0 8px 0 28px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .toolbar-action, .detail-heading button, .detail-actions button { justify-content: center; gap: 5px; min-height: 28px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .toolbar-action { color: #fff; border-color: var(--wb-accent); background: var(--wb-accent); }
  .workspace-body { display: grid; grid-template-columns: minmax(360px, 1fr) minmax(320px, .72fr); min-width: 0; min-height: 0; }
  .template-list, .template-detail { min-width: 0; min-height: 0; overflow: auto; }
  .template-list { padding: 9px; border-right: 1px solid var(--wb-border-subtle); }
  .template-card { width: 100%; gap: 9px; min-height: 58px; padding: 7px 9px; border: 1px solid transparent; border-radius: 7px; color: var(--wb-text-primary); background: transparent; text-align: left; }
  .template-card:hover, .template-card.selected { border-color: var(--wb-border-subtle); background: var(--wb-control-hover); }
  .template-card.selected { box-shadow: inset 3px 0 0 var(--wb-accent); }
  .resource-icon { display: grid; flex: 0 0 auto; width: 30px; height: 30px; place-items: center; border-radius: 7px; color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .card-copy { display: grid; flex: 1; gap: 3px; min-width: 0; }
  .card-copy strong { color: var(--text-strong); font-size: 12px; }
  .card-copy small, .relation-section small { overflow: hidden; color: var(--wb-text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .role-list { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 3px; }
  .role-list em, .origin { padding: 2px 5px; border-radius: 4px; color: var(--wb-text-muted); background: var(--surface-7); font-size: 11px; font-style: normal; white-space: nowrap; }
  .origin.theme { color: var(--wb-accent-strong); background: var(--wb-accent-soft); }
  .template-detail { padding: 17px; background: var(--wb-surface-chrome); }
  .detail-heading { align-items: flex-start; justify-content: space-between; gap: 12px; }
  .detail-heading > div { min-width: 0; }
  .detail-kicker { color: var(--wb-accent-strong); font-size: 11px; font-weight: 750; text-transform: uppercase; }
  h2 { margin: 6px 0 2px; color: var(--text-strong); font-size: 19px; }
  .detail-heading p { max-width: 540px; margin: 5px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .detail-heading code { color: var(--wb-text-muted); font-size: 11px; }
  .contract-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; margin: 14px 0; }
  .contract-grid div { min-width: 0; padding: 8px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; background: var(--wb-surface-document); }
  .contract-grid dd { overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .relation-section { margin-top: 13px; }
  h3 { margin: 0 0 5px; color: var(--text-strong); font-size: 11px; text-transform: uppercase; }
  .relation-section button { width: 100%; justify-content: space-between; gap: 8px; padding: 6px 7px; border: 0; border-top: 1px solid var(--wb-border-subtle); color: var(--wb-text-primary); background: transparent; text-align: left; }
  .relation-section button > span { display: grid; min-width: 0; }
  .relation-section button em { color: var(--wb-text-muted); font-size: 11px; font-style: normal; }
  .relation-section p, .guard-message { margin: 5px 0 0; color: var(--wb-text-muted); font-size: 11px; line-height: 1.45; }
  .detail-actions { gap: 6px; margin-top: 16px; }
  .detail-actions .primary { color: #fff; border-color: var(--wb-accent); background: var(--wb-accent); }
  .detail-actions .danger { margin-left: auto; color: var(--danger); }
  .template-form { display: grid; align-content: start; gap: 16px; }
  .form-fields { display: grid; gap: 12px; }
  .form-fields label { display: grid; gap: 5px; }
  .form-fields label > span, .source-summary > span { color: var(--wb-text-muted); font-size: 11px; font-weight: 650; text-transform: uppercase; }
  .form-fields input, .form-fields select { width: 100%; min-height: 32px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .form-fields small { color: var(--wb-text-muted); font-size: 11px; line-height: 1.45; }
  .form-fields small code { color: var(--wb-text-primary); }
  .source-summary { display: grid; gap: 5px; padding: 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .source-summary code, .source-summary strong { overflow: hidden; color: var(--wb-text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .form-error { gap: 6px; margin: 0; padding: 8px 9px; border-left: 3px solid var(--danger); border-radius: var(--radius-control); color: var(--danger); background: var(--wb-surface-document); font-size: 12px; }
  .form-actions { justify-content: flex-end; gap: 6px; padding-top: 2px; }
  .form-actions button, .code-note button, .delete-confirmation button { justify-content: center; gap: 5px; min-height: 29px; padding: 0 9px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); color: var(--wb-text-primary); background: var(--wb-surface-document); font-size: 12px; }
  .form-actions .primary { color: #fff; border-color: var(--wb-accent); background: var(--wb-accent); }
  .code-note { justify-content: space-between; gap: 12px; padding: 10px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .code-note > div { display: grid; gap: 3px; }
  .code-note strong { color: var(--text-strong); font-size: 12px; }
  .code-note span { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .delete-confirmation { display: grid; gap: 9px; margin-top: 10px; padding: 10px; border: 1px solid color-mix(in srgb, var(--danger) 42%, var(--wb-border-subtle)); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .delete-confirmation > div:first-child { display: grid; gap: 3px; }
  .delete-confirmation strong { color: var(--text-strong); font-size: 12px; }
  .delete-confirmation span { color: var(--wb-text-muted); font-size: 11px; line-height: 1.4; }
  .delete-confirmation > div:last-child { justify-content: flex-end; gap: 6px; }
  .delete-confirmation .danger { color: var(--danger); }
  .guard-message { padding: 7px; border: 1px solid var(--wb-border-subtle); border-radius: 6px; }
  .workspace-state { display: grid; min-height: 180px; place-items: center; gap: 7px; color: var(--wb-text-muted); font-size: 12px; text-align: center; }
  .workspace-state.error { color: var(--danger); }
  .spin { animation: spin 1s linear infinite; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { opacity: .45; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 900px) { .workspace-body { grid-template-columns: 1fr; } .template-detail { display: none; } .template-list { border-right: 0; } .workspace-header > dl { display: none; } }
</style>
