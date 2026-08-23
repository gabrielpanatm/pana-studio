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
  import type { ProjectWorkspaceMutationService } from "$lib/session/workspace-mutation-service";
  import type { GlobalStatusState } from "$lib/status/state.svelte";
  import type {
    AuditCategory,
    AuditFinding,
    AuditImpact,
    AuditOutcome,
    AuditPolicy,
    AuditProviderStatus,
    AuditRunMode,
    AuditRunReceipt,
    AuditSourceOrigin,
  } from "$lib/audit/contracts";
  import type { AuditRefreshResult } from "$lib/deploy/contracts";
  import type { SourceRange } from "$lib/source-graph/contracts";
  import type { WorkspaceSourceOpenOptions } from "$lib/workbench/contracts";
  import { auditProviderStatusCounts } from "$lib/audit/model";
  import { errorMessage } from "$lib/util";

  let {
    snapshot,
    workspaceMutations,
    projectAuditLoading,
    projectAuditError,
    validationRunningState,
    projectHealth,
    applySafeAuditFix,
    refreshProjectAudit,
    runZolaValidation,
    revealSourceRange,
    globalStatus,
    openWorkspaceSource,
    requestedView = "overview",
    observabilityFocusSerial = 0,
    onViewChange = undefined,
  }: {
    snapshot: AuditRunReceipt | null;
    workspaceMutations: ProjectWorkspaceMutationService;
    projectAuditLoading: boolean;
    projectAuditError: string;
    validationRunningState: { running: boolean; message: string };
    projectHealth: {
      currentProjectPath: string;
      projectFileCount: number;
      sourceNodeCount: number;
      dirtyAreas: string[];
      canSave: boolean;
      diskBlockedReason: string | null;
      projectStatus: string;
    };
    applySafeAuditFix: (finding: AuditFinding, fixId: string) => Promise<unknown>;
    refreshProjectAudit: (force?: boolean, mode?: AuditRunMode) => Promise<AuditRefreshResult>;
    runZolaValidation: () => Promise<boolean>;
    revealSourceRange: (file: string, range: SourceRange) => void;
    globalStatus: GlobalStatusState;
    openWorkspaceSource: (
      path: string,
      options?: WorkspaceSourceOpenOptions,
    ) => void | Promise<void>;
    requestedView?: AuditView;
    observabilityFocusSerial?: number;
    onViewChange?: (view: AuditView) => void;
  } = $props();

  type AuditView = "overview" | "runtime";
  type OutcomeFilter = "all" | AuditOutcome;
  type ImpactFilter = "all" | AuditImpact;
  type PolicyFilter = "all" | AuditPolicy;
  type CategoryFilter = "all" | AuditCategory;
  type OriginFilter = "all" | AuditSourceOrigin;

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
    components: t("audit-category-components"),
    content: t("audit-category-content"),
    data: t("audit-category-data"),
    deploy: t("audit-category-deploy"),
    performance: t("audit-category-performance"),
    crawl: t("audit-category-crawl"),
  });
  const outcomeLabels = $derived<Record<AuditOutcome, string>>({
    pass: t("audit-outcome-pass"),
    violation: t("audit-outcome-violation"),
    needs_review: t("audit-outcome-needs-review"),
    not_applicable: t("audit-outcome-not-applicable"),
    skipped: t("audit-outcome-skipped"),
    engine_error: t("audit-outcome-engine-error"),
    suppressed: t("audit-outcome-suppressed"),
  });
  const providerStatusLabels = $derived<Record<AuditProviderStatus, string>>({
    complete: t("audit-provider-status-complete"),
    partial: t("audit-provider-status-partial"),
    failed: t("audit-provider-status-failed"),
    skipped: t("audit-provider-status-skipped"),
  });

  let activeView = $state<AuditView>("overview");
  let outcomeFilter = $state<OutcomeFilter>("all");
  let impactFilter = $state<ImpactFilter>("all");
  let policyFilter = $state<PolicyFilter>("all");
  let categoryFilter = $state<CategoryFilter>("all");
  let providerFilter = $state("all");
  let originFilter = $state<OriginFilter>("all");
  let query = $state("");
  let validationRunning = $state(false);
  let expandedFixId = $state<string | null>(null);
  let applyingFixId = $state<string | null>(null);

  function findingTitle(finding: AuditFinding) {
    return errorMessage(finding.titleDiagnostic);
  }

  function findingMessage(finding: AuditFinding) {
    return errorMessage(finding.messageDiagnostic);
  }

  function visibleReplacement(value: string) {
    return value.replaceAll("\t", "⇥").replaceAll(" ", "·") || "∅";
  }

  async function applySafeFix(finding: AuditFinding, fixId: string) {
    if (applyingFixId) return;
    applyingFixId = fixId;
    try {
      await applySafeAuditFix(finding, fixId);
      expandedFixId = null;
    } catch (error) {
      globalStatus.set(t("audit-fix-failed", { error: errorMessage(error) }), "error");
    } finally {
      applyingFixId = null;
    }
  }

  $effect(() => {
    if (activeView !== requestedView) activeView = requestedView;
  });

  const normalizedQuery = $derived(query.trim().toLocaleLowerCase(l10n.locale));
  const findings = $derived.by(() => {
    const source = snapshot?.findings ?? [];
    return source.filter((finding) => {
      if (outcomeFilter !== "all" && finding.outcome !== outcomeFilter) return false;
      if (impactFilter !== "all" && finding.impact !== impactFilter) return false;
      if (policyFilter !== "all" && finding.policy !== policyFilter) return false;
      if (categoryFilter !== "all" && finding.category !== categoryFilter) return false;
      if (providerFilter !== "all" && finding.providerId !== providerFilter) return false;
      if (
        originFilter !== "all"
        && finding.primaryLocation?.origin !== originFilter
      ) return false;
      if (!normalizedQuery) return true;
      return [
        findingTitle(finding),
        findingMessage(finding),
        finding.primaryLocation?.file ?? "",
        finding.ruleCode,
        finding.providerId,
        categoryLabels[finding.category],
      ].some((value) => value.toLocaleLowerCase(l10n.locale).includes(normalizedQuery));
    });
  });
  const buildProvider = $derived(snapshot?.providers.find((provider) => provider.id === "build_zola") ?? null);
  const providerStatusCounts = $derived(auditProviderStatusCounts(snapshot?.providers ?? []));
  const incompleteProviderCount = $derived(
    providerStatusCounts.partial + providerStatusCounts.failed + providerStatusCounts.skipped,
  );

  $effect(() => {
    const projectRoot = workspaceMutations.identity?.expectedProjectRoot ?? "";
    const runtimeSessionId = workspaceMutations.identity?.expectedSessionId ?? "";
    const workspaceRevision = workspaceMutations.snapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void refreshProjectAudit();
  });

  async function runFullAudit() {
    if (validationRunning) return;
    validationRunning = true;
    try {
      let buildError = "";
      try {
        const valid = await runZolaValidation();
        if (!valid) buildError = validationRunningState.message;
      } catch (error) {
        buildError = error instanceof Error ? error.message : String(error);
      }
      const result = await refreshProjectAudit(true, "full");
      if (!result.ok) throw new Error(result.error || t("audit-full-no-receipt"));
      if (buildError) {
        globalStatus.set(t("audit-full-failed", { error: buildError }), "error");
      }
    } catch (error) {
      globalStatus.set(
        t("audit-full-failed", { error: error instanceof Error ? error.message : String(error) }),
        "error",
      );
    } finally {
      validationRunning = false;
    }
  }

  async function openFinding(finding: AuditFinding) {
    const location = finding.primaryLocation;
    if (!location) return;
    await openWorkspaceSource(location.file, { surface: "code" });
    if (location.range) revealSourceRange(location.file, location.range);
  }

  function findingLocation(finding: AuditFinding) {
    const location = finding.primaryLocation;
    if (!location) return t("audit-project-location");
    if (!location.range) return location.file;
    return `${location.file}:${location.range.line}:${location.range.column}`;
  }

  function resetFilters() {
    outcomeFilter = "all";
    impactFilter = "all";
    policyFilter = "all";
    categoryFilter = "all";
    providerFilter = "all";
    originFilter = "all";
    query = "";
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
        disabled={projectAuditLoading}
        onclick={() => { void refreshProjectAudit(true, "quick"); }}
      >
        <IconRefresh class={projectAuditLoading ? "spin" : undefined} size={15} stroke={1.9} />
        {t("audit-refresh")}
      </button>
      <button
        class="ui-button primary toolbar"
        type="button"
        disabled={validationRunning || validationRunningState.running}
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
        currentProjectPath={projectHealth.currentProjectPath}
        projectFileCount={projectHealth.projectFileCount}
        sourceNodeCount={projectHealth.sourceNodeCount}
        dirtyAreas={projectHealth.dirtyAreas}
        canSave={projectHealth.canSave}
        diskBlockedReason={projectHealth.diskBlockedReason}
        projectStatus={projectHealth.projectStatus}
        {observabilityFocusSerial}
        onStatusUpdate={(text, kind) => globalStatus.set(text, kind)}
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
        <article class:error={Boolean(snapshot?.summary.violations)}>
          <span>{t("audit-violations")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.violations ?? 0)}</strong>
        </article>
        <article class:warning={Boolean(snapshot?.summary.needsReview)}>
          <span>{t("audit-needs-review")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.needsReview ?? 0)}</strong>
        </article>
        <article class:error={Boolean(snapshot?.summary.engineErrors)}>
          <span>{t("audit-engine-errors")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.engineErrors ?? 0)}</strong>
        </article>
        <article>
          <span>{t("audit-passed")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.passed ?? 0)}</strong>
        </article>
        <article>
          <span>{t("audit-not-applicable")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.notApplicable ?? 0)}</strong>
        </article>
        <article aria-label={t("audit-files-count", { count: snapshot?.summary.affectedFiles ?? 0 })}>
          <span>{t("audit-affected-files")}</span>
          <strong>{l10n.formatNumber(snapshot?.summary.affectedFiles ?? 0)}</strong>
        </article>
        <article
          class:zola-error={buildProvider?.status === "failed"}
          class:zola-success={buildProvider?.status === "complete"}
        >
          <span>{t("audit-coverage")}</span>
          <strong>{snapshot?.completeness ?? t("audit-not-run")}</strong>
          <small>{t("audit-provider-incomplete-count", { count: incompleteProviderCount })}</small>
        </article>
      </section>

      {#if snapshot}
        <section class="provider-strip" aria-label={t("audit-providers-label")}>
          {#each snapshot.providers as provider (provider.id)}
            <span
              class:incomplete={provider.status !== "complete"}
              title={provider.coverage.limitations.map(errorMessage).join("\n")}
            >
              <strong>{provider.id.replaceAll("_", " ")}</strong>
              {providerStatusLabels[provider.status]} · {provider.coverage.analyzed}/{provider.coverage.eligible}
              · {provider.publishCoverageRequirement === "required"
                ? t("audit-provider-coverage-required")
                : t("audit-provider-coverage-advisory")}
              {#each provider.coverage.limitations as limitation}
                <small>{errorMessage(limitation)}</small>
              {/each}
            </span>
          {/each}
        </section>
      {/if}

      <section class="diagnostics-card" aria-labelledby="diagnostics-title">
        <header class="diagnostics-toolbar">
          <div>
            <h2 id="diagnostics-title">{t("audit-diagnostics")}</h2>
            <span>{t("audit-visible-count", {
              visible: l10n.formatNumber(findings.length),
              total: l10n.formatNumber(snapshot?.summary.total ?? 0),
            })}</span>
          </div>
          <label class="search-field">
            <span class="sr-only">{t("audit-search-label")}</span>
            <IconSearch size={14} stroke={1.9} />
            <input class="ui-field toolbar" bind:value={query} type="search" placeholder={t("audit-search-placeholder")} />
          </label>
          <label>
            <span>{t("audit-outcome")}</span>
            <select class="ui-field toolbar" bind:value={outcomeFilter}>
              <option value="all">{t("audit-all")}</option>
              {#each Object.entries(outcomeLabels) as [value, label]}
                <option {value}>{label}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>{t("audit-impact")}</span>
            <select class="ui-field toolbar" bind:value={impactFilter}>
              <option value="all">{t("audit-all")}</option>
              <option value="critical">critical</option>
              <option value="serious">serious</option>
              <option value="moderate">moderate</option>
              <option value="minor">minor</option>
              <option value="info">info</option>
            </select>
          </label>
          <label>
            <span>{t("audit-policy")}</span>
            <select class="ui-field toolbar" bind:value={policyFilter}>
              <option value="all">{t("audit-all")}</option>
              <option value="blocking">blocking</option>
              <option value="budget">budget</option>
              <option value="advisory">advisory</option>
              <option value="off">off</option>
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
              <option value="components">{categoryLabels.components}</option>
              <option value="content">{categoryLabels.content}</option>
              <option value="data">{categoryLabels.data}</option>
              <option value="deploy">{categoryLabels.deploy}</option>
              <option value="performance">{categoryLabels.performance}</option>
              <option value="crawl">{categoryLabels.crawl}</option>
            </select>
          </label>
          <label>
            <span>{t("audit-provider")}</span>
            <select class="ui-field toolbar" bind:value={providerFilter}>
              <option value="all">{t("audit-all")}</option>
              {#each snapshot?.providers ?? [] as provider (provider.id)}
                <option value={provider.id}>{provider.id.replaceAll("_", " ")}</option>
              {/each}
            </select>
          </label>
          <label>
            <span>{t("audit-scope")}</span>
            <select class="ui-field toolbar" bind:value={originFilter}>
              <option value="all">{t("audit-all")}</option>
              <option value="project">project</option>
              <option value="theme">theme</option>
              <option value="workspace">workspace</option>
              <option value="generated">generated</option>
            </select>
          </label>
        </header>

        <div class="diagnostics-list" aria-live="polite">
          {#if projectAuditError}
            <div class="empty-state error" role="alert">
              <IconAlertTriangle size={22} stroke={1.8} />
              <strong>{t("audit-rust-failed")}</strong>
              <span>{projectAuditError}</span>
              <button class="ui-button toolbar" type="button" onclick={() => { void refreshProjectAudit(true, "quick"); }}>{t("audit-retry")}</button>
            </div>
          {:else if projectAuditLoading && !snapshot}
            <div class="empty-state">{t("audit-building")}</div>
          {:else if findings.length === 0 && (snapshot?.summary.total ?? 0) > 0}
            <div class="empty-state">
              <IconSearch size={22} stroke={1.8} />
              <strong>{t("audit-no-filter-results")}</strong>
              <button class="ui-button toolbar" type="button" onclick={resetFilters}>{t("audit-reset-filters")}</button>
            </div>
          {:else if findings.length === 0}
            <div class="empty-state success">
              <IconCircleCheck size={24} stroke={1.8} />
              <strong>{t("audit-no-known-problems")}</strong>
              <span>{t("audit-run-full-help")}</span>
            </div>
          {:else}
            {#each findings as finding (finding.id)}
              <article
                aria-label={`${findingTitle(finding)}. ${findingMessage(finding)}. ${findingLocation(finding)}`}
                class:error={finding.outcome === "violation" || finding.outcome === "engine_error"}
                class:warning={finding.outcome === "needs_review"}
              >
                <span class="severity" aria-label={`${finding.outcome}, ${finding.impact}, ${finding.policy}`}>
                  {#if finding.outcome === "violation" || finding.outcome === "engine_error"}
                    <IconAlertTriangle size={16} stroke={2} />
                  {:else if finding.outcome === "needs_review"}
                    <IconAlertTriangle size={16} stroke={1.8} />
                  {:else if finding.outcome === "pass"}
                    <IconCircleCheck size={16} stroke={1.8} />
                  {:else}
                    <IconInfoCircle size={16} stroke={1.8} />
                  {/if}
                </span>
                <div class="diagnostic-copy">
                  <div><strong>{findingTitle(finding)}</strong><code>{finding.ruleCode}</code></div>
                  <p>{findingMessage(finding)}</p>
                  <span>{categoryLabels[finding.category]} · {finding.providerId} · {finding.outcome}/{finding.impact}/{finding.policy} · {findingLocation(finding)}</span>
                  {#each finding.fixes.filter((fix) => expandedFixId === fix.id) as fix (fix.id)}
                    <div class="fix-preview">
                      <strong>{t("audit-fix-preview-title")}</strong>
                      {#each fix.edits as edit, index (`${edit.location.file}:${edit.location.range?.start ?? index}`)}
                        <code>{edit.location.file}:{edit.location.range?.line ?? 1} · {visibleReplacement(edit.replacement)}</code>
                      {/each}
                    </div>
                  {/each}
                </div>
                <div class="diagnostic-actions">
                  {#if finding.primaryLocation}
                    <button class="ui-button compact" type="button" onclick={() => { void openFinding(finding); }}>
                      {t("audit-open")} <IconExternalLink size={13} stroke={1.9} />
                    </button>
                  {/if}
                  {#each finding.fixes.filter((fix) => fix.applicability === "safe") as fix (fix.id)}
                    <button
                      class="ui-button compact"
                      type="button"
                      onclick={() => { expandedFixId = expandedFixId === fix.id ? null : fix.id; }}
                    >{t("audit-fix-preview")}</button>
                    <button
                      class="ui-button primary compact"
                      type="button"
                      disabled={Boolean(applyingFixId)}
                      onclick={() => { void applySafeFix(finding, fix.id); }}
                    >{applyingFixId === fix.id ? t("audit-fix-applying") : t("audit-fix-apply-safe")}</button>
                  {/each}
                </div>
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
  .diagnostic-actions,
  .empty-state {
    display: flex;
    align-items: center;
  }

  .heading { min-width: 0; }
  .heading p { max-width: 720px; }

  .header-actions { flex: 0 0 auto; gap: 7px; }

  .overview-panel { min-width: 0; min-height: 0; overflow: auto; padding: 12px; }
  .runtime-panel { min-width: 0; min-height: 0; overflow: hidden; padding: 10px; }

  .audit-summary { display: grid; grid-template-columns: repeat(7, minmax(92px, 0.6fr)) minmax(220px, 1.7fr); gap: 8px; }
  .audit-summary article { display: grid; align-content: center; gap: 3px; min-height: 70px; padding: 10px 12px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--radius-control); background: var(--wb-surface-chrome, var(--surface-2)); }
  .audit-summary article.error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary article.warning { border-color: color-mix(in srgb, var(--wb-warning, #d97706) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-success { border-color: color-mix(in srgb, var(--success, #0f766e) 48%, var(--wb-border-subtle)); }
  .audit-summary article.zola-error { border-color: color-mix(in srgb, var(--danger, #dc2626) 48%, var(--wb-border-subtle)); }
  .audit-summary span { color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; font-weight: 800; letter-spacing: 0.04em; text-transform: uppercase; }
  .audit-summary strong { color: var(--text-strong); font-size: 20px; }
  .audit-summary article:last-child strong { font-size: 12px; }
  .audit-summary small { overflow: hidden; color: var(--wb-text-muted, var(--text-muted)); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }

  .provider-strip { display: flex; gap: 6px; margin-top: 8px; overflow-x: auto; }
  .provider-strip span { display: grid; flex: 0 0 auto; gap: 2px; padding: 5px 7px; border: 1px solid color-mix(in srgb, var(--success, #0f766e) 40%, var(--wb-border-subtle)); border-radius: var(--radius-control); color: var(--wb-text-muted, var(--text-muted)); background: var(--wb-surface-chrome, var(--surface-2)); font-size: 11px; }
  .provider-strip span.incomplete { border-color: color-mix(in srgb, var(--wb-warning, #d97706) 48%, var(--wb-border-subtle)); }
  .provider-strip strong { margin-right: 4px; color: var(--text-strong); text-transform: capitalize; }
  .provider-strip small { max-width: 320px; white-space: normal; }

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
  .diagnostic-actions { align-self: center; justify-content: flex-end; gap: 5px; }
  .diagnostic-actions button { min-height: 26px; }
  .diagnostic-copy > .fix-preview { display: grid; align-items: initial; gap: 4px; margin-top: 7px; padding: 7px; border: 1px solid var(--wb-border-subtle, var(--border)); border-radius: var(--radius-control); background: var(--surface-2); }
  .fix-preview code { display: block; overflow: hidden; color: var(--wb-text-muted, var(--text-muted)); text-overflow: ellipsis; white-space: nowrap; }

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
