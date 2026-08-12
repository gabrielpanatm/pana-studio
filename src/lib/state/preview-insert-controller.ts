import type { HtmlPaletteElement } from "$lib/project/html-palette";
import type { EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { DropPosition } from "$lib/ui/drag";
import { t } from "$lib/i18n/runtime.svelte";

export type PreviewInsertDropRequest = {
  targetRenderInstanceId: string | null;
  targetSessionId: string | null;
  targetSourceId: string | null;
  targetTemplateSourceId: string | null;
  targetBoundaryInstanceId: string | null;
  targetTag: string;
  targetKind?: "html" | "empty-tera-slot" | "active-document-root";
  position: DropPosition;
  element: HtmlPaletteElement;
};

export type PreviewInsertControllerHost = {
  insertPaletteElementAtTarget: (
    request: PreviewInsertDropRequest,
  ) => Promise<EditorActionOutcome>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  previewDropTargetStatus?: (target: {
    targetRenderInstanceId?: string | null;
    targetBoundarySourceId?: string | null;
    targetBoundaryInstanceId?: string | null;
  }) => { allowed: boolean; message?: string };
};

const dropPositions = new Set<DropPosition>(["before", "after", "inside"]);

function stringValue(value: unknown) {
  return typeof value === "string" ? value.trim() : "";
}

function dropPositionValue(value: unknown): DropPosition | null {
  return typeof value === "string" && dropPositions.has(value as DropPosition)
    ? value as DropPosition
    : null;
}

function paletteElementValue(value: unknown): HtmlPaletteElement | null {
  const data = value as Record<string, unknown> | null;
  if (!data || typeof data !== "object") return null;
  const tag = stringValue(data.tag).toLowerCase();
  const id = stringValue(data.id) || tag;
  const label = stringValue(data.label) || tag;
  if (!/^[a-z][a-z0-9-]*$/.test(tag)) return null;
  return {
    id,
    kind: data.kind === "block" ? "block" : "html",
    blockId: stringValue(data.blockId) || undefined,
    blockKind: data.blockKind === "js"
      ? "js"
      : data.blockKind === "css" ? "css" : data.blockKind === "static" ? "static" : undefined,
    tag,
    label,
    description: stringValue(data.description),
    text: typeof data.text === "string" ? data.text : "",
    className: typeof data.className === "string" ? data.className : "",
    html: typeof data.html === "string" ? data.html : "",
  };
}

export async function handlePreviewInsertDrop(
  host: PreviewInsertControllerHost,
  payload: unknown,
) {
  const data = payload as Record<string, unknown>;
  const targetRenderInstanceId = stringValue(data.targetRenderInstanceId) || null;
  const targetSessionId = stringValue(data.targetSessionId) || null;
  const targetSourceId = stringValue(data.targetSourceId) || null;
  const targetTemplateSourceId = stringValue(data.targetTemplateSourceId) || null;
  const targetBoundaryInstanceId = stringValue(data.targetBoundaryInstanceId) || null;
  const targetKind = data.targetKind === "active-document-root"
    ? "active-document-root"
    : data.targetKind === "empty-tera-slot" ? "empty-tera-slot" : "html";
  const documentRootTarget = targetKind !== "html";
  const targetTag = stringValue(data.targetTag).toLowerCase();
  const position = dropPositionValue(data.position);
  const element = paletteElementValue(data.element);

  if (!(targetSourceId || (documentRootTarget && targetTemplateSourceId)) || !targetTag || !position || !element) {
    host.setGlobalStatus(t("preview-drop-html-invalid"), "error");
    return;
  }

  const targetStatus = host.previewDropTargetStatus?.({
    targetRenderInstanceId,
    targetBoundarySourceId:
      documentRootTarget ? targetTemplateSourceId : null,
    targetBoundaryInstanceId:
      documentRootTarget ? targetBoundaryInstanceId : null,
  });
  if (targetStatus && !targetStatus.allowed) {
    host.setGlobalStatus(
      targetStatus.message || t("preview-drop-navigation-target-blocked"),
      "error",
    );
    return;
  }

  await host.insertPaletteElementAtTarget({
    targetRenderInstanceId,
    targetSessionId,
    targetSourceId,
    targetTemplateSourceId,
    targetBoundaryInstanceId,
    targetTag,
    targetKind,
    position,
    element,
  });
}
