import {
  blockedAction,
  committedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  canElementAcceptChildren,
  htmlVoidTags,
} from "$lib/html/mutations";
import { htmlPaletteInsertOptions } from "$lib/html/palette";
import { previewStructuralCommandIdentity } from "$lib/kernel/preview-structural-lane";
import { requireCommittedPreviewStructuralPatch } from "$lib/kernel/preview-projection-control";
import { executePreviewHtmlInsertDropIntent } from "$lib/preview/structural-io";
import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
import type { HtmlActionsHost } from "$lib/editor/html-actions/host";
import {
  actionErrorOutcome,
  cacheCommittedHtmlPatch,
  commitHtmlStructuralPatch,
  runHtmlStructuralAction,
} from "$lib/editor/html-actions/execution";
import { t } from "$lib/i18n/runtime.svelte";

function insertPositionLabel(position: PreviewInsertDropRequest["position"]) {
  if (position === "before") return t("html-actions-position-before");
  if (position === "after") return t("html-actions-position-after");
  return t("html-actions-position-inside");
}

export async function insertPaletteElementAtTarget(
  host: HtmlActionsHost,
  request: PreviewInsertDropRequest,
): Promise<EditorActionOutcome> {
  const capturedRequest = Object.freeze({
    ...request,
    element: Object.freeze({ ...request.element }),
  });
  try {
    return await runHtmlStructuralAction(host, async (lease) => {
      const targetSourceId = capturedRequest.targetSourceId || (
        capturedRequest.targetKind === "empty-tera-slot"
        || capturedRequest.targetKind === "active-document-root"
          ? capturedRequest.targetTemplateSourceId
          : null
      );
      if (!host.context().canEditStructure) {
        host.html.structureStatus = t("html-actions-switch-preview");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }
      if (
        capturedRequest.position === "inside"
        && !canElementAcceptChildren(capturedRequest.targetTag, htmlVoidTags)
      ) {
        host.html.structureStatus = t("html-actions-target-no-children");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }
      if (!targetSourceId) {
        host.html.structureStatus = t("html-actions-target-metadata-unstable");
        host.commands.setStatus(host.html.structureStatus, "error");
        return blockedAction(host.html.structureStatus);
      }

      const blockId = capturedRequest.element.kind === "block"
        ? capturedRequest.element.blockId ?? null
        : null;
      const options = blockId
        ? {
            tag: capturedRequest.element.tag,
            className: capturedRequest.element.className,
            text: capturedRequest.element.text,
            html: capturedRequest.element.html,
          }
        : htmlPaletteInsertOptions(capturedRequest.element);
      const label = insertPositionLabel(capturedRequest.position);
      const receipt = await executePreviewHtmlInsertDropIntent({
        intent: {
          messageType: "preview-insert-drop",
          targetSourceId,
          targetTemplateSourceId: capturedRequest.targetTemplateSourceId,
          targetSessionId: capturedRequest.targetSessionId,
          targetTag: capturedRequest.targetTag,
          targetKind: capturedRequest.targetKind ?? "html",
          position: capturedRequest.position,
          elementTag: capturedRequest.element.tag,
        },
        insertIntent: {
          targetSourceId,
          targetTag: capturedRequest.targetTag,
          targetKind: capturedRequest.targetKind ?? "html",
          position: capturedRequest.position,
          element: {
            kind: capturedRequest.element.kind,
            blockId,
            tag: options.tag,
            className: options.className,
            text: options.text,
            label: capturedRequest.element.label,
          },
        },
      }, previewStructuralCommandIdentity(lease));
      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-actions-insert-engine-blocked"),
      );
      await commitHtmlStructuralPatch(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.html.structureStatus = t("html-actions-inserted-saved", {
          tag: patch.tag,
          position: label,
        });
      });
      return committedAction();
    }, "");
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.html.structureStatus = t("html-actions-insert-failed", {
      message: result.reason ?? result.status,
    });
    host.commands.setStatus(t("html-actions-insert-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}
