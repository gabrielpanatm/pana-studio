import assert from "node:assert/strict";
import { test } from "node:test";
import {
  planFileExplorerEntryReveal,
  projectFileExplorerRows,
} from "$lib/project/file-explorer-view";
import {
  projectFileExplorerScrollTopForIndex,
  projectFileExplorerVirtualWindow,
} from "$lib/project/file-explorer-virtualization";

function directory(id, relativePath, parentId = null) {
  return {
    id,
    relativePath,
    parentId,
    kind: "directory",
  };
}

function file(id, relativePath, parentId) {
  return {
    id,
    relativePath,
    parentId,
    kind: "text",
  };
}

test("active Workbench document expands its Explorer ancestors independently of directory selection", () => {
  const entries = [
    directory("templates", "templates"),
    directory("partials", "templates/partials", "templates"),
    file("index", "templates/partials/index.html", "partials"),
  ];
  const collapsed = new Set(["templates", "templates/partials"]);

  const plan = planFileExplorerEntryReveal(
    entries,
    collapsed,
    "templates/partials/index.html",
  );

  assert.ok(plan);
  assert.equal(plan.entryId, "index");
  assert.deepEqual([...plan.collapsedDirs], []);
  assert.deepEqual([...collapsed], ["templates", "templates/partials"]);
});

test("revealing a selected directory expands only its ancestors", () => {
  const entries = [
    directory("templates", "templates"),
    directory("partials", "templates/partials", "templates"),
  ];
  const collapsed = new Set(["templates", "templates/partials"]);

  const plan = planFileExplorerEntryReveal(
    entries,
    collapsed,
    "templates/partials",
  );

  assert.ok(plan);
  assert.deepEqual([...plan.collapsedDirs], ["templates/partials"]);
});

test("folder expansion and visible children are projected atomically", () => {
  const entries = [
    directory("templates", "templates"),
    file("index", "templates/index.html", "templates"),
    file("layout", "templates/layout.html", "templates"),
    file("page", "templates/page.html", "templates"),
    directory("partials", "templates/partials", "templates"),
  ];

  const reveal = planFileExplorerEntryReveal(
    entries,
    new Set(["templates"]),
    "templates/page.html",
  );
  assert.ok(reveal);

  const rows = projectFileExplorerRows(entries, reveal.collapsedDirs);
  const templates = rows.find((row) => row.path === "templates");
  assert.equal(templates?.hasChildren, true);
  assert.equal(templates?.expanded, true);
  assert.deepEqual(
    rows.map((row) => row.path),
    [
      "templates",
      "templates/index.html",
      "templates/layout.html",
      "templates/page.html",
      "templates/partials",
    ],
  );
});

test("collapsed and empty folders cannot advertise a false open state", () => {
  const entries = [
    directory("templates", "templates"),
    file("page", "templates/page.html", "templates"),
  ];
  const collapsedRows = projectFileExplorerRows(entries, new Set(["templates"]));

  assert.equal(collapsedRows[0]?.hasChildren, true);
  assert.equal(collapsedRows[0]?.expanded, false);
  assert.deepEqual(collapsedRows.map((row) => row.path), ["templates"]);

  const emptyRows = projectFileExplorerRows(
    [directory("empty", "empty")],
    new Set(),
  );
  assert.equal(emptyRows[0]?.hasChildren, false);
  assert.equal(emptyRows[0]?.expanded, false);
});

test("large Explorer trees materialize a bounded viewport and preserve total height", () => {
  const first = projectFileExplorerVirtualWindow(991, 0, 500, 25, 10);
  assert.deepEqual(first, {
    start: 0,
    end: 30,
    topSpacerPx: 0,
    bottomSpacerPx: 24_025,
  });

  const middle = projectFileExplorerVirtualWindow(991, 12_000, 500, 25, 10);
  assert.equal(middle.end - middle.start, 40);
  assert.equal(
    middle.topSpacerPx
      + (middle.end - middle.start) * 25
      + middle.bottomSpacerPx,
    991 * 25,
  );

  const last = projectFileExplorerVirtualWindow(991, 24_700, 500, 25, 10);
  assert.equal(last.end, 991);
  assert.equal(last.bottomSpacerPx, 0);
});

test("Explorer reveal computes a local no-op or the smallest bounded scroll", () => {
  assert.equal(projectFileExplorerScrollTopForIndex(4, 0, 500, 25), 0);
  assert.equal(projectFileExplorerScrollTopForIndex(22, 0, 500, 25), 75);
  assert.equal(projectFileExplorerScrollTopForIndex(10, 500, 500, 25), 250);
  assert.equal(projectFileExplorerScrollTopForIndex(400, 0, 0, 25), 10_000);
});
