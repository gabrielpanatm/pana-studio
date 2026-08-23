import type {
  CanvasElementObservation,
  CoordinatedElementSelection,
} from "$lib/canvas/contracts";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type { SourceGraphNode } from "$lib/source-graph/contracts";
import type { EditorActionStatus } from "$lib/editor-runtime/action-outcome";
import { t } from "$lib/i18n/runtime.svelte";

export type EditorSurface = "preview" | "layers" | "inspector" | "code" | "shortcut" | "runtime";

export type EditorHtmlTarget = {
  kind: "html";
  tag: string;
  label?: string;
  text?: string;
  selectionRevision?: number | null;
  renderInstanceId?: string | null;
  sourceLocation?: SourceEditLocation | null;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sessionId?: string | null;
  observation?: CanvasElementObservation | null;
};

export type EditorTeraTarget = {
  kind: "tera";
  editorNodeId?: string | null;
  sourceId: string;
  renderInstanceId?: string | null;
  label?: string;
  kindLabel?: string;
  file?: string | null;
  origin?: "current" | "local" | "theme" | "unknown" | null;
  themeName?: string | null;
  canEnterBoundary?: boolean;
  sourceNode?: SourceGraphNode | null;
};

export type EditorTarget = EditorHtmlTarget | EditorTeraTarget;

export type EditorCommand =
  | { type: "select-html"; surface?: EditorSurface; target: EditorHtmlTarget; revealCode?: boolean }
  | { type: "open-html-code"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "duplicate-html"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "delete-html"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "select-tera"; surface?: EditorSurface; target: EditorTeraTarget }
  | { type: "enter-tera-boundary"; surface?: EditorSurface; target: EditorTeraTarget }
  | { type: "open-tera-code"; surface?: EditorSurface; target: EditorTeraTarget }
  | { type: "delete-tera"; surface?: EditorSurface; target: EditorTeraTarget };

export type EditorCommandResult = {
  ok: boolean;
  status: EditorActionStatus;
  revision: number;
  command: EditorCommand["type"];
  reason?: string;
};

export type EditorTransaction = {
  revision: number;
  command: EditorCommand["type"];
  surface: EditorSurface;
  targetKind: EditorTarget["kind"];
  sourceId: string | null;
  startedAt: number;
  completedAt?: number;
  ok?: boolean;
  status?: EditorActionStatus;
  reason?: string;
};

export function captureCanvasElementObservation(
  observation: CanvasElementObservation | null | undefined,
): CanvasElementObservation | null {
  if (!observation) return null;
  return Object.freeze({
    ...observation,
    classes: Object.freeze([...observation.classes]) as unknown as string[],
    rect: Object.freeze({ ...observation.rect }),
    styles: Object.freeze(observation.styles.map((row) => Object.freeze({ ...row }))) as unknown as CanvasElementObservation["styles"],
    variables: Object.freeze(observation.variables.map((row) => Object.freeze({ ...row }))) as unknown as CanvasElementObservation["variables"],
    matchedRules: Object.freeze(observation.matchedRules.map((rule) => Object.freeze({ ...rule }))) as unknown as CanvasElementObservation["matchedRules"],
    attributes: Object.freeze({ ...observation.attributes }),
    parentNode: observation.parentNode ? Object.freeze({ ...observation.parentNode }) : null,
    childNodes: Object.freeze(observation.childNodes.map((node) => Object.freeze({ ...node }))) as unknown as CanvasElementObservation["childNodes"],
    blockContext: observation.blockContext
      ? Object.freeze({ ...observation.blockContext })
      : null,
  });
}

function captureSourceGraphNode(node: SourceGraphNode | null | undefined): SourceGraphNode | null {
  if (!node) return null;
  return Object.freeze({
    ...node,
    range: node.range ? Object.freeze({ ...node.range }) : null,
    children: Object.freeze([...node.children]) as unknown as string[],
    capabilities: Object.freeze({ ...node.capabilities }),
  });
}

/**
 * Captures the complete mutation target at the interaction boundary. The
 * structural lane may wait behind another commit, so retaining references to
 * reactive selection/section objects would let a later selection retarget an
 * already queued command.
 */
export function captureEditorHtmlTarget(target: EditorHtmlTarget): EditorHtmlTarget {
  return Object.freeze({
    ...target,
    sourceLocation: target.sourceLocation
      ? Object.freeze({ ...target.sourceLocation })
      : null,
    observation: captureCanvasElementObservation(target.observation),
  });
}

export function captureEditorTeraTarget(target: EditorTeraTarget): EditorTeraTarget {
  return Object.freeze({
    ...target,
    sourceNode: captureSourceGraphNode(target.sourceNode),
  });
}

export function captureEditorCommand(command: EditorCommand): EditorCommand {
  return Object.freeze({
    ...command,
    target: command.target.kind === "html"
      ? captureEditorHtmlTarget(command.target)
      : captureEditorTeraTarget(command.target),
  }) as EditorCommand;
}

export function htmlTargetFromCoordinatedSelection(
  selection: CoordinatedElementSelection,
): EditorHtmlTarget {
  const observation = selection.observation;
  return captureEditorHtmlTarget({
    kind: "html",
    tag: observation.tag,
    label: observation.selector || `<${observation.tag}>`,
    text: observation.text,
    selectionRevision: selection.snapshot.selectionRevision,
    renderInstanceId: selection.renderInstanceId,
    sourceLocation: selection.sourceLocation,
    sourceId: selection.sourceNodeId,
    templateSourceId: null,
    sessionId: selection.snapshot.runtimeSessionId,
    observation,
  });
}

export function teraTargetFromBoundary(target: {
  sourceId: string;
  renderInstanceId?: string | null;
  origin?: "current" | "local" | "theme" | "unknown" | null;
  themeName?: string | null;
  editorNodeId?: string | null;
  canEnterBoundary?: boolean;
}, options: Partial<EditorTeraTarget> = {}): EditorTeraTarget {
  return captureEditorTeraTarget({
    kind: "tera",
    sourceId: target.sourceId,
    renderInstanceId: target.renderInstanceId ?? null,
    origin: target.origin ?? null,
    themeName: target.themeName ?? null,
    editorNodeId: target.editorNodeId ?? null,
    canEnterBoundary: target.canEnterBoundary,
    ...options,
  });
}

export function canMutateHtmlTarget(target: EditorHtmlTarget) {
  if (!target.sourceId) {
    return { allowed: false, reason: t("editor-runtime-html-source-id-missing") };
  }
  if (target.tag === "body" || target.tag === "html") {
    return { allowed: false, reason: t("editor-runtime-root-structural-blocked") };
  }
  return { allowed: true, reason: "" };
}
