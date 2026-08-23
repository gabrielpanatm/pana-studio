import type { EditorRuntime } from "$lib/editor-runtime/runtime";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";

export type CanvasInteractionWorkspaceDependencies = {
  preview: PreviewWorkspaceState;
  surface: PreviewSurfaceState;
  documents: ProjectDocumentWorkspaceState;
  shell: ApplicationShellState;
  project: ProjectSessionState;
  workbench: WorkbenchWorkspaceState;
  selection: SelectionWorkspaceState;
  analysis: ProjectAnalysisState;
  editorRuntime: () => EditorRuntime;
  commands: CanvasInteractionControllerHost["commands"];
};

/** Stable, domain-owned adapter shared by Canvas messages, effects and gestures. */
export class CanvasInteractionWorkspace implements CanvasInteractionControllerHost {
  readonly session: CanvasInteractionControllerHost["session"];
  readonly selection: CanvasInteractionControllerHost["selection"];
  readonly runtime: CanvasInteractionControllerHost["runtime"];
  readonly commands: CanvasInteractionControllerHost["commands"];

  constructor(dependencies: CanvasInteractionWorkspaceDependencies) {
    this.session = {
      get activeCanvasIdentity() { return dependencies.preview.activeIdentity; },
      get activeCanvasUrl() { return dependencies.preview.activeUrl; },
      get activeScannedPath() { return dependencies.documents.activeScannedPath; },
      get applicationSurface() { return dependencies.shell.surface; },
      get browserPreviewRoute() { return dependencies.documents.browserPreviewRoute; },
      get centerView() { return dependencies.shell.centerView; },
      get previewFrame() { return dependencies.surface.frame; },
      get previewSrc() { return dependencies.preview.src; },
      get scannedProject() { return dependencies.project.project; },
      get workbenchSnapshot() { return dependencies.workbench.snapshot; },
    };
    this.selection = {
      get coordinatedElementSelection() { return dependencies.selection.coordinatedElement; },
      editorSelection: dependencies.selection.session,
      get sourceGraph() { return dependencies.analysis.sourceGraph; },
    };
    this.runtime = {
      get editorRuntime() { return dependencies.editorRuntime(); },
      get gridOverlayEnabled() { return dependencies.preview.gridOverlayEnabled; },
    };
    this.commands = dependencies.commands;
  }
}
