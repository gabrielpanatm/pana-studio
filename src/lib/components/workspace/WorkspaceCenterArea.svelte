<script lang="ts">
  import { tick, type Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import EditorShell from "$lib/components/EditorShell.svelte";
  import MoodBoardCanvas from "$lib/components/canvas/MoodBoardCanvas.svelte";
  import KernelWorkspace from "$lib/components/kernel/KernelWorkspace.svelte";
  import SiteWorkspace from "$lib/components/site-workspace/SiteWorkspace.svelte";
  import MotionTimelinePanel from "$lib/components/workspace/MotionTimelinePanel.svelte";
  import StartupState from "$lib/components/workspace/StartupState.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type { MoodBoardRequestIdentity } from "$lib/mood-board/io";
  import { requireCurrentPreviewStructuralSession } from "$lib/kernel/preview-structural-lane";
  import { syncCommittedSiteStructurePreview } from "$lib/source-graph/template-actions";
  import type { TerminalQuickTask } from "$lib/terminal/runtime";

  let {
    app,
    TerminalPaneComponent = null,
    breakpointValue,
    openWorkspaceSource,
  }: {
    app: AppState;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
    breakpointValue: (name: string, fallback: string) => string;
    openWorkspaceSource: (path: string) => void | Promise<void>;
  } = $props();

  let motionTimelineExpanded = $state(false);
  let motionTimelineMountReady = $state(false);

  const motionTimelineAvailable = $derived(
    app.centerView === "preview"
      && app.activeInspectorTab === "js"
      && !app.rightPaneCollapsed
      && Boolean(app.scannedProject?.isZola),
  );
  const motionTimelineOpen = $derived(motionTimelineAvailable && motionTimelineExpanded);
  const moodBoardSessionIdentity = $derived.by((): MoodBoardRequestIdentity | null => {
    if (
      app.projectTransitionFrontendLeaseActive
      || app.kernelUndoRedoFrontendLeaseActive
      || app.aiEditLeaseFrontendLockActive
    ) return null;
    if (!app.currentProjectPath || !app.kernelProjectSessionId) return null;
    if (
      app.moodBoardLoadedForRoot !== app.currentProjectPath
      || app.moodBoardLoadedForSessionId !== app.kernelProjectSessionId
    ) return null;
    return {
      expectedProjectRoot: app.currentProjectPath,
      expectedSessionId: app.kernelProjectSessionId,
    };
  });

  function isMoodBoardSessionCurrent(identity: MoodBoardRequestIdentity) {
    return !app.projectTransitionFrontendLeaseActive
      && !app.kernelUndoRedoFrontendLeaseActive
      && !app.aiEditLeaseFrontendLockActive
      && app.currentProjectPath === identity.expectedProjectRoot
      && app.kernelProjectSessionId === identity.expectedSessionId
      && app.moodBoardLoadedForRoot === identity.expectedProjectRoot
      && app.moodBoardLoadedForSessionId === identity.expectedSessionId;
  }

  $effect(() => {
    if (motionTimelineAvailable) return;
    motionTimelineExpanded = false;
  });

  function clearTransientInteractionLocks() {
    document.body.classList.remove("is-resizing", "is-col-resizing", "is-row-resizing", "motion-timeline-dragging");
    document.documentElement.style.userSelect = "";
  }

  function openMotionTimeline() {
    clearTransientInteractionLocks();
    motionTimelineExpanded = true;
  }

  async function closeMotionTimeline() {
    try {
      await app.flushInteractiveEditorDrafts("template-switch");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      app.setGlobalStatus(`Închiderea Timeline a fost blocată: ${message}`, "error");
      return;
    }
    motionTimelineExpanded = false;
  }

  $effect(() => {
    if (!motionTimelineOpen) {
      motionTimelineMountReady = false;
      return;
    }

    clearTransientInteractionLocks();
    motionTimelineMountReady = false;
    let secondFrame = 0;
    let readyTimer = 0;
    const firstFrame = window.requestAnimationFrame(() => {
      secondFrame = window.requestAnimationFrame(() => {
        readyTimer = window.setTimeout(() => {
          motionTimelineMountReady = true;
        }, 0);
      });
    });

    return () => {
      window.cancelAnimationFrame(firstFrame);
      if (secondFrame) window.cancelAnimationFrame(secondFrame);
      if (readyTimer) window.clearTimeout(readyTimer);
    };
  });
</script>

<section
  class:terminal-open={app.terminalPaneOpen}
  class:motion-timeline-open={motionTimelineAvailable}
  class:motion-timeline-expanded={motionTimelineOpen}
  class="center-stack"
  style={`--terminal-pane-height: ${app.terminalPaneHeight}px; --motion-timeline-pane-height: ${app.motionTimelinePaneHeight}px;`}
  aria-label="Zona centrala"
>
  <div
    class="editor-shell-shell"
    inert={app.aiEditLeaseFrontendLockActive ? true : undefined}
    aria-busy={app.aiEditLeaseFrontendLockActive}
  >
    {#if !app.scannedProject || app.scannedProject.isEmpty || !app.scannedProject.isZola}
      <StartupState
        scannedProject={!!app.scannedProject}
        isEmpty={app.scannedProject?.isEmpty ?? false}
        isZola={app.scannedProject?.isZola ?? false}
        openProjectFolder={() => app.openProjectFolder()}
        initZolaProject={() => app.initZolaProject()}
      />
    {:else if app.centerView === "kernel"}
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
    {:else if app.centerView === "site"}
      <SiteWorkspace
        currentProjectPath={app.currentProjectPath}
        runtimeSessionId={app.kernelProjectSessionId}
        projectSessionEpoch={app.projectSessionEpoch}
        projectTransitionFrontendLeaseActive={app.projectTransitionFrontendLeaseActive || app.aiEditLeaseFrontendLockActive}
        kernelUndoRedoFrontendLeaseActive={app.kernelUndoRedoFrontendLeaseActive}
        activePath={app.activeScannedPath}
        selectedSourceId={app.selectedTemplateSourceId ?? app.selectedElement?.sourceId ?? null}
        refreshToken={app.refreshToken}
        previewSrc={app.previewSrc}
        previewDocumentMarkup={app.previewDocumentMarkup}
        previewDevice={app.previewDevice}
        previewZoom={app.previewZoom}
        tabletPreviewWidth={breakpointValue("bp-tableta", "1024px")}
        mobilePreviewWidth={breakpointValue("bp-mobil", "768px")}
        sourceCache={app.sourceCache}
        openFile={openWorkspaceSource}
        openExternalRun={(route) => app.openCurrentProjectInBrowser(route)}
        onActiveRouteChange={(route) => { app.browserPreviewRoute = route; }}
        beginPreviewStructuralWriteBoundary={() => app.beginPreviewStructuralWriteBoundary()}
        endPreviewStructuralWriteBoundary={() => app.endPreviewStructuralWriteBoundary()}
        projectCommittedSiteStructure={async (lease, touchedFiles, workspaceRevision, preferredRelativePath) => {
          requireCurrentPreviewStructuralSession(app, lease);
          await app.rescanCurrentProjectWithinStructuralLane(
            lease,
            preferredRelativePath ?? app.activeScannedPath,
            { strict: true, deferPreviewRefresh: true },
          );
          requireCurrentPreviewStructuralSession(app, lease);
          // Loading the newly created source can replace SiteWorkspace with
          // EditorShell. Wait for its Design Safe iframe binding before asking
          // the Canvas coordinator to confirm the new workspace revision.
          await tick();
          requireCurrentPreviewStructuralSession(app, lease);
          await syncCommittedSiteStructurePreview(app, lease, touchedFiles, workspaceRevision);
          return app.sourceGraph;
        }}
        loopDefinitions={app.loopDefinitions}
        onRegisterLoop={(definition) => app.registerLoopDefinition(definition)}
        onRemoveLoop={(id) => app.removeLoopDefinition(id)}
        onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
      />
    {:else if app.centerView === "canvas"}
      {#if moodBoardSessionIdentity}
        {#key `${moodBoardSessionIdentity.expectedProjectRoot}\u0000${moodBoardSessionIdentity.expectedSessionId}`}
          <MoodBoardCanvas
            board={app.moodBoard}
            tool={app.moodBoardTool}
            canUndo={app.canUndoMoodBoard}
            canRedo={app.canRedoMoodBoard}
            saveState={app.moodBoardSaveState}
            saveStatus={app.moodBoardSaveStatus}
            scssVariables={app.scssVariables}
            sessionIdentity={moodBoardSessionIdentity}
            isSessionCurrent={isMoodBoardSessionCurrent}
            setTool={(tool) => { app.moodBoardTool = tool; }}
            commitBoard={(board) => app.commitMoodBoard(board)}
            setTransientBoard={(board) => app.setMoodBoardTransient(board)}
            undo={() => app.undoMoodBoard()}
            redo={() => app.redoMoodBoard()}
            applyImageToSelectedElement={async (path) => {
              app.imageSourceValue = path;
              await app.applyImageSourceToHtml(path);
            }}
            applyColorToScssVariable={(color, label, variableName) => app.applyMoodBoardColorToScssVariable(color, label, variableName)}
            onStatusUpdate={(text, kind) => app.setGlobalStatus(text, kind)}
          />
        {/key}
      {:else}
        <section class="mood-board-loading" aria-label="Mood Board se încarcă">
          {#if app.moodBoardSaveState === "error"}
            <strong>Mood Board indisponibil</strong>
            <span>{app.moodBoardSaveStatus}</span>
            <button type="button" onclick={() => { void app.loadMoodBoard(); }}>
              Reîncearcă încărcarea
            </button>
          {:else}
            <strong>Mood Board</strong>
            <span>Se leagă documentul de sesiunea curentă…</span>
          {/if}
        </section>
      {/if}
    {:else}
      <EditorShell
        bind:previewFrame={app.previewFrame}
        bind:codeEditorHost={app.codeEditorHost}
        centerView={app.centerView}
        previewDevice={app.previewDevice}
        previewZoom={app.previewZoom}
        tabletPreviewWidth={breakpointValue("bp-tableta", "1024px")}
        mobilePreviewWidth={breakpointValue("bp-mobil", "768px")}
        previewDocumentMarkup={app.previewDocumentMarkup}
        previewSrc={app.previewSrc}
        interactivePreviewEnabled={app.interactivePreviewEnabled && !app.aiEditLeaseFrontendLockActive}
        interactivePreviewUrl={app.interactivePreviewUrl}
        interactiveDomNodeCount={app.interactivePreviewDomNodes.length}
        templateWorkbenchActive={app.templateWorkbenchActive}
        templateWorkbenchTarget={app.templateWorkbenchTarget}
        templateWorkbenchPlan={app.templateWorkbenchPlan}
        refreshToken={app.refreshToken}
        editorReadOnly={app.projectTransitionFrontendLeaseActive || app.kernelUndoRedoFrontendLeaseActive || app.aiEditLeaseFrontendLockActive}
        attachPreviewInspector={() => app.attachPreviewInspector()}
        exitTemplateWorkbench={() => app.exitTemplateWorkbench()}
        setInteractivePreviewEnabled={(enabled) => app.setInteractivePreviewEnabled(enabled)}
        onInteractiveLifecycleError={(message) => app.setGlobalStatus(
          `Interactive Preview: ${message}`,
          "error",
        )}
        onInteractiveDomSnapshot={(nodes) => app.acceptInteractivePreviewDomSnapshot(nodes)}
        onInteractiveRealmRestarted={(previewRevision, durationMs) => {
          void app.recordInteractivePreviewRealmEvent(
            "interactive_js_restarted",
            previewRevision,
            durationMs,
          );
        }}
        onInteractiveRealmFailed={(previewRevision, durationMs, diagnostic) => {
          void app.recordInteractivePreviewRealmEvent(
            "interactive_js_failed",
            previewRevision,
            durationMs,
            diagnostic,
          );
        }}
        currentSourcePath={app.currentSourcePath}
        source={app.source}
        sourceLanguage={app.sourceLanguage}
        sourceLength={app.source.length}
        onMarkdownChange={(nextSource, path) => app.updateMarkdownSource(nextSource, path)}
      />
    {/if}
  </div>

  {#if motionTimelineAvailable}
    {#if !motionTimelineExpanded}
      <section class="motion-timeline-launcher" aria-label="Timeline Motion" inert={app.aiEditLeaseFrontendLockActive ? true : undefined}>
        <button type="button" onclick={openMotionTimeline}>
          Deschide timeline
        </button>
        <span>Editarea Timeline este disponibilă; redarea JS rulează numai în Run extern.</span>
      </section>
    {:else}
    <WorkspaceResizeHandle
      kind="motionTimeline"
      active={app.activeResizeKind === "motionTimeline"}
      ariaLabel="Redimensioneaza timeline-ul Motion"
      onDrag={(event) => app.startResizeDrag("motionTimeline", event)}
      onReset={() => { void closeMotionTimeline(); }}
    />

    {#if motionTimelineMountReady}
      <div inert={app.aiEditLeaseFrontendLockActive ? true : undefined}>
      <MotionTimelinePanel
        activeTemplatePath={app.activeRenderedTemplatePath}
        projectRoot={app.sessionProjectRoot}
        runtimeSessionId={app.kernelProjectSessionId}
        refreshToken={app.jsRefreshToken}
        onPendingChange={(pending) => app.setInspectorPending("js", pending, "motion-timeline")}
      />
      </div>
    {:else}
      <section class="motion-timeline-loading" aria-label="Timeline Motion se încarcă">
        <strong>Timeline</strong>
        <span>Se pregătește editorul Timeline…</span>
      </section>
    {/if}
    {/if}
  {/if}

  {#if app.terminalPaneOpen}
    <WorkspaceResizeHandle
      kind="terminal"
      active={app.activeResizeKind === "terminal"}
      ariaLabel="Redimensioneaza terminalul"
      onDrag={(event) => app.startResizeDrag("terminal", event)}
      onReset={() => app.resetResize("terminal")}
    />

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
      />
    {:else}
      <section class="terminal-loading" aria-label="Terminalul se încarcă">
        <span>Se încarcă terminalul…</span>
      </section>
    {/if}
  {/if}
</section>

<style>
  .motion-timeline-launcher {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    min-height: 0;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-2) 84%, var(--brand-soft));
    box-shadow: var(--shadow);
    color: var(--text-muted);
    font-size: 11px;
  }

  .motion-timeline-launcher button {
    flex: 0 0 auto;
    min-height: 28px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font-size: 11px;
    font-weight: 900;
    cursor: pointer;
  }

  .motion-timeline-launcher span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .motion-timeline-loading,
  .mood-board-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-2) 88%, var(--brand-soft));
    box-shadow: var(--shadow);
    color: var(--text-muted);
    font-size: 11px;
  }

  .motion-timeline-loading strong,
  .mood-board-loading strong {
    color: var(--text);
    font-size: 12px;
  }

  .mood-board-loading span {
    max-width: min(680px, 80%);
    text-align: center;
  }

  .mood-board-loading button {
    min-height: 30px;
    margin-top: 6px;
    padding: 0 12px;
    border: 1px solid var(--brand);
    border-radius: 7px;
    background: var(--brand-soft);
    color: var(--brand-strong);
    font: inherit;
    font-weight: 800;
    cursor: pointer;
  }
</style>
