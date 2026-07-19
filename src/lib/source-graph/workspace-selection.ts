import type { SitePageType } from "$lib/source-graph/architecture";
import type { SourceGraph, SourceGraphPage, SourceGraphTemplate } from "$lib/types";

export function previewUrlForSourcePage(previewSrc: string, page: SourceGraphPage | null) {
  if (!previewSrc) return "";
  if (!page?.url) return previewSrc;
  try {
    return new URL(page.url, previewSrc).toString();
  } catch {
    return previewSrc;
  }
}

export function visitorPreviewUrlForSourcePage(previewSrc: string, page: SourceGraphPage | null) {
  return previewUrlWithoutEditorBridge(previewUrlForSourcePage(previewSrc, page));
}

export function previewUrlWithoutEditorBridge(url: string) {
  if (!url) return "";
  try {
    const next = new URL(url);
    next.searchParams.set("__pana_view", "visitor");
    return next.toString();
  } catch {
    return url.includes("?") ? `${url}&__pana_view=visitor` : `${url}?__pana_view=visitor`;
  }
}

export function currentTemplateTargetForWorkspace(options: {
  teraTemplate?: SourceGraphTemplate | null;
  selectedTemplate?: SourceGraphTemplate | null;
  templates: SourceGraphTemplate[];
}) {
  return options.teraTemplate && !options.teraTemplate.isPartial
    ? options.teraTemplate
    : options.selectedTemplate && !options.selectedTemplate.isPartial
      ? options.selectedTemplate
      : options.templates.find((template) => !template.isPartial) ?? null;
}

export function targetTemplateForWorkspace(
  graph: SourceGraph | null,
  targetTemplateNodeId: string | null,
  fallbackTemplate: SourceGraphTemplate | null,
) {
  const explicit = targetTemplateNodeId
    ? (graph?.templates.find((template) => template.nodeId === targetTemplateNodeId && !template.isPartial) ?? null)
    : null;
  return explicit ?? fallbackTemplate;
}

export function templateForSitePageType(
  graph: SourceGraph | null,
  pageType: SitePageType,
): SourceGraphTemplate | null {
  if (!graph || !pageType.templateName) return null;
  return graph.templates.find((template) => template.name === pageType.templateName && !template.isPartial) ?? null;
}

export function pageTypesUsingTemplate(pageTypes: SitePageType[], template: SourceGraphTemplate | null) {
  if (!template) return [];
  return pageTypes.filter((pageType) => pageType.templateName === template.name);
}
