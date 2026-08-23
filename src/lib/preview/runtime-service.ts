import type { ApplicationPreferencesState } from "$lib/application/preferences.svelte";
import type { CanvasInteractionControllerHost } from "$lib/state/canvas-interaction-host";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { EditorSelectionService } from "$lib/editor/selection-service";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { PageSectionsState } from "$lib/preview/page-sections.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { WorkspaceAuthorityService } from "$lib/session/workspace-authority-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { PreviewInsertService } from "$lib/preview/insert-service";
import {
  applyStagedOverrideStylesToPreview,
  attachPreviewInspector,
  handlePreviewMessage,
  refreshSourceGraph,
  resolveSourceEditLocationForSourceId,
  resolveSourceEditTargetForSourceId,
  syncHtmlCodeToPreview,
  type AppPreviewRuntimeControllerHost,
} from "$lib/state/app-preview-runtime-controller";
import type { EditableStyles } from "$lib/css/contracts";

export type PreviewRuntimeServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  analysis: ProjectAnalysisState;
  css: CssAuthoringState;
  selection: SelectionWorkspaceState;
  selectionService: EditorSelectionService;
  sections: PageSectionsState;
  preview: PreviewWorkspaceState;
  canvas: CanvasInteractionControllerHost;
  authority: WorkspaceAuthorityService;
  inserts: PreviewInsertService;
  preferences: ApplicationPreferencesState;
  status: GlobalStatusState;
  restoreLiveCssLayers: () => void;
}>;

/** Owns the Preview bridge message flow and derived SourceGraph projection. */
export class PreviewRuntimeService {
  private readonly dependencies: PreviewRuntimeServiceDependencies;

  constructor(dependencies: PreviewRuntimeServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): AppPreviewRuntimeControllerHost {
    const d = this.dependencies;
    const owner = this;
    return {
      get sessionProjectRoot() { return d.project.root; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      get projectSessionEpoch() { return d.project.epoch; },
      get projectTransitionFrontendLeaseActive() { return d.authority.structuralHost().projectTransitionFrontendLeaseActive; },
      get kernelUndoRedoFrontendLeaseActive() { return d.authority.structuralHost().kernelUndoRedoFrontendLeaseActive; },
      get aiEditLeaseFrontendLockActive() { return d.authority.structuralHost().aiEditLeaseFrontendLockActive; },
      editorSelection: d.selection.session,
      beginPreviewStructuralWriteBoundary: () => (
        d.authority.structuralHost().beginPreviewStructuralWriteBoundary()
      ),
      endPreviewStructuralWriteBoundary: () => (
        d.authority.structuralHost().endPreviewStructuralWriteBoundary()
      ),
      get scannedProject() { return d.project.project; },
      get previewWorkspaceRevision() { return d.preview.workspaceRevision; },
      set previewWorkspaceRevision(revision) { d.preview.workspaceRevision = revision; },
      get pendingCanvasProjection() { return d.preview.pendingProjection; },
      set pendingCanvasProjection(plan) { d.preview.setPendingProjection(plan); },
      canProjectWorkspacePreview: () => d.preview.canProjectWorkspacePreview(),
      deferWorkspacePreviewProjection: () => d.preview.deferSurfaceProjection(),
      get templateWorkbenchActive() { return d.documents.templateActive; },
      reprojectActiveTemplateWorkbench: (minimumRevision) => (
        d.authority.previewHost().reprojectActiveTemplateWorkbench?.(minimumRevision)
          ?? Promise.resolve(false)
      ),
      requestPreviewRefresh: (reason) => d.preview.requestRefresh(reason),
      requestWorkspaceProjectionPreviewRefresh: (reason) => (
        d.preview.requestWorkspaceProjectionRefresh(reason)
      ),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
      handlePreviewInsertDrop: (payload) => d.inserts.handleHtml(payload),
      handlePreviewTeraInsertDrop: (payload) => d.inserts.handleTera(payload),
      canvasInteraction: d.canvas,
      get coordinatedElementSelection() { return d.selection.coordinatedElement; },
      get currentSourceRelativePath() { return d.source.currentSourceRelativePath; },
      get isActiveRenderedPreviewPage() { return d.documents.isActiveRenderedPreviewPage; },
      get latestPreviewMessageRevision() { return d.preview.latestMessageRevision; },
      set latestPreviewMessageRevision(revision) { d.preview.latestMessageRevision = revision; },
      get overrideRules() { return d.css.overrideRules; },
      set overrideRules(rules: Record<string, EditableStyles>) { d.css.overrideRules = rules; },
      get pageSections() { return d.sections.sections; },
      set pageSections(sections) { d.sections.set(sections); },
      previewCommands: () => d.preview.commands(),
      get previewDocumentMarkup() { return d.preview.documentMarkup; },
      set previewDocumentMarkup(markup) { d.preview.documentMarkup = markup; },
      previewRuntime: d.preview.runtime,
      get previewSyncTimer() { return d.preview.syncTimer; },
      set previewSyncTimer(timer) { d.preview.syncTimer = timer; },
      get projectWorkspaceSnapshot() { return d.project.workspace; },
      set projectWorkspaceSnapshot(snapshot) { d.project.workspace = snapshot; },
      get sourceGraph() { return d.analysis.sourceGraph; },
      set sourceGraph(graph) { d.analysis.sourceGraph = graph; },
      get sourceGraphLoadSerial() { return d.analysis.sourceGraphLoadSerial; },
      set sourceGraphLoadSerial(serial) { d.analysis.sourceGraphLoadSerial = serial; },
      get sourceGraphProjectionStatus() { return d.analysis.sourceGraphProjectionStatus; },
      set sourceGraphProjectionStatus(status) { d.analysis.sourceGraphProjectionStatus = status; },
      get sourceGraphWorkspaceRevision() { return d.analysis.sourceGraphWorkspaceRevision; },
      set sourceGraphWorkspaceRevision(revision) { d.analysis.sourceGraphWorkspaceRevision = revision; },
      preferences: d.preferences,
      get variableOverrides() { return d.css.variableOverrides; },
      set variableOverrides(overrides) { d.css.variableOverrides = overrides; },
      applySelectionState: (selection) => d.selectionService.apply(selection),
      applyStagedOverrideStylesToPreview: (css) => owner.applyStagedOverrideStyles(css),
      cancelPreviewSync: () => d.preview.cancelSync(),
      fetchDomTreeFromPreview: () => d.preview.fetchDomTree(),
      getPreviewDocument: () => d.preview.getDocument(),
      hydratePageSections: (sections) => d.sections.hydrate(sections),
      restoreLiveCssLayersToPreview: d.restoreLiveCssLayers,
      setPageSections: (sections) => d.sections.set(sections),
    };
  }

  refreshSourceGraph(options: { strict?: boolean } = {}) {
    return refreshSourceGraph(this.host(), options);
  }

  resolveSourceEditTarget(sourceId: string | null | undefined) {
    return resolveSourceEditTargetForSourceId(this.host(), sourceId);
  }

  resolveSourceEditLocation(sourceId: string | null | undefined) {
    return resolveSourceEditLocationForSourceId(this.host(), sourceId);
  }

  syncHtmlCode(sourceText: string, cursorPosition: number) {
    syncHtmlCodeToPreview(this.host(), sourceText, cursorPosition);
  }

  attachInspector() {
    attachPreviewInspector(this.host());
  }

  applyStagedOverrideStyles(css: string) {
    applyStagedOverrideStylesToPreview(this.host(), css);
  }

  handleMessage(event: MessageEvent) {
    handlePreviewMessage(this.host(), event);
  }
}
