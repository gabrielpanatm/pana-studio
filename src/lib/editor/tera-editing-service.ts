import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { EditorNavigationService } from "$lib/editor/navigation-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { EditorTeraTarget } from "$lib/editor-runtime/commands";
import type { TeraDropRequest } from "$lib/tera/model";
import {
  deleteSelectedTeraNode,
  insertTeraPaletteItemAtTarget,
  type TeraActionsControllerHost,
} from "$lib/state/tera-actions-controller";

export type TeraEditingServiceDependencies = Readonly<{
  analysis: ProjectAnalysisState;
  documents: ProjectDocumentWorkspaceState;
  selection: SelectionWorkspaceState;
  source: SourceWorkspaceState;
  navigation: EditorNavigationService;
  status: GlobalStatusState;
  runStructural: TeraActionsControllerHost["runStructural"];
  projectCommitted: TeraActionsControllerHost["projectCommitted"];
}>;

/** Owns Tera insert/delete commands and their post-commit selection. */
export class TeraEditingService {
  private readonly controller: TeraActionsControllerHost;

  constructor(dependencies: TeraEditingServiceDependencies) {
    this.controller = {
      context: () => ({
        sourceGraph: dependencies.analysis.sourceGraph,
        selectedTemplateSourceNode: dependencies.selection.selectedTemplateSourceNode,
        activeScannedPath: dependencies.documents.activeScannedPath,
        activeRenderedTemplatePath: dependencies.documents.activeRenderedTemplatePath,
      }),
      source: dependencies.source,
      runStructural: dependencies.runStructural,
      projectCommitted: dependencies.projectCommitted,
      selectDynamicWidgetSourceInstance: (instanceId) => (
        dependencies.navigation.selectDynamicWidgetSourceInstance(instanceId)
      ),
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
    };
  }

  host() { return this.controller; }

  insert(request: TeraDropRequest) {
    return insertTeraPaletteItemAtTarget(this.controller, request);
  }

  delete(target: EditorTeraTarget | null = null) {
    const sourceNode = target
      ? target.sourceNode ?? null
      : this.controller.context().selectedTemplateSourceNode;
    return deleteSelectedTeraNode(this.controller, sourceNode);
  }
}
