<script lang="ts">
  import type { Component } from "svelte";
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
  import TemplatesWorkspace from "$lib/components/templates/TemplatesWorkspace.svelte";
  import ThemesWorkspace from "$lib/components/themes/ThemesWorkspace.svelte";
  import VersionControlWorkspace from "$lib/components/versioning/VersionControlWorkspace.svelte";
  import StartupState from "$lib/components/workspace/StartupState.svelte";
  import WorkbenchBottomPanel from "$lib/components/workbench/WorkbenchBottomPanel.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import type { AppState } from "$lib/state/app.svelte";
  import type {
    CenterView,
    WorkbenchDocumentSnapshot,
    WorkbenchGroupId,
    WorkbenchSurface,
  } from "$lib/types";

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

  const bottomPanelOpen = $derived(
    app.applicationSurface === "workbench"
      && Boolean(app.workbenchSnapshot?.bottomPanel.open),
  );
  const dirtyWorkbenchPaths = $derived(
    app.projectWorkspaceSnapshot?.documents.files
      .filter((file) => file.dirty)
      .map((file) => file.relativePath)
      ?? [],
  );
  const activeWorkbenchActivity = $derived(
    app.workbenchSnapshot?.activeActivity ?? "editor",
  );
  const responsiveBreakpoints = $derived([
    {
      id: "mobile",
      label: "Mobil",
      widthPx: Number.parseFloat(breakpointValue("bp-mobil", "768px")) || 768,
    },
    {
      id: "tablet",
      label: "Tabletă",
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
        `Documentul spațiului de lucru nu mai există în proiect: ${document.relativePath}`,
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
        `Documentul nu a putut fi activat: ${error instanceof Error ? error.message : String(error)}`,
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
        `Documentul nu a putut fi închis: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
    }
  }

  async function setWorkbenchSurface(surface: WorkbenchSurface) {
    await app.setCenterView(centerViewForSurface(surface));
  }

</script>

<section
  class:bottom-panel-open={bottomPanelOpen}
  class="center-stack"
  style={`--terminal-pane-height: ${app.terminalPaneHeight}px;`}
  aria-label="Zona centrala"
>
  <div
    class="editor-shell-shell"
    inert={app.aiEditLeaseFrontendLockActive ? true : undefined}
    aria-busy={app.aiEditLeaseFrontendLockActive}
  >
    {#if app.applicationSurface === "settings"}
      <SettingsWorkspace {app} />
    {:else if !app.scannedProject || app.scannedProject.isEmpty || !app.scannedProject.isZola}
      <StartupState
        scannedProject={!!app.scannedProject}
        isEmpty={app.scannedProject?.isEmpty ?? false}
        isZola={app.scannedProject?.isZola ?? false}
        openProjectFolder={() => app.openProjectFolder()}
        workspaceSnapshot={app.projectWorkspaceSnapshot}
        initZolaProject={(themeId) => app.initZolaProject(themeId)}
      />
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
    {:else if activeWorkbenchActivity === "data"}
      <DataWorkspace {app} {openWorkspaceSource} />
    {:else if activeWorkbenchActivity === "versioning"}
      <VersionControlWorkspace {app} />
    {:else if activeWorkbenchActivity === "publish"}
      <PublishWorkspace {app} />
    {:else if activeWorkbenchActivity === "audit"}
      <AuditWorkspace {app} {openWorkspaceSource} />
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
        setInteractivePreviewEnabled={(enabled) => app.setInteractivePreviewEnabled(enabled)}
        onInteractiveLifecycleError={(message) => app.setGlobalStatus(
          `Previzualizare interactivă: ${message}`,
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

  {#if bottomPanelOpen}
    <WorkspaceResizeHandle
      kind="terminal"
      active={app.activeResizeKind === "terminal"}
      ariaLabel="Redimensionează panoul inferior"
      onDrag={(event) => app.startResizeDrag("terminal", event)}
      onReset={() => app.resetResize("terminal")}
    />

    <WorkbenchBottomPanel
      {app}
      {TerminalPaneComponent}
      {openWorkspaceSource}
    />
  {/if}
</section>
