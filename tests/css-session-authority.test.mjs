import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, test } from "node:test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  createCssRequestIdentity,
  cssRequestIdentityMatches,
  getScssVariables,
  resolveCssInspectorContext,
  setCssRuleAtViewport,
  setScssVariable,
} from "$lib/project/io";
import { hashFileBufferText } from "$lib/session/file-buffer-draft-sync";
import {
  CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
  PROJECT_WORKSPACE_SCHEMA_VERSION,
} from "$lib/types";

if (!globalThis.window) globalThis.window = globalThis;

const projectRoot = "/project";
const runtimeA = "stable-project:runtime-a";
const runtimeB = "stable-project:runtime-b";

afterEach(() => clearMocks());

function mutation(identity, overrides = {}) {
  const touchedFiles = ["sass/site.scss"];
  const contents = ".hero { color: red; }";
  const hash = hashFileBufferText(contents);
  const bytes = new TextEncoder().encode(contents).byteLength;
  const workspaceMutation = {
    schemaVersion: PROJECT_WORKSPACE_SCHEMA_VERSION,
    changed: true,
    revisionBefore: 4,
    revisionAfter: 5,
    dirty: true,
    touchedFiles,
    transactionId: "workspace-css-5",
    files: [{
      relativePath: touchedFiles[0],
      currentHash: hash,
      currentBytes: bytes,
      revision: 2,
      dirty: true,
    }],
  };
  return {
    projectRoot: identity.expectedProjectRoot,
    runtimeSessionId: identity.expectedSessionId,
    workspaceRevision: 5,
    payload: null,
    authority: {
      schemaVersion: 2,
      operationId: "css-workspace-1",
      status: "staged",
      projectRoot: identity.expectedProjectRoot,
      sessionId: identity.expectedSessionId,
      revisionBefore: 4,
      revisionAfter: 5,
      dirty: true,
      touchedFiles,
      writtenFiles: [{ relativePath: touchedFiles[0], contents }],
      removedFiles: [],
      documents: [{
        relativePath: touchedFiles[0],
        snapshot: {
          relativePath: touchedFiles[0],
          text: contents,
          dirty: true,
          hash,
          bytes,
          revision: 2,
        },
      }],
      workspaceMutation,
      ...overrides,
    },
  };
}

test("CSS read receipt from another runtime is rejected", async () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  mockIPC((command, args) => {
    assert.equal(command, "get_scss_variables");
    assert.deepEqual(args.identity, identity);
    return {
      projectRoot,
      runtimeSessionId: runtimeB,
      payload: [],
    };
  });

  await assert.rejects(getScssVariables(identity), /\[css_stale_receipt\]/);
});

test("CSS mutation is staged in the captured ProjectWorkspace session", async () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  const calls = [];
  mockIPC((command, args) => {
    calls.push({ command, args });
    return mutation(identity);
  });

  const receipt = await setScssVariable(
    "sass/site.scss",
    "accent",
    "red",
    identity,
  );

  assert.equal(receipt.authority.status, "staged");
  assert.equal(receipt.authority.workspaceMutation.revisionAfter, 5);
  assert.deepEqual(calls, [{
    command: "set_scss_variable",
    args: {
      relativePath: "sass/site.scss",
      name: "accent",
      value: "red",
      identity,
    },
  }]);
  assert.equal("acceptedManifest" in receipt.authority, false);
  assert.equal("acceptedDiskGenerationAfter" in receipt.authority, false);
});

test("CSS mutation rejects stale and internally inconsistent authority", async () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  mockIPC((_command, _args) => mutation(identity, { sessionId: runtimeB }));
  await assert.rejects(
    setCssRuleAtViewport({
      relativePath: "sass/site.scss",
      selector: ".hero",
      properties: { color: "red" },
      viewport: "desktop",
    }, identity),
    /\[css_stale_receipt\]/,
  );

  clearMocks();
  const inconsistentRevision = mutation(identity, { revisionAfter: 7 });
  inconsistentRevision.workspaceRevision = 7;
  mockIPC(() => inconsistentRevision);
  await assert.rejects(
    setScssVariable("sass/site.scss", "accent", "blue", identity),
    /\[css_invalid_authority_receipt\]/,
  );

  clearMocks();
  const inconsistent = mutation(identity);
  inconsistent.authority.documents[0].snapshot.text = ".hero { color: blue; }";
  mockIPC(() => inconsistent);
  await assert.rejects(
    setScssVariable("sass/site.scss", "accent", "blue", identity),
    /\[css_invalid_authority_receipt\]/,
  );
});

test("Inspector CSS mutation forwards the captured semantic selection identity", async () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  const expectedSelection = {
    selectionRevision: 17,
    editorNodeId: "editor:hero",
    sourceNodeId: "source:hero",
    renderInstanceId: "render:hero",
  };
  mockIPC((command, args) => {
    assert.equal(command, "set_css_rule_at_viewport");
    assert.deepEqual(args.expectedSelection, expectedSelection);
    return mutation(identity);
  });

  await setCssRuleAtViewport({
    relativePath: "sass/site.scss",
    selector: ".hero",
    properties: { color: "red" },
    viewport: "desktop",
    expectedSelection,
  }, identity);
});

test("CSS inspector resolution is bound to one workspace and semantic selection revision", async () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  const expectedSelection = {
    selectionRevision: 17,
    editorNodeId: "editor:hero",
    sourceNodeId: "source:hero",
    renderInstanceId: "render:hero",
  };
  const ruleContext = {
    file: "sass/site.scss",
    selector: ".hero",
    viewport: "desktop",
    resolvedBreakpoint: null,
    baseRules: [{ property: "color", value: "red" }],
    viewportRules: [{ property: "color", value: "red" }],
    hasBaseRule: true,
    hasViewportRule: true,
    background: {
      schemaVersion: 1,
      color: null,
      layers: [],
      shorthand: null,
      opaqueProperties: {},
      structurallyEditable: true,
    },
    grid: {
      schemaVersion: 1,
      display: null,
      templateColumns: { raw: null, mode: "none", tracks: [], structurallyEditable: true },
      templateRows: { raw: null, mode: "none", tracks: [], structurallyEditable: true },
      templateAreas: { raw: null, rows: [], valid: true, error: null, structurallyEditable: true },
      autoColumns: null,
      autoRows: null,
      autoFlow: null,
      columnGap: null,
      rowGap: null,
      justifyContent: null,
      alignContent: null,
      justifyItems: null,
      alignItems: null,
      itemColumn: null,
      itemRow: null,
      itemArea: null,
      opaqueProperties: {},
      structurallyEditable: true,
    },
  };
  const target = {
    file: "sass/site.scss",
    selector: ".hero",
    targetKind: "existing",
    exists: true,
    linked: false,
    href: null,
    templatePath: "templates/index.html",
    pageOwned: false,
    consumerFiles: [],
    consumerTemplates: [],
    reason: "existing",
  };
  mockIPC((command, args) => {
    assert.equal(command, "resolve_css_inspector_context");
    assert.deepEqual(args, {
      templatePath: "templates/index.html",
      selector: ".hero",
      viewport: "desktop",
      fallbackFile: "sass/site.scss",
      expectedWorkspaceRevision: 4,
      expectedSelection,
      identity,
    });
    return {
      projectRoot,
      runtimeSessionId: runtimeA,
      workspaceRevision: 4,
      payload: {
        schemaVersion: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
        selectionRevision: 17,
        selector: ".hero",
        viewport: "desktop",
        state: "existing",
        target,
        ruleContext,
        candidates: [{ file: "sass/site.scss", ruleContext }],
      },
    };
  });

  const resolution = await resolveCssInspectorContext({
    templatePath: "templates/index.html",
    selector: ".hero",
    viewport: "desktop",
    fallbackFile: "sass/site.scss",
    expectedWorkspaceRevision: 4,
    expectedSelection,
  }, identity);
  assert.equal(resolution.target.file, "sass/site.scss");

  clearMocks();
  mockIPC(() => ({
    projectRoot,
    runtimeSessionId: runtimeA,
    workspaceRevision: 4,
    payload: {
      schemaVersion: CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION,
      selectionRevision: 18,
      selector: ".hero",
      viewport: "desktop",
      state: "existing",
      target,
      ruleContext,
      candidates: [{ file: "sass/site.scss", ruleContext }],
    },
  }));
  await assert.rejects(
    resolveCssInspectorContext({
      templatePath: "templates/index.html",
      selector: ".hero",
      viewport: "desktop",
      fallbackFile: "sass/site.scss",
      expectedWorkspaceRevision: 4,
      expectedSelection,
    }, identity),
    /\[css_inspector_invalid_receipt\]/,
  );
});

test("CSS identity distinguishes same-root runtime replacements", () => {
  const identity = createCssRequestIdentity(projectRoot, runtimeA);
  assert.equal(cssRequestIdentityMatches(identity, projectRoot, runtimeA), true);
  assert.equal(cssRequestIdentityMatches(identity, projectRoot, runtimeB), false);
  assert.equal(cssRequestIdentityMatches(identity, "/project-b", runtimeA), false);
});

test("CSS panel presents session staging and Save as the only disk boundary", () => {
  const source = readFileSync(
    new URL("../src/lib/components/InspectorPane.svelte", import.meta.url),
    "utf8",
  );
  assert.match(source, /t\("inspector-css-session-saved"/);
  assert.doesNotMatch(source, /acceptedDiskGeneration|acceptedManifest|InternalWriteEvidence/);
});
