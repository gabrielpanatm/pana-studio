import type { SourceGraph, SourceGraphNode, SourceGraphTemplate } from "$lib/types";
import { sourceNodeKindLabel } from "$lib/source-graph/view";

export type OutlineTreeItem = {
  node: SourceGraphNode;
  children: OutlineTreeItem[];
};

export type SemanticOutlineItem = {
  id: string;
  label: string;
  kind: "semantic" | "node";
  detail: string;
  node: SourceGraphNode | null;
  children: SemanticOutlineItem[];
};

export type OutlineRow = {
  item: SemanticOutlineItem;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
};

export function templateCompositionNodes(
  graph: SourceGraph | null,
  template: SourceGraphTemplate | null,
): SourceGraphNode[] {
  if (!template || !graph) return [];
  return graph.nodes
    .filter((node) => node.file === template.file && node.range && node.kind !== "template" && node.kind !== "partial")
    .sort((left, right) => (left.range?.start ?? 0) - (right.range?.start ?? 0));
}

export function templateCompositionTree(nodes: SourceGraphNode[]): OutlineTreeItem[] {
  const byId = new Map(nodes.map((node) => [node.id, { node, children: [] as OutlineTreeItem[] }]));
  const roots: OutlineTreeItem[] = [];

  for (const node of nodes) {
    const item = byId.get(node.id);
    if (!item) continue;
    const parent = node.parent ? byId.get(node.parent) : null;
    if (parent) {
      parent.children.push(item);
    } else {
      roots.push(item);
    }
  }

  sortOutlineItems(roots);
  return roots;
}

export function semanticOutlineTree(items: OutlineTreeItem[]): SemanticOutlineItem[] {
  const rootItems = items.map(outlineTreeToSemanticNode);
  const headItems: SemanticOutlineItem[] = [];
  const headerItems: SemanticOutlineItem[] = [];
  const mainItems: SemanticOutlineItem[] = [];
  const footerItems: SemanticOutlineItem[] = [];

  for (const item of rootItems) {
    const zone = semanticZoneForItem(item);
    if (zone === "head") {
      headItems.push(item);
    } else if (zone === "header") {
      headerItems.push(item);
    } else if (zone === "footer") {
      footerItems.push(item);
    } else {
      mainItems.push(item);
    }
  }

  const bodyChildren = [
    semanticGroup("semantic:header", "header", "Antet vizibil", headerItems),
    semanticGroup("semantic:main", "main", "Conținut principal", mainItems),
    semanticGroup("semantic:footer", "footer", "Subsol vizibil", footerItems),
  ].filter((item) => item.children.length > 0 || item.id === "semantic:main");

  return [
    semanticGroup("semantic:head", "head", "Metadate, stiluri și scripturi", headItems),
    semanticGroup("semantic:body", "body", "Structura vizibilă a paginii", bodyChildren),
  ];
}

export function visibleOutlineRows(
  items: SemanticOutlineItem[],
  expandedNodeIds: Set<string>,
  depth = 0,
): OutlineRow[] {
  const rows: OutlineRow[] = [];
  for (const item of items) {
    const hasChildren = item.children.length > 0;
    const expanded = expandedNodeIds.has(item.id);
    rows.push({ item, depth, hasChildren, expanded });
    if (hasChildren && expanded) rows.push(...visibleOutlineRows(item.children, expandedNodeIds, depth + 1));
  }
  return rows;
}

export function outlineItemContainsNode(item: SemanticOutlineItem, nodeId: string | null): boolean {
  if (!nodeId) return false;
  if (item.node?.id === nodeId) return true;
  return item.children.some((child) => outlineItemContainsNode(child, nodeId));
}

export function outlineChildCountLabel(count: number) {
  if (count === 1) return "1 element";
  return `${count} elemente`;
}

export function firstSourceNodeInOutline(item: SemanticOutlineItem): SourceGraphNode | null {
  if (item.node) return item.node;
  for (const child of item.children) {
    const found = firstSourceNodeInOutline(child);
    if (found) return found;
  }
  return null;
}

export function lastSourceNodeInOutline(item: SemanticOutlineItem): SourceGraphNode | null {
  for (const child of [...item.children].reverse()) {
    const found = lastSourceNodeInOutline(child);
    if (found) return found;
  }
  return item.node;
}

function sortOutlineItems(items: OutlineTreeItem[]) {
  items.sort((left, right) => (left.node.range?.start ?? 0) - (right.node.range?.start ?? 0));
  for (const item of items) sortOutlineItems(item.children);
}

function outlineTreeToSemanticNode(item: OutlineTreeItem): SemanticOutlineItem {
  return {
    id: item.node.id,
    label: semanticLabelForNode(item.node),
    kind: "node",
    detail: sourceNodeKindLabel(item.node.kind),
    node: item.node,
    children: item.children.map(outlineTreeToSemanticNode),
  };
}

function semanticGroup(id: string, label: string, detail: string, children: SemanticOutlineItem[]): SemanticOutlineItem {
  return { id, label, kind: "semantic", detail, node: null, children };
}

function semanticZoneForItem(item: SemanticOutlineItem): "head" | "header" | "main" | "footer" {
  const text = `${item.label} ${item.detail}`.toLowerCase();
  if (text.includes("<head") || text.includes("<title") || text.includes("<meta") || text.includes("<link")) return "head";
  if (text.includes("header") || text.includes("partials/header") || text.includes("<nav")) return "header";
  if (text.includes("footer") || text.includes("partials/footer")) return "footer";
  if (text.includes("<main") || text.includes("content")) return "main";
  return "main";
}

function semanticLabelForNode(node: SourceGraphNode) {
  const tag = tagNameFromLabel(node.label);
  if (tag) return tag;
  if (node.kind === "block") return `block ${node.label}`;
  if (node.kind === "include") return `include ${node.label}`;
  if (node.kind === "extends") return `extends ${node.label}`;
  return node.label;
}

function tagNameFromLabel(label: string) {
  return label.match(/^<\s*([a-zA-Z0-9-]+)/)?.[1]?.toLowerCase() ?? "";
}
