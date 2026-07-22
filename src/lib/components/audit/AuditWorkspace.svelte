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
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    AuditCategory,
    AuditDiagnostic,
    AuditSeverity,
  } from "$lib/types";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type AuditView = "overview" | "runtime";
  type SeverityFilter = "all" | AuditSeverity;
  type CategoryFilter = "all" | AuditCategory;

  const views: { id: AuditView; label: string }[] = [
    { id: "overview", label: "Audit proiect" },
    { id: "runtime", label: "Execuție" },
  ];
  const categoryLabels: Record<AuditCategory, string> = {
    build: "Construire",
    references: "Referințe",
    accessibility: "Accesibilitate",
    seo: "SEO",
    assets: "Resurse",
    workspace: "Spațiu de lucru",
  };

  let activeView = $state<AuditView>("overview");
  let severityFilter = $state<SeverityFilter>("all");
  let categoryFilter = $state<CategoryFilter>("all");
  let query = $state("");
  let validationRunning = $state(false);

  const snapshot = $derived(app.projectAuditSnapshot);
  const normalizedQuery = $derived(query.trim().toLocaleLowerCase("ro"));
  const diagnostics = $derived.by(() => {
    const source = snapshot?.diagnostics ?? [];
    return source.filter((diagnostic) => {
      if (severityFilter !== "all" && diagnostic.severity !== severityFilter) return false;
      if (categoryFilter !== "all" && diagnostic.category !== categoryFilter) return false;
      if (!normalizedQuery) return true;
      return [
        diagnostic.title,
        diagnostic.message,
        diagnostic.file ?? "",
        diagnostic.code,
        categoryLabels[diagnostic.category],
      ].some((value) => value.toLocaleLowerCase("ro").includes(normalizedQuery));
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
      case "valid": return "Validare Zola reușită";
      case "invalid": return "Proiect Zola invalid";
      case "error": return "Validare indisponibilă";
      case "queued": return "Validare programată";
      case "running": return "Validare în curs";
      default: return "Zola nevalidat";
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
        `Auditul complet a eșuat: ${error instanceof Error ? error.message : String(error)}`,
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
    if (!diagnostic.file) return "Proiect";
    if (!diagnostic.range) return diagnostic.file;
    return `${diagnostic.file}:${diagnostic.range.line}:${diagnostic.range.column}`;
  }

  function selectView(view: AuditView) {
    activeView = view;
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

<section class="audit-workspace" aria-labelledby="audit-title">
  <header class="audit-header">
    <div class="heading">
      <span class="eyebrow"><IconShieldCheck size={15} stroke={1.9} /> Quality workspace</span>
      <h1 id="audit-title">Audit proiect</h1>
      <p>Problemele structurale sunt derivate din sesiunea Rust curentă; validarea Zola confirmă separat construirea reală.</p>
    </div>
    <div class="header-actions">
      <button
        type="button"
        disabled={app.projectAuditLoading}
        onclick={() => { void app.refreshProjectAudit(true); }}
      >
        <IconRefresh class={app.projectAuditLoading ? "spin" : undefined} size={15} stroke={1.9} />
        Reanalizează
      </button>
      <button
        class="primary"
        type="button"
        disabled={validationRunning || app.controlledPreview.validation === "running"}
        onclick={() => { void runFullAudit(); }}
      >
        <IconCircleCheck size={15} stroke={1.9} />
        Rulează audit complet
      </button>
    </div>
  </header>

  <div class="audit-tabs" role="tablist" aria-label="Vizualizări Audit">
    {#each views as view, index (view.id)}
      <button
        id={`audit-tab-${view.id}`}
        type="button"
        role="tab"
        aria-selected={activeView === view.id ? "true" : "false"}
        aria-controls={`audit-panel-${view.id}`}
        tabindex={activeView === view.id ? 0 : -1}
        class:active={activeView === view.id}
        onclick={() => { selectView(view.id); }}
        onkeydown={(event) => { handleViewKeydown(event, index); }}
      >
        {#if view.id === "runtime"}<IconTerminal2 size={14} stroke={1.9} />{/if}
        {view.label}
      </button>
    {/each}
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
      <section class="audit-summary" aria-label="Rezumat audit">
        <article aria-label={`${snapshot?.summary.errors ?? 0} erori`} class:error={Boolean(snapshot?.summary.errors)}>
          <span>Erori</span>
          <strong>{snapshot?.summary.errors ?? 0}</strong>
        </article>
        <article aria-label={`${snapshot?.summary.warnings ?? 0} avertismente`} class:warning={Boolean(snapshot?.summary.warnings)}>
          <span>Avertismente</span>
          <strong>{snapshot?.summary.warnings ?? 0}</strong>
        </article>
        <article aria-label={`${snapshot?.summary.info ?? 0} diagnostice informative`}>
          <span>Informative</span>
          <strong>{snapshot?.summary.info ?? 0}</strong>
        </article>
        <article aria-label={`${snapshot?.summary.affectedFiles ?? 0} fișiere afectate`}>
          <span>Fișiere afectate</span>
          <strong>{snapshot?.summary.affectedFiles ?? 0}</strong>
        </article>
        <article
          aria-label={`Construire: ${zolaLabel}. ${app.controlledPreview.validationMessage}`}
          class:zola-error={zolaTone === "error"}
          class:zola-success={zolaTone === "success"}
        >
          <span>Construire</span>
          <strong>{zolaLabel}</strong>
          <small>{app.controlledPreview.validationMessage}</small>
        </article>
      </section>

      <section class="diagnostics-card" aria-labelledby="diagnostics-title">
        <header class="diagnostics-toolbar">
          <div>
            <h2 id="diagnostics-title">Diagnostice</h2>
            <span>{diagnostics.length} din {snapshot?.summary.total ?? 0}</span>
          </div>
          <label class="search-field">
            <span class="sr-only">Caută în diagnostice</span>
            <IconSearch size={14} stroke={1.9} />
            <input bind:value={query} type="search" placeholder="Caută mesaj, cod sau fișier" />
          </label>
          <label>
            <span>Severitate</span>
            <select bind:value={severityFilter}>
              <option value="all">Toate</option>
              <option value="error">Erori</option>
              <option value="warning">Avertismente</option>
              <option value="info">Informative</option>
            </select>
          </label>
          <label>
            <span>Categorie</span>
            <select bind:value={categoryFilter}>
              <option value="all">Toate</option>
              <option value="build">Construire</option>
              <option value="references">Referințe</option>
              <option value="accessibility">Accesibilitate</option>
              <option value="seo">SEO</option>
              <option value="assets">Resurse</option>
              <option value="workspace">Spațiu de lucru</option>
            </select>
          </label>
        </header>

        <div class="diagnostics-list" aria-live="polite">
          {#if app.projectAuditError}
            <div class="empty-state error" role="alert">
              <IconAlertTriangle size={22} stroke={1.8} />
              <strong>Auditul Rust nu a putut fi construit</strong>
              <span>{app.projectAuditError}</span>
              <button type="button" onclick={() => { void app.refreshProjectAudit(true); }}>Reîncearcă</button>
            </div>
          {:else if app.projectAuditLoading && !snapshot}
            <div class="empty-state">Se construiește proiecția auditului din sesiunea proiectului…</div>
          {:else if diagnostics.length === 0 && (snapshot?.summary.total ?? 0) > 0}
            <div class="empty-state">
              <IconSearch size={22} stroke={1.8} />
              <strong>Niciun rezultat pentru filtrele curente</strong>
              <button type="button" onclick={() => { severityFilter = "all"; categoryFilter = "all"; query = ""; }}>Resetează filtrele</button>
            </div>
          {:else if diagnostics.length === 0}
            <div class="empty-state success">
              <IconCircleCheck size={24} stroke={1.8} />
              <strong>Nu există probleme structurale cunoscute</strong>
              <span>Rulează auditul complet pentru a confirma și build-ul Zola.</span>
            </div>
          {:else}
            {#each diagnostics as diagnostic (diagnostic.id)}
              <article
                aria-label={`${diagnostic.title}. ${diagnostic.message}. ${diagnosticLocation(diagnostic)}`}
                class:error={diagnostic.severity === "error"}
                class:warning={diagnostic.severity === "warning"}
              >
                <span class="severity" aria-label={`Severitate: ${diagnostic.severity}`}>
                  {#if diagnostic.severity === "error"}
                    <IconAlertTriangle size={16} stroke={2} />
                  {:else if diagnostic.severity === "warning"}
                    <IconAlertTriangle size={16} stroke={1.8} />
                  {:else}
                    <IconInfoCircle size={16} stroke={1.8} />
                  {/if}
                </span>
                <div class="diagnostic-copy">
                  <div><strong>{diagnostic.title}</strong><code>{diagnostic.code}</code></div>
                  <p>{diagnostic.message}</p>
                  <span>{categoryLabels[diagnostic.category]} · {diagnosticLocation(diagnostic)}</span>
                </div>
                {#if diagnostic.file}
                  <button type="button" onclick={() => { void openDiagnostic(diagnostic); }}>
                    Deschide <IconExternalLink size={13} stroke={1.9} />
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
  .audit-workspace {
    display: grid;
    grid-template-rows: auto 38px minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: 10px;
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-document, var(--surface));
    box-shadow: var(--shadow);
  }

  .audit-header,
  .header-actions,
  .header-actions button,
  .audit-tabs,
  .audit-tabs button,
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

  .audit-header {
    justify-content: space-between;
    gap: 24px;
    padding: 18px 20px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background:
      radial-gradient(circle at 22% 0%, var(--wb-accent-soft, var(--brand-soft)), transparent 36%),
      var(--wb-surface-chrome, var(--surface-2));
  }

  .heading { min-width: 0; }
  .eyebrow {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--wb-accent-strong, var(--brand-strong));
    font-size: 12px;
    font-weight: 850;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  h1 { margin: 6px 0 0; color: var(--text-strong); font-size: 24px; letter-spacing: -0.025em; }
  .heading p { max-width: 720px; margin: 6px 0 0; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; line-height: 1.45; }

  .header-actions { flex: 0 0 auto; gap: 7px; }
  .header-actions button,
  .diagnostics-list article > button,
  .empty-state button {
    justify-content: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 10px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: var(--wb-radius-control, 6px);
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-surface-document, var(--surface));
    font-size: 12px;
    font-weight: 750;
  }
  .header-actions button.primary { border-color: var(--wb-accent, var(--brand)); color: var(--wb-accent-strong, var(--brand-strong)); background: var(--wb-accent-soft, var(--brand-soft)); }
  button:disabled { cursor: default; opacity: 0.55; }
  button:not(:disabled) { cursor: pointer; }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--wb-focus-ring, var(--brand-strong)); outline-offset: 1px; }

  .audit-tabs {
    gap: 2px;
    padding: 0 10px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--wb-surface-chrome, var(--surface-2));
  }
  .audit-tabs button { align-self: stretch; gap: 5px; padding: 0 11px; border: 0; border-bottom: 2px solid transparent; color: var(--wb-text-muted, var(--text-muted)); background: transparent; font-size: 12px; font-weight: 800; }
  .audit-tabs button.active { border-bottom-color: var(--wb-accent, var(--brand)); color: var(--wb-text-primary, var(--text)); }

  .overview-panel { min-width: 0; min-height: 0; overflow: auto; padding: 12px; }
  .runtime-panel { min-width: 0; min-height: 0; overflow: hidden; padding: 10px; }

  .audit-summary { display: grid; grid-template-columns: repeat(4, minmax(100px, 0.6fr)) minmax(220px, 1.7fr); gap: 8px; }
  .audit-summary article { display: grid; align-content: center; gap: 3px; min-height: 70px; padding: 10px 12px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: 8px; background: var(--wb-surface-chrome, var(--surface-2)); }
  .audit-summary article.error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary article.warning { border-color: color-mix(in srgb, var(--wb-warning, #d97706) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-success { border-color: color-mix(in srgb, var(--success, #0f766e) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary span { color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; font-weight: 800; letter-spacing: 0.04em; text-transform: uppercase; }
  .audit-summary strong { color: var(--text-strong); font-size: 20px; }
  .audit-summary article:last-child strong { font-size: 12px; }
  .audit-summary small { overflow: hidden; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }

  .diagnostics-card { margin-top: 10px; overflow: hidden; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: 9px; background: var(--wb-surface-document, var(--surface)); }
  .diagnostics-toolbar { gap: 8px; min-height: 48px; padding: 7px 9px; border-bottom: 1px solid var(--wb-border-subtle, var(--border)); background: var(--wb-surface-chrome, var(--surface-2)); }
  .diagnostics-toolbar > div { align-items: baseline; gap: 7px; margin-right: auto; }
  h2 { margin: 0; color: var(--text-strong); font-size: 12px; }
  .diagnostics-toolbar > div span { color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; }
  .diagnostics-toolbar label { display: grid; gap: 2px; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  .diagnostics-toolbar input,
  .diagnostics-toolbar select { height: 28px; border: 1px solid var(--wb-border-subtle, var(--border-3)); border-radius: 5px; color: var(--wb-text-primary, var(--text)); background: var(--wb-surface-document, var(--surface)); font-size: 12px; }
  .diagnostics-toolbar select { min-width: 105px; padding: 0 7px; }
  .search-field { position: relative; display: flex !important; flex-direction: row; gap: 0 !important; min-width: min(260px, 30vw); }
  .search-field :global(svg) { position: absolute; left: 8px; color: var(--wb-text-muted, var(--text-muted)); pointer-events: none; }
  .search-field input { width: 100%; padding: 0 8px 0 28px; text-transform: none; }

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
    .search-field { flex: 1 1 220px; }
  }
</style>
