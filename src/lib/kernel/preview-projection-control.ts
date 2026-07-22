import type { PreviewRefreshReason } from "$lib/preview/controlled";
import { readProjectWorkspaceState, type CanvasProjectionPlan } from "$lib/project/io";
import { isZolaTemplatePath } from "$lib/project/files";
import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
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
  PreviewLayerDropExecutionReceipt,
  PreviewProjectionDiagnostic,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionReceipt,
  PreviewTeraMoveDropExecutionReceipt,
  CanvasPatch,
  ProjectWorkspaceMutationReceipt,
} from "$lib/types";

export type PreviewStructuralExecutionReceipt =
  | PreviewLayerDropExecutionReceipt
  | PreviewHtmlInsertDropExecutionReceipt
  | PreviewHtmlAttributesExecutionReceipt
  | PreviewHtmlTextExecutionReceipt
  | PreviewHtmlTagExecutionReceipt
  | PreviewHtmlDuplicateExecutionReceipt
  | PreviewHtmlDeleteExecutionReceipt
  | PreviewTeraInsertDropExecutionReceipt
  | PreviewTeraMoveDropExecutionReceipt
  | PreviewTeraDeleteExecutionReceipt;

export type PreviewStructuralPatch = NonNullable<PreviewStructuralExecutionReceipt["patch"]>;

export type PreviewStructuralCanonicalProjectionHost = PreviewStructuralSessionHost & {
  scannedProject: { isZola: boolean } | null;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  applyCanvasPatchToPreview: (patch: CanvasPatch) => Promise<unknown>;
  rollbackCanvasPatchInPreview: (patch: CanvasPatch) => Promise<unknown>;
  refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
  requestPreviewRefresh: (
    reason: Extract<PreviewRefreshReason, "html-structural" | "tera-structural">,
  ) => Promise<boolean>;
};

export function previewStructuralBlockingDiagnostic(
  receipt: Pick<PreviewStructuralExecutionReceipt, "diagnostics">,
): PreviewProjectionDiagnostic | null {
  return receipt.diagnostics.find((diagnostic) => diagnostic.blocking) ?? null;
}

export function requireCommittedPreviewStructuralPatch<TPatch extends PreviewStructuralPatch>(
  receipt: {
    status: "committed" | "blocked";
    message: string;
    patch: TPatch | null;
    diagnostics: PreviewProjectionDiagnostic[];
  },
  fallbackMessage: string,
): TPatch {
  if (receipt.status === "committed" && receipt.patch) return receipt.patch;
  const blocking = receipt.diagnostics.find((diagnostic) => diagnostic.blocking);
  throw new Error(blocking?.message ?? receipt.message ?? fallbackMessage);
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
) {
  requireCurrentPreviewStructuralSession(host, lease);
  requirePreviewStructuralReceiptIdentity(receipt.intent, lease);
  const mutation = requireWorkspaceMutation(receipt.workspaceMutation);

  await projectLocalState();
  requireCurrentPreviewStructuralSession(host, lease);
  const canvasPatchApplied = receipt.canvasPatch
    ? await applyCommittedCanvasPatch(host, lease, receipt.canvasPatch, mutation)
    : false;
  requireCurrentPreviewStructuralSession(host, lease);
  await host.refreshSourceGraph({ strict: true });
  requireCurrentPreviewStructuralSession(host, lease);
  try {
    await projectCanonicalPreviewWorkspaceRevision(
      host,
      lease,
      receipt,
      patch,
      mutation,
      canvasPatchApplied ? receipt.canvasPatch : null,
    );
  } catch (projectionError) {
    if (canvasPatchApplied && receipt.canvasPatch) {
      try {
        await host.rollbackCanvasPatchInPreview(receipt.canvasPatch);
      } catch (rollbackError) {
        const projectionMessage = projectionError instanceof Error
          ? projectionError.message
          : String(projectionError);
        const rollbackMessage = rollbackError instanceof Error
          ? rollbackError.message
          : String(rollbackError);
        throw new Error(
          `Proiecția canonică a eșuat (${projectionMessage}), iar rollback-ul CanvasPatch a fost refuzat (${rollbackMessage}).`,
        );
      }
    }
    throw projectionError;
  }
}

async function applyCommittedCanvasPatch(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  patch: CanvasPatch,
  mutation: ProjectWorkspaceMutationReceipt,
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
    throw new Error("CanvasPatch nu corespunde mutației comise în sesiunea proiectului.");
  }
  const snapshot = await readProjectWorkspaceState();
  requireCurrentPreviewStructuralSession(host, lease);
  if (
    !snapshot
    || snapshot.projectRoot !== lease.projectRoot
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
  if (!mutation?.changed || mutation.revisionAfter <= mutation.revisionBefore) {
    throw new Error(
      "Mutația structurală comisă nu conține o tranziție validă a sesiunii proiectului.",
    );
  }
  return mutation;
}

function structuralRefreshReason(
  receipt: Pick<PreviewStructuralExecutionReceipt, "intent">,
): "html-structural" | "tera-structural" {
  return receipt.intent.kind.startsWith("tera_")
    ? "tera-structural"
    : "html-structural";
}

export async function projectCanonicalPreviewWorkspaceRevision(
  host: PreviewStructuralCanonicalProjectionHost,
  lease: PreviewStructuralSessionLease,
  receipt: Pick<PreviewStructuralExecutionReceipt, "intent" | "touchedFiles">,
  patch: { file?: string } | null,
  mutation: ProjectWorkspaceMutationReceipt,
  appliedCanvasPatch: CanvasPatch | null = null,
) {
  requireCurrentPreviewStructuralSession(host, lease);
  requirePreviewStructuralReceiptIdentity(receipt.intent, lease);
  const touchedFiles = previewStructuralTouchedFiles(receipt, patch);
  if (touchedFiles.length === 0) {
    throw new Error(
      "Mutația structurală a fost comisă fără fișiere pentru previzualizare.",
    );
  }
  if (!touchedFiles.some((file) => (
    isZolaTemplatePath(file) && file.toLowerCase().endsWith(".html")
  ))) {
    throw new Error(
      "Mutația structurală nu a identificat niciun template Zola proiectabil.",
    );
  }

  await projectLatestProjectWorkspacePreview(host, {
    reason: structuralRefreshReason(receipt),
    minimumWorkspaceRevision: mutation.revisionAfter,
    requestedPaths: touchedFiles,
    expectedWorkspaceTransactionId: appliedCanvasPatch?.workspaceTransactionId,
    expectedWorkspaceRevision: appliedCanvasPatch?.workspaceRevision,
  });
  requireCurrentPreviewStructuralSession(host, lease);
}

export function previewStructuralTouchedFiles(
  receipt: Pick<PreviewStructuralExecutionReceipt, "touchedFiles">,
  patch: { file?: string } | null,
) {
  const files = receipt.touchedFiles.length > 0
    ? receipt.touchedFiles
    : patch?.file
      ? [patch.file]
      : [];
  return [...new Set(files.map((file) => file.trim()).filter(Boolean))].sort();
}
