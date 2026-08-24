import {
  blockedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import type { HtmlAuthoringState } from "$lib/editor/html-authoring-state.svelte";
import type { EditorInteractionRuntime } from "$lib/editor/interaction-runtime.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import { markDiskMutation } from "$lib/session/disk-state";
import type { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { HistoryOperationState } from "$lib/versioning/history-operation-state.svelte";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  saveActiveFile as saveActiveDocument,
  savePendingHtmlChanges as savePendingHtmlChangesFromController,
  saveSessionDrafts as saveSessionDraftsFromController,
  saveSourceFile as saveSourceFileFromController,
  type SaveControllerHost,
} from "$lib/state/save-controller";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type ProjectSaveServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  disk: AcceptedDiskState;
  externalDisk: ExternalDiskState;
  transition: ProjectTransitionLeaseState;
  history: HistoryOperationState;
  ai: AiCoordinationState;
  html: HtmlAuthoringState;
  editor: EditorInteractionRuntime;
  status: GlobalStatusState;
  commands: Readonly<{
    applyTagChange: () => Promise<EditorActionOutcome>;
    applyClasses: () => Promise<EditorActionOutcome>;
    applyImageSource: (src?: string) => Promise<EditorActionOutcome>;
    reconcileWorkspaceDerivedState: SaveControllerHost["reconcileWorkspaceDerivedState"];
    projectLatestPreview: SaveControllerHost["projectLatestPreview"];
    markPreviewSavedToDisk: NonNullable<SaveControllerHost["markPreviewSavedToDisk"]>;
    scheduleZolaValidation: NonNullable<SaveControllerHost["scheduleZolaValidation"]>;
  }>;
}>;

/** Owns Save serialization, guards and the Rust ProjectWorkspace Save adapter. */
export class ProjectSaveService {
  private operation: Promise<boolean> | null = null;
  private readonly dependencies: ProjectSaveServiceDependencies;
  private readonly controller: SaveControllerHost;

  constructor(dependencies: ProjectSaveServiceDependencies) {
    this.dependencies = dependencies;
    const owner = this;
    this.controller = {
      context: () => ({
        projectRoot: dependencies.project.root,
        runtimeSessionId: dependencies.project.runtimeSessionId,
        editorMutationEpoch: dependencies.project.editorMutationEpoch,
        workspace: dependencies.project.workspace,
        diskState: dependencies.disk.snapshot,
        activeScannedPath: dependencies.documents.activeScannedPath,
      }),
      incrementSaveRequest: () => { dependencies.project.saveRequest += 1; },
      acceptWorkspace: (workspace) => { dependencies.project.workspace = workspace; },
      markDiskSaved: (activeScannedPath) => {
        dependencies.disk.snapshot = markDiskMutation(
          dependencies.disk.snapshot,
          "save",
          activeScannedPath,
        );
      },
      bumpRefreshTokens: () => {
        dependencies.project.refreshToken += 1;
        dependencies.project.jsRefreshToken += 1;
      },
      setGlobalStatus: (text, kind) => dependencies.status.set(text, kind),
      resolveGlobalStatus: (key) => dependencies.status.resolve(key),
      html: {
        get inspectorPending() { return dependencies.html.inspectorPending; },
        get pending() { return dependencies.html.htmlPending; },
        get pendingTag() { return dependencies.html.pendingTag; },
        setInspectorPending: (area, pending) => dependencies.html.setInspectorPending(area, pending),
        applyTagChange: dependencies.commands.applyTagChange,
        applyClasses: dependencies.commands.applyClasses,
        draft: dependencies.editor.htmlDraft,
        applyImageSource: dependencies.commands.applyImageSource,
      },
      reconcileWorkspaceDerivedState: dependencies.commands.reconcileWorkspaceDerivedState,
      projectLatestPreview: dependencies.commands.projectLatestPreview,
      markPreviewSavedToDisk: dependencies.commands.markPreviewSavedToDisk,
      scheduleZolaValidation: dependencies.commands.scheduleZolaValidation,
      acceptProjectWorkspaceSaveBaseline: (manifest, generation) => {
        owner.dependencies.externalDisk.acceptSaveBaseline(manifest, generation);
      },
    };
  }

  async saveSessionDrafts() {
    if (this.blockExternalProjectionConflict()) return false;
    if (this.blockHistoryLease()) return false;
    return await saveSessionDraftsFromController(this.controller);
  }

  async saveSourceFile() {
    if (this.blockAiLease()) return false;
    if (this.blockExternalProjectionConflict()) return false;
    if (this.blockHistoryLease()) return false;
    return await saveSourceFileFromController(this.controller);
  }

  async savePendingHtmlChanges(): Promise<EditorActionOutcome> {
    if (this.blockAiLease()) return blockedAction(t("workbench-html-save-ai-blocked"));
    if (this.blockExternalProjectionConflict()) {
      return blockedAction(t("workbench-html-save-external-blocked"));
    }
    if (this.blockHistoryLease()) {
      return blockedAction(t("workbench-html-save-history-blocked"));
    }
    return await savePendingHtmlChangesFromController(this.controller);
  }

  async saveActiveFile() {
    if (this.blockAiLease()) return false;
    if (this.blockExternalProjectionConflict()) return false;
    if (this.blockHistoryLease()) return false;
    if (this.dependencies.transition.isActive) {
      this.dependencies.status.set(t("workbench-save-transition-blocked"), "error");
      return false;
    }
    if (this.operation) return await this.operation;
    const operation = this.saveAtExternalDiskBoundary();
    this.operation = operation;
    try {
      return await operation;
    } finally {
      if (this.operation === operation) this.operation = null;
    }
  }

  async drain() {
    if (this.operation) await this.operation;
  }

  private async saveAtExternalDiskBoundary() {
    try {
      await this.dependencies.externalDisk.suspendAndDrain();
      if (this.blockExternalProjectionConflict()) return false;
      const disk = this.dependencies.externalDisk.snapshot;
      if (disk.checking || disk.reconciling || disk.changed || disk.blockedByDirtySession) {
        this.dependencies.status.set(t("workbench-save-external-state-blocked"), "error");
        return false;
      }
      return await saveActiveDocument(this.controller);
    } catch (error) {
      this.dependencies.status.set(
        t("workbench-save-disk-boundary-failed", { message: errorMessage(error) }),
        "error",
      );
      return false;
    } finally {
      this.dependencies.externalDisk.resumeAfterSave();
    }
  }

  private blockExternalProjectionConflict() {
    if (!this.dependencies.externalDisk.snapshot.workspaceProjectionRecoveryRequired) return false;
    this.dependencies.status.set(t("workbench-save-projection-recovery-blocked"), "error");
    return true;
  }

  private blockAiLease() {
    if (!this.dependencies.ai.frontendLockActive) return false;
    this.dependencies.status.set(t("workbench-save-ai-blocked"), "error");
    return true;
  }

  private blockHistoryLease() {
    if (!this.dependencies.history.quiesceActive && !this.dependencies.history.leaseActive) {
      return false;
    }
    this.dependencies.status.set(t("workbench-save-history-blocked"), "error");
    return true;
  }
}
