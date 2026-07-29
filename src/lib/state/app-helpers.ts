import {
  zolaRelativePath,
} from "$lib/project/files";
import { normalizeProjectPath, sourceInteractionOrigin } from "$lib/source-graph/interaction";
import type { HtmlPendingArea, InspectorPendingArea, SourceGraphNode } from "$lib/types";

export type PreviewTeraSelectionOrigin = "current" | "local" | "theme" | "unknown";

export type PreviewTeraSelectionTarget = {
  selector: string;
  sourceId: string;
  origin: PreviewTeraSelectionOrigin;
  themeName: string | null;
};

export function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, js: false };
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
): PreviewTeraSelectionOrigin {
  return sourceInteractionOrigin(node, activeScannedPath);
}

export function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return { tag: false, attributes: false, text: false, image: false, classes: false, structure: false };
}

export function initialUiTheme(): "dark" | "light" {
  if (typeof document === "undefined") return "dark";
  return document.documentElement.dataset.panaTheme === "light" ? "light" : "dark";
}

export function contrastingTextColor(color: string): "#111111" | "#ffffff" {
  const match = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(color);
  if (!match) return "#ffffff";
  const channels = match.slice(1).map((channel) => Number.parseInt(channel, 16) / 255);
  const [red, green, blue] = channels.map((channel) =>
    channel <= 0.04045
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  );
  const luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  return luminance > 0.42 ? "#111111" : "#ffffff";
}

export function shellQuote(value: string) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}
