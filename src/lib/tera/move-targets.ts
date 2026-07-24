import { canLayerReceiveChildren } from "$lib/project/layers-drag";
import { isZolaTemplatePath, projectRelativeZolaPath } from "$lib/project/files";
import { isTeraSourceNode, normalizeProjectPath, sourceNodeById } from "$lib/source-graph/interaction";
import type { TeraMoveRequest } from "$lib/tera/model";
import type { SourceGraph, SourceGraphNode } from "$lib/types";

export type TeraMoveContext = {
  activeScannedPath?: string | null;
  activeTemplatePath?: string | null;
};

export type TeraMoveResolution =
  | {
      allowed: true;
      source: SourceGraphNode;
      anchor: SourceGraphNode;
      sourceFile: string;
      targetFile: string;
      position: TeraMoveRequest["position"];
      direction: "same-template";
      label: string;
    }
  | {
      allowed: false;
      reason: string;
      source?: SourceGraphNode | null;
      anchor?: SourceGraphNode | null;
    };

const BODY_TERA_KINDS = new Set(["block", "macro", "for", "if", "raw"]);
const MOVABLE_TERA_KINDS = new Set([
  "block",
  "include",
  "import",
  "macro",
  "for",
  "if",
  "set",
  "teraVariable",
  "teraComment",
  "raw",
]);

export function resolveTeraMoveTarget(
  graph: SourceGraph | null,
  request: TeraMoveRequest,
  context: TeraMoveContext,
): TeraMoveResolution {
  const source = sourceNodeById(graph, request.sourceId);
  const targetSource = sourceNodeById(graph, request.targetSourceId);
  const targetTemplate = sourceNodeById(graph, request.targetTemplateSourceId);
  const anchor = preferredMoveAnchor(request, targetSource, targetTemplate);

  if (!source || !isTeraSourceNode(source)) {
    return { allowed: false, reason: "Selectează un nod Tera mutabil.", source, anchor };
  }
  if (source.kind === "tera") {
    return {
      allowed: false,
      reason: "Sintaxa Tera nespecializată se mută din cod sau printr-o acțiune dedicată, nu prin drag and drop vizual.",
      source,
      anchor,
    };
  }
  if (!MOVABLE_TERA_KINDS.has(source.kind)) {
    return { allowed: false, reason: "Selectează un nod Tera mutabil.", source, anchor };
  }
  if (!source.range) {
    return { allowed: false, reason: "Nodul Tera nu are range de sursă stabil pentru mutare.", source, anchor };
  }
  if (anchor?.kind === "tera") {
    return {
      allowed: false,
      reason: "Sintaxa Tera nespecializată nu este o destinație sigură pentru mutare vizuală.",
      source,
      anchor,
    };
  }
  if (!anchor || !anchor.range) {
    return { allowed: false, reason: "Alege o destinație cu ancoră Source Graph stabilă.", source, anchor };
  }
  if (source.id === anchor.id) {
    return { allowed: false, reason: "Nodul Tera este deja pe această țintă.", source, anchor };
  }
  if (!isZolaTemplatePath(projectRelativeZolaPath(source.file)) || !isZolaTemplatePath(projectRelativeZolaPath(anchor.file))) {
    return { allowed: false, reason: "Mutarea Tera este disponibilă doar în template-uri Zola.", source, anchor };
  }
  if (request.position === "inside" && !canReceiveTeraInside(request, anchor)) {
    return { allowed: false, reason: "Această destinație nu poate primi Tera în interior.", source, anchor };
  }

  const sourceFile = normalizeProjectPath(source.file);
  const targetFile = normalizeProjectPath(anchor.file);
  if (sourceFile === targetFile && source.range && anchor.range && rangesOverlap(source.range, anchor.range)) {
    return {
      allowed: false,
      reason: "Nu poți muta un nod Tera relativ la propriul conținut sau la propriul părinte.",
      source,
      anchor,
    };
  }

  const currentTemplateFile = currentEditableTemplateFile(context.activeTemplatePath ?? context.activeScannedPath);
  if (!currentTemplateFile) {
    return { allowed: false, reason: "Deschide template-ul curent înainte să muți noduri Tera.", source, anchor };
  }

  if (sourceFile !== currentTemplateFile) {
    return {
      allowed: false,
      reason: "Nodul Tera aparține altui template. Deschide acel template ca sursă activă înainte să îl muți.",
      source,
      anchor,
    };
  }

  if (targetFile !== currentTemplateFile) {
    return {
      allowed: false,
      reason: "Destinația aparține altui template. Mutarea Tera prin drag and drop rămâne în template-ul activ.",
      source,
      anchor,
    };
  }

  if (sourceFile !== targetFile) {
    return {
      allowed: false,
      reason: "Mutarea Tera între fișiere diferite trebuie făcută printr-o acțiune explicită, nu prin DnD.",
      source,
      anchor,
    };
  }

  return {
    allowed: true,
    source,
    anchor,
    sourceFile,
    targetFile,
    position: request.position,
    direction: "same-template",
    label: source.label,
  };
}

function preferredMoveAnchor(
  request: TeraMoveRequest,
  sourceNode: SourceGraphNode | null,
  templateNode: SourceGraphNode | null,
) {
  if (request.targetKind === "tera") return templateNode ?? sourceNode;
  if (sourceNode?.kind === "html") return sourceNode;
  return templateNode ?? sourceNode;
}

function canReceiveTeraInside(request: TeraMoveRequest, anchor: SourceGraphNode) {
  if (anchor.kind === "html") return canLayerReceiveChildren(request.targetTag);
  return BODY_TERA_KINDS.has(anchor.kind);
}

function currentEditableTemplateFile(activeScannedPath: string | null | undefined) {
  const active = normalizeProjectPath(activeScannedPath);
  return active && isZolaTemplatePath(active) ? active : null;
}

function rangesOverlap(left: { start: number; end: number }, right: { start: number; end: number }) {
  return left.start < right.end && right.start < left.end;
}
