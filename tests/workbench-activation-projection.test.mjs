import assert from "node:assert/strict";
import { test } from "node:test";
import {
  patchExactWorkbenchActivityChange,
  patchExactWorkbenchDocumentActivation,
} from "$lib/workbench/activation-projection";

function snapshot(overrides = {}) {
  return {
    schemaVersion: 2,
    projectRoot: "/tmp/project",
    projectSessionId: "project-session",
    runtimeSessionId: "runtime-session",
    revision: 4,
    activeActivity: "editor",
    activeGroupId: "primary",
    split: "none",
    splitRatioBasisPoints: 5_000,
    canvasViewport: {
      mode: "fit",
      preset: "desktop",
      widthPx: 1_440,
      zoomPercent: 100,
      showRulers: true,
    },
    groups: [
      {
        groupId: "primary",
        activeDocumentId: "document:config.toml",
        documents: [
          {
            documentId: "document:config.toml",
            relativePath: "config.toml",
            title: "config.toml",
            presentation: "code_only",
            surface: "code",
            pinned: false,
          },
          {
            documentId: "document:templates/index.html",
            relativePath: "templates/index.html",
            title: "index.html",
            presentation: "html",
            surface: "visual",
            pinned: false,
          },
        ],
      },
      {
        groupId: "secondary",
        activeDocumentId: null,
        documents: [],
      },
    ],
    bottomPanel: { open: false, activeView: "terminal" },
    contentWorkspace: { mode: "list", pagePath: null },
    selectedProjectEntry: { relativePath: "config.toml", kind: "text" },
    ...overrides,
  };
}

function activationReceiptSnapshot() {
  const next = structuredClone(snapshot({ revision: 5 }));
  next.groups[0].activeDocumentId = "document:templates/index.html";
  next.selectedProjectEntry = { relativePath: "templates/index.html", kind: "text" };
  return next;
}

const intent = {
  kind: "activate_document",
  groupId: "primary",
  documentId: "document:templates/index.html",
};

test("activarea exactă păstrează identitatea snapshotului și mută numai delta Rust", () => {
  const current = snapshot();
  const rootIdentity = current;
  const groupsIdentity = current.groups;
  const documentsIdentity = current.groups[0].documents;

  assert.equal(
    patchExactWorkbenchDocumentActivation(current, activationReceiptSnapshot(), intent),
    true,
  );
  assert.equal(current, rootIdentity);
  assert.equal(current.groups, groupsIdentity);
  assert.equal(current.groups[0].documents, documentsIdentity);
  assert.equal(current.revision, 5);
  assert.equal(current.groups[0].activeDocumentId, intent.documentId);
  assert.deepEqual(current.selectedProjectEntry, {
    relativePath: "templates/index.html",
    kind: "text",
  });
});

test("orice diferență structurală refuză patch-ul localizat fără efecte parțiale", () => {
  for (const mutate of [
    (next) => { next.activeActivity = "templates"; },
    (next) => { next.canvasViewport.zoomPercent = 90; },
    (next) => { next.groups[0].documents[1].surface = "code"; },
    (next) => { next.groups[0].documents[1].presentation = "code_only"; },
    (next) => { next.groups[1].activeDocumentId = "foreign"; },
    (next) => { next.selectedProjectEntry.relativePath = "foreign.html"; },
  ]) {
    const current = snapshot();
    const before = structuredClone(current);
    const next = activationReceiptSnapshot();
    mutate(next);
    assert.equal(patchExactWorkbenchDocumentActivation(current, next, intent), false);
    assert.deepEqual(current, before);
  }
});

test("schimbarea exactă a activității păstrează proiecțiile Workbench nemodificate", () => {
  const current = snapshot();
  const rootIdentity = current;
  const groupsIdentity = current.groups;
  const next = snapshot({ revision: 5, activeActivity: "templates" });

  assert.equal(
    patchExactWorkbenchActivityChange(current, next, {
      kind: "set_activity",
      activity: "templates",
    }),
    true,
  );
  assert.equal(current, rootIdentity);
  assert.equal(current.groups, groupsIdentity);
  assert.equal(current.revision, 5);
  assert.equal(current.activeActivity, "templates");
});

test("no-op-ul activității nu avansează revizia", () => {
  const current = snapshot();

  assert.equal(
    patchExactWorkbenchActivityChange(current, structuredClone(current), {
      kind: "set_activity",
      activity: "editor",
    }),
    true,
  );
  assert.equal(current.revision, 4);
});

test("intrarea în Content proiectează numai resetul confirmat de Rust", () => {
  const current = snapshot({
    contentWorkspace: { mode: "edit", pagePath: "content/post.md" },
  });
  const contentIdentity = current.contentWorkspace;
  const next = structuredClone(current);
  next.revision = 5;
  next.activeActivity = "content";
  next.contentWorkspace = { mode: "list", pagePath: null };

  assert.equal(
    patchExactWorkbenchActivityChange(current, next, {
      kind: "set_activity",
      activity: "content",
    }),
    true,
  );
  assert.equal(current.contentWorkspace, contentIdentity);
  assert.deepEqual(current.contentWorkspace, { mode: "list", pagePath: null });
});

test("diferențele străine activității refuză patch-ul fără mutații parțiale", () => {
  for (const mutate of [
    (next) => { next.runtimeSessionId = "foreign-session"; },
    (next) => { next.revision = 6; },
    (next) => { next.groups[0].documents[0].title = "foreign"; },
    (next) => { next.bottomPanel.open = true; },
    (next) => { next.contentWorkspace = { mode: "edit", pagePath: "foreign.md" }; },
  ]) {
    const current = snapshot();
    const before = structuredClone(current);
    const next = snapshot({ revision: 5, activeActivity: "templates" });
    mutate(next);
    assert.equal(
      patchExactWorkbenchActivityChange(current, next, {
        kind: "set_activity",
        activity: "templates",
      }),
      false,
    );
    assert.deepEqual(current, before);
  }
});
