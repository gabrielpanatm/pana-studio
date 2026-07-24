import { isZolaTemplatePath, projectRelativeZolaPath } from "$lib/project/files";
import { createDomPathSelector } from "$lib/preview/selection";
import { canRequestTemplateEditGateKind } from "$lib/tera/template-edit-gate";
import type { PageSection, SelectionInfo, SourceGraph, SourceGraphNode } from "$lib/types";
import {
  isTeraGateOpen,
  isTeraSourceNode,
  nearestTeraSourceNode,
  sourceNodeBelongsToOpenTeraGate,
  sourceNodeBelongsToSource,
  sourceNodeById,
} from "$lib/source-graph/interaction";
import {
  includeTemplateNameForRenderedFile,
  normalizedProjectPath,
  templateOriginKind,
  type PreviewTemplateGate,
} from "$lib/state/app-helpers";
import type { SourceNodeKind } from "$lib/types";

export type TemplateGateContext = {
  sourceGraph: SourceGraph | null;
  activeScannedPath: string | null;
  templateHtmlEditSourceId: string | null;
  activeTemplateFiles: string[];
  previewDocument: Document | null;
};

export type PreviewTemplateGateWithElement = PreviewTemplateGate & { element: Element };

const VISUAL_TERA_GATE_KINDS = new Set<SourceNodeKind>([
  "include",
  "for",
  "if",
  "macro",
]);
const PRIMARY_VISUAL_TERA_GATE_KINDS = new Set<SourceNodeKind>([
  "for",
  "include",
  "macro",
]);
const TEMPLATE_SOURCE_ID_ATTR = "data-pana-template-source-id";
const TEMPLATE_SOURCE_STACK_ATTR = "data-pana-template-source-stack";

export function templateGateForSelection(
  selection: SelectionInfo,
  context: TemplateGateContext,
): PreviewTemplateGate | null {
  const sourceNode = sourceNodeById(context.sourceGraph, selection.sourceId);
  if (sourceNode && isTeraSourceNode(sourceNode)) {
    return {
      selector: selection.domPath,
      sourceId: sourceNode.id,
      origin: templateOriginKind(sourceNode, context.activeScannedPath),
      themeName: sourceNode.themeName,
      canSelectHtml: canRequestTemplateEditGateKind(sourceNode.kind),
    };
  }

  if (sourceNode?.kind === "html") {
    const sourceId = externalPreviewGateSourceId(sourceNode, context, selection.templateSourceId);
    if (!sourceId || sourceId === context.templateHtmlEditSourceId) return null;
    return {
      selector:
        templateGateRootSelector(selection.domPath, {
          sourceNode,
          templateSourceId: sourceId,
        }, context) ?? selection.domPath,
      sourceId,
      origin: templateOriginKind(sourceNode, context.activeScannedPath),
      themeName: sourceNode.themeName,
      canSelectHtml: sourceNode.capabilities.canEditVisual,
    };
  }

  const templateNode = sourceNodeById(context.sourceGraph, selection.templateSourceId);
  if (!templateNode) return null;
  const sourceId = externalTemplateGateSourceId(templateNode, context);
  if (!sourceId || sourceId === context.templateHtmlEditSourceId) return null;

  return {
    selector: templateGateRootSelector(selection.domPath, { templateSourceId: templateNode.id }, context) ?? selection.domPath,
    sourceId,
    origin: templateOriginKind(templateNode, context.activeScannedPath),
    themeName: templateNode.themeName,
    canSelectHtml: canRequestTemplateEditGateKind(templateNode.kind),
  };
}

export function templateGateForPageSection(
  section: PageSection,
  context: TemplateGateContext,
): PreviewTemplateGate | null {
  const sourceNode = sourceNodeById(context.sourceGraph, section.sourceId);
  if (sourceNode?.kind === "html") {
    const sourceId = externalPreviewGateSourceId(sourceNode, context, section.templateSourceId);
    if (!sourceId || sourceId === context.templateHtmlEditSourceId) return null;
    return {
      selector:
        templateGateRootSelector(section.selector, {
          sourceNode,
          templateSourceId: sourceId,
        }, context) ?? section.selector,
      sourceId,
      origin: templateOriginKind(sourceNode, context.activeScannedPath),
      themeName: sourceNode.themeName,
      canSelectHtml: sourceNode.capabilities.canEditVisual,
    };
  }

  const templateNode = sourceNodeById(context.sourceGraph, section.templateSourceId);
  if (!templateNode) return null;
  const sourceId = externalTemplateGateSourceId(templateNode, context);
  if (!sourceId || sourceId === context.templateHtmlEditSourceId) return null;

  return {
    selector: templateGateRootSelector(section.selector, { templateSourceId: templateNode.id }, context) ?? section.selector,
    sourceId,
    origin: templateOriginKind(templateNode, context.activeScannedPath),
    themeName: templateNode.themeName,
    canSelectHtml: canRequestTemplateEditGateKind(templateNode.kind),
  };
}

export function templateGateForPreviewClick(
  element: Element,
  context: TemplateGateContext,
): PreviewTemplateGateWithElement | null {
  const emptyTeraSlot = element.closest("[data-pana-empty-tera-slot]");
  const emptyTeraSourceId = emptyTeraSlot?.getAttribute("data-pana-empty-tera-slot") ?? null;
  const emptyTeraNode = sourceNodeById(context.sourceGraph, emptyTeraSourceId);
  if (emptyTeraSlot && emptyTeraNode && isTeraSourceNode(emptyTeraNode)) {
    return {
      element: emptyTeraSlot,
      selector: createDomPathSelector(emptyTeraSlot),
      sourceId: emptyTeraNode.id,
      origin: templateOriginKind(emptyTeraNode, context.activeScannedPath),
      themeName: emptyTeraNode.themeName,
      canSelectHtml: canRequestTemplateEditGateKind(emptyTeraNode.kind),
    };
  }

  const templateElement = element.closest("[data-pana-template-source-id]");
  const templateSourceId = templateElement?.getAttribute("data-pana-template-source-id") ?? null;
  const sourceElement = element.closest("[data-pana-source-id]");
  const sourceId = sourceElement?.getAttribute("data-pana-source-id") ?? null;
  const sourceNode = sourceNodeById(context.sourceGraph, sourceId);

  if (sourceNode?.kind === "html") {
    const gateSourceId = externalPreviewGateSourceId(sourceNode, context, templateSourceId);
    if (!gateSourceId || gateSourceId === context.templateHtmlEditSourceId) return null;
    const gateElement = templateGateRootElement(sourceElement ?? element, {
      sourceNode,
      templateSourceId: gateSourceId,
    }, context);
    return {
      element: gateElement,
      selector: createDomPathSelector(gateElement),
      sourceId: gateSourceId,
      origin: templateOriginKind(sourceNode, context.activeScannedPath),
      themeName: sourceNode.themeName,
      canSelectHtml: sourceNode.capabilities.canEditVisual,
    };
  }

  const templateNode = sourceNodeById(context.sourceGraph, templateSourceId);
  if (!templateNode) return null;
  const gateSourceId = externalTemplateGateSourceId(templateNode, context);
  if (!gateSourceId || gateSourceId === context.templateHtmlEditSourceId) return null;
  const gateElement = templateGateRootElement(templateElement ?? element, { templateSourceId: templateNode.id }, context);

  return {
    element: gateElement,
    selector: createDomPathSelector(gateElement),
    sourceId: gateSourceId,
    origin: templateOriginKind(templateNode, context.activeScannedPath),
    themeName: templateNode.themeName,
    canSelectHtml: canRequestTemplateEditGateKind(templateNode.kind)
      && (renderedHtmlNodeForElement(gateElement, context)?.capabilities.canEditVisual ?? true),
  };
}

export function templateGateForTeraSource(
  sourceId: string | null | undefined,
  selector: string | null | undefined,
  context: TemplateGateContext,
): PreviewTemplateGate | null {
  const sourceNode = sourceNodeById(context.sourceGraph, sourceId);
  const teraNode = isTemplateTeraSourceNode(sourceNode)
    ? sourceNode
    : nearestTeraSourceNode(context.sourceGraph, sourceNode, sourceNode);
  if (!teraNode || !isTemplateTeraSourceNode(teraNode) || !selector) return null;

  const renderedHtmlNode = renderedHtmlNodeForSelector(selector, context);
  return {
    selector: templateGateRootSelector(selector, { templateSourceId: teraNode.id }, context) ?? selector,
    sourceId: teraNode.id,
    origin: templateOriginKind(teraNode, context.activeScannedPath),
    themeName: teraNode.themeName,
    canSelectHtml: canRequestTemplateEditGateKind(teraNode.kind)
      && (renderedHtmlNode?.capabilities.canEditVisual ?? true),
  };
}

function isTemplateTeraSourceNode(node: SourceGraphNode | null | undefined) {
  return Boolean(node && (isTeraSourceNode(node) || node.kind === "template" || node.kind === "partial"));
}

export function templateGateSourceIdForSelection(
  selection: SelectionInfo,
  context: TemplateGateContext,
) {
  if (selectionBelongsToGate(selection, context.templateHtmlEditSourceId, context.sourceGraph)) {
    return context.templateHtmlEditSourceId;
  }

  const sourceNode = sourceNodeById(context.sourceGraph, selection.sourceId);
  if (sourceNode?.kind === "html") {
    return externalPreviewGateSourceId(sourceNode, context, selection.templateSourceId);
  }
  const templateNode = sourceNodeById(context.sourceGraph, selection.templateSourceId);
  return templateNode ? externalTemplateGateSourceId(templateNode, context) : null;
}

export function selectionBelongsToGate(
  selection: Pick<SelectionInfo, "sourceId" | "templateSourceId">,
  gateSourceId: string | null | undefined,
  graph: SourceGraph | null,
) {
  if (!gateSourceId) return false;
  const sourceNode = sourceNodeById(graph, selection.sourceId);
  const templateNode = sourceNodeById(graph, selection.templateSourceId);
  return sourceNodeBelongsToSource(sourceNode, gateSourceId, graph)
    || sourceNodeBelongsToSource(templateNode, gateSourceId, graph);
}

export function templateGateFromBridgeData(
  data: Record<string, unknown>,
  context: TemplateGateContext,
): PreviewTemplateGate | null {
  const sourceId = typeof data.sourceId === "string" ? data.sourceId : null;
  const selector = typeof data.selector === "string" ? data.selector : null;
  if (!sourceId || !selector) return null;

  const node = sourceNodeById(context.sourceGraph, sourceId);
  const nodeAllowsHtml = node ? canRequestTemplateEditGateKind(node.kind) : true;
  return {
    selector,
    sourceId,
    origin: templateOriginKind(node, context.activeScannedPath),
    themeName: node?.themeName ?? null,
    canSelectHtml: data.canSelectHtml !== false && nodeAllowsHtml,
  };
}

export function includeNodeIdForRenderedSource(
  renderedFile: string,
  context: Pick<TemplateGateContext, "sourceGraph" | "activeScannedPath">,
) {
  const includePath = includeTemplateNameForRenderedFile(renderedFile);
  if (!includePath) return null;
  const normalizedIncludePath = includePath.toLowerCase();
  const active = context.activeScannedPath ? projectRelativeZolaPath(context.activeScannedPath) : "";
  const includeNodes = context.sourceGraph?.nodes.filter((node) => node.kind === "include") ?? [];
  const activeOwnerMatch = includeNodes.find((node) =>
    projectRelativeZolaPath(node.file) === active &&
    node.label.toLowerCase().includes(normalizedIncludePath),
  );
  if (activeOwnerMatch) return activeOwnerMatch.id;
  return includeNodes.find((node) => node.label.toLowerCase().includes(normalizedIncludePath))?.id ?? null;
}

function externalPreviewGateSourceId(
  node: SourceGraphNode,
  context: TemplateGateContext,
  templateSourceId: string | null | undefined = null,
) {
  const activeTemplateGateId = activeTemplateVisualGateSourceId(node, context, templateSourceId);
  if (activeTemplateGateId) return activeTemplateGateId;
  if (sourceNodeBelongsToOpenTeraGate(node, context.sourceGraph, gateOpenContext(context))) return null;
  const active = normalizedProjectPath(context.activeScannedPath);
  const owner = normalizedProjectPath(node.file);
  if (!active || owner === active || !isZolaTemplatePath(owner)) return null;
  const templateNode = sourceNodeById(context.sourceGraph, templateSourceId);
  const renderedTemplateNode = renderedTemplateNodeForFile(node.file, context.sourceGraph);
  if (templateNode?.kind === "include" && renderedTemplateNode) return renderedTemplateNode.id;
  if (templateNode && isTeraSourceNode(templateNode)) return templateNode.id;
  return renderedTemplateNode?.id ?? includeNodeIdForRenderedSource(node.file, context) ?? node.id;
}

function externalTemplateGateSourceId(node: SourceGraphNode, context: TemplateGateContext) {
  const activeTemplateGateId = activeTemplateVisualGateSourceId(node, context, node.id);
  if (activeTemplateGateId === node.id) return node.id;
  if (sourceNodeBelongsToOpenTeraGate(node, context.sourceGraph, gateOpenContext(context))) return null;
  const active = normalizedProjectPath(context.activeScannedPath);
  const owner = normalizedProjectPath(node.file);
  if (!active || owner === active || !isZolaTemplatePath(owner)) return null;
  return node.id;
}

function activeTemplateVisualGateSourceId(
  node: SourceGraphNode,
  context: TemplateGateContext,
  templateSourceId: string | null | undefined,
) {
  const active = normalizedProjectPath(context.activeScannedPath);
  const owner = normalizedProjectPath(node.file);
  if (!active || owner !== active) return null;

  const structuralGate = closedVisualTeraGateForNode(node, context);
  if (structuralGate) return structuralGate.id;

  const templateNode = sourceNodeById(context.sourceGraph, templateSourceId);
  if (
    templateNode
    && normalizedProjectPath(templateNode.file) === active
    && isClosedVisualTeraGate(templateNode, context)
  ) {
    return templateNode.id;
  }

  return null;
}

function closedVisualTeraGateForNode(
  node: SourceGraphNode,
  context: TemplateGateContext,
) {
  if (!context.sourceGraph) return null;
  const nodesById = new Map(context.sourceGraph.nodes.map((candidate) => [candidate.id, candidate]));
  const visited = new Set<string>();
  const closedGates: SourceGraphNode[] = [];
  let current: SourceGraphNode | null = node;

  while (current && !visited.has(current.id)) {
    if (isClosedVisualTeraGate(current, context)) closedGates.push(current);
    visited.add(current.id);
    current = current.parent ? (nodesById.get(current.parent) ?? null) : null;
  }

  return closedGates.find((candidate) => candidate.kind === "for")
    ?? closedGates.find((candidate) => PRIMARY_VISUAL_TERA_GATE_KINDS.has(candidate.kind))
    ?? closedGates[0]
    ?? null;
}

function isClosedVisualTeraGate(
  node: SourceGraphNode | null | undefined,
  context: TemplateGateContext,
) {
  if (!node || !VISUAL_TERA_GATE_KINDS.has(node.kind)) return false;
  return !isTeraGateOpen(node, context.sourceGraph, gateOpenContext(context));
}

function gateOpenContext(context: TemplateGateContext) {
  return {
    openedGateSourceId: context.templateHtmlEditSourceId,
    activeScannedPath: context.activeScannedPath,
    activeTemplateFiles: context.activeTemplateFiles,
  };
}

function templateGateRootSelector(
  selector: string,
  options: { sourceNode?: SourceGraphNode; templateSourceId?: string },
  context: TemplateGateContext,
) {
  const element = context.previewDocument?.querySelector(selector);
  if (!element) return selector;
  return createDomPathSelector(templateGateRootElement(element, options, context));
}

function templateGateRootElement(
  element: Element,
  options: { sourceNode?: SourceGraphNode; templateSourceId?: string },
  context: TemplateGateContext,
) {
  const sourceFile = options.sourceNode ? normalizedProjectPath(options.sourceNode.file) : null;
  const templateSourceId = options.templateSourceId;
  let root = element;
  let current = element.parentElement;

  if (templateSourceId && elementHasTemplateSource(element, templateSourceId)) {
    while (
      current
      && current !== current.ownerDocument.body
      && current !== current.ownerDocument.documentElement
      && elementHasTemplateSource(current, templateSourceId)
    ) {
      root = current;
      current = current.parentElement;
    }
    return root;
  }

  const semanticRoot = templateGateSemanticRootElement(element);
  if (semanticRoot) return semanticRoot;

  while (current && current !== current.ownerDocument.body && current !== current.ownerDocument.documentElement) {
    const sameSource = !templateSourceId && sourceFile && elementSourceFile(current, context.sourceGraph) === sourceFile;
    const sameTemplate = templateSourceId && elementHasTemplateSource(current, templateSourceId);
    if (!sameSource && !sameTemplate) break;
    root = current;
    current = current.parentElement;
  }

  return root;
}

function elementHasTemplateSource(element: Element, sourceId: string) {
  if (element.getAttribute(TEMPLATE_SOURCE_ID_ATTR) === sourceId) return true;
  return (element.getAttribute(TEMPLATE_SOURCE_STACK_ATTR) ?? "")
    .split(/\s+/)
    .includes(sourceId);
}

function templateGateSemanticRootElement(element: Element) {
  let fallback: Element | null = null;
  let current: Element | null = element;

  while (current && current !== current.ownerDocument.body && current !== current.ownerDocument.documentElement) {
    const tag = current.tagName.toLowerCase();
    if (tag === "header" || tag === "footer" || tag === "section" || tag === "article" || tag === "aside") {
      return current;
    }
    if (!fallback && (tag === "main" || tag === "nav")) {
      fallback = current;
    }
    current = current.parentElement;
  }

  return fallback;
}

function elementSourceFile(element: Element, graph: SourceGraph | null) {
  const sourceId = element.getAttribute("data-pana-source-id");
  const sourceNode = sourceNodeById(graph, sourceId);
  return sourceNode ? normalizedProjectPath(sourceNode.file) : null;
}

function renderedHtmlNodeForSelector(selector: string, context: TemplateGateContext) {
  const element = context.previewDocument?.querySelector(selector);
  return element ? renderedHtmlNodeForElement(element, context) : null;
}

function renderedHtmlNodeForElement(element: Element, context: TemplateGateContext) {
  const sourceElement = element.closest("[data-pana-source-id]");
  const sourceId = sourceElement?.getAttribute("data-pana-source-id") ?? null;
  const sourceNode = sourceNodeById(context.sourceGraph, sourceId);
  return sourceNode?.kind === "html" ? sourceNode : null;
}

function renderedTemplateNodeForFile(
  file: string,
  graph: SourceGraph | null,
): SourceGraphNode | null {
  const normalizedFile = normalizedProjectPath(file);
  return graph?.nodes.find((node) =>
    (node.kind === "template" || node.kind === "partial")
    && normalizedProjectPath(node.file) === normalizedFile,
  ) ?? null;
}
