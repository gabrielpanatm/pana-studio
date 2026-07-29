<script lang="ts">
  import {
    IconAlertTriangle,
    IconCheck,
    IconDownload,
    IconPalette,
    IconSearch,
  } from "@tabler/icons-svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import {
    applyThemeChange,
    planThemeChange,
    readThemeCatalog,
  } from "$lib/project/io";
  import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
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
  let loadSequence = 0;

  const selectedTheme = $derived(
    catalog?.themes.find((theme) => theme.id === selectedId) ?? catalog?.themes[0] ?? null,
  );
  const visibleThemes = $derived(
    (catalog?.themes ?? []).filter((theme) => {
      const needle = query.trim().toLocaleLowerCase(l10n.locale);
      return !needle || `${theme.name} ${theme.description} ${theme.category}`
        .toLocaleLowerCase(l10n.locale)
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
    const requestId = ++loadSequence;
    loadedIdentityKey = identityKey(currentIdentity);
    loading = true;
    loadError = "";
    try {
      const next = await readThemeCatalog(currentIdentity);
      const latestIdentity = identity();
      if (
        requestId !== loadSequence
        || !latestIdentity
        || identityKey(latestIdentity) !== identityKey(currentIdentity)
      ) return;
      if (
        next.projectRoot !== currentIdentity.expectedProjectRoot
        || next.runtimeSessionId !== currentIdentity.expectedSessionId
        || next.revision !== currentIdentity.expectedRevision
      ) {
        throw new Error(t("themes-catalog-revision-mismatch"));
      }
      catalog = next;
      selectedId = next.themes.some((theme) => theme.id === preferredId)
        ? preferredId
        : (next.activeThemeId ?? next.themes[0]?.id ?? "");
      pendingPlan = null;
    } catch (error) {
      if (requestId !== loadSequence) return;
      loadError = errorMessage(error);
    } finally {
      if (requestId === loadSequence) loading = false;
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
      const settlement = await settleProjectWorkspaceMutation(app, {
        projectRoot: receipt.workspace.projectRoot,
        runtimeSessionId: receipt.workspace.runtimeSessionId,
        mutation: receipt.mutation,
        workspace: receipt.workspace,
      }, {
        preferredRelativePath: null,
        warningLabel: t("themes-operation-label"),
      });
      app.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("themes-status-resync", { theme: themeId })
          : operation === "install"
          ? t("themes-status-installed", { theme: themeId })
          : t("themes-status-activated", { theme: themeId }),
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
      return theme.installComplete ? t("themes-status-active") : t("themes-status-active-incomplete");
    }
    if (theme.status === "installed") {
      return theme.installComplete ? t("themes-status-installed-label") : t("themes-status-install-incomplete");
    }
    return t("themes-status-available");
  }
</script>

<section class="activity-workspace themes-workspace" aria-label={t("themes-label")}>
  <header class="workspace-header">
    <div>
      <p class="eyebrow"><IconPalette size={14} stroke={1.8} /> {t("themes-eyebrow")}</p>
      <h1>{t("themes-title")}</h1>
      <p class="subtitle">{t("themes-subtitle")}</p>
    </div>
    <dl aria-label={t("themes-summary")}>
      <div><dt>{t("themes-metric-available")}</dt><dd>{l10n.formatNumber(catalog?.themes.length ?? 0)}</dd></div>
      <div><dt>{t("themes-metric-installed")}</dt><dd>{l10n.formatNumber(installedCount)}</dd></div>
      <div><dt>Zola</dt><dd>{catalog?.embeddedZolaVersion ?? "—"}</dd></div>
    </dl>
  </header>

  <div class="catalog-toolbar workspace-toolbar">
    <span>{t("themes-all")}</span>
    <label class="search-field">
      <IconSearch size={15} stroke={1.7} />
      <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("themes-search")} aria-label={t("themes-search")} />
    </label>
  </div>

  {#if loadError}
    <p class="error-message" role="alert">{loadError}</p>
  {/if}

  <div class="workspace-body">
    <div class="theme-list" aria-label={t("themes-catalog-label")} aria-busy={loading}>
      {#each visibleThemes as theme (theme.id)}
        <button
          type="button"
          class="theme-row ui-entity-selectable"
          data-ui-selected={selectedTheme?.id === theme.id ? "true" : undefined}
          aria-pressed={selectedTheme?.id === theme.id}
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
        <p class="empty-list">{loading ? t("themes-loading") : t("themes-no-match")}</p>
      {/each}
    </div>

    <aside class="theme-detail" aria-live="polite">
      {#if selectedTheme}
        <img class="theme-preview" src={selectedTheme.previewDataUrl} alt={t("themes-preview-alt", { name: selectedTheme.name })} />
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
          <div><dt>{t("themes-compatibility")}</dt><dd>Zola {selectedTheme.compatibility.minimum}–{selectedTheme.compatibility.tested}</dd></div>
          <div><dt>{t("themes-theme-files")}</dt><dd>{l10n.formatNumber(selectedTheme.themeFileCount)}</dd></div>
          <div><dt>{t("themes-project-recipe")}</dt><dd>{t("themes-files-count", { count: selectedTheme.recipeFileCount })}</dd></div>
          <div><dt>{t("themes-local-overrides")}</dt><dd>{l10n.formatNumber(selectedTheme.localOverrideCount)}</dd></div>
        </dl>

        {#if pendingPlan}
          <section class:blocking={pendingPlan.blocking} class="impact-panel">
            <h3>
              {#if pendingPlan.blocking}
                <IconAlertTriangle size={16} stroke={1.8} />
              {:else}
                <IconCheck size={16} stroke={1.8} />
              {/if}
              {pendingPlan.operation === "install" ? t("themes-install-impact") : t("themes-activation-impact")}
            </h3>
            <p>{t("themes-impact-summary", {
              files: l10n.formatNumber(pendingPlan.affectedFiles.length),
              overrides: l10n.formatNumber(pendingPlan.localOverrides.length),
            })}</p>
            {#each [...pendingPlan.conflicts, ...pendingPlan.missingRequirements, ...pendingPlan.localOverrides, ...pendingPlan.notices] as item}
              <p class:blocking={item.blocking} class="impact-item">{errorMessage(item.messageDiagnostic)}</p>
            {/each}
            <div class="detail-actions">
              <button type="button" class="ui-button" onclick={() => { pendingPlan = null; }}>
                {t("themes-cancel")}
              </button>
              <button
                type="button"
                class="ui-button primary primary-action"
                disabled={pendingPlan.blocking || !pendingPlan.changed || applying}
                onclick={applyPlan}
              >
                {applying ? t("themes-applying") : t("themes-confirm")}
              </button>
            </div>
          </section>
        {:else}
          <div class="detail-actions">
            {#if selectedTheme.status === "available" || !selectedTheme.installComplete}
              <button type="button" class="ui-button primary primary-action" onclick={() => prepare("install")}>
                <IconDownload size={15} stroke={1.8} /> {t("themes-check-install")}
              </button>
            {:else if selectedTheme.status !== "active"}
              <button type="button" class="ui-button primary primary-action" onclick={() => prepare("activate")}>
                {t("themes-check-activate")}
              </button>
            {:else}
              <button type="button" class="ui-button primary primary-action" onclick={() => app.setWorkbenchActivity("design_system")}>
                {t("themes-open-design-system")}
              </button>
            {/if}
          </div>
        {/if}
      {:else}
        <p class="empty-list">{t("themes-select")}</p>
      {/if}
    </aside>
  </div>
</section>

<style>
  h1,
  h2,
  h3,
  p {
    margin: 0;
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

  .catalog-toolbar > span {
    color: var(--brand-strong);
    font-size: 12px;
    font-weight: 700;
  }

  .workspace-body {
    display: grid;
    grid-template-columns: minmax(390px, 58%) minmax(300px, 42%);
    min-height: 0;
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .theme-list,
  .theme-detail {
    min-height: 0;
    overflow: auto;
  }

  .theme-list {
    padding: 10px;
    background: var(--surface-panel);
  }

  .theme-detail {
    padding: 16px;
    border-left: 1px solid var(--border);
    background: var(--material-panel);
    box-shadow: inset 1px 0 0 var(--skeuo-edge-highlight);
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

  .theme-row img {
    width: 126px;
    aspect-ratio: 16 / 10;
    border: 1px solid var(--border);
    border-radius: 6px;
    object-fit: cover;
    background: var(--surface-2);
    box-shadow: var(--shadow-control);
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
    background: var(--material-control);
    box-shadow: var(--shadow-control);
  }

  .status-badge.active {
    color: var(--brand-strong);
    background: var(--material-control-selected);
    box-shadow: var(--shadow-pressed);
  }

  .theme-preview {
    width: 100%;
    aspect-ratio: 16 / 10;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    object-fit: cover;
    background: var(--surface-2);
    box-shadow: var(--shadow-control);
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
    background: var(--material-control);
    box-shadow: var(--shadow-control);
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
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
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

  .error-message {
    padding: 8px 12px;
    color: var(--danger);
    font-size: 12px;
    background: var(--material-inset);
    box-shadow: var(--shadow-inset);
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

    .workspace-header dl div:nth-child(2) {
      display: none;
    }
  }
</style>
