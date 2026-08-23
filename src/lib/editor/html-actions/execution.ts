import {
  blockedAction,
  cancelledAction,
  failedAction,
  noopAction,
  committedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  previewStructuralBlockingDiagnostic,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  isPreviewStructuralCancellation,
  previewStructuralCommandIdentity,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import {
  executePreviewHtmlAttributesIntent,
  executePreviewHtmlTextIntent,
  executePreviewSelectionBatchIntent,
} from "$lib/preview/structural-io";
import { readProjectWorkspaceState } from "$lib/project/io/workspace";
import { scannedCacheKey } from "$lib/project/files";
import type { NativeIconMutationIntent } from "$lib/blocks/contracts";
import type {
  NativeBlockOptionIntent,
  PreviewSelectionBatchAction,
  ProjectGeneratedIdentityIntent,
  ProjectHtmlAttributePatch,
  ProjectHtmlTextPatch,
  ProjectZolaImageIntent,
} from "$lib/preview/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import type { HtmlActionTarget } from "$lib/editor/html-actions/target";
import { attributeMutationsFromRecord } from "$lib/editor/html-actions/attribute-values";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export function blockedReceiptOutcome(
  receipt: PreviewStructuralExecutionReceipt,
  fallback: string,
): EditorActionOutcome | null {
  if (receipt.status === "committed") return null;
  const diagnostic = previewStructuralBlockingDiagnostic(receipt);
  const reason = (diagnostic ? errorMessage(diagnostic.diagnostic) : "")
    || errorMessage(receipt.messageDiagnostic)
    || fallback;
  return blockedAction(reason);
}

export function actionErrorOutcome(error: unknown): EditorActionOutcome {
  const reason = errorMessage(error);
  return isPreviewStructuralCancellation(error)
    ? cancelledAction(reason)
    : failedAction(reason);
}

export function missingKernelIdentityMessage(action: string) {
  return t("html-actions-identity-missing", { action });
}

export function hasMultiElementSelection(host: HtmlActionsHost) {
  return (host.editorSelection.selectionSnapshot?.members.length ?? 0) > 1;
}

export async function runHtmlStructuralAction(
  host: HtmlActionsHost,
  operation: (lease: PreviewStructuralSessionLease) => Promise<EditorActionOutcome>,
  cancellationReason: string,
): Promise<EditorActionOutcome> {
  const result = await host.structural.run(operation);
  return result ?? cancelledAction(cancellationReason);
}

export function commitHtmlStructuralPatch(
  host: HtmlActionsHost,
  lease: PreviewStructuralSessionLease,
  receipt: PreviewStructuralExecutionReceipt,
  patch: NonNullable<PreviewStructuralExecutionReceipt["patch"]>,
  projectLocalState: () => Promise<void> | void,
) {
  return host.structural.projectCommitted(lease, receipt, patch, projectLocalState);
}

export function commitSelectionBatchReceipt(
  host: HtmlActionsHost,
  lease: PreviewStructuralSessionLease,
  receipt: import("$lib/preview/contracts").PreviewSelectionBatchExecutionReceipt,
) {
  return host.structural.projectCommittedBatch(lease, receipt);
}

export function cacheCommittedHtmlPatch(
  host: HtmlActionsHost,
  patch: { file: string; contents: string },
) {
  host.source.sourceCache = {
    ...host.source.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.context().activeScannedPath === patch.file) {
    host.source.source = patch.contents;
  }
}

export async function executeSelectionBatch(
  host: HtmlActionsHost,
  action: PreviewSelectionBatchAction,
): Promise<EditorActionOutcome> {
  return await runHtmlStructuralAction(host, async (lease) => {
    if (!lease.selection || lease.selection.members.length < 2) {
      return blockedAction(t("html-actions-batch-minimum"));
    }
    const receipt = await executePreviewSelectionBatchIntent({
      schemaVersion: 1,
      action,
    }, previewStructuralCommandIdentity(lease, true));
    if (receipt.status !== "committed") {
      return blockedAction(receipt.diagnostics[0] || t("html-actions-batch-blocked"));
    }
    await commitSelectionBatchReceipt(host, lease, receipt);
    return committedAction();
  }, t("html-actions-batch-cancelled"));
}

export type ExecuteSelectedHtmlAttributesRequest = Readonly<{
  target: HtmlActionTarget;
  attributes: Record<string, string | null>;
  project: (
    target: HtmlActionTarget,
    patch: ProjectHtmlAttributePatch,
  ) => Promise<void> | void;
  zolaImage?: ProjectZolaImageIntent | null;
  nativeBlockOption?: NativeBlockOptionIntent | null;
  nativeIcon?: NativeIconMutationIntent | null;
  generatedIdentity?: ProjectGeneratedIdentityIntent | null;
}>;

export async function executeSelectedHtmlAttributes(
  host: HtmlActionsHost,
  request: ExecuteSelectedHtmlAttributesRequest,
): Promise<EditorActionOutcome> {
  const {
    target,
    attributes,
    project,
    zolaImage = null,
    nativeBlockOption = null,
    nativeIcon = null,
    generatedIdentity = null,
  } = request;
  return await runHtmlStructuralAction(host, async (lease) => {
    if (!target.sourceId) {
      throw new Error(missingKernelIdentityMessage(t("html-actions-attributes-noun")));
    }

    const receipt = await executePreviewHtmlAttributesIntent({
      intent: {
        messageType: "preview-html-attributes",
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      attributeIntent: {
        targetSourceId: target.sourceId,
        targetTag: target.tag,
        attributes: attributeMutationsFromRecord(attributes),
        ...(zolaImage ? { zolaImage } : {}),
        ...(nativeBlockOption ? { nativeBlockOption } : {}),
        ...(nativeIcon ? { nativeIcon } : {}),
        ...(generatedIdentity ? { generatedIdentity } : {}),
      },
    }, previewStructuralCommandIdentity(lease, true));

    const blocked = blockedReceiptOutcome(receipt, t("html-actions-attributes-engine-blocked"));
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-attributes-engine-blocked"),
    );
    if (
      receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(target, patch);
      return noopAction(t("html-actions-attributes-already-match"));
    }
    await commitHtmlStructuralPatch(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      await project(target, patch);
    });
    return committedAction();
  }, t("html-actions-attributes-session-cancelled"));
}

export type ExecuteSelectedHtmlTextOptions = Readonly<{
  deferCanonicalProjection?: boolean;
  editSessionId?: string | null;
}>;

export async function executeSelectedHtmlText(
  host: HtmlActionsHost,
  target: HtmlActionTarget,
  text: string,
  project: (patch: ProjectHtmlTextPatch, target: HtmlActionTarget) => Promise<void> | void,
  options: ExecuteSelectedHtmlTextOptions = {},
): Promise<EditorActionOutcome> {
  return await runHtmlStructuralAction(host, async (lease) => {
    const groupedEditSession = Boolean(options.editSessionId);
    if (!target.sourceId) {
      throw new Error(missingKernelIdentityMessage(t("html-actions-text-noun")));
    }
    const receipt = await executePreviewHtmlTextIntent({
      intent: {
        messageType: "preview-html-text",
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      textIntent: {
        targetSourceId: target.sourceId,
        targetTag: target.tag,
        text,
      },
      deferCanonicalProjection: options.deferCanonicalProjection === true,
      editSessionId: options.editSessionId ?? null,
    }, previewStructuralCommandIdentity(lease, !groupedEditSession));

    const blocked = blockedReceiptOutcome(receipt, t("html-actions-text-engine-blocked"));
    if (blocked) return blocked;
    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-text-engine-blocked"),
    );
    if (
      !options.deferCanonicalProjection
      && receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(patch, target);
      return noopAction(t("html-actions-text-already-matches"));
    }
    if (options.deferCanonicalProjection) {
      if (!host.structural.leaseMatches(lease)) {
        return cancelledAction(t("html-actions-text-draft-session-closed"));
      }
      const mutation = receipt.workspaceMutation;
      if (mutation && !mutation.changed && mutation.revisionAfter === mutation.revisionBefore) {
        await project(patch, target);
        return noopAction(t("html-actions-text-draft-already-matches"));
      }
      if (!mutation?.changed || mutation.revisionAfter <= mutation.revisionBefore || !mutation.transactionId?.trim()) {
        throw new Error(t("html-actions-text-draft-transition-invalid"));
      }
      try {
        const snapshot = await readProjectWorkspaceState();
        if (
          !snapshot
          || snapshot.projectRoot !== lease.projectRoot
          || snapshot.runtimeSessionId !== lease.sessionId
          || snapshot.revision !== mutation.revisionAfter
        ) {
          throw new Error(t("html-actions-text-draft-revision-unconfirmed"));
        }
        await host.structural.settleMutation({
          projectRoot: lease.projectRoot,
          runtimeSessionId: lease.sessionId,
          mutation,
          workspace: snapshot,
        }, {
          preferredRelativePath: patch.file,
          refreshSourceGraph: false,
          refreshScss: false,
          projectPreview: false,
          warningLabel: t("html-actions-text-draft-operation"),
        });
      } catch (error) {
        host.commands.setStatus(
          t("html-actions-text-resync", { message: errorMessage(error) }),
          "unsaved",
        );
      }
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
      return committedAction();
    }
    await commitHtmlStructuralPatch(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
    });
    return committedAction();
  }, t("html-actions-text-session-cancelled"));
}
