<script lang="ts">
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconCloudUpload,
    IconDeviceFloppy,
    IconHammer,
    IconRefresh,
    IconRocket,
    IconSettings,
    IconShieldCheck,
  } from "@tabler/icons-svelte";
  import DeployPane from "$lib/components/DeployPane.svelte";
  import { l10n, t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";

  let { app }: { app: AppState } = $props();

  type PublishView = "release" | "configuration";
  const views = $derived([
    { id: "release" as const, label: t("publish-view-release") },
    { id: "configuration" as const, label: t("publish-view-configuration") },
  ]);

  let activeView = $state<PublishView>("release");
  let preflightRunning = $state(false);

  const audit = $derived(app.projectAuditSnapshot);
  const auditCurrent = $derived(Boolean(
    audit
    && audit.projectRoot === app.sessionProjectRoot
    && audit.runtimeSessionId === app.kernelProjectSessionId
    && audit.workspaceRevision === app.projectWorkspaceSnapshot?.revision,
  ));
  const auditErrors = $derived(auditCurrent ? (audit?.summary.errors ?? 0) : 0);
  const auditWarnings = $derived(auditCurrent ? (audit?.summary.warnings ?? 0) : 0);
  const validationValid = $derived(app.controlledPreview.validation === "valid");
  const sourceSaved = $derived(!app.globalDirtyState.dirty);
  const releaseReady = $derived(sourceSaved && auditCurrent && auditErrors === 0 && validationValid);

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshProjectAudit();
  });

  function validationLabel() {
    switch (app.controlledPreview.validation) {
      case "valid": return t("publish-validation-valid");
      case "invalid": return t("publish-validation-invalid");
      case "error": return t("publish-validation-error");
      case "queued": return t("publish-validation-queued");
      case "running": return t("publish-validation-running");
      default: return t("publish-validation-none");
    }
  }

  async function runPreflight() {
    if (preflightRunning) return;
    preflightRunning = true;
    try {
      await app.runZolaValidation("manual");
      await app.refreshProjectAudit(true);
      app.setGlobalStatus(t("publish-preflight-updated"), "saved");
    } catch (error) {
      app.setGlobalStatus(
        t("publish-preflight-failed", { error: error instanceof Error ? error.message : String(error) }),
        "error",
      );
    } finally {
      preflightRunning = false;
    }
  }

  async function saveSession() {
    const saved = await app.saveActiveFile();
    if (saved) await app.refreshProjectAudit(true);
  }

  function selectView(view: PublishView) {
    activeView = view;
  }

  function handleTabKeydown(event: KeyboardEvent, index: number) {
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
    requestAnimationFrame(() => document.getElementById(`publish-tab-${next.id}`)?.focus());
  }
</script>

<section class="activity-workspace publish-workspace" aria-labelledby="publish-title">
  <header class="workspace-header">
    <div>
      <span class="eyebrow"><IconRocket size={15} stroke={1.9} /> {t("publish-eyebrow")}</span>
      <h1 id="publish-title">{t("publish-title")}</h1>
      <p>{t("publish-description")}</p>
    </div>
    <div class="release-state" class:ready={releaseReady} class:blocked={!releaseReady}>
      {#if releaseReady}<IconCircleCheck size={19} stroke={1.9} />{:else}<IconAlertTriangle size={19} stroke={1.9} />{/if}
      <div><span>{t("publish-state")}</span><strong>{releaseReady ? t("publish-ready") : t("publish-needs-check")}</strong></div>
    </div>
  </header>

  <div class="workspace-toolbar">
    <div class="ui-tabs view-tabs" role="tablist" aria-label={t("publish-tabs-label")}>
      {#each views as view, index (view.id)}
        <button
          id={`publish-tab-${view.id}`}
          type="button"
          role="tab"
          aria-selected={activeView === view.id ? "true" : "false"}
          aria-controls={`publish-panel-${view.id}`}
          tabindex={activeView === view.id ? 0 : -1}
          class="ui-tab"
          class:active={activeView === view.id}
          onclick={() => selectView(view.id)}
          onkeydown={(event) => handleTabKeydown(event, index)}
        >
          {#if view.id === "configuration"}<IconSettings size={14} />{:else}<IconShieldCheck size={14} />{/if}
          {view.label}
        </button>
      {/each}
    </div>
  </div>

  {#if activeView === "release"}
    <div id="publish-panel-release" class="release-panel" role="tabpanel" aria-labelledby="publish-tab-release">
      <section class="preflight-card" aria-labelledby="preflight-title">
        <header>
          <div><span>{t("publish-quality-gates")}</span><h2 id="preflight-title">{t("publish-preflight-title")}</h2></div>
          <button class="ui-button toolbar" type="button" disabled={preflightRunning} onclick={() => { void runPreflight(); }}>
            <IconRefresh class={preflightRunning ? "spin" : undefined} size={14} />
            {preflightRunning ? t("publish-checking") : t("publish-run-preflight")}
          </button>
        </header>
        <div class="gate-list">
          <article class:passed={sourceSaved} class:failed={!sourceSaved}>
            <span class="gate-icon">{#if sourceSaved}<IconCircleCheck size={17} />{:else}<IconAlertTriangle size={17} />{/if}</span>
            <div><strong>{t("publish-sources-saved")}</strong><p>{sourceSaved
              ? t("publish-sources-synced")
              : t("publish-unsaved-areas", { count: app.globalDirtyState.areas.length })}</p></div>
            {#if !sourceSaved}<button class="ui-button toolbar" type="button" disabled={!app.globalDirtyState.canSave} onclick={() => { void saveSession(); }}><IconDeviceFloppy size={13} /> {t("publish-save")}</button>{/if}
          </article>
          <article class:passed={auditCurrent && auditErrors === 0} class:failed={!auditCurrent || auditErrors > 0}>
            <span class="gate-icon">{#if auditCurrent && auditErrors === 0}<IconCircleCheck size={17} />{:else}<IconAlertTriangle size={17} />{/if}</span>
            <div><strong>{t("publish-project-audit")}</strong><p>{auditCurrent
              ? t("publish-audit-summary", {
                errors: l10n.formatNumber(auditErrors),
                warnings: l10n.formatNumber(auditWarnings),
              })
              : t("publish-audit-stale")}</p></div>
            <button class="ui-button toolbar" type="button" onclick={() => { void app.setWorkbenchActivity("audit"); }}>{t("publish-open-audit")}</button>
          </article>
          <article class:passed={validationValid} class:failed={!validationValid}>
            <span class="gate-icon">{#if validationValid}<IconCircleCheck size={17} />{:else}<IconHammer size={17} />{/if}</span>
            <div><strong>{validationLabel()}</strong><p>{app.controlledPreview.validationMessage || t("publish-validation-help")}</p></div>
            <button class="ui-button toolbar" type="button" disabled={preflightRunning} onclick={() => { void runPreflight(); }}>{t("publish-check")}</button>
          </article>
          <article class="target-gate">
            <span class="gate-icon"><IconCloudUpload size={17} /></span>
            <div><strong>{t("publish-bunny-target")}</strong><p>{t("publish-bunny-description")}</p></div>
            <button class="ui-button toolbar" type="button" onclick={() => selectView("configuration")}>{t("publish-configure")}</button>
          </article>
        </div>
      </section>

      <aside class="release-actions" aria-labelledby="release-actions-title">
        <div class="release-copy">
          <span>{t("publish-build-and-release")}</span>
          <h2 id="release-actions-title">{t("publish-release-current")}</h2>
          <p>{t("publish-release-description")}</p>
        </div>
        {#if !releaseReady}
          <div class="release-warning" role="status"><IconAlertTriangle size={15} /><span>{t("publish-gates-warning")}</span></div>
        {/if}
        <DeployPane
          scannedProject={!!app.scannedProject}
          cachebustAssets={app.cachebustAssets}
          projectRoot={app.sessionProjectRoot}
          runtimeSessionId={app.kernelProjectSessionId}
          workspaceMode
          actionsOnly
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as import("$lib/status/global-status").GlobalStatusKind)}
          onCachebustAssetsChange={(value) => { app.cachebustAssets = value; }}
        />
        <button class="ui-button toolbar output-link" type="button" onclick={() => { void app.openAuditWorkspace("runtime", true); }}>{t("publish-open-log")}</button>
      </aside>
    </div>
  {:else}
    <div id="publish-panel-configuration" class="configuration-panel" role="tabpanel" aria-labelledby="publish-tab-configuration">
      <header>
        <div><span>{t("publish-config-sources")}</span><h2>{t("publish-config-title")}</h2><p>{t("publish-config-description")}</p></div>
      </header>
      <div class="configuration-scroll">
        <DeployPane
          scannedProject={!!app.scannedProject}
          cachebustAssets={app.cachebustAssets}
          projectRoot={app.sessionProjectRoot}
          runtimeSessionId={app.kernelProjectSessionId}
          workspaceMode
          onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind as import("$lib/status/global-status").GlobalStatusKind)}
          onCachebustAssetsChange={(value) => { app.cachebustAssets = value; }}
        />
      </div>
    </div>
  {/if}
</section>

<style>
  .release-state, .preflight-card > header, .gate-list article, .gate-icon, .release-warning { display: flex; align-items: center; }
  .release-state { min-width: 194px; gap: 9px; padding: 9px 11px; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-control); background: var(--wb-surface-document); }
  .release-state.ready { border-color: color-mix(in srgb, var(--success) 48%, var(--wb-border-subtle)); color: var(--success); }
  .release-state.blocked { border-color: color-mix(in srgb, var(--wb-warning) 48%, var(--wb-border-subtle)); color: var(--wb-warning); }
  .release-state > div { display: grid; gap: 2px; }
  .release-state span, .release-copy > span, .configuration-panel > header span, .preflight-card > header span { color: var(--wb-text-muted); font-size: 12px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  .release-state strong { color: var(--text-strong); font-size: 12px; }
  .release-panel { display: grid; grid-template-columns: minmax(430px, 1fr) minmax(330px, .72fr); gap: 11px; min-width: 0; min-height: 0; padding: 11px; overflow: auto; }
  .preflight-card, .release-actions, .configuration-panel { min-width: 0; border: 1px solid var(--wb-border-subtle); border-radius: var(--radius-panel); background: var(--wb-surface-document); }
  .preflight-card { overflow: hidden; align-self: start; }
  .preflight-card > header { justify-content: space-between; gap: 10px; min-height: 58px; padding: 9px 11px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .preflight-card > header > div { display: grid; gap: 3px; }
  h2 { margin: 0; color: var(--text-strong); font-size: 14px; }
  .gate-list { display: grid; }
  .gate-list article { display: grid; grid-template-columns: 30px minmax(0, 1fr) auto; align-items: center; gap: 8px; min-height: 72px; padding: 10px 11px; border-bottom: 1px solid var(--wb-border-subtle); border-left: 3px solid var(--wb-border-strong); }
  .gate-list article:last-child { border-bottom: 0; }
  .gate-list article.passed { border-left-color: var(--success); }
  .gate-list article.failed { border-left-color: var(--wb-warning); }
  .gate-icon { width: 28px; height: 28px; justify-content: center; border-radius: 7px; color: var(--wb-text-muted); background: var(--surface-4); }
  article.passed .gate-icon { color: var(--success); background: color-mix(in srgb, var(--success) 9%, var(--wb-surface-document)); }
  article.failed .gate-icon { color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 9%, var(--wb-surface-document)); }
  .gate-list article > div { min-width: 0; }
  .gate-list strong { color: var(--text-strong); font-size: 12px; }
  .gate-list p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; }
  .release-actions { align-self: start; padding: 15px; background: var(--wb-surface-chrome); }
  .release-copy { display: grid; gap: 5px; margin-bottom: 12px; }
  .release-copy h2 { font-size: 16px; }
  .release-copy p, .configuration-panel > header p { margin: 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.45; }
  .release-warning { align-items: flex-start; gap: 7px; margin-bottom: 10px; padding: 8px; border: 1px solid color-mix(in srgb, var(--wb-warning) 40%, var(--wb-border-subtle)); border-radius: 7px; color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 7%, var(--wb-surface-document)); font-size: 12px; line-height: 1.4; }
  .output-link { width: 100%; margin-top: 9px; }
  .configuration-panel { display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 0; margin: 11px; overflow: hidden; }
  .configuration-panel > header { padding: 14px 16px; border-bottom: 1px solid var(--wb-border-subtle); background: var(--wb-surface-chrome); }
  .configuration-panel > header > div { display: grid; gap: 4px; }
  .configuration-scroll { min-height: 0; padding: 12px 16px 24px; overflow: auto; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { cursor: default; opacity: .55; }
  button:focus-visible { outline: 2px solid var(--wb-focus-ring); outline-offset: 1px; }
  :global(.spin) { animation: publish-spin .8s linear infinite; }
  @keyframes publish-spin { to { transform: rotate(360deg); } }
  @media (max-width: 980px) { .release-panel { grid-template-columns: 1fr; } .release-state { display: none; } }
</style>
