import {
  blockedAction,
  committedAction,
  editorActionSucceeded,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import type { EditorHtmlTarget } from "$lib/editor-runtime/commands";
import { previewStructuralCommandIdentity } from "$lib/kernel/preview-structural-lane";
import {
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  executePreviewHtmlDeleteIntent,
  executePreviewHtmlDuplicateIntent,
  executePreviewHtmlInsertDropIntent,
} from "$lib/preview/structural-io";
import type { NativeBlockSlotMutationRequest } from "$lib/blocks/contracts";
import type { ProjectMovePosition } from "$lib/preview/contracts";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import { captureHtmlActionTarget } from "$lib/editor/html-actions/target";
import {
  actionErrorOutcome,
  blockedReceiptOutcome,
  cacheCommittedHtmlPatch,
  commitHtmlStructuralPatch,
  executeSelectionBatch,
  hasMultiElementSelection,
  missingKernelIdentityMessage,
  runHtmlStructuralAction,
} from "$lib/editor/html-actions/execution";
import { t } from "$lib/i18n/runtime.svelte";

export async function moveSelectedHtmlElements(
  host: HtmlActionsHost,
  targetSourceId: string,
  targetTag: string | null,
  position: ProjectMovePosition,
): Promise<EditorActionOutcome> {
  if (position === "inside") {
    return blockedAction(t("html-actions-multi-move-position-blocked"));
  }
  try {
    const result = await executeSelectionBatch(host, {
      kind: "move",
      targetSourceId,
      targetTag,
      position,
    });
    host.html.structureStatus = editorActionSucceeded(result)
      ? t("html-actions-multi-move-confirmed")
      : (result.reason ?? t("html-actions-multi-move-blocked"));
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.structureStatus = result.reason ?? result.status;
    host.commands.setStatus(host.html.structureStatus, "error");
    return result;
  }
}

export async function deleteSelectedHtmlElement(
  host: HtmlActionsHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  if (hasMultiElementSelection(host)) {
    try {
      const result = await executeSelectionBatch(host, { kind: "delete" });
      host.html.structureStatus = editorActionSucceeded(result)
        ? t("html-actions-multi-delete-confirmed")
        : (result.reason ?? t("html-actions-multi-delete-blocked"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.html.structureStatus = result.reason ?? result.status;
      host.commands.setStatus(host.html.structureStatus, "error");
      return result;
    }
  }
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.context().coordinatedSelection);
  try {
    return await runHtmlStructuralAction(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.html.structureStatus = t("html-actions-delete-select");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }
      if (!target.sourceId) {
        const message = missingKernelIdentityMessage(t("html-actions-delete-noun"));
        host.html.structureStatus = message;
        host.commands.setStatus(message, "error");
        return blockedAction(message);
      }
      const receipt = await executePreviewHtmlDeleteIntent({
        intent: {
          messageType: "preview-delete-selected",
          sourceId: target.sourceId,
          sourceTag: target.tag,
        },
        deleteIntent: {
          targetSourceId: target.sourceId,
          targetRenderInstanceId: target.renderInstanceId ?? null,
          targetTag: target.tag,
        },
      }, previewStructuralCommandIdentity(lease, true));
      const blocked = blockedReceiptOutcome(receipt, t("html-actions-delete-engine-blocked"));
      if (blocked) return blocked;
      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-actions-delete-engine-blocked"),
      );
      await commitHtmlStructuralPatch(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.html.structureStatus = t("html-actions-deleted", { tag: target.tag });
      });
      return committedAction();
    }, t("html-actions-delete-session-cancelled"));
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.structureStatus = t("html-actions-delete-failed", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-delete-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function duplicateSelectedHtmlElement(
  host: HtmlActionsHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  if (hasMultiElementSelection(host)) {
    try {
      const result = await executeSelectionBatch(host, { kind: "duplicate" });
      host.html.structureStatus = editorActionSucceeded(result)
        ? t("html-actions-multi-duplicate-confirmed")
        : (result.reason ?? t("html-actions-multi-duplicate-blocked"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.html.structureStatus = result.reason ?? result.status;
      host.commands.setStatus(host.html.structureStatus, "error");
      return result;
    }
  }
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.context().coordinatedSelection);
  try {
    return await runHtmlStructuralAction(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.html.structureStatus = t("html-actions-duplicate-select");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }
      if (target.tag === "body" || target.tag === "html") {
        host.html.structureStatus = t("html-actions-root-cannot-duplicate");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }
      if (!target.sourceId) {
        const message = missingKernelIdentityMessage(t("html-actions-duplicate-noun"));
        host.html.structureStatus = message;
        host.commands.setStatus(message, "error");
        return blockedAction(message);
      }
      const receipt = await executePreviewHtmlDuplicateIntent({
        intent: {
          messageType: "preview-duplicate-selected",
          sourceId: target.sourceId,
          sourceTag: target.tag,
        },
        duplicateIntent: {
          sourceSourceId: target.sourceId,
          sourceTag: target.tag,
        },
      }, previewStructuralCommandIdentity(lease, true));
      const blocked = blockedReceiptOutcome(receipt, t("html-actions-duplicate-engine-blocked"));
      if (blocked) return blocked;
      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-actions-duplicate-engine-blocked"),
      );
      await commitHtmlStructuralPatch(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.html.structureStatus = t("html-actions-duplicated", { tag: patch.tag });
      });
      return committedAction();
    }, t("html-actions-duplicate-session-cancelled"));
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.structureStatus = t("html-actions-duplicate-failed", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-duplicate-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function mutateNativeBlockSlotStructure(
  host: HtmlActionsHost,
  request: NativeBlockSlotMutationRequest,
): Promise<EditorActionOutcome> {
  if (request.operation === "move") {
    return blockedAction(t("html-actions-native-slot-move-atomic"));
  }
  try {
    return await runHtmlStructuralAction(host, async (lease) => {
      const context = Object.freeze({ ...request.context });
      let receipt: PreviewStructuralExecutionReceipt;
      if (request.operation === "insert") {
        const targetSourceId = request.slot.containerSourceNodeId;
        if (!targetSourceId) {
          return blockedAction(t("html-actions-native-slot-container-missing"));
        }
        receipt = await executePreviewHtmlInsertDropIntent({
          intent: {
            messageType: "preview-insert-drop",
            targetSourceId,
            targetTag: "div",
            targetKind: "html",
            position: "inside",
            elementTag: "div",
          },
          insertIntent: {
            targetSourceId,
            targetTag: "div",
            targetKind: "html",
            position: "inside",
            element: {
              kind: "nativeBlockSlotItem",
              blockId: context.providerId,
              tag: "div",
              label: request.slot.itemKind,
            },
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      } else if (request.operation === "duplicate") {
        const item = request.item;
        if (!item) return blockedAction(t("html-actions-native-slot-duplicate-missing"));
        receipt = await executePreviewHtmlDuplicateIntent({
          intent: {
            messageType: "preview-duplicate-selected",
            sourceId: item.sourceNodeId,
            sourceTag: item.tag,
          },
          duplicateIntent: {
            sourceSourceId: item.sourceNodeId,
            sourceTag: item.tag,
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      } else {
        const item = request.item;
        if (!item) return blockedAction(t("html-actions-native-slot-delete-missing"));
        receipt = await executePreviewHtmlDeleteIntent({
          intent: {
            messageType: "preview-delete-selected",
            sourceId: item.sourceNodeId,
            sourceTag: item.tag,
          },
          deleteIntent: {
            targetSourceId: item.sourceNodeId,
            targetTag: item.tag,
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      }
      const blocked = blockedReceiptOutcome(receipt, t("html-actions-native-slot-blocked"));
      if (blocked) return blocked;
      if (receipt.status !== "committed" || !receipt.patch) {
        throw new Error(t("html-actions-native-slot-patch-missing"));
      }
      const patch = receipt.patch;
      await commitHtmlStructuralPatch(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.html.structureStatus = t("html-actions-native-slot-saved");
      });
      return committedAction();
    }, t("html-actions-native-slot-session-changed"));
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.commands.setStatus(
      outcome.reason ?? t("html-actions-native-slot-failed"),
      "error",
    );
    return outcome;
  }
}
