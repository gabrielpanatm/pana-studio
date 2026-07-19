import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  buildDesignTokenGroups,
  colorPreviewValue,
  editableHexColor,
  pageRouteDepth,
  sitePageList,
  tokenHumanLabel,
} from "$lib/components/site-workspace/workspace-model";
import {
  sourceStylesForPage,
  sourceTemplateChainForPage,
} from "$lib/source-graph/view";

function page(overrides) {
  return {
    id: overrides.id,
    file: overrides.file ?? `sursa/content/${overrides.id}.md`,
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

test("navigatorul website pune Acasă prima și păstrează ierarhia rutelor", () => {
  const items = sitePageList([
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

test("style book grupează token-urile și expune culori sigure vizual", () => {
  const variables = [
    { file: "sursa/sass/_variabile.scss", name: "color-primary", value: "#3b82f6", line: 1 },
    { file: "sursa/sass/_variabile.scss", name: "font-display", value: "'Display', sans-serif", line: 2 },
    { file: "sursa/sass/_variabile.scss", name: "space-m", value: "1rem", line: 3 },
    { file: "sursa/sass/_variabile.scss", name: "radius-l", value: ".75rem", line: 4 },
  ];
  const groups = buildDesignTokenGroups(variables);

  assert.deepEqual(groups.map((group) => group.id), ["colors", "typography", "spacing", "radius"]);
  assert.equal(colorPreviewValue("#3b82f6"), "#3b82f6");
  assert.equal(colorPreviewValue("red; display:none"), "transparent");
  assert.equal(editableHexColor("#abc"), "#aabbcc");
  assert.equal(tokenHumanLabel("color-primary-dark"), "Color Primary Dark");
});

test("pagina își păstrează relația canonică spre template, layout și stil", () => {
  const activePage = page({
    id: "home",
    title: "Acasă",
    url: "/",
    pageKind: "home",
    resolvedTemplate: "index.html",
    templateNodeId: "template:index",
  });
  const graph = {
    projectRoot: "/project",
    zolaRoot: "/project/sursa",
    activeTheme: null,
    pages: [activePage],
    templates: [
      {
        id: "index",
        nodeId: "template:index",
        file: "sursa/templates/index.html",
        name: "index.html",
        origin: "local",
        themeName: null,
        isPartial: false,
        extends: "layout.html",
        includes: [], imports: [], getPages: [], getSections: [], internalLinks: [], assetUrls: [], assetHashes: [], dataLoads: [], imageMetadata: [], imageResizes: [], blocks: ["content"], macros: [],
      },
      {
        id: "layout",
        nodeId: "template:layout",
        file: "sursa/templates/layout.html",
        name: "layout.html",
        origin: "local",
        themeName: null,
        isPartial: false,
        extends: null,
        includes: [], imports: [], getPages: [], getSections: [], internalLinks: [], assetUrls: [], assetHashes: [], dataLoads: [], imageMetadata: [], imageResizes: [], blocks: ["body"], macros: [],
      },
    ],
    styles: [{ id: "style:index", nodeId: "style:index", file: "sursa/sass/pagini/index.scss", origin: "local", themeName: null, scope: "page" }],
    scripts: [], assets: [], dataFiles: [], nodes: [], diagnostics: [],
    relations: [
      { id: "extends", from: "template:index", to: "template:layout", kind: "extends", label: "layout.html" },
      { id: "style", from: "home", to: "style:index", kind: "usesStyle", label: "index.scss" },
    ],
  };

  assert.deepEqual(sourceTemplateChainForPage(graph, activePage).map((template) => template.name), ["index.html", "layout.html"]);
  assert.deepEqual(sourceStylesForPage(graph, activePage).map((style) => style.file), ["sursa/sass/pagini/index.scss"]);
});

test("vechiul template builder drag-and-drop nu mai există în Site Workspace", () => {
  const workspace = readFileSync(new URL("../src/lib/components/site-workspace/SiteWorkspace.svelte", import.meta.url), "utf8");
  const sources = readFileSync(new URL("../src/lib/components/site-workspace/SiteSourcesPanel.svelte", import.meta.url), "utf8");
  const actions = readFileSync(new URL("../src/lib/source-graph/workspace-actions.ts", import.meta.url), "utf8");

  for (const source of [workspace, sources, actions]) {
    assert.doesNotMatch(source, /draggable\s*=/);
    assert.doesNotMatch(source, /ondrag(?:start|over|end)\s*=/);
    assert.doesNotMatch(source, /ondrop\s*=/);
    assert.doesNotMatch(source, /includeDraggedPartial/);
    assert.doesNotMatch(source, /Trage componenta aici/);
  }
  assert.match(workspace, /SiteWorkspaceSidebar/);
  assert.match(workspace, /SiteOverviewPanel/);
  assert.match(workspace, /SitePagesPanel/);
  assert.match(workspace, /SiteStructurePanel/);
  assert.match(workspace, /SiteDesignPanel/);
  assert.match(workspace, /SiteSourcesPanel/);
});
