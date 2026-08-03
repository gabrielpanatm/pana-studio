<script lang="ts">
  import { onDestroy, type Component } from "svelte";
  import type { TerminalPaneProps } from "$lib/components/TerminalPane.svelte";
  import EditorShell from "$lib/components/EditorShell.svelte";
  import WorkbenchBottomPanel from "$lib/components/workbench/WorkbenchBottomPanel.svelte";
  import MotionTimelinePanel from "$lib/components/workspace/MotionTimelinePanel.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    CenterView,
    WorkbenchActivity,
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
  const editorSurfaceActive = $derived(
    app.applicationSurface === "workbench"
      && activeWorkbenchActivity === "editor"
      && app.centerView !== "kernel",
  );
  type RetainedAuxiliarySurface =
    | Exclude<WorkbenchActivity, "editor">
    | "settings"
    | "kernel";
  type AuxiliaryWorkspaceComponent = Component<any>;
  let AuditWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ContentWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ContentModelsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let DataWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let AssetsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let BlocksWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ComponentsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let DesignSystemWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let KernelWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let PublishWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let SettingsWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let TaxonomiesWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let TemplatesWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let ThemesWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  let VersionControlWorkspace = $state<AuxiliaryWorkspaceComponent | null>(null);
  const auxiliaryWorkspaceLoads = new Map<
    RetainedAuxiliarySurface,
    Promise<void>
  >();
  const auxiliaryWorkspaceLoaders: Record<
    RetainedAuxiliarySurface,
    () => Promise<void>
  > = {
    settings: async () => {
      SettingsWorkspace = (await import(
        "$lib/components/settings/SettingsWorkspace.svelte"
      )).default;
    },
    themes: async () => {
      ThemesWorkspace = (await import(
        "$lib/components/themes/ThemesWorkspace.svelte"
      )).default;
    },
    templates: async () => {
      TemplatesWorkspace = (await import(
        "$lib/components/templates/TemplatesWorkspace.svelte"
      )).default;
    },
    components: async () => {
      ComponentsWorkspace = (await import(
        "$lib/components/creation/ComponentsWorkspace.svelte"
      )).default;
    },
    blocks: async () => {
      BlocksWorkspace = (await import(
        "$lib/components/creation/BlocksWorkspace.svelte"
      )).default;
    },
    design_system: async () => {
      DesignSystemWorkspace = (await import(
        "$lib/components/creation/DesignSystemWorkspace.svelte"
      )).default;
    },
    assets: async () => {
      AssetsWorkspace = (await import(
        "$lib/components/creation/AssetsWorkspace.svelte"
      )).default;
    },
    content: async () => {
      ContentWorkspace = (await import(
        "$lib/components/content/ContentWorkspace.svelte"
      )).default;
    },
    content_models: async () => {
      ContentModelsWorkspace = (await import(
        "$lib/components/content-models/ContentModelsWorkspace.svelte"
      )).default;
    },
    taxonomies: async () => {
      TaxonomiesWorkspace = (await import(
        "$lib/components/taxonomies/TaxonomiesWorkspace.svelte"
      )).default;
    },
    data: async () => {
      DataWorkspace = (await import(
        "$lib/components/data/DataWorkspace.svelte"
      )).default;
    },
    versioning: async () => {
      VersionControlWorkspace = (await import(
        "$lib/components/versioning/VersionControlWorkspace.svelte"
      )).default;
    },
    publish: async () => {
      PublishWorkspace = (await import(
        "$lib/components/publish/PublishWorkspace.svelte"
      )).default;
    },
    audit: async () => {
      AuditWorkspace = (await import(
        "$lib/components/audit/AuditWorkspace.svelte"
      )).default;
    },
    kernel: async () => {
      KernelWorkspace = (await import(
        "$lib/components/kernel/KernelWorkspace.svelte"
      )).default;
    },
  };
  let retainedAuxiliarySurface = $state<RetainedAuxiliarySurface | null>(null);
  let retainedAuxiliarySessionId = $state("");

  function ensureAuxiliaryWorkspaceLoaded(surface: RetainedAuxiliarySurface) {
    if (auxiliaryWorkspaceLoads.has(surface)) return;
    const load = auxiliaryWorkspaceLoaders[surface]()
      .then(() => {
        app.clearNotification("workbench.activity.lazy-load");
      })
      .catch((error) => {
        auxiliaryWorkspaceLoads.delete(surface);
        app.escalateGlobalStatus({
          id: "workbench.activity.lazy-load",
          level: "error",
          title: t("workbench-activity-open-failed"),
          message: errorMessage(error),
        });
      });
    auxiliaryWorkspaceLoads.set(surface, load);
  }

  $effect(() => {
    const sessionId = app.kernelProjectSessionId;
    if (retainedAuxiliarySessionId !== sessionId) {
      retainedAuxiliarySessionId = sessionId;
      retainedAuxiliarySurface = null;
    }
    if (app.applicationSurface === "settings") {
      retainedAuxiliarySurface = "settings";
    } else if (activeWorkbenchActivity !== "editor") {
      retainedAuxiliarySurface = activeWorkbenchActivity;
    } else if (app.centerView === "kernel") {
      retainedAuxiliarySurface = "kernel";
    }
    if (retainedAuxiliarySurface) {
      ensureAuxiliaryWorkspaceLoaded(retainedAuxiliarySurface);
    }
  });
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
    inert={app.aiEditLeaseFrontendLockActive
      || app.kernelUndoRedoFrontendQuiesceActive
      || app.kernelUndoRedoFrontendLeaseActive
      ? true
      : undefined}
    aria-busy={app.aiEditLeaseFrontendLockActive
      || app.kernelUndoRedoFrontendQuiesceActive
      || app.kernelUndoRedoFrontendLeaseActive}
  >
    {#if app.scannedProject && app.kernelProjectSessionId}
      {#key app.kernelProjectSessionId}
        <div
          class="stable-editor-surface"
          class:surface-inactive={!editorSurfaceActive}
          inert={!editorSurfaceActive ? true : undefined}
          aria-hidden={!editorSurfaceActive}
        >
          <EditorShell
            bind:previewFrame={app.previewFrame}
            bind:codeEditorHost={app.codeEditorHost}
            surfaceActive={editorSurfaceActive}
            centerView={app.centerView}
            previewZoom={app.previewZoom}
            previewCanvasMode={app.previewCanvasMode}
            previewCanvasPreset={app.previewCanvasPreset}
            previewWidthPx={app.previewWidthPx}
            previewRulers={app.previewRulers}
            {responsiveBreakpoints}
            previewDocumentMarkup={app.previewDocumentMarkup}
            previewSrc={app.previewSrc}
            previewNavigationGuardActive={app.previewNavigationGuardActive}
            interactivePreviewEnabled={app.interactivePreviewEnabled
              && !app.aiEditLeaseFrontendLockActive
              && !app.kernelUndoRedoFrontendQuiesceActive
              && !app.kernelUndoRedoFrontendLeaseActive}
            interactivePreviewUrl={app.interactivePreviewUrl}
            motionPreviewMode={app.motionWorkspace.previewMode}
            motionPreviewRequest={app.motionWorkspace.previewRequest}
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
          />
        </div>
      {/key}
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

{#if retainedAuxiliarySurface}
  <div
    class="workspace-auxiliary-overlay"
    class:surface-inactive={editorSurfaceActive}
    inert={editorSurfaceActive ? true : undefined}
    aria-hidden={editorSurfaceActive}
  >
    {#if retainedAuxiliarySurface === "settings" && SettingsWorkspace}
      <SettingsWorkspace {app} />
    {:else if retainedAuxiliarySurface === "themes" && ThemesWorkspace}
      <ThemesWorkspace {app} />
    {:else if retainedAuxiliarySurface === "templates" && TemplatesWorkspace}
      <TemplatesWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "components" && ComponentsWorkspace}
      <ComponentsWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "blocks" && BlocksWorkspace}
      <BlocksWorkspace {app} />
    {:else if retainedAuxiliarySurface === "design_system" && DesignSystemWorkspace}
      <DesignSystemWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "assets" && AssetsWorkspace}
      <AssetsWorkspace {app} />
    {:else if retainedAuxiliarySurface === "content" && ContentWorkspace}
      <ContentWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "content_models" && ContentModelsWorkspace}
      <ContentModelsWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "taxonomies" && TaxonomiesWorkspace}
      <TaxonomiesWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "data" && DataWorkspace}
      <DataWorkspace {app} {openWorkspaceSource} />
    {:else if retainedAuxiliarySurface === "versioning" && VersionControlWorkspace}
      <VersionControlWorkspace {app} />
    {:else if retainedAuxiliarySurface === "publish" && PublishWorkspace}
      <PublishWorkspace {app} />
    {:else if retainedAuxiliarySurface === "audit" && AuditWorkspace}
      <AuditWorkspace
        {app}
        {openWorkspaceSource}
        requestedView={app.auditWorkspaceView}
        observabilityFocusSerial={app.auditObservabilityFocusSerial}
        onViewChange={(view: "overview" | "runtime") => { app.auditWorkspaceView = view; }}
      />
    {:else if retainedAuxiliarySurface === "kernel" && KernelWorkspace}
      <KernelWorkspace
        currentProjectPath={app.currentProjectPath}
        projectFileCount={app.scannedProject?.files.length ?? 0}
        sourceNodeCount={app.sourceGraph?.nodes.length ?? 0}
        dirtyAreas={app.globalDirtyState.areas}
        canSave={app.globalDirtyState.canSave}
        diskBlockedReason={app.immediateDiskOperationBlockedReason}
        projectStatus={app.projectStatus}
        onStatusUpdate={(
          text: string,
          kind: "restored" | "saving" | "error",
        ) => app.setGlobalStatus(text, kind)}
      />
    {:else}
      <div class="workspace-lazy-loading" role="status" aria-live="polite">
        {t("common-loading")}
      </div>
    {/if}
  </div>
{/if}
