import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { PageSectionsState } from "$lib/preview/page-sections.svelte";
import type { VersionPreviewState } from "$lib/versioning/preview-state.svelte";
import type { EditorSelectionService } from "$lib/editor/selection-service";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { FileExplorerWorkspaceState } from "$lib/workbench/file-explorer-state.svelte";
import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";
import type { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  resetProjectScopedState,
  type ProjectSessionResetOptions,
} from "$lib/state/project-session-reset";

export type ProjectResetSources = Readonly<{
  documents: ProjectDocumentWorkspaceState;
  analysis: ProjectAnalysisState;
  source: SourceWorkspaceState;
}>;

export type ProjectResetPreview = Readonly<{
  workspace: PreviewWorkspaceState;
  sections: PageSectionsState;
  version: VersionPreviewState;
  selection: EditorSelectionService;
}>;

export type ProjectResetEditor = Readonly<{
  html: HtmlAuthoringState;
  css: CssAuthoringState;
  selection: SelectionWorkspaceState;
  runtime: EditorInteractionRuntime;
}>;

export type ProjectResetWorkspace = Readonly<{
  project: ProjectSessionState;
  workbench: WorkbenchWorkspaceState;
  explorer: FileExplorerWorkspaceState;
  publish: PublishWorkspaceState;
  acceptedDisk: AcceptedDiskState;
  externalDisk: ExternalDiskState;
  status: GlobalStatusState;
}>;

export type ProjectResetServiceDependencies = Readonly<{
  sources: ProjectResetSources;
  preview: ProjectResetPreview;
  editor: ProjectResetEditor;
  workspace: ProjectResetWorkspace;
  setProjectRoot: (root?: string) => void;
}>;

/** Deterministically clears every project-scoped projection after close/reload. */
export class ProjectResetService {
  private readonly dependencies: ProjectResetServiceDependencies;

  constructor(dependencies: ProjectResetServiceDependencies) {
    this.dependencies = dependencies;
  }

  reset(options: ProjectSessionResetOptions = {}) {
    const d = this.dependencies;
    const host = {
      get source() { return d.sources.source.source; },
      set source(source) { d.sources.source.source = source; },
      get sourceCache() { return d.sources.source.sourceCache; },
      set sourceCache(cache) { d.sources.source.sourceCache = cache; },
      get activeScannedPath() { return d.sources.documents.activeScannedPath; },
      set activeScannedPath(path) { d.sources.documents.activeScannedPath = path; },
      get sourceGraph() { return d.sources.analysis.sourceGraph; },
      set sourceGraph(graph) { d.sources.analysis.sourceGraph = graph; },
      get sourceGraphProjectionStatus() { return d.sources.analysis.sourceGraphProjectionStatus; },
      set sourceGraphProjectionStatus(status) { d.sources.analysis.sourceGraphProjectionStatus = status; },
      get sourceGraphWorkspaceRevision() { return d.sources.analysis.sourceGraphWorkspaceRevision; },
      set sourceGraphWorkspaceRevision(revision) { d.sources.analysis.sourceGraphWorkspaceRevision = revision; },
      get scssVariables() { return d.sources.analysis.scssVariables; },
      set scssVariables(variables) { d.sources.analysis.scssVariables = variables; },
      get targetCssFile() { return d.editor.css.targetFile; },
      set targetCssFile(file) { d.editor.css.targetFile = file; },
      get previewSrc() { return d.preview.workspace.src; },
      set previewSrc(src) { d.preview.workspace.src = src; },
      get activePreviewPath() { return d.sources.documents.activePreviewPath; },
      set activePreviewPath(path) { d.sources.documents.activePreviewPath = path; },
      get browserPreviewRoute() { return d.sources.documents.browserPreviewRoute; },
      set browserPreviewRoute(route) { d.sources.documents.browserPreviewRoute = route; },
      get previewDocumentMarkup() { return d.preview.workspace.documentMarkup; },
      set previewDocumentMarkup(markup) { d.preview.workspace.documentMarkup = markup; },
      get previewWorkspaceRevision() { return d.preview.workspace.workspaceRevision; },
      set previewWorkspaceRevision(revision) { d.preview.workspace.workspaceRevision = revision; },
      get activeVersionPreview() { return d.preview.version.active; },
      set activeVersionPreview(version) { d.preview.version.active = version; },
      clearPreviewSelection: (options?: { clearCanvasOverlay?: boolean }) => d.preview.selection.clear(options),
      resetControlledPreviewState: () => d.preview.workspace.resetControlled(),
      resetPageSections: () => d.preview.sections.reset(),
      get templateWorkbenchPlan() { return d.sources.documents.templatePlan; },
      set templateWorkbenchPlan(plan) { d.sources.documents.templatePlan = plan; },
      get templateWorkbenchPreferredPagePath() { return d.sources.documents.templatePreferredPagePath; },
      set templateWorkbenchPreferredPagePath(path) { d.sources.documents.templatePreferredPagePath = path; },
      get templateWorkbenchPreferredRoute() { return d.sources.documents.templatePreferredRoute; },
      set templateWorkbenchPreferredRoute(route) { d.sources.documents.templatePreferredRoute = route; },
      get templateWorkbenchReuseToken() { return d.sources.documents.templateReuseToken; },
      set templateWorkbenchReuseToken(token) { d.sources.documents.templateReuseToken = token; },
      get templateWorkbenchActive() { return d.sources.documents.templateActive; },
      set templateWorkbenchActive(active) { d.sources.documents.templateActive = active; },
      get templateWorkbenchTarget() { return d.sources.documents.templateTarget; },
      set templateWorkbenchTarget(target) { d.sources.documents.templateTarget = target; },
      get templateWorkbenchReturnPreviewPath() { return d.sources.documents.templateReturnPreviewPath; },
      set templateWorkbenchReturnPreviewPath(path) { d.sources.documents.templateReturnPreviewPath = path; },
      get templateWorkbenchRequestSerial() { return d.sources.documents.templateRequestSerial; },
      set templateWorkbenchRequestSerial(serial) { d.sources.documents.templateRequestSerial = serial; },
      get overrideRules() { return d.editor.css.overrideRules; },
      set overrideRules(rules) { d.editor.css.overrideRules = rules; },
      get variableOverrides() { return d.editor.css.variableOverrides; },
      set variableOverrides(overrides) { d.editor.css.variableOverrides = overrides; },
      get htmlPending() { return d.editor.html.htmlPending; },
      set htmlPending(pending) { d.editor.html.htmlPending = pending; },
      get inspectorPending() { return d.editor.html.inspectorPending; },
      set inspectorPending(pending) { d.editor.html.inspectorPending = pending; },
      get pendingTag() { return d.editor.html.pendingTag; },
      set pendingTag(tag) { d.editor.html.pendingTag = tag; },
      get pendingTagOriginal() { return d.editor.html.pendingTagOriginal; },
      set pendingTagOriginal(tag) { d.editor.html.pendingTagOriginal = tag; },
      get pendingTagSourceLocation() { return d.editor.html.pendingTagSourceLocation; },
      set pendingTagSourceLocation(location) { d.editor.html.pendingTagSourceLocation = location; },
      get tagStatus() { return d.editor.html.tagStatus; },
      set tagStatus(status) { d.editor.html.tagStatus = status; },
      editorSelection: d.editor.selection.session,
      resetInspectorPendingSources: () => d.editor.html.resetPendingSources(),
      cancelPendingHtmlMutations: () => {
        d.editor.runtime.htmlDraft.cancel();
        d.editor.html.mutationRevision += 1;
      },
      get projectWorkspaceSnapshot() { return d.workspace.project.workspace; },
      set projectWorkspaceSnapshot(snapshot) { d.workspace.project.workspace = snapshot; },
      get workbenchSnapshot() { return d.workspace.workbench.snapshot; },
      set workbenchSnapshot(snapshot) {
        if (snapshot) d.workspace.workbench.acceptSnapshot(snapshot);
        else d.workspace.workbench.reset();
      },
      get fileExplorerSnapshot() { return d.workspace.explorer.snapshot; },
      set fileExplorerSnapshot(snapshot) { d.workspace.explorer.snapshot = snapshot; },
      get fileExplorerLoading() { return d.workspace.explorer.loading; },
      set fileExplorerLoading(loading) { d.workspace.explorer.loading = loading; },
      get fileExplorerError() { return d.workspace.explorer.error; },
      set fileExplorerError(error) { d.workspace.explorer.error = error; },
      publishWorkspace: d.workspace.publish,
      get diskState() { return d.workspace.acceptedDisk.snapshot; },
      set diskState(snapshot) { d.workspace.acceptedDisk.snapshot = snapshot; },
      get kernelProjectSessionId() { return d.workspace.project.runtimeSessionId; },
      set kernelProjectSessionId(sessionId) { d.workspace.project.runtimeSessionId = sessionId; },
      get refreshToken() { return d.workspace.project.refreshToken; },
      set refreshToken(token) { d.workspace.project.refreshToken = token; },
      setSessionProjectRoot: this.dependencies.setProjectRoot,
      resetExternalDiskState: () => d.workspace.externalDisk.reset(),
      setGlobalStatus: (text: string, kind: Parameters<GlobalStatusState["set"]>[1]) => (
        d.workspace.status.set(text, kind)
      ),
    };
    resetProjectScopedState(host, options);
  }

  resetWithWorkbench(options: ProjectSessionResetOptions = {}) {
    this.reset(options);
    this.dependencies.workspace.workbench.reset();
  }
}
