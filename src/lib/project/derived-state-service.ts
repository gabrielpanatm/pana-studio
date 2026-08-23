import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { PreviewRuntimeService } from "$lib/preview/runtime-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  reconcileWorkspaceDerivedState,
  rescanCurrentProject,
  rescanCurrentProjectForCommittedHistory,
  type ProjectDerivedStateHost,
} from "$lib/state/project-derived-state-controller";
import type {
  CommittedHistoryProjectionContext,
  ReconcileWorkspaceDerivedStateOptions,
} from "$lib/project/controller-contracts";
import type { ProjectFile } from "$lib/project/lifecycle-contract";

export type ProjectDerivedStateServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  analysis: ProjectAnalysisState;
  disk: AcceptedDiskState;
  externalDisk: ExternalDiskState;
  transition: ProjectTransitionLeaseState;
  preview: PreviewWorkspaceState;
  previewRuntime: PreviewRuntimeService;
  status: GlobalStatusState;
  loadFile: (
    file: ProjectFile,
    options?: { strict?: boolean; skipDraftFlush?: boolean; deferPreviewRefresh?: boolean },
  ) => Promise<void>;
}>;

/** Owns rescan and derived projection reconciliation for the active Rust session. */
export class ProjectDerivedStateService {
  private readonly dependencies: ProjectDerivedStateServiceDependencies;

  constructor(dependencies: ProjectDerivedStateServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): ProjectDerivedStateHost {
    const d = this.dependencies;
    return {
      get activeScannedPath() { return d.documents.activeScannedPath; },
      set activeScannedPath(path) { d.documents.activeScannedPath = path; },
      get diskState() { return d.disk.snapshot; },
      set diskState(snapshot) { d.disk.snapshot = snapshot; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      get projectSessionEpoch() { return d.project.epoch; },
      get projectStatus() { return d.project.status; },
      set projectStatus(status) { d.project.status = status; },
      get projectWorkspaceSnapshot() { return d.project.workspace; },
      set projectWorkspaceSnapshot(snapshot) { d.project.workspace = snapshot; },
      get refreshToken() { return d.project.refreshToken; },
      set refreshToken(token) { d.project.refreshToken = token; },
      get scannedProject() { return d.project.project; },
      set scannedProject(project) { d.project.project = project; },
      get scssVariables() { return d.analysis.scssVariables; },
      set scssVariables(variables) { d.analysis.scssVariables = variables; },
      get sessionProjectRoot() { return d.project.root; },
      loadScannedProjectFile: d.loadFile,
      refreshSourceGraph: async (options) => { await d.previewRuntime.refreshSourceGraph(options); },
      requestPreviewRefresh: () => d.preview.requestRefresh("project-rescan"),
      requireProjectTransitionFrontendLease: (lease) => d.transition.require(lease),
      runWithProjectTransitionFrontendLease: (request, operation) => (
        d.transition.run(request, operation)
      ),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
      startExternalDiskMonitoring: () => d.externalDisk.start(),
    };
  }

  rescan(
    preferredRelativePath: string | null = this.dependencies.documents.activeScannedPath,
    options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
  ) {
    return rescanCurrentProject(this.host(), preferredRelativePath, options);
  }

  reconcile(options: ReconcileWorkspaceDerivedStateOptions) {
    return reconcileWorkspaceDerivedState(this.host(), options);
  }

  rescanCommittedHistory(
    context: CommittedHistoryProjectionContext,
    preferredRelativePath: string | null = this.dependencies.documents.activeScannedPath,
    options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
  ) {
    return rescanCurrentProjectForCommittedHistory(
      this.host(),
      context,
      preferredRelativePath,
      options,
    );
  }
}
