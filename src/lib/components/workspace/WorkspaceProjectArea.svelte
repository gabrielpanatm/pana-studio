<script lang="ts">
  import ProjectPane from "$lib/components/ProjectPane.svelte";
  import WorkspaceResizeHandle from "$lib/components/workspace/WorkspaceResizeHandle.svelte";
  import { t } from "$lib/i18n/runtime.svelte";
  import type { WorkspaceLayoutState } from "$lib/ui/workspace-layout.svelte";
  import type {
    InsertCatalogContext,
    InsertCatalogItem,
    InsertCatalogSnapshot,
  } from "$lib/blocks/contracts";
  import type { EditorMovePlan } from "$lib/editor/contracts";
  import type {
    EditorNavigationNode,
    EditorNavigationSnapshot,
  } from "$lib/editor/contracts";
  import type { ProjectMovePosition } from "$lib/preview/contracts";
  import type {
    FileExplorerOperationPlan,
    FileExplorerOperationRequest,
    FileExplorerSnapshot,
  } from "$lib/project/file-explorer-contract";
  import type { ProjectFile } from "$lib/project/lifecycle-contract";

  let {
    visible,
    sessionId,
    interactionLocked,
    pane,
    commands,
    workspaceLayout,
  }: {
    visible: boolean;
    sessionId: string;
    interactionLocked: boolean;
    pane: {
      projectRoot: string;
      workspaceRevision: number;
      allProjectFiles: ProjectFile[];
      activeScannedPath: string | null;
      fileExplorerSnapshot: FileExplorerSnapshot | null;
      fileExplorerLoading: boolean;
      fileExplorerError: string;
      insertCatalogContext: InsertCatalogContext;
      editorNavigationSnapshot: EditorNavigationSnapshot | null;
      editorNavigationLoading: boolean;
      editorNavigationError: string;
      coordinatedSelectionNodeIds: string[];
      coordinatedPrimaryNodeId: string | null;
      hoveredEditorNavigationNodeId: string | null;
      editorEditScopeId: string | null;
    };
    commands: {
      selectFileExplorerEntry: (entryId: string) => void | Promise<void>;
      planFileExplorerOperation: (operation: FileExplorerOperationRequest) => Promise<FileExplorerOperationPlan>;
      commitFileExplorerOperation: (plan: FileExplorerOperationPlan) => Promise<unknown>;
      openScannedFile: (file: ProjectFile) => void | Promise<void>;
      startInsertCatalogDrag: (item: InsertCatalogItem, snapshot: InsertCatalogSnapshot, event: PointerEvent) => void;
      selectEditorNavigationNode: (
        node: EditorNavigationNode,
        options?: { toggle?: boolean; extendRange?: boolean; setPrimary?: boolean },
      ) => void | Promise<unknown>;
      hoverEditorNavigationNode: (node: EditorNavigationNode | null) => void;
      enterEditorNavigationScope: (scopeId: string) => void | Promise<unknown>;
      exitEditorNavigationScope: () => void;
      previewEditorNavigationMove: (
        sourceNodeId: string,
        targetNodeId: string,
        position: ProjectMovePosition,
      ) => Promise<EditorMovePlan>;
      moveEditorNavigationNode: (
        sourceNodeId: string,
        targetNodeId: string,
        position: ProjectMovePosition,
      ) => void | Promise<unknown>;
      deleteEditorNavigationNode: (node: EditorNavigationNode) => void | Promise<unknown>;
      openEditorNavigationContextMenu: (node: EditorNavigationNode, x: number, y: number) => void | Promise<unknown>;
    };
    workspaceLayout: WorkspaceLayoutState;
  } = $props();

  const editorSidebarActive = $derived(visible);
</script>

{#if pane.projectRoot && sessionId}
  {#key sessionId}
    <div
      class="project-pane-shell"
      hidden={workspaceLayout.leftPaneCollapsed}
      inert={!editorSidebarActive
        || workspaceLayout.leftPaneCollapsed
        || interactionLocked
        ? true
        : undefined}
      aria-hidden={!editorSidebarActive || workspaceLayout.leftPaneCollapsed}
    >
      <ProjectPane
        scannedProject={true}
        projectRoot={pane.projectRoot}
        runtimeSessionId={sessionId}
        workspaceRevision={pane.workspaceRevision}
        allProjectFiles={pane.allProjectFiles}
        activeScannedPath={pane.activeScannedPath}
        fileExplorerSnapshot={pane.fileExplorerSnapshot}
        fileExplorerLoading={pane.fileExplorerLoading}
        fileExplorerError={pane.fileExplorerError}
        insertCatalogContext={pane.insertCatalogContext}
        editorNavigationSnapshot={pane.editorNavigationSnapshot}
        editorNavigationLoading={pane.editorNavigationLoading}
        editorNavigationError={pane.editorNavigationError}
        coordinatedSelectionNodeIds={pane.coordinatedSelectionNodeIds}
        coordinatedPrimaryNodeId={pane.coordinatedPrimaryNodeId}
        hoveredEditorNavigationNodeId={pane.hoveredEditorNavigationNodeId}
        editorEditScopeId={pane.editorEditScopeId}
        selectFileExplorerEntry={commands.selectFileExplorerEntry}
        planFileExplorerOperation={commands.planFileExplorerOperation}
        commitFileExplorerOperation={commands.commitFileExplorerOperation}
        openScannedFile={commands.openScannedFile}
        startInsertCatalogDrag={commands.startInsertCatalogDrag}
        selectEditorNavigationNode={commands.selectEditorNavigationNode}
        hoverEditorNavigationNode={commands.hoverEditorNavigationNode}
        enterEditorNavigationScope={commands.enterEditorNavigationScope}
        exitEditorNavigationScope={commands.exitEditorNavigationScope}
        previewEditorNavigationMove={commands.previewEditorNavigationMove}
        moveEditorNavigationNode={commands.moveEditorNavigationNode}
        deleteEditorNavigationNode={commands.deleteEditorNavigationNode}
        openEditorNavigationContextMenu={commands.openEditorNavigationContextMenu}
      />
    </div>
  {/key}
{/if}

{#if !workspaceLayout.leftPaneCollapsed && editorSidebarActive}
  <WorkspaceResizeHandle
    kind="left"
    active={workspaceLayout.activeResizeKind === "left"}
    ariaLabel={t("workbench-resize-left-panel")}
    onDrag={(event) => workspaceLayout.startResizeDrag("left", event)}
    onReset={() => workspaceLayout.resetResize("left")}
  />
{/if}
