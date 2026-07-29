<script lang="ts">
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconExternalLink,
    IconInfoCircle,
    IconRefresh,
    IconSearch,
    IconShieldCheck,
    IconTerminal2,
  } from "@tabler/icons-svelte";
  import KernelWorkspace from "$lib/components/kernel/KernelWorkspace.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    AuditCategory,
    AuditDiagnostic,
    AuditSeverity,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
    requestedView = "overview",
    observabilityFocusSerial = 0,
    onViewChange = undefined,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
    requestedView?: AuditView;
    observabilityFocusSerial?: number;
    onViewChange?: (view: AuditView) => void;
  } = $props();

  type AuditView = "overview" | "runtime";
  type SeverityFilter = "all" | AuditSeverity;
  type CategoryFilter = "all" | AuditCategory;

  const views = $derived([
    { id: "overview" as const, label: t("audit-view-project") },
    { id: "runtime" as const, label: t("audit-view-runtime") },
  ]);
  const categoryLabels = $derived<Record<AuditCategory, string>>({
    build: t("audit-category-build"),
    references: t("audit-category-references"),
    accessibility: t("audit-category-accessibility"),
    seo: t("audit-category-seo"),
    assets: t("audit-category-assets"),
    workspace: t("audit-category-workspace"),
  });

  let activeView = $state<AuditView>("overview");
  let severityFilter = $state<SeverityFilter>("all");
  let categoryFilter = $state<CategoryFilter>("all");
  let query = $state("");
  let validationRunning = $state(false);

  function diagnosticTitle(diagnostic: AuditDiagnostic) {
    return errorMessage(diagnostic.titleDiagnostic);
  }

  function diagnosticMessage(diagnostic: AuditDiagnostic) {
    return errorMessage(diagnostic.messageDiagnostic);
  }

  $effect(() => {
    if (activeView !== requestedView) activeView = requestedView;
  });

  const snapshot = $derived(app.projectAuditSnapshot);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const diagnostics = $derived.by(() => {
    const source = snapshot?.diagnostics ?? [];
    return source.filter((diagnostic) => {
      if (severityFilter !== "all" && diagnostic.severity !== severityFilter) return false;
      if (categoryFilter !== "all" && diagnostic.category !== categoryFilter) return false;
      if (!normalizedQuery) return true;
      return [
        diagnosticTitle(diagnostic),
        diagnosticMessage(diagnostic),
        diagnostic.file ?? "",
        diagnostic.code,
        categoryLabels[diagnostic.category],
      ].some((value) => value.toLocaleLowerCase(l10n.locale).includes(normalizedQuery));
    });
  });
  const zolaTone = $derived(
    app.controlledPreview.validation === "valid"
      ? "success"
      : app.controlledPreview.validation === "invalid"
        || app.controlledPreview.validation === "error"
        ? "error"
        : "neutral",
  );
  const zolaLabel = $derived.by(() => {
    switch (app.controlledPreview.validation) {
      case "valid": return t("audit-zola-valid");
      case "invalid": return t("audit-zola-invalid");
      case "error": return t("audit-zola-unavailable");
      case "queued": return t("audit-zola-queued");
      case "running": return t("audit-zola-running");
      default: return t("audit-zola-none");
    }
  });

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshProjectAudit();
  });

  async function runFullAudit() {
    if (validationRunning) return;
    validationRunning = true;
    try {
      await app.runZolaValidation("manual");
      await app.refreshProjectAudit(true);
    } catch (error) {
      app.setGlobalStatus(
        t("audit-full-failed", { error: error instanceof Error ? error.message : String(error) }),
        "error",
      );
    } finally {
      validationRunning = false;
    }
  }

  async function openDiagnostic(diagnostic: AuditDiagnostic) {
    if (!diagnostic.file) return;
    await openWorkspaceSource(diagnostic.file);
  }

  function diagnosticLocation(diagnostic: AuditDiagnostic) {
    if (!diagnostic.file) return t("audit-project-location");
    if (!diagnostic.range) return diagnostic.file;
    return `${diagnostic.file}:${diagnostic.range.line}:${diagnostic.range.column}`;
  }

  function selectView(view: AuditView) {
    activeView = view;
    onViewChange?.(view);
  }

  function handleViewKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + views.length) % views.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % views.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = views.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const next = views[nextIndex];
    if (!next) return;
    selectView(next.id);
    requestAnimationFrame(() => document.getElementById(`audit-tab-${next.id}`)?.focus());
  }
</script>

<section class="activity-workspace audit-workspace" aria-labelledby="audit-title">
  <header class="workspace-header audit-header">
    <div class="heading">
      <span class="eyebrow"><IconShieldCheck size={15} stroke={1.9} /> {t("audit-eyebrow")}</span>
      <h1 id="audit-title">{t("audit-title")}</h1>
      <p>{t("audit-description")}</p>
    </div>
    <div class="header-actions">
      <button
        class="ui-button toolbar"
        type="button"
        disabled={app.projectAuditLoading}
        onclick={() => { void app.refreshProjectAudit(true); }}
      >
        <IconRefresh class={app.projectAuditLoading ? "spin" : undefined} size={15} stroke={1.9} />
        {t("audit-refresh")}
      </button>
      <button
        class="ui-button primary toolbar"
        type="button"
        disabled={validationRunning || app.controlledPreview.validation === "running"}
        onclick={() => { void runFullAudit(); }}
      >
        <IconCircleCheck size={15} stroke={1.9} />
        {t("audit-run-full")}
      </button>
    </div>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("audit-tabs-label")}>
      {#each views as view, index (view.id)}
        <button
          id={`audit-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`audit-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => { selectView(view.id); }}
          onkeydown={(event) => { handleViewKeydown(event, index); }}
        >
          {#if view.id === "runtime"}<IconTerminal2 size={14} stroke={1.9} />{/if}
          {view.label}
        </button>
      {/each}
    </div>
  </div>

  {#if activeView === "runtime"}
    <div
      id="audit-panel-runtime"
      class="runtime-panel"
      role="tabpanel"
      aria-labelledby="audit-tab-runtime"
    >
      <KernelWorkspace
        currentProjectPath={app.currentProjectPath}
        projectFileCount={app.scannedProject?.files.length ?? 0}
        sourceNodeCount={app.sourceGraph?.nodes.length ?? 0}
        dirtyAreas={app.globalDirtyState.areas}
        canSave={app.globalDirtyState.canSave}
        diskBlockedReason={app.immediateDiskOperationBlockedReason}
        projectStatus={app.projectStatus}
        {observabilityFocusSerial}
        onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
      />
    </div>
  {:else}
    <div
      id="audit-panel-overview"
      class="overview-panel"
      role="tabpanel"
      aria-labelledby="audit-tab-overview"
    >
      <section class="audit-summary" aria-label={t("audit-summary-label")}>
        <article aria-label={t("audit-errors-count", { count: snapshot?.summary.errors ?? 0 })} class:error={Boolean(snapshot?.summary.errors)}>
          <span>{t("audit-errors")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.errors ?? 0)}</strong>
        </article>
        <article aria-label={t("audit-warnings-count", { count: snapshot?.summary.warnings ?? 0 })} class:warning={Boolean(snapshot?.summary.warnings)}>
          <span>{t("audit-warnings")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.warnings ?? 0)}</strong>
        </article>
        <article aria-label={t("audit-info-count", { count: snapshot?.summary.info ?? 0 })}>
          <span>{t("audit-informational")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.info ?? 0)}</strong>
        </article>
        <article aria-label={t("audit-files-count", { count: snapshot?.summary.affectedFiles ?? 0 })}>
          <span>{t("audit-affected-files")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.affectedFiles ?? 0)}</strong>
        </article>
        <article
          aria-label={t("audit-build-label", {
            status: zolaLabel,
            message: app.controlledPreview.validationMessage,
          })}
          class:zola-error={zolaTone === "error"}
          class:zola-success={zolaTone === "success"}
        >
          <span>{t("audit-build")}</span>
          <strong>{zolaLabel}</strong>
          <small>{app.controlledPreview.validationMessage}</small>
        </article>
      </section>

      <section class="diagnostics-card" aria-labelledby="diagnostics-title">
        <header class="diagnostics-toolbar">
          <div>
            <h2 id="diagnostics-title">{t("audit-diagnostics")}</h2>
            <span>{t("audit-visible-count", {
              visible: l10n.formatNumber(diagnostics.length),
              total: l10n.formatNumber(snapshot?.summary.total ?? 0),
            })}</span>
          </div>
          <label class="search-field">
            <span class="sr-only">{t("audit-search-label")}</span>
            <IconSearch size={14} stroke={1.9} />
            <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("audit-search-placeholder")} />
          </label>
          <label>
            <span>{t("audit-severity")}</span>
            <select class="ui-field toolbar" bind:value={severityFilter}>
              <option value="all">{t("audit-all")}</option>
              <option value="error">{t("audit-errors")}</option>
              <option value="warning">{t("audit-warnings")}</option>
              <option value="info">{t("audit-informational")}</option>
            </select>
          </label>
          <label>
            <span>{t("audit-category")}</span>
            <select class="ui-field toolbar" bind:value={categoryFilter}>
              <option value="all">{t("audit-all")}</option>
              <option value="build">{categoryLabels.build}</option>
              <option value="references">{categoryLabels.references}</option>
              <option value="accessibility">{categoryLabels.accessibility}</option>
              <option value="seo">{categoryLabels.seo}</option>
              <option value="assets">{categoryLabels.assets}</option>
              <option value="workspace">{categoryLabels.workspace}</option>
            </select>
          </label>
        </header>

        <div class="diagnostics-list" aria-live="polite">
          {#if app.projectAuditError}
            <div class="empty-state error" role="alert">
              <IconAlertTriangle size={22} stroke={1.8} />
              <strong>{t("audit-rust-failed")}</strong>
              <span>{app.projectAuditError}</span>
              <button class="ui-button toolbar" type="button" onclick={() => { void app.refreshProjectAudit(true); }}>{t("audit-retry")}</button>
            </div>
          {:else if app.projectAuditLoading && !snapshot}
            <div class="empty-state">{t("audit-building")}</div>
          {:else if diagnostics.length === 0 && (snapshot?.summary.total ?? 0) > 0}
            <div class="empty-state">
              <IconSearch size={22} stroke={1.8} />
              <strong>{t("audit-no-filter-results")}</strong>
              <button class="ui-button toolbar" type="button" onclick={() => { severityFilter = "all"; categoryFilter = "all"; query = ""; }}>{t("audit-reset-filters")}</button>
            </div>
          {:else if diagnostics.length === 0}
            <div class="empty-state success">
              <IconCircleCheck size={24} stroke={1.8} />
              <strong>{t("audit-no-known-problems")}</strong>
              <span>{t("audit-run-full-help")}</span>
            </div>
          {:else}
            {#each diagnostics as diagnostic (diagnostic.id)}
              <article
                aria-label={`${diagnosticTitle(diagnostic)}. ${diagnosticMessage(diagnostic)}. ${diagnosticLocation(diagnostic)}`}
                class:error={diagnostic.severity === "error"}
                class:warning={diagnostic.severity === "warning"}
              >
                <span class="severity" aria-label={t("audit-severity-label", { severity: diagnostic.severity })}>
                  {#if diagnostic.severity === "error"}
                    <IconAlertTriangle size={16} stroke={2} />
                  {:else if diagnostic.severity === "warning"}
                    <IconAlertTriangle size={16} stroke={1.8} />
                  {:else}
                    <IconInfoCircle size={16} stroke={1.8} />
                  {/if}
                </span>
                <div class="diagnostic-copy">
                  <div><strong>{diagnosticTitle(diagnostic)}</strong><code>{diagnostic.code}</code></div>
                  <p>{diagnosticMessage(diagnostic)}</p>
                  <span>{categoryLabels[diagnostic.category]} · {diagnosticLocation(diagnostic)}</span>
                </div>
                {#if diagnostic.file}
                  <button class="ui-button compact" type="button" onclick={() => { void openDiagnostic(diagnostic); }}>
                    {t("audit-open")} <IconExternalLink size={13} stroke={1.9} />
                  </button>
                {/if}
              </article>
            {/each}
          {/if}
        </div>
      </section>
    </div>
  {/if}
</section>

<style>
  .header-actions,
  .diagnostics-toolbar,
  .diagnostics-toolbar > div,
  .search-field,
  .diagnostics-list article,
  .diagnostic-copy > div,
  .diagnostics-list article > button,
  .empty-state {
    display: flex;
    align-items: center;
  }

  .heading { min-width: 0; }
  .heading p { max-width: 720px; }

  .header-actions { flex: 0 0 auto; gap: 7px; }

  .overview-panel { min-width: 0; min-height: 0; overflow: auto; padding: 12px; }
  .runtime-panel { min-width: 0; min-height: 0; overflow: hidden; padding: 10px; }

  .audit-summary { display: grid; grid-template-columns: repeat(4, minmax(100px, 0.6fr)) minmax(220px, 1.7fr); gap: 8px; }
  .audit-summary article { display: grid; align-content: center; gap: 3px; min-height: 70px; padding: 10px 12px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--radius-control); background: var(--wb-surface-chrome, var(--surface-2)); }
  .audit-summary article.error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary article.warning { border-color: color-mix(in srgb, var(--wb-warning, #d97706) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-success { border-color: color-mix(in srgb, var(--success, #0f766e) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary span { color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; font-weight: 800; letter-spacing: 0.04em; text-transform: uppercase; }
  .audit-summary strong { color: var(--text-strong); font-size: 20px; }
  .audit-summary article:last-child strong { font-size: 12px; }
  .audit-summary small { overflow: hidden; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }

  .diagnostics-card { margin-top: 10px; overflow: hidden; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--radius-panel); background: var(--wb-surface-document, var(--surface)); }
  .diagnostics-toolbar { gap: 8px; min-height: 48px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle, var(--border)); background: var(--wb-surface-chrome, var(--surface-2)); }
  .diagnostics-toolbar > div { align-items: baseline; gap: 7px; margin-right: auto; }
  h2 { margin: 0; color: var(--text-strong); font-size: 12px; }
  .diagnostics-toolbar > div span { color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; }
  .diagnostics-toolbar label { display: grid; gap: 2px; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .diagnostics-toolbar select { min-width: 105px; }
  .diagnostics-toolbar .search-field { position: relative; display: flex; flex-direction: row; gap: 0; min-width: min(260px, 30vw); }

  .diagnostics-list { display: grid; }
  .diagnostics-list article { display: grid; grid-template-columns: 26px minmax(0, 1fr) auto; align-items: start; gap: 8px; min-height: 66px; padding: 9px 10px; border-bottom: 1px solid var(--wb-border-subtle, var(--border)); border-left: 3px solid var(--wb-accent, var(--brand)); }
  .diagnostics-list article:last-child { border-bottom: 0; }
  .diagnostics-list article.error { border-left-color: var(--danger, #dc2626); }
  .diagnostics-list article.warning { border-left-color: var(--wb-warning, #d97706); }
  .severity { display: grid; width: 24px; height: 24px; place-items: center; color: var(--wb-accent-strong, var(--brand-strong)); }
  article.error .severity { color: var(--danger, #dc2626); }
  article.warning .severity { color: var(--wb-warning, #d97706); }
  .diagnostic-copy { min-width: 0; }
  .diagnostic-copy > div { align-items: baseline; gap: 7px; }
  .diagnostic-copy strong { color: var(--text-strong); font-size: 12px; }
  .diagnostic-copy code { padding: 1px 4px; border-radius: 4px; color: var(--wb-text-muted, var(--text-muted)); background: var(--surface-4); font-size: 12px; }
  .diagnostic-copy p { margin: 4px 0; color: var(--wb-text-primary, var(--text)); font-size: 12px; line-height: 1.35; }
  .diagnostic-copy > span { color: var(--wb-text-muted, var(--text-muted)); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 12px; }
  .diagnostics-list article > button { align-self: center; min-height: 26px; }

  .empty-state { justify-content: center; flex-direction: column; gap: 6px; min-height: 190px; padding: 24px; color: var(--wb-text-muted, var(--text-muted)); text-align: center; font-size: 12px; }
  .empty-state strong { color: var(--text-strong); font-size: 12px; }
  .empty-state.error { color: var(--danger, #dc2626); }
  .empty-state.success :global(svg) { color: var(--success, #0f766e); }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  :global(.spin) { animation: audit-spin 0.8s linear infinite; }
  @keyframes audit-spin { to { transform: rotate(360deg); } }

  @media (max-width: 1050px) {
    .audit-summary { grid-template-columns: repeat(4, minmax(90px, 1fr)); }
    .audit-summary article:last-child { grid-column: 1 / -1; }
    .diagnostics-toolbar { align-items: stretch; flex-wrap: wrap; }
    .diagnostics-toolbar > div { width: 100%; }
    .diagnostics-toolbar .search-field { flex: 1 1 220px; }
  }
</style>
