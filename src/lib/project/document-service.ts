import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { TemplateWorkbenchService } from "$lib/project/template-workbench-service";
import type { WorkspaceAuthorityService } from "$lib/session/workspace-authority-service";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import { previewStructuralCommandIdentity } from "$lib/kernel/preview-structural-lane";
import {
  createContentPageFromInput,
  loadScannedProjectFile,
  type ProjectDocumentHost,
} from "$lib/state/project-document-controller";
import type { ProjectFile } from "$lib/project/lifecycle-contract";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type ProjectDocumentServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  preview: PreviewWorkspaceState;
  shell: ApplicationShellState;
  template: TemplateWorkbenchService;
  authority: WorkspaceAuthorityService;
  workbench: WorkbenchWorkspaceState;
  selection: SelectionWorkspaceState;
  status: GlobalStatusState;
}>;

export type ProjectFileLoadOptions = {
  strict?: boolean;
  skipDraftFlush?: boolean;
  deferPreviewRefresh?: boolean;
  activateTemplateWorkbench?: boolean;
  preferredTemplatePagePath?: string | null;
  preferredTemplateRoute?: string | null;
  syncWorkbench?: boolean;
};

/** Owns project document creation, loading and Workbench synchronization. */
export class ProjectDocumentService {
  private readonly dependencies: ProjectDocumentServiceDependencies;

  constructor(dependencies: ProjectDocumentServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): ProjectDocumentHost {
    const d = this.dependencies;
    return {
      get source() { return d.source.source; },
      set source(source) { d.source.source = source; },
      get sourceCache() { return d.source.sourceCache; },
      set sourceCache(cache) { d.source.sourceCache = cache; },
      get activeScannedPath() { return d.documents.activeScannedPath; },
      set activeScannedPath(path) { d.documents.activeScannedPath = path; },
      get activePreviewPath() { return d.documents.activePreviewPath; },
      set activePreviewPath(path) { d.documents.activePreviewPath = path; },
      get browserPreviewRoute() { return d.documents.browserPreviewRoute; },
      set browserPreviewRoute(route) { d.documents.browserPreviewRoute = route; },
      get previewSrc() { return d.preview.src; },
      set previewSrc(src) { d.preview.src = src; },
      get previewDocumentMarkup() { return d.preview.documentMarkup; },
      set previewDocumentMarkup(markup) { d.preview.documentMarkup = markup; },
      get pendingCanvasProjection() { return d.preview.pendingProjection; },
      set pendingCanvasProjection(plan) { d.preview.setPendingProjection(plan); },
      get centerView() { return d.shell.centerView; },
      set centerView(view) { d.shell.centerView = view; },
      get templateWorkbenchPlan() { return d.documents.templatePlan; },
      set templateWorkbenchPlan(plan) { d.documents.templatePlan = plan; },
      get templateWorkbenchPreferredPagePath() { return d.documents.templatePreferredPagePath; },
      set templateWorkbenchPreferredPagePath(path) { d.documents.templatePreferredPagePath = path; },
      get templateWorkbenchPreferredRoute() { return d.documents.templatePreferredRoute; },
      set templateWorkbenchPreferredRoute(route) { d.documents.templatePreferredRoute = route; },
      get templateWorkbenchActive() { return d.documents.templateActive; },
      set templateWorkbenchActive(active) { d.documents.templateActive = active; },
      get templateWorkbenchTarget() { return d.documents.templateTarget; },
      set templateWorkbenchTarget(target) { d.documents.templateTarget = target; },
      get projectStatus() { return d.project.status; },
      set projectStatus(status) { d.project.status = status; },
      get scannedProject() { return d.project.project; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      get projectSessionEpoch() { return d.project.epoch; },
      runProjectDocumentStructuralLane: (operation) => d.authority.runStructural(
        async (lease) => await operation({
          identity: previewStructuralCommandIdentity(lease),
          isCurrent: () => d.authority.leaseMatches(lease),
          requireCurrent: () => d.authority.requireLease(lease),
        }),
      ),
      settleProjectDocumentMutation: (receipt, options) => d.authority.settle(receipt, options),
      flushInteractiveEditorDrafts: () => flushWorkspaceMutationInputs("manual"),
      previewUrlForScannedFile: (file) => d.preview.urlForFile(file),
      refreshRenderedPreviewDocument: () => d.preview.refreshDocument(),
      cancelPreviewSync: () => d.preview.cancelSync(),
      exitTemplateWorkbench: (options) => d.template.exit(options),
      updateTemplateWorkbenchContext: (project, file, path, options) => (
        d.template.update(project, file, path, options)
      ),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
    };
  }

  create(input: { title: string; slug?: string | null; section?: string | null }) {
    return createContentPageFromInput(this.host(), input);
  }

  async load(file: ProjectFile, options: ProjectFileLoadOptions = {}) {
    const d = this.dependencies;
    const workbenchSessionId = d.project.runtimeSessionId;
    const shouldSyncWorkbench = options.syncWorkbench !== false
      && d.workbench.isHydrated(workbenchSessionId);
    await loadScannedProjectFile(this.host(), file, options);
    if (
      shouldSyncWorkbench
      && d.project.runtimeSessionId === workbenchSessionId
      && d.documents.activeScannedPath === file.relativePath
      && d.project.root
      && d.project.runtimeSessionId
    ) {
      try {
        await d.workbench.openDocument(file, d.shell.centerView);
        d.status.clear("workbench.document-sync");
        if (d.preview.activeIdentity && file.role === "template") {
          await d.selection.session.refreshNavigationSnapshot();
        }
      } catch (error) {
        d.status.escalate({
          id: "workbench.document-sync",
          level: "warning",
          title: t("workbench-document-sync-failed"),
          message: errorMessage(error),
        });
      }
    }
  }
}
