import { readProjectWorkspaceState } from "$lib/project/io";
import {
  settleProjectWorkspaceMutation,
  type WorkspaceMutationSettlement,
  type WorkspaceMutationSettlementHost,
} from "$lib/session/workspace-mutation-coordinator";
import {
  requireCurrentPreviewStructuralSession,
  requirePreviewStructuralReceiptIdentity,
  type PreviewStructuralSessionHost,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import type {
  PreviewHtmlAttributesExecutionReceipt,
  PreviewHtmlDeleteExecutionReceipt,
  PreviewHtmlDuplicateExecutionReceipt,
  PreviewHtmlInsertDropExecutionReceipt,
  PreviewHtmlTagExecutionReceipt,
  PreviewHtmlTextExecutionReceipt,
  PreviewProjectionDiagnostic,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionReceipt,
  EditorMoveExecutionReceipt,
  CanvasPatch,
  LocalizedDiagnostic,
  ProjectWorkspaceMutationReceipt,
} from "$lib/types";
import { PROJECT_WORKSPACE_SCHEMA_VERSION } from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type PreviewStructuralExecutionReceipt =
  | PreviewHtmlInsertDropExecutionReceipt
  | PreviewHtmlAttributesExecutionReceipt
  | PreviewHtmlTextExecutionReceipt
  | PreviewHtmlTagExecutionReceipt
  | PreviewHtmlDuplicateExecutionReceipt
  | PreviewHtmlDeleteExecutionReceipt
  | PreviewTeraInsertDropExecutionReceipt
  | PreviewTeraDeleteExecutionReceipt;

export type PreviewStructuralPatch = NonNullable<PreviewStructuralExecutionReceipt["patch"]>;

export type PreviewStructuralCanonicalProjectionHost =
  PreviewStructuralSessionHost & WorkspaceMutationSettlementHost & {
  applyCanvasPatchToPreview: (patch: CanvasPatch) => Promise<unknown>;
  rollbackCanvasPatchInPreview: (patch: CanvasPatch) => Promise<unknown>;
};

export function previewStructuralBlockingDiagnostic(
  receipt: Pick<PreviewStructuralExecutionReceipt, "diagnostics">,
): PreviewProjectionDiagnostic | null {
  return receipt.diagnostics.find((diagnostic) => diagnostic.blocking) ?? null;
}

export function requireCommittedPreviewStructuralPatch<TPatch extends PreviewStructuralPatch>(
  receipt: {
    status: "committed" | "blocked";
    messageDiagnostic: LocalizedDiagnostic;
    patch: TPatch | null;
    diagnostics: PreviewProjectionDiagnostic[];
  },
  fallbackMessage: string,
): TPatch {
  if (receipt.status === "committed" && receipt.patch) return receipt.patch;
  const blocking = receipt.diagnostics.find((diagnostic) => diagnostic.blocking);
  throw new Error(
    (blocking ? errorMessage(blocking.diagnostic) : "")
      || errorMessage(receipt.messageDiagnostic)
      || fallbackMessage,
  );
}

/**
 * Projects one already-committed Rust workspace mutation into the immutable
 * Preview generation carrying exactly the mutation's `revisionAfter`.
 * ProjectWorkspace remains authoritative if the derived renderer is
 * temporarily unavailable; no disk acknowledgement or recovery promotion is
 * involved in this path.
 */
export async function projectCommittedPreviewStructuralMutation(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  receipt: Pick<
    PreviewStructuralExecutionReceipt,
    "intent" | "touchedFiles" | "workspaceMutation" | "canvasPatch"
  >,
  patch: { file?: string } | null,
  projectLocalState: () => Promise<void> | void,
): Promise<WorkspaceMutationSettlement> {
  return projectCommittedStructuralMutation(
    host,
    lease,
    receipt.intent,
    receipt,
    patch,
    projectLocalState,
    structuralRefreshReason(receipt),
  );
}

export async function projectCommittedEditorMoveMutation(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  receipt: Pick<
    EditorMoveExecutionReceipt,
    | "operation"
    | "projectRoot"
    | "runtimeSessionId"
    | "touchedFiles"
    | "workspaceMutation"
    | "canvasPatch"
  >,
  projectLocalState: () => Promise<void> | void = () => {},
): Promise<WorkspaceMutationSettlement> {
  return projectCommittedStructuralMutation(
    host,
    lease,
    receipt,
    receipt,
    null,
    projectLocalState,
    receipt.canvasPatch ? "html-structural" : "tera-structural",
  );
}

async function projectCommittedStructuralMutation(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  receiptIdentity: { projectRoot: string | null; runtimeSessionId: string | null },
  receipt: {
    touchedFiles: string[];
    workspaceMutation: ProjectWorkspaceMutationReceipt | null;
    canvasPatch: CanvasPatch | null;
  },
  patch: { file?: string } | null,
  projectLocalState: () => Promise<void> | void,
  previewReason: "html-structural" | "tera-structural",
): Promise<WorkspaceMutationSettlement> {
  requireCurrentPreviewStructuralSession(host, lease);
  requirePreviewStructuralReceiptIdentity(receiptIdentity, lease);
  const mutation = requireWorkspaceMutation(receipt.workspaceMutation);
  let snapshot: NonNullable<Awaited<ReturnType<typeof readProjectWorkspaceState>>>;
  try {
    const candidate = await readProjectWorkspaceState();
    requireCurrentPreviewStructuralSession(host, lease);
    if (
      !candidate
      || candidate.projectRoot !== lease.projectRoot
      || candidate.runtimeSessionId !== lease.sessionId
    ) {
      throw new Error(
        t("structural-projection-workspace-snapshot-missing"),
      );
    }
    if (candidate.revision > mutation.revisionAfter) {
      host.projectWorkspaceSnapshot = candidate;
      return supersededCommittedStructuralSettlement(
        mutation,
        candidate.revision,
      );
    }
    if (candidate.revision !== mutation.revisionAfter) {
      throw new Error(
        t("structural-projection-exact-snapshot-missing"),
      );
    }
    snapshot = candidate;
  } catch (error) {
    requireCurrentPreviewStructuralSession(host, lease);
    const warnings = [
      t("structural-projection-workspace-resync", {
        message: error instanceof Error ? error.message : String(error),
      }),
    ];
    try {
      await projectLocalState();
    } catch (localError) {
      warnings.push(
        t("structural-projection-local-resync", {
          message: localError instanceof Error ? localError.message : String(localError),
        }),
      );
    }
    requireCurrentPreviewStructuralSession(host, lease);
    const settlement = degradedCommittedStructuralSettlement(mutation, warnings);
    publishStructuralProjectionWarning(host, settlement);
    return settlement;
  }
  host.projectWorkspaceSnapshot = snapshot;

  const localWarnings: string[] = [];
  try {
    await projectLocalState();
  } catch (error) {
    localWarnings.push(
      t("structural-projection-local-resync", {
        message: error instanceof Error ? error.message : String(error),
      }),
    );
  }
  requireCurrentPreviewStructuralSession(host, lease);

  let canvasPatchApplied = false;
  if (receipt.canvasPatch) {
    try {
      canvasPatchApplied = await applyCommittedCanvasPatch(
        host,
        lease,
        receipt.canvasPatch,
        mutation,
        snapshot,
      );
    } catch (error) {
      localWarnings.push(
        t("structural-projection-canvas-patch-skipped", {
          message: error instanceof Error ? error.message : String(error),
        }),
      );
    }
  }
  requireCurrentPreviewStructuralSession(host, lease);

  const settlement = await settleProjectWorkspaceMutation(host, {
    projectRoot: lease.projectRoot,
    runtimeSessionId: lease.sessionId,
    mutation,
    workspace: snapshot,
  }, {
    preferredRelativePath: patch?.file ?? receipt.touchedFiles[0] ?? null,
    refreshSourceGraph: true,
    refreshScss: false,
    previewReason,
    warningLabel: t("structural-projection-operation"),
  });
  settlement.warnings.push(...localWarnings);

  if (
    settlement.projections.preview === "degraded"
    && canvasPatchApplied
    && receipt.canvasPatch
  ) {
    try {
      await host.rollbackCanvasPatchInPreview(receipt.canvasPatch);
    } catch (rollbackError) {
      settlement.warnings.push(
        t("structural-projection-canvas-rollback-refused", {
          message: rollbackError instanceof Error ? rollbackError.message : String(rollbackError),
        }),
      );
    }
  }
  publishStructuralProjectionWarning(host, settlement);
  return settlement;
}

function degradedCommittedStructuralSettlement(
  mutation: ProjectWorkspaceMutationReceipt,
  warnings: string[],
): WorkspaceMutationSettlement {
  return {
    authority: "committed",
    workspaceRevision: mutation.revisionAfter,
    transactionId: mutation.transactionId,
    projections: {
      workspaceRevision: mutation.revisionAfter,
      topology: "degraded",
      sourceGraph: "degraded",
      scss: "degraded",
      preview: "degraded",
      previewOutcome: null,
      warnings,
    },
    warnings: [...new Set(warnings)],
  };
}

function supersededCommittedStructuralSettlement(
  mutation: ProjectWorkspaceMutationReceipt,
  publishedWorkspaceRevision: number,
): WorkspaceMutationSettlement {
  return {
    authority: "committed",
    workspaceRevision: mutation.revisionAfter,
    transactionId: mutation.transactionId,
    projections: {
      workspaceRevision: mutation.revisionAfter,
      topology: "superseded",
      sourceGraph: "superseded",
      scss: "superseded",
      preview: "superseded",
      previewOutcome: {
        status: "superseded",
        workspaceRevision: publishedWorkspaceRevision,
      },
      warnings: [],
    },
    warnings: [],
  };
}

function publishStructuralProjectionWarning(
  host: PreviewStructuralCanonicalProjectionHost,
  settlement: WorkspaceMutationSettlement,
) {
  if (settlement.warnings.length === 0) return;
  host.setGlobalStatus?.(
    t("structural-projection-committed-resync"),
    "unsaved",
  );
}

async function applyCommittedCanvasPatch(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  patch: CanvasPatch,
  mutation: ProjectWorkspaceMutationReceipt,
  snapshot: NonNullable<Awaited<ReturnType<typeof readProjectWorkspaceState>>>,
) {
  const transactionId = mutation.transactionId?.trim() ?? "";
  if (
    patch.schemaVersion !== 1
    || patch.projectRoot !== lease.projectRoot
    || patch.runtimeSessionId !== lease.sessionId
    || patch.baseWorkspaceRevision !== mutation.revisionBefore
    || patch.workspaceRevision !== mutation.revisionAfter
    || patch.workspaceTransactionId !== transactionId
  ) {
    throw new Error(t("structural-projection-canvas-patch-mismatch"));
  }
  if (
    snapshot.projectRoot !== lease.projectRoot
    || snapshot.runtimeSessionId !== lease.sessionId
    || snapshot.revision !== patch.workspaceRevision
    || snapshot.history.nextUndo?.transactionId !== patch.workspaceTransactionId
  ) {
    // A second kernel mutation (for example a component contract) has already
    // superseded this one. Applying an incomplete fast patch would create a
    // false Canvas, so canonical Zola projection is used directly.
    return false;
  }
  try {
    await host.applyCanvasPatchToPreview(patch);
    return true;
  } catch {
    // Repeated render instances or a changed mounted anchor are legitimate
    // reasons to skip acceleration. The authoritative mutation remains in
    // ProjectWorkspace and will be projected canonically below.
    return false;
  }
}

function requireWorkspaceMutation(
  mutation: ProjectWorkspaceMutationReceipt | null,
): ProjectWorkspaceMutationReceipt {
  const transactionId = mutation?.transactionId?.trim() ?? "";
  if (
    !mutation?.changed
    || mutation.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
    || mutation.revisionAfter <= mutation.revisionBefore
    || !transactionId
    || mutation.entry?.transactionId !== transactionId
  ) {
    throw new Error(
      t("structural-projection-transition-invalid"),
    );
  }
  return mutation;
}

function structuralRefreshReason(
  receipt: Pick<PreviewStructuralExecutionReceipt, "intent">,
): "html-structural" | "tera-structural" {
  return receipt.intent.kind?.startsWith("tera_")
    ? "tera-structural"
    : "html-structural";
}
