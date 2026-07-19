import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  createPageStructure,
  createReusablePartial,
} from "$lib/source-graph/template-actions";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => clearMocks());

const lease = {
  projectRoot: "/project",
  sessionId: "session:runtime",
  projectSessionEpoch: 3,
};

function workspaceMutation(revisionBefore, revisionAfter, touchedFiles) {
  return {
    schemaVersion: 1,
    changed: true,
    revisionBefore,
    revisionAfter,
    dirty: true,
    touchedFiles,
  };
}

function stagedAuthority(touchedFiles, overrides = {}) {
  return {
    schemaVersion: 1,
    operationId: "site-structure-1",
    status: "staged",
    projectRoot: "/project",
    sessionId: "session:runtime",
    revisionBefore: 4,
    revisionAfter: 5,
    dirty: true,
    touchedFiles,
    ...overrides,
  };
}

function pageReceipt(overrides = {}) {
  const touchedFiles = ["sursa/content/despre.md", "sursa/templates/page.html"];
  return {
    slug: "despre",
    contentPath: "sursa/content/despre.md",
    templatePath: "sursa/templates/page.html",
    pageTemplate: "page.html",
    origin: "local",
    themeName: null,
    created: touchedFiles,
    workspaceMutation: workspaceMutation(4, 5, touchedFiles),
    authority: stagedAuthority(touchedFiles),
    ...overrides,
  };
}

function activeHost(overrides = {}) {
  return {
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 3,
    projectTransitionFrontendLeaseActive: false,
    kernelUndoRedoFrontendLeaseActive: false,
    async beginPreviewStructuralWriteBoundary() {},
    endPreviewStructuralWriteBoundary() {},
    projections: [],
    async projectCommittedSiteStructure(
      capturedLease,
      touchedFiles,
      workspaceRevision,
      preferredRelativePath,
    ) {
      this.projections.push({ capturedLease, touchedFiles, workspaceRevision, preferredRelativePath });
    },
    ...overrides,
  };
}

test("Site Structure sends one exact session identity and projects the staged revision", async () => {
  const calls = [];
  mockIPC((command, args) => {
    calls.push({ command, args });
    return pageReceipt();
  });
  const host = activeHost();

  const result = await createPageStructure(host, lease, {
    title: "Despre",
    slug: "despre",
    pageTemplateName: "page.html",
    draft: false,
  });

  assert.equal(result.contentPath, "sursa/content/despre.md");
  assert.deepEqual(calls, [{
    command: "create_site_page_structure",
    args: {
      input: {
        title: "Despre",
        slug: "despre",
        pageTemplateName: "page.html",
        draft: false,
        targetOrigin: "local",
        targetThemeName: null,
      },
      identity: {
        expectedProjectRoot: "/project",
        expectedSessionId: "session:runtime",
      },
    },
  }]);
  assert.deepEqual(host.projections, [{
    capturedLease: lease,
    touchedFiles: ["sursa/content/despre.md", "sursa/templates/page.html"],
    workspaceRevision: 5,
    preferredRelativePath: "sursa/content/despre.md",
  }]);
});

test("Site Structure rejects stale or non-canonical authority before projection", async () => {
  const host = activeHost();
  mockIPC(() => pageReceipt({
    authority: stagedAuthority(
      ["sursa/content/despre.md", "sursa/templates/page.html"],
      { sessionId: "session:replacement" },
    ),
  }));
  await assert.rejects(
    createPageStructure(host, lease, {
      title: "Despre",
      slug: "despre",
      pageTemplateName: "page.html",
      draft: false,
    }),
    /site_structure_invalid_authority_receipt/,
  );
  assert.deepEqual(host.projections, []);

  clearMocks();
  mockIPC(() => pageReceipt({
    authority: stagedAuthority([
      "sursa/templates/page.html",
      "sursa/content/despre.md",
    ]),
  }));
  await assert.rejects(
    createPageStructure(host, lease, {
      title: "Despre",
      slug: "despre",
      pageTemplateName: "page.html",
      draft: false,
    }),
    /site_structure_invalid_authority_receipt/,
  );
});

test("a Site Structure no-op does not manufacture a Preview projection", async () => {
  mockIPC(() => ({
    path: "sursa/templates/partials/card.html",
    templateName: "partials/card.html",
    origin: "local",
    themeName: null,
    created: false,
    workspaceMutation: null,
    authority: {
      schemaVersion: 1,
      operationId: "site-noop",
      status: "noop",
      projectRoot: "/project",
      sessionId: "session:runtime",
      revisionBefore: 8,
      revisionAfter: 8,
      dirty: false,
      touchedFiles: [],
    },
  }));
  const host = activeHost();
  const result = await createReusablePartial(host, lease, "card");
  assert.equal(result.created, false);
  assert.deepEqual(host.projections, []);
});

test("same-root runtime replacement after commit cannot project completion into runtime B", async () => {
  mockIPC(() => pageReceipt());
  const host = activeHost({
    async projectCommittedSiteStructure() {
      this.kernelProjectSessionId = "session:replacement";
      this.projectSessionEpoch += 1;
    },
  });
  await assert.rejects(
    createPageStructure(host, lease, {
      title: "Despre",
      slug: "despre",
      pageTemplateName: "page.html",
      draft: false,
    }),
    /ProjectSession s-a schimbat/,
  );
});

test("Site Workspace callback carries touched files and authoritative workspace revision", () => {
  const center = readFileSync(
    new URL("../src/lib/components/workspace/WorkspaceCenterArea.svelte", import.meta.url),
    "utf8",
  );
  const actions = readFileSync(
    new URL("../src/lib/source-graph/template-actions.ts", import.meta.url),
    "utf8",
  );
  assert.match(
    center,
    /projectCommittedSiteStructure=\{async \(lease, touchedFiles, workspaceRevision, preferredRelativePath\) =>/,
  );
  assert.match(center, /syncCommittedSiteStructurePreview\(app, lease, touchedFiles, workspaceRevision\)/);
  assert.match(actions, /minimumWorkspaceRevision: workspaceRevision/);
  assert.doesNotMatch(actions, /acceptedManifest|acceptedDiskGeneration|InternalWriteEvidence/);
});
