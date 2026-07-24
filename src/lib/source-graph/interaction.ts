import { projectRelativeZolaPath } from "$lib/project/files";
import type { SourceGraph, SourceGraphNode, SourceNodeKind } from "$lib/types";

export type SourceInteractionOrigin = "current" | "local" | "theme" | "unknown";

export const TERA_SOURCE_KINDS = new Set<SourceNodeKind>([
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
]);

export function isTeraSourceKind(kind: SourceNodeKind | null | undefined): boolean {
  return Boolean(kind && TERA_SOURCE_KINDS.has(kind));
}

export function isTeraSourceNode(node: SourceGraphNode | null | undefined): boolean {
  return Boolean(node && isTeraSourceKind(node.kind));
}

export function sourceNodeById(
  graph: SourceGraph | null,
  sourceId: string | null | undefined,
): SourceGraphNode | null {
  if (!graph || !sourceId) return null;
  return graph.nodes.find((node) => node.id === sourceId) ?? null;
}

export function sourceInteractionOrigin(
  node: SourceGraphNode | null | undefined,
  activeScannedPath: string | null | undefined,
): SourceInteractionOrigin {
  if (!node) return "unknown";
  const active = normalizeProjectPath(activeScannedPath);
  const owner = normalizeProjectPath(node.file);
  if (active && owner === active) return "current";
  return node.origin === "theme" ? "theme" : "local";
}

export function nearestTeraSourceNode(
  graph: SourceGraph | null,
  node: SourceGraphNode | null | undefined,
  fallbackNode: SourceGraphNode | null | undefined = null,
): SourceGraphNode | null {
  if (isTeraSourceNode(fallbackNode)) return fallbackNode ?? null;
  if (isTeraSourceNode(node)) return node ?? null;
  if (!graph || !node) return null;

  const nodesById = new Map(graph.nodes.map((candidate) => [candidate.id, candidate]));
  const visited = new Set<string>();
  let parentId = node.parent;

  while (parentId && !visited.has(parentId)) {
    visited.add(parentId);
    const parent = nodesById.get(parentId);
    if (!parent) return null;
    if (isTeraSourceNode(parent)) return parent;
    parentId = parent.parent;
  }

  return null;
}

export type TeraGateOpenContext = {
  openedGateSourceId?: string | null;
  activeScannedPath?: string | null;
  activeTemplateFiles?: Array<string | null | undefined>;
};

export type TeraGateDropTarget = {
  targetSourceId?: string | null;
  targetTemplateSourceId?: string | null;
};

export type TeraGateDropVerdict = {
  allowed: boolean;
  message?: string;
};

export const TERA_GATE_DROP_BLOCKED_MESSAGE = "Deschide gate-ul Tera înainte de drop.";

export function activeTemplateFilesForContext(
  graph: SourceGraph | null,
  activeScannedPath: string | null | undefined,
  selectedSourceFile: string | null | undefined = null,
) {
  const files = new Set<string>();
  addNormalizedFile(files, activeScannedPath);
  addNormalizedFile(files, selectedSourceFile);

  const active = normalizeProjectPath(activeScannedPath);
  const page = graph?.pages.find((candidate) => normalizeProjectPath(candidate.file) === active) ?? null;
  if (page?.templateNodeId) {
    for (const file of templateChainFiles(graph, page.templateNodeId)) {
      addNormalizedFile(files, file);
    }
  } else {
    const template = graph?.templates.find((candidate) => normalizeProjectPath(candidate.file) === active) ?? null;
    if (template?.nodeId) {
      for (const file of templateChainFiles(graph, template.nodeId)) {
        addNormalizedFile(files, file);
      }
    }
  }

  return [...files];
}

export function sourceNodeBelongsToSource(
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

export function isTeraGateOpen(
  node: SourceGraphNode | null | undefined,
  graph: SourceGraph | null,
  context: TeraGateOpenContext,
) {
  if (!node || !isTeraSourceNode(node)) return false;
  if (sourceNodeBelongsToSource(node, context.openedGateSourceId, graph)) return true;
  if (node.kind !== "block") return false;

  const activeTemplateFile = normalizeProjectPath(context.activeScannedPath);
  return Boolean(activeTemplateFile && normalizeProjectPath(node.file) === activeTemplateFile);
}

export function sourceNodeBelongsToOpenTeraGate(
  node: SourceGraphNode | null | undefined,
  graph: SourceGraph | null,
  context: TeraGateOpenContext,
) {
  if (!node) return false;
  if (sourceNodeBelongsToSource(node, context.openedGateSourceId, graph)) return true;

  const nodesById = new Map((graph?.nodes ?? []).map((candidate) => [candidate.id, candidate]));
  const visited = new Set<string>();
  let current: SourceGraphNode | null = node;

  while (current && !visited.has(current.id)) {
    if (isTeraGateOpen(current, graph, context)) return true;
    visited.add(current.id);
    current = current.parent ? (nodesById.get(current.parent) ?? null) : null;
  }

  return false;
}

export function openTeraGateSourceIds(
  graph: SourceGraph | null,
  context: TeraGateOpenContext,
) {
  if (!graph) return [];
  return graph.nodes
    .filter((node) => isTeraGateOpen(node, graph, context))
    .map((node) => node.id);
}

export function teraGateDropStatus(
  graph: SourceGraph | null,
  context: TeraGateOpenContext,
  target: TeraGateDropTarget,
): TeraGateDropVerdict {
  const openSourceIds = new Set(openTeraGateSourceIds(graph, context));
  const templateSourceId = target.targetTemplateSourceId ?? null;
  if (templateSourceId) {
    if (templateSourceId === context.openedGateSourceId) return { allowed: true };
    if (openSourceIds.has(templateSourceId)) return { allowed: true };
    return { allowed: false, message: TERA_GATE_DROP_BLOCKED_MESSAGE };
  }

  const sourceNode = sourceNodeById(graph, target.targetSourceId);
  if (isClosedInheritedTemplateHtmlNode(sourceNode, graph, context)) {
    return { allowed: false, message: TERA_GATE_DROP_BLOCKED_MESSAGE };
  }
  const teraNode = nearestTeraSourceNode(graph, sourceNode);
  if (!teraNode) return { allowed: true };
  if (teraNode.id === context.openedGateSourceId) return { allowed: true };
  if (openSourceIds.has(teraNode.id)) return { allowed: true };
  return { allowed: false, message: TERA_GATE_DROP_BLOCKED_MESSAGE };
}

export function isClosedInheritedTemplateHtmlNode(
  node: SourceGraphNode | null | undefined,
  graph: SourceGraph | null,
  context: TeraGateOpenContext,
) {
  if (!node || node.kind !== "html") return false;
  if (!isTemplatePath(node.file)) return false;
  const active = normalizeProjectPath(context.activeScannedPath);
  if (active && normalizeProjectPath(node.file) === active) return false;
  return !sourceNodeBelongsToOpenTeraGate(node, graph, context);
}

export function normalizeProjectPath(path: string | null | undefined) {
  if (!path) return "";
  return projectRelativeZolaPath(path)
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\.\//, "");
}

function addNormalizedFile(files: Set<string>, path: string | null | undefined) {
  const normalized = normalizeProjectPath(path);
  if (normalized) files.add(normalized);
}

function isTemplatePath(path: string | null | undefined) {
  const normalized = normalizeProjectPath(path);
  return normalized.startsWith("templates/") || /^themes\/[^/]+\/templates\//.test(normalized);
}

function templateChainFiles(graph: SourceGraph | null, templateNodeId: string) {
  if (!graph) return [];
  const files: string[] = [];
  const visited = new Set<string>();
  let currentNodeId: string | null = templateNodeId;

  while (currentNodeId && !visited.has(currentNodeId)) {
    visited.add(currentNodeId);
    const template = graph.templates.find((candidate) => candidate.nodeId === currentNodeId);
    if (!template) break;
    files.push(template.file);
    currentNodeId = graph.relations.find((relation) =>
      relation.kind === "extends" && relation.from === template.nodeId,
    )?.to ?? null;
  }

  return files;
}
