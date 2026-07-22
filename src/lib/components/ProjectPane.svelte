<script lang="ts">
  import { tick } from "svelte";
  import {
    IconFileText,
    IconFiles,
    IconHierarchy2,
    IconPlus,
    IconX,
  } from "@tabler/icons-svelte";
  import type {
    PageSection,
    PreviewSelectionState,
    ProjectFile,
    ProjectPaneTab,
    SelectionInfo,
    SourceGraph,
    TemplateWorkbenchPlan,
  } from "$lib/types";
  import type { EditorLayerContextMenuRequest } from "$lib/editor-runtime/commands";
  import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
  import type { LayerMoveRequest } from "$lib/project/layers-drag";
  import type { FileMoveRequest } from "$lib/project/files-drag";
  import type { FileRenameRequest } from "$lib/project/files-rename";
  import type { HtmlPaletteElement } from "$lib/project/html-palette";
  import type { TeraMoveRequest, TeraPaletteItem } from "$lib/tera/model";
  import ProjectFilesTab from "$lib/components/project/ProjectFilesTab.svelte";
  import ProjectLayersTab from "$lib/components/project/ProjectLayersTab.svelte";
  import ProjectPageSettingsTab from "$lib/components/project/ProjectPageSettingsTab.svelte";
  import ProjectStructureTab from "$lib/components/project/ProjectStructureTab.svelte";

  export let scannedProject = false;
  export let projectRoot = "";
  export let runtimeSessionId = "";
  export let allProjectFiles: ProjectFile[] = [];
  export let scannedPages: ProjectFile[] = [];
  export let scannedStyles: ProjectFile[] = [];
  export let scannedTemplates: ProjectFile[] = [];
  export let scannedScripts: ProjectFile[] = [];
  export let scannedAssets: ProjectFile[] = [];
  export let activeScannedPath: string | null = null;
  export let pageSections: PageSection[] = [];
  export let selectedElement: SelectionInfo | null = null;
  export let previewSelection: PreviewSelectionState = { kind: "none" };
  export let sourceGraph: SourceGraph | null = null;
  export let loopPaletteItems: TeraPaletteItem[] = [];
  export let activeRenderedTemplatePath: string | null = null;
  export let templateHtmlEditSourceId: string | null = null;
  export let templateWorkbenchPlan: TemplateWorkbenchPlan | null = null;
  export let fileMoveBlockedReason = "";
  export let pageSource = "";

  let projectPaneTab: ProjectPaneTab = "layers";
  let elementPaletteOpen = false;
  let elementPaletteTrigger: HTMLButtonElement;
  let elementPaletteClose: HTMLButtonElement;
  let elementPaletteDialog: HTMLElement;
  let fileCollapsedDirs = new Set<string>();
  let fileKnownDirPaths = new Set<string>();
  let fileTreeMemoryProjectRoot: string | null = null;
  $: projectPaneTitle = projectPaneTab === "files"
    ? "Fișiere"
    : projectPaneTab === "page"
      ? "Pagină"
      : "Straturi";

  async function setElementPaletteOpen(open: boolean, restoreFocus = true) {
    elementPaletteOpen = open;
    await tick();
    if (open) elementPaletteClose?.focus();
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

  const projectPaneTabs: ProjectPaneTab[] = ["layers", "files", "page"];
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

  $: if (fileTreeMemoryProjectRoot !== projectRoot) {
    fileCollapsedDirs = new Set<string>();
    fileKnownDirPaths = new Set<string>();
    fileTreeMemoryProjectRoot = projectRoot;
  }

  export let openScannedFile: (file: ProjectFile) => void;
  export let createProjectFile: (relativePath: string, content: string) => Promise<void>;
  export let moveProjectFile: (request: FileMoveRequest) => void | Promise<void>;
  export let renameProjectFile: (request: FileRenameRequest & { type: "file" | "dir" }) => boolean | void | Promise<boolean | void>;
  export let deleteProjectFile: (request: { path: string; type: "file" | "dir" }) => void | Promise<void>;
  export let selectPageSection: (section: PageSection) => void;
  export let selectTeraSource: (section: PageSection, sourceId: string) => void;
  export let hoverPageSection: (section: PageSection | null) => void;
  export let hoverTeraSource: (section: PageSection, sourceId: string) => void;
  export let startElementPaletteDrag: (element: HtmlPaletteElement, event: PointerEvent) => void;
  export let startTeraPaletteDrag: (item: TeraPaletteItem, event: PointerEvent) => void;
  export let moveLayerElement: (request: LayerMoveRequest) => Promise<EditorActionOutcome>;
  export let moveTeraNode: (request: TeraMoveRequest) => void | Promise<void>;
  export let deleteLayerElement: (selector: string) => void | Promise<void>;
  export let openLayerContextMenu: (request: EditorLayerContextMenuRequest) => void;
  export let editSelectedTeraLayer: () => void | Promise<void>;
  export let deleteSelectedTeraNode: () => void | Promise<void>;
  export let openSelectedTeraSource: () => void | Promise<void>;
  export let openTemplateWorkbenchSource: (file: string) => void | Promise<void> = () => {};
  export let updatePageFrontmatterSource: (relativePath: string, source: string) => void;

</script>

<aside class="project-pane" aria-label="Navigator proiect">
  <div class="pane-title">
    <div class="pane-title-main">
      {#if projectPaneTab === "files"}
        <IconFiles size={16} stroke={1.8} />
      {:else if projectPaneTab === "page"}
        <IconFileText size={16} stroke={1.8} />
      {:else if projectPaneTab === "layers"}
        <IconHierarchy2 size={16} stroke={1.8} />
      {/if}
      <h2>{projectPaneTitle}</h2>
    </div>
    <div class="pane-title-actions">
      {#if projectPaneTab === "layers"}
        <button
          bind:this={elementPaletteTrigger}
          class="ui-icon-button pane-action-btn"
          class:active={elementPaletteOpen}
          type="button"
          title="Deschide panoul Adaugă element"
          aria-label="Adaugă element"
          aria-haspopup="dialog"
          aria-expanded={elementPaletteOpen}
          aria-controls="element-palette-dialog"
          onclick={() => { void setElementPaletteOpen(!elementPaletteOpen); }}
        >
          <IconPlus size={14} stroke={1.9} />
        </button>
      {/if}
    </div>
  </div>

  <div class="ui-tabs pane-tabs" role="tablist" aria-label="Zonele panoului de proiect">
    <button id="project-pane-tab-layers" class="ui-tab tab-btn" class:active={projectPaneTab === "layers"} type="button" role="tab" title="Straturi"
      aria-selected={projectPaneTab === "layers"} aria-controls="project-pane-panel-layers" tabindex={projectPaneTab === "layers" ? 0 : -1}
      onclick={() => selectProjectPaneTab("layers")} onkeydown={(event) => handleProjectPaneTabKeydown(event, "layers")}>
      <IconHierarchy2 size={14} stroke={1.8} /><span>Straturi</span>
    </button>
    <button id="project-pane-tab-files" class="ui-tab tab-btn" class:active={projectPaneTab === "files"} type="button" role="tab" title="Fișiere"
      aria-selected={projectPaneTab === "files"} aria-controls="project-pane-panel-files" tabindex={projectPaneTab === "files" ? 0 : -1}
      onclick={() => selectProjectPaneTab("files")} onkeydown={(event) => handleProjectPaneTabKeydown(event, "files")}>
      <IconFiles size={14} stroke={1.8} /><span>Fișiere</span>
    </button>
    <button id="project-pane-tab-page" class="ui-tab tab-btn" class:active={projectPaneTab === "page"} type="button" role="tab" title="Pagină Markdown"
      aria-selected={projectPaneTab === "page"} aria-controls="project-pane-panel-page" tabindex={projectPaneTab === "page" ? 0 : -1}
      onclick={() => selectProjectPaneTab("page")} onkeydown={(event) => handleProjectPaneTabKeydown(event, "page")}>
      <IconFileText size={14} stroke={1.8} /><span>Pagină</span>
    </button>
  </div>

  <!-- ── LAYERS TAB ── -->
  {#if projectPaneTab === "layers"}
    <div class="pane-tab-panel" id="project-pane-panel-layers" role="tabpanel" aria-labelledby="project-pane-tab-layers">
    <ProjectLayersTab
      {pageSections}
      {selectedElement}
      {previewSelection}
      {sourceGraph}
      {activeScannedPath}
      {activeRenderedTemplatePath}
      {templateHtmlEditSourceId}
      {templateWorkbenchPlan}
      {selectPageSection}
      {selectTeraSource}
      {hoverPageSection}
      {hoverTeraSource}
      {moveLayerElement}
      {moveTeraNode}
      {deleteLayerElement}
      {openLayerContextMenu}
      {editSelectedTeraLayer}
      {deleteSelectedTeraNode}
      {openSelectedTeraSource}
      {openTemplateWorkbenchSource}
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
      {allProjectFiles}
      {scannedPages}
      {scannedStyles}
      {scannedTemplates}
      {scannedScripts}
      {scannedAssets}
      {activeScannedPath}
      {fileMoveBlockedReason}
      bind:collapsedDirs={fileCollapsedDirs}
      bind:knownDirPaths={fileKnownDirPaths}
      {openScannedFile}
      {createProjectFile}
      {moveProjectFile}
      {renameProjectFile}
      {deleteProjectFile}
    />
    </div>
  {/if}

  <!-- ── PAGE SETTINGS TAB ── -->
  {#if projectPaneTab === "page"}
    <div class="pane-tab-panel" id="project-pane-panel-page" role="tabpanel" aria-labelledby="project-pane-tab-page">
    <ProjectPageSettingsTab
      {activeScannedPath}
      {scannedPages}
      {scannedTemplates}
      activeTheme={sourceGraph?.activeTheme ?? null}
      {pageSource}
      {updatePageFrontmatterSource}
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
      aria-labelledby="element-palette-title"
      aria-describedby="element-palette-description"
      onkeydown={handleElementPaletteKeydown}
    >
      <header class="element-palette-header">
        <div>
          <h2 id="element-palette-title">Adaugă element</h2>
          <p id="element-palette-description">Trage un element HTML, o componentă sau o structură Tera în suprafața vizuală.</p>
        </div>
        <button
          bind:this={elementPaletteClose}
          type="button"
          class="ui-icon-button pane-action-btn"
          title="Închide panoul Adaugă element"
          aria-label="Închide panoul Adaugă element"
          onclick={() => { void setElementPaletteOpen(false); }}
        >
          <IconX size={16} stroke={1.9} />
        </button>
      </header>
      <div class="element-palette-body">
        <ProjectStructureTab
          {selectedElement}
          {sourceGraph}
          {loopPaletteItems}
          {startElementPaletteDrag}
          {startTeraPaletteDrag}
        />
      </div>
    </div>
  {/if}

</aside>

<style>
  .project-pane {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: auto;
    overscroll-behavior: contain;
    box-shadow: var(--shadow);
    background: var(--surface);
  }

  .pane-title { display: flex; align-items: center; justify-content: space-between; }
  .pane-title h2 { margin: 0; font-size: 13px; font-weight: 800; letter-spacing: 0.01em; }
  .pane-title-main { display: inline-flex; align-items: center; gap: 6px; }
  .pane-title-main :global(svg) { display: block; flex: 0 0 auto; }
  .pane-title-actions { display: inline-flex; align-items: center; gap: 4px; }
  .pane-tab-panel { display: flex; flex: 1 1 auto; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; }
  .pane-action-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 24px; height: 24px; padding: 0;
    border: 1px solid var(--border-3); border-radius: 6px;
    background: var(--surface-4); color: var(--text-muted); cursor: pointer;
    transition: color 120ms, border-color 120ms, background 120ms;
  }
  .pane-action-btn :global(svg) { display: block; }
  .pane-action-btn:hover { color: var(--text); border-color: var(--border-4); }
  .pane-action-btn.active { border-color: var(--brand); color: var(--brand-strong); background: var(--brand-soft); }
  .pane-action-btn:focus-visible,
  .tab-btn:focus-visible { outline: 2px solid var(--wb-focus-ring, var(--brand-strong)); outline-offset: 1px; }
  /* ── Tabs ── */
  .pane-tabs {
    position: relative;
    z-index: 2;
    flex: 0 0 auto;
    display: grid; grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 3px; padding: 3px; border: 1px solid var(--border-3);
    border-radius: 8px; background: var(--surface-2);
  }
  .tab-btn {
    display: inline-flex; align-items: center; justify-content: center; gap: 5px;
    width: 100%; min-height: 26px; padding: 0 5px;
    border: 1px solid transparent; border-radius: 6px;
    color: var(--text-muted); font-size: 12px; font-weight: 600;
    background: transparent; cursor: pointer;
    transition: color 120ms ease, background 120ms ease, border-color 120ms ease;
  }
  .tab-btn :global(svg) { display: block; flex: 0 0 auto; }
  .tab-btn.active { border-color: var(--border-4); color: var(--text-strong); background: var(--surface-5); }
  .tab-btn:hover:not(.active) { color: var(--text); }

  .element-palette-dialog {
    position: absolute;
    z-index: 12;
    inset: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    border-radius: inherit;
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 18px 40px rgba(0, 0, 0, .24);
  }

  .element-palette-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    padding: 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }

  .element-palette-header > div { display: grid; gap: 4px; min-width: 0; }
  .element-palette-header h2 { margin: 0; color: var(--text-strong); font-size: 14px; }
  .element-palette-header p { margin: 0; color: var(--text-muted); font-size: 12px; line-height: 1.4; }
  .element-palette-body { min-height: 0; padding: 8px; overflow: auto; }

  button:disabled { opacity: 0.45; cursor: not-allowed; }
</style>
