import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

const project = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, project), "utf8");
const lines = (source) => source.split(/\r?\n/).length;

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const url = new URL(`${entry.name}${entry.isDirectory() ? "/" : ""}`, directory);
    if (entry.isDirectory()) return sourceFiles(url);
    return /\.(?:svelte|ts)$/.test(entry.name) ? [url] : [];
  });
}

test("composition root is narrow and the AppState compatibility path stays deleted", () => {
  assert.equal(existsSync(new URL("src/lib/state/app.svelte.ts", project)), false);

  const route = read("src/routes/+page.svelte");
  assert.ok(lines(route) <= 300, `route composition root has ${lines(route)} lines`);
  assert.match(route, /import ApplicationWorkspace/);
  assert.match(route, /<ApplicationWorkspace\s*\/>/);
  assert.doesNotMatch(route, /AppState|\$lib\/state\/app\.svelte|new ApplicationComposition/);

  for (const path of [
    "src/lib/application/composition.svelte.ts",
    "src/lib/components/application/ApplicationWorkspace.svelte",
    "src/lib/application/workspace-page-lifecycle.ts",
  ]) {
    const contents = read(path);
    assert.ok(lines(contents) <= 1_000, `${path} has ${lines(contents)} lines`);
    assert.doesNotMatch(contents, /\bAppState\b|\$lib\/state\/app\.svelte/);
  }
});

test("frontend sources expose no AppState façade and Vite emits acyclic core layers", () => {
  const offenders = sourceFiles(new URL("src/", project)).filter((url) => (
    /\bAppState\b|\$lib\/state\/app\.svelte|state\/app\.svelte\.ts/.test(readFileSync(url, "utf8"))
  ));
  assert.deepEqual(offenders.map((url) => url.pathname), []);

  const vite = read("vite.config.js");
  assert.doesNotMatch(vite, /pana-state/);
  for (const chunk of [
    "pana-core-foundation",
    "pana-core-domain",
    "pana-core-runtime",
    "pana-core-orchestration",
    "pana-application-shell",
  ]) assert.match(vite, new RegExp(chunk));
  assert.match(vite, /createSourceLayerChunkMap/);
});

test("startup keeps the heavy workspace surfaces outside the initial graph", () => {
  const application = read("src/lib/components/application/ApplicationWorkspace.svelte");
  const boundary = read("src/lib/application/workspace-surfaces.ts");
  const surfaces = [
    ["ActivityRail", "$lib/components/workbench/ActivityRail.svelte"],
    ["WorkspaceCenterArea", "$lib/components/workspace/WorkspaceCenterArea.svelte"],
    ["WorkspaceInspectorArea", "$lib/components/workspace/WorkspaceInspectorArea.svelte"],
    ["WorkspaceProjectArea", "$lib/components/workspace/WorkspaceProjectArea.svelte"],
  ];

  for (const [component, modulePath] of surfaces) {
    const escapedPath = modulePath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    assert.doesNotMatch(
      application,
      new RegExp(`import\\s+${component}\\s+from\\s+["']${escapedPath}["']`),
      `${component} must not be a static startup dependency`,
    );
    assert.match(
      boundary,
      new RegExp(`import\\(\\s*["']${escapedPath}["']\\s*\\)`),
      `${component} must be loaded through its workspace boundary`,
    );
  }
  assert.match(application, /import \{[\s\S]*loadWorkspaceSurfaces[\s\S]*from "\$lib\/application\/workspace-surfaces"/);
  assert.match(
    application,
    /projectSession\.project \|\| shell\.surface === "settings"[\s\S]*ensureWorkspaceSurfacesLoaded\(\)/,
  );
});

test("oversized legacy controller adapters are explicit, bounded and behaviorally covered", () => {
  const justifiedAdapters = [
    ["src/lib/state/project-preview-bootstrap-controller.ts", "ProjectPreviewBootstrapHost", 31, "tests/project-transition-controller.test.mjs"],
    ["src/lib/state/project-attachment-controller.ts", "ProjectAttachmentHost", 28, "tests/project-transition-controller.test.mjs"],
    ["src/lib/state/project-document-controller.ts", "ProjectDocumentHost", 27, "tests/project-document-workspace-state.test.mjs"],
    ["src/lib/state/project-template-workbench-controller.ts", "ProjectTemplateWorkbenchHost", 25, "tests/template-workbench-session.test.mjs"],
    ["src/lib/state/project-transition-controller.ts", "ProjectTransitionStateHost", 18, "tests/project-transition-controller.test.mjs"],
    ["src/lib/state/project-derived-state-controller.ts", "ProjectDerivedStateHost", 17, "tests/project-lifecycle-initialization.test.mjs"],
  ];

  for (const [path, typeName, maximumMembers, behavioralTest] of justifiedAdapters) {
    const contents = read(path);
    const tree = ts.createSourceFile(path, contents, ts.ScriptTarget.Latest, true);
    const declaration = tree.statements.find((statement) => (
      ts.isTypeAliasDeclaration(statement) && statement.name.text === typeName
    ));
    assert.ok(declaration && ts.isTypeLiteralNode(declaration.type), `${typeName} missing`);
    assert.ok(
      declaration.type.members.length <= maximumMembers,
      `${typeName} grew from its transaction-adapter ceiling of ${maximumMembers}`,
    );
    assert.doesNotMatch(contents, /\bAppState\b|\$lib\/state\/app\.svelte/);
    assert.equal(existsSync(new URL(behavioralTest, project)), true, behavioralTest);
  }
});

test("document navigation preserves the explicit no-resync load contract", () => {
  const path = "src/lib/components/application/ApplicationWorkspace.svelte";
  assert.match(
    read(path),
    /loadProjectFile:\s*\(file, options\)\s*=>\s*projectDocuments\.load\(file, options\)/,
    `${path} must forward syncWorkbench: false instead of reopening the document`,
  );
});
