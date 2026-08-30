import {
  isZolaTemplatePath,
  logicalTemplateName,
  projectRelativeZolaPath,
  templateNameForPath,
} from "$lib/project/files";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type {
  SourceGraphNode,
  SourceGraphTemplate,
} from "$lib/source-graph/contracts";
import type { TeraDropRequest, TeraDropResolution } from "$lib/tera/model";
import { t } from "$lib/i18n/runtime.svelte";

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
    "componentDefinition",
    "componentCall",
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
  return ["block", "componentDefinition", "componentCall", "for", "if", "raw"].includes(node.kind);
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

function componentDefinitionExists(graph: SourceGraph | null, name: string | null | undefined) {
  const normalized = (name || "").trim();
  if (!normalized || !graph) return false;
  return graph.componentGraph.definitions.some((definition) => (
    definition.kind === "teraComponent"
    && definition.active
    && definition.name === normalized
  ));
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
      reason: t("tera-drop-anchor-missing"),
    };
  }

  if (!isTemplateOwner(anchor)) {
    return {
      allowed: false,
      reason: t("tera-drop-zola-template-only"),
      anchor,
    };
  }

  if (anchor.kind === "tera") {
    return {
      allowed: false,
      reason: t("tera-drop-generic-unsafe"),
      anchor,
    };
  }

  if (request.item.kind === "extends" && request.position === "inside") {
    return {
      allowed: false,
      reason: t("tera-drop-extends-template-level"),
      anchor,
    };
  }

  if (request.item.kind === "extends" && template?.extends) {
    return {
      allowed: false,
      reason: t("tera-drop-extends-exists", {
        template: templateReferenceForFile(anchor.file),
        target: template.extends,
      }),
      anchor,
    };
  }

  if (template?.isPartial && request.item.kind === "extends") {
    return {
      allowed: false,
      reason: t("tera-drop-partial-no-extends"),
      anchor,
    };
  }

  if (template?.isPartial && request.item.kind === "block") {
    return {
      allowed: false,
      reason: t("tera-drop-partial-no-blocks"),
      anchor,
    };
  }

  if (request.item.kind === "block") {
    const name = request.item.name || "content";
    if (template?.blocks.includes(name)) {
      return {
        allowed: false,
        reason: t("tera-drop-block-exists", {
          name,
          template: templateReferenceForFile(anchor.file),
        }),
        anchor,
      };
    }
  }

  if (request.item.kind === "componentDefinition") {
    const name = request.item.name || "component";
    if (componentDefinitionExists(graph, name)) {
      return {
        allowed: false,
        reason: t("tera-drop-component-exists", {
          name,
          template: templateReferenceForFile(anchor.file),
        }),
        anchor,
      };
    }
  }

  if (
    request.item.kind === "include"
    && !templateTargetExists(graph, request.item.target)
  ) {
    return {
      allowed: false,
      reason: t("tera-drop-target-missing", {
        target: request.item.target || t("tera-drop-empty"),
      }),
      anchor,
    };
  }

  if (
    request.item.kind === "componentCall"
    && !componentDefinitionExists(graph, request.item.name || request.item.target)
  ) {
    return {
      allowed: false,
      reason: t("tera-drop-component-missing", {
        name: request.item.name || request.item.target || t("tera-drop-empty"),
      }),
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
      reason: t("tera-drop-cannot-receive-inside"),
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
    label: request.item.label,
  };
}
