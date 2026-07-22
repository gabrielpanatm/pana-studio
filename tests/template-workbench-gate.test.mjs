import assert from "node:assert/strict";
import { test } from "node:test";
import { buildLayerRows } from "$lib/project/layers-tree";
import {
  TERA_GATE_DROP_BLOCKED_MESSAGE,
  teraGateDropStatus,
} from "$lib/source-graph/interaction";

function capabilities() {
  return {
    canEditVisual: true,
    canMove: true,
    canDelete: true,
    canEditSource: true,
    reason: null,
  };
}

function node(id, kind, file, parent = null, label = id) {
  return {
    id,
    kind,
    file,
    parent,
    label,
    origin: "local",
    themeName: null,
    range: null,
    capabilities: capabilities(),
  };
}

function graphFixture() {
  const nodes = [
    node("template-index", "template", "templates/index.html"),
    node("block-index", "block", "templates/index.html", "template-index", "content"),
    node("include-card", "include", "templates/index.html", "block-index", "include partials/card.html"),
    node("template-card", "partial", "templates/partials/card.html"),
    node("card-html", "html", "templates/partials/card.html", "template-card", "<article .card>"),
    node("card-title", "html", "templates/partials/card.html", "card-html", "<h2>"),
  ];
  return {
    schemaVersion: 1,
    revision: "graph-1",
    nodes,
    relations: [{ from: "template-index", to: "template-card", kind: "includes" }],
    pages: [],
    templates: [
      {
        nodeId: "template-index",
        file: "templates/index.html",
        name: "index.html",
        origin: "local",
        themeName: null,
        isPartial: false,
        macros: [],
      },
      {
        nodeId: "template-card",
        file: "templates/partials/card.html",
        name: "partials/card.html",
        origin: "local",
        themeName: null,
        isPartial: true,
        macros: [],
      },
    ],
  };
}

test("Tera Gate exposes an included template intentionally but never grants cross-source transfer", () => {
  const graph = graphFixture();
  const closed = teraGateDropStatus(
    graph,
    {
      activeScannedPath: "templates/index.html",
      activeTemplateFiles: ["templates/index.html"],
      openedGateSourceId: null,
    },
    { targetSourceId: "card-html", targetTemplateSourceId: "template-card" },
  );
  assert.deepEqual(closed, {
    allowed: false,
    message: TERA_GATE_DROP_BLOCKED_MESSAGE,
  });

  const opened = teraGateDropStatus(
    graph,
    {
      activeScannedPath: "templates/index.html",
      activeTemplateFiles: ["templates/index.html"],
      openedGateSourceId: "template-card",
    },
    { targetSourceId: "card-html", targetTemplateSourceId: "template-card" },
  );
  assert.deepEqual(opened, { allowed: true });

  // This verdict only opens selection/drop targeting in the canvas. The
  // authoritative Rust move engine still rejects moving HTML from index.html
  // into partials/card.html; that contract is covered by its cross-source test.
});

test("an active Workbench template shows its HTML directly and hides Tera block syntax", () => {
  const graph = graphFixture();
  const nodesById = new Map(graph.nodes.map((candidate) => [candidate.id, candidate]));
  const sections = [
    {
      id: "layer-card",
      selector: "article.card",
      tag: "article",
      label: "Card",
      depth: 0,
      hasChildren: true,
      sourceId: "card-html",
      templateSourceId: "template-card",
    },
    {
      id: "layer-title",
      selector: "article.card > h2",
      tag: "h2",
      label: "Titlu",
      depth: 1,
      hasChildren: false,
      sourceId: "card-title",
      templateSourceId: "template-card",
    },
  ];
  const rows = buildLayerRows(sections, nodesById, {
    sourceGraph: graph,
    gateOpenContext: {
      activeScannedPath: "templates/partials/card.html",
      activeTemplateFiles: ["templates/partials/card.html"],
      openedGateSourceId: null,
    },
    templateWorkbenchPlan: {
      activeTemplate: { file: "templates/partials/card.html" },
    },
  });

  assert.deepEqual(rows.map((row) => row.kind), ["html", "html"]);
  assert.deepEqual(rows.map((row) => row.node.tag), ["article", "h2"]);
  assert.equal(rows.some((row) => row.kind === "tera" && row.sourceNode?.kind === "block"), false);
});
