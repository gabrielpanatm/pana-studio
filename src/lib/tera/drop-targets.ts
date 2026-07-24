import {
  isZolaTemplatePath,
  logicalTemplateName,
  projectRelativeZolaPath,
  templateNameForPath,
} from "$lib/project/files";
import type { SourceGraph, SourceGraphNode, SourceGraphTemplate } from "$lib/types";
import type { TeraDropRequest, TeraDropResolution } from "$lib/tera/model";
import { teraSnippetForItem } from "$lib/tera/palette";

function nodeById(graph: SourceGraph | null, id: string | null | undefined) {
  if (!id) return null;
  return graph?.nodes.find((node) => node.id === id) ?? null;
}

function ownerPath(node: SourceGraphNode) {
  return projectRelativeZolaPath(node.file);
}

function normalizeTemplateReference(value: string | null | undefined) {
  return logicalTemplateName((value || "").trim().replace(/^["']|["']$/g, ""));
}

function templateReferenceForFile(file: string) {
  return templateNameForPath(file);
}

function templateForNode(graph: SourceGraph | null, node: SourceGraphNode | null) {
  if (!graph || !node) return null;
  const owner = projectRelativeZolaPath(node.file);
  return graph.templates.find((template) => projectRelativeZolaPath(template.file) === owner) ?? null;
}

function isTemplateOwner(node: SourceGraphNode) {
  return isZolaTemplatePath(ownerPath(node));
}

function isStructuralTeraNode(node: SourceGraphNode) {
  return [
    "extends",
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
    "tera",
  ].includes(node.kind);
}

function isBodyTeraNode(node: SourceGraphNode) {
  return ["block", "macro", "for", "if", "raw"].includes(node.kind);
}

function canReceiveTeraInsideTag(tag: string) {
  return [
    "body",
    "main",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "aside",
    "div",
    "ul",
    "ol",
    "li",
    "form",
    "fieldset",
    "figure",
    "template",
  ].includes(tag.toLowerCase());
}

function preferredAnchor(
  request: TeraDropRequest,
  sourceNode: SourceGraphNode | null,
  templateNode: SourceGraphNode | null,
) {
  // data-pana-template-source-id is the Tera context rendered around an HTML element
  // (for example the surrounding `{% block content %}`). When the pointer is over
  // a real HTML source node, the user's spatial intent is that HTML node, not the
  // enclosing Tera scope.
  if (sourceNode?.kind === "html") return sourceNode;
  if (request.position === "inside") return sourceNode ?? templateNode;
  if (!sourceNode && templateNode && isStructuralTeraNode(templateNode)) return templateNode;
  return sourceNode ?? templateNode;
}

function templateTargetExists(graph: SourceGraph | null, target: string | null | undefined) {
  const normalized = normalizeTemplateReference(target);
  if (!normalized || !graph) return false;
  return graph.templates.some((template) => normalizeTemplateReference(template.name) === normalized);
}

function validateTeraDrop(
  graph: SourceGraph | null,
  request: TeraDropRequest,
  anchor: SourceGraphNode | null,
  template: SourceGraphTemplate | null,
): TeraDropResolution | null {
  if (!anchor) {
    return {
      allowed: false,
      reason: "Nu există o ancoră Source Graph stabilă pentru drop-ul Tera.",
    };
  }

  if (!anchor.range) {
    return {
      allowed: false,
      reason: "Ancora aleasă nu are range de sursă suficient pentru inserare source-preserving.",
      anchor,
    };
  }

  if (!isTemplateOwner(anchor)) {
    return {
      allowed: false,
      reason: "Tera poate fi inserat doar în template-uri Zola.",
      anchor,
    };
  }

  if (anchor.kind === "tera") {
    return {
      allowed: false,
      reason: "Sintaxa Tera nespecializată nu este o ancoră sigură pentru inserare vizuală.",
      anchor,
    };
  }

  if (request.item.kind === "extends" && request.position === "inside") {
    return {
      allowed: false,
      reason: "Extends trebuie inserat la nivel de template, nu în interiorul unui element HTML.",
      anchor,
    };
  }

  if (request.item.kind === "extends" && template?.extends) {
    return {
      allowed: false,
      reason: `Template-ul ${templateReferenceForFile(anchor.file)} are deja extends către ${template.extends}.`,
      anchor,
    };
  }

  if (template?.isPartial && request.item.kind === "extends") {
    return {
      allowed: false,
      reason: "Partialurile nu folosesc extends. Creează un template de pagină/layout pentru extends.",
      anchor,
    };
  }

  if (template?.isPartial && request.item.kind === "block") {
    return {
      allowed: false,
      reason: "Partialurile nu definesc block-uri Tera. Pune HTML-ul direct în partial și include partialul în pagina dorită.",
      anchor,
    };
  }

  if (request.item.kind === "block") {
    const name = request.item.name || "content";
    if (template?.blocks.includes(name)) {
      return {
        allowed: false,
        reason: `Block-ul ${name} există deja în ${templateReferenceForFile(anchor.file)}.`,
        anchor,
      };
    }
  }

  if (request.item.kind === "macro") {
    const name = request.item.name || "componenta";
    if (template?.macros.includes(name)) {
      return {
        allowed: false,
        reason: `Macro-ul ${name} există deja în ${templateReferenceForFile(anchor.file)}.`,
        anchor,
      };
    }
  }

  if ((request.item.kind === "include" || request.item.kind === "import") && !templateTargetExists(graph, request.item.target)) {
    return {
      allowed: false,
      reason: `Template-ul țintă nu există încă: ${request.item.target || "(gol)"}. Creează fișierul sau alege un partial existent.`,
      anchor,
    };
  }

  if (request.item.kind === "import" && request.position === "inside") {
    return {
      allowed: false,
      reason: "Importurile Tera se inserează la nivel de template, înainte sau după o ancoră stabilă.",
      anchor,
    };
  }

  if (
    request.position === "inside" &&
    !canReceiveTeraInsideTag(request.targetTag) &&
    !isBodyTeraNode(anchor)
  ) {
    return {
      allowed: false,
      reason: "Acest element nu poate primi conținut Tera în interior. Alege înainte sau după element.",
      anchor,
    };
  }

  return null;
}

export function resolveTeraDropTarget(graph: SourceGraph | null, request: TeraDropRequest): TeraDropResolution {
  const sourceNode = nodeById(graph, request.targetSourceId);
  const templateNode = nodeById(graph, request.targetTemplateSourceId);
  const anchor = preferredAnchor(request, sourceNode, templateNode);
  const ownerTemplate = templateForNode(graph, anchor);
  const blocked = validateTeraDrop(graph, request, anchor, ownerTemplate);
  if (blocked) return blocked;

  return {
    allowed: true,
    anchor: anchor!,
    position: request.position,
    item: request.item,
    snippet: teraSnippetForItem(request.item),
    label: request.item.label,
  };
}
