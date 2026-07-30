import assert from "node:assert/strict";
import { test } from "node:test";
import {
  planFileExplorerEntryReveal,
  projectFileExplorerRows,
} from "$lib/project/file-explorer-view";

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
