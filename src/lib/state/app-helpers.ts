import {
  zolaRelativePath,
} from "$lib/project/files";
import { normalizeProjectPath, sourceInteractionOrigin } from "$lib/source-graph/interaction";
import type { HtmlPendingArea, InspectorPendingArea, SourceGraphNode } from "$lib/types";

export type PreviewTemplateGateOrigin = "current" | "local" | "theme" | "unknown";

export type PreviewTemplateGate = {
  selector: string;
  sourceId: string;
  origin: PreviewTemplateGateOrigin;
  themeName: string | null;
  canSelectHtml?: boolean;
};

export function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, vars: false, js: false };
}

export function includeTemplateNameForRenderedFile(file: string) {
  const zolaPath = zolaRelativePath(file).replace(/^\/+/, "");
  const themeTemplate = zolaPath.match(/^themes\/[^/]+\/templates\/(.+)$/);
  if (themeTemplate) return themeTemplate[1].toLowerCase();
  const localTemplate = zolaPath.match(/^templates\/(.+)$/);
  return localTemplate?.[1]?.toLowerCase() ?? null;
}

export function normalizedProjectPath(path: string | null | undefined) {
  return normalizeProjectPath(path);
}

export function templateOriginKind(
  node: SourceGraphNode | null,
  activeScannedPath: string | null,
): PreviewTemplateGateOrigin {
  return sourceInteractionOrigin(node, activeScannedPath);
}

export function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return { tag: false, attributes: false, text: false, image: false, classes: false, structure: false };
}

export function initialUiTheme(): "dark" | "light" {
  if (typeof document === "undefined") return "dark";
  return document.documentElement.dataset.panaTheme === "light" ? "light" : "dark";
}

export function shellQuote(value: string) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}
