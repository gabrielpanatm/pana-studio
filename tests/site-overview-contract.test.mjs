import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import {
  pageRouteDepth,
  siteOverviewPages,
} from "$lib/site/overview";

function page(overrides) {
  return {
    id: overrides.id,
    file: overrides.file ?? `content/${overrides.id}.md`,
    title: overrides.title,
    url: overrides.url,
    pageKind: overrides.pageKind ?? "page",
    frontmatterTemplate: overrides.frontmatterTemplate ?? null,
    frontmatterPageTemplate: null,
    resolvedTemplate: overrides.resolvedTemplate ?? null,
    contentNodeId: `${overrides.id}:content`,
    templateNodeId: overrides.templateNodeId ?? null,
    pageTemplateNodeId: null,
  };
}

test("overview-ul Site pune Acasă prima și păstrează ierarhia rutelor", () => {
  const items = siteOverviewPages([
    page({ id: "contact", title: "Contact", url: "/contact/" }),
    page({ id: "article", title: "Articol", url: "/blog/articol/" }),
    page({ id: "home", title: "Acasă", url: "/", pageKind: "home" }),
    page({ id: "blog", title: "Blog", url: "/blog/", pageKind: "section" }),
  ]);

  assert.deepEqual(items.map((item) => item.page.id), ["home", "blog", "article", "contact"]);
  assert.equal(pageRouteDepth("/"), 0);
  assert.equal(pageRouteDepth("/blog/"), 0);
  assert.equal(pageRouteDepth("/blog/articol/"), 1);
});

test("Site este orchestrator peste Source Graph și activitățile canonice", () => {
  const overview = readFileSync(
    new URL("../src/lib/components/site/SiteOverviewWorkspace.svelte", import.meta.url),
    "utf8",
  );
  const center = readFileSync(
    new URL("../src/lib/components/workspace/WorkspaceCenterArea.svelte", import.meta.url),
    "utf8",
  );

  assert.match(overview, /app\.sourceGraph/);
  assert.match(overview, /setWorkbenchActivity\(\"editor\"\)/);
  for (const activity of ["content", "components", "design_system", "assets", "audit", "publish"]) {
    assert.match(overview, new RegExp(`(?:activity:\\s*|navigate\\()\\"${activity}\\"`));
  }
  for (const duplicate of [
    "SiteWorkspaceSidebar",
    "SitePagesPanel",
    "SiteStructurePanel",
    "SiteDesignPanel",
    "SiteSourcesPanel",
    "readSiteWorkspaceGraph",
    "activeSection",
    "openAction",
  ]) {
    assert.doesNotMatch(overview, new RegExp(duplicate));
  }
  assert.match(center, /SiteOverviewWorkspace/);
  assert.doesNotMatch(center, /site-workspace\/SiteWorkspace/);
});

test("componentele vechii arhitecturi Site au fost eliminate", () => {
  for (const relativePath of [
    "SiteWorkspace.svelte",
    "SiteWorkspaceSidebar.svelte",
    "SitePagesPanel.svelte",
    "SiteStructurePanel.svelte",
    "SiteDesignPanel.svelte",
    "SiteSourcesPanel.svelte",
  ]) {
    assert.equal(
      existsSync(new URL(`../src/lib/components/site-workspace/${relativePath}`, import.meta.url)),
      false,
      `${relativePath} nu trebuie să mai existe`,
    );
  }
});

test("ProjectPane nu mai modelează Adaugă element ca tab ascuns", () => {
  const projectPane = readFileSync(
    new URL("../src/lib/components/ProjectPane.svelte", import.meta.url),
    "utf8",
  );
  const types = readFileSync(new URL("../src/lib/types.ts", import.meta.url), "utf8");

  assert.doesNotMatch(types, /ProjectPaneTab\s*=.*\"structure\"/);
  assert.doesNotMatch(projectPane, /projectPaneTab\s*===\s*\"structure\"/);
  assert.match(projectPane, /role=\"dialog\"/);
  assert.match(projectPane, /aria-expanded=\{elementPaletteOpen\}/);
  assert.match(projectPane, /event\.key === \"Escape\"/);
  assert.match(projectPane, /elementPaletteTrigger\?\.focus\(\)/);
});
