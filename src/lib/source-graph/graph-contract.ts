import type { BlockGraph } from "$lib/blocks/contracts";
import type {
  ContentModelCatalog,
  DynamicWidgetGraph,
  ListingItemCatalog,
} from "$lib/content-models/contracts";
import type {
  ComponentGraph,
  MarkdownProjection,
  SourceGraphAsset,
  SourceGraphDataFile,
  SourceGraphDiagnostic,
  SourceGraphNode,
  SourceGraphPage,
  SourceGraphRelation,
  SourceGraphScript,
  SourceGraphStyle,
  SourceGraphTemplate,
  SourceStructuredDocument,
} from "$lib/source-graph/contracts";

export type SourceGraph = {
  projectRoot: string;
  zolaRoot: string;
  activeTheme: string | null;
  pages: SourceGraphPage[];
  templates: SourceGraphTemplate[];
  styles: SourceGraphStyle[];
  scripts: SourceGraphScript[];
  assets: SourceGraphAsset[];
  dataFiles: SourceGraphDataFile[];
  structuredDocuments: SourceStructuredDocument[];
  componentGraph: ComponentGraph;
  blockGraph: BlockGraph;
  contentModels: ContentModelCatalog;
  listingItems: ListingItemCatalog;
  dynamicWidgetGraph: DynamicWidgetGraph;
  markdownProjections: MarkdownProjection[];
  nodes: SourceGraphNode[];
  relations: SourceGraphRelation[];
  assetReferenceCoverage: {
    eligible: number;
    analyzed: number;
    unanalysable: number;
  };
  diagnostics: SourceGraphDiagnostic[];
};

export type SourceGraphProjectionReceipt = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  graph: SourceGraph;
};

export type WorkspaceCatalogProjectionReceipt<T> = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  catalog: T;
};
