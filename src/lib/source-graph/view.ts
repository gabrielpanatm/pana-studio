import type {
  SourceGraph,
  SourceGraphNode,
  SourceGraphPage,
  SourceGraphRelation,
  SourceGraphAsset,
  SourceGraphDataFile,
  SourceGraphStyle,
  SourceGraphTemplate,
  SourceNodeKind,
  SourceRelationKind,
} from "$lib/types";

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

export function sourceRelationsTo(
  graph: SourceGraph | null,
  nodeId: string | null,
  kind?: SourceRelationKind,
): SourceGraphRelation[] {
  if (!graph || !nodeId) return [];
  return graph.relations.filter((relation) => relation.to === nodeId && (!kind || relation.kind === kind));
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

export function sourceAssetByNodeId(graph: SourceGraph | null, nodeId: string | null): SourceGraphAsset | null {
  if (!graph || !nodeId) return null;
  return graph.assets.find((asset) => asset.nodeId === nodeId) ?? null;
}

export function sourceDataFileByNodeId(
  graph: SourceGraph | null,
  nodeId: string | null,
): SourceGraphDataFile | null {
  if (!graph || !nodeId) return null;
  return graph.dataFiles.find((dataFile) => dataFile.nodeId === nodeId) ?? null;
}

export function sourcePageByNodeId(graph: SourceGraph | null, nodeId: string | null): SourceGraphPage | null {
  if (!graph || !nodeId) return null;
  return graph.pages.find((page) => page.contentNodeId === nodeId || page.id === nodeId) ?? null;
}

export function sourceTemplateChainForPage(
  graph: SourceGraph | null,
  page: SourceGraphPage | null,
): SourceGraphTemplate[] {
  if (!graph || !page?.templateNodeId) return [];
  const chain: SourceGraphTemplate[] = [];
  const visited = new Set<string>();
  let currentNodeId: string | null = page.templateNodeId;

  while (currentNodeId && !visited.has(currentNodeId)) {
    visited.add(currentNodeId);
    const template = sourceTemplateByNodeId(graph, currentNodeId);
    if (!template) break;
    chain.push(template);
    currentNodeId = sourceRelationsFrom(graph, currentNodeId, "extends")[0]?.to ?? null;
  }

  return chain;
}

export function sourcePageTemplateSideRelations(
  graph: SourceGraph | null,
  templates: SourceGraphTemplate[],
  kind: "includes" | "imports",
): SourceGraphRelation[] {
  if (!graph) return [];
  const ids = new Set(templates.map((template) => template.nodeId));
  return graph.relations.filter((relation) => ids.has(relation.from) && relation.kind === kind);
}

export function sourceStylesForPage(
  graph: SourceGraph | null,
  page: SourceGraphPage | null,
): SourceGraphStyle[] {
  if (!graph || !page) return [];
  return sourceRelationsFrom(graph, page.id, "usesStyle")
    .map((relation) => sourceStyleByNodeId(graph, relation.to))
    .filter((style): style is SourceGraphStyle => Boolean(style));
}

export function initialSourceNodeIdForPath(
  graph: SourceGraph | null,
  activePath: string | null,
): string | null {
  if (!graph) return null;
  if (activePath) {
    const page = graph.pages.find((candidate) => candidate.file === activePath);
    if (page) return page.id;
    const template = graph.templates.find((candidate) => candidate.file === activePath);
    if (template) return template.nodeId;
    const style = graph.styles.find((candidate) => candidate.file === activePath);
    if (style) return style.nodeId;
    const dataFile = graph.dataFiles.find((candidate) => candidate.file === activePath);
    if (dataFile) return dataFile.nodeId;
    const node = graph.nodes.find((candidate) => candidate.file === activePath);
    if (node) return node.id;
  }
  return graph.pages[0]?.id ?? graph.templates[0]?.nodeId ?? graph.styles[0]?.nodeId ?? graph.nodes[0]?.id ?? null;
}

export function sourceNodeKindLabel(kind: SourceNodeKind): string {
  const labels: Record<SourceNodeKind, string> = {
    page: "Pagină",
    template: "Template",
    partial: "Partial",
    style: "Stil",
    script: "Script",
    asset: "Asset",
    dataFile: "Date",
    dataTable: "Tabel de date",
    dataArray: "Listă de date",
    dataValue: "Valoare",
    dataComment: "Comentariu de date",
    configFile: "Configurație",
    html: "HTML",
    blockMarker: "Bloc nativ",
    macroCall: "Apel macro",
    functionCall: "Apel funcție",
    shortcode: "Shortcode",
    extends: "Extends",
    block: "Block",
    include: "Include",
    import: "Import",
    macro: "Macro",
    for: "For",
    if: "If",
    elif: "Else if",
    else: "Else",
    set: "Set",
    setGlobal: "Set global",
    filter: "Filtru",
    break: "Break",
    continue: "Continue",
    super: "Super",
    teraVariable: "Variabilă",
    teraComment: "Comentariu",
    raw: "Raw",
    tera: "Tera",
  };
  return labels[kind];
}

export function sourceOriginLabel(origin: "local" | "theme", themeName?: string | null): string {
  if (origin === "theme") return themeName ? `Theme: ${themeName}` : "Theme";
  return "Local";
}

export function sourceRelationKindLabel(kind: SourceRelationKind): string {
  const labels: Record<SourceRelationKind, string> = {
    pageTemplate: "template",
    sectionPageTemplate: "template pagini secțiune",
    getsPage: "get_page",
    getsSection: "get_section",
    internalContentLink: "link intern",
    assetUrl: "asset URL",
    assetHash: "asset hash",
    dataLoad: "load_data",
    dataFileLoad: "load_data data",
    contentDataLoad: "load_data content",
    imageMetadata: "get_image_metadata",
    imageResize: "resize_image",
    extends: "extinde",
    includes: "include",
    imports: "importă",
    definesBlock: "definește block",
    overridesBlock: "suprascrie block",
    usesStyle: "stil",
    usesScript: "script",
  };
  return labels[kind];
}
