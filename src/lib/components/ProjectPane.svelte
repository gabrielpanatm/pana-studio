<script lang="ts">
  import {
    IconFileText,
    IconFiles,
    IconHierarchy2,
    IconStack2,
    IconPlus,
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
  let fileCollapsedDirs = new Set<string>();
  let fileKnownDirPaths = new Set<string>();
  let fileTreeMemoryProjectRoot: string | null = null;

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

<aside class="project-pane" aria-label="Project browser">
  <div class="pane-title">
    <div class="pane-title-main">
      <IconStack2 size={16} stroke={1.8} />
      <h2>Straturi</h2>
    </div>
    <div class="pane-title-actions">
      <button class="pane-action-btn" class:active={projectPaneTab === "structure"} type="button" title="Adaugă element HTML"
        onclick={() => { projectPaneTab = projectPaneTab === "structure" ? "layers" : "structure"; }}>
        <IconPlus size={14} stroke={1.9} />
      </button>
    </div>
  </div>

  <nav class="pane-tabs" aria-label="Taburi panou proiect">
    <button class="tab-btn" class:active={projectPaneTab === "layers"} type="button" title="Straturi"
      onclick={() => { projectPaneTab = "layers"; }}>
      <IconHierarchy2 size={14} stroke={1.8} /><span>Straturi</span>
    </button>
    <button class="tab-btn" class:active={projectPaneTab === "files"} type="button" title="Fișiere"
      onclick={() => { projectPaneTab = "files"; }}>
      <IconFiles size={14} stroke={1.8} /><span>Fișiere</span>
    </button>
    <button class="tab-btn" class:active={projectPaneTab === "page"} type="button" title="Pagină Markdown"
      onclick={() => { projectPaneTab = "page"; }}>
      <IconFileText size={14} stroke={1.8} /><span>Pagină</span>
    </button>
  </nav>

  <!-- ── LAYERS TAB ── -->
  {#if projectPaneTab === "layers"}
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
  {/if}

  <!-- ── FILES TAB ── -->
  {#if projectPaneTab === "files"}
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
  {/if}

  <!-- ── STRUCTURE TAB ── -->
  {#if projectPaneTab === "structure"}
      <ProjectStructureTab
        {selectedElement}
        {sourceGraph}
        {loopPaletteItems}
        {startElementPaletteDrag}
        {startTeraPaletteDrag}
      />
  {/if}

  <!-- ── PAGE SETTINGS TAB ── -->
  {#if projectPaneTab === "page"}
    <ProjectPageSettingsTab
      {activeScannedPath}
      {scannedPages}
      {scannedTemplates}
      activeTheme={sourceGraph?.activeTheme ?? null}
      {pageSource}
      {updatePageFrontmatterSource}
    />
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
    color: var(--text-muted); font-size: 11px; font-weight: 600;
    background: transparent; cursor: pointer;
    transition: color 120ms ease, background 120ms ease, border-color 120ms ease;
  }
  .tab-btn :global(svg) { display: block; flex: 0 0 auto; }
  .tab-btn.active { border-color: var(--border-4); color: var(--text-strong); background: var(--surface-5); }
  .tab-btn:hover:not(.active) { color: var(--text); }

  button:disabled { opacity: 0.45; cursor: not-allowed; }
</style>
