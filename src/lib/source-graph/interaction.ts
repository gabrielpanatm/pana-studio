import { projectRelativeZolaPath } from "$lib/project/files";
import type { SourceNodeKind } from "$lib/source-graph/contracts";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { SourceGraphNode } from "$lib/source-graph/contracts";

export type SourceInteractionOrigin = "current" | "local" | "theme" | "unknown";

export const TERA_SOURCE_KINDS = new Set<SourceNodeKind>([
  "extends",
  "block",
  "include",
  "componentDefinition",
  "componentCall",
  "legacyTera",
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

export function normalizeProjectPath(path: string | null | undefined) {
  if (!path) return "";
  return projectRelativeZolaPath(path)
    .replaceAll("\\", "/")
    .replace(/\/+/g, "/")
    .replace(/^\.\//, "");
}
