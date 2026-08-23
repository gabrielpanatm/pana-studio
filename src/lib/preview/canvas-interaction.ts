import type {
  CanvasProjectionIdentity,
} from "$lib/contracts/canvas-projection";
import {
  CANVAS_INTERACTION_SCHEMA_VERSION,
  type CanvasDragPosition,
  type CanvasDragSample,
  type CanvasHitCandidate,
  type CanvasHitCandidateKind,
  type CanvasInteractionGesture,
  type CanvasInteractionIdentity,
  type CanvasInteractionRequest,
  type CanvasPointerButton,
  type CanvasPointerModifiers,
  type CanvasPointerSample,
} from "$lib/canvas/contracts";
import type { CanvasElementObservation } from "$lib/canvas/contracts";

export const CANVAS_AGENT_MESSAGE_SOURCE = "pana-studio-canvas-agent";
/**
 * Versiunea protocolului fizic dintre iframe și frontend.
 *
 * Este deliberat independentă de CANVAS_INTERACTION_SCHEMA_VERSION, care
 * version-ează contractul semantic frontend <-> kernel Rust.
 */
export const CANVAS_AGENT_MESSAGE_SCHEMA_VERSION = 2 as const;
const MAX_AGENT_INSTANCE_ID_BYTES = 128;
const MAX_HIT_CANDIDATES = 64;
const MAX_INSTRUMENTED_ID_BYTES = 512;
const MAX_POINTER_COORDINATE = 10_000_000;
const MAX_ROUTE_BYTES = 2_048;
const MAX_DRAG_SESSION_ID_BYTES = 128;

const GESTURES = new Set<CanvasInteractionGesture>([
  "pointerMove",
  "pointerDown",
  "click",
  "contextMenu",
  "dragStart",
  "dragOver",
  "drop",
]);
const POINTER_BUTTONS = new Set<CanvasPointerButton>([
  "none",
  "primary",
  "auxiliary",
  "secondary",
  "back",
  "forward",
]);
const HIT_CANDIDATE_KINDS = new Set<CanvasHitCandidateKind>([
  "renderInstance",
  "boundaryInstance",
]);
const DRAG_POSITIONS = new Set<CanvasDragPosition>(["before", "after", "inside"]);

export type CanvasAgentReadyMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "agentReady";
  agentInstanceId: string;
};

export type CanvasAgentActivatedMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "agentActivated";
  agentInstanceId: string;
  documentEpoch: number;
};

export type CanvasAgentDragPreviewAppliedMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "dragPreviewApplied";
  agentInstanceId: string;
  documentEpoch: number;
  dragSessionId: string;
  gestureSequence: number;
  planToken: string;
  dragPreviewAppliedMs: number;
};

export type CanvasAgentGestureMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "gesture";
  agentInstanceId: string;
  documentEpoch: number;
  emittedAtMs: number;
  gestureSequence: number;
  gesture: CanvasInteractionGesture;
  pointer: CanvasPointerSample;
  hitPath: CanvasHitCandidate[];
  drag: CanvasDragSample | null;
};

export type CanvasAgentDomInspectionMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "domInspection";
  agentInstanceId: string;
  documentEpoch: number;
  inspectionRequestId: string;
  renderInstanceId: string;
  observation: CanvasElementObservation;
};

export type CanvasAgentActionMessage = {
  source: typeof CANVAS_AGENT_MESSAGE_SOURCE;
  schemaVersion: typeof CANVAS_AGENT_MESSAGE_SCHEMA_VERSION;
  type: "action";
  agentInstanceId: string;
  documentEpoch: number;
  actionSequence: number;
  selectionRevision: number;
  editorNodeId: string;
  action: "enterBoundary" | "deleteSelection";
};

export type CanvasAgentMessage =
  | CanvasAgentReadyMessage
  | CanvasAgentActivatedMessage
  | CanvasAgentDragPreviewAppliedMessage
  | CanvasAgentGestureMessage
  | CanvasAgentDomInspectionMessage
  | CanvasAgentActionMessage;

/**
 * Validează exclusiv mesajele fizice ale CanvasAgent-ului montat.
 *
 * Tipul semantic, sursa, capabilitățile și limita Tera lipsesc intenționat
 * din acest contract: ele sunt proiectate ulterior de kernelul Rust.
 */
export function parseCanvasAgentMessage(
  frame: Pick<HTMLIFrameElement, "contentWindow"> | null | undefined,
  event: Pick<MessageEvent, "source" | "data">,
  expectedAgentInstanceId: string | null = null,
): CanvasAgentMessage | null {
  if (!frame?.contentWindow || event.source !== frame.contentWindow) return null;
  if (!isRecord(event.data)) return null;
  const data = event.data;
  if (
    data.source !== CANVAS_AGENT_MESSAGE_SOURCE
    || data.schemaVersion !== CANVAS_AGENT_MESSAGE_SCHEMA_VERSION
  ) return null;

  const agentInstanceId = boundedNonEmptyString(
    data.agentInstanceId,
    MAX_AGENT_INSTANCE_ID_BYTES,
  );
  if (
    !agentInstanceId
    || (expectedAgentInstanceId !== null && agentInstanceId !== expectedAgentInstanceId)
  ) return null;

  if (data.type === "agentReady") {
    return {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      type: "agentReady",
      agentInstanceId,
    };
  }
  if (data.type === "agentActivated") {
    const documentEpoch = positiveSafeInteger(data.documentEpoch);
    if (documentEpoch === null) return null;
    return {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      type: "agentActivated",
      agentInstanceId,
      documentEpoch,
    };
  }
  if (data.type === "dragPreviewApplied") {
    const documentEpoch = positiveSafeInteger(data.documentEpoch);
    const dragSessionId = boundedNonEmptyString(
      data.dragSessionId,
      MAX_DRAG_SESSION_ID_BYTES,
    );
    const gestureSequence = positiveSafeInteger(data.gestureSequence);
    const planToken = boundedNonEmptyString(data.planToken, 256);
    const dragPreviewAppliedMs = boundedSafeInteger(
      data.dragPreviewAppliedMs,
      0,
      600_000,
    );
    if (
      documentEpoch === null
      || !dragSessionId
      || gestureSequence === null
      || !planToken
      || dragPreviewAppliedMs === null
    ) return null;
    return {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      type: "dragPreviewApplied",
      agentInstanceId,
      documentEpoch,
      dragSessionId,
      gestureSequence,
      planToken,
      dragPreviewAppliedMs,
    };
  }
  if (data.type === "domInspection") {
    const documentEpoch = positiveSafeInteger(data.documentEpoch);
    const inspectionRequestId = boundedNonEmptyString(
      data.inspectionRequestId,
      MAX_AGENT_INSTANCE_ID_BYTES,
    );
    const renderInstanceId = boundedNonEmptyString(
      data.renderInstanceId,
      MAX_INSTRUMENTED_ID_BYTES,
    );
    const observation = renderInstanceId
      ? parseDomInspection(data.observation, renderInstanceId)
      : null;
    if (
      documentEpoch === null
      || !inspectionRequestId
      || !renderInstanceId
      || !observation
    ) return null;
    return {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      type: "domInspection",
      agentInstanceId,
      documentEpoch,
      inspectionRequestId,
      renderInstanceId,
      observation,
    };
  }
  if (data.type === "action") {
    const documentEpoch = positiveSafeInteger(data.documentEpoch);
    const actionSequence = positiveSafeInteger(data.actionSequence);
    const selectionRevision = positiveSafeInteger(data.selectionRevision);
    const editorNodeId = boundedNonEmptyString(
      data.editorNodeId,
      MAX_INSTRUMENTED_ID_BYTES,
    );
    if (
      documentEpoch === null
      || actionSequence === null
      || selectionRevision === null
      || !editorNodeId
      || (data.action !== "enterBoundary" && data.action !== "deleteSelection")
    ) return null;
    return {
      source: CANVAS_AGENT_MESSAGE_SOURCE,
      schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
      type: "action",
      agentInstanceId,
      documentEpoch,
      actionSequence,
      selectionRevision,
      editorNodeId,
      action: data.action,
    };
  }
  if (data.type !== "gesture") return null;

  const documentEpoch = positiveSafeInteger(data.documentEpoch);
  const emittedAtMs = positiveSafeInteger(data.emittedAtMs);
  const gestureSequence = positiveSafeInteger(data.gestureSequence);
  const gesture = enumValue(data.gesture, GESTURES);
  const pointer = parsePointer(data.pointer);
  const hitPath = parseHitPath(data.hitPath);
  const drag = parseDragSample(data.drag, gesture);
  if (
    documentEpoch === null
    || emittedAtMs === null
    || gestureSequence === null
    || !gesture
    || !pointer
    || !hitPath
    || drag === undefined
  ) return null;

  return {
    source: CANVAS_AGENT_MESSAGE_SOURCE,
    schemaVersion: CANVAS_AGENT_MESSAGE_SCHEMA_VERSION,
    type: "gesture",
    agentInstanceId,
    documentEpoch,
    emittedAtMs,
    gestureSequence,
    gesture,
    pointer,
    hitPath,
    drag,
  };
}

export function createCanvasInteractionIdentity(
  canvas: CanvasProjectionIdentity,
  route: string,
  documentEpoch: number,
  agentInstanceId: string,
): CanvasInteractionIdentity {
  const normalizedRoute = route.trim();
  const normalizedAgentId = agentInstanceId.trim();
  if (
    !normalizedRoute
    || utf8Length(normalizedRoute) > MAX_ROUTE_BYTES
    || !Number.isSafeInteger(documentEpoch)
    || documentEpoch <= 0
    || !normalizedAgentId
    || utf8Length(normalizedAgentId) > MAX_AGENT_INSTANCE_ID_BYTES
  ) {
    throw new Error("Identitatea CanvasAgent este invalidă.");
  }
  return {
    canvas: { ...canvas },
    route: normalizedRoute,
    documentEpoch,
    agentInstanceId: normalizedAgentId,
  };
}

export function createCanvasInteractionRequest(
  identity: CanvasInteractionIdentity,
  message: CanvasAgentGestureMessage,
): CanvasInteractionRequest {
  if (
    message.agentInstanceId !== identity.agentInstanceId
    || message.documentEpoch !== identity.documentEpoch
  ) {
    throw new Error("Gestul nu aparține binding-ului CanvasAgent activ.");
  }
  return {
    schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
    identity,
    emittedAtMs: message.emittedAtMs,
    gestureSequence: message.gestureSequence,
    gesture: message.gesture,
    pointer: message.pointer,
    hitPath: message.hitPath,
    drag: message.drag,
  };
}

function parseDragSample(
  value: unknown,
  gesture: CanvasInteractionGesture | null,
): CanvasDragSample | null | undefined {
  const dragGesture = gesture === "dragStart"
    || gesture === "dragOver"
    || gesture === "drop";
  if (!dragGesture) return value === null ? null : undefined;
  if (!isRecord(value)) return undefined;
  const sessionId = boundedNonEmptyString(value.sessionId, MAX_DRAG_SESSION_ID_BYTES);
  const position = value.position === null
    ? null
    : enumValue(value.position, DRAG_POSITIONS);
  if (!sessionId || position === undefined || (value.position !== null && !position)) {
    return undefined;
  }
  if (gesture === "dragStart" && position !== null) return undefined;
  if ((gesture === "dragOver" || gesture === "drop") && position === null) {
    return undefined;
  }
  return { sessionId, position };
}

function parsePointer(value: unknown): CanvasPointerSample | null {
  if (!isRecord(value)) return null;
  const clientX = finiteBoundedNumber(value.clientX, MAX_POINTER_COORDINATE);
  const clientY = finiteBoundedNumber(value.clientY, MAX_POINTER_COORDINATE);
  const button = enumValue(value.button, POINTER_BUTTONS);
  const buttons = boundedSafeInteger(value.buttons, 0, 65_535);
  const modifiers = parseModifiers(value.modifiers);
  if (
    clientX === null
    || clientY === null
    || !button
    || buttons === null
    || !modifiers
  ) return null;
  return { clientX, clientY, button, buttons, modifiers };
}

function parseModifiers(value: unknown): CanvasPointerModifiers | null {
  if (!isRecord(value)) return null;
  if (
    typeof value.alt !== "boolean"
    || typeof value.control !== "boolean"
    || typeof value.meta !== "boolean"
    || typeof value.shift !== "boolean"
  ) return null;
  return {
    alt: value.alt,
    control: value.control,
    meta: value.meta,
    shift: value.shift,
  };
}

function parseHitPath(value: unknown): CanvasHitCandidate[] | null {
  if (!Array.isArray(value) || value.length > MAX_HIT_CANDIDATES) return null;
  const candidates: CanvasHitCandidate[] = [];
  const seen = new Set<string>();
  for (const candidate of value) {
    if (!isRecord(candidate)) return null;
    const kind = enumValue(candidate.kind, HIT_CANDIDATE_KINDS);
    const id = boundedNonEmptyString(candidate.id, MAX_INSTRUMENTED_ID_BYTES);
    if (!kind || !id) return null;
    const key = `${kind}\u0000${id}`;
    if (seen.has(key)) return null;
    seen.add(key);
    candidates.push({ kind, id });
  }
  return candidates;
}

function parseDomInspection(
  value: unknown,
  expectedRenderInstanceId: string,
): CanvasElementObservation | null {
  if (!isRecord(value)) return null;
  const renderInstanceId = boundedNonEmptyString(
    value.renderInstanceId,
    MAX_INSTRUMENTED_ID_BYTES,
  );
  const tag = boundedNonEmptyString(value.tag, 64)?.toLowerCase() ?? null;
  const rect = parseStringRecord(value.rect, ["width", "height", "top", "left"], 64);
  if (renderInstanceId !== expectedRenderInstanceId || !tag || !rect) return null;

  return {
    selector: boundedString(value.selector, 2_048),
    cssSelector: boundedString(value.cssSelector, 2_048),
    domPath: boundedString(value.domPath, 4_096),
    tag,
    id: boundedString(value.id, 512),
    href: boundedString(value.href, 4_096),
    title: boundedString(value.title, 2_048),
    alt: boundedString(value.alt, 2_048),
    classes: parseStringArray(value.classes, 64, 256),
    text: boundedString(value.text, 512),
    rawText: boundedString(value.rawText, 65_536),
    hasChildElements: value.hasChildElements === true,
    rect: {
      width: rect.width,
      height: rect.height,
      top: rect.top,
      left: rect.left,
    },
    styles: parseKeyValueRows(value.styles, "label", 32),
    variables: parseKeyValueRows(value.variables, "name", 256),
    matchedRules: parseMatchedRules(value.matchedRules),
    imageSrc: nullableBoundedString(value.imageSrc, 4_096),
    zolaImage: parseZolaImage(value.zolaImage),
    attributes: parseAttributes(value.attributes),
    parentNode: parseDomNodeLink(value.parentNode),
    childNodes: Array.isArray(value.childNodes)
      ? value.childNodes
        .slice(0, 24)
        .map(parseDomNodeLink)
        .filter((item): item is NonNullable<CanvasElementObservation["parentNode"]> => item !== null)
      : [],
    blockContext: parseBlockContext(value.blockContext),
  };
}

function parseKeyValueRows(
  value: unknown,
  keyName: "label",
  maximumRows: number,
): CanvasElementObservation["styles"];
function parseKeyValueRows(
  value: unknown,
  keyName: "name",
  maximumRows: number,
): CanvasElementObservation["variables"];
function parseKeyValueRows(
  value: unknown,
  keyName: "label" | "name",
  maximumRows: number,
) {
  if (!Array.isArray(value)) return [];
  return value.slice(0, maximumRows).flatMap((entry) => {
    if (!isRecord(entry)) return [];
    const key = boundedNonEmptyString(entry[keyName], 256);
    if (!key) return [];
    return [{
      [keyName]: key,
      value: boundedString(entry.value, 2_048),
    }];
  });
}

function parseMatchedRules(value: unknown): CanvasElementObservation["matchedRules"] {
  if (!Array.isArray(value)) return [];
  return value.slice(0, 256).flatMap((entry) => {
    if (!isRecord(entry)) return [];
    const selector = boundedNonEmptyString(entry.selector, 2_048);
    if (!selector) return [];
    return [{
      selector,
      source: boundedString(entry.source, 2_048),
      media: nullableBoundedString(entry.media, 1_024),
      declarations: boundedSafeInteger(entry.declarations, 0, 100_000) ?? 0,
      kind: boundedString(entry.kind, 128),
      score: boundedFiniteNumber(entry.score, -1_000_000, 1_000_000) ?? 0,
    }];
  });
}

function parseAttributes(value: unknown) {
  if (!isRecord(value)) return {};
  const result: Record<string, string> = {};
  for (const [name, attributeValue] of Object.entries(value).slice(0, 128)) {
    const boundedName = boundedNonEmptyString(name, 256);
    if (!boundedName || boundedName.toLowerCase().startsWith("data-pana-")) continue;
    result[boundedName] = boundedString(attributeValue, 4_096);
  }
  return result;
}

function parseDomNodeLink(value: unknown): CanvasElementObservation["parentNode"] {
  if (!isRecord(value)) return null;
  const tag = boundedNonEmptyString(value.tag, 64)?.toLowerCase();
  const selector = boundedNonEmptyString(value.selector, 4_096);
  if (!tag || !selector) return null;
  return {
    tag,
    selector,
    label: boundedString(value.label, 512),
  };
}

function parseBlockContext(value: unknown): CanvasElementObservation["blockContext"] {
  if (!isRecord(value)) return null;
  const providerId = boundedNonEmptyString(value.providerId, 256);
  const rootSelector = boundedNonEmptyString(value.rootSelector, 4_096);
  const rootTag = boundedNonEmptyString(value.rootTag, 64)?.toLowerCase();
  if (!providerId || !rootSelector || !rootTag) return null;
  return {
    providerId,
    rootSelector,
    rootTag,
  };
}

function parseZolaImage(value: unknown): CanvasElementObservation["zolaImage"] {
  if (!isRecord(value)) return null;
  const sourceUrl = boundedNonEmptyString(value.sourceUrl, 4_096);
  const sourcePath = boundedNonEmptyString(value.sourcePath, 4_096);
  const width = boundedSafeInteger(value.width, 1, 100_000);
  const height = value.height === null
    ? null
    : boundedSafeInteger(value.height, 1, 100_000);
  const operation = enumValue(
    value.operation,
    new Set<"fit_width" | "fit" | "fill">(["fit_width", "fit", "fill"]),
  );
  const format = enumValue(
    value.format,
    new Set<"auto" | "webp" | "avif" | "jpg" | "png">([
      "auto",
      "webp",
      "avif",
      "jpg",
      "png",
    ]),
  );
  const quality = boundedSafeInteger(value.quality, 1, 100);
  if (
    !sourceUrl
    || !sourcePath
    || width === null
    || (value.height !== null && height === null)
    || !operation
    || !format
    || quality === null
  ) return null;
  return { sourceUrl, sourcePath, width, height, operation, format, quality };
}

function parseStringArray(value: unknown, maximumItems: number, maximumBytes: number) {
  if (!Array.isArray(value)) return [];
  return value
    .slice(0, maximumItems)
    .map((entry) => boundedNonEmptyString(entry, maximumBytes))
    .filter((entry): entry is string => entry !== null);
}

function parseStringRecord(
  value: unknown,
  keys: readonly string[],
  maximumBytes: number,
) {
  if (!isRecord(value)) return null;
  const result: Record<string, string> = {};
  for (const key of keys) {
    if (typeof value[key] !== "string") return null;
    result[key] = boundedString(value[key], maximumBytes);
  }
  return result;
}

function enumValue<T extends string>(
  value: unknown,
  accepted: ReadonlySet<T>,
): T | null {
  return typeof value === "string" && accepted.has(value as T)
    ? value as T
    : null;
}

function positiveSafeInteger(value: unknown) {
  return boundedSafeInteger(value, 1, Number.MAX_SAFE_INTEGER);
}

function boundedSafeInteger(value: unknown, minimum: number, maximum: number) {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= minimum
    && value <= maximum
    ? value
    : null;
}

function boundedFiniteNumber(value: unknown, minimum: number, maximum: number) {
  return typeof value === "number"
    && Number.isFinite(value)
    && value >= minimum
    && value <= maximum
    ? value
    : null;
}

function finiteBoundedNumber(value: unknown, maximumMagnitude: number) {
  return typeof value === "number"
    && Number.isFinite(value)
    && Math.abs(value) <= maximumMagnitude
    ? value
    : null;
}

function boundedNonEmptyString(value: unknown, maximumBytes: number) {
  if (typeof value !== "string") return null;
  const normalized = value.trim();
  return normalized && utf8Length(normalized) <= maximumBytes ? normalized : null;
}

function boundedString(value: unknown, maximumBytes: number) {
  if (typeof value !== "string") return "";
  if (utf8Length(value) <= maximumBytes) return value;
  return new TextDecoder().decode(
    new TextEncoder().encode(value).slice(0, maximumBytes),
  );
}

function nullableBoundedString(value: unknown, maximumBytes: number) {
  if (value === null || value === undefined) return null;
  const bounded = boundedString(value, maximumBytes);
  return bounded || null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function utf8Length(value: string) {
  return new TextEncoder().encode(value).byteLength;
}
