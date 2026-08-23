import { isZolaTemplatePath, zolaRelativePath } from "$lib/project/files";
import type { PageSection } from "$lib/canvas/contracts";
import type { SourceGraph } from "$lib/source-graph/graph-contract";
import type { SourceEditLocation } from "$lib/source-graph/contracts";
import type {
  SourceEditTarget,
  SourceGraphNode,
} from "$lib/source-graph/contracts";

export function parseSourceEditLocation(source: string | null | undefined): SourceEditLocation | null {
  if (!source) return null;
  const raw = String(source);
  const last = raw.lastIndexOf(":");
  if (last < 1) return null;

  const tail = raw.slice(last + 1);
  const maybeColumn = parseInt(tail, 10);
  if (Number.isNaN(maybeColumn)) return null;

  const beforeTail = raw.slice(0, last);
  const previous = beforeTail.lastIndexOf(":");
  if (previous > 0) {
    const maybeLine = parseInt(beforeTail.slice(previous + 1), 10);
    if (!Number.isNaN(maybeLine)) {
      return { file: beforeTail.slice(0, previous), line: maybeLine, column: maybeColumn };
    }
  }

  const line = maybeColumn;
  if (Number.isNaN(line)) return null;
  return { file: raw.slice(0, last), line };
}

export function formatSourceEditLocation(source: SourceEditLocation) {
  return typeof source.column === "number"
    ? `${source.file}:${source.line}:${source.column}`
    : `${source.file}:${source.line}`;
}

export function sourceNodeById(
  graph: SourceGraph | null,
  sourceId: string | null | undefined,
): SourceGraphNode | null {
  if (!graph || !sourceId) return null;
  return graph.nodes.find((node) => node.id === sourceId) ?? null;
}

export function sourceEditLocationFromSourceNode(node: SourceGraphNode | null): SourceEditLocation | null {
  if (!node || !node.range) return null;
  const file = zolaRelativePath(node.file);
  if (!isZolaTemplatePath(file)) return null;
  return {
    file,
    line: node.range.line,
    column: node.range.column,
  };
}

export function resolveSourceEditLocationForSourceId(
  graph: SourceGraph | null,
  sourceId: string | null | undefined,
): SourceEditLocation | null {
  return sourceEditLocationFromSourceNode(sourceNodeById(graph, sourceId));
}

export function sourceEditTargetFromSourceNode(
  node: SourceGraphNode | null,
): SourceEditTarget | null {
  if (!node || !node.range) return null;
  if (node.kind === "html" && !node.capabilities.canEditVisual) return null;
  const location = sourceEditLocationFromSourceNode(node);
  if (!location) return null;
  return {
    sourceId: node.id,
    file: location.file,
    location,
    range: node.range,
    kind: node.kind,
    label: node.label,
    capabilities: node.capabilities,
  };
}

export function resolveSourceEditTargetForSourceId(
  graph: SourceGraph | null,
  sourceId: string | null | undefined,
): SourceEditTarget | null {
  return sourceEditTargetFromSourceNode(sourceNodeById(graph, sourceId));
}

export function hydratePageSectionSources(
  sections: PageSection[],
  graph: SourceGraph | null,
): PageSection[] {
  return sections.map((section) => {
    const resolved =
      resolveSourceEditLocationForSourceId(graph, section.sourceId) ??
      section.sourceLocation ??
      null;
    return resolved === section.sourceLocation ? section : { ...section, sourceLocation: resolved };
  });
}
