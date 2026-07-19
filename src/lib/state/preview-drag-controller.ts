import { blockedAction, type EditorActionOutcome } from "$lib/editor-runtime/action-outcome";
import type { LayerMoveRequest } from "$lib/project/layers-drag";
import type { SaveState } from "$lib/types";
import type { DropPosition } from "$lib/ui/drag";

export type PreviewDragControllerHost = {
  moveLayerElement: (request: LayerMoveRequest) => Promise<EditorActionOutcome>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  previewDropGateStatus?: (target: {
    targetSourceId?: string | null;
    targetTemplateSourceId?: string | null;
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

export async function handlePreviewLayerDrop(
  host: PreviewDragControllerHost,
  payload: unknown,
) {
  const data = payload as Record<string, unknown>;
  const sourceSelector = stringValue(data.sourceSelector);
  const targetSelector = stringValue(data.targetSelector);
  const targetKind = data.targetKind === "empty-tera-slot" ? "empty-tera-slot" : "html";
  const sourceSessionId = stringValue(data.sourceSessionId) || null;
  const sourceSourceId = stringValue(data.sourceSourceId) || null;
  const sourceTemplateSourceId = stringValue(data.sourceTemplateSourceId) || null;
  const targetSessionId = stringValue(data.targetSessionId) || null;
  const targetSourceId = stringValue(data.targetSourceId) || null;
  const targetTemplateSourceId = stringValue(data.targetTemplateSourceId) || null;
  const position = dropPositionValue(data.position);

  if (!sourceSelector || !targetSelector || !position) {
    const reason = "Drop invalid din preview.";
    host.setGlobalStatus(reason, "error");
    return blockedAction(reason);
  }

  const gate = host.previewDropGateStatus?.({ targetSourceId, targetTemplateSourceId });
  if (gate && !gate.allowed) {
    const reason = gate.message || "Drop blocat de gate-ul Tera.";
    host.setGlobalStatus(reason, "error");
    return blockedAction(reason);
  }

  return await host.moveLayerElement({
    sourceSelector,
    targetSelector,
    sourceSessionId,
    sourceSourceId,
    sourceTemplateSourceId,
    targetSessionId,
    targetSourceId,
    targetTemplateSourceId,
    targetKind,
    position,
  });
}
