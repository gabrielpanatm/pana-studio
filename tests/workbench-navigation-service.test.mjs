import assert from "node:assert/strict";
import { test } from "node:test";
import { WorkbenchNavigationService } from "$lib/workbench/navigation-service";

function dependencies({ documentSurface = "code", activeActivity = "editor" } = {}) {
  const calls = [];
  const shell = { centerView: "code" };
  const snapshot = {
    activeActivity,
    activeGroupId: "primary",
    split: "none",
    groups: [{
      groupId: "primary",
      activeDocumentId: "document:site",
      documents: [{
        documentId: "document:site",
        relativePath: "sass/site.scss",
        presentation: "code_only",
        surface: documentSurface,
      }],
    }],
  };
  const workbench = {
    snapshot,
    activeDocumentPresentation: "code_only",
    isHydrated: () => true,
    async apply(intent) {
      calls.push(`activity:${intent.activity}`);
      snapshot.activeActivity = intent.activity;
    },
    async setActiveDocumentSurface(path, view) {
      calls.push(`surface:${path}:${view}`);
    },
  };
  return {
    calls,
    shell,
    service: new WorkbenchNavigationService({
      shell,
      workbench,
      project: { runtimeSessionId: "runtime", project: {}, root: "/project", epoch: 1 },
      documents: { activeScannedPath: "sass/site.scss" },
      source: { requestSelectionReveal: () => calls.push("reveal") },
      status: { set() {}, clear() {}, escalate() {} },
      flushDrafts: async () => { calls.push("flush"); },
      projectLatestPreview: async () => { calls.push("preview"); },
    }),
  };
}

test("aceeași vedere canonică nu persistă din nou suprafața documentului", async () => {
  const { calls, service } = dependencies();

  assert.equal(await service.setCenterView("code"), true);
  assert.deepEqual(calls, []);
});

test("aceeași vedere repară suprafața Rust numai când documentul activ diferă", async () => {
  const { calls, service } = dependencies({ documentSurface: "visual" });

  assert.equal(await service.setCenterView("code"), true);
  assert.deepEqual(calls, ["surface:sass/site.scss:code"]);
});

test("aceeași vedere nu ascunde o activitate Workbench nealiniată", async () => {
  const { calls, service } = dependencies({ activeActivity: "audit" });

  assert.equal(await service.setCenterView("code"), true);
  assert.deepEqual(calls, ["activity:editor", "surface:sass/site.scss:code"]);
});

test("vizualul cerut pentru un document code-only este normalizat la cod", async () => {
  const { calls, shell, service } = dependencies();

  assert.equal(await service.setCenterView("preview"), true);
  assert.equal(shell.centerView, "code");
  assert.deepEqual(calls, []);
});
