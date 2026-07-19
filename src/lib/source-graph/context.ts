import type {
  SourceGraph,
  SourceGraphNode,
  SourceGraphPage,
  SourceGraphTemplate,
  SourceNodeKind,
} from "$lib/types";
import { sourceNodeById, sourceTemplateChainForPage } from "$lib/source-graph/view";

export type TeraEditabilityLevel = "safe" | "limited" | "code";

export type TeraEditorContext = {
  node: SourceGraphNode | null;
  template: SourceGraphTemplate | null;
  page: SourceGraphPage | null;
  teraAncestors: SourceGraphNode[];
  nearestTera: SourceGraphNode | null;
  structureLabel: string;
  visitorLabel: string;
  impactLabel: string;
  editabilityLevel: TeraEditabilityLevel;
  editabilityLabel: string;
  editabilityReason: string;
};

const TERA_KINDS = new Set<SourceNodeKind>([
  "extends",
  "block",
  "include",
  "import",
  "macro",
  "for",
  "if",
  "set",
  "with",
  "teraVariable",
  "teraComment",
  "raw",
  "tera",
]);

const TERA_CONTEXT_KINDS = new Set<SourceNodeKind>(
  [...TERA_KINDS].filter((kind) => kind !== "extends"),
);

export function buildTeraEditorContext(
  graph: SourceGraph | null,
  selectedNodeId: string | null,
): TeraEditorContext {
  const nodes = sourceNodeById(graph);
  const node = selectedNodeId ? (nodes.get(selectedNodeId) ?? null) : null;
  const ancestors = node ? sourceAncestors(nodes, node) : [];
  const teraAncestors = ancestors.filter((ancestor) => TERA_KINDS.has(ancestor.kind));
  const nearestTera = teraAncestors.find((ancestor) => TERA_CONTEXT_KINDS.has(ancestor.kind)) ?? null;
  const template = findOwningTemplate(graph, node, ancestors);
  const page = findPrimaryPageForNode(graph, node, template);
  const editability = editabilityForNode(node, nearestTera);
  const impactLabel = impactForNode(graph, node, template, page);

  return {
    node,
    template,
    page,
    teraAncestors,
    nearestTera,
    structureLabel: structureForNode(node, nearestTera, template),
    visitorLabel: visitorLabelForNode(node, nearestTera, template),
    impactLabel,
    editabilityLevel: editability.level,
    editabilityLabel: editability.label,
    editabilityReason: editability.reason,
  };
}

export function sourceAncestors(
  nodesById: Map<string, SourceGraphNode>,
  node: SourceGraphNode,
): SourceGraphNode[] {
  const ancestors: SourceGraphNode[] = [];
  const visited = new Set<string>();
  let parentId = node.parent;

  while (parentId && !visited.has(parentId)) {
    visited.add(parentId);
    const parent = nodesById.get(parentId);
    if (!parent) break;
    ancestors.push(parent);
    parentId = parent.parent;
  }

  return ancestors;
}

export function teraKindLabel(kind: SourceNodeKind): string {
  const labels: Partial<Record<SourceNodeKind, string>> = {
    template: "template Tera",
    partial: "partial Tera",
    extends: "layout",
    block: "block",
    include: "partial inclus",
    import: "import macro",
    macro: "macro",
    for: "listă generată",
    if: "zonă condițională",
    set: "variabilă locală",
    with: "context local",
    teraVariable: "variabilă afișată",
    teraComment: "comentariu",
    raw: "raw",
    tera: "Tera",
  };
  return labels[kind] ?? kind;
}

function findOwningTemplate(
  graph: SourceGraph | null,
  node: SourceGraphNode | null,
  ancestors: SourceGraphNode[],
): SourceGraphTemplate | null {
  if (!graph || !node) return null;
  const templateNode =
    [node, ...ancestors].find((candidate) => candidate.kind === "template" || candidate.kind === "partial") ?? null;
  if (templateNode) {
    return graph.templates.find((template) => template.nodeId === templateNode.id) ?? null;
  }
  return graph.templates.find((template) => template.file === node.file) ?? null;
}

function findPrimaryPageForNode(
  graph: SourceGraph | null,
  node: SourceGraphNode | null,
  template: SourceGraphTemplate | null,
): SourceGraphPage | null {
  if (!graph || !node) return null;
  const directPage = graph.pages.find((page) => page.id === node.id || page.contentNodeId === node.id);
  if (directPage) return directPage;
  if (!template) return null;
  return graph.pages.find((page) => pageUsesTemplate(graph, page, template)) ?? null;
}

function pageUsesTemplate(
  graph: SourceGraph,
  page: SourceGraphPage,
  template: SourceGraphTemplate,
): boolean {
  if (page.templateNodeId === template.nodeId) return true;
  const chain = sourceTemplateChainForPage(graph, page);
  if (chain.some((entry) => entry.nodeId === template.nodeId)) return true;
  const chainIds = new Set(chain.map((entry) => entry.nodeId));
  return graph.relations.some((relation) => {
    if (!chainIds.has(relation.from)) return false;
    if (relation.to !== template.nodeId) return false;
    return relation.kind === "includes" || relation.kind === "imports";
  });
}

function pagesUsingTemplate(
  graph: SourceGraph | null,
  template: SourceGraphTemplate | null,
): SourceGraphPage[] {
  if (!graph || !template) return [];
  return graph.pages.filter((page) => pageUsesTemplate(graph, page, template));
}

function structureForNode(
  node: SourceGraphNode | null,
  nearestTera: SourceGraphNode | null,
  template: SourceGraphTemplate | null,
) {
  if (!node) return "Selectează o zonă din preview sau din navigator.";
  if (node.kind === "page") return "Pagină de conținut Zola.";
  if (node.kind === "template") return "Template de pagină.";
  if (node.kind === "partial") return "Parte comună reutilizabilă.";
  if (node.kind === "style") return "Fișier de stil conectat la site.";
  if (node.kind === "asset") return "Asset static referențiat din template-uri Zola.";
  if (nearestTera) {
    return `${node.label} este în ${teraKindLabel(nearestTera.kind)} "${nearestTera.label}" din ${template?.name ?? node.file}.`;
  }
  if (node.kind === "html") return `${node.label} este HTML static în ${template?.name ?? node.file}.`;
  return `${teraKindLabel(node.kind)} în ${template?.name ?? node.file}.`;
}

function visitorLabelForNode(
  node: SourceGraphNode | null,
  nearestTera: SourceGraphNode | null,
  template: SourceGraphTemplate | null,
) {
  if (!node) return "Vezi site-ul ca vizitator; contextul apare când selectezi o zonă.";
  if (node.kind === "page") return "Aceasta este pagina pe care o vizitează utilizatorul.";
  if (template?.isPartial) return "Această zonă poate apărea pe mai multe pagini ca parte comună.";
  if (nearestTera?.kind === "for") return "Vizitatorul vede o listă generată din date sau conținut.";
  if (nearestTera?.kind === "if") return "Vizitatorul vede această zonă doar când condiția Tera este adevărată.";
  if (nearestTera?.kind === "block") return "Vizitatorul vede conținutul paginii așezat într-un layout.";
  return "Această zonă este randată direct în pagina vizitată.";
}

function impactForNode(
  graph: SourceGraph | null,
  node: SourceGraphNode | null,
  template: SourceGraphTemplate | null,
  page: SourceGraphPage | null,
) {
  if (!node) return "Niciun impact calculat încă.";
  if (node.kind === "style") return "Poate afecta toate locurile unde este încărcat acest fișier CSS/SCSS.";
  if (node.kind === "asset") return "Poate afecta template-urile care generează URL-uri sau hash-uri pentru acest asset.";
  const pages = pagesUsingTemplate(graph, template);
  if (template?.isPartial) {
    if (pages.length === 0) return "Parte comună; impactul exact nu este încă legat de pagini.";
    if (pages.length === 1) return `Parte comună folosită de pagina ${pages[0].title}.`;
    return `Parte comună cu impact pe ${pages.length} pagini.`;
  }
  if (pages.length > 1) return `Template folosit de ${pages.length} pagini.`;
  if (page) return `Impact local pe pagina ${page.title}.`;
  return "Impact local probabil, dar pagina nu a fost rezolvată complet.";
}

function editabilityForNode(
  node: SourceGraphNode | null,
  nearestTera: SourceGraphNode | null,
): { level: TeraEditabilityLevel; label: string; reason: string } {
  if (!node) {
    return {
      level: "code",
      label: "Fără selecție",
      reason: "Selectează o zonă ca să vezi capabilitățile.",
    };
  }
  if (!node.capabilities.canEditVisual) {
    return {
      level: "code",
      label: "Cod",
      reason: node.capabilities.reason ?? "Zona se editează în cod.",
    };
  }
  if (nearestTera && nearestTera.kind !== "block") {
    return {
      level: "limited",
      label: "Limitat",
      reason: `Zona este influențată de ${teraKindLabel(nearestTera.kind)}; editarea vizuală trebuie restrânsă.`,
    };
  }
  return {
    level: "safe",
    label: "Vizual sigur",
    reason: "Zona are sursă HTML stabilă în template.",
  };
}
