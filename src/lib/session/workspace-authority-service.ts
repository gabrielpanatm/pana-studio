import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { PreviewSurfaceState } from "$lib/preview/surface-state.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { HistoryOperationState } from "$lib/versioning/history-operation-state.svelte";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  projectLatestProjectWorkspacePreview,
  type ProjectWorkspacePreviewHost,
  type ProjectWorkspacePreviewProjectionOptions,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  settleProjectWorkspaceMutation,
  type WorkspaceDerivedReconciliationOutcome,
  type WorkspaceMutationAuthorityReceipt,
  type WorkspaceMutationSettlementHost,
  type WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import {
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import {
  projectCommittedEditorMoveMutation,
  projectCommittedPreviewSelectionBatchMutation,
  projectCommittedPreviewStructuralMutation,
  type PreviewStructuralCanonicalProjectionHost,
} from "$lib/kernel/preview-projection-control";
import type { PreviewRefreshReason } from "$lib/preview/controlled";
import { t } from "$lib/i18n/runtime.svelte";

export type WorkspaceAuthoritySession = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  source: SourceWorkspaceState;
  analysis: ProjectAnalysisState;
}>;

export type WorkspaceAuthorityPreview = Readonly<{
  surface: PreviewSurfaceState;
  workspace: PreviewWorkspaceState;
}>;

export type WorkspaceAuthorityLocks = Readonly<{
  transition: ProjectTransitionLeaseState;
  history: HistoryOperationState;
  ai: AiCoordinationState;
}>;

export type WorkspaceAuthorityServiceDependencies = Readonly<{
  session: WorkspaceAuthoritySession;
  preview: WorkspaceAuthorityPreview;
  selection: SelectionWorkspaceState;
  locks: WorkspaceAuthorityLocks;
  disk: ExternalDiskState;
  status: GlobalStatusState;
  reconcileDerived: (
    options: Parameters<WorkspaceMutationSettlementHost["reconcileWorkspaceDerivedState"]>[0],
  ) => Promise<WorkspaceDerivedReconciliationOutcome>;
  reprojectTemplate: (minimumWorkspaceRevision: number) => Promise<boolean>;
}>;

/**
 * One Rust-authority boundary for structural commands, settlement and Preview
 * publication. It owns no document data and exposes only command-oriented APIs.
 */
export class WorkspaceAuthorityService {
  private readonly dependencies: WorkspaceAuthorityServiceDependencies;

  constructor(dependencies: WorkspaceAuthorityServiceDependencies) {
    this.dependencies = dependencies;
  }

  previewHost(): ProjectWorkspacePreviewHost {
    const { session, preview, status } = this.dependencies;
    return {
      get sessionProjectRoot() { return session.project.root; },
      get kernelProjectSessionId() { return session.project.runtimeSessionId; },
      get scannedProject() { return session.project.project; },
      get previewWorkspaceRevision() { return preview.workspace.workspaceRevision; },
      set previewWorkspaceRevision(revision) { preview.workspace.workspaceRevision = revision; },
      get pendingCanvasProjection() { return preview.workspace.pendingProjection; },
      set pendingCanvasProjection(plan) { preview.workspace.setPendingProjection(plan); },
      get canvasSurfaceGeneration() { return preview.surface.generation; },
      canProjectWorkspacePreview: () => preview.workspace.canProjectWorkspacePreview(),
      deferWorkspacePreviewProjection: () => preview.workspace.deferSurfaceProjection(),
      get templateWorkbenchActive() { return session.documents.templateActive; },
      reprojectActiveTemplateWorkbench: this.dependencies.reprojectTemplate,
      setGlobalStatus: (text, kind) => status.set(text, kind),
      requestPreviewRefresh: (reason) => preview.workspace.requestRefresh(reason),
      requestWorkspaceProjectionPreviewRefresh: (reason) => (
        preview.workspace.requestWorkspaceProjectionRefresh(reason)
      ),
    };
  }

  settlementHost(): WorkspaceMutationSettlementHost {
    const { session, status } = this.dependencies;
    const preview = this.previewHost();
    return {
      ...preview,
      get projectWorkspaceSnapshot() { return session.project.workspace; },
      set projectWorkspaceSnapshot(snapshot) { session.project.workspace = snapshot; },
      get activeScannedPath() { return session.documents.activeScannedPath; },
      set activeScannedPath(path) { session.documents.activeScannedPath = path; },
      get source() { return session.source.source; },
      set source(source) { session.source.source = source; },
      get sourceCache() { return session.source.sourceCache; },
      set sourceCache(cache) { session.source.sourceCache = cache; },
      setGlobalStatus: (text, kind) => status.set(text, kind),
      reconcileWorkspaceDerivedState: this.dependencies.reconcileDerived,
    };
  }

  structuralHost(): PreviewStructuralCanonicalProjectionHost {
    const { session, preview, selection, locks } = this.dependencies;
    return {
      ...this.settlementHost(),
      get sessionProjectRoot() { return session.project.root; },
      get kernelProjectSessionId() { return session.project.runtimeSessionId; },
      get projectSessionEpoch() { return session.project.epoch; },
      get projectTransitionFrontendLeaseActive() { return locks.transition.isActive; },
      get kernelUndoRedoFrontendLeaseActive() { return locks.history.leaseActive; },
      get aiEditLeaseFrontendLockActive() { return locks.ai.frontendLockActive; },
      editorSelection: selection.session,
      beginPreviewStructuralWriteBoundary: () => this.beginStructuralWrite(),
      endPreviewStructuralWriteBoundary: () => this.endStructuralWrite(),
      applyCanvasPatchToPreview: (patch) => preview.workspace.applyCanvasPatch(patch),
      rollbackCanvasPatchInPreview: (patch) => preview.workspace.rollbackCanvasPatch(patch),
    };
  }

  runStructural<T>(operation: (lease: PreviewStructuralSessionLease) => Promise<T>) {
    return runInPreviewStructuralLane(this.structuralHost(), operation);
  }

  leaseMatches(lease: PreviewStructuralSessionLease) {
    return previewStructuralSessionLeaseMatches(this.structuralHost(), lease);
  }

  requireLease(lease: PreviewStructuralSessionLease) {
    requireCurrentPreviewStructuralSession(this.structuralHost(), lease);
  }

  settle(
    receipt: WorkspaceMutationAuthorityReceipt,
    options: WorkspaceMutationSettlementOptions = {},
  ) {
    return settleProjectWorkspaceMutation(this.settlementHost(), receipt, options);
  }

  projectLatest<TReason extends PreviewRefreshReason>(
    options: ProjectWorkspacePreviewProjectionOptions<TReason>,
  ) {
    return projectLatestProjectWorkspacePreview(
      this.previewHost() as ProjectWorkspacePreviewHost<TReason>,
      options,
    );
  }

  projectCommittedStructural(
    lease: PreviewStructuralSessionLease,
    receipt: Parameters<typeof projectCommittedPreviewStructuralMutation>[2],
    patch: Parameters<typeof projectCommittedPreviewStructuralMutation>[3],
    projectLocalState: Parameters<typeof projectCommittedPreviewStructuralMutation>[4],
  ) {
    return projectCommittedPreviewStructuralMutation(
      this.structuralHost(),
      lease,
      receipt,
      patch,
      projectLocalState,
    );
  }

  projectCommittedSelectionBatch(
    lease: PreviewStructuralSessionLease,
    receipt: Parameters<typeof projectCommittedPreviewSelectionBatchMutation>[2],
  ) {
    return projectCommittedPreviewSelectionBatchMutation(this.structuralHost(), lease, receipt);
  }

  projectCommittedEditorMove(
    context: Parameters<typeof projectCommittedEditorMoveMutation>[1],
    receipt: Parameters<typeof projectCommittedEditorMoveMutation>[2],
  ) {
    return projectCommittedEditorMoveMutation(this.structuralHost(), context, receipt);
  }

  private async beginStructuralWrite() {
    const { preview, disk } = this.dependencies;
    if (preview.workspace.structuralWriteBoundaryActive) {
      throw new Error(t("workbench-structural-boundary-busy"));
    }
    const resumesMonitoring = !disk.suspended;
    try {
      await disk.suspendAndDrain();
      const state = disk.snapshot;
      if (
        state.checking
        || state.reconciling
        || state.changed
        || state.blockedByDirtySession
        || state.workspaceProjectionRecoveryRequired
      ) throw new Error(t("workbench-structural-boundary-disk-dirty"));
      preview.workspace.structuralWriteBoundaryResumesMonitoring = resumesMonitoring;
      preview.workspace.structuralWriteBoundaryActive = true;
    } catch (error) {
      if (resumesMonitoring) disk.resumeAfterSave();
      throw error;
    }
  }

  private endStructuralWrite() {
    const { preview, disk } = this.dependencies;
    if (!preview.workspace.structuralWriteBoundaryActive) return;
    const resumesMonitoring = preview.workspace.structuralWriteBoundaryResumesMonitoring;
    preview.workspace.structuralWriteBoundaryActive = false;
    preview.workspace.structuralWriteBoundaryResumesMonitoring = false;
    if (resumesMonitoring) disk.resumeAfterSave();
  }
}
