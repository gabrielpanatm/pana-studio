import assert from "node:assert/strict";
import { test } from "node:test";
import {
  applyStartupProject,
  cancelStartupCreationPlan,
  openProjectFolder,
  planStartupProject,
  retryStartupProjectOpen,
  selectStartupCreationOption,
} from "$lib/state/project-startup-controller";

function candidate(kind = "valid_project") {
  return {
    root: "/project-a",
    displayName: "Project A",
    kind,
    snapshotToken: "snapshot-a",
    entryCount: 1,
    truncated: false,
    diagnostics: [],
  };
}

function startupFlow(nextCandidate = null) {
  return {
    schemaVersion: 1,
    revision: 1,
    stage: nextCandidate ? "ready" : "idle",
    candidate: nextCandidate,
    diagnostics: [],
  };
}

function catalog() {
  return {
    schemaVersion: 1,
    registryVersion: "test",
    embeddedZolaVersion: "0.20",
    expectedSnapshotToken: "snapshot-a",
    options: [{
      id: "minimal",
      kind: "minimal",
      name: "Minimal",
      description: "Minimal project",
      previewDataUrl: null,
      compatibilityLabel: "Compatible",
      capabilities: [],
    }],
  };
}

function creationPlan() {
  return {
    schemaVersion: 1,
    expectedSnapshotToken: "snapshot-a",
    planToken: "plan-a",
    projectRoot: "/project-a",
    optionId: "minimal",
    optionKind: "minimal",
    optionName: "Minimal",
    affectedFiles: ["config.toml"],
    totalBytes: 10,
    diagnostics: [],
  };
}

function host(overrides = {}) {
  return {
    startupFlow: startupFlow(),
    startupCreationCatalog: null,
    startupCreationPlan: null,
    startupSelectedOptionId: null,
    startupPending: false,
    startupError: "",
    notifications: [],
    cleared: [],
    escalateGlobalStatus(notification) {
      this.notifications.push(notification);
    },
    clearNotification(id) {
      this.cleared.push(id);
    },
    ...overrides,
  };
}

function dependencies(overrides = {}) {
  return {
    async chooseFolder() { return "/project-a"; },
    async inspectFolder() { return startupFlow(candidate()); },
    async readCreationCatalog() { return catalog(); },
    async planCreation() { return creationPlan(); },
    async applyCreation() {
      return {
        schemaVersion: 1,
        projectRoot: "/project-a",
        optionId: "minimal",
        planToken: "plan-a",
        publishedFiles: ["config.toml"],
        validation: "valid",
        startup: startupFlow(candidate()),
      };
    },
    async nextRender() {},
    ...overrides,
  };
}

test("startup deschide numai candidatul valid inspectat", async () => {
  const current = host({
    startupCreationCatalog: catalog(),
    startupCreationPlan: creationPlan(),
    startupSelectedOptionId: "minimal",
  });
  const opened = [];

  await openProjectFolder(
    current,
    async (root, options) => opened.push({ root, options }),
    dependencies(),
  );

  assert.equal(current.startupPending, false);
  assert.equal(current.startupError, "");
  assert.equal(current.startupCreationCatalog, null);
  assert.equal(current.startupCreationPlan, null);
  assert.equal(current.startupSelectedOptionId, null);
  assert.equal(opened.length, 1);
  assert.equal(opened[0].root, "/project-a");
  assert.equal(opened[0].options.startupCandidate.snapshotToken, "snapshot-a");
});

test("startup proiectează catalogul numai pentru directorul gol inspectat", async () => {
  const emptyCandidate = candidate("empty_directory");
  const current = host();
  let opened = false;

  await openProjectFolder(
    current,
    async () => { opened = true; },
    dependencies({
      async inspectFolder() { return startupFlow(emptyCandidate); },
    }),
  );

  assert.equal(opened, false);
  assert.equal(current.startupCreationCatalog.expectedSnapshotToken, "snapshot-a");
  assert.equal(current.startupPending, false);
});

test("anularea dialogului nu inspectează și lasă startup-ul terminal", async () => {
  const current = host({ startupError: "old" });
  let inspections = 0;
  await openProjectFolder(current, async () => {}, dependencies({
    async chooseFolder() { return null; },
    async inspectFolder() { inspections += 1; return startupFlow(); },
  }));

  assert.equal(inspections, 0);
  assert.equal(current.startupPending, false);
  assert.equal(current.startupError, "");
});

test("eroarea de inspecție este proiectată și pending este eliberat", async () => {
  const current = host();
  await openProjectFolder(current, async () => {}, dependencies({
    async inspectFolder() { throw new Error("inspection failed"); },
  }));

  assert.equal(current.startupPending, false);
  assert.equal(current.startupError, "inspection failed");
  assert.equal(current.notifications.at(-1).id, "startup.folder.error");
});

test("select-plan-cancel folosește exact snapshot-ul și opțiunea inspectate", async () => {
  const current = host({
    startupFlow: startupFlow(candidate("empty_directory")),
    startupCreationCatalog: catalog(),
  });
  const requests = [];

  selectStartupCreationOption(current, "foreign");
  assert.equal(current.startupSelectedOptionId, null);
  selectStartupCreationOption(current, "minimal");
  await planStartupProject(current, {
    async planCreation(request) {
      requests.push(request);
      return creationPlan();
    },
  });

  assert.deepEqual(requests, [{
    expectedSnapshotToken: "snapshot-a",
    optionId: "minimal",
  }]);
  assert.equal(current.startupCreationPlan.planToken, "plan-a");
  assert.equal(current.startupPending, false);

  cancelStartupCreationPlan(current);
  assert.equal(current.startupCreationPlan, null);
  assert.equal(current.startupError, "");
});

test("apply publică receipt-ul înainte de deschiderea proiectului creat", async () => {
  const current = host({
    startupFlow: startupFlow(candidate("empty_directory")),
    startupCreationCatalog: catalog(),
    startupCreationPlan: creationPlan(),
    startupSelectedOptionId: "minimal",
  });
  const events = [];

  await applyStartupProject(
    current,
    async (root, options) => {
      events.push({ root, token: options.startupCandidate.snapshotToken });
      assert.equal(current.startupCreationPlan, null);
      assert.equal(current.startupCreationCatalog, null);
    },
    dependencies({
      async applyCreation(request) {
        assert.deepEqual(request, {
          expectedSnapshotToken: "snapshot-a",
          expectedPlanToken: "plan-a",
        });
        return dependencies().applyCreation();
      },
    }),
  );

  assert.deepEqual(events, [{ root: "/project-a", token: "snapshot-a" }]);
  assert.equal(current.startupSelectedOptionId, null);
  assert.equal(current.startupPending, false);
});

test("retry refuză candidatul stale și raportează eșecul callback-ului autoritar", async () => {
  const stale = host();
  let opens = 0;
  await retryStartupProjectOpen(stale, async () => { opens += 1; });
  assert.equal(opens, 0);
  assert.match(stale.startupError, /nu mai este disponibil/);

  const current = host({ startupFlow: startupFlow(candidate()) });
  await retryStartupProjectOpen(
    current,
    async () => { throw new Error("open failed"); },
    { async nextRender() {} },
  );
  assert.equal(current.startupPending, false);
  assert.equal(current.startupError, "open failed");
  assert.equal(current.notifications.at(-1).id, "startup.folder.error");
});
