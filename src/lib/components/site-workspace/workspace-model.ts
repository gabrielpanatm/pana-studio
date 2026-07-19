import type {
  ScssVariable,
  SourceGraphPage,
  SourceGraphStyle,
  SourcePageKind,
} from "$lib/types";

export type SiteWorkspaceSection = "overview" | "pages" | "structure" | "design" | "sources";

export type DesignTokenGroupId =
  | "colors"
  | "typography"
  | "spacing"
  | "radius"
  | "layout"
  | "other";

export type DesignTokenGroup = {
  id: DesignTokenGroupId;
  label: string;
  description: string;
  variables: ScssVariable[];
};

export type SitePageListItem = {
  page: SourceGraphPage;
  depth: number;
  routeLabel: string;
};

export function pageCountLabel(count: number) {
  return count === 1 ? "1 pagină" : `${count} pagini`;
}

export function pageKindLabel(kind: SourcePageKind) {
  if (kind === "home") return "Pagina principală";
  if (kind === "section") return "Secțiune";
  return "Pagină";
}

export function pageKindShortLabel(kind: SourcePageKind) {
  if (kind === "home") return "Acasă";
  if (kind === "section") return "Secțiune";
  return "Pagină";
}

export function pageTemplateLabel(page: SourceGraphPage) {
  return page.resolvedTemplate ?? page.frontmatterTemplate ?? "Template implicit";
}

export function pageRouteDepth(url: string) {
  return Math.max(0, url.split("/").filter(Boolean).length - 1);
}

export function pageRouteLabel(url: string) {
  return url || "/";
}

export function sitePageList(pages: SourceGraphPage[]): SitePageListItem[] {
  return [...pages]
    .sort((left, right) => {
      if (left.pageKind === "home" && right.pageKind !== "home") return -1;
      if (right.pageKind === "home" && left.pageKind !== "home") return 1;
      return left.url.localeCompare(right.url, "ro");
    })
    .map((page) => ({
      page,
      depth: pageRouteDepth(page.url),
      routeLabel: pageRouteLabel(page.url),
    }));
}

export function styleScopeLabel(scope: SourceGraphStyle["scope"]) {
  if (scope === "global") return "Întregul site";
  if (scope === "page") return "Pagină";
  if (scope === "partial") return "Componentă reutilizabilă";
  return "Alte stiluri";
}

export function sourceDisplayPath(path: string) {
  return path.replace(/^sursa\//, "");
}

export function sourceFileName(path: string) {
  return path.split("/").filter(Boolean).at(-1) ?? path;
}

export function variableLooksLikeColor(variable: ScssVariable) {
  const name = variable.name.toLowerCase();
  const value = variable.value.trim().toLowerCase();
  return name.includes("color")
    || name.startsWith("bg-")
    || /^#[0-9a-f]{3,8}$/i.test(value)
    || /^(rgb|rgba|hsl|hsla|oklch|oklab|color|color-mix)\(/i.test(value);
}

export function safeCssPreviewValue(value: string) {
  const candidate = value.trim();
  if (!candidate || /[;{}<>]/.test(candidate)) return "";
  return candidate;
}

export function colorPreviewValue(value: string) {
  const candidate = safeCssPreviewValue(value);
  if (!candidate) return "transparent";
  if (
    /^#[0-9a-f]{3,8}$/i.test(candidate)
    || /^(rgb|rgba|hsl|hsla|oklch|oklab|color|color-mix)\(/i.test(candidate)
    || /^(transparent|currentcolor)$/i.test(candidate)
  ) return candidate;
  return "transparent";
}

export function editableHexColor(value: string) {
  const candidate = value.trim();
  if (/^#[0-9a-f]{6}$/i.test(candidate)) return candidate;
  if (/^#[0-9a-f]{3}$/i.test(candidate)) {
    return `#${candidate.slice(1).split("").map((part) => `${part}${part}`).join("")}`;
  }
  return null;
}

export function tokenHumanLabel(name: string) {
  const normalized = name
    .replace(/^\$/, "")
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
  return normalized || name;
}

export function buildDesignTokenGroups(variables: ScssVariable[]): DesignTokenGroup[] {
  const groups: DesignTokenGroup[] = [
    { id: "colors", label: "Culori", description: "Paleta folosită în întregul site", variables: [] },
    { id: "typography", label: "Tipografie", description: "Fonturi, dimensiuni și ritm de text", variables: [] },
    { id: "spacing", label: "Spațiere", description: "Distanțe și ritm vizual", variables: [] },
    { id: "radius", label: "Colțuri", description: "Rotunjirea suprafețelor și componentelor", variables: [] },
    { id: "layout", label: "Layout", description: "Containere, breakpoints și straturi", variables: [] },
    { id: "other", label: "Alte token-uri", description: "Valori tehnice suplimentare", variables: [] },
  ];
  const byId = new Map(groups.map((group) => [group.id, group]));

  for (const variable of variables) {
    const name = variable.name.toLowerCase();
    if (variableLooksLikeColor(variable)) {
      byId.get("colors")?.variables.push(variable);
    } else if (name.includes("font") || name.includes("text") || name.includes("leading") || name.includes("tracking")) {
      byId.get("typography")?.variables.push(variable);
    } else if (name.includes("space") || name.includes("gap") || name.includes("padding") || name.includes("margin")) {
      byId.get("spacing")?.variables.push(variable);
    } else if (name.includes("radius")) {
      byId.get("radius")?.variables.push(variable);
    } else if (name.includes("container") || name.startsWith("bp-") || name.includes("breakpoint") || name.startsWith("z-")) {
      byId.get("layout")?.variables.push(variable);
    } else {
      byId.get("other")?.variables.push(variable);
    }
  }

  return groups.filter((group) => group.variables.length > 0);
}
