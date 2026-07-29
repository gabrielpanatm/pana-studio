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
    page: t("source-view-kind-page"),
    template: t("source-view-kind-template"),
    partial: t("source-view-kind-partial"),
    style: t("source-view-kind-style"),
    script: t("source-view-kind-script"),
    asset: t("source-view-kind-asset"),
    dataFile: t("source-view-kind-data-file"),
    dataTable: t("source-view-kind-data-table"),
    dataArray: t("source-view-kind-data-array"),
    dataValue: t("source-view-kind-data-value"),
    dataComment: t("source-view-kind-data-comment"),
    configFile: t("source-view-kind-config-file"),
    html: t("source-view-kind-html"),
    blockMarker: t("source-view-kind-block-marker"),
    macroCall: t("source-view-kind-macro-call"),
    functionCall: t("source-view-kind-function-call"),
    shortcode: t("source-view-kind-shortcode"),
    extends: t("source-view-kind-extends"),
    block: t("source-view-kind-block"),
    include: t("source-view-kind-include"),
    import: t("source-view-kind-import"),
    macro: t("source-view-kind-macro"),
    for: t("source-view-kind-for"),
    if: t("source-view-kind-if"),
    elif: t("source-view-kind-elif"),
    else: t("source-view-kind-else"),
    set: t("source-view-kind-set"),
    setGlobal: t("source-view-kind-set-global"),
    filter: t("source-view-kind-filter"),
    break: t("source-view-kind-break"),
    continue: t("source-view-kind-continue"),
    super: t("source-view-kind-super"),
    teraVariable: t("source-view-kind-tera-variable"),
    teraComment: t("source-view-kind-tera-comment"),
    raw: t("source-view-kind-raw"),
    tera: t("source-view-kind-tera"),
  };
  return labels[kind];
}

export function sourceOriginLabel(origin: "local" | "theme", themeName?: string | null): string {
  if (origin === "theme") {
    return themeName
      ? t("source-view-origin-theme-name", { name: themeName })
      : t("source-view-origin-theme");
  }
  return t("source-view-origin-local");
}

export function sourceRelationKindLabel(kind: SourceRelationKind): string {
  const labels: Record<SourceRelationKind, string> = {
    pageTemplate: t("source-view-relation-page-template"),
    sectionPageTemplate: t("source-view-relation-section-template"),
    getsPage: "get_page",
    getsSection: "get_section",
    internalContentLink: t("source-view-relation-internal-link"),
    assetUrl: "asset URL",
    assetHash: "asset hash",
    dataLoad: "load_data",
    dataFileLoad: t("source-view-relation-data-file-load"),
    contentDataLoad: t("source-view-relation-content-data-load"),
    imageMetadata: "get_image_metadata",
    imageResize: "resize_image",
    extends: t("source-view-relation-extends"),
    includes: t("source-view-relation-includes"),
    imports: t("source-view-relation-imports"),
    definesBlock: t("source-view-relation-defines-block"),
    overridesBlock: t("source-view-relation-overrides-block"),
    usesStyle: t("source-view-relation-style"),
    usesScript: t("source-view-relation-script"),
  };
  return labels[kind];
}
