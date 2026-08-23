import type { SourceRelationKind } from "$lib/source-graph/contracts";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type {
  SourceGraphNode,
  SourceGraphRelation,
  SourceGraphStyle,
  SourceGraphTemplate,
} from "$lib/source-graph/contracts";
import { t } from "$lib/i18n/runtime.svelte";

export function sourceNodeById(graph: SourceGraph | null): Map<string, SourceGraphNode> {
  return new Map((graph?.nodes ?? []).map((node) => [node.id, node]));
}

export function sourceRelationsFrom(
  graph: SourceGraph | null,
  nodeId: string | null,
  kind?: SourceRelationKind,
): SourceGraphRelation[] {
  if (!graph || !nodeId) return [];
  return graph.relations.filter((relation) => relation.from === nodeId && (!kind || relation.kind === kind));
}

export function sourceTemplateByNodeId(
  graph: SourceGraph | null,
  nodeId: string | null,
): SourceGraphTemplate | null {
  if (!graph || !nodeId) return null;
  return graph.templates.find((template) => template.nodeId === nodeId) ?? null;
}

export function sourceStyleByNodeId(graph: SourceGraph | null, nodeId: string | null): SourceGraphStyle | null {
  if (!graph || !nodeId) return null;
  return graph.styles.find((style) => style.nodeId === nodeId) ?? null;
}

export function sourceOriginLabel(origin: "local" | "theme", themeName?: string | null): string {
  if (origin === "theme") {
    return themeName
      ? t("source-view-origin-theme-name", { name: themeName })
      : t("source-view-origin-theme");
  }
  return t("source-view-origin-local");
}
