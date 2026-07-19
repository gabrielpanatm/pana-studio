import { teraKindLabel } from "$lib/source-graph/context";
import {
  isTeraGateOpen,
  isTeraSourceNode,
  normalizeProjectPath,
  type TeraGateOpenContext,
} from "$lib/source-graph/interaction";
import type { SourceGraph, SourceGraphNode, TemplateWorkbenchPlan } from "$lib/types";
import type { LayerNode } from "$lib/project/pane-tree";

export type TeraLayerNode = {
  kind: "tera";
  id: string;
  selector: string;
  depth: number;
  label: string;
  kindLabel: string;
  origin: "local" | "theme" | "unknown";
  themeName: string | null;
  section: LayerNode;
  sourceNode: SourceGraphNode | null;
};

export type HtmlLayerRow = {
  kind: "html";
  node: LayerNode;
  depth: number;
  hasTeraParentRow: boolean;
};

export type LayerRow = TeraLayerNode | HtmlLayerRow;

export type BuildLayerRowsOptions = {
  sourceGraph?: SourceGraph | null;
  gateOpenContext?: TeraGateOpenContext;
  templateWorkbenchPlan?: TemplateWorkbenchPlan | null;
};

type TeraChainItem = {
  id: string;
  node: SourceGraphNode | null;
};

export function teraSourceNodeFor(
  section: LayerNode,
  sourceNodesById: Map<string, SourceGraphNode>,
): SourceGraphNode | null {
  if (!section.templateSourceId) return null;
  const node = sourceNodesById.get(section.templateSourceId) ?? null;
  if (!node || node.kind === "html") return null;
  if (isPartialTemplateLayoutGate(node)) return null;
  return node;
}

export function buildLayerRows(
  sections: LayerNode[],
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions = {},
): LayerRow[] {
  const rows: LayerRow[] = [];
  let activeTeraChain: string[] = [];
  const transparentWrapperDepths: number[] = [];

  for (let sectionIndex = 0; sectionIndex < sections.length; sectionIndex += 1) {
    const section = sections[sectionIndex];
    while (
      transparentWrapperDepths.length > 0
      && section.depth <= transparentWrapperDepths[transparentWrapperDepths.length - 1]
    ) {
      transparentWrapperDepths.pop();
    }

    if (isTransparentInheritedContentWrapper(section, sectionIndex, sections, sourceNodesById, options)) {
      transparentWrapperDepths.push(section.depth);
      continue;
    }

    const depth = Math.max(0, section.depth - transparentWrapperDepths.length);
    const teraChain = teraSourceChainFor(section, sourceNodesById, options);
    const visibleChain = visibleTeraSourceChain(teraChain, options);
    if (visibleChain.items.length === 0 && isClosedInheritedTemplateHtml(section, sourceNodesById, options)) {
      continue;
    }
    const visibleTeraIds = visibleChain.items.map((item) => item.id);
    const commonPrefix = commonPrefixLength(activeTeraChain, visibleTeraIds);

    for (let index = commonPrefix; index < visibleChain.items.length; index += 1) {
      const item = visibleChain.items[index];
      const teraNode = item.node;
      rows.push({
        kind: "tera",
        id: item.id,
        selector: section.selector,
        depth: depth + index,
        label: teraNode ? teraNode.label : "Sursă Tera",
        kindLabel: teraNode ? teraLayerKindLabel(teraNode) : "Tera",
        origin: teraNode ? teraNode.origin : "unknown",
        themeName: teraNode ? teraNode.themeName : null,
        section,
        sourceNode: teraNode,
      });
    }
    activeTeraChain = visibleTeraIds;

    if (!visibleChain.allOpen) {
      continue;
    }

    rows.push({
      kind: "html",
      node: section,
      depth: depth + teraChain.length,
      hasTeraParentRow: teraChain.length > 0,
    });
  }

  return rows;
}

function teraSourceChainFor(
  section: LayerNode,
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions = {},
): TeraChainItem[] {
  const sourceNode = section.sourceId ? (sourceNodesById.get(section.sourceId) ?? null) : null;

  const templateNode = section.templateSourceId ? (sourceNodesById.get(section.templateSourceId) ?? null) : null;

  const includeNode = includeNodeForRenderedSection(section, sourceNode, sourceNodesById, options);
  if (includeNode) {
    return includeTeraChainForRenderedSource(includeNode, sourceNode, sourceNodesById);
  }

  if (templateNode && templateNode.kind !== "html" && !isPartialTemplateLayoutGate(templateNode)) {
    return visibleTemplateChain(teraChainStartingAt(templateNode, sourceNodesById), options);
  }

  const sourceChain = sourceNode ? teraAncestorChainForSourceNode(sourceNode, sourceNodesById) : [];
  if (sourceChain.length > 0) return visibleTemplateChain(sourceChain, options);

  if (section.templateSourceId && !templateNode) return [{ id: section.templateSourceId, node: null }];
  return [];
}

function teraAncestorChainForSourceNode(
  node: SourceGraphNode,
  sourceNodesById: Map<string, SourceGraphNode>,
): TeraChainItem[] {
  if (isLayerTeraSourceNode(node)) return teraChainStartingAt(node, sourceNodesById);
  const parent = node.parent ? (sourceNodesById.get(node.parent) ?? null) : null;
  return parent ? teraChainStartingAt(parent, sourceNodesById) : [];
}

function teraChainStartingAt(
  node: SourceGraphNode,
  sourceNodesById: Map<string, SourceGraphNode>,
): TeraChainItem[] {
  const chain: TeraChainItem[] = [];
  const visited = new Set<string>();
  let current: SourceGraphNode | null = node.kind === "html"
    ? (node.parent ? (sourceNodesById.get(node.parent) ?? null) : null)
    : node;

  while (current && isLayerTeraSourceNode(current) && !visited.has(current.id)) {
    visited.add(current.id);
    if (!isPartialTemplateLayoutGate(current)) {
      chain.unshift({ id: current.id, node: current });
    }
    current = current.parent ? (sourceNodesById.get(current.parent) ?? null) : null;
  }

  return chain;
}

function isLayerTeraSourceNode(node: SourceGraphNode | null | undefined) {
  return Boolean(node && (isTeraSourceNode(node) || node.kind === "template" || node.kind === "partial"));
}

function mergeTeraChains(chain: TeraChainItem[]) {
  const merged: TeraChainItem[] = [];
  const seen = new Set<string>();
  for (const item of chain) {
    if (seen.has(item.id)) continue;
    seen.add(item.id);
    merged.push(item);
  }
  return merged;
}

function isPartialTemplateLayoutGate(node: SourceGraphNode) {
  if (node.kind !== "block" && node.kind !== "extends") return false;
  const path = normalizeProjectPath(node.file);
  const logicalPath = path.includes("/templates/")
    ? (path.split("/templates/").pop() ?? path)
    : path.replace(/^templates\//, "");
  return logicalPath.startsWith("partials/") || logicalPath.startsWith("macros/");
}

function includeNodeForRenderedSource(
  sourceNode: SourceGraphNode,
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions,
): SourceGraphNode | null {
  if (sourceNode.kind !== "html") return null;

  const includePath = includeTemplateNameForRenderedFile(sourceNode.file);
  if (!includePath) return null;

  const activeFile = normalizeProjectPath(options.gateOpenContext?.activeScannedPath);
  if (activeFile && normalizeProjectPath(sourceNode.file) === activeFile) return null;

  const includeNodes = [...sourceNodesById.values()].filter((node) =>
    node.kind === "include" && node.label.toLowerCase().includes(includePath),
  );
  if (includeNodes.length === 0) return null;

  const activeTemplateFiles = normalizedFileSet([
    options.gateOpenContext?.activeScannedPath,
    ...(options.gateOpenContext?.activeTemplateFiles ?? []),
  ]);
  return includeNodes.find((node) => activeTemplateFiles.has(normalizeProjectPath(node.file)))
    ?? includeNodes[0]
    ?? null;
}

function includeNodeForRenderedSection(
  section: LayerNode,
  sourceNode: SourceGraphNode | null,
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions,
) {
  const templateNode = section.templateSourceId ? (sourceNodesById.get(section.templateSourceId) ?? null) : null;
  if (templateNode?.kind === "include") return templateNode;
  return sourceNode ? includeNodeForRenderedSource(sourceNode, sourceNodesById, options) : null;
}

function includeTeraChainForRenderedSource(
  includeNode: SourceGraphNode,
  sourceNode: SourceGraphNode | null,
  sourceNodesById: Map<string, SourceGraphNode>,
) {
  const renderedChain = sourceNode ? teraAncestorChainForSourceNode(sourceNode, sourceNodesById) : [];
  if (renderedChain.length > 0) return mergeTeraChains(renderedChain);
  return [{ id: includeNode.id, node: includeNode }];
}

function includeTemplateNameForRenderedFile(file: string) {
  const path = normalizeProjectPath(file).replace(/^sursa\//, "").toLowerCase();
  const themed = path.match(/^themes\/[^/]+\/templates\/(.+)$/);
  if (themed) return themed[1];
  const local = path.match(/^templates\/(.+)$/);
  return local?.[1] ?? null;
}

function visibleTemplateChain(chain: TeraChainItem[], options: BuildLayerRowsOptions) {
  return chain.filter((item) => {
    const node = item.node;
    if (!node) return true;
    if (node.kind === "extends") return false;
    if (options.templateWorkbenchPlan && node.kind === "block") return false;
    if (node.kind === "template" && isRenderChainTemplateFile(node.file, options)) return false;
    if (isActiveTemplateFile(node.file, options)) {
      if (node.kind === "partial") return false;
      if (node.kind === "block" && isTransparentInheritanceBlock(node)) return false;
    }
    if (node.kind === "block" && isTransparentInheritanceBlock(node)) return false;
    return true;
  });
}

function isClosedInheritedTemplateHtml(
  section: LayerNode,
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions,
) {
  const sourceNode = section.sourceId ? (sourceNodesById.get(section.sourceId) ?? null) : null;
  if (sourceNode?.kind !== "html") return false;
  if (isActiveTemplateFile(sourceNode.file, options)) return false;
  if (isPartialTemplateFile(sourceNode.file)) return false;
  return isTemplateFile(sourceNode.file);
}

function isTransparentInheritedContentWrapper(
  section: LayerNode,
  sectionIndex: number,
  sections: LayerNode[],
  sourceNodesById: Map<string, SourceGraphNode>,
  options: BuildLayerRowsOptions,
) {
  if (!section.hasChildren) return false;
  if (section.tag.toLowerCase() !== "main") return false;

  const sourceNode = section.sourceId ? (sourceNodesById.get(section.sourceId) ?? null) : null;
  if (sourceNode?.kind !== "html") return false;
  if (isActiveTemplateFile(sourceNode.file, options)) return false;

  for (let index = sectionIndex + 1; index < sections.length; index += 1) {
    const descendant = sections[index];
    if (descendant.depth <= section.depth) break;
    const descendantSource = descendant.sourceId ? (sourceNodesById.get(descendant.sourceId) ?? null) : null;
    if (descendantSource?.kind === "html" && isActiveTemplateFile(descendantSource.file, options)) {
      return true;
    }
  }

  return false;
}

function isActiveTemplateFile(file: string | null | undefined, options: BuildLayerRowsOptions) {
  const activeFile = normalizeProjectPath(options.gateOpenContext?.activeScannedPath);
  return Boolean(activeFile && normalizeProjectPath(file) === activeFile);
}

function isRenderChainTemplateFile(file: string | null | undefined, options: BuildLayerRowsOptions) {
  const renderChainFiles = normalizedFileSet([
    options.gateOpenContext?.activeScannedPath,
    ...(options.gateOpenContext?.activeTemplateFiles ?? []),
  ]);
  return renderChainFiles.has(normalizeProjectPath(file));
}

function isTransparentInheritanceBlock(node: SourceGraphNode) {
  const label = node.label.trim().toLowerCase();
  return label === "content" || label === "body";
}

function isTemplateFile(file: string | null | undefined) {
  const normalized = normalizeProjectPath(file).replace(/^sursa\//, "");
  return normalized.startsWith("templates/") || /^themes\/[^/]+\/templates\//.test(normalized);
}

function isPartialTemplateFile(file: string | null | undefined) {
  const normalized = normalizeProjectPath(file).replace(/^sursa\//, "");
  const logical = normalized.match(/^themes\/[^/]+\/templates\/(.+)$/)?.[1]
    ?? normalized.replace(/^templates\//, "");
  return logical.startsWith("partials/") || logical.startsWith("macros/");
}

function normalizedFileSet(paths: Array<string | null | undefined>) {
  const files = new Set<string>();
  for (const path of paths) {
    const normalized = normalizeProjectPath(path);
    if (normalized) files.add(normalized);
  }
  return files;
}

function teraLayerKindLabel(node: SourceGraphNode) {
  if (node.kind === "template") return "template Tera";
  if (node.kind === "partial") return "partial Tera";
  if (node.kind === "block") return "block pagină";
  return teraKindLabel(node.kind);
}

function visibleTeraSourceChain(
  chain: TeraChainItem[],
  options: BuildLayerRowsOptions,
): { items: TeraChainItem[]; allOpen: boolean } {
  const visible: TeraChainItem[] = [];
  for (let index = 0; index < chain.length; index += 1) {
    const item = chain[index];
    visible.push(item);
    if (!item.node) return { items: visible, allOpen: false };
    if (
      !chainHasOpenDescendant(chain, index, options)
      && !isLayerTeraGateOpen(item.node, options, index > 0)
    ) {
      return { items: visible, allOpen: false };
    }
  }
  return { items: visible, allOpen: true };
}

function chainHasOpenDescendant(
  chain: TeraChainItem[],
  index: number,
  options: BuildLayerRowsOptions,
) {
  const openedSourceId = options.gateOpenContext?.openedGateSourceId;
  if (!openedSourceId) return false;
  const openNode = options.sourceGraph?.nodes.find((candidate) => candidate.id === openedSourceId) ?? null;
  return chain.slice(index + 1).some((item) =>
    item.id === openedSourceId || sourceNodeBelongsTo(openNode, item.id, options.sourceGraph ?? null),
  );
}

function isLayerTeraGateOpen(
  node: SourceGraphNode,
  options: BuildLayerRowsOptions,
  isNestedRenderedTemplate: boolean,
) {
  if (node.kind === "template" || node.kind === "partial") {
    if (isNestedRenderedTemplate) return true;
    const openNode = options.gateOpenContext?.openedGateSourceId
      ? options.sourceGraph?.nodes.find((candidate) => candidate.id === options.gateOpenContext?.openedGateSourceId)
      : null;
    if (sourceNodeBelongsTo(openNode, node.id, options.sourceGraph ?? null)) return true;

    const activeTemplateFiles = normalizedFileSet([
      options.gateOpenContext?.activeScannedPath,
      ...(options.gateOpenContext?.activeTemplateFiles ?? []),
    ]);
    return activeTemplateFiles.has(normalizeProjectPath(node.file));
  }

  return isTeraGateOpen(node, options.sourceGraph ?? null, options.gateOpenContext ?? {});
}

function sourceNodeBelongsTo(
  node: SourceGraphNode | null | undefined,
  sourceId: string | null | undefined,
  graph: SourceGraph | null,
) {
  if (!node || !sourceId) return false;
  if (node.id === sourceId) return true;

  const nodesById = new Map((graph?.nodes ?? []).map((candidate) => [candidate.id, candidate]));
  const visited = new Set<string>();
  let parentId = node.parent;
  while (parentId && !visited.has(parentId)) {
    if (parentId === sourceId) return true;
    visited.add(parentId);
    parentId = nodesById.get(parentId)?.parent ?? null;
  }
  return false;
}

function commonPrefixLength(left: string[], right: string[]) {
  const length = Math.min(left.length, right.length);
  let index = 0;
  while (index < length && left[index] === right[index]) index += 1;
  return index;
}
