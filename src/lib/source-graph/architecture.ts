import type {
  SourceGraph,
  SourceGraphPage,
  SourceGraphTemplate,
  SourceRelationKind,
} from "$lib/types";
import { sourceRelationsTo, sourceTemplateByNodeId } from "$lib/source-graph/view";

export type SitePageTypeKind = "home" | "archive" | "single" | "page";
export type ReusablePartRole = "header" | "footer" | "cta" | "navigation" | "macro" | "partial";

export type SitePageType = {
  id: string;
  kind: SitePageTypeKind;
  label: string;
  description: string;
  file: string;
  url: string | null;
  templateName: string | null;
  nodeId: string;
  examplePageFile?: string;
  section?: string;
};

export type SiteLayout = {
  id: string;
  label: string;
  file: string;
  nodeId: string;
  usedBy: number;
  blocks: string[];
};

export type ReusablePart = {
  id: string;
  role: ReusablePartRole;
  label: string;
  file: string;
  nodeId: string;
  usedBy: number;
};

export type SiteArchitecture = {
  pageTypes: SitePageType[];
  layouts: SiteLayout[];
  reusableParts: ReusablePart[];
};

export function buildSiteArchitecture(graph: SourceGraph | null): SiteArchitecture {
  if (!graph) {
    return { pageTypes: [], layouts: [], reusableParts: [] };
  }

  return {
    pageTypes: buildPageTypes(graph),
    layouts: buildLayouts(graph),
    reusableParts: buildReusableParts(graph),
  };
}

export function reusablePartRoleLabel(role: ReusablePartRole) {
  const labels: Record<ReusablePartRole, string> = {
    header: "Header global",
    footer: "Footer global",
    cta: "CTA reutilizabil",
    navigation: "Navigație",
    macro: "Macro",
    partial: "Partial",
  };
  return labels[role];
}

export function pageTypeKindLabel(kind: SitePageTypeKind) {
  const labels: Record<SitePageTypeKind, string> = {
    home: "Index / Acasă",
    archive: "Arhivă / Secțiune",
    single: "Single dinamic",
    page: "Pagină simplă",
  };
  return labels[kind];
}

function buildPageTypes(graph: SourceGraph): SitePageType[] {
  const result: SitePageType[] = [];
  const pagesByFile = [...graph.pages].sort((left, right) => left.file.localeCompare(right.file));

  for (const page of pagesByFile) {
    if (page.pageKind === "home") {
      result.push({
        id: `home:${page.file}`,
        kind: "home",
        label: "Acasă",
        description: "Index-ul site-ului, pagina de la rădăcină.",
        file: page.file,
        url: page.url,
        templateName: page.resolvedTemplate,
        nodeId: page.id,
      });
    } else if (page.pageKind === "section") {
      const section = sectionNameForPage(page);
      result.push({
        id: `archive:${page.file}`,
        kind: "archive",
        label: section ? `Arhivă ${section}` : page.title,
        description: "Pagină de secțiune/arhivă; aici se listează de obicei articole sau pagini copil.",
        file: page.file,
        url: page.url,
        templateName: page.resolvedTemplate,
        nodeId: page.id,
        section,
      });
    }
  }

  const singleGroups = new Map<string, SourceGraphPage[]>();
  for (const page of pagesByFile.filter((candidate) => candidate.pageKind === "page")) {
    const section = sectionNameForPage(page) || "pagini";
    const template = page.resolvedTemplate ?? "page.html";
    const key = `${section}|${template}`;
    singleGroups.set(key, [...(singleGroups.get(key) ?? []), page]);
  }

  for (const [key, pages] of singleGroups) {
    const [section, templateName] = key.split("|");
    const example = pages[0];
    result.push({
      id: `single:${key}`,
      kind: section === "pagini" ? "page" : "single",
      label: section === "pagini" ? "Pagină simplă" : `Single ${section}`,
      description:
        section === "pagini"
          ? "Template pentru pagini simple individuale."
          : `Template aplicat intrărilor individuale din secțiunea ${section}.`,
      file: example.file,
      url: example.url,
      templateName,
      nodeId: example.templateNodeId ?? example.id,
      examplePageFile: example.file,
      section,
    });
  }

  return result;
}

function buildLayouts(graph: SourceGraph): SiteLayout[] {
  return graph.templates
    .filter((template) => !template.isPartial)
    .map((template) => {
      const usedByExtends = relationCountTo(graph, template.nodeId, "extends");
      const looksLikeLayout = template.name.includes("base") || template.name.includes("layout");
      return { template, usedByExtends, looksLikeLayout };
    })
    .filter(({ usedByExtends, looksLikeLayout, template }) => usedByExtends > 0 || looksLikeLayout || template.blocks.length > 0)
    .map(({ template, usedByExtends }) => ({
      id: `layout:${template.nodeId}`,
      label: template.name.includes("base") ? "Layout global" : template.name,
      file: template.file,
      nodeId: template.nodeId,
      usedBy: usedByExtends,
      blocks: template.blocks,
    }));
}

function buildReusableParts(graph: SourceGraph): ReusablePart[] {
  return graph.templates
    .filter((template) => template.isPartial)
    .map((template) => ({
      id: `part:${template.nodeId}`,
      role: reusableRoleForTemplate(template),
      label: reusableLabelForTemplate(template),
      file: template.file,
      nodeId: template.nodeId,
      usedBy: relationCountTo(graph, template.nodeId, "includes") + relationCountTo(graph, template.nodeId, "imports"),
    }))
    .sort((left, right) => reusablePartRoleLabel(left.role).localeCompare(reusablePartRoleLabel(right.role)) || left.file.localeCompare(right.file));
}

function reusableRoleForTemplate(template: SourceGraphTemplate): ReusablePartRole {
  const name = template.name.toLowerCase();
  if (name.includes("header")) return "header";
  if (name.includes("footer")) return "footer";
  if (name.includes("cta") || name.includes("call-to-action")) return "cta";
  if (name.includes("nav") || name.includes("meniu") || name.includes("menu")) return "navigation";
  if (template.macros.length > 0 || name.startsWith("macros/")) return "macro";
  return "partial";
}

function reusableLabelForTemplate(template: SourceGraphTemplate) {
  const role = reusableRoleForTemplate(template);
  if (role !== "partial" && role !== "macro") return reusablePartRoleLabel(role);
  return template.name;
}

function relationCountTo(graph: SourceGraph, nodeId: string, kind: SourceRelationKind) {
  return sourceRelationsTo(graph, nodeId, kind).length;
}

function sectionNameForPage(page: SourceGraphPage) {
  const contentPath = page.file.replace(/^sursa\/content\//, "");
  if (contentPath === "_index.md") return "";
  const parts = contentPath.split("/");
  if (page.pageKind === "section" && parts.at(-1) === "_index.md") {
    return parts.slice(0, -1).join("/") || "";
  }
  if (parts.length > 1) return parts[0];
  return "";
}

export function templateForPageType(graph: SourceGraph | null, pageType: SitePageType): SourceGraphTemplate | null {
  if (!graph || !pageType.nodeId) return null;
  return sourceTemplateByNodeId(graph, pageType.nodeId);
}
