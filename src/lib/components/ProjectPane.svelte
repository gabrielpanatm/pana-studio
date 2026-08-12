<script lang="ts">
  import { tick } from "svelte";
  import {
    IconFiles,
    IconPlus,
    IconStack2,
  } from "@tabler/icons-svelte";
  import type {
    EditorMovePlan,
    EditorNavigationNode,
    EditorNavigationSnapshot,
    FileExplorerOperationPlan,
    FileExplorerOperationRequest,
    FileExplorerSnapshot,
    ProjectMovePosition,
    ProjectFile,
    ProjectPaneTab,
    InsertCatalogContext,
    InsertCatalogItem,
    InsertCatalogSnapshot,
  } from "$lib/types";
  import ProjectFilesTab from "$lib/components/project/ProjectFilesTab.svelte";
  import EditorNavigationTree from "$lib/components/project/EditorNavigationTree.svelte";
  import InsertCatalogPanel from "$lib/components/project/InsertCatalogPanel.svelte";
  import {
    legacyTranslator,
    localeRevision,
  } from "$lib/i18n/runtime.svelte";

  $: t = legacyTranslator($localeRevision);

  export let scannedProject = false;
  export let projectRoot = "";
  export let runtimeSessionId = "";
  export let workspaceRevision = 0;
  export let allProjectFiles: ProjectFile[] = [];
  export let activeScannedPath: string | null = null;
  export let fileExplorerSnapshot: FileExplorerSnapshot | null = null;
  export let fileExplorerLoading = false;
  export let fileExplorerError = "";
  export let insertCatalogContext: InsertCatalogContext;
  export let editorNavigationSnapshot: EditorNavigationSnapshot | null = null;
  export let editorNavigationLoading = false;
  export let editorNavigationError = "";
  export let coordinatedSelectionNodeIds: string[] = [];
  export let coordinatedPrimaryNodeId: string | null = null;
  export let hoveredEditorNavigationNodeId: string | null = null;
  export let editorEditScopeId: string | null = null;
  export let selectFileExplorerEntry: (entryId: string) => void | Promise<void>;
  export let planFileExplorerOperation: (
    operation: FileExplorerOperationRequest,
  ) => Promise<FileExplorerOperationPlan>;
  export let commitFileExplorerOperation: (
    plan: FileExplorerOperationPlan,
  ) => Promise<unknown>;

  let projectPaneTab: ProjectPaneTab = "layers";
  let elementPaletteOpen = false;
  let elementPaletteTrigger: HTMLButtonElement;
  let elementPaletteDialog: HTMLElement;
  let fileCollapsedDirs = new Set<string>();
  let fileKnownDirPaths = new Set<string>();
  let fileRevealedEntryKey = "";
  let fileTreeMemorySessionKey: string | null = null;
  let editorNavigationCallers: Array<{ caller: string; target: string }> = [];

  async function setElementPaletteOpen(open: boolean, restoreFocus = true) {
    elementPaletteOpen = open;
    await tick();
    if (open) elementPaletteDialog?.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
    else if (restoreFocus) elementPaletteTrigger?.focus();
  }

  function handleElementPaletteKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      void setElementPaletteOpen(false);
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = Array.from(
      elementPaletteDialog?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }

  function selectProjectPaneTab(tab: ProjectPaneTab) {
    projectPaneTab = tab;
    if (elementPaletteOpen) void setElementPaletteOpen(false, false);
  }

  const projectPaneTabs: ProjectPaneTab[] = ["layers", "files"];
  async function focusProjectPaneTab(tab: ProjectPaneTab) {
    selectProjectPaneTab(tab);
    await tick();
    document.getElementById(`project-pane-tab-${tab}`)?.focus();
  }

  function handleProjectPaneTabKeydown(event: KeyboardEvent, tab: ProjectPaneTab) {
    const index = projectPaneTabs.indexOf(tab);
    let nextIndex = index;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % projectPaneTabs.length;
    else if (event.key === "ArrowLeft") nextIndex = (index - 1 + projectPaneTabs.length) % projectPaneTabs.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = projectPaneTabs.length - 1;
    else return;
    event.preventDefault();
    void focusProjectPaneTab(projectPaneTabs[nextIndex]);
  }

  $: if (fileTreeMemorySessionKey !== `${projectRoot}::${runtimeSessionId}`) {
    fileCollapsedDirs = new Set<string>();
    fileKnownDirPaths = new Set<string>();
    fileRevealedEntryKey = "";
    fileTreeMemorySessionKey = `${projectRoot}::${runtimeSessionId}`;
  }

  $: {
    const caller = editorNavigationCallers.at(-1);
    if (
      caller
      && activeScannedPath !== caller.target
      && activeScannedPath !== caller.caller
    ) {
      editorNavigationCallers = [];
    }
  }

  export let openScannedFile: (file: ProjectFile) => void | Promise<void>;
  export let startInsertCatalogDrag: (
    item: InsertCatalogItem,
    snapshot: InsertCatalogSnapshot,
    event: PointerEvent,
  ) => void;
  export let selectEditorNavigationNode: (
    node: EditorNavigationNode,
    options?: { toggle?: boolean; extendRange?: boolean; setPrimary?: boolean },
  ) => void | Promise<unknown>;
  export let hoverEditorNavigationNode: (node: EditorNavigationNode | null) => void;
  export let enterEditorNavigationScope: (scopeId: string) => void | Promise<unknown>;
  export let exitEditorNavigationScope: () => void;
  export let previewEditorNavigationMove: (
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
  ) => Promise<EditorMovePlan>;
  export let moveEditorNavigationNode: (
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
  ) => void | Promise<unknown>;
  export let deleteEditorNavigationNode: (
    node: EditorNavigationNode,
  ) => void | Promise<unknown>;
  export let openEditorNavigationContextMenu: (
    node: EditorNavigationNode,
    x: number,
    y: number,
  ) => void | Promise<unknown>;

  async function openEditorNavigationDocument(
    documentPath: string,
    rememberCaller = false,
  ) {
    const file = allProjectFiles.find(
      (candidate) => candidate.relativePath === documentPath,
    );
    if (!file) return;
    const callerFrame = (
      rememberCaller
      && activeScannedPath
      && activeScannedPath !== documentPath
    )
      ? { caller: activeScannedPath, target: documentPath }
      : null;
    if (callerFrame) {
      editorNavigationCallers = [
        ...editorNavigationCallers,
        callerFrame,
      ];
    }
    try {
      await openScannedFile(file);
    } catch (error) {
      if (
        callerFrame
        && editorNavigationCallers.at(-1) === callerFrame
      ) {
        editorNavigationCallers = editorNavigationCallers.slice(0, -1);
      }
      throw error;
    }
  }

  async function returnFromEditorNavigationDocument() {
    const caller = editorNavigationCallers.at(-1);
    if (!caller) return;
    await openEditorNavigationDocument(caller.caller);
    if (
      activeScannedPath === caller.caller
      && editorNavigationCallers.at(-1) === caller
    ) {
      editorNavigationCallers = editorNavigationCallers.slice(0, -1);
    }
  }

</script>

<aside class="project-pane" aria-label={t("project-pane-navigator")}>
  <button
    bind:this={elementPaletteTrigger}
    class="ui-button primary pane-add-element-btn"
    class:active={elementPaletteOpen}
    type="button"
    title={t("project-pane-open-add-element")}
    aria-haspopup="dialog"
    aria-expanded={elementPaletteOpen}
    aria-controls="element-palette-dialog"
    onclick={() => { void setElementPaletteOpen(!elementPaletteOpen); }}
  >
    <IconPlus size={15} stroke={2} />
    <span>{t("project-pane-add-element")}</span>
  </button>

  <div class="ui-tabs pane-tabs" role="tablist" aria-label={t("project-pane-areas")}>
    <button id="project-pane-tab-layers" class="ui-tab tab-btn" class:active={projectPaneTab === "layers"} type="button" role="tab" title={t("project-pane-layers")}
      aria-selected={projectPaneTab === "layers"} aria-controls="project-pane-panel-layers" tabindex={projectPaneTab === "layers" ? 0 : -1}
      onclick={() => selectProjectPaneTab("layers")} onkeydown={(event) => handleProjectPaneTabKeydown(event, "layers")}>
      <IconStack2 size={15} stroke={1.8} /><span>{t("project-pane-layers")}</span>
    </button>
    <button id="project-pane-tab-files" class="ui-tab tab-btn" class:active={projectPaneTab === "files"} type="button" role="tab" title={t("project-pane-files")}
      aria-selected={projectPaneTab === "files"} aria-controls="project-pane-panel-files" tabindex={projectPaneTab === "files" ? 0 : -1}
      onclick={() => selectProjectPaneTab("files")} onkeydown={(event) => handleProjectPaneTabKeydown(event, "files")}>
      <IconFiles size={14} stroke={1.8} /><span>{t("project-pane-files")}</span>
    </button>
  </div>

  <!-- ── LAYERS TAB ── -->
  {#if projectPaneTab === "layers"}
    <div class="pane-tab-panel" id="project-pane-panel-layers" role="tabpanel" aria-labelledby="project-pane-tab-layers">
    <EditorNavigationTree
      snapshot={editorNavigationSnapshot}
      loading={editorNavigationLoading}
      error={editorNavigationError}
      selectedNodeIds={coordinatedSelectionNodeIds}
      primaryNodeId={coordinatedPrimaryNodeId}
      hoveredNodeId={hoveredEditorNavigationNodeId}
      openScopeId={editorEditScopeId}
      selectNode={selectEditorNavigationNode}
      hoverNode={hoverEditorNavigationNode}
      enterScope={enterEditorNavigationScope}
      exitScope={exitEditorNavigationScope}
      previewMove={previewEditorNavigationMove}
      moveNode={moveEditorNavigationNode}
      deleteNode={deleteEditorNavigationNode}
      openContextMenu={openEditorNavigationContextMenu}
      openDocument={openEditorNavigationDocument}
      activeDocumentPath={activeScannedPath}
      callerDocumentPath={editorNavigationCallers.at(-1)?.caller ?? null}
      callerTargetDocumentPath={editorNavigationCallers.at(-1)?.target ?? null}
      returnToCaller={returnFromEditorNavigationDocument}
    />
    </div>
  {/if}

  <!-- ── FILES TAB ── -->
  {#if projectPaneTab === "files"}
    <div class="pane-tab-panel" id="project-pane-panel-files" role="tabpanel" aria-labelledby="project-pane-tab-files">
    <ProjectFilesTab
      {scannedProject}
      {projectRoot}
      {runtimeSessionId}
      snapshot={fileExplorerSnapshot}
      loading={fileExplorerLoading}
      error={fileExplorerError}
      bind:collapsedDirs={fileCollapsedDirs}
      bind:knownDirPaths={fileKnownDirPaths}
      bind:revealedEntryKey={fileRevealedEntryKey}
      selectEntry={selectFileExplorerEntry}
      planOperation={planFileExplorerOperation}
      commitOperation={commitFileExplorerOperation}
    />
    </div>
  {/if}

  {#if elementPaletteOpen}
    <div
      bind:this={elementPaletteDialog}
      id="element-palette-dialog"
      class="element-palette-dialog"
      role="dialog"
      tabindex="-1"
      aria-modal="false"
      aria-label={t("project-pane-add-element")}
      onkeydown={handleElementPaletteKeydown}
    >
      <div class="element-palette-body">
        <InsertCatalogPanel
          {projectRoot}
          {runtimeSessionId}
          {workspaceRevision}
          context={insertCatalogContext}
          startDrag={startInsertCatalogDrag}
          closeLabel={t("project-pane-close-add-element")}
          close={() => { void setElementPaletteOpen(false); }}
        />
      </div>
    </div>
  {/if}

</aside>

<style>
  .project-pane {
    --project-pane-padding: 10px;
    position: relative;
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: var(--project-pane-padding);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--material-panel);
    box-shadow: var(--shadow-panel);
  }

  .pane-add-element-btn {
    flex: 0 0 auto;
    width: 100%;
    min-height: 34px;
    border-radius: calc(var(--radius-control) + 1px);
    letter-spacing: 0.005em;
  }
  .pane-add-element-btn :global(svg) { display: block; flex: 0 0 auto; }
  .pane-tab-panel {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    margin-right: calc(-1 * var(--project-pane-padding));
    padding-right: var(--project-pane-padding);
    overflow: auto;
    overscroll-behavior: contain;
  }
  .tab-btn:focus-visible { outline: 2px solid var(--wb-focus-ring, var(--brand-strong)); outline-offset: 1px; }
  /* ── Tabs ── */
  .pane-tabs {
    position: relative;
    z-index: 2;
    flex: 0 0 auto;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .tab-btn {
    width: 100%;
  }

  .tab-btn :global(svg) {
    display: block;
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
    color: currentColor;
    transition: color 120ms ease;
  }

  .element-palette-dialog {
    position: absolute;
    z-index: 12;
    inset: 0;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-radius: inherit;
    overflow: hidden;
    background: var(--material-panel);
    box-shadow: var(--shadow-float);
  }

  .element-palette-body { display: flex; flex: 1 1 auto; min-height: 0; padding: 8px; overflow: hidden; }

  button:disabled { opacity: 0.45; cursor: not-allowed; }
</style>
