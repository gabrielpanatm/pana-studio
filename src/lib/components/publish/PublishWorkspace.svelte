<script lang="ts">
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconCloudUpload,
    IconDeviceFloppy,
    IconRefresh,
    IconRocket,
    IconSettings,
    IconShieldCheck,
  } from "@tabler/icons-svelte";
  import DeployPane from "$lib/components/DeployPane.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    PublishPreflightGate,
    PublishPreflightRemediation,
    WorkspaceSourceOpenOptions,
  } from "$lib/types";
  import { errorMessage } from "$lib/util";

  let {
    app,
    openWorkspaceSource,
  }: {
    app: AppState;
    openWorkspaceSource: (
      path: string,
      options?: WorkspaceSourceOpenOptions,
    ) => void | Promise<void>;
  } = $props();

  type PublishView = "release" | "configuration";
  const views = $derived([
    { id: "release" as const, label: t("publish-view-release") },
    { id: "configuration" as const, label: t("publish-view-configuration") },
  ]);

  let activeView = $state<PublishView>("release");
  let preflightRunning = $state(false);
  let authorityLoadKey = $state("");

  const preflight = $derived(app.currentPublishPreflightReceipt());
  const releaseReady = $derived(preflight?.status === "ready");

  $effect(() => {
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    const diskGeneration = app.projectWorkspaceSnapshot?.diskGeneration;
    const diskWatchRevision = app.externalDiskWatchRevision;
    const key = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision ?? ""}\u0000${diskGeneration ?? ""}\u0000${diskWatchRevision}`;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === undefined || key === authorityLoadKey) return;
    authorityLoadKey = key;
    app.invalidatePublishAuthorization();
    void app.refreshPublishAuthorization().catch((error) => {
      app.setGlobalStatus(t("publish-authorization-refresh-failed", { error: errorMessage(error) }), "error");
    });
  });

  function statusLabel() {
    if (!preflight) return t("publish-preflight-not-run");
    switch (preflight.status) {
      case "ready": return t("publish-preflight-status-ready");
      case "blocked": return t("publish-preflight-status-blocked");
      case "failed": return t("publish-preflight-status-failed");
    }
  }

  function gateOutcomeLabel(gate: PublishPreflightGate) {
    switch (gate.outcome) {
      case "passed": return t("publish-preflight-outcome-passed");
      case "blocked": return t("publish-preflight-outcome-blocked");
      case "advisory": return t("publish-preflight-outcome-advisory");
      case "skipped": return t("publish-preflight-outcome-skipped");
      case "engine_error": return t("publish-preflight-outcome-engine-error");
    }
  }

  function preflightResultLabel(status: "ready" | "blocked" | "failed") {
    switch (status) {
      case "ready": return t("publish-preflight-result-ready");
      case "blocked": return t("publish-preflight-result-blocked");
      case "failed": return t("publish-preflight-result-failed");
    }
  }

  async function runPreflight() {
    if (preflightRunning) return;
    preflightRunning = true;
    try {
      const receipt = await app.runPublishPreflight();
      app.setGlobalStatus(
        preflightResultLabel(receipt.status),
        receipt.status === "ready" ? "saved" : receipt.status === "blocked" ? "unsaved" : "error",
      );
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
    await app.saveActiveFile();
    app.invalidatePublishAuthorization();
  }

  async function openAuditFinding(fingerprint: string) {
    const finding = preflight?.auditReceipt.findings.find((item) => item.fingerprint === fingerprint);
    const location = finding?.primaryLocation;
    if (!location) {
      await app.setWorkbenchActivity("audit");
      return;
    }
    await openWorkspaceSource(location.file, { surface: "code" });
    if (location.range) app.revealSourceRange(location.file, location.range);
  }

  async function runRemediation(remediation: PublishPreflightRemediation) {
    switch (remediation.kind) {
      case "save_workspace":
        await saveSession();
        return;
      case "open_audit":
        await app.setWorkbenchActivity("audit");
        return;
      case "open_source":
        if (remediation.location) {
          await openWorkspaceSource(remediation.location.file, { surface: "code" });
          if (remediation.location.range) {
            app.revealSourceRange(remediation.location.file, remediation.location.range);
          }
        }
        return;
      case "configure_deploy":
      case "configure_credentials":
        selectView("configuration");
        return;
      case "reconcile_disk":
        app.setGlobalStatus(t("publish-reconcile-disk-guidance"), "unsaved");
        return;
      case "retry":
        await runPreflight();
    }
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
      <div><span>{t("publish-state")}</span><strong>{statusLabel()}</strong></div>
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
          {#if preflight}
            {#each preflight.gates as gate (gate.id)}
              <article
                class:passed={gate.outcome === "passed"}
                class:failed={gate.outcome === "blocked" || gate.outcome === "engine_error"}
                class:advisory={gate.outcome === "advisory" || gate.outcome === "skipped"}
              >
                <span class="gate-icon">{#if gate.outcome === "passed"}<IconCircleCheck size={17} />{:else if gate.id === "deploy_target" || gate.id === "deploy_credentials"}<IconCloudUpload size={17} />{:else}<IconAlertTriangle size={17} />{/if}</span>
                <div class="gate-copy">
                  <strong>{errorMessage(gate.diagnostic)}</strong>
                  <p>{gateOutcomeLabel(gate)}</p>
                  {#each gate.evidence as item (`${gate.id}-${item.kind}-${item.diagnostic.code}`)}
                    <small>{errorMessage(item.diagnostic)}{item.value ? `: ${item.value}` : ""}</small>
                  {/each}
                  {#if gate.auditFingerprints.length}
                    <div class="finding-links">
                      {#each gate.auditFingerprints as fingerprint}
                        <button type="button" onclick={() => { void openAuditFinding(fingerprint); }}>{t("publish-open-finding")}</button>
                      {/each}
                    </div>
                  {/if}
                </div>
                <div class="gate-actions">
                  {#each gate.remediations as remediation (`${gate.id}-${remediation.kind}`)}
                    <button class="ui-button toolbar" type="button" onclick={() => { void runRemediation(remediation); }}>
                      {#if remediation.kind === "save_workspace"}<IconDeviceFloppy size={13} />{/if}
                      {errorMessage(remediation.diagnostic)}
                    </button>
                  {/each}
                </div>
              </article>
            {/each}
          {:else}
            <div class="preflight-empty"><IconShieldCheck size={18} /><p>{t("publish-preflight-empty")}</p></div>
          {/if}
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
          {app}
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
          {app}
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
  .gate-list article.advisory { border-left-color: var(--wb-accent); }
  .gate-icon { width: 28px; height: 28px; justify-content: center; border-radius: 7px; color: var(--wb-text-muted); background: var(--surface-4); }
  article.passed .gate-icon { color: var(--success); background: color-mix(in srgb, var(--success) 9%, var(--wb-surface-document)); }
  article.failed .gate-icon { color: var(--wb-warning); background: color-mix(in srgb, var(--wb-warning) 9%, var(--wb-surface-document)); }
  article.advisory .gate-icon { color: var(--wb-accent); background: color-mix(in srgb, var(--wb-accent) 9%, var(--wb-surface-document)); }
  .gate-list article > div { min-width: 0; }
  .gate-list strong { color: var(--text-strong); font-size: 12px; }
  .gate-list p { margin: 4px 0 0; color: var(--wb-text-muted); font-size: 12px; line-height: 1.35; }
  .gate-copy small { display: block; margin-top: 4px; color: var(--wb-text-muted); font-size: 11px; line-height: 1.35; overflow-wrap: anywhere; }
  .gate-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 5px; }
  .finding-links { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 6px; }
  .finding-links button { padding: 0; border: 0; color: var(--wb-accent); background: transparent; font: inherit; font-size: 11px; text-decoration: underline; }
  .preflight-empty { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 108px; padding: 16px; color: var(--wb-text-muted); }
  .preflight-empty p { margin: 0; }
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
