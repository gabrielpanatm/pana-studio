import { handlePreviewInsertDrop } from "$lib/state/preview-insert-controller";
import { handlePreviewLayerDrop } from "$lib/state/preview-drag-controller";
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
import type { TeraMoveRequest } from "$lib/tera/model";
import { errorMessage } from "$lib/util";
import {
  captureEditorHtmlTarget,
  captureEditorTeraTarget,
  htmlTargetFromPageSection,
  htmlTargetFromSelection,
  type EditorHtmlTarget,
  type EditorTeraTarget,
} from "$lib/editor-runtime/commands";
import type { SelectionInfo } from "$lib/types";

const projectionIntentTypes = new Set([
  "preview-layer-drop",
  "preview-insert-drop",
  "preview-tera-drop",
  "preview-tera-move-drop",
  "preview-delete-selected",
  "preview-template-delete-selected",
  "preview-template-edit-selected",
]);

export function isPreviewProjectionIntentMessage(type: unknown): type is string {
  return typeof type === "string" && projectionIntentTypes.has(type);
}

export async function handlePreviewProjectionIntent(
  app: AppState,
  data: Record<string, unknown>,
) {
  const htmlDeleteTarget = data.type === "preview-delete-selected"
    ? captureHtmlDeleteTarget(app, data)
    : null;
  const teraDeleteTarget = data.type === "preview-template-delete-selected"
    ? captureTeraDeleteTarget(app, data)
    : null;
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
      const diagnostic = receipt.diagnostics.find((item) => item.blocking)?.message;
      app.setGlobalStatus(diagnostic || receipt.message, "error");
      return;
    }
  } catch (error) {
    if (isPreviewStructuralCancellation(error)) return;
    app.setGlobalStatus(`Verificarea proiecției de previzualizare a eșuat: ${errorMessage(error)}`, "error");
    return;
  }

  if (data.type === "preview-layer-drop") {
    await handlePreviewLayerDrop(app.previewDragControllerHost(), data);
    return;
  }
  if (data.type === "preview-insert-drop") {
    await handlePreviewInsertDrop(app.previewInsertControllerHost(), data);
    return;
  }
  if (data.type === "preview-tera-drop") {
    await handlePreviewTeraInsertDrop(app.previewTeraInsertControllerHost(), data);
    return;
  }
  if (data.type === "preview-tera-move-drop") {
    await app.moveTeraNodeAtTarget(data as TeraMoveRequest);
    return;
  }
  if (data.type === "preview-delete-selected") {
    await app.editorRuntime.dispatch({
      type: "delete-html",
      surface: "preview",
      target: htmlDeleteTarget ?? captureEditorHtmlTarget({
        kind: "html",
        selector: stringField(data.selector) ?? "",
        tag: stringField(data.sourceTag) ?? "",
        sourceId: stringField(data.sourceId),
        sessionId: stringField(data.sourceSessionId) ?? stringField(data.sessionId),
      }),
    });
    return;
  }
  if (data.type === "preview-template-delete-selected") {
    if (teraDeleteTarget) {
      await app.editorRuntime.dispatch({
        type: "delete-tera",
        surface: "preview",
        target: teraDeleteTarget,
      });
    }
    return;
  }
  if (data.type === "preview-template-edit-selected") {
    app.selectTemplateGateFromBridge(data);
    await app.allowTemplateHtmlEditFromBridge(data);
  }
}

function captureHtmlDeleteTarget(
  app: AppState,
  data: Record<string, unknown>,
): EditorHtmlTarget | null {
  const embedded = data.target;
  if (
    embedded
    && typeof embedded === "object"
    && typeof (embedded as Record<string, unknown>).domPath === "string"
    && typeof (embedded as Record<string, unknown>).tag === "string"
  ) {
    try {
      return htmlTargetFromSelection(embedded as SelectionInfo);
    } catch {
      // A malformed/forged preview payload must fail closed without breaking
      // the shell message loop. The scalar fallback below remains bounded.
    }
  }

  const selector = stringField(data.selector);
  if (!selector) return null;
  const section = app.pageSections.find((item) => item.selector === selector) ?? null;
  if (section) return htmlTargetFromPageSection(section);
  return captureEditorHtmlTarget({
    kind: "html",
    selector,
    tag: stringField(data.sourceTag) ?? "",
    sourceId: stringField(data.sourceId),
    templateSourceId: stringField(data.templateSourceId),
    sessionId: stringField(data.sourceSessionId) ?? stringField(data.sessionId),
  });
}

function captureTeraDeleteTarget(
  app: AppState,
  data: Record<string, unknown>,
): EditorTeraTarget | null {
  const sourceId = stringField(data.sourceId);
  if (!sourceId) return null;
  const sourceNode = app.sourceGraph?.nodes.find((node) => node.id === sourceId) ?? null;
  return captureEditorTeraTarget({
    kind: "tera",
    sourceId,
    selector: stringField(data.selector),
    label: sourceNode?.label,
    kindLabel: sourceNode?.kind,
    file: sourceNode?.file ?? null,
    origin: sourceNode?.origin ?? null,
    themeName: sourceNode?.themeName ?? null,
    sourceNode,
  });
}

function previewProjectionIntentInputFromMessage(
  data: Record<string, unknown>,
): PreviewProjectionIntentInput {
  return {
    messageType: typeof data.type === "string" ? data.type : "",
    previewRevision: numberField(data.previewRevision),
    sourceSelector: stringField(data.sourceSelector),
    targetSelector: stringField(data.targetSelector),
    selector: stringField(data.selector),
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
