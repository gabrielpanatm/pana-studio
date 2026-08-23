import assert from "node:assert/strict";
import { test } from "node:test";
import { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import { ProjectSessionState } from "$lib/project/session-state.svelte";

function file(relativePath, role) {
  return { name: relativePath.split("/").at(-1), relativePath, role, kind: "FILE" };
}

test("catalogul documentelor traversează o singură dată fiecare ProjectScan", () => {
  const session = new ProjectSessionState();
  session.project = {
    root: "/project",
    files: [
      file("content/_index.md", "page"),
      file("templates/index.html", "template"),
      file("static/logo.svg", "asset"),
    ],
  };
  const documents = new ProjectDocumentWorkspaceState({ session, sourceGraph: () => null });

  const pages = documents.scannedPages;
  assert.equal(documents.scannedPages, pages);
  assert.deepEqual(pages.map((entry) => entry.relativePath), ["content/_index.md"]);
  assert.deepEqual(documents.scannedTemplates.map((entry) => entry.relativePath), ["templates/index.html"]);

  session.project = {
    ...session.project,
    files: [...session.project.files, file("content/about.md", "page")],
  };
  assert.notEqual(documents.scannedPages, pages);
  assert.deepEqual(
    documents.scannedPages.map((entry) => entry.relativePath),
    ["content/_index.md", "content/about.md"],
  );
});
