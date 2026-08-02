import { splitTopLevelCssList } from "$lib/inspector/background-model";

export const CSS_GRID_SCHEMA_VERSION = 1;

export const GRID_CONTAINER_PROPERTIES = [
  "display",
  "grid-template-columns",
  "grid-template-rows",
  "grid-template-areas",
  "grid-auto-columns",
  "grid-auto-rows",
  "grid-auto-flow",
  "column-gap",
  "row-gap",
  "justify-content",
  "align-content",
  "justify-items",
  "align-items",
] as const;

export const GRID_ITEM_PROPERTIES = ["grid-column", "grid-row", "grid-area"] as const;
export const GRID_OPAQUE_PROPERTIES = ["grid", "grid-template", "place-content", "place-items"] as const;
export const GRID_PROPERTIES = [
  ...GRID_CONTAINER_PROPERTIES,
  ...GRID_ITEM_PROPERTIES,
  ...GRID_OPAQUE_PROPERTIES,
] as const;

export type CssGridTrackListMode = "none" | "tracks" | "subgrid" | "masonry" | "opaque";
export type CssGridTrackKind =
  | "keyword"
  | "flex"
  | "length"
  | "minmax"
  | "fit_content"
  | "repeat"
  | "line_names"
  | "dynamic"
  | "opaque";

export type CssGridTrack = {
  id: string;
  kind: CssGridTrackKind;
  raw: string;
  repeatCount: string | null;
  repeatTracks: CssGridTrack[];
  structurallyEditable: boolean;
};

export type CssGridTrackList = {
  raw: string | null;
  mode: CssGridTrackListMode;
  tracks: CssGridTrack[];
  structurallyEditable: boolean;
};

export type CssGridAreas = {
  raw: string | null;
  rows: string[][];
  valid: boolean;
  error: string | null;
  structurallyEditable: boolean;
};

export type CssGrid = {
  schemaVersion: number;
  display: string | null;
  templateColumns: CssGridTrackList;
  templateRows: CssGridTrackList;
  templateAreas: CssGridAreas;
  autoColumns: string | null;
  autoRows: string | null;
  autoFlow: string | null;
  columnGap: string | null;
  rowGap: string | null;
  justifyContent: string | null;
  alignContent: string | null;
  justifyItems: string | null;
  alignItems: string | null;
  itemColumn: string | null;
  itemRow: string | null;
  itemArea: string | null;
  opaqueProperties: Record<string, string>;
  structurallyEditable: boolean;
};

let nextTransientTrackId = 1;

function id(prefix: string) {
  const value = nextTransientTrackId++;
  return `${prefix}-${value}`;
}

function functionBody(value: string, name: string) {
  const trimmed = value.trim();
  const prefix = `${name}(`;
  return trimmed.toLowerCase().startsWith(prefix) && trimmed.endsWith(")")
    ? trimmed.slice(prefix.length, -1)
    : null;
}

function numericUnit(value: string, unit: string) {
  if (!value.endsWith(unit)) return false;
  const numeric = value.slice(0, -unit.length);
  return numeric !== "" && Number.isFinite(Number(numeric));
}

function isLength(value: string) {
  return value === "0"
    || ["px", "em", "rem", "%", "ch", "ex", "vw", "vh", "vmin", "vmax", "cm", "mm", "q", "in", "pc", "pt"]
      .some((unit) => numericUnit(value, unit))
    || ["calc(", "min(", "max(", "clamp(", "env("].some((prefix) => value.startsWith(prefix));
}

export function parseGridTrack(raw: string, index = 0): CssGridTrack {
  const value = raw.trim();
  const normalized = value.toLowerCase();
  const base: CssGridTrack = {
    id: id(`grid-track-${index}`),
    kind: "opaque",
    raw: value,
    repeatCount: null,
    repeatTracks: [],
    structurallyEditable: true,
  };
  if (value.startsWith("[") && value.endsWith("]")) return { ...base, kind: "line_names" };
  if (value.startsWith("$") || value.startsWith("#{") || value.startsWith("var(")) {
    return { ...base, kind: "dynamic", structurallyEditable: false };
  }
  if (["auto", "min-content", "max-content"].includes(normalized)) return { ...base, kind: "keyword" };
  if (numericUnit(normalized, "fr")) return { ...base, kind: "flex" };
  if (isLength(normalized)) return { ...base, kind: "length" };
  const minmax = functionBody(value, "minmax");
  if (minmax !== null) {
    const parts = splitTopLevelCssList(minmax, "comma");
    return { ...base, kind: "minmax", structurallyEditable: parts?.length === 2 };
  }
  if (functionBody(value, "fit-content") !== null) return { ...base, kind: "fit_content" };
  const repeat = functionBody(value, "repeat");
  if (repeat !== null) {
    const parts = splitTopLevelCssList(repeat, "comma");
    const children = parts?.length === 2
      ? (splitTopLevelCssList(parts[1], "space") ?? []).map(parseGridTrack)
      : [];
    return {
      ...base,
      kind: "repeat",
      repeatCount: parts?.length === 2 ? parts[0] : null,
      repeatTracks: children,
      structurallyEditable: children.length > 0 && children.every((track) => track.structurallyEditable),
    };
  }
  return { ...base, structurallyEditable: false };
}

export function parseGridTrackList(raw: string | null | undefined): CssGridTrackList {
  const value = raw?.trim() ?? "";
  if (!value || value === "none") {
    return { raw: value || null, mode: "none", tracks: [], structurallyEditable: true };
  }
  if (value === "subgrid" || value.startsWith("subgrid ")) {
    return { raw: value, mode: "subgrid", tracks: [], structurallyEditable: false };
  }
  if (value === "masonry") {
    return { raw: value, mode: "masonry", tracks: [], structurallyEditable: false };
  }
  const parts = splitTopLevelCssList(value, "space");
  if (!parts) return { raw: value, mode: "opaque", tracks: [], structurallyEditable: false };
  const tracks = parts.map(parseGridTrack);
  return {
    raw: value,
    mode: "tracks",
    tracks,
    structurallyEditable: tracks.every((track) => track.structurallyEditable),
  };
}

export function serializeGridTrack(track: CssGridTrack): string {
  if (track.kind === "repeat" && track.repeatCount && track.repeatTracks.length) {
    return `repeat(${track.repeatCount}, ${track.repeatTracks.map(serializeGridTrack).join(" ")})`;
  }
  return track.raw.trim();
}

export function serializeGridTrackList(value: CssGridTrackList) {
  if (value.mode === "tracks") return value.tracks.map(serializeGridTrack).join(" ");
  return value.raw ?? "";
}

export function createGridTrack(kind: CssGridTrackKind, axis: "columns" | "rows"): CssGridTrack {
  const rawByKind: Record<CssGridTrackKind, string> = {
    keyword: axis === "columns" ? "auto" : "min-content",
    flex: "1fr",
    length: axis === "columns" ? "12rem" : "auto",
    minmax: "minmax(0, 1fr)",
    fit_content: "fit-content(20rem)",
    repeat: "repeat(2, minmax(0, 1fr))",
    line_names: "[linie]",
    dynamic: "$track-grid",
    opaque: "auto",
  };
  return parseGridTrack(rawByKind[kind]);
}

export function cloneGridTrack(track: CssGridTrack): CssGridTrack {
  const cloned = JSON.parse(JSON.stringify(track)) as CssGridTrack;
  cloned.id = id("grid-track-copy");
  cloned.repeatTracks = cloned.repeatTracks.map((child) => ({ ...child, id: id("grid-repeat-track") }));
  return cloned;
}

export function parseGridAreasText(value: string): string[][] {
  return value
    .split(/\r?\n/)
    .map((row) => row.trim().replace(/^['"]|['"]$/g, ""))
    .filter(Boolean)
    .map((row) => row.split(/\s+/));
}

export function serializeGridAreasRows(rows: readonly (readonly string[])[]) {
  return rows.filter((row) => row.length).map((row) => `"${row.join(" ")}"`).join(" ");
}

export type GridAreasValidationError = "rectangular" | "name" | "contiguous";

export function validateGridAreasRows(rows: readonly (readonly string[])[]): GridAreasValidationError | null {
  const width = rows[0]?.length ?? 0;
  if (!width || rows.some((row) => row.length !== width)) return "rectangular";
  const names = new Set(rows.flatMap((row) => row.filter((cell) => !/^\.+$/.test(cell))));
  for (const name of names) {
    if (!/^-?[_a-z][_a-z0-9-]*$/i.test(name)) return "name";
    const cells = rows.flatMap((row, rowIndex) => row.flatMap((cell, columnIndex) => (
      cell === name ? [[rowIndex, columnIndex] as const] : []
    )));
    const rowIndexes = cells.map(([row]) => row);
    const columnIndexes = cells.map(([, column]) => column);
    const minRow = Math.min(...rowIndexes);
    const maxRow = Math.max(...rowIndexes);
    const minColumn = Math.min(...columnIndexes);
    const maxColumn = Math.max(...columnIndexes);
    for (let row = minRow; row <= maxRow; row += 1) {
      for (let column = minColumn; column <= maxColumn; column += 1) {
        if (rows[row]?.[column] !== name) return "contiguous";
      }
    }
  }
  return null;
}

export function gridAreasEditorText(areas: CssGridAreas) {
  if (areas.rows.length) return areas.rows.map((row) => row.join(" ")).join("\n");
  return areas.raw && areas.raw !== "none" ? areas.raw : "";
}

export function gridFromProperties(properties: Readonly<Record<string, string>>): CssGrid {
  const gap = splitTopLevelCssList(properties.gap ?? "", "space") ?? [];
  const areaRaw = properties["grid-template-areas"] ?? "";
  const dynamicAreas = areaRaw.startsWith("$") || areaRaw.startsWith("#{") || areaRaw.startsWith("var(");
  const areaRows = dynamicAreas ? [] : parseGridAreasText(areaRaw.replace(/"\s+"/g, "\n"));
  const areaValidation = areaRows.length ? validateGridAreasRows(areaRows) : null;
  const templateColumns = parseGridTrackList(properties["grid-template-columns"]);
  const templateRows = parseGridTrackList(properties["grid-template-rows"]);
  const opaqueProperties = Object.fromEntries(
    GRID_OPAQUE_PROPERTIES
      .filter((property) => Boolean(properties[property]?.trim()))
      .map((property) => [property, properties[property]]),
  );
  return {
    schemaVersion: CSS_GRID_SCHEMA_VERSION,
    display: properties.display || null,
    templateColumns,
    templateRows,
    templateAreas: {
      raw: properties["grid-template-areas"] || null,
      rows: areaRows,
      valid: areaValidation === null,
      error: areaValidation,
      structurallyEditable: !dynamicAreas && areaValidation === null,
    },
    autoColumns: properties["grid-auto-columns"] || null,
    autoRows: properties["grid-auto-rows"] || null,
    autoFlow: properties["grid-auto-flow"] || null,
    columnGap: properties["column-gap"] || gap[1] || gap[0] || null,
    rowGap: properties["row-gap"] || gap[0] || null,
    justifyContent: properties["justify-content"] || null,
    alignContent: properties["align-content"] || null,
    justifyItems: properties["justify-items"] || null,
    alignItems: properties["align-items"] || null,
    itemColumn: properties["grid-column"] || null,
    itemRow: properties["grid-row"] || null,
    itemArea: properties["grid-area"] || null,
    opaqueProperties,
    structurallyEditable: Object.keys(opaqueProperties).length === 0
      && templateColumns.structurallyEditable
      && templateRows.structurallyEditable
      && !dynamicAreas
      && areaValidation === null,
  };
}

export function gridToProperties(grid: CssGrid): Record<string, string> {
  return {
    display: grid.display ?? "",
    "grid-template-columns": serializeGridTrackList(grid.templateColumns),
    "grid-template-rows": serializeGridTrackList(grid.templateRows),
    "grid-template-areas": grid.templateAreas.rows.length
      ? serializeGridAreasRows(grid.templateAreas.rows)
      : grid.templateAreas.raw ?? "",
    "grid-auto-columns": grid.autoColumns ?? "",
    "grid-auto-rows": grid.autoRows ?? "",
    "grid-auto-flow": grid.autoFlow ?? "",
    "column-gap": grid.columnGap ?? "",
    "row-gap": grid.rowGap ?? "",
    "justify-content": grid.justifyContent ?? "",
    "align-content": grid.alignContent ?? "",
    "justify-items": grid.justifyItems ?? "",
    "align-items": grid.alignItems ?? "",
    "grid-column": grid.itemColumn ?? "",
    "grid-row": grid.itemRow ?? "",
    "grid-area": grid.itemArea ?? "",
    ...grid.opaqueProperties,
  };
}
