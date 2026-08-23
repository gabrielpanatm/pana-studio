import { normalizeProjectPath } from "$lib/source-graph/interaction";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/canvas/contracts";

export type PreviewTeraSelectionOrigin = "current" | "local" | "theme" | "unknown";

export type PreviewTeraSelectionTarget = {
  sourceId: string;
  renderInstanceId: string | null;
  origin: PreviewTeraSelectionOrigin;
  themeName: string | null;
};

export function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, js: false };
}

export function normalizedProjectPath(path: string | null | undefined) {
  return normalizeProjectPath(path);
}

export function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return { tag: false, attributes: false, text: false, image: false, classes: false, structure: false };
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
