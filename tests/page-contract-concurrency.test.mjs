import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { reconcilePageComponentContracts } from "$lib/page-components/contract";
import { reconcilePageAssetContracts } from "$lib/page-assets/contract";
import {
  resetFileBufferDraftSyncState,
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";
import {
  resetPageJsDraftSyncState,
  setPageJsDraftSyncSession,
} from "$lib/session/page-js-draft-sync";
import { runInPageContractLane } from "$lib/page-contracts/projection";

if (!globalThis.window) globalThis.window = globalThis;

afterEach(() => {
  clearMocks();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
});

function deferred() {
  let resolve;
  const promise = new Promise((resolvePromise) => { resolve = resolvePromise; });
  return { promise, resolve };
}

function authority(status, revisionBefore, revisionAfter, overrides = {}) {
  return {
    schemaVersion: 2,
    operationId: `page-contract-${revisionBefore}-${revisionAfter}`,
    status,
    projectRoot: "/project",
    sessionId: "session:runtime",
    revisionBefore,
    revisionAfter,
    dirty: status === "staged",
    consumedSources: [],
    touchedFiles: status === "staged" ? ["sursa/templates/index.html"] : [],
    ...overrides,
  };
}

function workspaceMutation(revisionBefore, revisionAfter) {
  return {
    schemaVersion: 1,
    changed: true,
    revisionBefore,
    revisionAfter,
    dirty: true,
    touchedFiles: ["sursa/templates/index.html"],
  };
}

function componentReceipt(overrides = {}) {
  return {
    plan: {
      templatePath: "templates/index.html",
      stylesheetPath: "sursa/sass/pagini/index.scss",
      stylesheetHref: "/pagini/index.css",
      activeComponentIds: ["counter"],
      template: { changed: true, contents: "<main><div data-component=\"counter\"></div></main>" },
      stylesheet: { changed: false, contents: "" },
      pageJsConfig: { version: 1 },
      pageJsChanged: false,
      previewCss: ".counter { display: block; }",
      diagnostics: [],
    },
    workspaceMutation: workspaceMutation(10, 11),
    pageJs: null,
    authority: authority("staged", 10, 11),
    ...overrides,
  };
}

function assetNoopReceipt() {
  return {
    plan: {
      templatePath: "templates/index.html",
      stylesheetPath: "sursa/sass/pagini/index.scss",
      stylesheetHref: "/pagini/index.css",
      activeDataAnimIds: [],
      activeGeneratedClasses: [],
      template: { changed: false, contents: "<main></main>" },
      stylesheet: { changed: false, contents: "" },
      pageJsConfig: { version: 1 },
      pageJsChanged: false,
      diagnostics: [],
    },
    workspaceMutation: null,
    pageJs: null,
    authority: authority("noop", 20, 20),
  };
}

function contractHost() {
  setFileBufferDraftSyncSession("/project", "session:runtime");
  setPageJsDraftSyncSession("/project", "session:runtime");
  return {
    sourceCache: { "sursa/templates/index.html": "<main></main>" },
    activeScannedPath: "sursa/templates/index.html",
    source: "<main></main>",
    sessionProjectRoot: "/project",
    kernelProjectSessionId: "session:runtime",
    projectSessionEpoch: 4,
    previewMessages: [],
    statuses: [],
    postPreviewMessage(message) { this.previewMessages.push(message); },
    setGlobalStatus(text, kind) { this.statuses.push({ text, kind }); },
  };
}

test("a Page Contract receipt cannot overwrite text edited while Rust is in flight", async () => {
  const started = deferred();
  const gate = deferred();
  mockIPC(async (command) => {
    if (command !== "apply_page_component_contract") {
      throw new Error(`Comandă IPC neașteptată: ${command}`);
    }
    started.resolve();
    await gate.promise;
    return componentReceipt();
  });
  const host = contractHost();
  const operation = reconcilePageComponentContracts(host, {
    file: "sursa/templates/index.html",
    line: 1,
    column: 1,
  });
  await started.promise;
  host.source = "<main><h1>editare concurentă</h1></main>";
  host.sourceCache["sursa/templates/index.html"] = host.source;
  gate.resolve();
  await operation;

  assert.equal(host.source, "<main><h1>editare concurentă</h1></main>");
  assert.equal(host.previewMessages.length, 1);
  assert.deepEqual(host.previewMessages[0], {
    type: "set-live-style-css",
    id: "pana-component-contract-preview-css",
    css: ".counter { display: block; }",
    refreshSelection: false,
  });
});

test("a receipt from project A has zero effects after same-root runtime replacement", async () => {
  const started = deferred();
  const gate = deferred();
  mockIPC(async () => {
    started.resolve();
    await gate.promise;
    return componentReceipt();
  });
  const host = contractHost();
  const operation = reconcilePageComponentContracts(host, {
    file: "sursa/templates/index.html",
    line: 1,
    column: 1,
  });
  await started.promise;
  host.kernelProjectSessionId = "session:replacement";
  host.projectSessionEpoch += 1;
  gate.resolve();

  await assert.rejects(operation, /sesiuni înlocuite|ProjectSession s-a schimbat/);
  assert.equal(host.source, "<main></main>");
  assert.deepEqual(host.previewMessages, []);
});

test("Page Asset sends intent only and accepts an exact workspace no-op", async () => {
  const calls = [];
  mockIPC((command, args) => {
    calls.push({ command, args });
    return assetNoopReceipt();
  });
  const host = contractHost();
  await reconcilePageAssetContracts(host, {
    file: "sursa/templates/index.html",
    line: 1,
    column: 1,
  });

  assert.deepEqual(calls, [{
    command: "apply_page_asset_contract",
    args: { input: {
      expectedProjectRoot: "/project",
      expectedSessionId: "session:runtime",
      templatePath: "templates/index.html",
    } },
  }]);
  assert.deepEqual(host.statuses, []);
});

test("Page Contract rejects impossible workspace authority before UI projection", async () => {
  mockIPC(() => componentReceipt({
    authority: authority("staged", 10, 10),
    workspaceMutation: null,
  }));
  const host = contractHost();
  await assert.rejects(
    reconcilePageComponentContracts(host, {
      file: "sursa/templates/index.html",
      line: 1,
      column: 1,
    }),
    /staged trebuie să avanseze revizia/,
  );
  assert.deepEqual(host.previewMessages, []);
});

test("Page Component and Page Asset share one FIFO per ProjectWorkspace session", async () => {
  const lease = { projectRoot: "/project", sessionId: "session:runtime", projectSessionEpoch: 4 };
  const firstGate = deferred();
  const firstStarted = deferred();
  const order = [];
  const first = runInPageContractLane(lease, "templates/a.html", async () => {
    order.push("first:start");
    firstStarted.resolve();
    await firstGate.promise;
    order.push("first:end");
  });
  const second = runInPageContractLane(lease, "templates/b.html", async () => {
    order.push("second:start");
    order.push("second:end");
  });
  await firstStarted.promise;
  assert.deepEqual(order, ["first:start"]);
  firstGate.resolve();
  await Promise.all([first, second]);
  assert.deepEqual(order, ["first:start", "first:end", "second:start", "second:end"]);
});
