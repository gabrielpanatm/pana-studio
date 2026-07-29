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

export function normalizeProjectPath(path: string | null | undefined) {
  if (!path) return "";
  return projectRelativeZolaPath(path)
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\.\//, "");
}
