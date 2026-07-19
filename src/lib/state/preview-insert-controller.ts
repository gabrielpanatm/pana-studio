import type { HtmlPaletteElement } from "$lib/project/html-palette";
import type { SaveState, SourceEditLocation, SourceEditTarget } from "$lib/types";
import type { DropPosition } from "$lib/ui/drag";

export type PreviewInsertDropRequest = {
  targetSelector: string;
  targetSessionId: string | null;
  targetSourceId: string | null;
  targetTemplateSourceId: string | null;
  targetSourceLocation: SourceEditLocation | null;
  targetTag: string;
  targetKind?: "html" | "empty-tera-slot";
  position: DropPosition;
  element: HtmlPaletteElement;
};

export type PreviewInsertControllerHost = {
  insertPaletteElementAtTarget: (request: PreviewInsertDropRequest) => Promise<void>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  resolveSourceEditTargetForSourceId?: (sourceId: string | null | undefined) => SourceEditTarget | null;
  previewDropGateStatus?: (target: {
    targetSourceId?: string | null;
    targetTemplateSourceId?: string | null;
  }) => { allowed: boolean; message?: string };
};

const dropPositions = new Set<DropPosition>(["before", "after", "inside"]);

function stringValue(value: unknown) {
  return typeof value === "string" ? value.trim() : "";
}

function sourceEditLocationValue(value: unknown): SourceEditLocation | null {
  if (!value || typeof value !== "object") return null;
  const data = value as Record<string, unknown>;
  if (typeof data.file !== "string" || typeof data.line !== "number") return null;
  return {
    file: data.file,
    line: data.line,
    column: typeof data.column === "number" ? data.column : undefined,
  };
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
    kind: data.kind === "component" ? "component" : "html",
    componentId: stringValue(data.componentId) || undefined,
    componentKind: data.componentKind === "js" ? "js" : data.componentKind === "css" ? "css" : undefined,
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
  const targetSelector = stringValue(data.targetSelector);
  const targetSessionId = stringValue(data.targetSessionId) || null;
  const targetSourceId = stringValue(data.targetSourceId) || null;
  const targetTemplateSourceId = stringValue(data.targetTemplateSourceId) || null;
  const targetKind = data.targetKind === "empty-tera-slot" ? "empty-tera-slot" : "html";
  const targetSource = host.resolveSourceEditTargetForSourceId?.(
    targetSourceId || (targetKind === "empty-tera-slot" ? targetTemplateSourceId : null),
  ) ?? null;
  const targetSourceLocation =
    sourceEditLocationValue(data.targetSourceLocation) ||
    targetSource?.location ||
    null;
  const targetTag = stringValue(data.targetTag).toLowerCase();
  const position = dropPositionValue(data.position);
  const element = paletteElementValue(data.element);

  if (!targetSelector || !targetTag || !position || !element) {
    host.setGlobalStatus("Drop invalid pentru element HTML.", "error");
    return;
  }

  const gate = host.previewDropGateStatus?.({ targetSourceId, targetTemplateSourceId });
  if (gate && !gate.allowed) {
    host.setGlobalStatus(gate.message || "Drop blocat de gate-ul Tera.", "error");
    return;
  }

  await host.insertPaletteElementAtTarget({
    targetSelector,
    targetSessionId,
    targetSourceId,
    targetTemplateSourceId,
    targetSourceLocation,
    targetTag,
    targetKind,
    position,
    element,
  });
}
