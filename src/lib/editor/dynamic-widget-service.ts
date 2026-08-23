import {
  blockedAction,
  committedAction,
  failedAction,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import {
  deleteDynamicWidget,
  updateDynamicWidget,
} from "$lib/editor/dynamic-widget-io";
import type {
  WorkspaceMutationAuthorityReceipt,
  WorkspaceMutationSettlement,
  WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type {
  DynamicWidgetProperties,
  DynamicWidgetSnapshot,
} from "$lib/content-models/contracts";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type DynamicWidgetServiceDependencies = Readonly<{
  project: ProjectSessionState;
  selection: SelectionWorkspaceState;
  status: GlobalStatusState;
  settleMutation: (
    receipt: WorkspaceMutationAuthorityReceipt,
    options?: WorkspaceMutationSettlementOptions,
  ) => Promise<WorkspaceMutationSettlement>;
}>;

/** Owns Inspector mutations for one Rust DynamicWidget source instance. */
export class DynamicWidgetService {
  private readonly dependencies: DynamicWidgetServiceDependencies;

  constructor(dependencies: DynamicWidgetServiceDependencies) {
    this.dependencies = dependencies;
  }

  async update(
    snapshot: DynamicWidgetSnapshot,
    properties: DynamicWidgetProperties,
  ): Promise<EditorActionOutcome> {
    if (!this.snapshotIsCurrent(snapshot)) {
      return blockedAction(t("inspector-dynamic-selection-stale"));
    }
    try {
      const receipt = await updateDynamicWidget({
        request: {
          identity: {
            expectedProjectRoot: snapshot.projectRoot,
            expectedSessionId: snapshot.runtimeSessionId,
          },
          expectedWorkspaceRevision: snapshot.workspaceRevision,
          expectedModelRevision: snapshot.modelRevision,
          previewRevision: snapshot.previewRevision,
          sourceInstanceId: snapshot.sourceInstance.id,
        },
        expectedSourceRevision: snapshot.sourceInstance.sourceRevision,
        properties,
      });
      if (!this.receiptSessionIsCurrent(receipt)) {
        return blockedAction(t("inspector-dynamic-session-changed-update"));
      }
      const settlement = await this.dependencies.settleMutation(receipt, {
        preferredRelativePath: snapshot.sourceInstance.file,
        warningLabel: t("inspector-dynamic-update-operation"),
      });
      this.dependencies.status.set(
        settlement.warnings.length > 0
          ? t("inspector-dynamic-updated-resync-status")
          : t("inspector-dynamic-updated-status"),
        "unsaved",
      );
      return settlement.authority === "committed"
        ? committedAction()
        : noopAction(t("inspector-dynamic-no-changes"));
    } catch (error) {
      return failedAction(errorMessage(error));
    }
  }

  async delete(snapshot: DynamicWidgetSnapshot): Promise<EditorActionOutcome> {
    if (!this.snapshotIsCurrent(snapshot)) {
      return blockedAction(t("inspector-dynamic-selection-stale"));
    }
    try {
      const receipt = await deleteDynamicWidget({
        request: {
          identity: {
            expectedProjectRoot: snapshot.projectRoot,
            expectedSessionId: snapshot.runtimeSessionId,
          },
          expectedWorkspaceRevision: snapshot.workspaceRevision,
          expectedModelRevision: snapshot.modelRevision,
          previewRevision: snapshot.previewRevision,
          sourceInstanceId: snapshot.sourceInstance.id,
        },
        expectedSourceRevision: snapshot.sourceInstance.sourceRevision,
      });
      if (!this.receiptSessionIsCurrent(receipt)) {
        return blockedAction(t("inspector-dynamic-session-changed-delete"));
      }
      const settlement = await this.dependencies.settleMutation(receipt, {
        preferredRelativePath: snapshot.sourceInstance.file,
        warningLabel: t("inspector-dynamic-delete-operation"),
      });
      this.dependencies.status.set(t("inspector-dynamic-deleted-status"), "unsaved");
      return settlement.authority === "committed"
        ? committedAction()
        : noopAction(t("inspector-dynamic-already-deleted"));
    } catch (error) {
      return failedAction(errorMessage(error));
    }
  }

  private snapshotIsCurrent(snapshot: DynamicWidgetSnapshot) {
    const workspace = this.dependencies.project.workspace;
    const context = this.dependencies.selection.dynamicWidgetContext;
    return Boolean(
      workspace
      && context
      && workspace.projectRoot === snapshot.projectRoot
      && workspace.runtimeSessionId === snapshot.runtimeSessionId
      && workspace.revision === snapshot.workspaceRevision
      && context.sourceInstanceId === snapshot.sourceInstance.id
      && context.modelRevision === snapshot.modelRevision
      && context.previewRevision === snapshot.previewRevision,
    );
  }

  private receiptSessionIsCurrent(receipt: { projectRoot: string; runtimeSessionId: string }) {
    return this.dependencies.project.root === receipt.projectRoot
      && this.dependencies.project.runtimeSessionId === receipt.runtimeSessionId;
  }
}
