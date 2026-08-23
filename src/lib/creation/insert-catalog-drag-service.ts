import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  startInsertCatalogDrag,
  type InsertCatalogDragHost,
} from "$lib/state/insert-catalog-drag-controller";
import {
  INSERT_CATALOG_SCHEMA_VERSION,
  type InsertCatalogItem,
  type InsertCatalogSnapshot,
} from "$lib/blocks/contracts";

export type InsertCatalogDragServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  selection: SelectionWorkspaceState;
  surface: PreviewSurfaceState;
  preview: PreviewWorkspaceState;
  shell: ApplicationShellState;
  workbench: WorkbenchWorkspaceState;
  status: GlobalStatusState;
}>;

/** Validates catalog snapshot authority before starting a Canvas drag. */
export class InsertCatalogDragService {
  private readonly dependencies: InsertCatalogDragServiceDependencies;

  constructor(dependencies: InsertCatalogDragServiceDependencies) {
    this.dependencies = dependencies;
  }

  private host(): InsertCatalogDragHost {
    const { shell, surface, workbench, preview, status } = this.dependencies;
    return {
      centerView: shell.centerView,
      previewFrame: surface.frame,
      previewZoom: workbench.previewZoom,
      postPreviewMessage: (payload) => preview.postMessage(payload),
      setGlobalStatus: (text, kind) => status.set(text, kind),
    };
  }

  start(item: InsertCatalogItem, snapshot: InsertCatalogSnapshot, event: PointerEvent) {
    const { project, documents, selection, preview, shell } = this.dependencies;
    const currentRevision = project.workspace?.revision ?? null;
    const context = snapshot.context;
    if (
      snapshot.schemaVersion !== INSERT_CATALOG_SCHEMA_VERSION
      || snapshot.projectRoot !== project.root
      || snapshot.runtimeSessionId !== project.runtimeSessionId
      || snapshot.workspaceRevision !== currentRevision
      || context.activeDocumentPath !== documents.activeScannedPath
      || context.activeTemplatePath !== documents.activeRenderedTemplatePath
      || context.activePagePath !== documents.templatePreferredPagePath
      || context.canvasPreviewRevision !== (preview.activeIdentity?.previewRevision ?? null)
      || context.canvasAvailable !== (shell.centerView === "preview" && Boolean(preview.activeIdentity))
      || context.targetSourceId !== (selection.coordinatedElement?.sourceNodeId ?? null)
      || context.targetTag !== (selection.coordinatedElement?.observation.tag ?? null)
    ) {
      this.dependencies.status.set(
        "Catalogul de inserare s-a actualizat. Reia tragerea din lista curentă.",
        "error",
      );
      return;
    }
    startInsertCatalogDrag(this.host(), item, event);
  }
}
