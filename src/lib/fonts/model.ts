import type { FontInventory, LocalFontFamily, ScssVariable } from "$lib/types";

export type FontRoleId = "body" | "display" | "ui" | "mono";

export type FontRoleRow = {
  id: FontRoleId;
  label: string;
  description: string;
  variable: ScssVariable | null;
  dirty: boolean;
};

export function buildFontRoleRows(
  variables: ScssVariable[],
): FontRoleRow[] {
  return [
    {
      id: "body",
      label: "Text",
      description: "Fontul principal pentru paragrafe și conținut.",
      variable: findFontVariable(variables, ["font-primary", "font-base", "font-body", "font-text", "font-sans"]),
      dirty: false,
    },
    {
      id: "display",
      label: "Titluri",
      description: "Font pentru heading-uri, hero și zone editoriale.",
      variable: findFontVariable(variables, ["font-display", "font-heading", "font-title", "font-headings"]),
      dirty: false,
    },
    {
      id: "ui",
      label: "UI",
      description: "Font pentru butoane, meniuri și componente mici.",
      variable: findFontVariable(variables, ["font-ui", "font-interface", "font-control"]),
      dirty: false,
    },
    {
      id: "mono",
      label: "Mono",
      description: "Font pentru cod, badge-uri tehnice și valori.",
      variable: findFontVariable(variables, ["font-mono", "font-code", "font-monospace"]),
      dirty: false,
    },
  ];
}

export function missingFontRoles(rows: FontRoleRow[]) {
  return rows.filter((row) => !row.variable);
}

export function cleanFontStack(value: string | null | undefined) {
  if (!value) return "";
  return value
    .split(",")
    .map((part) => part.trim().replace(/^['"]|['"]$/g, ""))
    .filter(Boolean)
    .join(", ");
}

export function primaryFontFamily(value: string | null | undefined) {
  const stack = cleanFontStack(value);
  return stack.split(",")[0]?.trim() ?? "";
}

export function familyMatchesVariable(family: LocalFontFamily, variable: ScssVariable | null | undefined) {
  if (!variable) return false;
  const primary = normalizeFontName(primaryFontFamily(variable.value));
  if (!primary) return false;
  return normalizeFontName(family.family) === primary;
}

export function inventoryFontCount(inventory: FontInventory | null) {
  return inventory?.families.reduce((total, family) => total + family.files.length, 0) ?? 0;
}

export function fontRootLabel(origin: "local" | "theme", themeName: string | null) {
  if (origin === "theme") return themeName ? `Temă: ${themeName}` : "Temă";
  return "Local";
}

export function fontStackForFamily(roleId: FontRoleId, family: string, currentValue: string | null | undefined) {
  const quotedFamily = quoteFontFamily(family);
  const fallback = fallbackStackForRole(roleId);
  const existing = cleanFontStack(currentValue);
  const parts = existing.split(",").map((part) => part.trim()).filter(Boolean);
  const genericStart = parts.findIndex((part) => isGenericFontFamily(part));
  const preservedFallback = genericStart >= 0 ? parts.slice(genericStart).join(", ") : fallback;
  return `${quotedFamily}, ${preservedFallback || fallback}`;
}

export function fontFaceTargetForVariable(variable: ScssVariable | null | undefined) {
  if (!variable) return null;
  const projectRelativeFile = toProjectRelativeZolaFile(variable.file);
  if (projectRelativeFile.endsWith("/css-framework/_variabile.scss")) {
    return projectRelativeFile.replace(/\/css-framework\/_variabile\.scss$/, "/css-framework/_baza.scss");
  }
  if (projectRelativeFile.endsWith("/_variabile.scss")) {
    return projectRelativeFile.replace(/\/_variabile\.scss$/, "/_baza.scss");
  }
  return null;
}

function toProjectRelativeZolaFile(file: string) {
  const normalized = file.replace(/^\/+/, "");
  return normalized;
}

function findFontVariable(variables: ScssVariable[], candidates: string[]) {
  const normalized = variables.map((variable) => ({ variable, name: variable.name.toLowerCase() }));
  for (const candidate of candidates) {
    const exact = normalized.find((entry) => entry.name === candidate);
    if (exact) return exact.variable;
  }
  return normalized.find((entry) => candidates.some((candidate) => entry.name.includes(candidate.replace(/^font-/, ""))))?.variable ?? null;
}

function quoteFontFamily(family: string) {
  return `'${family.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}

function fallbackStackForRole(roleId: FontRoleId) {
  if (roleId === "mono") return "'SF Mono', SFMono-Regular, Consolas, monospace";
  return "system-ui, sans-serif";
}

function isGenericFontFamily(value: string) {
  return [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
  ].includes(value.toLowerCase().replace(/^['"]|['"]$/g, ""));
}

function normalizeFontName(value: string) {
  return value
    .toLowerCase()
    .replace(/^['"]|['"]$/g, "")
    .replace(/[^a-z0-9]+/g, "")
    .trim();
}
