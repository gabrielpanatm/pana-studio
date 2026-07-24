<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconDownload,
    IconPalette,
    IconSearch,
  } from "@tabler/icons-svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import {
    applyThemeChange,
    planThemeChange,
    readThemeCatalog,
  } from "$lib/project/io";
  import type {
    ProjectWorkspaceIdentity,
    ThemeCatalogSnapshot,
    ThemeOperation,
    ThemePackSnapshot,
    ThemePlan,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let { app }: { app: AppState } = $props();

  let catalog = $state<ThemeCatalogSnapshot | null>(null);
  let selectedId = $state("");
  let query = $state("");
  let pendingPlan = $state<ThemePlan | null>(null);
  let loading = $state(false);
  let applying = $state(false);
  let loadError = $state("");
  let loadedIdentityKey = "";

  const selectedTheme = $derived(
    catalog?.themes.find((theme) => theme.id === selectedId) ?? catalog?.themes[0] ?? null,
  );
  const visibleThemes = $derived(
    (catalog?.themes ?? []).filter((theme) => {
      const needle = query.trim().toLocaleLowerCase("ro");
      return !needle || `${theme.name} ${theme.description} ${theme.category}`
        .toLocaleLowerCase("ro")
        .includes(needle);
    }),
  );
  const installedCount = $derived(
    catalog?.themes.filter((theme) => theme.status !== "available").length ?? 0,
  );

  $effect(() => {
    const snapshot = app.projectWorkspaceSnapshot;
    const key = snapshot ? identityKey({
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    }) : "";
    if (!key || key === loadedIdentityKey) return;
    loadedIdentityKey = key;
    void loadCatalog();
  });

  function identity(): ProjectWorkspaceIdentity | null {
    const snapshot = app.projectWorkspaceSnapshot;
    if (!snapshot) return null;
    return {
      expectedProjectRoot: snapshot.projectRoot,
      expectedSessionId: snapshot.runtimeSessionId,
      expectedRevision: snapshot.revision,
    };
  }

  function identityKey(value: ProjectWorkspaceIdentity) {
    return `${value.expectedProjectRoot}:${value.expectedSessionId}:${value.expectedRevision}`;
  }

  async function loadCatalog(preferredId = selectedId) {
    const currentIdentity = identity();
    if (!currentIdentity) return;
    loadedIdentityKey = identityKey(currentIdentity);
    loading = true;
    loadError = "";
    try {
      const next = await readThemeCatalog(currentIdentity);
      catalog = next;
      selectedId = next.themes.some((theme) => theme.id === preferredId)
        ? preferredId
        : (next.activeThemeId ?? next.themes[0]?.id ?? "");
      pendingPlan = null;
    } catch (error) {
      loadError = errorMessage(error);
    } finally {
      loading = false;
    }
  }

  async function prepare(operation: ThemeOperation) {
    const currentIdentity = identity();
    if (!selectedTheme || !currentIdentity) return;
    loadError = "";
    try {
      pendingPlan = await planThemeChange({
        themeId: selectedTheme.id,
        operation,
        identity: currentIdentity,
      });
    } catch (error) {
      loadError = errorMessage(error);
    }
  }

  async function applyPlan() {
    const currentIdentity = identity();
    if (!pendingPlan || !currentIdentity || pendingPlan.blocking) return;
    applying = true;
    loadError = "";
    const themeId = pendingPlan.themeId;
    const operation = pendingPlan.operation;
    try {
      const receipt = await applyThemeChange(
        { themeId, operation, identity: currentIdentity },
        pendingPlan.planToken,
      );
      app.projectWorkspaceSnapshot = receipt.workspace;
      await app.rescanCurrentProject(null, { strict: true });
      app.setGlobalStatus(
        operation === "install"
          ? `Tema ${themeId} a fost instalată și rămâne inactivă.`
          : `Tema ${themeId} este activă; proiectul a fost revalidat.`,
        "unsaved",
      );
      loadedIdentityKey = "";
      await loadCatalog(themeId);
    } catch (error) {
      loadError = errorMessage(error);
    } finally {
      applying = false;
    }
  }

  function selectTheme(theme: ThemePackSnapshot) {
    selectedId = theme.id;
    pendingPlan = null;
  }

  function statusLabel(theme: ThemePackSnapshot) {
    if (theme.status === "active") {
      return theme.installComplete ? "Activă" : "Activă, incompletă";
    }
    if (theme.status === "installed") {
      return theme.installComplete ? "Instalată" : "Instalare incompletă";
    }
    return "Disponibilă";
  }
</script>

<section class="themes-workspace" aria-label="Teme Zola">
  <header class="workspace-header">
    <div>
      <p class="eyebrow"><IconPalette size={14} stroke={1.8} /> Teme Zola incluse</p>
      <h1>Teme</h1>
      <p class="subtitle">Instalează și activează pachetele Zola incluse în aplicație.</p>
    </div>
    <div class="metrics" aria-label="Rezumat catalog">
      <span><strong>{catalog?.themes.length ?? 0}</strong> disponibile</span>
      <span><strong>{installedCount}</strong> instalate</span>
      <span><strong>{catalog?.embeddedZolaVersion ?? "—"}</strong> Zola</span>
    </div>
  </header>

  <div class="catalog-toolbar">
    <span>Toate temele</span>
    <label class="search">
      <IconSearch size={15} stroke={1.7} />
      <input bind:value={query} type="search" placeholder="Caută teme" aria-label="Caută teme" />
    </label>
  </div>

  {#if loadError}
    <p class="error-message" role="alert">{loadError}</p>
  {/if}

  <div class="workspace-body">
    <div class="theme-list" aria-label="Catalog de teme" aria-busy={loading}>
      {#each visibleThemes as theme (theme.id)}
        <button
          type="button"
          class:selected={selectedTheme?.id === theme.id}
          class="theme-row"
          onclick={() => selectTheme(theme)}
        >
          <img src={theme.previewDataUrl} alt="" />
          <span class="theme-row-copy">
            <strong>{theme.name}</strong>
            <small>{theme.description}</small>
          </span>
          <span class:active={theme.status === "active"} class="status-badge">
            {statusLabel(theme)}
          </span>
        </button>
      {:else}
        <p class="empty-list">{loading ? "Se încarcă catalogul..." : "Nicio temă nu corespunde căutării."}</p>
      {/each}
    </div>

    <aside class="theme-detail" aria-live="polite">
      {#if selectedTheme}
        <img class="theme-preview" src={selectedTheme.previewDataUrl} alt={`Previzualizare ${selectedTheme.name}`} />
        <div class="detail-title">
          <div>
            <p class="eyebrow">{selectedTheme.category} · v{selectedTheme.version}</p>
            <h2>{selectedTheme.name}</h2>
          </div>
          <span class:active={selectedTheme.status === "active"} class="status-badge">
            {statusLabel(selectedTheme)}
          </span>
        </div>
        <p class="detail-description">{selectedTheme.description}</p>

        <dl class="theme-facts">
          <div><dt>Compatibilitate</dt><dd>Zola {selectedTheme.compatibility.minimum}–{selectedTheme.compatibility.tested}</dd></div>
          <div><dt>Fișiere temă</dt><dd>{selectedTheme.themeFileCount}</dd></div>
          <div><dt>Rețetă proiect</dt><dd>{selectedTheme.recipeFileCount} fișiere</dd></div>
          <div><dt>Override-uri locale</dt><dd>{selectedTheme.localOverrideCount}</dd></div>
        </dl>

        {#if pendingPlan}
          <section class:blocking={pendingPlan.blocking} class="impact-panel">
            <h3>
              {#if pendingPlan.blocking}
                <IconAlertTriangle size={16} stroke={1.8} />
              {:else}
                <IconCheck size={16} stroke={1.8} />
              {/if}
              Impact {pendingPlan.operation === "install" ? "instalare" : "activare"}
            </h3>
            <p>{pendingPlan.affectedFiles.length} fișiere afectate; {pendingPlan.localOverrides.length} override-uri locale.</p>
            {#each [...pendingPlan.conflicts, ...pendingPlan.missingRequirements, ...pendingPlan.localOverrides, ...pendingPlan.notices] as item}
              <p class:blocking={item.blocking} class="impact-item">{item.message}</p>
            {/each}
            <div class="detail-actions">
              <button type="button" class="secondary-action" onclick={() => { pendingPlan = null; }}>
                Renunță
              </button>
              <button
                type="button"
                class="primary-action"
                disabled={pendingPlan.blocking || !pendingPlan.changed || applying}
                onclick={applyPlan}
              >
                {applying ? "Se aplică..." : "Confirmă"}
              </button>
            </div>
          </section>
        {:else}
          <div class="detail-actions">
            {#if selectedTheme.status === "available" || !selectedTheme.installComplete}
              <button type="button" class="primary-action" onclick={() => prepare("install")}>
                <IconDownload size={15} stroke={1.8} /> Verifică instalarea
              </button>
            {:else if selectedTheme.status !== "active"}
              <button type="button" class="primary-action" onclick={() => prepare("activate")}>
                Verifică impactul și activează
              </button>
            {:else}
              <button type="button" class="primary-action" onclick={() => app.setWorkbenchActivity("design_system")}>
                Deschide în Sistem de design
              </button>
            {/if}
          </div>
        {/if}
      {:else}
        <p class="empty-list">Selectează o temă din catalog.</p>
      {/if}
    </aside>
  </div>
</section>

<style>
  .themes-workspace {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    width: 100%;
    height: 100%;
    min-height: 0;
    color: var(--text);
    background: var(--surface-6);
  }

  .workspace-header,
  .catalog-toolbar,
  .workspace-body {
    border: 1px solid var(--border);
  }

  .workspace-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 110px;
    padding: 18px 20px;
    background: var(--surface-5);
  }

  h1,
  h2,
  h3,
  p {
    margin: 0;
  }

  h1 {
    margin-top: 6px;
    font-size: 20px;
  }

  h2 {
    margin-top: 4px;
    font-size: 20px;
  }

  h3 {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .eyebrow {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--brand-strong);
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .subtitle,
  .detail-description {
    margin-top: 6px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5;
  }

  .metrics {
    display: flex;
    gap: 8px;
  }

  .metrics span {
    display: grid;
    min-width: 82px;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 11px;
    text-transform: uppercase;
    background: var(--surface-3);
  }

  .metrics strong {
    color: var(--text);
    font-size: 15px;
  }

  .catalog-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 44px;
    padding: 5px 10px;
    border-top: 0;
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 700;
    background: var(--surface-4);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    width: min(280px, 40%);
    height: 32px;
    padding: 0 9px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    background: var(--surface-1);
  }

  .search input {
    min-width: 0;
    flex: 1;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
  }

  .workspace-body {
    display: grid;
    grid-template-columns: minmax(390px, 58%) minmax(300px, 42%);
    min-height: 0;
    border-top: 0;
  }

  .theme-list,
  .theme-detail {
    min-height: 0;
    overflow: auto;
  }

  .theme-list {
    padding: 10px;
  }

  .theme-detail {
    padding: 16px;
    border-left: 1px solid var(--border);
    background: var(--surface-4);
  }

  .theme-row {
    display: grid;
    grid-template-columns: 126px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    width: 100%;
    margin-bottom: 8px;
    padding: 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-panel);
    color: inherit;
    text-align: left;
    background: transparent;
  }

  .theme-row:hover,
  .theme-row.selected {
    border-color: var(--border-3);
    background: var(--control-selected);
  }

  .theme-row.selected {
    box-shadow: inset 3px 0 0 var(--brand);
  }

  .theme-row img {
    width: 126px;
    aspect-ratio: 16 / 10;
    border: 1px solid var(--border);
    border-radius: 6px;
    object-fit: cover;
    background: var(--surface-2);
  }

  .theme-row-copy {
    display: grid;
    min-width: 0;
    gap: 4px;
  }

  .theme-row-copy small {
    overflow: hidden;
    color: var(--text-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-badge {
    padding: 4px 7px;
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 11px;
    background: var(--surface-2);
  }

  .status-badge.active {
    color: var(--brand-strong);
    background: var(--control-selected);
  }

  .theme-preview {
    width: 100%;
    aspect-ratio: 16 / 10;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    object-fit: cover;
    background: var(--surface-2);
  }

  .detail-title {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 12px;
    margin-top: 14px;
  }

  .theme-facts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 7px;
    margin: 14px 0;
  }

  .theme-facts div {
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    background: var(--surface-3);
  }

  .theme-facts dt {
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
  }

  .theme-facts dd {
    margin: 4px 0 0;
    font-size: 12px;
    font-weight: 700;
  }

  .impact-panel {
    padding: 12px;
    border: 1px solid var(--border-3);
    border-radius: var(--radius-panel);
    background: var(--surface-3);
  }

  .impact-panel.blocking {
    border-color: var(--danger);
  }

  .impact-panel > p {
    margin-top: 7px;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.45;
  }

  .impact-item {
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }

  .impact-item.blocking {
    color: var(--danger);
  }

  .detail-actions {
    display: flex;
    justify-content: flex-end;
    gap: 7px;
    margin-top: 14px;
  }

  .primary-action,
  .secondary-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 13px;
    border-radius: var(--radius-control);
    font-size: 12px;
    font-weight: 700;
  }

  .primary-action {
    border: 1px solid var(--brand);
    color: white;
    background: var(--brand);
  }

  .secondary-action {
    border: 1px solid var(--border-3);
    color: var(--text);
    background: var(--surface-2);
  }

  .primary-action:disabled {
    opacity: 0.45;
  }

  .error-message {
    padding: 8px 12px;
    color: var(--danger);
    font-size: 12px;
    background: var(--surface-4);
  }

  .empty-list {
    padding: 24px;
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
  }

  @media (max-width: 1040px) {
    .workspace-body {
      grid-template-columns: minmax(320px, 52%) minmax(280px, 48%);
    }

    .metrics span:nth-child(2) {
      display: none;
    }
  }
</style>
