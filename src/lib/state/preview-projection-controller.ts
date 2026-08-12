import { handlePreviewInsertDrop } from "$lib/state/preview-insert-controller";
import { handlePreviewTeraInsertDrop } from "$lib/state/preview-tera-insert-controller";
import { normalizePreviewProjectionIntent } from "$lib/project/io";
import {
  capturePreviewStructuralSessionLease,
  isPreviewStructuralCancellation,
  previewStructuralCommandIdentity,
  requireCurrentPreviewStructuralSession,
  requirePreviewStructuralReceiptIdentity,
} from "$lib/kernel/preview-structural-lane";
import type { AppState } from "$lib/state/app.svelte";
import type { PreviewProjectionIntentInput } from "$lib/types";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

const projectionIntentTypes = new Set([
  "preview-insert-drop",
  "preview-tera-drop",
]);

export function isPreviewProjectionIntentMessage(type: unknown): type is string {
  return typeof type === "string" && projectionIntentTypes.has(type);
}

export async function handlePreviewProjectionIntent(
  app: AppState,
  data: Record<string, unknown>,
) {
  const input = previewProjectionIntentInputFromMessage(data);
  try {
    const lease = capturePreviewStructuralSessionLease(app);
    const receipt = await normalizePreviewProjectionIntent(
      input,
      previewStructuralCommandIdentity(lease),
    );
    requirePreviewStructuralReceiptIdentity(receipt, lease);
    requireCurrentPreviewStructuralSession(app, lease);
    if (!receipt.accepted) {
      const diagnostic = receipt.diagnostics.find((item) => item.blocking);
      app.setGlobalStatus(
        (diagnostic ? errorMessage(diagnostic.diagnostic) : "")
          || errorMessage(receipt.messageDiagnostic),
        "error",
      );
      return;
    }
  } catch (error) {
    if (isPreviewStructuralCancellation(error)) return;
    app.setGlobalStatus(t("preview-projection-verification-failed", {
      message: errorMessage(error),
    }), "error");
    return;
  }

  if (data.type === "preview-insert-drop") {
    await handlePreviewInsertDrop(app.previewInsertControllerHost(), data);
    return;
  }
  if (data.type === "preview-tera-drop") {
    await handlePreviewTeraInsertDrop(app.previewTeraInsertControllerHost(), data);
  }
}

function previewProjectionIntentInputFromMessage(
  data: Record<string, unknown>,
): PreviewProjectionIntentInput {
  return {
    messageType: typeof data.type === "string" ? data.type : "",
    previewRevision: numberField(data.previewRevision),
    sourceId: stringField(data.sourceId),
    targetSourceId: stringField(data.targetSourceId),
    sourceTemplateSourceId: stringField(data.sourceTemplateSourceId),
    targetTemplateSourceId: stringField(data.targetTemplateSourceId),
    sourceSessionId: stringField(data.sourceSessionId),
    targetSessionId: stringField(data.targetSessionId),
    sourceTag: stringField(data.sourceTag),
    targetTag: stringField(data.targetTag),
    targetKind: stringField(data.targetKind),
    position: stringField(data.position),
    itemKind: nestedStringField(data.item, "kind"),
    elementTag: nestedStringField(data.element, "tag"),
  };
}

function stringField(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function numberField(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function nestedStringField(value: unknown, field: string): string | null {
  if (!value || typeof value !== "object") return null;
  return stringField((value as Record<string, unknown>)[field]);
}
