import { buildHtmlSnippet } from "$lib/html/snippets";
import type { HtmlPaletteElement } from "$lib/project/html-palette";
import {
  startElementPaletteDrag,
  type ElementPaletteDragHost,
} from "$lib/state/element-palette-drag-controller";
import {
  startTeraPaletteDrag,
  type TeraPaletteDragHost,
} from "$lib/state/tera-palette-drag-controller";
import {
  isTeraConstructKind,
  type TeraPaletteFamily,
  type TeraPaletteItem,
} from "$lib/tera/model";
import { teraSnippetForItem } from "$lib/tera/palette";
import type { InsertCatalogItem } from "$lib/types";

export type InsertCatalogDragHost = ElementPaletteDragHost & TeraPaletteDragHost;

const teraFamilies = new Set<TeraPaletteFamily>([
  "composition",
  "logic",
  "data",
  "reuse",
  "safe",
]);

function htmlElementForItem(item: InsertCatalogItem): HtmlPaletteElement | null {
  if (item.payload.kind === "html") {
    const payload = item.payload;
    return {
      id: item.id,
      kind: "html",
      tag: payload.tag,
      label: item.label,
      description: item.description,
      text: payload.text,
      className: payload.className,
      html: buildHtmlSnippet({
        tag: payload.tag,
        className: payload.className,
        text: payload.text,
      }),
    };
  }
  if (item.payload.kind === "block") {
    return {
      id: item.id,
      kind: "block",
      blockId: item.payload.blockId,
      blockKind: item.payload.blockKind,
      tag: item.payload.tag,
      label: item.label,
      description: item.description,
      text: item.payload.text,
      className: item.payload.className,
      // Rust renders the authoritative block source from blockId.
      html: "",
    };
  }
  return null;
}

function teraItemForCatalogItem(item: InsertCatalogItem): TeraPaletteItem | null {
  const payload = item.payload;
  if (payload.kind === "dynamicWidget") {
    const result: TeraPaletteItem = {
      id: item.id,
      kind: "dynamicWidget",
      family: "data",
      label: item.label,
      description: item.description,
      snippet: "",
      dynamicWidget: payload.properties,
    };
    result.snippet = teraSnippetForItem(result);
    return result;
  }
  if (
    payload.kind !== "component"
    && payload.kind !== "tera"
    && payload.kind !== "dynamicField"
  ) return null;
  if (!isTeraConstructKind(payload.teraKind)) return null;
  const family = teraFamilies.has(payload.family as TeraPaletteFamily)
    ? payload.family as TeraPaletteFamily
    : "safe";
  const result: TeraPaletteItem = {
    id: item.id,
    kind: payload.teraKind,
    family,
    label: item.label,
    description: item.description,
    snippet: "",
    target: payload.kind === "dynamicField" ? undefined : payload.target ?? undefined,
    name: payload.kind === "dynamicField" ? undefined : payload.name ?? undefined,
    expression: payload.expression ?? undefined,
    sourceNodeId: payload.kind === "component" ? payload.componentId : undefined,
    dynamicBinding: payload.kind === "dynamicField" ? payload.binding : undefined,
  };
  result.snippet = teraSnippetForItem(result);
  return result;
}

export function startInsertCatalogDrag(
  host: InsertCatalogDragHost,
  item: InsertCatalogItem,
  event: PointerEvent,
) {
  if (!item.capabilities.canDrag) return;
  const html = htmlElementForItem(item);
  if (html) {
    startElementPaletteDrag(host, html, event);
    return;
  }
  const tera = teraItemForCatalogItem(item);
  if (tera) startTeraPaletteDrag(host, tera, event);
}
