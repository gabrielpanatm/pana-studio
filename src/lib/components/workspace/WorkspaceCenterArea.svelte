<script lang="ts">
  import { onDestroy, type Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import EditorShell from "$lib/components/EditorShell.svelte";
  import AuditWorkspace from "$lib/components/audit/AuditWorkspace.svelte";
  import ContentWorkspace from "$lib/components/content/ContentWorkspace.svelte";
  import DataWorkspace from "$lib/components/data/DataWorkspace.svelte";
  import AssetsWorkspace from "$lib/components/creation/AssetsWorkspace.svelte";
  import BlocksWorkspace from "$lib/components/creation/BlocksWorkspace.svelte";
  import ComponentsWorkspace from "$lib/components/creation/ComponentsWorkspace.svelte";
  import DesignSystemWorkspace from "$lib/components/creation/DesignSystemWorkspace.svelte";
  import KernelWorkspace from "$lib/components/kernel/KernelWorkspace.svelte";
  import PublishWorkspace from "$lib/components/publish/PublishWorkspace.svelte";
  import SettingsWorkspace from "$lib/components/settings/SettingsWorkspace.svelte";
  import TaxonomiesWorkspace from "$lib/components/taxonomies/TaxonomiesWorkspace.svelte";
  import TemplatesWorkspace from "$lib/components/templates/TemplatesWorkspace.svelte";
  import ThemesWorkspace from "$lib/components/themes/ThemesWorkspace.svelte";
  import VersionControlWorkspace from "$lib/components/versioning/VersionControlWorkspace.svelte";
  import WorkbenchBottomPanel from "$lib/components/workbench/WorkbenchBottomPanel.svelte";
  import MotionTimelinePanel from "$lib/components/workspace/MotionTimelinePanel.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    CenterView,
    WorkbenchDocumentSnapshot,
    WorkbenchGroupId,
    WorkbenchSurface,
    WorkspaceSourceOpenOptions,
  } from "$lib/types";
  import { t } from "$lib/i18n/runtime.svelte";
  import { errorMessage } from "$lib/util";

  let {
    app,
    TerminalPaneComponent = null,
    breakpointValue,
    openWorkspaceSource,
  }: {
    app: AppState;
    TerminalPaneComponent?: Component<TerminalPaneProps> | null;
    breakpointValue: (name: string, fallback: string) => string;
    openWorkspaceSource: (
      path: string,
      options?: WorkspaceSourceOpenOptions,
    ) => void | Promise<void>;
  } = $props();

  const bottomPanelOpen = $derived(
    app.applicationSurface === "workbench"
      && Boolean(app.workbenchSnapshot?.bottomPanel.open)
      && app.workbenchSnapshot?.bottomPanel.activeView === "terminal",
  );
  const activeWorkbenchActivity = $derived(
    app.workbenchSnapshot?.activeActivity ?? "editor",
  );
  const motionTimelineAvailable = $derived(
    app.applicationSurface === "workbench"
      && activeWorkbenchActivity === "editor"
      && app.centerView === "preview"
      && Boolean(app.activeRenderedTemplatePath)
      && Boolean(app.scannedProject),
  );
  const motionTimelineOpen = $derived(
    motionTimelineAvailable && app.motionWorkspace.timelineOpen,
  );
  const motionTimelineHeight = $derived(
    app.motionWorkspace.timelineCollapsed ? 36 : app.motionWorkspace.timelineHeight,
  );
  let motionTimelineResizing = $state(false);
  let cancelMotionTimelineResize: (() => void) | null = null;
  const dirtyWorkbenchPaths = $derived(
    app.projectWorkspaceSnapshot?.documents.files
      .filter((file) => file.dirty)
      .map((file) => file.relativePath)
      ?? [],
  );
  const responsiveBreakpoints = $derived([
    {
      id: "mobile",
      label: t("workbench-responsive-mobile"),
      widthPx: Number.parseFloat(breakpointValue("bp-mobil", "768px")) || 768,
    },
    {
      id: "tablet",
      label: t("workbench-responsive-tablet"),
      widthPx: Number.parseFloat(breakpointValue("bp-tableta", "1024px")) || 1_024,
    },
  ]);
  function centerViewForSurface(surface: WorkbenchSurface): CenterView {
    if (surface === "code") return "code";
    if (surface === "markdown") return "markdown";
    return "preview";
  }

  async function showWorkbenchDocument(document: WorkbenchDocumentSnapshot) {
    const file = app.scannedProject?.files.find(
      (candidate) => candidate.relativePath === document.relativePath,
    );
    if (!file) {
      app.setGlobalStatus(
        t("workbench-document-missing", { path: document.relativePath }),
        "error",
      );
      return;
    }
    await app.loadScannedProjectFile(file);
    await app.setCenterView(centerViewForSurface(document.surface));
  }

  async function activateWorkbenchDocument(
    groupId: WorkbenchGroupId,
    document: WorkbenchDocumentSnapshot,
  ) {
    try {
      await app.applyWorkbenchIntent({
        kind: "activate_document",
        documentId: document.documentId,
        groupId,
      });
      await showWorkbenchDocument(document);
    } catch (error) {
      app.setGlobalStatus(
        t("workbench-document-activate-failed", { detail: errorMessage(error) }),
        "error",
      );
    }
  }

  async function closeWorkbenchDocument(
    groupId: WorkbenchGroupId,
    document: WorkbenchDocumentSnapshot,
  ) {
    const wasActive = app.workbenchSnapshot?.groups
      .find((group) => group.groupId === groupId)
      ?.activeDocumentId === document.documentId;
    try {
      const receipt = await app.applyWorkbenchIntent({
        kind: "close_document",
        documentId: document.documentId,
        groupId,
      });
      if (!wasActive) return;
      const nextGroup = receipt.snapshot.groups.find((group) => group.groupId === groupId);
      const nextDocument = nextGroup?.documents.find(
        (candidate) => candidate.documentId === nextGroup.activeDocumentId,
      );
      if (nextDocument) await showWorkbenchDocument(nextDocument);
    } catch (error) {
      app.setGlobalStatus(
        t("workbench-document-close-failed", { detail: errorMessage(error) }),
        "error",
      );
    }
  }

  async function setWorkbenchSurface(surface: WorkbenchSurface) {
    await app.setCenterView(centerViewForSurface(surface));
  }

  $effect(() => {
    app.motionWorkspace.bind(
      app.activeRenderedTemplatePath,
      app.sessionProjectRoot,
      app.kernelProjectSessionId,
      app.jsRefreshToken,
    );
  });

  $effect(() => {
    app.setInspectorPending(
      "js",
      app.motionWorkspace.pendingCount > 0,
      "motion-timeline",
    );
  });

  $effect(() => {
    const mode = app.motionWorkspace.previewMode;
    const shouldExecute = mode !== "design";
    if (app.interactivePreviewEnabled !== shouldExecute) {
      app.setPreviewExecutionMode(mode);
    }
  });

  function resizeMotionTimeline(event: MouseEvent) {
    if (event.button !== 0) return;
    cancelMotionTimelineResize?.();
    event.preventDefault();

    const startY = event.clientY;
    const startHeight = app.motionWorkspace.timelineHeight;
    let latestY = startY;
    let animationFrame: number | null = null;
    let safetyTimer: number | null = null;
    let stopped = false;

    motionTimelineResizing = true;
    document.body.classList.add("is-resizing", "is-row-resizing");

    const heightAt = (clientY: number) => Math.max(
      190,
      Math.min(560, startHeight - (clientY - startY)),
    );

    const flushResize = () => {
      animationFrame = null;
      app.motionWorkspace.timelineHeight = heightAt(latestY);
    };

    const move = (moveEvent: MouseEvent) => {
      moveEvent.preventDefault();
      latestY = moveEvent.clientY;
      if (animationFrame !== null) return;
      animationFrame = window.requestAnimationFrame(flushResize);
    };

    const cleanup = () => {
      if (animationFrame !== null) {
        window.cancelAnimationFrame(animationFrame);
        animationFrame = null;
      }
      if (safetyTimer !== null) {
        window.clearTimeout(safetyTimer);
        safetyTimer = null;
      }
      document.body.classList.remove("is-resizing", "is-row-resizing");
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", commit);
      window.removeEventListener("blur", cancel);
      window.removeEventListener("keydown", handleKeydown);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      cancelMotionTimelineResize = null;
      motionTimelineResizing = false;
    };

    const stop = (shouldCommit: boolean) => {
      if (stopped) return;
      stopped = true;
      if (shouldCommit) {
        app.motionWorkspace.timelineHeight = heightAt(latestY);
      } else {
        app.motionWorkspace.timelineHeight = startHeight;
      }
      cleanup();
    };

    const commit = () => stop(true);
    const cancel = () => stop(false);
    const handleKeydown = (keyEvent: KeyboardEvent) => {
      if (keyEvent.key === "Escape") cancel();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "hidden") cancel();
    };

    cancelMotionTimelineResize = cancel;
    safetyTimer = window.setTimeout(cancel, 8_000);
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", commit, { once: true });
    window.addEventListener("blur", cancel, { once: true });
    window.addEventListener("keydown", handleKeydown);
    document.addEventListener("visibilitychange", handleVisibilityChange);
  }

  onDestroy(() => cancelMotionTimelineResize?.());
</script>

<section
  class:bottom-panel-open={bottomPanelOpen}
  class:motion-timeline-open={motionTimelineOpen}
  class="center-stack"
  style={`--terminal-pane-height: ${app.terminalPaneHeight}px; --motion-timeline-height: ${motionTimelineHeight}px;`}
  aria-label={t("workbench-center-area")}
>
  <div
    class="editor-shell-shell"
    inert={app.aiEditLeaseFrontendLockActive ? true : undefined}
    aria-busy={app.aiEditLeaseFrontendLockActive}
  >
    {#if app.applicationSurface === "settings"}
      <SettingsWorkspace {app} />
    {:else if activeWorkbenchActivity === "themes"}
      <ThemesWorkspace {app} />
    {:else if activeWorkbenchActivity === "templates"}
      <TemplatesWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "components"}
      <ComponentsWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "blocks"}
      <BlocksWorkspace {app} />
    {:else if activeWorkbenchActivity === "design_system"}
      <DesignSystemWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "assets"}
      <AssetsWorkspace {app} />
    {:else if activeWorkbenchActivity === "content"}
      <ContentWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "taxonomies"}
      <TaxonomiesWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "data"}
      <DataWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "versioning"}
      <VersionControlWorkspace {app} />
    {:else if activeWorkbenchActivity === "publish"}
      <PublishWorkspace {app} />
    {:else if activeWorkbenchActivity === "audit"}
      <AuditWorkspace
        {app}
        {openWorkspaceSource}
        requestedView={app.auditWorkspaceView}
        observabilityFocusSerial={app.auditObservabilityFocusSerial}
        onViewChange={(view) => { app.auditWorkspaceView = view; }}
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
    {:else}
      <EditorShell
        bind:previewFrame={app.previewFrame}
        bind:codeEditorHost={app.codeEditorHost}
        centerView={app.centerView}
        previewZoom={app.previewZoom}
        previewCanvasMode={app.previewCanvasMode}
        previewCanvasPreset={app.previewCanvasPreset}
        previewWidthPx={app.previewWidthPx}
        previewRulers={app.previewRulers}
        {responsiveBreakpoints}
        previewDocumentMarkup={app.previewDocumentMarkup}
        previewSrc={app.previewSrc}
        interactivePreviewEnabled={app.interactivePreviewEnabled && !app.aiEditLeaseFrontendLockActive}
        interactivePreviewUrl={app.interactivePreviewUrl}
        motionPreviewMode={app.motionWorkspace.previewMode}
        motionPreviewRequest={app.motionWorkspace.previewRequest}
        refreshToken={app.refreshToken}
        editorReadOnly={app.projectTransitionFrontendLeaseActive || app.kernelUndoRedoFrontendLeaseActive || app.aiEditLeaseFrontendLockActive}
        workbenchSnapshot={app.workbenchSnapshot}
        {dirtyWorkbenchPaths}
        {activateWorkbenchDocument}
        {closeWorkbenchDocument}
        {setWorkbenchSurface}
        setWorkbenchSplit={async (split) => { await app.setSynchronizedWorkbenchSplit(split); }}
        setWorkbenchSplitRatio={async (ratioBasisPoints) => { await app.setWorkbenchSplitRatio(ratioBasisPoints); }}
        setCanvasViewport={async (viewport) => { await app.setWorkbenchCanvasViewport(viewport); }}
        setPreviewZoom={(value) => app.setPreviewZoom(value)}
        commitPreviewZoom={async (value) => { await app.commitPreviewZoom(value); }}
        resetPreviewZoom={() => app.resetPreviewZoom()}
        attachPreviewInspector={() => app.attachPreviewInspector()}
        mountPreviewSurface={(frame) => app.mountCanvasProjectionSurface(frame)}
        unmountPreviewSurface={(frame) => app.unmountCanvasProjectionSurface(frame)}
        previewSurfaceLoaded={(frame) => app.onCanvasProjectionSurfaceLoaded(frame)}
        setPreviewExecutionMode={(mode) => app.setPreviewExecutionMode(mode)}
        onInteractiveLifecycleError={(message) => app.setGlobalStatus(
          t("workbench-interactive-error", { detail: message }),
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
        onMotionPreviewStatus={(status) => app.motionWorkspace.acceptPreviewStatus(status)}
        currentSourcePath={app.currentSourcePath}
        source={app.source}
        sourceLanguage={app.sourceLanguage}
        sourceLength={app.source.length}
        onMarkdownChange={(nextSource, path) => app.updateMarkdownSource(nextSource, path)}
      />
    {/if}
  </div>

  {#if motionTimelineOpen}
    {#if !app.motionWorkspace.timelineCollapsed}
      <WorkspaceResizeHandle
        kind="timeline"
        active={motionTimelineResizing}
        withBottomPanel={bottomPanelOpen}
        ariaLabel={t("workbench-resize-motion-timeline")}
        onDrag={resizeMotionTimeline}
        onReset={() => { app.motionWorkspace.timelineHeight = 300; }}
      />
    {/if}
    <MotionTimelinePanel
      workspace={app.motionWorkspace}
      selectionSummary={app.inspectorSelectionSummary}
      dataAnim={app.coordinatedElementSelection?.observation.attributes["data-anim"] ?? null}
    />
  {/if}

  {#if bottomPanelOpen}
    <WorkspaceResizeHandle
      kind="terminal"
      active={app.activeResizeKind === "terminal"}
      ariaLabel={t("workbench-resize-bottom-panel")}
      onDrag={(event) => app.startResizeDrag("terminal", event)}
      onReset={() => app.resetResize("terminal")}
    />

    <WorkbenchBottomPanel
      {app}
      {TerminalPaneComponent}
    />
  {/if}
</section>
