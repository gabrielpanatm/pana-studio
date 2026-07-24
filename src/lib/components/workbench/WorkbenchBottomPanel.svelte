<script lang="ts">
  import {
    IconActivity,
    IconAlertTriangle,
    IconCircleCheck,
    IconExternalLink,
    IconRefresh,
    IconTerminal2,
    IconTimeline,
    IconX,
  } from "@tabler/icons-svelte";
  import { tick, type Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import MotionTimelinePanel from "$lib/components/workspace/MotionTimelinePanel.svelte";
  import { formatKernelLogTime } from "$lib/kernel/observability-log-control";
  import { readKernelObservabilityLog } from "$lib/project/io";
  import type { AppState } from "$lib/state/app.svelte";
  import type { TerminalQuickTask } from "$lib/terminal/runtime";
  import type {
    AuditDiagnostic,
    KernelObservabilityLogSnapshot,
    WorkbenchBottomPanelView,
  } from "$lib/types";

  let {
    app,
    TerminalPaneComponent = null,
    openWorkspaceSource,
  }: {
    app: AppState;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  type PanelTab = {
    id: WorkbenchBottomPanelView;
    label: string;
  };

  const tabs: PanelTab[] = [
    { id: "problems", label: "Probleme" },
    { id: "output", label: "Jurnal" },
    { id: "terminal", label: "Terminal" },
    { id: "timeline", label: "Cronologie" },
  ];

  const activeView = $derived(
    app.workbenchSnapshot?.bottomPanel.activeView ?? "problems",
  );
  const auditDiagnostics = $derived(app.projectAuditSnapshot?.diagnostics ?? []);
  const validationProblem = $derived(
    app.controlledPreview.validation === "invalid"
      || app.controlledPreview.validation === "error",
  );
  const diskProblem = $derived(Boolean(app.immediateDiskOperationBlockedReason));
  const problemCount = $derived(
    auditDiagnostics.length
      + Number(validationProblem)
      + Number(diskProblem)
      + Number(Boolean(app.projectAuditError)),
  );
  const timelineAvailable = $derived(
    (app.workbenchSnapshot?.activeActivity ?? "editor") === "editor"
      && app.centerView === "preview"
      && Boolean(app.activeRenderedTemplatePath)
      && Boolean(app.scannedProject?.isZola),
  );

  let outputSnapshot = $state<KernelObservabilityLogSnapshot | null>(null);
  let outputLoading = $state(false);
  let outputError = $state("");
  let outputRequestSerial = 0;
  let outputLoadedIdentity = "";
  let timelineMountReady = $state(false);

  $effect(() => {
    const shouldLoad = activeView === "problems";
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    const workspaceRevision = app.projectWorkspaceSnapshot?.revision;
    if (!shouldLoad || !projectRoot || !runtimeSessionId || workspaceRevision === undefined) return;
    void app.refreshProjectAudit();
  });

  $effect(() => {
    const shouldLoad = activeView === "output";
    const projectRoot = app.sessionProjectRoot;
    const runtimeSessionId = app.kernelProjectSessionId;
    if (!shouldLoad || !projectRoot || !runtimeSessionId) return;
    void refreshOutput(projectRoot, runtimeSessionId);
  });

  $effect(() => {
    const shouldMount = activeView === "timeline" && timelineAvailable;
    if (!shouldMount) {
      timelineMountReady = false;
      return;
    }
    clearTransientInteractionLocks();
    timelineMountReady = false;
    let secondFrame = 0;
    const firstFrame = window.requestAnimationFrame(() => {
      secondFrame = window.requestAnimationFrame(() => {
        timelineMountReady = true;
      });
    });
    return () => {
      window.cancelAnimationFrame(firstFrame);
      if (secondFrame) window.cancelAnimationFrame(secondFrame);
    };
  });

  async function refreshOutput(
    expectedProjectRoot = app.sessionProjectRoot,
    expectedRuntimeSessionId = app.kernelProjectSessionId,
  ) {
    const serial = ++outputRequestSerial;
    const identity = `${expectedProjectRoot}\u0000${expectedRuntimeSessionId}`;
    if (identity !== outputLoadedIdentity) outputSnapshot = null;
    outputLoading = true;
    outputError = "";
    try {
      const snapshot = await readKernelObservabilityLog(
        120,
        false,
        false,
        ["info", "warn", "error"],
        "active",
      );
      if (
        serial !== outputRequestSerial
        || app.sessionProjectRoot !== expectedProjectRoot
        || app.kernelProjectSessionId !== expectedRuntimeSessionId
      ) return;
      outputSnapshot = snapshot;
      outputLoadedIdentity = identity;
    } catch (error) {
      if (serial !== outputRequestSerial) return;
      outputError = error instanceof Error ? error.message : String(error);
    } finally {
      if (serial === outputRequestSerial) outputLoading = false;
    }
  }

  function clearTransientInteractionLocks() {
    document.body.classList.remove(
      "is-resizing",
      "is-col-resizing",
      "is-row-resizing",
      "motion-timeline-dragging",
    );
    document.documentElement.style.userSelect = "";
  }

  async function flushTimelineBeforeLeave() {
    if (activeView !== "timeline") return true;
    try {
      await app.flushInteractiveEditorDrafts("template-switch");
      return true;
    } catch (error) {
      app.setGlobalStatus(
        `Timeline nu a putut fi închis: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
      return false;
    }
  }

  async function selectView(view: WorkbenchBottomPanelView) {
    if (view === activeView) return;
    if (!(await flushTimelineBeforeLeave())) return;
    clearTransientInteractionLocks();
    await app.setWorkbenchBottomPanel(true, view);
    await tick();
  }

  async function focusTab(index: number) {
    const tab = tabs[index];
    if (!tab) return;
    await selectView(tab.id);
    await tick();
    document.getElementById(`bottom-panel-tab-${tab.id}`)?.focus();
  }

  function handleTabKeydown(event: KeyboardEvent, index: number) {
    let nextIndex: number | null = null;
    if (event.key === "ArrowLeft") {
      nextIndex = (index - 1 + tabs.length) % tabs.length;
    } else if (event.key === "ArrowRight") {
      nextIndex = (index + 1) % tabs.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = tabs.length - 1;
    }
    if (nextIndex === null) return;
    event.preventDefault();
    void focusTab(nextIndex);
  }

  async function closePanel() {
    if (!(await flushTimelineBeforeLeave())) return;
    await app.setWorkbenchBottomPanel(false, activeView);
  }

  async function openDiagnostic(diagnostic: AuditDiagnostic) {
    if (!diagnostic.file) return;
    await openWorkspaceSource(diagnostic.file);
  }

  function tabCount(tab: WorkbenchBottomPanelView) {
    if (tab === "problems") return problemCount;
    if (tab === "output") return outputSnapshot?.returnedCount ?? 0;
    if (tab === "terminal") return app.terminalTabs.length;
    return 0;
  }
</script>

<section class="bottom-panel" aria-label="Panoul inferior al spațiului de lucru">
  <header class="panel-header">
    <div class="panel-tabs" role="tablist" aria-label="Vizualizări panou inferior">
      {#each tabs as tab, index (tab.id)}
        <button
          id={`bottom-panel-tab-${tab.id}`}
          type="button"
          role="tab"
          class:active={activeView === tab.id}
          aria-selected={activeView === tab.id ? "true" : "false"}
          aria-controls={`bottom-panel-view-${tab.id}`}
          tabindex={activeView === tab.id ? 0 : -1}
          onclick={() => { void selectView(tab.id); }}
          onkeydown={(event) => { handleTabKeydown(event, index); }}
        >
          {#if tab.id === "problems"}
            <IconAlertTriangle size={14} stroke={1.9} />
          {:else if tab.id === "output"}
            <IconActivity size={14} stroke={1.9} />
          {:else if tab.id === "terminal"}
            <IconTerminal2 size={14} stroke={1.9} />
          {:else}
            <IconTimeline size={14} stroke={1.9} />
          {/if}
          <span>{tab.label}</span>
          {#if tabCount(tab.id) > 0}
            <small>{tabCount(tab.id)}</small>
          {/if}
        </button>
      {/each}
    </div>

    <div class="panel-actions">
      {#if activeView === "output"}
        <button
          type="button"
          title="Reîncarcă jurnalul"
          aria-label="Reîncarcă jurnalul"
          disabled={outputLoading}
          onclick={() => { void refreshOutput(); }}
        >
          <IconRefresh class={outputLoading ? "spin" : undefined} size={15} stroke={1.9} />
        </button>
      {:else if activeView === "problems"}
        <button
          type="button"
          title="Reanalizează problemele"
          aria-label="Reanalizează problemele"
          disabled={app.projectAuditLoading}
          onclick={() => { void app.refreshProjectAudit(true); }}
        >
          <IconRefresh class={app.projectAuditLoading ? "spin" : undefined} size={15} stroke={1.9} />
        </button>
      {/if}
      <button
        type="button"
        title="Închide panoul inferior"
        aria-label="Închide panoul inferior"
        onclick={() => { void closePanel(); }}
      >
        <IconX size={15} stroke={2} />
      </button>
    </div>
  </header>

  <div
    id={`bottom-panel-view-${activeView}`}
    class="panel-body"
    role="tabpanel"
    aria-labelledby={`bottom-panel-tab-${activeView}`}
  >
    {#if activeView === "problems"}
      <section class="problems-view" aria-label="Problems">
        <div class:clean={problemCount === 0} class="problems-summary">
          {#if problemCount === 0}
            <IconCircleCheck size={18} stroke={1.9} />
            <span><strong>Fără probleme cunoscute</strong>Auditul Rust și validarea curentă nu raportează probleme.</span>
          {:else}
            <IconAlertTriangle size={18} stroke={1.9} />
            <span><strong>{problemCount} {problemCount === 1 ? "problemă" : "probleme"}</strong>Sesiunea proiectului, construirea și validarea Zola curentă.</span>
          {/if}
          <div>
            <button type="button" onclick={() => { void app.runZolaValidation("manual"); }}>Validează cu Zola embedded</button>
            <button type="button" onclick={() => { void app.setWorkbenchActivity("audit"); }}>
              Deschide Audit <IconExternalLink size={13} stroke={1.9} />
            </button>
          </div>
        </div>

        <div class="problem-list">
          {#if app.projectAuditError}
            <article class="error">
              <span>Audit</span>
              <div><strong>Proiecția Audit nu este disponibilă</strong><p>{app.projectAuditError}</p></div>
              <button type="button" onclick={() => { void app.refreshProjectAudit(true); }}>Reîncearcă</button>
            </article>
          {:else if app.projectAuditLoading && !app.projectAuditSnapshot}
            <div class="panel-state compact">Se construiește auditul din sesiunea proiectului…</div>
          {/if}
          {#if diskProblem}
            <article class="error">
              <span>Disc</span>
              <div><strong>Operație blocată</strong><p>{app.immediateDiskOperationBlockedReason}</p></div>
            </article>
          {/if}
          {#if validationProblem}
            <article class="error">
              <span>Zola</span>
              <div><strong>{app.controlledPreview.validation === "invalid" ? "Proiect invalid" : "Validare eșuată"}</strong><p>{app.controlledPreview.validationMessage}</p></div>
            </article>
          {/if}
          {#each auditDiagnostics as diagnostic (diagnostic.id)}
            <article
              class={diagnostic.severity}
              aria-label={`${diagnostic.title}. ${diagnostic.message}. ${diagnostic.file ?? "Proiect"}${diagnostic.range ? `, linia ${diagnostic.range.line}` : ""}`}
            >
              <span>{diagnostic.severity === "error" ? "Error" : diagnostic.severity === "warning" ? "Warn" : "Info"}</span>
              <div>
                <strong>
                  {diagnostic.title} · {diagnostic.file ?? "Proiect"}{diagnostic.range ? `:${diagnostic.range.line}` : ""}
                </strong>
                <p>{diagnostic.message}</p>
              </div>
              {#if diagnostic.file}
                <button type="button" onclick={() => { void openDiagnostic(diagnostic); }}>Deschide</button>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {:else if activeView === "output"}
      <section class="output-view" aria-label="Jurnal operațional Rust">
        {#if outputError}
          <div class="panel-state error" role="alert">{outputError}</div>
        {:else if outputLoading && !outputSnapshot}
          <div class="panel-state">Se citește logul operațional Rust…</div>
        {:else if outputSnapshot && outputSnapshot.events.length > 0}
          <div class="output-list">
            {#each outputSnapshot.events as event (event.id)}
              <article class={event.level}>
                <time>{formatKernelLogTime(event.timestampMs)}</time>
                <span>{event.level.toUpperCase()}</span>
                <strong>{event.owner} · {event.eventName}</strong>
                <p>{event.diagnostic || event.message}</p>
              </article>
            {/each}
          </div>
        {:else}
          <div class="panel-state">Kernel-ul nu a înregistrat încă evenimente pentru sesiunea activă.</div>
        {/if}
      </section>
    {:else if activeView === "terminal"}
      {#if TerminalPaneComponent}
        <TerminalPaneComponent
          bind:terminalHost={app.terminalHost}
          terminalTabs={app.terminalTabs}
          activeTerminalTabId={app.activeTerminalTabId}
          quickTasks={app.terminalQuickTasks}
          openTab={() => app.openTerminalTab()}
          selectTab={(id: string) => app.selectTerminalTab(id)}
          closeTab={(id: string) => app.closeTerminalTab(id)}
          runQuickTask={(task: TerminalQuickTask) => app.runTerminalQuickTask(task)}
          clearActiveTerminal={() => app.clearActiveTerminal()}
          closePane={closePanel}
        />
      {:else}
        <div class="panel-state">Se încarcă Terminalul…</div>
      {/if}
    {:else if timelineAvailable}
      {#if timelineMountReady}
        <div class="timeline-view" inert={app.aiEditLeaseFrontendLockActive ? true : undefined}>
          <MotionTimelinePanel
            activeTemplatePath={app.activeRenderedTemplatePath}
            projectRoot={app.sessionProjectRoot}
            runtimeSessionId={app.kernelProjectSessionId}
            refreshToken={app.jsRefreshToken}
            onPendingChange={(pending) => app.setInspectorPending("js", pending, "motion-timeline")}
          />
        </div>
      {:else}
        <div class="panel-state">Se pregătește cronologia…</div>
      {/if}
    {:else}
      <div class="panel-state">
        Cronologia este disponibilă pentru un template deschis în suprafața Vizual.
        <button type="button" onclick={() => { void app.setWorkbenchActivity("editor"); }}>Deschide editorul</button>
      </div>
    {/if}
  </div>
</section>

<style>
  .bottom-panel {
    display: grid;
    grid-template-rows: 34px minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--wb-border-subtle, var(--border));
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--wb-surface-document, var(--surface));
  }

  .panel-header,
  .panel-tabs,
  .panel-actions,
  .panel-header button,
  .problems-summary,
  .problems-summary > span,
  .problems-summary > div,
  .problem-list article,
  .output-list article {
    display: flex;
    align-items: center;
  }

  .panel-header {
    justify-content: space-between;
    min-width: 0;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    background: var(--wb-surface-chrome, var(--surface-2));
  }

  .panel-tabs {
    align-self: stretch;
    min-width: 0;
    overflow: auto hidden;
  }

  .panel-header button {
    justify-content: center;
    gap: 5px;
    height: 100%;
    padding: 0 10px;
    border: 0;
    border-radius: 0;
    color: var(--wb-text-muted, var(--text-muted));
    background: transparent;
    font-size: 12px;
    font-weight: 750;
  }

  .panel-tabs button {
    position: relative;
  }

  .panel-tabs button::after {
    content: "";
    position: absolute;
    right: 8px;
    bottom: 0;
    left: 8px;
    height: 2px;
    border-radius: 999px 999px 0 0;
    background: transparent;
  }

  .panel-tabs button.active {
    color: var(--wb-text-primary, var(--text));
  }

  .panel-tabs button.active::after {
    background: var(--wb-accent, var(--brand));
  }

  .panel-tabs button:hover,
  .panel-actions button:hover:not(:disabled) {
    color: var(--wb-text-primary, var(--text));
    background: var(--wb-control-hover, var(--brand-soft));
  }

  .panel-tabs small {
    min-width: 16px;
    padding: 1px 4px;
    border-radius: 999px;
    color: var(--wb-text-muted, var(--text-muted));
    background: var(--surface-4);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .panel-actions {
    align-self: stretch;
  }

  .panel-actions button {
    width: 32px;
    padding: 0;
  }

  .panel-body,
  .problems-view,
  .output-view,
  .timeline-view {
    min-width: 0;
    min-height: 0;
  }

  .panel-body {
    overflow: hidden;
  }

  .problems-view,
  .output-view {
    height: 100%;
    overflow: auto;
  }

  .problems-summary {
    position: sticky;
    z-index: 1;
    top: 0;
    gap: 9px;
    min-height: 48px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    color: #d97706;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(8px);
  }

  .problems-summary.clean {
    color: #10b981;
  }

  .problems-summary > span {
    align-items: flex-start;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
  }

  .problems-summary strong {
    color: var(--wb-text-primary, var(--text));
    font-size: 12px;
  }

  .problems-summary > div {
    gap: 6px;
  }

  .problems-summary button,
  .problem-list button,
  .panel-state button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 26px;
    padding: 0 8px;
    border: 1px solid var(--wb-border-subtle, var(--border-3));
    border-radius: 5px;
    color: var(--wb-text-primary, var(--text));
    background: var(--surface-4);
    font-size: 12px;
  }

  .problem-list {
    display: grid;
  }

  .problem-list article {
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr) auto;
    gap: 9px;
    min-height: 44px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    border-left: 3px solid var(--wb-accent, var(--brand));
  }

  .problem-list article.error {
    border-left-color: var(--danger, #ef4444);
  }

  .problem-list article.warning {
    border-left-color: var(--wb-warning, #d97706);
  }

  .problem-list article > span {
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
    font-weight: 850;
    text-transform: uppercase;
  }

  .problem-list article > div {
    min-width: 0;
  }

  .problem-list strong,
  .problem-list p {
    display: block;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .problem-list strong {
    color: var(--wb-text-primary, var(--text));
    font-size: 12px;
  }

  .problem-list p {
    margin-top: 3px;
    color: var(--wb-text-muted, var(--text-muted));
    font-size: 12px;
  }

  .output-list {
    display: grid;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  }

  .output-list article {
    display: grid;
    grid-template-columns: 72px 42px minmax(150px, 0.35fr) minmax(240px, 1fr);
    gap: 8px;
    min-height: 30px;
    padding: 5px 10px;
    border-bottom: 1px solid var(--wb-border-subtle, var(--border));
    border-left: 3px solid var(--wb-accent, var(--brand));
    font-size: 12px;
  }

  .output-list article.warn { border-left-color: #d97706; }
  .output-list article.error { border-left-color: #ef4444; }
  .output-list time,
  .output-list span { color: var(--wb-text-muted, var(--text-muted)); }
  .output-list strong,
  .output-list p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .panel-state {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    padding: 18px;
    color: var(--wb-text-muted, var(--text-muted));
    text-align: center;
    font-size: 12px;
  }

  .panel-state.error {
    color: var(--danger, #ef4444);
  }

  .panel-state.compact {
    min-height: 44px;
    height: auto;
    padding: 9px;
  }

  .timeline-view {
    height: 100%;
    overflow: hidden;
  }

  :global(.panel-body > .terminal-pane) {
    height: 100%;
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }

  :global(.timeline-view > .motion-timeline-pane-shell) {
    height: 100%;
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }

  :global(.spin) {
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
