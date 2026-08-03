<script lang="ts">
  import ProjectPane from "$lib/components/ProjectPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { AppState } from "$lib/state/app.svelte";

  let { app }: { app: AppState } = $props();

  const editorSidebarActive = $derived(
    app.applicationSurface === "workbench"
      && (app.workbenchSnapshot?.activeActivity ?? "editor") === "editor",
  );
</script>

{#if app.scannedProject && app.kernelProjectSessionId}
  {#key app.kernelProjectSessionId}
    <div
      class="project-pane-shell"
      hidden={app.leftPaneCollapsed}
      inert={!editorSidebarActive
        || app.leftPaneCollapsed
        || app.kernelUndoRedoFrontendQuiesceActive
        || app.kernelUndoRedoFrontendLeaseActive
        ? true
        : undefined}
      aria-hidden={!editorSidebarActive || app.leftPaneCollapsed}
    >
      <ProjectPane
        scannedProject={true}
        projectRoot={app.scannedProject.root}
        runtimeSessionId={app.kernelProjectSessionId}
        workspaceRevision={app.projectWorkspaceSnapshot?.revision ?? 0}
        allProjectFiles={app.scannedProject.files}
        activeScannedPath={app.activeScannedPath}
        fileExplorerSnapshot={app.fileExplorerSnapshot}
        fileExplorerLoading={app.fileExplorerLoading}
        fileExplorerError={app.fileExplorerError}
        insertCatalogContext={{
          activeDocumentPath: app.activeScannedPath,
          activeTemplatePath: app.activeRenderedTemplatePath,
          activePagePath: app.templateWorkbenchPreferredPagePath,
          canvasPreviewRevision: app.activeCanvasIdentity?.previewRevision ?? null,
          canvasAvailable: app.centerView === "preview" && Boolean(app.activeCanvasIdentity),
          targetSourceId: app.coordinatedElementSelection?.sourceNodeId ?? null,
          targetTag: app.coordinatedElementSelection?.observation.tag ?? null,
        }}
        editorNavigationSnapshot={app.editorNavigationSnapshot}
        editorNavigationLoading={app.editorNavigationLoading}
        editorNavigationError={app.editorNavigationError}
        coordinatedSelectionNodeId={app.selectionSnapshot?.projections.layers.editorNodeId ?? null}
        hoveredEditorNavigationNodeId={app.hoverSnapshot?.editorNodeId ?? null}
        editorEditScopeId={app.editorEditScopeId}
        selectFileExplorerEntry={(entryId) => app.selectFileExplorerEntry(entryId)}
        planFileExplorerOperation={(operation) => app.planFileExplorerOperation(operation)}
        commitFileExplorerOperation={(plan) => app.commitFileExplorerOperation(plan)}
        openScannedFile={(file) => app.loadScannedProjectFile(file)}
        startInsertCatalogDrag={(item, snapshot, event) =>
          app.startInsertCatalogDrag(item, snapshot, event)}
        selectEditorNavigationNode={(node) => app.selectEditorNavigationNode(node)}
        hoverEditorNavigationNode={(node) => app.hoverEditorNavigationNode(node)}
        enterEditorNavigationScope={(scopeId) => app.enterEditorNavigationScope(scopeId)}
        exitEditorNavigationScope={() => app.exitEditorNavigationScope()}
        previewEditorNavigationMove={(sourceNodeId, targetNodeId, position) =>
          app.previewEditorNavigationMove(sourceNodeId, targetNodeId, position)}
        moveEditorNavigationNode={(sourceNodeId, targetNodeId, position) =>
          app.moveEditorNavigationNode(sourceNodeId, targetNodeId, position)}
        deleteEditorNavigationNode={(node) =>
          app.deleteEditorNavigationNode(node)}
        openEditorNavigationContextMenu={(node, x, y) =>
          app.openEditorNavigationContextMenu(node, x, y)}
      />
    </div>
  {/key}
{/if}

{#if !app.leftPaneCollapsed && editorSidebarActive}
  <WorkspaceResizeHandle
    kind="left"
    active={app.activeResizeKind === "left"}
    ariaLabel={t("workbench-resize-left-panel")}
    onDrag={(event) => app.startResizeDrag("left", event)}
    onReset={() => app.resetResize("left")}
  />
{/if}
