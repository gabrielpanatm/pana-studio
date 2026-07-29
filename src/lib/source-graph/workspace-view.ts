import {
  firstSourceNodeInOutline,
  type SemanticOutlineItem,
} from "$lib/source-graph/outline";
import {
  sourceOriginLabel,
  sourceTemplateByNodeId,
} from "$lib/source-graph/view";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  SourceGraph,
  SourceGraphNode,
  SourceGraphRelation,
  SourceGraphTemplate,
  SourceOrigin,
} from "$lib/types";

export function originForSourceNodeId(
  nodesById: Map<string, SourceGraphNode>,
  nodeId: string | null | undefined,
): SourceOrigin {
  return nodesById.get(nodeId ?? "")?.origin ?? "local";
}

export function originForSemanticOutlineItem(item: SemanticOutlineItem): SourceOrigin {
  return (item.node ?? firstSourceNodeInOutline(item))?.origin ?? "local";
}

export function isMatchingNodeId(targetNodeId: string | null, nodeId: string | null | undefined) {
  return Boolean(nodeId && targetNodeId === nodeId);
}

export function relationTargetNode(
  nodesById: Map<string, SourceGraphNode>,
  relation: SourceGraphRelation,
): SourceGraphNode | null {
  return nodesById.get(relation.to) ?? null;
}

export function relationSourceNode(
  nodesById: Map<string, SourceGraphNode>,
  relation: SourceGraphRelation,
): SourceGraphNode | null {
  return nodesById.get(relation.from) ?? null;
}

export function templateRelationTarget(
  graph: SourceGraph | null,
  relation: SourceGraphRelation,
): SourceGraphTemplate | null {
  return sourceTemplateByNodeId(graph, relation.to);
}

export function sourceNodeSubtitle(node: SourceGraphNode | null) {
  if (!node) return "";
  const origin = sourceOriginLabel(node.origin, node.themeName);
  if (node.range) return `${origin} · ${node.file}:${node.range.line}:${node.range.column}`;
  return `${origin} · ${node.file}`;
}

export function templateBlocksLabel(template: SourceGraphTemplate) {
  if (!template.blocks.length) return t("source-workspace-no-blocks");
  return template.blocks.join(", ");
}
