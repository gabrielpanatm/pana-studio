import type { SourceGraphPage, SourcePageKind } from "$lib/types";

export type SiteOverviewPage = {
  page: SourceGraphPage;
  depth: number;
  routeLabel: string;
};

export function pageRouteDepth(url: string) {
  return Math.max(0, url.split("/").filter(Boolean).length - 1);
}

export function siteOverviewPages(pages: SourceGraphPage[]): SiteOverviewPage[] {
  return [...pages]
    .sort((left, right) => {
      if (left.pageKind === "home" && right.pageKind !== "home") return -1;
      if (right.pageKind === "home" && left.pageKind !== "home") return 1;
      return left.url.localeCompare(right.url, "ro");
    })
    .map((page) => ({
      page,
      depth: pageRouteDepth(page.url),
      routeLabel: page.url || "/",
    }));
}

export function pageKindLabel(kind: SourcePageKind) {
  if (kind === "home") return "Acasă";
  if (kind === "section") return "Secțiune";
  return "Pagină";
}

export function pageTemplateLabel(page: SourceGraphPage) {
  return page.resolvedTemplate ?? page.frontmatterTemplate ?? "Template implicit";
}

export function sourceDisplayPath(path: string) {
  return path;
}
