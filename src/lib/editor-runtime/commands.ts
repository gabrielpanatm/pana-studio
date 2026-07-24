import type { PageSection, SelectionInfo, SourceGraphNode } from "$lib/types";
import type { EditorActionStatus } from "$lib/editor-runtime/action-outcome";

export type EditorSurface = "preview" | "layers" | "inspector" | "code" | "shortcut" | "runtime";

export type EditorHtmlTarget = {
  kind: "html";
  selector: string;
  tag: string;
  label?: string;
  text?: string;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sessionId?: string | null;
  selection?: SelectionInfo | null;
  section?: PageSection | null;
};

export type EditorTeraTarget = {
  kind: "tera";
  sourceId: string;
  selector: string | null;
  label?: string;
  kindLabel?: string;
  file?: string | null;
  origin?: "current" | "local" | "theme" | "unknown" | null;
  themeName?: string | null;
  canSelectHtml?: boolean;
  section?: PageSection | null;
  sourceNode?: SourceGraphNode | null;
};

export type EditorTarget = EditorHtmlTarget | EditorTeraTarget;

export type EditorCommand =
  | { type: "select-html"; surface?: EditorSurface; target: EditorHtmlTarget; revealCode?: boolean }
  | { type: "open-html-code"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "duplicate-html"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "delete-html"; surface?: EditorSurface; target: EditorHtmlTarget }
  | { type: "select-tera"; surface?: EditorSurface; target: EditorTeraTarget }
  | { type: "edit-tera-html"; surface?: EditorSurface; target: EditorTeraTarget }
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
  selector: string | null;
  sourceId: string | null;
  startedAt: number;
  completedAt?: number;
  ok?: boolean;
  status?: EditorActionStatus;
  reason?: string;
};

export type EditorLayerContextMenuRequest =
  | {
      kind: "html";
      section: PageSection;
      x: number;
      y: number;
      label?: string;
    }
  | {
      kind: "tera";
      section: PageSection;
      sourceId: string;
      selector: string | null;
      x: number;
      y: number;
      label?: string;
      kindLabel?: string;
      file?: string | null;
      origin?: "local" | "theme" | "unknown" | null;
      themeName?: string | null;
    };

function capturePageSection(section: PageSection | null | undefined): PageSection | null {
  if (!section) return null;
  return Object.freeze({
    ...section,
    sourceLocation: section.sourceLocation
      ? Object.freeze({ ...section.sourceLocation })
      : null,
  });
}

function captureSelectionInfo(selection: SelectionInfo | null | undefined): SelectionInfo | null {
  if (!selection) return null;
  return Object.freeze({
    ...selection,
    classes: Object.freeze([...selection.classes]) as unknown as string[],
    rect: Object.freeze({ ...selection.rect }),
    styles: Object.freeze(selection.styles.map((row) => Object.freeze({ ...row }))) as unknown as SelectionInfo["styles"],
    variables: Object.freeze(selection.variables.map((row) => Object.freeze({ ...row }))) as unknown as SelectionInfo["variables"],
    matchedRules: Object.freeze(selection.matchedRules.map((rule) => Object.freeze({ ...rule }))) as unknown as SelectionInfo["matchedRules"],
    attributes: Object.freeze({ ...selection.attributes }),
    parentNode: selection.parentNode ? Object.freeze({ ...selection.parentNode }) : null,
    childNodes: Object.freeze(selection.childNodes.map((node) => Object.freeze({ ...node }))) as unknown as SelectionInfo["childNodes"],
    sourceLocation: selection.sourceLocation
      ? Object.freeze({ ...selection.sourceLocation })
      : null,
    blockContext: selection.blockContext
      ? Object.freeze({ ...selection.blockContext })
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
    selection: captureSelectionInfo(target.selection),
    section: capturePageSection(target.section),
  });
}

export function captureEditorTeraTarget(target: EditorTeraTarget): EditorTeraTarget {
  return Object.freeze({
    ...target,
    section: capturePageSection(target.section),
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

export function htmlTargetFromSelection(selection: SelectionInfo): EditorHtmlTarget {
  return captureEditorHtmlTarget({
    kind: "html",
    selector: selection.domPath || selection.cssSelector || "",
    tag: selection.tag,
    label: selection.selector || `<${selection.tag}>`,
    text: selection.text,
    sourceId: selection.sourceId,
    templateSourceId: selection.templateSourceId,
    sessionId: selection.sessionId,
    selection,
  });
}

export function htmlTargetFromPageSection(section: PageSection, label?: string): EditorHtmlTarget {
  return captureEditorHtmlTarget({
    kind: "html",
    selector: section.selector,
    tag: section.tag,
    label: label ?? section.label,
    sourceId: section.sourceId ?? null,
    templateSourceId: section.templateSourceId ?? null,
    sessionId: section.sessionId ?? null,
    section,
  });
}

export function teraTargetFromGate(target: {
  selector: string | null;
  sourceId: string;
  origin?: "current" | "local" | "theme" | "unknown" | null;
  themeName?: string | null;
  canSelectHtml?: boolean;
}, options: Partial<EditorTeraTarget> = {}): EditorTeraTarget {
  return captureEditorTeraTarget({
    kind: "tera",
    selector: target.selector,
    sourceId: target.sourceId,
    origin: target.origin ?? null,
    themeName: target.themeName ?? null,
    canSelectHtml: target.canSelectHtml,
    ...options,
  });
}

export function canMutateHtmlTarget(target: EditorHtmlTarget) {
  if (!target.selector) {
    return { allowed: false, reason: "Elementul nu are selector stabil." };
  }
  if (target.tag === "body" || target.tag === "html") {
    return { allowed: false, reason: "Elementul rădăcină nu poate fi modificat structural." };
  }
  return { allowed: true, reason: "" };
}
